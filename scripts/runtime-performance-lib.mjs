export const percentile = (values, percentileValue) => {
  if (!Array.isArray(values) || values.length === 0) {
    throw new Error("cannot calculate a percentile without samples");
  }
  if (!(percentileValue > 0 && percentileValue <= 1)) {
    throw new Error("percentile must be greater than 0 and no greater than 1");
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
    throw new Error("every runtime performance sample must contain stable management RSS");
  }
  const gatewayRSS = isV2
    ? checkpointValues(samples, "stable_10s", "gateway_rss_bytes")
    : nonNullValues(samples, "gateway_rss_bytes");
  const loadPeakRSS = checkpointValues(samples, "load_peak", "gateway_rss_bytes");
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
  const proxyThroughput = loadValues(samples, "proxy_2mib", "requests_per_second");
  return {
    readiness_p95_ms: percentile(readiness, 0.95),
    management_rss_p95_bytes: percentile(managementRSS, 0.95),
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

export const compareRuntimeSummaries = (base, current, tolerances) => {
  const checks = [
    ["readiness_p95_ms", tolerances.readiness],
    ["management_rss_p95_bytes", tolerances.rss],
    ["gateway_rss_p95_bytes", tolerances.rss],
    ["proxy_2mib_rps_p50", tolerances.throughput, "minimum"],
  ];
  const failures = [];
  for (const [field, tolerance, direction = "maximum"] of checks) {
    const baseValue = base[field];
    const currentValue = current[field];
    if (baseValue == null && currentValue == null) continue;
    if (!(typeof baseValue === "number" && baseValue >= 0)) {
      failures.push(`${field} is missing from the baseline result`);
      continue;
    }
    if (!(typeof currentValue === "number" && currentValue >= 0)) {
      failures.push(`${field} is missing from the current result`);
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
  for (const field of [
    "gateway_load_peak_rss_p95_bytes",
    "gateway_post_load_rss_p95_bytes",
  ]) {
    const baseValue = base[field];
    const currentValue = current[field];
    if (baseValue == null && currentValue == null) continue;
    if (!(typeof baseValue === "number" && baseValue > 0)) {
      failures.push(`${field} is missing from the baseline result`);
      continue;
    }
    if (!(typeof currentValue === "number" && currentValue >= 0)) {
      failures.push(`${field} is missing from the current result`);
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
