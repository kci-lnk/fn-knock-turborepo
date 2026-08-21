use super::*;

pub(crate) struct LdapBindingClaim<'a> {
    pub invite_key: &'a str,
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub provider_id: &'a str,
    pub totp_id: &'a str,
    pub score: i64,
}

pub(crate) struct OwnedBindingUpdate<'a> {
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub score: i64,
}

pub(crate) struct OwnedBindingDelete<'a> {
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
}

pub(crate) struct OidcBindingClaim<'a> {
    pub invite_key: &'a str,
    pub subject_key: &'a str,
    pub binding_key: &'a str,
    pub bindings_index_key: &'a str,
    pub binding_id: &'a str,
    pub binding: &'a Value,
    pub provider_id: &'a str,
    pub totp_id: &'a str,
    pub score: i64,
}

pub(super) struct ConfigFenceSnapshot {
    pub(super) config_raw: Option<String>,
    pub(super) generation_raw: Option<String>,
    pub(super) config: Value,
    pub(super) generation: u64,
}

pub(super) struct ConfigGenerationMarker {
    pub(super) generation: u64,
    pub(super) host_fingerprint: String,
}

pub(super) async fn load_config_fence_snapshot(
    conn: &mut ConnectionManager,
) -> crate::storage::StorageResult<ConfigFenceSnapshot> {
    let values: Vec<Option<String>> = redis::cmd("MGET")
        .arg(vec![
            CONFIG_KEY.to_string(),
            HOST_MAPPINGS_GENERATION_KEY.to_string(),
        ])
        .query_async(conn)
        .await?;
    config_fence_snapshot_from_raw(
        values.first().cloned().flatten(),
        values.get(1).cloned().flatten(),
    )
}

pub(super) fn config_fence_snapshot_from_raw(
    config_raw: Option<String>,
    generation_raw: Option<String>,
) -> crate::storage::StorageResult<ConfigFenceSnapshot> {
    let config = match config_raw.as_deref() {
        Some(raw) => serde_json::from_str(raw)?,
        None => default_config(),
    };
    let generation = generation_raw
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .map_err(|_| crate::storage::storage_error("host mappings generation is invalid"))?;
    Ok(ConfigFenceSnapshot {
        config_raw,
        generation_raw,
        config,
        generation,
    })
}

pub(super) async fn compare_and_set_config_fence_snapshot(
    conn: &mut ConnectionManager,
    snapshot: &ConfigFenceSnapshot,
    replacement_raw: &str,
    replacement_generation: u64,
) -> crate::storage::StorageResult<Option<u64>> {
    let applied: i64 = redis::cmd("EVAL")
        .arg(
            r#"
-- fn-knock:eval:cas-config-host-generation-raw:v3
local current_config = redis.call("GET", KEYS[1])
local current_generation = redis.call("GET", KEYS[2])
local function raw_matches(current, expected_exists, expected)
  if expected_exists == "0" then
    return not current
  end
  return current and current == expected
end
if not raw_matches(current_config, ARGV[1], ARGV[2])
    or not raw_matches(current_generation, ARGV[3], ARGV[4]) then
  return 0
end
redis.call("SET", KEYS[1], ARGV[5])
redis.call("SET", KEYS[2], ARGV[6])
return 1
"#,
        )
        .arg(2)
        .arg(CONFIG_KEY)
        .arg(HOST_MAPPINGS_GENERATION_KEY)
        .arg(if snapshot.config_raw.is_some() {
            "1"
        } else {
            "0"
        })
        .arg(snapshot.config_raw.as_deref().unwrap_or(""))
        .arg(if snapshot.generation_raw.is_some() {
            "1"
        } else {
            "0"
        })
        .arg(snapshot.generation_raw.as_deref().unwrap_or(""))
        .arg(replacement_raw)
        .arg(replacement_generation.to_string())
        .query_async(conn)
        .await?;
    if applied == 0 {
        return Ok(None);
    }
    let revision = u64::try_from(applied)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| crate::storage::storage_error("typed config revision is invalid"))?;
    Ok(Some(revision))
}

pub(super) fn config_host_mappings(config: &Value) -> Value {
    config
        .get("host_mappings")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

pub(super) fn config_host_mappings_fingerprint(
    config: &Value,
) -> crate::storage::StorageResult<String> {
    Ok(crate::crypto_utils::sha256_hex_bytes(serde_json::to_vec(
        &config_host_mappings(config),
    )?))
}

pub(super) fn replace_visibility_policies_for_host_mappings(
    config: &mut Value,
    replacement_mappings: &[Value],
    supplied_policies: &Map<String, Value>,
) -> crate::storage::StorageResult<()> {
    let existing_policies = config
        .get("visibility_policies")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut referenced = referenced_host_ipset_policy_ids(replacement_mappings);
    if let Some(id) = config
        .pointer("/gateway_visibility/policy_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        referenced.insert(id.to_string());
    }
    let mut next = Map::new();
    for id in referenced {
        let policy = supplied_policies
            .get(&id)
            .or_else(|| existing_policies.get(&id))
            .cloned()
            .ok_or_else(|| {
                crate::storage::storage_error(format!(
                    "visibility policy {id} is missing from the host mapping transaction"
                ))
            })?;
        next.insert(id, policy);
    }
    let object = config
        .as_object_mut()
        .ok_or_else(|| crate::storage::storage_error("stored config must be a JSON object"))?;
    object.insert("visibility_policies".to_string(), Value::Object(next));
    Ok(())
}

pub(super) fn take_config_generation_marker(
    config: &mut Value,
) -> crate::storage::StorageResult<Option<ConfigGenerationMarker>> {
    let Some(object) = config.as_object_mut() else {
        return Ok(None);
    };
    let Some(marker) = object.remove(CONFIG_GENERATION_MARKER) else {
        return Ok(None);
    };
    let generation = marker
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            crate::storage::storage_error("host mappings generation marker is invalid")
        })?;
    let host_fingerprint = marker
        .get("host_fingerprint")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            crate::storage::storage_error("host mappings generation fingerprint is invalid")
        })?
        .to_string();
    Ok(Some(ConfigGenerationMarker {
        generation,
        host_fingerprint,
    }))
}

pub(super) fn inject_config_generation_marker(
    config: &mut Value,
    generation: u64,
) -> crate::storage::StorageResult<()> {
    let host_fingerprint = config_host_mappings_fingerprint(config)?;
    if let Some(object) = config.as_object_mut() {
        object.insert(
            CONFIG_GENERATION_MARKER.to_string(),
            json!({
                "generation": generation,
                "host_fingerprint": host_fingerprint,
            }),
        );
    }
    Ok(())
}
