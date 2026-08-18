use std::time::Duration;

use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::time::{MissedTickBehavior, interval};

use crate::state::AppState;

use super::{model::RunTrigger, repository::Repository, service};

pub fn start(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("panel-sync-scheduler", async move {
        let mut tick = interval(Duration::from_secs(60));
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = task_state.panel_sync.source_changed.notified() => {
                    loop {
                        tokio::select! {
                            _ = task_state.shutdown.cancelled() => return,
                            _ = tokio::time::sleep(Duration::from_secs(5)) => break,
                            _ = task_state.panel_sync.source_changed.notified() => continue,
                        }
                    }
                    dispatch(&task_state, true).await;
                }
                _ = tick.tick() => dispatch(&task_state, false).await,
            }
        }
    });
}

async fn dispatch(state: &AppState, changed: bool) {
    let Ok(connections) = Repository::new(state).connections().await else {
        return;
    };
    for connection in connections
        .into_iter()
        .filter(|item| item.auto_sync.enabled && item.verified_at.is_some())
    {
        let due = if changed {
            true
        } else {
            let runs = Repository::new(state)
                .runs(&connection.id)
                .await
                .unwrap_or_default();
            let last = runs
                .first()
                .and_then(|run| OffsetDateTime::parse(&run.started_at, &Rfc3339).ok());
            last.is_none_or(|last| {
                OffsetDateTime::now_utc() - last
                    >= time::Duration::minutes(connection.auto_sync.interval_minutes.into())
            })
        };
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
        }
    }
}
