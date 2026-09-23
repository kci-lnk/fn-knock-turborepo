// Read-only analysis of raw long-run samples. It does not infer Tokio task
// counts from native threads or infer missing goroutine metrics as zero.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const median = (values) => {
  const sorted = values.filter(Number.isFinite).sort((a, b) => a - b);
  if (!sorted.length) return null;
  const middle = sorted.length >> 1;
  return sorted.length % 2
    ? sorted[middle]
    : (sorted[middle - 1] + sorted[middle]) / 2;
};

export function gatewayRuntimeSample(snapshot) {
  const component = snapshot?.components?.gateway_process;
  if (!component) return null;
  return Object.fromEntries(
    [
      "pid",
      "last_checked_at",
      "goroutines",
      "heap_alloc_bytes",
      "heap_sys_bytes",
      "num_gc",
      "active_proxy_requests",
      "active_client_connections",
      "idle_client_connections",
      "open_upstream_connections",
    ].map((field) => [field, component[field] ?? null]),
  );
}

export function summarizeSoak(run, windowMs = 300000) {
  const duration = run.measurement?.elapsed_ms;
  assert.ok(Number.isFinite(duration) && duration > 0, "missing load duration");
  assert.ok(Number.isFinite(windowMs) && windowMs > 0, "invalid window");
  const samples = run.samples ?? [];
  assert.ok(samples.length, "raw process samples are required");
  const first = samples.filter((sample) => sample.elapsed_ms < windowMs);
  const last = samples.filter(
    (sample) => sample.elapsed_ms >= duration - windowMs,
  );
  const change = (a, b) =>
    Number.isFinite(a) && Number.isFinite(b) && a > 0 ? b / a - 1 : null;
  const processSummary = (name) => {
    const fields = ["rss_bytes", "fds", "threads"];
    return Object.fromEntries(
      fields.map((field) => {
        const read = (sample) => sample.processes?.[name]?.[field];
        const values = samples.map(read).filter(Number.isFinite);
        const a = median(first.map(read)),
          b = median(last.map(read));
        return [
          field,
          {
            samples: values.length,
            first_window_median: a,
            last_window_median: b,
            median_change: change(a, b),
            minimum: values.length ? Math.min(...values) : null,
            peak: values.length ? Math.max(...values) : null,
            before: run.resources?.[name]?.before?.[field] ?? null,
            after: run.resources?.[name]?.after?.[field] ?? null,
          },
        ];
      }),
    );
  };
  const health = run.runtime_health ?? [];
  const goroutines = health.filter((sample) =>
    Number.isFinite(sample.gateway_runtime?.goroutines),
  );
  const goFirst = median(
    goroutines
      .filter((sample) => sample.elapsed_ms < windowMs)
      .map((sample) => sample.gateway_runtime.goroutines),
  );
  const goLast = median(
    goroutines
      .filter((sample) => sample.elapsed_ms >= duration - windowMs)
      .map((sample) => sample.gateway_runtime.goroutines),
  );
  const buckets = [];
  for (let begin = 0; begin < duration; begin += windowMs) {
    const end = Math.min(begin + windowMs, duration);
    const bucket = samples.filter(
      (sample) => sample.elapsed_ms >= begin && sample.elapsed_ms < end,
    );
    buckets.push({
      from_ms: begin,
      to_ms: end,
      samples: bucket.length,
      processes: Object.fromEntries(
        ["go", "rust"].map((name) => [
          name,
          Object.fromEntries(
            ["rss_bytes", "fds", "threads"].map((field) => [
              field,
              median(bucket.map((sample) => sample.processes?.[name]?.[field])),
            ]),
          ),
        ]),
      ),
    });
  }
  return {
    schema_version: 1,
    candidate: run.candidate,
    scenario: run.scenario,
    role: run.role,
    elapsed_ms: duration,
    completed_30_minutes_without_request_or_quality_failure:
      duration >= 1800000 &&
      run.validation?.passed === true &&
      run.measurement.failures === 0,
    window_ms: windowMs,
    windows_overlap: duration < 2 * windowMs,
    successful_requests: run.measurement.successful_requests,
    failures: run.measurement.failures,
    quality: run.quality,
    processes: Object.fromEntries(
      ["go", "rust"].map((name) => [name, processSummary(name)]),
    ),
    go_goroutines: {
      samples: goroutines.length,
      first_window_median: goFirst,
      last_window_median: goLast,
      median_change: change(goFirst, goLast),
      peak: goroutines.length
        ? Math.max(
            ...goroutines.map((sample) => sample.gateway_runtime.goroutines),
          )
        : null,
    },
    rust_async_task_count: null,
    buckets,
    notes: [
      "RSS/FD/thread values are sampled during load, not after an idle GC window.",
      "Five-minute medians show resource evolution; this is not a proof of no leaks.",
      "Go goroutines come from the existing cached gateway_process health snapshot; repeated observations are not independent.",
      "Rust async task counts are unavailable; native thread counts do not substitute for them.",
      "The completion flag checks elapsed time, request correctness and existing harness quality gates, not a resource growth acceptance threshold.",
    ],
  };
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  assert.ok(
    process.argv[2],
    "usage: node scripts/auth-performance-soak.mjs raw-trial-result.json",
  );
  console.log(
    JSON.stringify(
      summarizeSoak(JSON.parse(await readFile(process.argv[2], "utf8"))),
      null,
      2,
    ),
  );
}
