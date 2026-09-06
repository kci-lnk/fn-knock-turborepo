use std::{future::Future, task::Poll};

use super::*;

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
    manager.call_analytics(|_| Ok(())).await.unwrap();
    manager.call_auth_read(|_| Ok(())).await.unwrap();
    manager.ping().await.unwrap();
    let snapshot = recorder.snapshot();
    assert_eq!(snapshot.operations.len(), 5);
    assert_eq!(
        snapshot
            .operations
            .iter()
            .map(|stats| stats.calls)
            .sum::<u64>(),
        5
    );
    assert_eq!(
        snapshot
            .operations
            .iter()
            .map(|stats| stats.failures)
            .sum::<u64>(),
        1
    );
    for kind in [
        "sqlite_primary",
        "sqlite_analytics",
        "sqlite_auth_read",
        "sqlite_health",
    ] {
        assert!(snapshot.operations.iter().any(|stats| stats.kind == kind));
    }
    #[cfg(unix)]
    assert!(
        snapshot
            .operations
            .iter()
            .all(|stats| stats.total_cpu_ms.is_some())
    );
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
    drop(waiting);
    drop(permit);
    assert!(recorder.snapshot().operations.is_empty());
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
    assert_eq!(pending.operations[0].in_flight, 1);
    assert_eq!(pending.operations[0].calls, 0);
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
    let work = done
        .operations
        .iter()
        .find(|stats| stats.label == "actual.work")
        .unwrap();
    assert_eq!((work.calls, work.cancelled, work.in_flight), (1, 0, 0));
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
    assert_eq!(stopped.operations[0].in_flight, 1);
    let generation = recorder.start();
    release_tx.send(()).unwrap();
    call.await.unwrap();
    let snapshot = recorder.snapshot();
    assert_eq!(snapshot.generation, generation);
    assert!(snapshot.operations.is_empty());
}
