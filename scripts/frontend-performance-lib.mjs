import { percentile } from "./runtime-performance-lib.mjs";

export const summarizeFrontendRuns = (runs) => {
  const scenarios = {};
  for (const run of runs) {
    const values = (scenarios[run.scenario] ??= {
      routeReady: [],
      longTasks: [],
    });
    values.routeReady.push(run.route_ready_ms);
    values.longTasks.push(run.long_task_total_ms);
  }
  return Object.fromEntries(
    Object.entries(scenarios).map(([name, values]) => [
      name,
      {
        sample_count: values.routeReady.length,
        route_ready_p75_ms: percentile(values.routeReady, 0.75),
        long_task_total_p75_ms: percentile(values.longTasks, 0.75),
      },
    ]),
  );
};

export const compareFrontendSummaries = (base, current, tolerance = 0.1) => {
  const failures = [];
  for (const [scenario, baseMetrics] of Object.entries(base)) {
    const currentMetrics = current[scenario];
    if (!currentMetrics) {
      failures.push(`${scenario} is missing from current measurements`);
      continue;
    }
    for (const field of ["route_ready_p75_ms", "long_task_total_p75_ms"]) {
      const before = baseMetrics[field];
      const after = currentMetrics[field];
      const limit = before === 0 ? 0 : before * (1 + tolerance);
      if (after > limit) {
        failures.push(
          `${scenario}.${field} regressed ${before === 0 ? "from zero" : `${((after / before - 1) * 100).toFixed(1)}%`} (${before} -> ${after}; limit ${(tolerance * 100).toFixed(1)}%)`,
        );
      }
    }
  }
  return failures;
};
