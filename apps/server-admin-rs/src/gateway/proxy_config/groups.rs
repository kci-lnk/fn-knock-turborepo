use super::*;

pub(super) const MAX_HOST_MAPPING_GROUPS: usize = 32;
pub(super) const MAX_HOST_MAPPING_GROUP_NAME_CHARS: usize = 40;

pub(super) fn host_mapping_groups_from_config(config: &Value) -> Vec<Value> {
    config
        .get("host_mapping_groups")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

pub(super) fn host_mapping_grouped_view_from_config(config: &Value) -> bool {
    config
        .get("host_mapping_grouped_view")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) fn normalize_host_mapping_groups(groups: Vec<Value>) -> Result<Vec<Value>, String> {
    if groups.len() > MAX_HOST_MAPPING_GROUPS {
        return Err(format!(
            "At most {MAX_HOST_MAPPING_GROUPS} host mapping groups are allowed"
        ));
    }

    let mut normalized = Vec::with_capacity(groups.len());
    let mut seen_ids = HashSet::with_capacity(groups.len());
    let mut seen_names = HashSet::with_capacity(groups.len());

    for group in groups {
        let Some(object) = group.as_object() else {
            return Err("Host mapping group must be an object".to_string());
        };
        let raw_id = object.get("id").and_then(Value::as_str).unwrap_or("");
        let parsed_id = uuid::Uuid::parse_str(raw_id.trim())
            .map_err(|_| "Host mapping group id must be a UUID".to_string())?;
        let id = parsed_id.hyphenated().to_string();
        if !seen_ids.insert(id.clone()) {
            return Err(format!("Duplicate host mapping group id {id}"));
        }

        let name = object
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let name_chars = name.chars().count();
        if name_chars == 0 || name_chars > MAX_HOST_MAPPING_GROUP_NAME_CHARS {
            return Err(format!(
                "Host mapping group name must contain 1 to {MAX_HOST_MAPPING_GROUP_NAME_CHARS} characters"
            ));
        }
        let normalized_name = name.to_lowercase();
        if !seen_names.insert(normalized_name) {
            return Err(format!("Duplicate host mapping group name {name}"));
        }

        normalized.push(json!({
            "id": id,
            "name": name,
        }));
    }

    Ok(normalized)
}

pub(super) fn host_mapping_group_names(groups: &[Value]) -> HashMap<String, String> {
    groups
        .iter()
        .filter_map(|group| {
            let id = group.get("id").and_then(Value::as_str)?.trim();
            let name = group.get("name").and_then(Value::as_str)?.trim();
            (!id.is_empty() && !name.is_empty()).then(|| (id.to_string(), name.to_string()))
        })
        .collect()
}

pub(super) fn normalize_host_mapping_group_id(
    requested: Option<&Value>,
    previous: Option<&Value>,
    valid_group_ids: &HashSet<String>,
    is_auth: bool,
) -> Result<Value, String> {
    if is_auth {
        return Ok(Value::Null);
    }

    let source = requested.or(previous);
    let Some(source) = source else {
        return Ok(Value::Null);
    };
    if source.is_null() {
        return Ok(Value::Null);
    }
    let Some(group_id) = source.as_str() else {
        return Err("group id must be a UUID or null".to_string());
    };
    let group_id = group_id.trim();
    if group_id.is_empty() {
        return Ok(Value::Null);
    }
    let parsed = uuid::Uuid::parse_str(group_id)
        .map_err(|_| "group id must be a UUID or null".to_string())?;
    let canonical = parsed.hyphenated().to_string();
    if !valid_group_ids.contains(&canonical) {
        return Err(format!("references unknown group {canonical}"));
    }
    Ok(Value::String(canonical))
}

pub(super) fn ordered_host_mappings_for_groups(mappings: &[Value], groups: &[Value]) -> Vec<Value> {
    if groups.is_empty() {
        return mappings.to_vec();
    }

    let mut grouped = Vec::with_capacity(mappings.len());
    let mut consumed_hosts = HashSet::with_capacity(mappings.len());
    for group in groups {
        let Some(group_id) = group.get("id").and_then(Value::as_str) else {
            continue;
        };
        for mapping in mappings {
            if mapping.get("group_id").and_then(Value::as_str) != Some(group_id) {
                continue;
            }
            if let Some(host) = mapping.get("host").and_then(Value::as_str) {
                consumed_hosts.insert(host.to_string());
            }
            grouped.push(mapping.clone());
        }
    }
    for mapping in mappings {
        let already_consumed = mapping
            .get("host")
            .and_then(Value::as_str)
            .is_some_and(|host| consumed_hosts.contains(host));
        if !already_consumed {
            grouped.push(mapping.clone());
        }
    }
    grouped
}
