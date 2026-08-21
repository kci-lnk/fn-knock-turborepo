use std::time::Duration;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::state::AppState;

use super::{model::RunTrigger, repository::Repository, service};

pub fn start(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("panel-sync-scheduler", async move {
        let mut next_wakeup = Some(Duration::ZERO);
        loop {
            let changed = if let Some(delay) = next_wakeup.take() {
                tokio::select! {
                    _ = task_state.shutdown.cancelled() => break,
                    _ = task_state.panel_sync.source_changed.notified() => true,
                    _ = tokio::time::sleep(delay) => false,
                }
            } else {
                tokio::select! {
                    _ = task_state.shutdown.cancelled() => break,
                    _ = task_state.panel_sync.source_changed.notified() => true,
                }
            };
            if changed {
                loop {
                    tokio::select! {
                        _ = task_state.shutdown.cancelled() => return,
                        _ = tokio::time::sleep(Duration::from_secs(5)) => break,
                        _ = task_state.panel_sync.source_changed.notified() => continue,
                    }
                }
            }
            next_wakeup = dispatch(&task_state, changed).await;
        }
    });
}

async fn dispatch(state: &AppState, changed: bool) -> Option<Duration> {
    let Ok(connections) = Repository::new(state).connections().await else {
        return Some(Duration::from_secs(60));
    };
    let now = OffsetDateTime::now_utc();
    let mut next_wakeup: Option<Duration> = None;
    for connection in connections
        .into_iter()
        .filter(|item| item.auto_sync.enabled && item.verified_at.is_some())
    {
        let interval = Duration::from_secs(
            u64::from(connection.auto_sync.interval_minutes.max(1)).saturating_mul(60),
        );
        let interval_seconds = i64::try_from(interval.as_secs()).unwrap_or(i64::MAX);
        let last = Repository::new(state)
            .runs(&connection.id)
            .await
            .unwrap_or_default()
            .first()
            .and_then(|run| OffsetDateTime::parse(&run.started_at, &Rfc3339).ok());
        let elapsed_seconds = last
            .map(|last| (now - last).whole_seconds())
            .unwrap_or(i64::MAX);
        let due = changed || elapsed_seconds >= interval_seconds;
        if due {
            let task_state = state.clone();
            let trigger = if changed {
                RunTrigger::ConfigChange
            } else {
                RunTrigger::Periodic
            };
            state.spawn_background("panel-sync-dispatch", async move {
                service::enqueue_automatic(task_state, connection, trigger).await;
            });
            next_wakeup = Some(next_wakeup.map_or(interval, |current| current.min(interval)));
        } else {
            let remaining = Duration::from_secs(
                u64::try_from(interval_seconds.saturating_sub(elapsed_seconds)).unwrap_or(u64::MAX),
            );
            next_wakeup = Some(next_wakeup.map_or(remaining, |current| current.min(remaining)));
        }
    }
    next_wakeup
}
