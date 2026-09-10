//! Request-local diagnostics containing only static phase names and timings.
//! Timings are inclusive: parent phases include time spent in child phases.
use std::{
    collections::BTreeMap,
    future::Future,
    sync::{Arc, Mutex},
};
use tokio::time::Instant;

tokio::task_local! { static CURRENT: Diagnostics; }

#[derive(Clone, Default)]
pub(crate) struct Diagnostics(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    next_id: u64,
    active: Vec<(u64, &'static str, Instant)>,
    elapsed: BTreeMap<&'static str, u64>,
}

impl Diagnostics {
    pub(crate) async fn scope<T>(&self, future: impl Future<Output = T>) -> T {
        CURRENT.scope(self.clone(), future).await
    }

    pub(crate) fn fields(&self) -> serde_json::Map<String, serde_json::Value> {
        let state = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        let mut elapsed = state.elapsed.clone();
        for (_, name, started) in &state.active {
            *elapsed.entry(name).or_default() += now.duration_since(*started).as_millis() as u64;
        }
        serde_json::Map::from_iter([
            (
                "phase".into(),
                serde_json::json!(state.active.last().map(|(_, name, _)| name)),
            ),
            (
                "phase_active_ms".into(),
                serde_json::json!(
                    state
                        .active
                        .last()
                        .map(|(_, _, started)| now.duration_since(*started).as_millis() as u64)
                ),
            ),
            ("phase_elapsed_ms".into(), serde_json::json!(elapsed)),
        ])
    }
}

pub(crate) struct Phase(Option<(Diagnostics, u64)>);

/// Snapshot before dropping the timed-out handler. Identify live guards by ID
/// rather than restoring a saved parent: joined futures may finish out of order.
/// Without a request context this is a no-op, including no clock reads.
pub(crate) fn enter(name: &'static str) -> Phase {
    let Ok(context) = CURRENT.try_with(Clone::clone) else {
        return Phase(None);
    };
    let id = {
        let mut state = context.0.lock().unwrap_or_else(|e| e.into_inner());
        let id = state.next_id;
        state.next_id += 1;
        state.active.push((id, name, Instant::now()));
        id
    };
    Phase(Some((context, id)))
}

impl Drop for Phase {
    fn drop(&mut self) {
        if let Some((context, id)) = &self.0 {
            let mut state = context.0.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(index) = state
                .active
                .iter()
                .position(|(active_id, _, _)| active_id == id)
            {
                let (_, name, started) = state.active.remove(index);
                *state.elapsed.entry(name).or_default() += started.elapsed().as_millis() as u64;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_identifies_nested_wait_without_cross_request_leakage() {
        let diagnostics = Diagnostics::default();
        let future = diagnostics.scope(async {
            let _outer = enter("normal_access");
            let _inner = enter("sqlite_primary_wait");
            std::future::pending::<()>().await;
        });
        tokio::pin!(future);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(1), &mut future)
                .await
                .is_err()
        );
        assert_eq!(diagnostics.fields()["phase"], "sqlite_primary_wait");
        assert!(Diagnostics::default().fields()["phase"].is_null());
    }

    #[tokio::test]
    async fn completed_nested_phase_restores_parent() {
        let diagnostics = Diagnostics::default();
        diagnostics
            .scope(async {
                let _outer = enter("normal_access");
                {
                    let _inner = enter("sqlite_primary_wait");
                }
                assert_eq!(diagnostics.fields()["phase"], "normal_access");
            })
            .await;
        assert!(
            diagnostics.fields()["phase_elapsed_ms"]
                .get("sqlite_primary_wait")
                .is_some()
        );
    }
    #[tokio::test]
    async fn out_of_order_phase_completion_never_resurrects_finished_work() {
        let diagnostics = Diagnostics::default();
        diagnostics
            .scope(async {
                let first = enter("sqlite_primary_wait");
                let second = enter("sqlite_reader_wait");
                drop(first);
                assert_eq!(diagnostics.fields()["phase"], "sqlite_reader_wait");
                drop(second);
                assert!(diagnostics.fields()["phase"].is_null());
            })
            .await;
    }

    #[tokio::test]
    async fn snapshot_includes_unfinished_phase_time_without_double_counting() {
        let diagnostics = Diagnostics::default();
        diagnostics
            .scope(async {
                let phase = enter("mobility_lock");
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                let during = diagnostics.fields();
                let elapsed = during["phase_elapsed_ms"]["mobility_lock"]
                    .as_u64()
                    .unwrap();
                assert!(elapsed >= 5);
                assert_eq!(during["phase_active_ms"], elapsed);
                drop(phase);
                assert!(diagnostics.fields()["phase"].is_null());
                assert_eq!(diagnostics.0.lock().unwrap().elapsed.len(), 1);
            })
            .await;
    }
}
