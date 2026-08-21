use super::*;

pub(super) struct AuthMobilitySessionSnapshot {
    pub(super) whitelist_ids: BTreeSet<String>,
    pub(super) owned_binding_keys: BTreeSet<String>,
    pub(super) owner_record_keys: BTreeSet<String>,
}

pub(super) fn collect_auth_mobility_document_references(
    whitelist_ids: &mut BTreeSet<String>,
    owner_record_keys: &mut BTreeSet<String>,
    value: &serde_json::Value,
    collect_owner_record_key: bool,
) {
    if let Some(id) = value
        .get("whitelistRecordId")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        whitelist_ids.insert(id.to_string());
    }
    if collect_owner_record_key
        && let Some(key) = value
            .get("autoWhitelistOwnerRecordKey")
            .and_then(serde_json::Value::as_str)
            .filter(|value| !value.is_empty())
    {
        owner_record_keys.insert(key.to_string());
    }
}

pub(super) fn auth_mobility_session_snapshot_tx(
    tx: &rusqlite::Transaction<'_>,
    session_id: &str,
    session_index_key: &str,
    active_details_key: &str,
    proxy_binding_key: &str,
    pending_key: &str,
) -> RedisResult<AuthMobilitySessionSnapshot> {
    for key in [
        session_index_key,
        active_details_key,
        proxy_binding_key,
        pending_key,
    ] {
        purge_expired_tx(tx, key)?;
    }

    let mut binding_keys = {
        let mut statement = tx.prepare("SELECT member FROM kv_set WHERE key = ?1")?;
        let rows = statement.query_map([session_index_key], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<BTreeSet<_>, _>>()?
    };
    binding_keys.insert(proxy_binding_key.to_string());

    let mut whitelist_ids = BTreeSet::new();
    let mut owned_binding_keys = BTreeSet::new();
    let mut owner_record_keys = BTreeSet::new();
    for binding_key in binding_keys {
        let Some(raw) = string_get_tx(tx, &binding_key)? else {
            continue;
        };
        let parsed = serde_json::from_str::<serde_json::Value>(&raw).ok();
        let owner_matches = binding_key == proxy_binding_key
            || parsed
                .as_ref()
                .and_then(|value| value.get("ownerSessionId"))
                .and_then(serde_json::Value::as_str)
                == Some(session_id);
        if !owner_matches {
            continue;
        }
        owned_binding_keys.insert(binding_key);
        if let Some(value) = parsed.as_ref() {
            collect_auth_mobility_document_references(
                &mut whitelist_ids,
                &mut owner_record_keys,
                value,
                false,
            );
        }
    }

    let active_values = {
        let mut statement = tx.prepare("SELECT value FROM kv_hash WHERE key = ?1")?;
        let rows = statement.query_map([active_details_key], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for raw in active_values {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
            collect_auth_mobility_document_references(
                &mut whitelist_ids,
                &mut owner_record_keys,
                &value,
                true,
            );
        }
    }

    let pending = {
        let mut statement = tx.prepare("SELECT field, value FROM kv_hash WHERE key = ?1")?;
        let rows = statement.query_map([pending_key], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (record_id, owner_record_key) in pending {
        if !record_id.is_empty() {
            whitelist_ids.insert(record_id);
        }
        if !owner_record_key.is_empty() {
            owner_record_keys.insert(owner_record_key);
        }
    }

    Ok(AuthMobilitySessionSnapshot {
        whitelist_ids,
        owned_binding_keys,
        owner_record_keys,
    })
}
