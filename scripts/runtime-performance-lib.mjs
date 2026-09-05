export const percentile = (values, percentileValue) => {
  if (!Array.isArray(values) || values.length === 0) {
    throw new Error("cannot calculate a percentile without samples");
  }
  if (!(percentileValue > 0 && percentileValue <= 1)) {
    throw new Error("percentile must be greater than 0 and no greater than 1");
  }
  if (!values.every((value) => Number.isFinite(value) && value >= 0)) {
    throw new Error(
      "runtime performance samples must be finite non-negative numbers",
    );
  }
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.ceil(sorted.length * percentileValue) - 1];
};

const nonNullValues = (samples, field) =>
  samples
    .map((sample) => sample[field])
    .filter((value) => typeof value === "number" && Number.isFinite(value));

const checkpointValues = (samples, checkpoint, field) =>
  samples
    .map((sample) => sample.checkpoints?.[checkpoint]?.[field])
    .filter((value) => typeof value === "number" && Number.isFinite(value));

const loadValues = (samples, name, field) =>
  samples
    .flatMap((sample) => sample.loads ?? [])
    .filter((load) => load.name === name)
    .map((load) => load[field])
    .filter((value) => typeof value === "number" && Number.isFinite(value));

export const summarizeRuntimeSamples = (samples) => {
  if (!Array.isArray(samples) || samples.length === 0) {
    throw new Error("runtime performance requires at least one sample");
  }
  const readiness = nonNullValues(samples, "readiness_ms");
  if (readiness.length !== samples.length) {
    throw new Error(
      "every runtime performance sample must contain readiness_ms",
    );
  }
  const isV2 = samples.every((sample) => sample.checkpoints?.stable_10s);
  const managementRSS = isV2
    ? checkpointValues(samples, "stable_10s", "management_rss_bytes")
    : nonNullValues(samples, "management_rss_bytes");
  if (managementRSS.length !== samples.length) {
    throw new Error(
      "every runtime performance sample must contain stable management RSS",
    );
  }
  const gatewayRSS = isV2
    ? checkpointValues(samples, "stable_10s", "gateway_rss_bytes")
    : nonNullValues(samples, "gateway_rss_bytes");
  const loadPeakRSS = checkpointValues(
    samples,
    "load_peak",
    "gateway_rss_bytes",
  );
  const postLoadRSS = checkpointValues(
    samples,
    "post_load_30s",
    "gateway_rss_bytes",
  );
  const postReclaimRSS = checkpointValues(
    samples,
    "post_reclaim",
    "gateway_rss_bytes",
  );
  const proxyThroughput = loadValues(
    samples,
    "proxy_2mib",
    "requests_per_second",
  );
  const managementThroughput = loadValues(
    samples,
    "management_locale",
    "requests_per_second",
  );
  const managementLoadRSS = checkpointValues(
    samples,
    "load_peak",
    "management_rss_bytes",
  );
  const managementRetainedRSS = checkpointValues(
    samples,
    "post_load_30s",
    "management_rss_bytes",
  );
  const managementReclaimedRSS = checkpointValues(
    samples,
    "post_reclaim",
    "management_rss_bytes",
  );
  // Linux retains VmHWM after a short allocation burst has already ended.
  // Keep it separate from per-load RSS: it includes process startup as well.
  const managementLifetimeRSS = samples
    .map((sample) =>
      Math.max(
        ...Object.values(sample.checkpoints ?? {})
          .map((checkpoint) => checkpoint.management_peak_rss_bytes)
          .filter(
            (value) => typeof value === "number" && Number.isFinite(value),
          ),
      ),
    )
    .filter(Number.isFinite);
  return {
    readiness_p95_ms: percentile(readiness, 0.95),
    management_rss_p95_bytes: percentile(managementRSS, 0.95),
    management_lifetime_peak_rss_p95_bytes:
      managementLifetimeRSS.length > 0
        ? percentile(managementLifetimeRSS, 0.95)
        : null,
    management_load_peak_rss_p95_bytes:
      managementLoadRSS.length > 0 ? percentile(managementLoadRSS, 0.95) : null,
    management_post_load_rss_p95_bytes:
      managementRetainedRSS.length > 0
        ? percentile(managementRetainedRSS, 0.95)
        : null,
    management_post_reclaim_rss_p95_bytes:
      managementReclaimedRSS.length > 0
        ? percentile(managementReclaimedRSS, 0.95)
        : null,
    management_locale_rps_p50:
      managementThroughput.length > 0
        ? percentile(managementThroughput, 0.5)
        : null,
    gateway_rss_p95_bytes:
      gatewayRSS.length === samples.length
        ? percentile(gatewayRSS, 0.95)
        : null,
    gateway_load_peak_rss_p95_bytes:
      loadPeakRSS.length > 0 ? percentile(loadPeakRSS, 0.95) : null,
    gateway_post_load_rss_p95_bytes:
      postLoadRSS.length > 0 ? percentile(postLoadRSS, 0.95) : null,
    gateway_post_reclaim_rss_p95_bytes:
      postReclaimRSS.length > 0 ? percentile(postReclaimRSS, 0.95) : null,
    proxy_2mib_rps_p50:
      proxyThroughput.length > 0 ? percentile(proxyThroughput, 0.5) : null,
  };
};

export const compareRuntimeSummaries = (base, current, options = {}) => {
  const tolerances = {
    readiness: 0.1,
    rss: 0.05,
    throughput: 0.05,
    // Ordinary PRs must avoid regressions; an improvement target is opt-in.
    loadRssImprovement: 0,
    ...options,
  };
  const required = new Set(["readiness_p95_ms", "management_rss_p95_bytes"]);
  const gatewayLoadRssFields = [
    "gateway_load_peak_rss_p95_bytes",
    "gateway_post_load_rss_p95_bytes",
    "gateway_post_reclaim_rss_p95_bytes",
  ];
  const checks = [
    ["readiness_p95_ms", tolerances.readiness],
    ["management_rss_p95_bytes", tolerances.rss],
    ["management_lifetime_peak_rss_p95_bytes", tolerances.rss],
    ["management_load_peak_rss_p95_bytes", tolerances.rss],
    ["management_post_load_rss_p95_bytes", tolerances.rss],
    ["management_post_reclaim_rss_p95_bytes", tolerances.rss],
    ["management_locale_rps_p50", tolerances.throughput, "minimum"],
    ["gateway_rss_p95_bytes", tolerances.rss],
    ...gatewayLoadRssFields.map((field) => [field, tolerances.rss]),
    ["proxy_2mib_rps_p50", tolerances.throughput, "minimum"],
  ];
  const failures = [];
  const validImprovement =
    Number.isFinite(tolerances.loadRssImprovement) &&
    tolerances.loadRssImprovement >= 0 &&
    tolerances.loadRssImprovement <= 1;
  if (!validImprovement) {
    failures.push("load RSS has an invalid improvement requirement");
  }
  for (const [field, tolerance, direction = "maximum"] of checks) {
    const baseValue = base[field];
    const currentValue = current[field];
    if (baseValue == null && currentValue == null && !required.has(field))
      continue;
    if (!(Number.isFinite(baseValue) && baseValue >= 0)) {
      failures.push(`${field} is missing from the baseline result`);
      continue;
    }
    if (!(Number.isFinite(currentValue) && currentValue >= 0)) {
      failures.push(`${field} is missing from the current result`);
      continue;
    }
    if (!(Number.isFinite(tolerance) && tolerance >= 0 && tolerance <= 10)) {
      failures.push(`${field} has an invalid regression tolerance`);
      continue;
    }
    const regressed =
      direction === "minimum"
        ? baseValue > 0 && currentValue < baseValue * (1 - tolerance)
        : baseValue === 0
          ? currentValue > 0
          : currentValue > baseValue * (1 + tolerance);
    if (regressed) {
      const change =
        baseValue === 0
          ? "increased from zero"
          : `regressed ${((currentValue / baseValue - 1) * 100).toFixed(1)}%`;
      failures.push(
        `${field} ${change} (${baseValue} -> ${currentValue}; limit ${(tolerance * 100).toFixed(1)}%)`,
      );
    }
  }
  if (!validImprovement || tolerances.loadRssImprovement === 0) return failures;
  for (const field of gatewayLoadRssFields) {
    const baseValue = base[field];
    const currentValue = current[field];
    if (baseValue == null && currentValue == null) continue;
    // Missing/invalid values were already reported by the regression checks.
    if (
      !(Number.isFinite(baseValue) && baseValue >= 0) ||
      !(Number.isFinite(currentValue) && currentValue >= 0)
    )
      continue;
    if (baseValue === 0) {
      failures.push(`${field} cannot measure an improvement from a zero baseline`);
      continue;
    }
    const improvement = 1 - currentValue / baseValue;
    if (improvement + 1e-12 < tolerances.loadRssImprovement) {
      failures.push(
        `${field} improved ${(improvement * 100).toFixed(1)}% (${baseValue} -> ${currentValue}; required ${(tolerances.loadRssImprovement * 100).toFixed(1)}%)`,
      );
    }
  }
  return failures;
};
