use std::{future::Future, task::Poll};

use super::*;
use crate::runtime_health::operations::{OperationSnapshot, OperationStats};

fn operation<'a>(snapshot: &'a OperationSnapshot, kind: &str, label: &str) -> &'a OperationStats {
    snapshot
        .operations
        .iter()
        .find(|stats| stats.kind == kind && stats.label == label)
        .unwrap_or_else(|| panic!("missing captured operation {kind}/{label}"))
}

fn reader_execution(conn: &mut rusqlite::Connection) -> RedisResult<()> {
    conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?;
    Ok(())
}

#[tokio::test]
async fn sqlite_operation_capture_observes_actual_primary_and_reader_execution() {
    let manager = temp_manager().await;
    let recorder = manager.diagnostics();
    assert!(Arc::ptr_eq(&recorder, &manager.clone().diagnostics()));
    recorder.start();
    let caller_thread = std::thread::current().id();
    manager
        .call_named("test.primary", move |conn| {
            assert_ne!(caller_thread, std::thread::current().id());
            conn.query_row("SELECT 1", [], |row| row.get::<_, i64>(0))?;
            Ok(())
        })
        .await
        .unwrap();
    manager
        .call_named("test.failed", |_| {
            Err::<(), _>(storage_error("test failure"))
        })
        .await
        .unwrap_err();
    manager.call_analytics(reader_execution).await.unwrap();
    manager.call_auth_read(reader_execution).await.unwrap();
    manager.ping().await.unwrap();
    let snapshot = recorder.snapshot();
    let reader_label = std::any::type_name_of_val(&reader_execution);
    let executions = [
        ("sqlite_primary", "test.primary", 0),
        ("sqlite_primary", "test.failed", 1),
        ("sqlite_analytics", reader_label, 0),
        ("sqlite_auth_read", reader_label, 0),
        ("sqlite_health", "health.ping", 0),
    ];
    let admissions = [
        ("sqlite_primary", 2),
        ("sqlite_analytics", 1),
        ("sqlite_auth_read", 1),
        ("sqlite_health", 1),
    ];
    assert_eq!(
        snapshot.operations.len(),
        executions.len() + admissions.len()
    );
    for (kind, label, failures) in executions {
        let stats = operation(&snapshot, kind, label);
        assert_eq!(
            (
                stats.calls,
                stats.failures,
                stats.cancelled,
                stats.in_flight
            ),
            (1, failures, 0, 0)
        );
        #[cfg(unix)]
        assert!(stats.total_cpu_ms.is_some());
    }
    for (label, calls) in admissions {
        let stats = operation(&snapshot, "sqlite_admission", label);
        assert_eq!(
            (
                stats.calls,
                stats.failures,
                stats.cancelled,
                stats.in_flight
            ),
            (calls, 0, 0, 0)
        );
        // Admission measures async waiting, not CPU on a SQLite execution thread.
        assert!(stats.total_cpu_ms.is_none());
    }
}

#[tokio::test]
async fn sqlite_operation_capture_excludes_cancelled_admission_waiters() {
    let manager = temp_manager().await;
    let recorder = manager.diagnostics();
    recorder.start();
    let permit = manager
        .primary_admission
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let mut waiting = Box::pin(manager.call_named::<(), _>("never.executed", |_| {
        panic!("cancelled waiter must not reach SQLite")
    }));
    std::future::poll_fn(|context| {
        assert!(waiting.as_mut().poll(context).is_pending());
        Poll::Ready(())
    })
    .await;
    let pending = recorder.snapshot();
    let admission = operation(&pending, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (admission.calls, admission.cancelled, admission.in_flight),
        (0, 0, 1)
    );
    assert_eq!(pending.operations.len(), 1);
    drop(waiting);
    drop(permit);
    let cancelled = recorder.snapshot();
    let admission = operation(&cancelled, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (
            admission.calls,
            admission.failures,
            admission.cancelled,
            admission.in_flight
        ),
        (1, 0, 1, 0)
    );
    assert!(admission.total_cpu_ms.is_none());
    assert_eq!(cancelled.operations.len(), 1);
    assert!(
        !cancelled
            .operations
            .iter()
            .any(|stats| stats.kind == "sqlite_primary" || stats.label == "never.executed")
    );
}

#[tokio::test]
async fn sqlite_operation_capture_survives_http_cancellation_until_closure_finishes() {
    let manager = temp_manager().await;
    let recorder = manager.diagnostics();
    recorder.start();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut call = Box::pin(manager.call_named("actual.work", move |conn| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        conn.execute(
            "CREATE TABLE diagnostics_cancellation_test(value INTEGER)",
            [],
        )?;
        Ok(())
    }));
    tokio::select! {
        result = &mut call => panic!("SQLite finished before release: {result:?}"),
        result = started_rx => result.unwrap(),
    }
    drop(call);
    let pending = recorder.snapshot();
    let work = operation(&pending, "sqlite_primary", "actual.work");
    assert_eq!((work.calls, work.cancelled, work.in_flight), (0, 0, 1));
    let admission = operation(&pending, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (admission.calls, admission.cancelled, admission.in_flight),
        (1, 0, 0)
    );
    assert!(
        manager
            .primary_admission
            .clone()
            .try_acquire_owned()
            .is_err()
    );
    release_tx.send(()).unwrap();
    // Admission to this next closure proves the previous one really finished.
    manager
        .call_named("after.work", |conn| {
            conn.execute("INSERT INTO diagnostics_cancellation_test VALUES (1)", [])?;
            Ok(())
        })
        .await
        .unwrap();
    let done = recorder.snapshot();
    let work = operation(&done, "sqlite_primary", "actual.work");
    assert_eq!(
        (work.calls, work.failures, work.cancelled, work.in_flight),
        (1, 0, 0, 0)
    );
    let after = operation(&done, "sqlite_primary", "after.work");
    assert_eq!(
        (
            after.calls,
            after.failures,
            after.cancelled,
            after.in_flight
        ),
        (1, 0, 0, 0)
    );
    let admission = operation(&done, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (admission.calls, admission.cancelled, admission.in_flight),
        (2, 0, 0)
    );
}

#[tokio::test]
async fn sqlite_operation_capture_keeps_old_execution_out_of_new_generation() {
    let manager = temp_manager().await;
    let recorder = manager.diagnostics();
    let old_generation = recorder.start();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let mut call = Box::pin(manager.call_named("old.work", move |_| {
        started_tx.send(()).unwrap();
        release_rx.recv().unwrap();
        Ok(())
    }));
    tokio::select! {
        result = &mut call => panic!("SQLite finished before release: {result:?}"),
        result = started_rx => result.unwrap(),
    }
    recorder.stop(old_generation);
    let stopped = recorder.snapshot();
    assert_eq!(stopped.generation, old_generation);
    let work = operation(&stopped, "sqlite_primary", "old.work");
    assert_eq!((work.calls, work.cancelled, work.in_flight), (0, 0, 1));
    let admission = operation(&stopped, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (admission.calls, admission.cancelled, admission.in_flight),
        (1, 0, 0)
    );
    let generation = recorder.start();
    release_tx.send(()).unwrap();
    call.await.unwrap();
    let snapshot = recorder.snapshot();
    assert_eq!(snapshot.generation, generation);
    assert!(snapshot.operations.is_empty());
}

#[tokio::test]
async fn command_capture_distinguishes_batch_reads_without_recording_keys() {
    let mut manager = temp_manager().await;
    let recorder = manager.diagnostics();
    recorder.start();
    let _: Vec<Option<String>> = cmd("MGET")
        .arg("private-session-key")
        .query_async(&mut manager)
        .await
        .unwrap();
    let _: (String, Vec<String>) = cmd("SCAN")
        .arg("0")
        .arg("MATCH")
        .arg("private:*")
        .query_async(&mut manager)
        .await
        .unwrap();
    let snapshot = recorder.snapshot();
    let execution_labels = ["redis_compat.MGET", "redis_compat.SCAN"];
    assert_eq!(snapshot.operations.len(), execution_labels.len() + 1);
    for label in execution_labels {
        let stats = operation(&snapshot, "sqlite_primary", label);
        assert_eq!(
            (
                stats.calls,
                stats.failures,
                stats.cancelled,
                stats.in_flight
            ),
            (1, 0, 0, 0)
        );
    }
    let admission = operation(&snapshot, "sqlite_admission", "sqlite_primary");
    assert_eq!(
        (admission.calls, admission.cancelled, admission.in_flight),
        (2, 0, 0)
    );
    assert!(
        !serde_json::to_string(&snapshot)
            .unwrap()
            .contains("private")
    );
}
