use super::*;

async fn drain(runtime: &RuntimeHealth, state: &AppState) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !runtime.inner.pending_events.lock().await.is_empty() {
            runtime.flush_pending(state).await;
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("buffered events drained");
}

async fn events(state: &AppState) -> Vec<Value> {
    state
        .storage
        .store
        .list_system_events(1, 100, "", None, None, Some("RUNTIME_MONITOR"))
        .await
        .unwrap()["events"]
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test]
async fn blocked_sqlite_does_not_stall_health_transitions_and_retains_evidence() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let checked_at = time_utils::now_iso();
    tokio::time::timeout(Duration::from_secs(1), async {
        for _ in 0..3 {
            runtime
                .apply_probe(
                    &state,
                    "storage",
                    ProbeResult {
                        ok: false,
                        reason_code: "sqlite_primary_queue_saturated",
                        metadata: ProbeMetadata {
                            queue_depth: Some(20),
                            queue_wait_ms: Some(61_785),
                            active_operation_ms: Some(60_000),
                            ..ProbeMetadata::default()
                        },
                    },
                    &checked_at,
                )
                .await;
        }
        runtime.publish_snapshot(&checked_at).await;
    })
    .await
    .expect("SQLite event writes must not stall health updates");
    assert_eq!(
        runtime.snapshot().await.components["storage"].status,
        HealthStatus::Unhealthy
    );
    assert_eq!(runtime.inner.pending_events.lock().await.len(), 1);
    assert!(!runtime.recovery_active());

    // Recovery can also be observed while persistence is unavailable.
    for _ in 0..2 {
        runtime
            .apply_probe(
                &state,
                "storage",
                ProbeResult {
                    ok: true,
                    reason_code: "sqlite_ping_ok",
                    metadata: ProbeMetadata::default(),
                },
                &time_utils::now_iso(),
            )
            .await;
    }
    assert_eq!(runtime.inner.pending_events.lock().await.len(), 2);
    lock.execute_batch("ROLLBACK").unwrap();
    drain(runtime, &state).await;

    let emitted = events(&state).await;
    assert_eq!(emitted.len(), 2);
    let failure = emitted
        .iter()
        .find(|e| e["type"] == "FN_EVENT_RUNTIME_HEALTH_FAILED")
        .unwrap();
    let recovery = emitted
        .iter()
        .find(|e| e["type"] == "FN_EVENT_RUNTIME_RECOVERED")
        .unwrap();
    assert_eq!(failure["happened_at"], checked_at);
    assert_eq!(
        failure["payload"]["incident_id"],
        recovery["payload"]["incident_id"]
    );
    assert_eq!(failure["payload"]["queue_wait_ms"], 61_785);
    assert_eq!(failure["payload"]["queue_depth"], 20);

    runtime.flush_operational_log().await;
    let rows: Vec<Value> = std::fs::read_to_string(runtime.logs_dir().join("management.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let unhealthy = rows
        .iter()
        .find(|row| row["fields"]["status"] == "unhealthy")
        .expect("escalation must not be suppressed by the preceding degraded record");
    assert_eq!(unhealthy["fields"]["active_operation_ms"], 60_000);
    assert_eq!(unhealthy["fields"]["queue_depth"], 20);
}

#[tokio::test]
async fn retry_after_an_unacknowledged_commit_does_not_duplicate_the_event() {
    let (_directory, state) = tests::runtime_test_state().await;
    let input = RuntimeEventInput {
        event_type: "FN_EVENT_RUNTIME_HEALTH_FAILED",
        level: "ERROR",
        component: "storage".into(),
        happened_at: time_utils::now_iso(),
        payload: json!({"component": "storage", "incident_id": "same-incident"}),
    };
    // Simulate the commit completing after its original caller timed out.
    assert!(publish_runtime_event(&state, input.clone()).await.unwrap());
    state
        .runtime_health
        .publish_or_buffer(&state, input.clone())
        .await;
    drain(&state.runtime_health, &state).await;
    assert_eq!(events(&state).await.len(), 1);
    state
        .runtime_health
        .publish_or_buffer(
            &state,
            RuntimeEventInput {
                event_type: "FN_EVENT_RUNTIME_RECOVERED",
                level: "INFO",
                ..input
            },
        )
        .await;
    drain(&state.runtime_health, &state).await;
    assert_eq!(
        events(&state).await.len(),
        2,
        "recovery has its own dedupe key"
    );
}

#[tokio::test]
async fn blocked_gateway_baseline_write_does_not_repeat_restart_events() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    *runtime.inner.seen_gateway_instance.lock().await = Some("old-instance".into());
    runtime
        .inner
        .trackers
        .lock()
        .await
        .get_mut("gateway_process")
        .unwrap()
        .health
        .instance_id = Some("new-instance".into());
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        runtime.observe_gateway_instance(&state).await;
        runtime.observe_gateway_instance(&state).await;
    })
    .await
    .expect("baseline persistence must not block probes");
    assert_eq!(runtime.inner.pending_events.lock().await.len(), 1);
    assert_eq!(
        runtime
            .inner
            .pending_gateway_instance
            .lock()
            .await
            .as_deref(),
        Some("new-instance")
    );
    lock.execute_batch("ROLLBACK").unwrap();
    drain(runtime, &state).await;
    runtime.observe_gateway_instance(&state).await;
    assert_eq!(events(&state).await.len(), 1);
    assert_eq!(
        state
            .storage
            .store
            .get_string_value(GATEWAY_INSTANCE_KEY)
            .await
            .unwrap()
            .as_deref(),
        Some("new-instance")
    );
}

#[tokio::test]
async fn health_transition_logs_preserve_repeated_incidents_within_one_minute() {
    let directory = tempfile::tempdir().unwrap();
    let logger = DiagnosticLogger::new(directory.path().to_path_buf()).unwrap();
    for _ in 0..2 {
        for (previous, status, level) in [
            ("healthy", "degraded", "WARN"),
            ("degraded", "unhealthy", "ERROR"),
            ("unhealthy", "healthy", "INFO"),
        ] {
            logger.log(
                level,
                "storage",
                "health_transition",
                "test",
                Map::from_iter([
                    ("previous_status".into(), json!(previous)),
                    ("status".into(), json!(status)),
                ]),
            );
        }
    }
    logger.flush().await;
    assert_eq!(
        std::fs::read_to_string(directory.path().join("management.jsonl"))
            .unwrap()
            .lines()
            .count(),
        6
    );
    assert!(logger.shutdown(Duration::from_secs(2)).await);
}

fn buffered_event(incident: &str) -> RuntimeEventInput {
    RuntimeEventInput {
        event_type: "FN_EVENT_RUNTIME_HEALTH_FAILED",
        level: "ERROR",
        component: "storage".into(),
        happened_at: time_utils::now_iso(),
        payload: json!({"component": "storage", "incident_id": incident}),
    }
}

#[tokio::test]
async fn shutdown_drains_events_after_the_background_registry_is_closed() {
    let (_directory, state) = tests::runtime_test_state().await;
    start_runtime_monitor(state.clone()).await.unwrap();
    let runtime = &state.runtime_health;
    runtime.mark_session_ready(&state).await.unwrap();
    // Buffer more than one event; the monitor must not depend on registering
    // another background task when it advances past the first write.
    let flush = runtime.inner.event_flush.lock().await;
    for incident in ["shutdown-one", "shutdown-two", "shutdown-three"] {
        runtime
            .publish_or_buffer(&state, buffered_event(incident))
            .await;
    }
    state.shutdown.cancel();
    let closing = state.shutdown_background_tasks(Duration::from_secs(5));
    tokio::pin!(closing);
    // Poll once to close registration, while the monitor is still held above.
    std::future::poll_fn(|cx| {
        assert!(closing.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    drop(flush);
    assert!(
        closing.await.is_empty(),
        "shutdown must not abort the monitor"
    );
    assert!(runtime.inner.monitor_stopped.load(Ordering::Acquire));
    assert!(runtime.inner.pending_events.lock().await.is_empty());
    assert_eq!(events(&state).await.len(), 3);
    let session: Value = serde_json::from_str(
        &state
            .storage
            .store
            .get_string_value(SESSION_KEY)
            .await
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(session["state"], "stopped");
}

#[tokio::test]
async fn shutdown_allows_slow_writes_beyond_the_normal_probe_budget() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    // Reproduce production shutdown ordering without a monitor racing this test.
    state.shutdown.cancel();
    assert!(
        state
            .shutdown_background_tasks(Duration::from_secs(1))
            .await
            .is_empty()
    );
    {
        let mut pending = runtime.inner.pending_events.lock().await;
        RuntimeHealth::enqueue_event(&mut pending, buffered_event("slow-shutdown-one"));
        RuntimeHealth::enqueue_event(&mut pending, buffered_event("slow-shutdown-two"));
    }
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    let release = tokio::spawn(async move {
        tokio::time::sleep(EVENT_WRITE_BUDGET * 3).await;
        lock.execute_batch("ROLLBACK").unwrap();
    });
    runtime.finish_monitor(&state).await;
    release.await.unwrap();
    assert!(runtime.inner.pending_events.lock().await.is_empty());
    assert_eq!(events(&state).await.len(), 2);
}

#[tokio::test]
async fn shutdown_with_a_stuck_database_is_bounded_and_reports_pending_events() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    state.shutdown.cancel();
    {
        let mut pending = runtime.inner.pending_events.lock().await;
        RuntimeHealth::enqueue_event(&mut pending, buffered_event("stuck-shutdown"));
    }
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    tokio::time::timeout(
        SHUTDOWN_WRITE_BUDGET + Duration::from_secs(1),
        runtime.finish_monitor(&state),
    )
    .await
    .expect("shutdown must not wait indefinitely for SQLite");
    assert!(runtime.inner.monitor_stopped.load(Ordering::Acquire));
    assert_eq!(runtime.inner.pending_events.lock().await.len(), 1);
    lock.execute_batch("ROLLBACK").unwrap();
    let rows: Vec<Value> = std::fs::read_to_string(runtime.logs_dir().join("management.jsonl"))
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let warning = rows
        .iter()
        .find(|row| row["event"] == "events_pending_at_shutdown")
        .unwrap();
    assert_eq!(warning["fields"]["count"], 1);
}

#[tokio::test]
async fn canceled_gateway_observation_does_not_repeat_an_already_queued_restart() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    *runtime.inner.seen_gateway_instance.lock().await = Some("old-instance".into());
    runtime
        .inner
        .trackers
        .lock()
        .await
        .get_mut("gateway_process")
        .unwrap()
        .health
        .instance_id = Some("new-instance".into());
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    {
        let observation = runtime.observe_gateway_instance(&state);
        tokio::pin!(observation);
        tokio::time::timeout(Duration::from_secs(1), async {
            tokio::select! {
                _ = &mut observation => panic!("write unexpectedly completed while database was locked"),
                _ = async {
                    while runtime.inner.pending_events.lock().await.is_empty() {
                        tokio::task::yield_now().await;
                    }
                } => {},
            }
        }).await.unwrap();
        // Drop the observation in the middle of waiting for event persistence.
    }
    assert_eq!(
        runtime.inner.seen_gateway_instance.lock().await.as_deref(),
        Some("new-instance")
    );
    assert_eq!(
        runtime
            .inner
            .pending_gateway_instance
            .lock()
            .await
            .as_deref(),
        Some("new-instance")
    );
    lock.execute_batch("ROLLBACK").unwrap();
    runtime.observe_gateway_instance(&state).await;
    drain(runtime, &state).await;
    assert_eq!(events(&state).await.len(), 1);
}

#[tokio::test]
async fn cancellation_before_enqueue_does_not_consume_a_gateway_observation() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    *runtime.inner.seen_gateway_instance.lock().await = Some("old-instance".into());
    runtime
        .inner
        .trackers
        .lock()
        .await
        .get_mut("gateway_process")
        .unwrap()
        .health
        .instance_id = Some("new-instance".into());
    let pending = runtime.inner.pending_events.lock().await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(20),
            runtime.observe_gateway_instance(&state)
        )
        .await
        .is_err()
    );
    assert_eq!(
        runtime.inner.seen_gateway_instance.lock().await.as_deref(),
        Some("old-instance")
    );
    assert!(
        runtime
            .inner
            .pending_gateway_instance
            .lock()
            .await
            .is_none()
    );
    drop(pending);
    runtime.observe_gateway_instance(&state).await;
    drain(runtime, &state).await;
    assert_eq!(events(&state).await.len(), 1);
}

#[tokio::test]
async fn shutdown_retries_a_lost_writer_ack_without_duplicates_or_dropping_the_tail() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    let committed = buffered_event("committed-before-shutdown");
    publish_runtime_event(&state, committed.clone())
        .await
        .unwrap();
    state.shutdown.cancel();
    state
        .shutdown_background_tasks(Duration::from_secs(1))
        .await;
    {
        let mut pending = runtime.inner.pending_events.lock().await;
        RuntimeHealth::enqueue_event(&mut pending, committed.clone());
        RuntimeHealth::enqueue_event(&mut pending, buffered_event("shutdown-tail"));
    }
    let (sender, result) = oneshot::channel();
    drop(sender);
    *runtime.inner.event_flush.lock().await = Some(InFlightRuntimeEvent {
        input: committed,
        result,
    });
    runtime.finish_monitor(&state).await;
    assert!(runtime.inner.pending_events.lock().await.is_empty());
    assert!(runtime.inner.event_flush.lock().await.is_none());
    assert_eq!(events(&state).await.len(), 2);
}

#[tokio::test]
async fn overflow_during_an_inflight_write_does_not_acknowledge_the_wrong_event() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    let lock = tokio_rusqlite::rusqlite::Connection::open(&state.settings.sqlite_path).unwrap();
    lock.execute_batch("BEGIN IMMEDIATE").unwrap();
    runtime
        .publish_or_buffer(&state, buffered_event("evicted-inflight"))
        .await;
    assert!(runtime.inner.event_flush.lock().await.is_some());
    {
        let mut pending = runtime.inner.pending_events.lock().await;
        for index in 0..MAX_PENDING_EVENTS {
            RuntimeHealth::enqueue_event(
                &mut pending,
                buffered_event(&format!("retained-{index}")),
            );
        }
        assert_eq!(pending.len(), MAX_PENDING_EVENTS);
        assert_eq!(
            pending.front().unwrap().input.payload["incident_id"],
            "retained-0"
        );
    }
    lock.execute_batch("ROLLBACK").unwrap();
    drain(runtime, &state).await;
    let emitted = events(&state).await;
    assert_eq!(emitted.len(), MAX_PENDING_EVENTS + 1);
    assert!(
        emitted
            .iter()
            .any(|event| event["payload"]["incident_id"] == "retained-0")
    );
}
