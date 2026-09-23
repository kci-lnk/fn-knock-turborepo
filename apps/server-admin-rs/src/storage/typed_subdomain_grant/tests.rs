use super::*;
use tokio_rusqlite::rusqlite::Connection;

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE kv_keys (key TEXT PRIMARY KEY, kind TEXT NOT NULL, expires_at_ms INTEGER);
         CREATE TABLE kv_strings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
         CREATE TABLE kv_zset (key TEXT NOT NULL, member TEXT NOT NULL, score REAL NOT NULL,
           PRIMARY KEY (key, member));",
    )
    .unwrap();
    conn
}

fn raw_grant() -> String {
    serde_json::json!({
        "host": "grant.example.test", "policy_version": "policy-a", "group_id": "group-a",
        "issued_at": 1, "last_access_at": 2, "hard_expires_at": 9_999_999_999_i64
    })
    .to_string()
}

fn insert_member(
    tx: &Transaction<'_>,
    host: &str,
    key: &str,
    score: f64,
    value: Option<(&str, Option<i64>, &str)>,
) {
    tx.execute(
        "INSERT INTO kv_zset VALUES (?1, ?2, ?3)",
        params![host, key, score],
    )
    .unwrap();
    if let Some((kind, expiry, raw)) = value {
        tx.execute(
            "INSERT INTO kv_keys VALUES (?1, ?2, ?3)",
            params![key, kind, expiry],
        )
        .unwrap();
        tx.execute("INSERT INTO kv_strings VALUES (?1, ?2)", params![key, raw])
            .unwrap();
    }
}

// The original N+1 implementation is retained only as a differential oracle.
fn previous_entries(
    tx: &Transaction<'_>,
    key: &str,
) -> StorageResult<(Vec<TypedSubdomainGrantActiveEntry>, bool)> {
    let Some(host_digest) = parse_active_key(key) else {
        return Ok((Vec::new(), true));
    };
    let mut statement =
        tx.prepare("SELECT member, score FROM kv_zset WHERE key = ?1 ORDER BY member")?;
    let rows = statement.query_map([key], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let mut entries = Vec::new();
    let mut invalid = false;
    for row in rows {
        let (member, score) = row?;
        let Some(grant_digest) = parse_grant_key(&member) else {
            invalid = true;
            continue;
        };
        if live_legacy_grant_tx(tx, &member)?.is_none() {
            continue;
        }
        if !score.is_finite()
            || score.fract() != 0.0
            || score < i64::MIN as f64
            || score > i64::MAX as f64
        {
            invalid = true;
            continue;
        }
        entries.push(TypedSubdomainGrantActiveEntry {
            host_digest: host_digest.to_ascii_lowercase(),
            grant_digest: grant_digest.to_ascii_lowercase(),
            expires_at_score: score as i64,
        });
    }
    entries.sort();
    Ok((entries, invalid))
}

#[test]
fn batched_active_entries_preserve_legacy_validation_and_orphans() {
    let mut conn = database();
    let tx = conn.transaction().unwrap();
    let host = format!("{ACTIVE_INDEX_PREFIX}{}", "A".repeat(64));
    let expiry = crate::time_utils::now_ms() + 60_000;
    let raw = raw_grant();
    let mut expected_live = Vec::new();
    for (index, kind, expires, json, score) in [
        (1, "string", Some(expiry), raw.as_str(), 10.0),
        (2, "string", Some(expiry), raw.as_str(), 20.0),
        (3, "string", Some(0), raw.as_str(), 1.5),
        (4, "string", None, raw.as_str(), 1.5),
        (5, "hash", Some(expiry), raw.as_str(), 1.5),
        (6, "string", Some(expiry), "{broken", 1.5),
    ] {
        let key = format!("{GRANT_PREFIX}{index:064X}");
        insert_member(&tx, &host, &key, score, Some((kind, expires, json)));
        if index < 3 {
            expected_live.push(TypedSubdomainGrantActiveEntry {
                host_digest: "a".repeat(64),
                grant_digest: format!("{index:064x}"),
                expires_at_score: score as i64,
            });
        }
    }
    insert_member(&tx, &host, &format!("{GRANT_PREFIX}{:064x}", 7), 1.5, None);
    let result = legacy_active_entries_checked_tx(&tx, &host).unwrap();
    assert_eq!(result, (expected_live, false));
    assert_eq!(result, previous_entries(&tx, &host).unwrap());

    // LEFT JOIN must retain the malformed member even when it has no grant.
    insert_member(&tx, &host, "invalid-member", 2.0, None);
    assert!(legacy_active_entries_checked_tx(&tx, &host).unwrap().1);
    assert_eq!(
        legacy_active_entries_checked_tx(&tx, &host).unwrap(),
        previous_entries(&tx, &host).unwrap()
    );
}

#[test]
fn batched_active_entries_reject_invalid_scores_only_for_live_grants() {
    let mut conn = database();
    let tx = conn.transaction().unwrap();
    let host = active_key("grant.example.test");
    let raw = raw_grant();
    let expiry = crate::time_utils::now_ms() + 60_000;
    for (index, score) in [1.5, f64::INFINITY, f64::NEG_INFINITY]
        .into_iter()
        .enumerate()
    {
        let key = format!("{GRANT_PREFIX}{index:064x}");
        insert_member(
            &tx,
            &host,
            &key,
            score,
            Some(("string", Some(expiry), &raw)),
        );
    }
    assert_eq!(
        legacy_active_entries_checked_tx(&tx, &host).unwrap(),
        (Vec::new(), true)
    );
    assert_eq!(
        legacy_active_entries_checked_tx(&tx, &host).unwrap(),
        previous_entries(&tx, &host).unwrap()
    );
}

#[test]
#[ignore = "manual paired elapsed-time comparison; no timing assertions"]
fn active_grant_batch_benchmark() {
    use std::{hint::black_box, time::Instant};
    for count in [1, 100, 1_000, 10_000] {
        let mut conn = database();
        let tx = conn.transaction().unwrap();
        let host = active_key("grant.example.test");
        let raw = raw_grant();
        let expiry = crate::time_utils::now_ms() + 3_600_000;
        for index in 0..count {
            insert_member(
                &tx,
                &host,
                &format!("{GRANT_PREFIX}{index:064x}"),
                100.0,
                Some(("string", Some(expiry), &raw)),
            );
        }
        assert_eq!(
            legacy_active_entries_checked_tx(&tx, &host).unwrap(),
            previous_entries(&tx, &host).unwrap()
        );
        for pair in 0..6 {
            for batch in if pair % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let started = Instant::now();
                for _ in 0..10 {
                    black_box(if batch {
                        legacy_active_entries_checked_tx(&tx, &host)
                    } else {
                        previous_entries(&tx, &host)
                    })
                    .unwrap();
                }
                eprintln!(
                    "grant_batch count={count} pair={pair} batch={batch} iterations=10 elapsed_us={}",
                    started.elapsed().as_micros()
                );
            }
        }
    }
}
