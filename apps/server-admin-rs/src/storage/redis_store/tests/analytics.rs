use super::*;

#[tokio::test]
async fn traffic_cleanup_prunes_stale_series_in_batches_and_preserves_live_counters() {
    let (_dir, store) = open_test_store().await;
    let now = chrono_like_now_seconds();
    let stale = (0..40)
        .map(|index| TrafficSnapshotRecord {
            host: Some(format!("stale-{index}.example.com")),
            stream: None,
            total_in: 10.0,
            total_out: 20.0,
            error_5xx: 0.0,
        })
        .collect::<Vec<_>>();
    store
        .record_traffic_snapshot("global", &stale, now - 120, 60)
        .await
        .unwrap();
    let live = TrafficSnapshotRecord {
        host: None,
        stream: None,
        total_in: 100.0,
        total_out: 200.0,
        error_5xx: 0.0,
    };
    store
        .record_traffic_snapshot("global", &[live], now, 60)
        .await
        .unwrap();
    assert_eq!(store.cleanup_traffic_metrics(60).await.unwrap(), 123);

    let traffic_keys = store
        .conn()
        .smembers(super::super::traffic::TRAFFIC_KEY_INDEX)
        .await
        .unwrap();
    assert_eq!(traffic_keys.len(), 2);
    assert!(traffic_keys.iter().all(|key| !key.contains("stale")));
    assert_eq!(
        store
            .get_string_value("fn_knock:traffic:last:global:in")
            .await
            .unwrap(),
        Some("100".to_string())
    );
    assert_eq!(
        store
            .get_string_value("fn_knock:traffic:last:global:host:stale-0.example.com:in")
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        store
            .list_error5xx_points("global", now - 1, now, None, None)
            .await
            .unwrap(),
        vec![TrafficDeltaPoint {
            ts: now,
            delta: 0.0
        }],
        "a zero-valued point still keeps its series and counter alive"
    );
    assert_eq!(store.cleanup_traffic_metrics(60).await.unwrap(), 3);
}

#[tokio::test]
async fn sorted_set_record_cap_removes_oldest_members() {
    let (_dir, store) = open_test_store().await;
    for (member, score) in [("old", 1), ("middle", 2), ("new", 3)] {
        store
            .zadd_string_member("fn_knock:test:bounded-history", member, score)
            .await
            .unwrap();
    }
    let removed = store
        .trim_oldest_zset_members("fn_knock:test:bounded-history", 2)
        .await
        .unwrap();
    assert_eq!(removed, vec!["old".to_string()]);
    assert_eq!(
        store
            .zrevrange_strings("fn_knock:test:bounded-history")
            .await
            .unwrap(),
        vec!["new".to_string(), "middle".to_string()]
    );
}

#[tokio::test]
async fn expired_key_gc_physically_removes_unread_keys() {
    let (_dir, store) = open_test_store().await;
    let key = "fn_knock:test:expired-gc";
    store
        .set_string_value_with_optional_ttl(key, "stale", Some(60))
        .await
        .unwrap();
    let connection = open_fixture_connection(&store.path);
    connection
        .execute("UPDATE kv_keys SET expires_at_ms = 0 WHERE key = ?1", [key])
        .unwrap();
    drop(connection);

    assert_eq!(store.purge_expired_keys().await.unwrap(), 1);
    assert_eq!(store.manager.key_count_by_prefix(key).await.unwrap(), 0);
}

#[test]
fn parses_traffic_members_and_ignores_invalid_values() {
    assert_eq!(
        parse_traffic_points(&[
            "10:5".to_string(),
            "bad".to_string(),
            "11:nope".to_string(),
            "12:0".to_string()
        ]),
        vec![
            TrafficDeltaPoint { ts: 10, delta: 5.0 },
            TrafficDeltaPoint { ts: 12, delta: 0.0 }
        ]
    );
}

#[tokio::test]
async fn traffic_history_reads_do_not_wait_for_primary_storage_executor() {
    let (_dir, store) = open_test_store().await;
    let manager = store.manager.clone();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let blocker = tokio::spawn(async move {
        manager
            .call(move |_conn| -> crate::storage::StorageResult<()> {
                let _ = started_tx.send(());
                release_rx
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .map_err(|error| {
                        crate::storage::storage_error(format!("release blocker: {error}"))
                    })?;
                Ok(())
            })
            .await
            .expect("primary executor blocker");
    });
    started_rx.await.expect("primary executor started");

    let points = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store.list_traffic_points("global", "in", 0, 10, None, None),
    )
    .await;
    release_tx.send(()).expect("release primary executor");
    let points = points
        .expect("analytics read should use its isolated executor")
        .expect("traffic history read");
    assert!(points.is_empty());
    blocker.await.expect("primary executor task");
}

#[test]
fn traffic_cleanup_maps_metric_keys_to_last_total_keys() {
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key(
            "fn_knock:traffic:global:host:example.com:in"
        )
        .as_deref(),
        Some("fn_knock:traffic:last:global:host:example.com:in")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:traffic:global:out")
            .as_deref(),
        Some("fn_knock:traffic:last:global:out")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:errors:global:5xx")
            .as_deref(),
        Some("fn_knock:errors:last:global:5xx")
    );
    assert_eq!(
        super::traffic::traffic_last_total_key_for_metric_key("fn_knock:traffic:global:bad"),
        None
    );
}

#[test]
fn counter_delta_handles_first_sample_and_resets() {
    assert_eq!(compute_counter_delta(100.0, None), 100.0);
    assert_eq!(compute_counter_delta(120.0, Some(100.0)), 20.0);
    assert_eq!(compute_counter_delta(12.0, Some(100.0)), 12.0);
    assert_eq!(compute_counter_delta(-1.0, Some(100.0)), 0.0);
}

#[test]
fn waf_log_dates_include_neighboring_utc_days() {
    let dates = waf_log_dates_for_range(1_704_067_200_000, 1_704_153_600_000);
    assert!(dates.contains(&"2023-12-31".to_string()));
    assert!(dates.contains(&"2024-01-01".to_string()));
    assert!(dates.contains(&"2024-01-02".to_string()));
    assert!(dates.contains(&"2024-01-03".to_string()));
}

#[tokio::test]
async fn waf_event_persistence_is_atomic_and_idempotent_for_lease_retries() {
    let (_dir, store) = open_test_store().await;
    let event = json!({
        "trace_id": "waf_retry",
        "time": "2026-08-15T15:28:20Z",
        "action": "deny",
        "status": 403,
        "rule_ids": [921150]
    });
    let events = vec![event.clone()];

    let (first, duplicate) = tokio::join!(
        store.persist_waf_events(&events, 7),
        store.persist_waf_events(&events, 7)
    );
    first.expect("persist leased event");
    duplicate.expect("persist duplicate lease delivery");

    assert_eq!(
        store.get_waf_log_event("waf_retry").await.unwrap(),
        Some(event)
    );
    let score = crate::time_utils::parse_iso_ms("2026-08-15T15:28:20Z").unwrap();
    let date = crate::time_utils::local_date_from_ms(score);
    assert_eq!(store.waf_log_date_total(&date).await.unwrap(), 1);
    let stats = store
        .conn()
        .hgetall(&waf_log_stats_key(&date))
        .await
        .unwrap();
    assert_eq!(stats.get("events").map(String::as_str), Some("1"));
    assert_eq!(stats.get("action:deny").map(String::as_str), Some("1"));
}
