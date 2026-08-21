use super::*;

pub(super) fn escape_like_pattern(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' | '%' | '_' => {
                escaped.push('\\');
                escaped.push(character);
            }
            _ => escaped.push(character),
        }
    }
    escaped
}

pub(super) fn count_rows(
    conn: &rusqlite::Connection,
    sql: &str,
    args: &[&dyn ToSql],
) -> RedisResult<i64> {
    conn.query_row(sql, params_from_iter(args.iter().copied()), |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

pub(super) fn count_rows_tx(
    tx: &rusqlite::Transaction<'_>,
    sql: &str,
    args: &[&dyn ToSql],
) -> RedisResult<i64> {
    tx.query_row(sql, params_from_iter(args.iter().copied()), |row| {
        row.get(0)
    })
    .map_err(Into::into)
}

pub(super) fn query_strings(
    conn: &rusqlite::Connection,
    sql: &str,
    args: &[&dyn ToSql],
) -> RedisResult<Vec<String>> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map(params_from_iter(args.iter().copied()), |row| row.get(0))?;
    rows.collect::<Result<Vec<String>, _>>().map_err(Into::into)
}

pub(super) fn stream_id_from_row(
    id: String,
    fields_json: String,
) -> rusqlite::Result<streams::StreamId> {
    let fields = stream_fields_vec(&fields_json)?;
    let mut object = HashMap::new();
    for pair in fields.chunks(2) {
        if let [field, value] = pair {
            object.insert(field.clone(), value.clone());
        }
    }
    Ok(streams::StreamId::new(id, object))
}

pub(super) fn stream_fields_vec(fields_json: &str) -> rusqlite::Result<Vec<String>> {
    let value = serde_json::from_str::<serde_json::Value>(fields_json)
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
    match value {
        serde_json::Value::Array(items) => {
            let mut fields = Vec::with_capacity(items.len());
            for item in items {
                let Some(text) = item.as_str() else {
                    return Err(invalid_stream_fields_json());
                };
                fields.push(text.to_string());
            }
            if fields.is_empty() || fields.len() % 2 != 0 {
                return Err(invalid_stream_fields_json());
            }
            Ok(fields)
        }
        serde_json::Value::Object(object) => {
            let ordered = object.into_iter().collect::<BTreeMap<_, _>>();
            let mut fields = Vec::with_capacity(ordered.len() * 2);
            for (key, value) in ordered {
                let Some(text) = value.as_str() else {
                    return Err(invalid_stream_fields_json());
                };
                fields.push(key);
                fields.push(text.to_string());
            }
            Ok(fields)
        }
        _ => Err(invalid_stream_fields_json()),
    }
}

pub(super) fn invalid_stream_fields_json() -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid stream fields JSON",
    )))
}

pub(super) fn normalize_range(len: i64, start: isize, end: isize) -> Option<(i64, i64)> {
    if len <= 0 {
        return None;
    }
    let len = len as isize;
    let start = if start < 0 {
        (len + start).max(0)
    } else {
        start
    };
    let end = if end < 0 { len + end } else { end.min(len - 1) };
    if start >= len || end < 0 {
        return None;
    }
    if start > end {
        return None;
    }
    Some((start as i64, (end - start + 1) as i64))
}

pub(super) fn arg(args: &[String], index: usize) -> RedisResult<&str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| storage_error(format!("missing command argument {index}")))
}

pub(super) fn parse_i64(value: &str) -> RedisResult<i64> {
    value
        .parse::<i64>()
        .map_err(|_| storage_error(format!("invalid integer argument: {value}")))
}

pub(super) fn parse_f64(value: &str) -> RedisResult<f64> {
    value
        .parse::<f64>()
        .map_err(|_| storage_error(format!("invalid float argument: {value}")))
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ScoreBound {
    pub(super) value: f64,
    pub(super) exclusive: bool,
}

impl ScoreBound {
    pub(super) fn inclusive(value: f64) -> Self {
        Self {
            value,
            exclusive: false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct ParsedStreamId {
    pub(super) milliseconds: u128,
    pub(super) sequence: u128,
}

pub(super) fn parse_score_bound(value: &str) -> RedisResult<ScoreBound> {
    let (exclusive, raw_value) = value
        .strip_prefix('(')
        .map(|value| (true, value))
        .unwrap_or((false, value));
    if raw_value.is_empty() {
        return Err(storage_error("invalid score bound: empty exclusive bound"));
    }
    Ok(ScoreBound {
        value: parse_score_value(raw_value)?,
        exclusive,
    })
}

pub(super) fn parse_score_value(value: &str) -> RedisResult<f64> {
    match value {
        "-inf" => Ok(f64::NEG_INFINITY),
        "+inf" | "inf" => Ok(f64::INFINITY),
        _ => parse_f64(value),
    }
}

pub(super) fn parse_stream_id(value: &str) -> Option<ParsedStreamId> {
    if value == "0" {
        return Some(ParsedStreamId::default());
    }
    let (milliseconds, sequence) = value.split_once('-')?;
    Some(ParsedStreamId {
        milliseconds: milliseconds.parse().ok()?,
        sequence: sequence.parse().ok()?,
    })
}

pub(super) fn parse_stream_bound(value: &str, is_min: bool) -> RedisResult<Option<ParsedStreamId>> {
    if (is_min && value == "-") || (!is_min && value == "+") {
        return Ok(None);
    }
    parse_stream_id(value)
        .map(Some)
        .ok_or_else(|| storage_error(format!("invalid stream ID bound: {value}")))
}

pub(super) fn stream_id_sql_tuple(id: ParsedStreamId) -> RedisResult<(i64, i64)> {
    Ok((
        i64::try_from(id.milliseconds)
            .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?,
        i64::try_from(id.sequence)
            .map_err(|_| storage_error("stream ID sequence exceeds SQLite range"))?,
    ))
}

pub(super) fn query_stream_rows(
    conn: &rusqlite::Connection,
    key: &str,
    min: Option<ParsedStreamId>,
    min_exclusive: bool,
    max: Option<ParsedStreamId>,
    reverse: bool,
    count: usize,
) -> RedisResult<Vec<(String, String)>> {
    let (min_ms, min_sequence) = stream_id_sql_tuple(min.unwrap_or_default())?;
    let (max_ms, max_sequence) = stream_id_sql_tuple(max.unwrap_or(ParsedStreamId {
        milliseconds: i64::MAX as u128,
        sequence: i64::MAX as u128,
    }))?;
    let min_operator = if min_exclusive { ">" } else { ">=" };
    let order = if reverse { "DESC" } else { "ASC" };
    let sql = format!(
        "SELECT id, fields_json
         FROM kv_stream
         WHERE key = ?1
           AND (id_ms, id_sequence) {min_operator} (?2, ?3)
           AND (id_ms, id_sequence) <= (?4, ?5)
         ORDER BY id_ms {order}, id_sequence {order}
         LIMIT ?6"
    );
    let limit = i64::try_from(count).unwrap_or(i64::MAX);
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![key, min_ms, min_sequence, max_ms, max_sequence, limit],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub(super) fn stream_last_generated_id_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
) -> RedisResult<ParsedStreamId> {
    if let Some((milliseconds, sequence)) = tx
        .query_row(
            "SELECT last_generated_ms, last_generated_seq FROM kv_stream_meta WHERE key = ?1",
            params![key],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .optional()?
    {
        return Ok(ParsedStreamId {
            milliseconds: milliseconds.max(0) as u128,
            sequence: sequence.max(0) as u128,
        });
    }

    tx.query_row(
        "SELECT id_ms, id_sequence
         FROM kv_stream
         WHERE key = ?1
         ORDER BY id_ms DESC, id_sequence DESC
         LIMIT 1",
        params![key],
        |row| {
            Ok(ParsedStreamId {
                milliseconds: row.get::<_, i64>(0)?.max(0) as u128,
                sequence: row.get::<_, i64>(1)?.max(0) as u128,
            })
        },
    )
    .optional()
    .map(Option::unwrap_or_default)
    .map_err(Into::into)
}

pub(super) fn set_stream_last_generated_id_tx(
    tx: &rusqlite::Transaction<'_>,
    key: &str,
    id: ParsedStreamId,
) -> RedisResult<()> {
    let milliseconds = i64::try_from(id.milliseconds)
        .map_err(|_| storage_error("stream ID milliseconds exceed SQLite range"))?;
    let sequence = i64::try_from(id.sequence)
        .map_err(|_| storage_error("stream ID sequence exceeds SQLite range"))?;
    tx.execute(
        "INSERT INTO kv_stream_meta(key, last_generated_ms, last_generated_seq)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET
           last_generated_ms = excluded.last_generated_ms,
           last_generated_seq = excluded.last_generated_seq",
        params![key, milliseconds, sequence],
    )?;
    Ok(())
}

#[cfg(unix)]
pub(super) async fn secure_directory_permissions(path: &Path) -> RedisResult<()> {
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) async fn secure_directory_permissions(_path: &Path) -> RedisResult<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) async fn secure_sqlite_file_permissions(path: &Path) -> RedisResult<()> {
    set_file_mode_if_exists(path, 0o600).await?;
    set_file_mode_if_exists(&sqlite_companion_path(path, "-wal"), 0o600).await?;
    set_file_mode_if_exists(&sqlite_companion_path(path, "-shm"), 0o600).await?;
    Ok(())
}

#[cfg(not(unix))]
pub(super) async fn secure_sqlite_file_permissions(_path: &Path) -> RedisResult<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) async fn set_file_mode_if_exists(path: &Path, mode: u32) -> RedisResult<()> {
    match tokio::fs::metadata(path).await {
        Ok(_) => {
            tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).await?;
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(unix)]
pub(super) fn sqlite_companion_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

pub(super) fn now_ms() -> i64 {
    crate::time_utils::now_ms()
}

pub(super) fn node_locale_compare_ordering(left: &str, right: &str) -> Ordering {
    crate::store::node_locale_compare_ordering(left, right)
}
