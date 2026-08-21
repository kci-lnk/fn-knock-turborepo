use super::*;

pub(super) fn set_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?.to_string();
    let value = arg(args, 1)?.to_string();
    let mut ttl_ms = None;
    let mut nx = false;
    let mut xx = false;
    let mut index = 2;
    while index < args.len() {
        match args[index].to_ascii_uppercase().as_str() {
            "EX" => {
                ttl_ms = args
                    .get(index + 1)
                    .map(|value| parse_i64(value))
                    .transpose()?;
                ttl_ms = ttl_ms.map(|value| value.max(1) * 1000);
                index += 2;
            }
            "PX" => {
                ttl_ms = args
                    .get(index + 1)
                    .map(|value| parse_i64(value))
                    .transpose()?;
                ttl_ms = ttl_ms.map(|value| value.max(1));
                index += 2;
            }
            "NX" => {
                nx = true;
                index += 1;
            }
            "XX" => {
                xx = true;
                index += 1;
            }
            _ => index += 1,
        }
    }
    purge_expired_tx(tx, &key)?;
    if nx && xx {
        return Err(storage_error("SET cannot combine NX and XX"));
    }
    if nx && key_kind_tx(tx, &key)?.is_some() {
        return Ok(CmdOutput::OptionalString(None));
    }
    if xx && key_kind_tx(tx, &key)?.is_none() {
        return Ok(CmdOutput::OptionalString(None));
    }
    set_string_tx(tx, &key, &value, ttl_ms.map(|ttl| now_ms() + ttl))?;
    Ok(if nx || xx {
        CmdOutput::OptionalString(Some("OK".to_string()))
    } else {
        CmdOutput::String("OK".to_string())
    })
}

pub(super) fn zrange_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
    reverse: bool,
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?;
    let start = parse_i64(arg(args, 1)?)? as isize;
    let end = parse_i64(arg(args, 2)?)? as isize;
    let with_scores = args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("WITHSCORES"));
    purge_expired_tx(tx, key)?;
    let len = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_zset WHERE key = ?1", &[&key])?;
    let Some((offset, limit)) = normalize_range(len, start, end) else {
        return Ok(if with_scores {
            CmdOutput::ZPairs(Vec::new())
        } else {
            CmdOutput::Strings(Vec::new())
        });
    };
    let order = if reverse {
        "ORDER BY score DESC, member DESC"
    } else {
        "ORDER BY score ASC, member ASC"
    };
    let sql =
        format!("SELECT member, score FROM kv_zset WHERE key = ?1 {order} LIMIT ?2 OFFSET ?3");
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![key, limit, offset], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
    })?;
    let pairs = rows.collect::<Result<Vec<_>, _>>()?;
    if with_scores {
        Ok(CmdOutput::ZPairs(pairs))
    } else {
        Ok(CmdOutput::Strings(
            pairs.into_iter().map(|(member, _)| member).collect(),
        ))
    }
}

pub(super) fn zrangebyscore_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
    reverse: bool,
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?;
    let (min, max) = if reverse {
        (
            parse_score_bound(arg(args, 2)?)?,
            parse_score_bound(arg(args, 1)?)?,
        )
    } else {
        (
            parse_score_bound(arg(args, 1)?)?,
            parse_score_bound(arg(args, 2)?)?,
        )
    };
    let with_scores = args
        .iter()
        .any(|arg| arg.eq_ignore_ascii_case("WITHSCORES"));
    let limit = args
        .windows(3)
        .find(|window| window[0].eq_ignore_ascii_case("LIMIT"))
        .and_then(|window| parse_i64(&window[2]).ok())
        .map(|value| value.max(0) as usize);
    let members = zrangebyscore_tx(tx, key, min, max, limit, reverse)?;
    if with_scores {
        let mut pairs = Vec::with_capacity(members.len() * 2);
        for member in members {
            let score = tx
                .query_row(
                    "SELECT score FROM kv_zset WHERE key = ?1 AND member = ?2",
                    params![key, member],
                    |row| row.get::<_, f64>(0),
                )
                .optional()?
                .unwrap_or(0.0);
            pairs.push(member);
            pairs.push(score.to_string());
        }
        Ok(CmdOutput::StringPairs(pairs))
    } else {
        Ok(CmdOutput::Strings(members))
    }
}

pub(super) fn xadd_command_tx(
    tx: &rusqlite::Transaction<'_>,
    args: &[String],
) -> RedisResult<CmdOutput> {
    let key = arg(args, 0)?.to_string();
    let raw_id = arg(args, 1)?.to_string();
    if args.len() <= 2 || !args[2..].len().is_multiple_of(2) {
        return Err(storage_error("XADD requires field/value pairs"));
    }
    ensure_key_tx(tx, &key, "stream", None)?;
    let last_id = stream_last_generated_id_tx(tx, &key)?;
    let next_id = if raw_id == "*" {
        let now = now_ms().max(0) as u128;
        if now > last_id.milliseconds {
            ParsedStreamId {
                milliseconds: now,
                sequence: 0,
            }
        } else {
            ParsedStreamId {
                milliseconds: last_id.milliseconds,
                sequence: last_id
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| storage_error("stream ID sequence overflow"))?,
            }
        }
    } else {
        let explicit = parse_stream_id(&raw_id)
            .ok_or_else(|| storage_error(format!("invalid XADD stream ID: {raw_id}")))?;
        if explicit == ParsedStreamId::default() || explicit <= last_id {
            return Err(storage_error(
                "XADD stream ID must be greater than the last generated ID",
            ));
        }
        explicit
    };
    let id = format!("{}-{}", next_id.milliseconds, next_id.sequence);
    let mut fields = Vec::with_capacity(args[2..].len());
    for value in &args[2..] {
        fields.push(value.clone());
    }
    tx.execute(
        "INSERT INTO kv_stream(key, id, id_ms, id_sequence, fields_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            key,
            id,
            i64::try_from(next_id.milliseconds)
                .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?,
            i64::try_from(next_id.sequence)
                .map_err(|_| storage_error("stream ID sequence exceed SQLite range"))?,
            serde_json::to_string(&fields)?
        ],
    )?;
    set_stream_last_generated_id_tx(tx, &key, next_id)?;
    Ok(CmdOutput::String(id))
}

pub(crate) fn string_get_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> RedisResult<Option<String>> {
    purge_expired_tx(tx, key)?;
    if key_kind_tx(tx, key)? != Some("string".to_string()) {
        return Ok(None);
    }
    tx.query_row(
        "SELECT value FROM kv_strings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn set_string_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    value: &str,
    expires_at_ms: Option<i64>,
) -> RedisResult<()> {
    ensure_key_with_ttl_policy_tx(tx, key, "string", expires_at_ms, false)?;
    tx.execute(
        "INSERT OR REPLACE INTO kv_strings(key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub(super) fn set_string_preserve_ttl_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    value: &str,
) -> RedisResult<()> {
    ensure_key_tx(tx, key, "string", None)?;
    tx.execute(
        "INSERT OR REPLACE INTO kv_strings(key, value) VALUES (?1, ?2)",
        params![key, value],
    )?;
    Ok(())
}

pub(super) fn zrangebyscore_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
    limit: Option<usize>,
    reverse: bool,
) -> RedisResult<Vec<String>> {
    purge_expired_tx(tx, key)?;
    let order = if reverse {
        "ORDER BY score DESC, member DESC"
    } else {
        "ORDER BY score ASC, member ASC"
    };
    let limit_sql = limit
        .map(|limit| format!(" LIMIT {}", limit))
        .unwrap_or_default();
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql = format!(
        "SELECT member FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3 {order}{limit_sql}"
    );
    let mut stmt = tx.prepare(&sql)?;
    let rows = stmt.query_map(params![key, min.value, max.value], |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

pub(super) fn delete_zset_score_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
) -> RedisResult<usize> {
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql =
        format!("DELETE FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3");
    let deleted = tx.execute(&sql, params![key, min.value, max.value])?;
    delete_collection_key_if_empty_tx(tx, key, "zset")?;
    Ok(deleted)
}

pub(super) fn count_zset_score_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    min: ScoreBound,
    max: ScoreBound,
) -> RedisResult<i64> {
    let min_op = if min.exclusive { ">" } else { ">=" };
    let max_op = if max.exclusive { "<" } else { "<=" };
    let sql = format!(
        "SELECT COUNT(*) FROM kv_zset WHERE key = ?1 AND score {min_op} ?2 AND score {max_op} ?3"
    );
    tx.query_row(&sql, params![key, min.value, max.value], |row| row.get(0))
        .map_err(Into::into)
}

pub(super) fn list_range_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    start: isize,
    end: isize,
) -> RedisResult<Vec<String>> {
    let len = count_rows_tx(tx, "SELECT COUNT(*) FROM kv_list WHERE key = ?1", &[&key])?;
    let Some((offset, limit)) = normalize_range(len, start, end) else {
        return Ok(Vec::new());
    };
    let mut stmt =
        tx.prepare("SELECT value FROM kv_list WHERE key = ?1 ORDER BY idx ASC LIMIT ?2 OFFSET ?3")?;
    let rows = stmt.query_map(params![key, limit, offset], |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

pub(super) fn scan_keys_tx(
    tx: &rusqlite::Transaction<'_>,
    prefix: &str,
) -> RedisResult<Vec<String>> {
    purge_expired_all_tx(tx)?;
    let pattern = format!("{}%", escape_like_pattern(prefix));
    let mut stmt =
        tx.prepare("SELECT key FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key ASC")?;
    let rows = stmt.query_map(params![pattern], |row| row.get::<_, String>(0))?;
    let mut keys = rows.collect::<Result<Vec<_>, _>>()?;
    keys.sort_by(|left, right| node_locale_compare_ordering(left, right));
    Ok(keys)
}

pub(super) fn keys_matching_pattern_tx(
    tx: &rusqlite::Transaction<'_>,
    pattern: &str,
) -> RedisResult<Vec<String>> {
    let mut statement =
        tx.prepare("SELECT key FROM kv_keys WHERE key LIKE ?1 ESCAPE '\\' ORDER BY key ASC")?;
    statement
        .query_map([pattern], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

pub(super) fn purge_expired(conn: &mut rusqlite::Connection, key: &str) -> RedisResult<()> {
    let tx = immediate_transaction(conn)?;
    let deleted = tx.execute(
        "DELETE FROM kv_keys WHERE key = ?1 AND expires_at_ms IS NOT NULL AND expires_at_ms <= ?2",
        params![key, now_ms()],
    )?;
    if deleted > 0 {
        sync_typed_mobility_tx(&tx, TypedMobilitySyncScope::from_key(key))?;
    }
    tx.commit()?;
    Ok(())
}

pub(super) fn purge_expired_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<()> {
    let deleted = tx.execute(
        "DELETE FROM kv_keys WHERE key = ?1 AND expires_at_ms IS NOT NULL AND expires_at_ms <= ?2",
        params![key, now_ms()],
    )?;
    if deleted > 0 {
        sync_typed_mobility_tx(tx, TypedMobilitySyncScope::from_key(key))?;
    }
    Ok(())
}

pub(super) fn purge_expired_all_tx(tx: &rusqlite::Transaction<'_>) -> RedisResult<()> {
    let cutoff = now_ms();
    let expired_typed_shadow_keys = {
        let mut statement = tx.prepare(
            "SELECT key FROM kv_keys
             WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
        )?;
        statement
            .query_map(params![cutoff], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };
    tx.execute(
        "DELETE FROM kv_keys WHERE expires_at_ms IS NOT NULL AND expires_at_ms <= ?1",
        params![cutoff],
    )?;
    sync_typed_mobility_tx(
        tx,
        TypedMobilitySyncScope::from_keys(expired_typed_shadow_keys),
    )?;
    Ok(())
}

pub(super) fn key_kind(conn: &rusqlite::Connection, key: &str) -> RedisResult<Option<String>> {
    conn.query_row(
        "SELECT kind FROM kv_keys WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn key_kind_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> RedisResult<Option<String>> {
    tx.query_row(
        "SELECT kind FROM kv_keys WHERE key = ?1",
        params![key],
        |row| row.get(0),
    )
    .optional()
    .map_err(Into::into)
}

pub(super) fn ensure_key_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
    expires_at_ms: Option<i64>,
) -> RedisResult<()> {
    ensure_key_with_ttl_policy_tx(tx, key, kind, expires_at_ms, true)
}

pub(super) fn ensure_key_with_ttl_policy_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
    expires_at_ms: Option<i64>,
    preserve_existing_ttl_when_none: bool,
) -> RedisResult<()> {
    purge_expired_tx(tx, key)?;
    if let Some(existing) = key_kind_tx(tx, key)?
        && existing != kind
    {
        delete_key_tx(tx, key)?;
    }
    if preserve_existing_ttl_when_none {
        tx.execute(
            "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET kind = excluded.kind,
               expires_at_ms = COALESCE(excluded.expires_at_ms, kv_keys.expires_at_ms)",
            params![key, kind, expires_at_ms],
        )?;
    } else {
        tx.execute(
            "INSERT INTO kv_keys(key, kind, expires_at_ms) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET kind = excluded.kind,
               expires_at_ms = excluded.expires_at_ms",
            params![key, kind, expires_at_ms],
        )?;
    }
    Ok(())
}

pub(super) fn delete_key_tx(tx: &rusqlite::Transaction<'_>, key: &str) -> RedisResult<usize> {
    Ok(tx.execute("DELETE FROM kv_keys WHERE key = ?1", params![key])?)
}

pub(super) fn delete_collection_key_if_empty_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    kind: &str,
) -> RedisResult<()> {
    let table = match kind {
        "hash" => "kv_hash",
        "list" => "kv_list",
        "set" => "kv_set",
        "zset" => "kv_zset",
        _ => {
            return Err(storage_error(format!(
                "unsupported collection kind: {kind}"
            )));
        }
    };
    if key_kind_tx(tx, key)?.as_deref() != Some(kind) {
        return Ok(());
    }
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE key = ?1)");
    let has_members = tx.query_row(&sql, params![key], |row| row.get::<_, bool>(0))?;
    if !has_members {
        delete_key_tx(tx, key)?;
    }
    Ok(())
}

pub(super) fn immediate_transaction(
    conn: &mut rusqlite::Connection,
) -> rusqlite::Result<rusqlite::Transaction<'_>> {
    conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
}
