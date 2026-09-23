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
const median = (values) => {
  const a = [...values].sort((a, b) => a - b);
  return a.length % 2
    ? a[a.length >> 1]
    : (a[a.length / 2 - 1] + a[a.length / 2]) / 2;
};
function pairedInterval(values) {
  if (values.length < 6) return null;
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
    const key = `${run.scenario}/${run.concurrency}/${run.candidate}/cache-${run.cache_ttl_seconds ?? "unknown"}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(run);
  }
  return [...groups].map(([key, values]) => {
    const pairs = new Map();
    for (const value of values) {
      if (!pairs.has(value.pair)) pairs.set(value.pair, {});
      pairs.get(value.pair)[value.role] = value;
    }
    const complete = [...pairs.values()].filter(
      (pair) => pair.baseline?.measurement && pair.candidate?.measurement,
    );
    const invalid = values.filter((value) => !value.validation?.passed).length;
    const ratios = (metric) =>
      complete.map(
        (pair) =>
          pair.candidate.measurement[metric] /
            pair.baseline.measurement[metric] -
          1,
      );
    const throughput = ratios("successful_requests_per_second");
    const p99 = ratios("p99_ms");
    return {
      key,
      complete_pairs: complete.length,
      incomplete_pairs: pairs.size - complete.length,
      invalid_runs: invalid,
      six_valid_pairs: invalid === 0 && complete.length >= 6,
      throughput_change_median: throughput.length ? median(throughput) : null,
      p99_change_median: p99.length ? median(p99) : null,
      throughput_change_bootstrap_95: pairedInterval(throughput),
      p99_change_bootstrap_95: pairedInterval(p99),
      paired_throughput_changes: throughput,
      paired_p99_changes: p99,
    };
  });
}
