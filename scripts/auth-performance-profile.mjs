// Operation scopes count executor admissions/jobs, never individual SQL statements.
export function summarizeOperationProfile(report, requests, idleReport) {
  const snapshot = report.capture.operations;
  const executorKinds = new Set(["sqlite_primary", "sqlite_auth_read"]);
  const summarize = (operations) => {
    const total = {
      executor_calls: 0,
      executor_wall_ms: 0,
      executor_cpu_ms: null,
      admission_calls: 0,
      admission_wall_ms: 0,
    };
    for (const operation of operations) {
      if (executorKinds.has(operation.kind)) {
        total.executor_calls += operation.calls;
        total.executor_wall_ms += operation.total_wall_ms;
        if (operation.total_cpu_ms !== null)
          total.executor_cpu_ms =
            (total.executor_cpu_ms ?? 0) + operation.total_cpu_ms;
      } else if (operation.kind === "sqlite_admission") {
        total.admission_calls += operation.calls;
        total.admission_wall_ms += operation.total_wall_ms;
      }
    }
    return total;
  };
  const totals = summarize(snapshot.operations);
  const admissionAvailable = snapshot.operations.some(
    (operation) => operation.kind === "sqlite_admission",
  );
  const idleSnapshot = idleReport?.capture?.operations;
  const idle = idleSnapshot ? summarize(idleSnapshot.operations) : null;
  return {
    measurement:
      "process-wide instrumented executor scopes; not SQL statement count",
    successful_request_denominator: requests,
    includes_background_operations: true,
    elapsed_ms: snapshot.elapsed_ms,
    dropped_operations: snapshot.dropped_operations,
    unfinished_scopes: snapshot.operations.reduce(
      (sum, operation) => sum + operation.in_flight,
      0,
    ),
    ...totals,
    admission_instrumentation_observed: admissionAvailable,
    executor_calls_per_success:
      requests > 0 ? totals.executor_calls / requests : null,
    executor_wall_ms_per_success:
      requests > 0 ? totals.executor_wall_ms / requests : null,
    executor_cpu_ms_per_success:
      requests > 0 && totals.executor_cpu_ms !== null
        ? totals.executor_cpu_ms / requests
        : null,
    admission_calls_per_success:
      requests > 0 && admissionAvailable
        ? totals.admission_calls / requests
        : null,
    admission_wall_ms_per_success:
      requests > 0 && admissionAvailable
        ? totals.admission_wall_ms / requests
        : null,
    idle_executor_calls_per_second:
      idle && idleSnapshot.elapsed_ms > 0
        ? (idle.executor_calls * 1000) / idleSnapshot.elapsed_ms
        : null,
    operations: snapshot.operations,
  };
}
