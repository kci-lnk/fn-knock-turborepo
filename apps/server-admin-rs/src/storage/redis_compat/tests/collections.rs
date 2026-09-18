use super::*;

#[tokio::test]
async fn set_clears_existing_ttl() {
    let mut conn = temp_manager().await;
    conn.set_ex("fn_knock:test:string", "old", 60)
        .await
        .expect("set expiring value");
    assert!(conn.ttl("fn_knock:test:string").await.expect("read ttl") > 0);

    conn.set("fn_knock:test:string", "new")
        .await
        .expect("overwrite value");
    let value: Option<String> = conn.get("fn_knock:test:string").await.expect("read value");
    assert_eq!(value.as_deref(), Some("new"));
    assert_eq!(
        conn.ttl("fn_knock:test:string")
            .await
            .expect("read cleared ttl"),
        -1
    );
}

#[tokio::test]
async fn sorted_set_score_bounds_follow_redis_exclusive_syntax() {
    let mut conn = temp_manager().await;
    conn.zadd("fn_knock:test:zset-bounds", "a", 10)
        .await
        .expect("zadd a");
    conn.zadd("fn_knock:test:zset-bounds", "b", 20)
        .await
        .expect("zadd b");
    conn.zadd("fn_knock:test:zset-bounds", "c", 30)
        .await
        .expect("zadd c");

    let count: i64 = cmd("ZCOUNT")
        .arg("fn_knock:test:zset-bounds")
        .arg("10")
        .arg("(30")
        .query_async(&mut conn)
        .await
        .expect("zcount exclusive max");
    assert_eq!(count, 2);

    let members: Vec<String> = cmd("ZRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("(10")
        .arg("30")
        .query_async(&mut conn)
        .await
        .expect("zrangebyscore exclusive min");
    assert_eq!(members, vec!["b".to_string(), "c".to_string()]);

    let reverse_pairs: Vec<String> = cmd("ZREVRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("+inf")
        .arg("(10")
        .arg("WITHSCORES")
        .arg("LIMIT")
        .arg(0)
        .arg(2)
        .query_async(&mut conn)
        .await
        .expect("zrevrangebyscore reverse score bounds");
    assert_eq!(
        reverse_pairs,
        vec![
            "c".to_string(),
            "30".to_string(),
            "b".to_string(),
            "20".to_string(),
        ]
    );

    let _: () = cmd("ZREMRANGEBYSCORE")
        .arg("fn_knock:test:zset-bounds")
        .arg("-inf")
        .arg("(20")
        .query_async(&mut conn)
        .await
        .expect("zremrangebyscore exclusive max");
    assert_eq!(
        conn.zrange("fn_knock:test:zset-bounds", 0, -1)
            .await
            .expect("zrange remaining"),
        vec!["b".to_string(), "c".to_string()]
    );
}

#[tokio::test]
async fn ranges_outside_collection_bounds_are_empty() {
    let mut conn = temp_manager().await;
    let _: () = cmd("RPUSH")
        .arg("fn_knock:test:range-list")
        .arg(vec!["a".to_string(), "b".to_string(), "c".to_string()])
        .query_async(&mut conn)
        .await
        .expect("seed list");
    for (member, score) in [("a", 1), ("b", 2), ("c", 3)] {
        conn.zadd("fn_knock:test:range-zset", member, score)
            .await
            .expect("seed zset");
    }

    assert!(
        conn.lrange("fn_knock:test:range-list", 3, -1)
            .await
            .expect("list start at length")
            .is_empty()
    );
    assert!(
        conn.zrange("fn_knock:test:range-zset", 4, 10)
            .await
            .expect("zset start beyond length")
            .is_empty()
    );
    assert!(
        conn.lrange("fn_knock:test:range-list", 0, -4)
            .await
            .expect("list end before first item")
            .is_empty()
    );
    assert_eq!(
        conn.zrange("fn_knock:test:range-zset", -100, -1)
            .await
            .expect("large negative start"),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );
}

#[tokio::test]
async fn empty_collections_remove_their_redis_keys() {
    let mut conn = temp_manager().await;

    conn.hset("fn_knock:test:empty-hash", "field", "value")
        .await
        .expect("seed hash");
    conn.hdel("fn_knock:test:empty-hash", "field")
        .await
        .expect("empty hash");
    assert_eq!(conn.exists("fn_knock:test:empty-hash").await.unwrap(), 0);

    conn.sadd("fn_knock:test:empty-set", "member")
        .await
        .expect("seed set");
    conn.srem("fn_knock:test:empty-set", "member")
        .await
        .expect("empty set");
    assert_eq!(conn.exists("fn_knock:test:empty-set").await.unwrap(), 0);

    conn.zadd("fn_knock:test:empty-zset", "member", 1)
        .await
        .expect("seed zset");
    let _: () = cmd("ZREMRANGEBYSCORE")
        .arg("fn_knock:test:empty-zset")
        .arg("-inf")
        .arg("+inf")
        .query_async(&mut conn)
        .await
        .expect("empty zset");
    assert_eq!(conn.exists("fn_knock:test:empty-zset").await.unwrap(), 0);

    let _: () = cmd("RPUSH")
        .arg("fn_knock:test:empty-list")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("seed list");
    let _: () = cmd("LTRIM")
        .arg("fn_knock:test:empty-list")
        .arg(1)
        .arg(0)
        .query_async(&mut conn)
        .await
        .expect("empty list");
    assert_eq!(conn.exists("fn_knock:test:empty-list").await.unwrap(), 0);
}

#[tokio::test]
async fn analytics_batch_excludes_keys_that_expire_while_waiting_for_admission() {
    let conn = temp_manager().await;
    let key = "fn_knock:test:analytics-queued-expiry";
    let expires_at = now_ms() + 100;
    conn.call(move |connection| {
        let tx = immediate_transaction(connection)?;
        set_string_tx(&tx, key, "cached", Some(expires_at))?;
        tx.commit()?;
        Ok(())
    })
    .await
    .unwrap();
    let permit = conn
        .analytics_admission
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let read = conn.get_live_strings_analytics(vec![key.to_string()]);
    tokio::pin!(read);
    tokio::select! {
        _ = &mut read => panic!("analytics admission should still be occupied"),
        _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {}
    }
    assert!(now_ms() >= expires_at);
    drop(permit);
    assert_eq!(read.await.unwrap(), vec![None]);
    let still_exists = conn
        .call(move |connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM kv_keys WHERE key = ?1)",
                    [key],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(Into::into)
        })
        .await
        .unwrap();
    assert!(still_exists, "analytics reads must remain read-only");
}

#[tokio::test]
async fn mget_batches_preserve_order_duplicates_types_and_missing_values_without_writes() {
    let manager = temp_manager().await;
    manager
        .call(|conn| {
            let tx = immediate_transaction(conn)?;
            for index in 0..1205 {
                set_string_tx(
                    &tx,
                    &format!("batch:{index}"),
                    &format!("value:{index}"),
                    None,
                )?;
            }
            set_string_tx(&tx, "future", "alive", Some(now_ms() + 60_000))?;
            ensure_key_tx(&tx, "hash", "hash", None)?;
            // Quoting characters must stay parameters, never become SQL syntax.
            set_string_tx(&tx, "quote:'?雪", "quoted", None)?;
            let mut keys = (0..1205)
                .rev()
                .map(|index| format!("batch:{index}"))
                .collect::<Vec<_>>();
            keys.extend(
                [
                    "missing",
                    "hash",
                    "batch:400",
                    "batch:400",
                    "future",
                    "quote:'?雪",
                ]
                .map(str::to_string),
            );
            let expected = keys
                .iter()
                .map(|key| string_get_tx(&tx, key))
                .collect::<RedisResult<Vec<_>>>()?;
            let changes = tx.total_changes();
            assert_eq!(strings_get_tx(&tx, &keys)?, expected);
            assert_eq!(
                tx.total_changes(),
                changes,
                "live reads should not mutate storage"
            );
            assert!(strings_get_tx(&tx, &[])?.is_empty());
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn mget_expiry_keeps_cleanup_and_cascade_semantics() {
    let mut manager = temp_manager().await;
    manager
        .call(|conn| {
            let tx = immediate_transaction(conn)?;
            set_string_tx(&tx, "expired", "stale", Some(now_ms() - 1))?;
            tx.execute(
                "UPDATE kv_strings SET value = x'80' WHERE key = 'expired'",
                [],
            )?;
            set_string_tx(&tx, "live", "fresh", None)?;
            ensure_key_tx(&tx, "expired_hash", "hash", Some(now_ms() - 1))?;
            tx.execute(
                "INSERT INTO kv_hash VALUES ('expired_hash', 'field', 'stale')",
                [],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
    let values: Vec<Option<String>> = cmd("MGET")
        .arg(
            [
                "live",
                "expired",
                "expired_hash",
                "expired",
                "missing",
                "live",
            ]
            .map(str::to_string)
            .to_vec(),
        )
        .query_async(&mut manager)
        .await
        .unwrap();
    assert_eq!(
        values,
        vec![
            Some("fresh".into()),
            None,
            None,
            None,
            None,
            Some("fresh".into())
        ]
    );
    manager
        .call(|conn| {
            let count: i64 = conn.query_row(
                "SELECT (SELECT COUNT(*) FROM kv_keys WHERE key IN ('expired', 'expired_hash'))
                  + (SELECT COUNT(*) FROM kv_strings WHERE key = 'expired')
                  + (SELECT COUNT(*) FROM kv_hash WHERE key = 'expired_hash')",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .await
        .unwrap();
}

#[test]
#[ignore = "manual elapsed-time comparison; timing is not a CI assertion"]
fn mget_event_batch_elapsed_comparison() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    conn.execute_batch(REDIS_COMPATIBLE_KEYSPACE_SQL).unwrap();
    conn.pragma_update(None, "foreign_keys", "ON").unwrap();
    let tx = immediate_transaction(&mut conn).unwrap();
    let keys = (0..200).map(|i| format!("event:{i}")).collect::<Vec<_>>();
    for key in &keys {
        set_string_tx(&tx, key, &"x".repeat(500), None).unwrap();
    }
    let expected = keys
        .iter()
        .map(|key| string_get_tx(&tx, key).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(strings_get_tx(&tx, &keys).unwrap(), expected);
    let started = Instant::now();
    for _ in 0..100 {
        for key in &keys {
            std::hint::black_box(string_get_tx(&tx, key).unwrap());
        }
    }
    let original = started.elapsed();
    let started = Instant::now();
    for _ in 0..100 {
        std::hint::black_box(strings_get_tx(&tx, &keys).unwrap());
    }
    eprintln!(
        "200 event keys x 100, original={original:?}, batched={:?}",
        started.elapsed()
    );
}

#[tokio::test]
async fn mget_checks_expiry_after_primary_admission() {
    let mut manager = temp_manager().await;
    manager.set("queued-expiry", "stale").await.unwrap();
    let permit = manager
        .primary_admission
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let worker = manager.clone();
    let read = cmd("MGET")
        .arg("queued-expiry")
        .query_async::<Vec<Option<String>>>(&mut manager);
    tokio::pin!(read);
    // Poll once to put the read behind admission, then expire the key without
    // timing sleeps. The queued read must sample expiry after it is admitted.
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(read.as_mut(), cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
    worker
        .db
        .call(|conn| {
            conn.execute(
                "UPDATE kv_keys SET expires_at_ms = 0 WHERE key = 'queued-expiry'",
                [],
            )?;
            Ok::<(), rusqlite::Error>(())
        })
        .await
        .unwrap();
    drop(permit);
    assert_eq!(read.await.unwrap(), vec![None]);
    worker
        .call(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kv_keys WHERE key = 'queued-expiry'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn mget_later_batch_failure_rolls_back_earlier_expiry_cleanup() {
    let mut manager = temp_manager().await;
    manager
        .call(|conn| {
            let tx = immediate_transaction(conn)?;
            set_string_tx(&tx, "expired-first-batch", "old", Some(now_ms() - 1))?;
            set_string_tx(&tx, "corrupt-next-batch", "invalid", None)?;
            tx.execute(
                "UPDATE kv_strings SET value = x'80' WHERE key = 'corrupt-next-batch'",
                [],
            )?;
            tx.commit()?;
            Ok(())
        })
        .await
        .unwrap();
    let mut keys = vec!["expired-first-batch".to_string(); 400];
    keys.push("corrupt-next-batch".to_string());
    assert!(
        cmd("MGET")
            .arg(keys)
            .query_async::<Vec<Option<String>>>(&mut manager)
            .await
            .is_err()
    );
    manager
        .call(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM kv_keys WHERE key = 'expired-first-batch'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(
                count, 1,
                "a later chunk must not commit an earlier chunk's cleanup"
            );
            Ok(())
        })
        .await
        .unwrap();
}
