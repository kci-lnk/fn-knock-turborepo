use std::{future::Future, time::Duration};

use tokio::{sync::watch, time::Instant};

use super::{
    AppState,
    service::{drain_waf_events_now, waf_drain_schedule},
};

const PROBE_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Copy)]
enum Phase {
    Drain,
    Wait,
    Batch(Instant),
    Legacy(Instant),
    Backoff {
        deadline: Instant,
        retry_drain: bool,
    },
}

enum Wake<T> {
    Complete(T),
    Reload,
    Stop,
}

// Keep the same future alive across unrelated snapshot publications. The
// caller marks the watch revision seen before reading `schedule`.
async fn interruptible<T>(
    state: &AppState,
    updates: &mut watch::Receiver<u64>,
    schedule: Option<u64>,
    future: impl Future<Output = T>,
) -> Wake<T> {
    tokio::pin!(future);
    loop {
        tokio::select! {
            biased;
            _ = state.shutdown.cancelled() => return Wake::Stop,
            result = &mut future => return Wake::Complete(result),
            changed = updates.changed() => {
                if changed.is_err() { return Wake::Stop; }
                if waf_drain_schedule(state) != schedule { return Wake::Reload; }
            }
            _ = state.waf_event_drain_reload_notify.notified() => {
                if waf_drain_schedule(state) != schedule { return Wake::Reload; }
            }
        }
    }
}

pub(super) fn retry_delay(failures: &mut u32) -> Duration {
    let seconds = [1, 2, 4, 8, 16, 30][(*failures).min(5) as usize];
    *failures = failures.saturating_add(1);
    Duration::from_secs(seconds)
}

pub(super) async fn run(state: &AppState) {
    let mut updates = state.storage.store.subscribe_config_snapshot();
    let mut phase = Phase::Drain; // Preserve the immediate startup drain.
    let mut legacy_probe = None;
    let mut failures = 0;
    loop {
        updates.borrow_and_update();
        let schedule = waf_drain_schedule(state);
        if state.shutdown.is_cancelled() {
            return;
        }
        let Some(seconds) = schedule else {
            match interruptible(state, &mut updates, schedule, std::future::pending::<()>()).await {
                Wake::Stop => return,
                _ => {
                    phase = Phase::Wait;
                    continue;
                }
            }
        };
        let interval = Duration::from_secs(seconds);
        match phase {
            Phase::Drain => {
                // Never cancel a persisted batch because of a config update:
                // finish its ACK, then recheck enabled/shutdown between batches.
                let result = tokio::select! {
                    biased;
                    _ = state.shutdown.cancelled() => return,
                    result = drain_waf_events_now(state) => result,
                };
                match result {
                    Ok(value) => {
                        failures = 0;
                        phase = if value["remaining"].as_i64().unwrap_or(0) > 0 {
                            Phase::Drain
                        } else if legacy_probe.is_some() {
                            Phase::Legacy(Instant::now())
                        } else {
                            Phase::Wait
                        };
                    }
                    Err(error) => {
                        tracing::debug!(%error, "failed to drain WAF events");
                        phase = Phase::Backoff {
                            deadline: Instant::now() + retry_delay(&mut failures),
                            retry_drain: true,
                        };
                    }
                }
                // drain_waf_events_now has released the processing mutex.
                tokio::task::yield_now().await;
            }
            Phase::Wait => {
                let operation = state
                    .storage
                    .store
                    .diagnostics()
                    .scope("wait", "waf.wait_events");
                let wake = interruptible(
                    state,
                    &mut updates,
                    schedule,
                    state.gateway.client.wait_waf_events(),
                )
                .await;
                match &wake {
                    Wake::Complete(result) => operation.finish(
                        !matches!(result, Err(error) if error.code() != tonic::Code::Unimplemented),
                        None,
                    ),
                    Wake::Reload | Wake::Stop => drop(operation),
                }
                match wake {
                    Wake::Stop => return,
                    Wake::Reload => {}
                    Wake::Complete(Ok(available)) => {
                        failures = 0;
                        legacy_probe = None;
                        if available {
                            phase = Phase::Batch(Instant::now());
                        }
                    }
                    Wake::Complete(Err(error)) if error.code() == tonic::Code::Unimplemented => {
                        legacy_probe = Some(Instant::now() + PROBE_INTERVAL);
                        phase = Phase::Legacy(Instant::now());
                    }
                    Wake::Complete(Err(error)) => {
                        tracing::debug!(%error, "failed to wait for WAF events");
                        phase = Phase::Backoff {
                            deadline: Instant::now() + retry_delay(&mut failures),
                            retry_drain: false,
                        };
                    }
                }
            }
            Phase::Batch(started) | Phase::Legacy(started) => {
                let legacy = matches!(phase, Phase::Legacy(_));
                let deadline = if legacy {
                    legacy_probe.map_or(started + interval, |probe| probe.min(started + interval))
                } else {
                    started + interval
                };
                match interruptible(
                    state,
                    &mut updates,
                    schedule,
                    tokio::time::sleep_until(deadline),
                )
                .await
                {
                    Wake::Stop => return,
                    Wake::Reload => {} // Recompute from original start, never postpone on unrelated writes.
                    Wake::Complete(()) => {
                        phase = if legacy
                            && legacy_probe.is_some_and(|probe| Instant::now() >= probe)
                        {
                            legacy_probe = None;
                            Phase::Wait
                        } else {
                            Phase::Drain
                        };
                    }
                }
            }
            Phase::Backoff {
                deadline,
                retry_drain,
            } => {
                match interruptible(
                    state,
                    &mut updates,
                    schedule,
                    tokio::time::sleep_until(deadline),
                )
                .await
                {
                    Wake::Stop => return,
                    Wake::Reload => {}
                    Wake::Complete(()) => {
                        phase = if retry_drain {
                            Phase::Drain
                        } else {
                            Phase::Wait
                        }
                    }
                }
            }
        }
    }
}
