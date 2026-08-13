import assert from "node:assert/strict";
import test from "node:test";
import {
  compareFrontendSummaries,
  summarizeFrontendRuns,
} from "../frontend-performance-lib.mjs";

test("frontend performance summary reports per-scenario p75", () => {
  const summary = summarizeFrontendRuns([
    { scenario: "dashboard", route_ready_ms: 100, long_task_total_ms: 0 },
    { scenario: "dashboard", route_ready_ms: 130, long_task_total_ms: 20 },
    { scenario: "dashboard", route_ready_ms: 110, long_task_total_ms: 10 },
    { scenario: "dashboard", route_ready_ms: 120, long_task_total_ms: 5 },
  ]);
  assert.deepEqual(summary.dashboard, {
    sample_count: 4,
    route_ready_p75_ms: 120,
    long_task_total_p75_ms: 10,
  });
});

test("frontend performance comparison enforces the ten-percent gate", () => {
  const base = {
    dashboard: { route_ready_p75_ms: 100, long_task_total_p75_ms: 20 },
  };
  assert.deepEqual(
    compareFrontendSummaries(base, {
      dashboard: { route_ready_p75_ms: 110, long_task_total_p75_ms: 22 },
    }),
    [],
  );
  const failures = compareFrontendSummaries(base, {
    dashboard: { route_ready_p75_ms: 111, long_task_total_p75_ms: 23 },
  });
  assert.equal(failures.length, 2);
});
