use super::*;

#[tokio::test]
async fn analytics_location_batches_preserve_order_and_expiry_without_primary_access() {
    let (_directory, store) = open_test_store().await;
    let mut ips = (0..300)
        .map(|index| format!("198.51.100.{index}"))
        .collect::<Vec<_>>();
    let mut connection = open_fixture_connection(&store.path);
    let tx = connection.transaction().unwrap();
    for (index, ip) in ips.iter().enumerate() {
        for kind in ["cache", "state"] {
            let key = format!("fn_knock:ip_location:{kind}:{ip}");
            let expires_at = if index == 1 && kind == "cache" {
                Some(0_i64)
            } else {
                None
            };
            tx.execute(
                "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, 'string', ?2)",
                tokio_rusqlite::rusqlite::params![key, expires_at],
            )
            .unwrap();
            let value = if index == 2 && kind == "cache" {
                "malformed-json".to_string()
            } else {
                json!({ "index": index, "kind": kind }).to_string()
            };
            tx.execute(
                "INSERT INTO kv_strings(key, value) VALUES (?1, ?2)",
                [&key, &value],
            )
            .unwrap();
        }
    }
    tx.commit().unwrap();
    ips.push(ips[0].clone());
    ips.push("missing".to_string());

    let (release, blocker) = block_primary_executor(&store).await;
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        store.get_ip_location_records_analytics(&ips),
    )
    .await;
    release.send(()).unwrap();
    blocker.await.unwrap();
    let records = result
        .expect("analytics must not wait for the primary writer")
        .unwrap();
    assert_eq!(records.len(), ips.len());
    for (index, (cache, state)) in records.iter().take(300).enumerate() {
        if index == 1 || index == 2 {
            assert_eq!(*cache, None);
        } else {
            assert_eq!(cache.as_ref().unwrap()["index"], json!(index));
        }
        assert_eq!(state.as_ref().unwrap()["index"], json!(index));
    }
    assert_eq!(
        records[300], records[0],
        "duplicate IPs retain their position"
    );
    assert_eq!(records[301], (None, None));
    let expired_still_exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM kv_keys WHERE key = 'fn_knock:ip_location:cache:198.51.100.1')",
        [],
        |row| row.get(0),
    ).unwrap();
    assert!(
        expired_still_exists,
        "read-only analytics must not perform TTL cleanup"
    );
}

#[tokio::test]
async fn ip_location_lock_can_only_be_released_by_its_owner() {
    let (_directory, store) = open_test_store().await;
    let ip = "203.0.113.10";

    assert!(
        store
            .acquire_ip_location_lock(ip, "owner-a", 60)
            .await
            .expect("acquire initial lock")
    );
    store
        .release_ip_location_lock(ip, "owner-b")
        .await
        .expect("ignore non-owner release");
    assert!(
        !store
            .acquire_ip_location_lock(ip, "owner-c", 60)
            .await
            .expect("lock remains owned")
    );

    store
        .release_ip_location_lock(ip, "owner-a")
        .await
        .expect("release owned lock");
    assert!(
        store
            .acquire_ip_location_lock(ip, "owner-c", 60)
            .await
            .expect("acquire after owner release")
    );
}
