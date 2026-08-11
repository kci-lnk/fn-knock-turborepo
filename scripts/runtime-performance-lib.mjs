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
  const managementRSS = nonNullValues(samples, "management_rss_bytes");
  if (managementRSS.length !== samples.length) {
    throw new Error(
      "every runtime performance sample must contain management_rss_bytes",
    );
  }
  const gatewayRSS = nonNullValues(samples, "gateway_rss_bytes");
  return {
    readiness_p95_ms: percentile(readiness, 0.95),
    management_rss_p95_bytes: percentile(managementRSS, 0.95),
    gateway_rss_p95_bytes:
      gatewayRSS.length === samples.length
        ? percentile(gatewayRSS, 0.95)
        : null,
  };
};

export const compareRuntimeSummaries = (base, current, tolerances) => {
  const checks = [
    ["readiness_p95_ms", tolerances.readiness],
    ["management_rss_p95_bytes", tolerances.rss],
    ["gateway_rss_p95_bytes", tolerances.rss],
  ];
  const failures = [];
  for (const [field, tolerance] of checks) {
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
    if (
      baseValue === 0
        ? currentValue > 0
        : currentValue > baseValue * (1 + tolerance)
    ) {
      const change =
        baseValue === 0
          ? "increased from zero"
          : `regressed ${((currentValue / baseValue - 1) * 100).toFixed(1)}%`;
      failures.push(
        `${field} ${change} (${baseValue} -> ${currentValue}; limit ${(tolerance * 100).toFixed(1)}%)`,
      );
    }
  }
  return failures;
};
