import assert from "node:assert/strict";
import test from "node:test";
import {
  gatewayRuntimeSample,
  summarizeSoak,
} from "../auth-performance-soak.mjs";

function fixture() {
  return {
    candidate: "fixture",
    scenario: "auto_ip_hit",
    role: "candidate",
    measurement: {
      elapsed_ms: 1800000,
      successful_requests: 100000,
      failures: 0,
    },
    validation: { passed: true },
    samples: Array.from({ length: 1800 }, (_, second) => ({
      elapsed_ms: second * 1000,
      processes: {
        go: { rss_bytes: second < 900 ? 100 : 110, fds: 20, threads: 5 },
        rust: { rss_bytes: 200, fds: 30, threads: 9 },
      },
    })),
    runtime_health: Array.from({ length: 360 }, (_, index) => ({
      elapsed_ms: index * 5000,
      gateway_runtime: { goroutines: 30 + (index % 2) },
    })),
  };
}

test("soak reports separate early/late windows and resource evolution without treating growth as acceptance", () => {
  const summary = summarizeSoak(fixture());
  assert.equal(
    summary.completed_30_minutes_without_request_or_quality_failure,
    true,
  );
  assert.equal(summary.windows_overlap, false);
  assert.equal(summary.processes.go.rss_bytes.first_window_median, 100);
  assert.equal(summary.processes.go.rss_bytes.last_window_median, 110);
  assert.ok(
    Math.abs(summary.processes.go.rss_bytes.median_change - 0.1) < 1e-12,
  );
  assert.equal(summary.processes.rust.fds.median_change, 0);
  assert.equal(summary.go_goroutines.first_window_median, 30.5);
  assert.equal(summary.go_goroutines.median_change, 0);
  assert.equal(summary.go_goroutines.peak, 31);
  assert.equal(summary.buckets.length, 6);
  assert.equal(summary.buckets[3].processes.go.rss_bytes, 110);
  assert.equal(summary.rust_async_task_count, null);
});

test("short, failed or missing samples cannot claim a successful 30 minute run", () => {
  const run = fixture();
  run.measurement.elapsed_ms = 60000;
  let summary = summarizeSoak(run);
  assert.equal(
    summary.completed_30_minutes_without_request_or_quality_failure,
    false,
  );
  assert.equal(summary.windows_overlap, true);
  run.measurement.elapsed_ms = 1800000;
  run.validation.passed = false;
  assert.equal(
    summarizeSoak(run).completed_30_minutes_without_request_or_quality_failure,
    false,
  );
  run.validation.passed = true;
  run.measurement.failures = 1;
  assert.equal(
    summarizeSoak(run).completed_30_minutes_without_request_or_quality_failure,
    false,
  );
  delete run.runtime_health;
  summary = summarizeSoak(run);
  assert.equal(summary.go_goroutines.samples, 0);
  assert.equal(summary.go_goroutines.median_change, null);
  assert.equal(summary.go_goroutines.peak, null);
  delete run.samples;
  assert.throws(() => summarizeSoak(run), /raw process samples/);
});

test("gateway sampler preserves zero versus missing metrics and uses the process component", () => {
  assert.equal(gatewayRuntimeSample({}), null);
  const sample = gatewayRuntimeSample({
    components: {
      gateway_process: { pid: 42, goroutines: 0, active_proxy_requests: 0 },
      gateway_dataplane: { goroutines: 200 },
    },
  });
  assert.equal(sample.pid, 42);
  assert.equal(sample.goroutines, 0);
  assert.equal(sample.active_proxy_requests, 0);
  assert.equal(sample.heap_alloc_bytes, null);
});
