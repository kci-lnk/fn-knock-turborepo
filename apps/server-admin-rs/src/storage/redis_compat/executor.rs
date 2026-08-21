use super::*;

pub(super) fn execute_command_tx(
    tx: &rusqlite::Transaction<'_>,
    command: CommandSpec,
) -> RedisResult<CmdOutput> {
    let args = command.args;
    match command.name.as_str() {
        "PING" => Ok(CmdOutput::Nil),
        "TYPE" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::String(
                key_kind_tx(tx, key)?.unwrap_or_else(|| "none".to_string()),
            ))
        }
        "PTTL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let expires_at: Option<Option<i64>> = tx
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(CmdOutput::Int(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => (value - now_ms()).max(0),
            }))
        }
        "TTL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let expires_at: Option<Option<i64>> = tx
                .query_row(
                    "SELECT expires_at_ms FROM kv_keys WHERE key = ?1",
                    params![key],
                    |row| row.get(0),
                )
                .optional()?;
            Ok(CmdOutput::Int(match expires_at {
                None => -2,
                Some(None) => -1,
                Some(Some(value)) => ((value - now_ms()).max(0) + 999) / 1000,
            }))
        }
        "GET" => {
            let key = arg(&args, 0)?;
            Ok(CmdOutput::OptionalString(string_get_tx(tx, key)?))
        }
        "MGET" => {
            let mut values = Vec::with_capacity(args.len());
            for key in args {
                values.push(string_get_tx(tx, &key)?);
            }
            Ok(CmdOutput::OptionalStrings(values))
        }
        "SET" => set_command_tx(tx, &args),
        "SETEX" => {
            let key = arg(&args, 0)?.to_string();
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            let value = arg(&args, 2)?.to_string();
            set_string_tx(tx, &key, &value, Some(now_ms() + ttl * 1000))?;
            Ok(CmdOutput::Nil)
        }
        "DEL" => {
            let mut deleted = 0usize;
            for key in args {
                purge_expired_tx(tx, &key)?;
                deleted += delete_key_tx(tx, &key)?;
            }
            Ok(CmdOutput::Int(deleted as i64))
        }
        "EXPIRE" => {
            let key = arg(&args, 0)?;
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            purge_expired_tx(tx, key)?;
            tx.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, now_ms() + ttl * 1000],
            )?;
            Ok(CmdOutput::Nil)
        }
        "PEXPIRE" => {
            let key = arg(&args, 0)?;
            let ttl = parse_i64(arg(&args, 1)?)?.max(1);
            purge_expired_tx(tx, key)?;
            tx.execute(
                "UPDATE kv_keys SET expires_at_ms = ?2 WHERE key = ?1",
                params![key, now_ms() + ttl],
            )?;
            Ok(CmdOutput::Nil)
        }
        "HSET" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "hash", None)?;
            for pair in args[1..].chunks(2) {
                if pair.len() == 2 {
                    tx.execute(
                        "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                        params![key, pair[0], pair[1]],
                    )?;
                }
            }
            Ok(CmdOutput::Nil)
        }
        "HDEL" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for field in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "hash")?;
            Ok(CmdOutput::Nil)
        }
        "HMGET" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let mut values = Vec::new();
            for field in &args[1..] {
                values.push(
                    tx.query_row(
                        "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                        params![key, field],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?,
                );
            }
            Ok(CmdOutput::OptionalStrings(values))
        }
        "HINCRBY" => {
            let key = arg(&args, 0)?.to_string();
            let field = arg(&args, 1)?.to_string();
            let delta = parse_i64(arg(&args, 2)?)?;
            ensure_key_tx(tx, &key, "hash", None)?;
            let current = tx
                .query_row(
                    "SELECT value FROM kv_hash WHERE key = ?1 AND field = ?2",
                    params![key, field],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0);
            let next = current + delta;
            tx.execute(
                "INSERT OR REPLACE INTO kv_hash(key, field, value) VALUES (?1, ?2, ?3)",
                params![key, field, next.to_string()],
            )?;
            Ok(CmdOutput::Int(next))
        }
        "SADD" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "set", None)?;
            for member in &args[1..] {
                tx.execute(
                    "INSERT OR IGNORE INTO kv_set(key, member) VALUES (?1, ?2)",
                    params![key, member],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "SREM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for member in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_set WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "set")?;
            Ok(CmdOutput::Nil)
        }
        "ZADD" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "zset", None)?;
            for pair in args[1..].chunks(2) {
                if pair.len() == 2 {
                    tx.execute(
                        "INSERT OR REPLACE INTO kv_zset(key, member, score) VALUES (?1, ?2, ?3)",
                        params![key, pair[1], parse_f64(&pair[0])?],
                    )?;
                }
            }
            Ok(CmdOutput::Nil)
        }
        "ZREM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            for member in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, key, "zset")?;
            Ok(CmdOutput::Nil)
        }
        "ZREMRANGEBYSCORE" => {
            let key = arg(&args, 0)?;
            let min = parse_score_bound(arg(&args, 1)?)?;
            let max = parse_score_bound(arg(&args, 2)?)?;
            purge_expired_tx(tx, key)?;
            delete_zset_score_range_tx(tx, key, min, max)?;
            Ok(CmdOutput::Nil)
        }
        "ZCARD" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::Int(count_rows_tx(
                tx,
                "SELECT COUNT(*) FROM kv_zset WHERE key = ?1",
                &[&key],
            )?))
        }
        "ZCOUNT" => {
            let key = arg(&args, 0)?;
            let min = parse_score_bound(arg(&args, 1)?)?;
            let max = parse_score_bound(arg(&args, 2)?)?;
            purge_expired_tx(tx, key)?;
            Ok(CmdOutput::Int(count_zset_score_range_tx(
                tx, key, min, max,
            )?))
        }
        "ZRANGEBYSCORE" => zrangebyscore_command_tx(tx, &args, false),
        "ZREVRANGEBYSCORE" => zrangebyscore_command_tx(tx, &args, true),
        "ZRANGE" => zrange_command_tx(tx, &args, false),
        "RPUSH" => {
            let key = arg(&args, 0)?.to_string();
            ensure_key_tx(tx, &key, "list", None)?;
            let idx = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
            for (offset, value) in args[1..].iter().enumerate() {
                tx.execute(
                    "INSERT INTO kv_list(key, idx, value) VALUES (?1, ?2, ?3)",
                    params![key, idx + offset as i64, value],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "LTRIM" => {
            let key = arg(&args, 0)?.to_string();
            let start = parse_i64(arg(&args, 1)?)? as isize;
            let end = parse_i64(arg(&args, 2)?)? as isize;
            purge_expired_tx(tx, &key)?;
            let values = list_range_tx(tx, &key, start, end)?;
            tx.execute("DELETE FROM kv_list WHERE key = ?1", params![key])?;
            for (idx, value) in values.iter().enumerate() {
                tx.execute(
                    "INSERT INTO kv_list(key, idx, value) VALUES (?1, ?2, ?3)",
                    params![key, idx as i64, value],
                )?;
            }
            delete_collection_key_if_empty_tx(tx, &key, "list")?;
            Ok(CmdOutput::Nil)
        }
        "INCRBY" => {
            let key = arg(&args, 0)?.to_string();
            let delta = parse_i64(arg(&args, 1)?)?;
            let current = string_get_tx(tx, &key)?
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(0);
            let next = current + delta;
            set_string_preserve_ttl_tx(tx, &key, &next.to_string())?;
            Ok(CmdOutput::Int(next))
        }
        "SCAN" => {
            let mut prefix = "";
            for pair in args.windows(2) {
                if pair[0].eq_ignore_ascii_case("MATCH") {
                    prefix = pair[1].trim_end_matches('*');
                }
            }
            let keys = scan_keys_tx(tx, prefix)?;
            Ok(CmdOutput::Scan("0".to_string(), keys))
        }
        "XADD" => xadd_command_tx(tx, &args),
        "XRANGE" => {
            let key = arg(&args, 0)?;
            let min = parse_stream_bound(arg(&args, 1)?, true)?;
            let max = parse_stream_bound(arg(&args, 2)?, false)?;
            purge_expired_tx(tx, key)?;
            let entries = query_stream_rows(tx, key, min, false, max, false, usize::MAX)?;
            Ok(CmdOutput::StreamEntries(
                entries
                    .into_iter()
                    .map(|(id, fields_json)| Ok((id, stream_fields_vec(&fields_json)?)))
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?,
            ))
        }
        "XTRIM" => {
            let key = arg(&args, 0)?;
            purge_expired_tx(tx, key)?;
            let strategy = arg(&args, 1)?.to_ascii_uppercase();
            match strategy.as_str() {
                "MINID" => {
                    let min_id = parse_stream_id(arg(&args, args.len().saturating_sub(1))?)
                        .ok_or_else(|| storage_error("XTRIM MINID requires a valid stream ID"))?;
                    let (min_ms, min_sequence) = stream_id_sql_tuple(min_id)?;
                    tx.execute(
                        "DELETE FROM kv_stream
                         WHERE key = ?1 AND (id_ms, id_sequence) < (?2, ?3)",
                        params![key, min_ms, min_sequence],
                    )?;
                }
                "MAXLEN" => {
                    let max_len = parse_i64(arg(&args, args.len().saturating_sub(1))?)?.max(0);
                    tx.execute(
                        "DELETE FROM kv_stream
                         WHERE key = ?1 AND rowid IN (
                           SELECT rowid FROM kv_stream
                           WHERE key = ?1
                           ORDER BY id_ms DESC, id_sequence DESC
                           LIMIT -1 OFFSET ?2
                         )",
                        params![key, max_len],
                    )?;
                }
                _ => return Err(storage_error("XTRIM requires MINID or MAXLEN")),
            }
            Ok(CmdOutput::Nil)
        }
        "XDEL" => {
            let key = arg(&args, 0)?;
            for id in &args[1..] {
                tx.execute(
                    "DELETE FROM kv_stream WHERE key = ?1 AND id = ?2",
                    params![key, id],
                )?;
            }
            Ok(CmdOutput::Nil)
        }
        "EVAL" => eval_command_tx(tx, &args),
        _ => Err(storage_error(format!(
            "unsupported Redis-compatible command {}",
            command.name
        ))),
    }
}
