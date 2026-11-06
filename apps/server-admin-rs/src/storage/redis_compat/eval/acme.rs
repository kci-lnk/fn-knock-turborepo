use super::*;

pub(super) fn journal_rates(
    tx: &rusqlite::Transaction<'_>,
    keys: &[String],
    argv: &[String],
) -> RedisResult<CmdOutput> {
    let [lease, evidence] = keys else {
        return Err(storage_error("invalid ACME rate journal keys"));
    };
    let [job_id, value, ttl] = argv else {
        return Err(storage_error("invalid ACME rate journal args"));
    };
    let lock = string_get_tx(tx, lease)?
        .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
    if lock.is_none_or(|lock| lock["jobId"].as_str() != Some(job_id)) {
        return Ok(CmdOutput::Int(0));
    }
    set_string_tx(
        tx,
        evidence,
        value,
        Some(now_ms().saturating_add(parse_i64(ttl)?.saturating_mul(1000))),
    )?;
    Ok(CmdOutput::Int(1))
}

pub(super) fn extend_cooldowns(
    tx: &rusqlite::Transaction<'_>,
    keys: &[String],
    argv: &[String],
) -> RedisResult<CmdOutput> {
    let [value, until, ttl] = argv else {
        return Err(storage_error("invalid cooldown arguments"));
    };
    let until = parse_i64(until)?;
    let expiry = now_ms().saturating_add(parse_i64(ttl)?.saturating_mul(1000));
    for key in keys {
        let old = string_get_tx(tx, key)?
            .map(|raw| {
                serde_json::from_str::<serde_json::Value>(&raw)
                    .ok()
                    .and_then(|v| v["retryAt"].as_i64())
                    .ok_or_else(|| storage_error("invalid stored cooldown"))
            })
            .transpose()?;
        if old.is_none_or(|old| old < until) {
            set_string_tx(tx, key, value, Some(expiry))?;
        }
    }
    Ok(CmdOutput::Int(1))
}
