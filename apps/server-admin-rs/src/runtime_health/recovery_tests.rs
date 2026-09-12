use super::*;

fn sample(ok: bool) -> ProbeResult {
    ProbeResult {
        ok,
        reason_code: if ok { "serving" } else { "not_serving" },
        metadata: ProbeMetadata::default(),
    }
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
async fn startup_grace_is_bounded_and_ends_after_first_success() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    for id in [
        "gateway_process",
        "gateway_dataplane",
        "auth_bridge",
        "config_sync",
    ] {
        for _ in 0..4 {
            runtime
                .apply_sampled_probe(
                    &state,
                    id,
                    sample(false),
                    &time_utils::now_iso(),
                    false,
                    None,
                )
                .await;
        }
        let trackers = runtime.inner.trackers.lock().await;
        let tracker = &trackers[id];
        assert_eq!(tracker.health.status, HealthStatus::Degraded);
        assert_eq!(tracker.health.consecutive_failures, 0);
        assert!(startup_grace_active(
            id,
            tracker,
            STARTUP_GRACE - Duration::from_millis(1)
        ));
        assert!(!startup_grace_active(id, tracker, STARTUP_GRACE));
        assert!(!startup_grace_active("storage", tracker, Duration::ZERO));
    }
    assert!(events(&state).await.is_empty());
    runtime
        .apply_sampled_probe(
            &state,
            "auth_bridge",
            sample(true),
            &time_utils::now_iso(),
            false,
            None,
        )
        .await;
    for _ in 0..3 {
        runtime
            .apply_sampled_probe(
                &state,
                "auth_bridge",
                sample(false),
                &time_utils::now_iso(),
                false,
                None,
            )
            .await;
    }
    let emitted = events(&state).await;
    assert_eq!(emitted.len(), 1);
    assert_eq!(emitted[0]["type"], "FN_EVENT_RUNTIME_HEALTH_FAILED");
    assert_eq!(emitted[0]["payload"]["component"], "auth_bridge");
}

#[tokio::test]
async fn blocked_incident_recovers_once_without_duplicate_failure_on_flapping() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    for _ in 0..3 {
        runtime
            .apply_probe(&state, "auth_bridge", sample(false), &time_utils::now_iso())
            .await;
    }
    let original = events(&state).await[0]["payload"]["incident_id"].clone();
    runtime
        .apply_sampled_blocked("auth_bridge", &time_utils::now_iso(), None)
        .await;
    // One good sample followed by failures must keep the original incident.
    runtime
        .apply_probe(&state, "auth_bridge", sample(true), &time_utils::now_iso())
        .await;
    for _ in 0..4 {
        runtime
            .apply_probe(&state, "auth_bridge", sample(false), &time_utils::now_iso())
            .await;
    }
    assert_eq!(events(&state).await.len(), 1);
    runtime
        .apply_sampled_blocked("auth_bridge", &time_utils::now_iso(), None)
        .await;
    for _ in 0..3 {
        runtime
            .apply_probe(&state, "auth_bridge", sample(true), &time_utils::now_iso())
            .await;
    }
    let emitted = events(&state).await;
    assert_eq!(emitted.len(), 2);
    let recovered = emitted
        .iter()
        .find(|event| event["type"] == "FN_EVENT_RUNTIME_RECOVERED")
        .unwrap();
    assert_eq!(recovered["payload"]["incident_id"], original);
    assert!(recovered["payload"]["duration_ms"].as_i64().unwrap() >= 0);
    assert_eq!(
        runtime.inner.trackers.lock().await["auth_bridge"]
            .health
            .status,
        HealthStatus::Healthy
    );
}

#[tokio::test]
async fn dependency_block_without_an_incident_emits_no_fake_recovery() {
    let (_directory, state) = tests::runtime_test_state().await;
    let runtime = &state.runtime_health;
    runtime
        .apply_sampled_blocked("auth_bridge", &time_utils::now_iso(), None)
        .await;
    for _ in 0..2 {
        runtime
            .apply_probe(&state, "auth_bridge", sample(true), &time_utils::now_iso())
            .await;
    }
    assert!(events(&state).await.is_empty());
}

#[tokio::test]
async fn startup_timeout_and_storage_failure_still_raise_real_alerts() {
    let (directory, state) = tests::runtime_test_state().await;
    let mut runtime = RuntimeHealth::new(directory.path(), "fpk").unwrap();
    Arc::get_mut(&mut runtime.inner).unwrap().process_started = Instant::now() - STARTUP_GRACE;
    for _ in 0..3 {
        runtime
            .apply_sampled_probe(
                &state,
                "gateway_process",
                sample(false),
                &time_utils::now_iso(),
                false,
                None,
            )
            .await;
        state
            .runtime_health
            .apply_sampled_probe(
                &state,
                "storage",
                sample(false),
                &time_utils::now_iso(),
                false,
                None,
            )
            .await;
    }
    let emitted = events(&state).await;
    assert_eq!(emitted.len(), 2);
    for id in ["gateway_process", "storage"] {
        assert!(
            emitted
                .iter()
                .any(|event| event["type"] == "FN_EVENT_RUNTIME_HEALTH_FAILED"
                    && event["payload"]["component"] == id)
        );
    }
}
