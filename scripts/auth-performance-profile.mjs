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
  // Every auth reader slot shares the recorder. Keep its execution/admission
  // scopes separate from primary work and other reader families. Neither scope
  // includes the checkpoint gate wait between admission and SQLite execution.
  const isAuthReader = (operation) =>
    operation.kind === "sqlite_auth_read" ||
    (operation.kind === "sqlite_admission" &&
      operation.label === "sqlite_auth_read");
  const authOperations = snapshot.operations.filter(isAuthReader);
  const authTotals = summarize(authOperations);
  const authExecutorAvailable = authOperations.some(
    (operation) => operation.kind === "sqlite_auth_read",
  );
  const authAdmissionAvailable = authOperations.some(
    (operation) => operation.kind === "sqlite_admission",
  );
  const idleAuthOperations = idleSnapshot?.operations.filter(isAuthReader);
  const idleAuth = idleAuthOperations ? summarize(idleAuthOperations) : null;
  const authPerSuccess = (value, observed) =>
    requests > 0 && observed && value !== null ? value / requests : null;
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
    auth_reader: {
      measurement: "all auth reader slots in the shared operation recorder",
      checkpoint_wait_included: false,
      ...authTotals,
      executor_instrumentation_observed: authExecutorAvailable,
      admission_instrumentation_observed: authAdmissionAvailable,
      unfinished_scopes: authOperations.reduce(
        (sum, operation) => sum + operation.in_flight,
        0,
      ),
      executor_calls_per_success: authPerSuccess(
        authTotals.executor_calls,
        authExecutorAvailable,
      ),
      executor_wall_ms_per_success: authPerSuccess(
        authTotals.executor_wall_ms,
        authExecutorAvailable,
      ),
      executor_cpu_ms_per_success: authPerSuccess(
        authTotals.executor_cpu_ms,
        authExecutorAvailable,
      ),
      admission_calls_per_success: authPerSuccess(
        authTotals.admission_calls,
        authAdmissionAvailable,
      ),
      admission_wall_ms_per_success: authPerSuccess(
        authTotals.admission_wall_ms,
        authAdmissionAvailable,
      ),
      idle_executor_calls_per_second:
        idleAuth &&
        idleSnapshot.elapsed_ms > 0 &&
        idleAuthOperations.some(
          (operation) => operation.kind === "sqlite_auth_read",
        )
          ? (idleAuth.executor_calls * 1000) / idleSnapshot.elapsed_ms
          : null,
    },
    operations: snapshot.operations,
  };
}
