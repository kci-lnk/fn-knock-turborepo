use super::*;
use tokio_util::task::AbortOnDropHandle;

#[tokio::test]
async fn auth_metadata_reads_keep_snapshot_isolation_in_every_reader_mode() {
    for count in [1, 2, 4] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("auth-reads.sqlite3");
        let mut manager = ConnectionManager::open_with_auth_readers(&path, count)
            .await
            .unwrap();
        manager
            .set("settings", r#"{"enabled":true}"#)
            .await
            .unwrap();
        manager
            .hset("blacklist", "malformed", "still-blocked")
            .await
            .unwrap();
        manager.zadd("recent", "203.0.113.56", 12345).await.unwrap();
        let fixture = rusqlite::Connection::open(&path).unwrap();
        fixture.execute_batch("BEGIN IMMEDIATE; UPDATE kv_keys SET expires_at_ms = 0 WHERE key IN ('settings','blacklist','recent');").unwrap();
        // Uncommitted writes must not leak through any pool slot. All three
        // authorization reads must remain usable while the writer is held.
        let result = tokio::time::timeout(std::time::Duration::from_millis(500), async {
            let mut reads = Vec::new();
            for _ in 0..16 {
                let reader = manager.clone();
                reads.push(AbortOnDropHandle::new(tokio::spawn(async move {
                    assert_eq!(
                        reader
                            .get_auth_live_strings(vec!["settings".into()])
                            .await
                            .unwrap(),
                        vec![Some(r#"{"enabled":true}"#.into())]
                    );
                    assert!(
                        reader
                            .auth_live_key_exists("blacklist".into())
                            .await
                            .unwrap()
                    );
                    assert_eq!(
                        reader
                            .get_auth_live_zscore("recent".into(), "203.0.113.56".into())
                            .await
                            .unwrap(),
                        Some(12345)
                    );
                })));
            }
            for read in reads {
                read.await.unwrap();
            }
        })
        .await;
        fixture.execute_batch("COMMIT").unwrap();
        result.expect("authorization readers must not wait for the writer");
        for _ in 0..count * 2 {
            assert_eq!(
                manager
                    .get_auth_live_strings(vec!["settings".into()])
                    .await
                    .unwrap(),
                vec![None]
            );
            assert!(
                !manager
                    .auth_live_key_exists("blacklist".into())
                    .await
                    .unwrap()
            );
            assert_eq!(
                manager
                    .get_auth_live_zscore("recent".into(), "203.0.113.56".into())
                    .await
                    .unwrap(),
                None
            );
        }
        assert_eq!(
            fixture
                .query_row(
                    "SELECT COUNT(*) FROM kv_keys WHERE key IN ('settings','blacklist','recent')",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .unwrap(),
            3
        );
    }
}
