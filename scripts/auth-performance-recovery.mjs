const clock = () => performance.timeOrigin + performance.now();

export async function captureFailureSnapshot(row, error, readHealth) {
  row.validation = { passed: false, error: error?.stack ?? String(error) };
  try {
    row.failure_health = await readHealth();
  } catch (snapshotError) {
    row.failure_health = {
      error: snapshotError?.stack ?? String(snapshotError),
    };
  }
}

export async function runRecoveryProbe(report, hooks) {
  const now = hooks.now ?? clock;
  Object.assign(report, {
    burst_concurrency: 64,
    burst_duration_ms: 2000,
    recovery_concurrency: 1,
    recovery_deadline_ms: 10000,
    success_window_ms: 2000,
    performance_claim_allowed: false,
    checks: [],
    recovered: false,
    passed: false,
  });
  report.before_health = await hooks.readHealth();
  try {
    report.burst = await hooks.runBurst();
  } catch (error) {
    report.burst_error = error?.stack ?? String(error);
    report.burst_partial = error.phase_evidence ?? null;
  }
  report.burst_503_responses = report.burst?.measurement
    ? (report.burst.measurement.statuses?.[503] ?? 0)
    : null;
  // runBurst has drained/terminated its workers before lowering concurrency.
  const recoveryStarted = now();
  const deadline = recoveryStarted + report.recovery_deadline_ms;
  report.recovery_started_at_epoch_ms = recoveryStarted;
  report.after_burst_health = await hooks.readHealth();
  while (deadline - now() >= report.success_window_ms) {
    const check = { started_after_burst_ms: now() - recoveryStarted };
    report.checks.push(check);
    try {
      check.result = await hooks.runCheck(deadline);
      const result = check.result;
      check.requests_finished_after_burst_ms =
        result.requests_finished_at_epoch_ms - recoveryStarted;
      check.continuous_success =
        result.measurement.failures === 0 &&
        result.measurement.successful_requests > 0 &&
        result.measurement.elapsed_ms >= report.success_window_ms &&
        check.requests_finished_after_burst_ms <= report.recovery_deadline_ms;
      if (check.continuous_success) {
        report.recovered = true;
        report.recovery_elapsed_ms = check.requests_finished_after_burst_ms;
        break;
      }
    } catch (error) {
      check.error = error?.stack ?? String(error);
      check.partial = error.phase_evidence ?? null;
      break;
    }
  }
  report.observed_until_ms = now() - recoveryStarted;
  report.passed = report.recovered && !report.burst_error;
  report.after_recovery_health = await hooks.readHealth();
  return report;
}
