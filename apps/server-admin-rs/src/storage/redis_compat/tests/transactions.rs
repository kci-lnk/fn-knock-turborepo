use super::*;

#[tokio::test]
async fn keyspace_primitives_cover_core_types() {
    let mut conn = temp_manager().await;

    let _: () = cmd("HSET")
        .arg("fn_knock:test:hash")
        .arg("field")
        .arg("value")
        .query_async(&mut conn)
        .await
        .expect("hset");
    let hash = conn.hgetall("fn_knock:test:hash").await.expect("hgetall");
    assert_eq!(hash.get("field").map(String::as_str), Some("value"));

    conn.sadd("fn_knock:test:set", vec!["b".to_string(), "a".to_string()])
        .await
        .expect("sadd");
    assert_eq!(
        conn.smembers("fn_knock:test:set").await.expect("smembers"),
        vec!["a".to_string(), "b".to_string()]
    );

    let _: () = cmd("RPUSH")
        .arg("fn_knock:test:list")
        .arg(vec!["one".to_string(), "two".to_string()])
        .query_async(&mut conn)
        .await
        .expect("rpush");
    assert_eq!(
        conn.lrange("fn_knock:test:list", 0, -1)
            .await
            .expect("lrange"),
        vec!["one".to_string(), "two".to_string()]
    );

    conn.zadd("fn_knock:test:zset", "low", 1)
        .await
        .expect("zadd low");
    conn.zadd("fn_knock:test:zset", "high", 2)
        .await
        .expect("zadd high");
    assert_eq!(
        conn.zrevrange("fn_knock:test:zset", 0, 0)
            .await
            .expect("zrevrange"),
        vec!["high".to_string()]
    );

    let stream_id: String = cmd("XADD")
        .arg("fn_knock:test:stream")
        .arg("*")
        .arg("kind")
        .arg("created")
        .query_async(&mut conn)
        .await
        .expect("xadd");
    let stream_entries: Vec<(String, Vec<String>)> = cmd("XRANGE")
        .arg("fn_knock:test:stream")
        .arg("-")
        .arg("+")
        .query_async(&mut conn)
        .await
        .expect("xrange");
    assert_eq!(stream_entries.len(), 1);
    assert_eq!(stream_entries[0].0, stream_id);
    assert_eq!(
        stream_entries[0].1,
        vec!["kind".to_string(), "created".to_string()]
    );

    let (_, keys): (String, Vec<String>) = cmd("SCAN")
        .arg("0")
        .arg("MATCH")
        .arg("fn_knock:test:*")
        .arg("COUNT")
        .arg(100)
        .query_async(&mut conn)
        .await
        .expect("scan");
    assert!(keys.contains(&"fn_knock:test:hash".to_string()));
    assert!(keys.contains(&"fn_knock:test:stream".to_string()));
}

#[tokio::test]
async fn pipeline_can_replace_prefix_atomically() {
    let mut conn = temp_manager().await;
    conn.set("fn_knock:old:a", "a").await.expect("set old a");
    conn.set("fn_knock:old:b", "b").await.expect("set old b");
    conn.set("other:old", "keep")
        .await
        .expect("set outside key");

    let mut restore = pipe();
    restore.set("fn_knock:new", "value").ignore();
    let (deleted, _): (usize, ()) = restore
        .query_async_replacing_prefix(&mut conn, "fn_knock:")
        .await
        .expect("replace prefix");

    assert_eq!(deleted, 2);
    let old: Option<String> = conn.get("fn_knock:old:a").await.expect("read old");
    let restored: Option<String> = conn.get("fn_knock:new").await.expect("read restored");
    let outside: Option<String> = conn.get("other:old").await.expect("read outside");
    assert_eq!(old, None);
    assert_eq!(restored.as_deref(), Some("value"));
    assert_eq!(outside.as_deref(), Some("keep"));
}

#[tokio::test]
async fn caller_owned_hash_cas_commits_typed_and_compatibility_writes_together() {
    let mut manager = temp_manager().await;
    let _: () = cmd("HSET")
        .arg("fn_knock:test:cas-records")
        .arg("record-1")
        .arg("old")
        .query_async(&mut manager)
        .await
        .expect("seed compatibility record");
    manager
        .call(|conn| {
            conn.execute_batch(
                "CREATE TABLE test_typed_records (
                       id TEXT PRIMARY KEY,
                       document_json TEXT NOT NULL
                     );",
            )?;
            Ok(())
        })
        .await
        .expect("create typed test table");

    let applied = manager
        .call(|conn| {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let matched = hash_field_matches_in_transaction(
                &tx,
                "fn_knock:test:cas-records",
                "record-1",
                |current| current == Some("old"),
            )?;
            if matched {
                tx.execute(
                    "INSERT INTO test_typed_records(id, document_json) VALUES (?1, ?2)",
                    params!["record-1", "new"],
                )?;
                let mut pipeline = pipe();
                pipeline
                    .hset("fn_knock:test:cas-records", "record-1", "new")
                    .ignore();
                pipeline
                    .zadd("fn_knock:test:cas-order", "record-1", 1)
                    .ignore();
                pipeline.query_in_transaction::<()>(&tx)?;
            }
            tx.commit()?;
            Ok(matched)
        })
        .await
        .expect("apply caller-owned CAS");
    assert!(applied);

    let (typed, compatibility, indexed) = manager
        .call(|conn| {
            let typed = conn.query_row(
                "SELECT document_json FROM test_typed_records WHERE id = ?1",
                ["record-1"],
                |row| row.get::<_, String>(0),
            )?;
            let compatibility = conn.query_row(
                "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                params!["fn_knock:test:cas-records", "record-1"],
                |row| row.get::<_, String>(0),
            )?;
            let indexed = conn.query_row(
                "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND member = ?2",
                params!["fn_knock:test:cas-order", "record-1"],
                |row| row.get::<_, i64>(0),
            )?;
            Ok((typed, compatibility, indexed))
        })
        .await
        .expect("read committed dual write");
    assert_eq!(typed, "new");
    assert_eq!(compatibility, "new");
    assert_eq!(indexed, 1);

    let stale_applied = manager
        .call(|conn| {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            let matched = hash_field_matches_in_transaction(
                &tx,
                "fn_knock:test:cas-records",
                "record-1",
                |current| current == Some("old"),
            )?;
            if matched {
                tx.execute(
                    "UPDATE test_typed_records SET document_json = 'stale' WHERE id = 'record-1'",
                    [],
                )?;
                let mut pipeline = pipe();
                pipeline
                    .hset("fn_knock:test:cas-records", "record-1", "stale")
                    .ignore();
                pipeline.query_in_transaction::<()>(&tx)?;
            }
            tx.commit()?;
            Ok(matched)
        })
        .await
        .expect("reject stale caller-owned CAS");
    assert!(!stale_applied);
}

#[tokio::test]
async fn caller_owned_pipeline_failure_rolls_back_typed_write() {
    let manager = temp_manager().await;
    manager
        .call(|conn| {
            conn.execute_batch(
                "CREATE TABLE test_typed_rollback (
                       id TEXT PRIMARY KEY,
                       document_json TEXT NOT NULL
                     );",
            )?;
            Ok(())
        })
        .await
        .expect("create rollback test table");

    let error = manager
        .call(|conn| {
            let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO test_typed_rollback(id, document_json) VALUES ('record-1', 'new')",
                [],
            )?;
            let mut pipeline = pipe();
            pipeline.cmd("UNSUPPORTED_FOR_ROLLBACK_TEST").ignore();
            pipeline.query_in_transaction::<()>(&tx)?;
            tx.commit()?;
            Ok(())
        })
        .await
        .expect_err("unsupported compatibility command must fail");
    assert!(
        error
            .to_string()
            .contains("unsupported Redis-compatible command")
    );

    let count = manager
        .call(|conn| {
            conn.query_row("SELECT COUNT(*) FROM test_typed_rollback", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(Into::into)
        })
        .await
        .expect("count rolled-back typed rows");
    assert_eq!(count, 0);
}

#[tokio::test]
async fn concurrent_caller_owned_hash_cas_has_one_winner() {
    let mut manager = temp_manager().await;
    let _: () = cmd("HSET")
        .arg("fn_knock:test:concurrent-cas")
        .arg("record-1")
        .arg("start")
        .query_async(&mut manager)
        .await
        .expect("seed concurrent CAS record");

    let apply = |manager: ConnectionManager, replacement: &'static str| async move {
        manager
            .call(move |conn| {
                let tx =
                    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
                let matched = hash_field_matches_in_transaction(
                    &tx,
                    "fn_knock:test:concurrent-cas",
                    "record-1",
                    |current| current == Some("start"),
                )?;
                if matched {
                    let mut pipeline = pipe();
                    pipeline
                        .hset("fn_knock:test:concurrent-cas", "record-1", replacement)
                        .ignore();
                    pipeline.query_in_transaction::<()>(&tx)?;
                }
                tx.commit()?;
                Ok(matched)
            })
            .await
    };
    let (left, right) = tokio::join!(
        apply(manager.clone(), "left"),
        apply(manager.clone(), "right")
    );
    let left = left.expect("left CAS");
    let right = right.expect("right CAS");
    assert_ne!(left, right);

    let final_value: Option<String> = manager
        .hget("fn_knock:test:concurrent-cas", "record-1")
        .await
        .expect("read concurrent CAS winner");
    assert!(matches!(final_value.as_deref(), Some("left" | "right")));
}
