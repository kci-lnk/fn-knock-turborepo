import assert from "node:assert/strict";

export const scenarios = [
  "bootstrap",
  "challenge",
  "session_api",
  "session_hit",
  "session_miss",
  "grant_hit",
  "grant_miss",
  "grant_renewal",
  "auto_ip_hit",
  "auto_ip_miss",
];
export const marker = "auth-performance-owned-origin-v1";

export function benchmarkEnvironment(inherited, variant = {}) {
  const env = { ...inherited, ...variant };
  const capacity = "FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT";
  // Host shell settings must not silently replace either service's default.
  delete env[capacity];
  if (Object.hasOwn(variant, capacity)) env[capacity] = variant[capacity];
  return env;
}

export function requestSpec(scenario, index = 0, phase = "load") {
  assert.ok(scenarios.includes(scenario), `unknown scenario ${scenario}`);
  const headers = {
    host: "protected.authperf.test",
    accept: "text/html",
    "accept-encoding": "identity",
    "user-agent": "auth-performance",
  };
  let path = "/payload";
  let expected = "origin";
  if (scenario === "bootstrap" || scenario === "challenge") {
    path = `/__auth__/api/auth/${scenario}`;
    expected = scenario;
  } else if (scenario.startsWith("session")) {
    headers.cookie = `x-go-reauth-proxy-session-id=${scenario === "session_miss" ? `missing-${phase}-${index}` : "authperf-session-0"}`;
    if (scenario === "session_api") {
      path = "/__auth__/api/auth/session";
      expected = "session";
    }
    if (scenario === "session_miss") expected = "redirect";
  } else if (scenario.startsWith("grant")) {
    headers.host = "grant.authperf.test";
    const token =
      scenario === "grant_miss"
        ? `missing-${phase}-${index}`
        : scenario === "grant_renewal"
          ? `authperf-grant-${phase}-${index}`
          : "authperf-grant-hit";
    headers.cookie = `fn-knock-subdomain-rule-grant=${token}`;
    if (scenario === "grant_miss") expected = "redirect";
    if (scenario === "grant_renewal") expected = "renewal";
  } else if (scenario === "auto_ip_miss") expected = "redirect";
  return { path, headers, expected };
}

export function validateResponse(spec, response) {
  const { status, headers, body } = response;
  if (headers["x-fn-knock-upstream-error-class"])
    return `upstream_error:${headers["x-fn-knock-upstream-error-class"]}`;
  if (spec.expected === "redirect") {
    return status === 302 &&
      Boolean(headers.location) &&
      !headers["x-authperf-origin"]
      ? null
      : `expected login redirect, got ${status}`;
  }
  if (status !== 200) return `expected HTTP 200, got ${status}`;
  if (spec.expected === "origin" || spec.expected === "renewal") {
    if (headers["x-authperf-origin"] !== marker || body !== marker)
      return "response did not reach the owned business origin";
    if (
      spec.expected === "renewal" &&
      !(headers["set-cookie"] ?? []).some(
        (cookie) =>
          cookie.startsWith("fn-knock-subdomain-rule-grant=") &&
          !cookie.includes("Max-Age=0"),
      )
    )
      return "grant request succeeded without renewal Set-Cookie";
    return null;
  }
  try {
    const value = JSON.parse(body);
    if (spec.expected === "challenge")
      return typeof value.challenge === "string" &&
        typeof value.signature === "string"
        ? null
        : "invalid challenge payload";
    if (value.success !== true) return "auth API did not report success";
    if (spec.expected === "session" && value.data?.auth?.authenticated !== true)
      return "session not authenticated";
    if (
      spec.expected === "bootstrap" &&
      value.data?.client?.ip !== "198.18.0.1"
    )
      return `unexpected client IP ${value.data?.client?.ip}`;
    return null;
  } catch {
    return "invalid JSON body";
  }
}

export function percentileHistogram(counts, fraction) {
  const total = counts.reduce((sum, value) => sum + value, 0);
  if (!total) return null;
  let seen = 0;
  for (let index = 0; index < counts.length; index++) {
    seen += counts[index];
    if (seen >= Math.ceil(total * fraction)) return index;
  }
  return null;
}

export function mergeMeasurements(rows) {
  const histogram = new Array(60001).fill(0);
  const statuses = {};
  const anomalies = [];
  for (const row of rows) {
    row.histogram.forEach((value, index) => {
      histogram[index] += value;
    });
    for (const [key, count] of Object.entries(row.statuses))
      statuses[key] = (statuses[key] ?? 0) + count;
    anomalies.push(...row.anomalies);
  }
  const elapsed = Math.max(...rows.map((row) => row.elapsed_ms));
  const requests = rows.reduce((sum, row) => sum + row.requests, 0);
  const failures = rows.reduce((sum, row) => sum + row.failures, 0);
  return {
    elapsed_ms: elapsed,
    requests,
    failures,
    successful_requests: requests - failures,
    requests_per_second: (requests * 1000) / elapsed,
    successful_requests_per_second: ((requests - failures) * 1000) / elapsed,
    p50_ms: percentileHistogram(histogram, 0.5),
    p95_ms: percentileHistogram(histogram, 0.95),
    p99_ms: percentileHistogram(histogram, 0.99),
    latency_overflow: histogram[60000],
    statuses,
    anomalies: anomalies.slice(0, 20),
    client_event_loop_delay_max_ms: Math.max(
      ...rows.map((row) => row.event_loop_delay_max_ms),
    ),
    client_start_delay_max_ms: Math.max(
      ...rows.map((row) => row.start_delay_ms),
    ),
    all_tokens_consumed: rows.every((row) => row.all_tokens_consumed),
    histogram_ms: histogram
      .map((count, ms) => [ms, count])
      .filter(([, count]) => count > 0),
  };
}

export function pairOrder(pair) {
  return pair % 2 === 0 ? ["baseline", "candidate"] : ["candidate", "baseline"];
}

export function comparisonKey(run) {
  const seed = run.seed ?? {};
  const scale = [
    ["sessions", seed.sessions],
    ["accounts", seed.accounts],
    ["grants", seed.ordinary_grants],
    ["total-grants", seed.total_grants],
    ["renewals", seed.renewal_tokens_per_phase],
  ]
    .map(([name, value]) => `${name}-${value ?? "unknown"}`)
    .join("/");
  return `${run.scenario}/${run.concurrency}/${run.candidate}/cache-${run.cache_ttl_seconds ?? "unknown"}/profile-${run.profiling ? "on" : "off"}/recovery-${run.recovery_probe ? "on" : "off"}/credential-${seed.credential_kind ?? "unknown"}/${scale}`;
}

const median = (values) => {
  const a = [...values].sort((a, b) => a - b);
  return a.length % 2
    ? a[a.length >> 1]
    : (a[a.length / 2 - 1] + a[a.length / 2]) / 2;
};
function pairedInterval(values) {
  if (values.length < 6 || !values.every(Number.isFinite)) return null;
  let state = 23;
  const random = () => {
    state = (1664525 * state + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
  const medians = Array.from({ length: 10000 }, () =>
    median(values.map(() => values[Math.floor(random() * values.length)])),
  ).sort((a, b) => a - b);
  return [medians[250], medians[9750]];
}
export function compareRuns(runs) {
  const groups = new Map();
  for (const run of runs) {
    const key = comparisonKey(run);
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(run);
  }
  return [...groups].map(([key, values]) => {
    const pairs = new Map();
    let duplicateRuns = 0;
    for (const value of values) {
      if (!pairs.has(value.pair)) pairs.set(value.pair, {});
      const pair = pairs.get(value.pair);
      if (pair[value.role]) duplicateRuns++;
      else pair[value.role] = value;
    }
    const complete = [...pairs.values()].filter(
      (pair) => pair.baseline?.measurement && pair.candidate?.measurement,
    );
    const invalid = values.filter((value) => !value.validation?.passed).length;
    const positive = (value) => Number.isFinite(value) && value > 0;
    const ratios = (read) =>
      complete.map((pair) => {
        const baseline = read(pair.baseline);
        const candidate = read(pair.candidate);
        return positive(baseline) && positive(candidate)
          ? candidate / baseline - 1
          : null;
      });
    const throughput = ratios(
      (run) => run.measurement.successful_requests_per_second,
    );
    const p99 = ratios((run) => run.measurement.p99_ms);
    const rss = Object.fromEntries(
      ["go", "rust", "combined"].map((name) => [
        name,
        ratios((run) => {
          if (name !== "combined") return run.resources?.[name]?.peak_rss_bytes;
          const go = run.resources?.go?.peak_rss_bytes;
          const rust = run.resources?.rust?.peak_rss_bytes;
          return positive(go) && positive(rust) ? go + rust : null;
        }),
      ]),
    );
    const completeMedian = (values) =>
      values.length && values.every(Number.isFinite) ? median(values) : null;
    return {
      key,
      recovery_probe: values.some((run) => run.recovery_probe),
      complete_pairs: complete.length,
      incomplete_pairs: pairs.size - complete.length,
      invalid_runs: invalid,
      duplicate_runs: duplicateRuns,
      six_valid_pairs:
        invalid === 0 &&
        duplicateRuns === 0 &&
        complete.length >= 6 &&
        complete.length === pairs.size,
      throughput_change_median: completeMedian(throughput),
      p99_change_median: completeMedian(p99),
      throughput_change_bootstrap_95: pairedInterval(throughput),
      p99_change_bootstrap_95: pairedInterval(p99),
      paired_throughput_changes: throughput,
      paired_p99_changes: p99,
      rss_complete_pairs: rss.combined.filter(Number.isFinite).length,
      peak_rss_change_median: Object.fromEntries(
        Object.entries(rss).map(([name, values]) => [
          name,
          completeMedian(values),
        ]),
      ),
      paired_peak_rss_changes: rss,
    };
  });
}

export function comparisonFailures(comparison, options = {}) {
  const failures = [];
  if (!comparison.length) failures.push("no comparisons");
  const strict = options.requireSixPairs || options.requireImprovement;
  // Ratios such as 105/100 - 1 may exceed their exact threshold by an ulp.
  const atLeast = (value, threshold) =>
    Number.isFinite(value) && value >= threshold - 1e-12;
  const atMost = (value, threshold) =>
    Number.isFinite(value) && value <= threshold + 1e-12;
  for (const group of comparison) {
    const fail = (reason) => failures.push(`${group.key}: ${reason}`);
    if (group.invalid_runs !== 0) fail("invalid trials");
    if (group.duplicate_runs > 0) fail("duplicate pair/role trials");
    if (group.incomplete_pairs !== 0) fail("incomplete pairs");
    if (!strict) continue;
    if (group.recovery_probe)
      fail("recovery probes cannot support performance claims");
    if (!group.six_valid_pairs)
      fail("at least six valid complete pairs required");
    if (
      !atLeast(group.throughput_change_median, -0.05) ||
      !atMost(group.p99_change_median, 0.1)
    )
      fail(
        "missing performance data or throughput/P99 regression budget exceeded",
      );
    if (
      group.rss_complete_pairs !== group.complete_pairs ||
      !Number.isFinite(group.peak_rss_change_median?.combined)
    )
      fail("missing or invalid Go/Rust peak RSS data");
    else if (!atMost(group.peak_rss_change_median.combined, 0.05))
      fail("combined Go+Rust peak RSS median growth exceeded 5%");
    if (options.requireImprovement) {
      const throughput =
        atLeast(group.throughput_change_median, 0.1) &&
        Number.isFinite(group.throughput_change_bootstrap_95?.[0]) &&
        group.throughput_change_bootstrap_95[0] > 0;
      const p99 =
        atMost(group.p99_change_median, -0.15) &&
        Number.isFinite(group.p99_change_bootstrap_95?.[1]) &&
        group.p99_change_bootstrap_95[1] < 0;
      if (!throughput && !p99)
        fail(
          "improvement target not established: throughput >=10% with paired 95% lower bound >0, or P99 <=-15% with paired 95% upper bound <0 required",
        );
    }
  }
  return failures;
}
