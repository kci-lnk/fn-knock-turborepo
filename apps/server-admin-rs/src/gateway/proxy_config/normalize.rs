use super::*;

pub(super) fn ensure_go_success(value: Value) -> Result<(), String> {
    if crate::go_backend::response_success(&value) {
        return Ok(());
    }
    Err(crate::go_backend::response_message(
        &value,
        "Go backend returned an unsuccessful response",
    ))
}

pub(super) async fn rollback_proxy_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback proxy mappings config");
        return;
    }
    let previous_rules = previous_config
        .get("proxy_mappings")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    if let Err(error) = sync_go_rules(state, &previous_rules).await {
        tracing::warn!(%error, "failed to rollback proxy mappings runtime");
    }
}

pub(super) async fn rollback_host_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback host mappings config");
        return;
    }
    let previous_mappings = previous_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Err(error) = sync_host_mappings_runtime(state, previous_config, &previous_mappings).await
    {
        tracing::warn!(%error, "failed to rollback host mappings runtime");
    }
}

pub(super) async fn rollback_stream_mappings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings config");
        return;
    }
    if let Err(error) = sync_stream_mappings_runtime(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback stream mappings runtime");
    }
}

pub(super) async fn rollback_subdomain_mode(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.store.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback subdomain mode config");
        return;
    }
    if let Err(error) = sync_go_auth_config(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback subdomain mode runtime");
    }
}

pub(super) fn normalize_proxy_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, &'static str> {
    let mut normalized = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Proxy mapping must be an object");
        };
        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err("Proxy mapping target must be a supported HTTP/WebSocket URL");
        }
        object.insert("target".to_string(), Value::String(target));
        normalized.push(Value::Object(object));
    }
    Ok(normalized)
}

pub(super) fn normalize_host_mappings_for_route(
    mappings: Vec<Value>,
    previous_config: &Value,
) -> Result<Vec<Value>, String> {
    let previous_by_host = previous_host_mappings_by_host(previous_config);
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut has_default_mapping = false;
    let mut auth_mapping_count = 0;

    for mapping in mappings {
        let Some(mut object) = mapping.as_object().cloned() else {
            return Err("Host mapping must be an object".to_string());
        };
        let host = normalize_host_value(object.get("host").and_then(Value::as_str).unwrap_or(""));
        if host.is_empty() {
            return Err("Host mapping host is required".to_string());
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_supported_proxy_target_url(&target) {
            return Err(format!(
                "Host mapping {host} target must be a supported HTTP/WebSocket URL"
            ));
        }

        let service_role = if is_auth_service_target(&target) {
            "auth"
        } else {
            "app"
        };
        if service_role == "auth" {
            auth_mapping_count += 1;
            if auth_mapping_count > 1 {
                return Err("Only one auth service host mapping is allowed".to_string());
            }
            if object
                .get("use_auth")
                .and_then(Value::as_bool)
                .unwrap_or(true)
                || object
                    .get("access_mode")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == "strict_whitelist")
            {
                return Err(format!("Auth host mapping {host} must be public"));
            }
            if host_basic_auth_enabled(object.get("basic_auth")) {
                return Err(format!("Auth host mapping {host} cannot enable Basic Auth"));
            }
        } else if host_basic_auth_invalid(object.get("basic_auth")) {
            return Err(format!(
                "Host mapping {host} Basic Auth settings are invalid"
            ));
        }
        let previous = previous_by_host.get(&host);
        let disabled = service_role != "auth"
            && object
                .get("disabled")
                .or_else(|| previous.and_then(|value| value.get("disabled")))
                .and_then(Value::as_bool)
                .unwrap_or(false);
        let availability_source = if object.contains_key("availability") {
            object.get("availability")
        } else {
            previous.and_then(|value| value.get("availability"))
        };
        let availability = if service_role == "auth" {
            Value::Null
        } else {
            normalize_host_mapping_availability_for_route(&host, availability_source)?
        };

        let locations = if service_role == "auth" {
            Vec::new()
        } else {
            normalize_host_mapping_locations_for_route(&host, object.get("locations"))?
        };

        let can_reuse_previous_metadata = previous
            .and_then(|value| value.get("target"))
            .and_then(Value::as_str)
            .is_some_and(|value| value.trim() == target);
        let normalized_basic_auth = if service_role == "auth" {
            disabled_host_basic_auth()
        } else {
            normalize_host_basic_auth(
                object
                    .get("basic_auth")
                    .or_else(|| previous.and_then(|value| value.get("basic_auth"))),
            )
        };
        let is_default = service_role != "auth"
            && object
                .get("is_default")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && !has_default_mapping;
        if is_default {
            has_default_mapping = true;
        }

        object.insert("host".to_string(), Value::String(host));
        object.insert("target".to_string(), Value::String(target));
        object.insert(
            "use_auth".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("use_auth")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
            ),
        );
        object.insert(
            "access_mode".to_string(),
            Value::String(if service_role == "auth" {
                "login_first".to_string()
            } else {
                normalize_access_mode(object.get("access_mode"))
            }),
        );
        object.insert(
            "suppress_toolbar".to_string(),
            Value::Bool(
                service_role != "auth"
                    && object
                        .get("suppress_toolbar")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
            ),
        );
        object.insert(
            "preserve_host".to_string(),
            Value::Bool(
                object
                    .get("preserve_host")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            ),
        );
        object.insert("is_default".to_string(), Value::Bool(is_default));
        object.insert("disabled".to_string(), Value::Bool(disabled));
        object.insert("availability".to_string(), availability);
        object.insert("basic_auth".to_string(), normalized_basic_auth);
        object.insert("locations".to_string(), Value::Array(locations));
        object.insert(
            "service_role".to_string(),
            Value::String(service_role.to_string()),
        );
        object.insert(
            "title".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title"),
                previous,
                "title",
                can_reuse_previous_metadata,
            )),
        );
        object.insert(
            "title_override".to_string(),
            Value::String(normalize_metadata_string(
                object.get("title_override"),
                previous,
                "title_override",
                true,
            )),
        );
        object.insert(
            "favicon".to_string(),
            Value::String(normalize_metadata_string(
                object.get("favicon"),
                previous,
                "favicon",
                can_reuse_previous_metadata,
            )),
        );
        normalized.push(Value::Object(object));
    }

    Ok(normalized)
}

fn normalize_host_mapping_availability_for_route(
    host: &str,
    value: Option<&Value>,
) -> Result<Value, String> {
    let Some(value) = value else {
        return Ok(Value::Null);
    };
    if value.is_null() {
        return Ok(Value::Null);
    }
    let Some(object) = value.as_object() else {
        return Err(format!(
            "Host mapping {host} availability must be an object"
        ));
    };
    if !object
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Value::Null);
    }

    let start_time = object
        .get("start_time")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let end_time = object
        .get("end_time")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    match validate_host_availability_window(start_time, end_time) {
        Ok(()) => {}
        Err(HostAvailabilityWindowError::InvalidStartTime) => {
            return Err(format!(
                "Host mapping {host} availability start_time must use HH:mm"
            ));
        }
        Err(HostAvailabilityWindowError::InvalidEndTime) => {
            return Err(format!(
                "Host mapping {host} availability end_time must use HH:mm"
            ));
        }
        Err(HostAvailabilityWindowError::SameTime) => {
            return Err(format!(
                "Host mapping {host} availability start_time and end_time must be different"
            ));
        }
    }

    Ok(json!({
        "enabled": true,
        "start_time": start_time,
        "end_time": end_time,
    }))
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum HostAvailabilityWindowError {
    InvalidStartTime,
    InvalidEndTime,
    SameTime,
}

pub(super) fn validate_host_availability_window(
    start_time: &str,
    end_time: &str,
) -> Result<(), HostAvailabilityWindowError> {
    let start_minute = parse_host_availability_minute(start_time)
        .ok_or(HostAvailabilityWindowError::InvalidStartTime)?;
    let end_minute = parse_host_availability_minute(end_time)
        .ok_or(HostAvailabilityWindowError::InvalidEndTime)?;
    if start_minute == end_minute {
        return Err(HostAvailabilityWindowError::SameTime);
    }
    Ok(())
}

fn parse_host_availability_minute(value: &str) -> Option<u16> {
    let bytes = value.as_bytes();
    if bytes.len() != 5 || bytes[2] != b':' {
        return None;
    }
    let hour = parse_two_digit_host_availability_part(bytes[0], bytes[1])?;
    let minute = parse_two_digit_host_availability_part(bytes[3], bytes[4])?;
    if hour > 23 || minute > 59 {
        return None;
    }
    Some(hour * 60 + minute)
}

fn parse_two_digit_host_availability_part(a: u8, b: u8) -> Option<u16> {
    if !a.is_ascii_digit() || !b.is_ascii_digit() {
        return None;
    }
    Some(u16::from(a - b'0') * 10 + u16::from(b - b'0'))
}

pub(super) fn normalize_stream_mappings(mappings: Vec<Value>) -> Result<Vec<Value>, String> {
    let mut normalized = Vec::with_capacity(mappings.len());
    let mut seen = HashSet::new();
    for mapping in mappings {
        let Some(object) = mapping.as_object() else {
            return Err("Stream mapping must be an object".to_string());
        };
        let protocol = if object
            .get("protocol")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "udp")
        {
            "udp"
        } else {
            "tcp"
        };
        let Some(listen_port) = object.get("listen_port").and_then(json_integer) else {
            return Err("Stream mapping listen_port must be an integer".to_string());
        };
        if listen_port <= 0 || listen_port > 65535 {
            return Err(format!(
                "Stream mapping listen_port {listen_port} is out of range"
            ));
        }
        let key = format!("{protocol}:{listen_port}");
        if !seen.insert(key) {
            return Err(format!(
                "Duplicate stream mapping for {} port {listen_port}",
                protocol.to_ascii_uppercase()
            ));
        }

        let target = object
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if !is_valid_host_port(&target) {
            return Err(format!("Stream mapping target must be host:port: {target}"));
        }

        normalized.push(json!({
            "protocol": protocol,
            "listen_port": listen_port,
            "target": target,
            "use_auth": object.get("use_auth").and_then(Value::as_bool).unwrap_or(true),
        }));
    }
    Ok(normalized)
}

pub(super) fn validate_host_mappings_section(config: &Value) -> Result<(), String> {
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    normalize_host_mappings_for_route(mappings, config).map(|_| ())
}

pub(super) fn validate_passkey_rp_config(config: &Value) -> Result<(), String> {
    let Some(subdomain_mode) = config.get("subdomain_mode").and_then(Value::as_object) else {
        return Ok(());
    };
    let mode = subdomain_mode
        .get("passkey_rp_mode")
        .and_then(Value::as_str)
        .unwrap_or("auth_host");
    if mode != "parent_domain" {
        return Ok(());
    }

    let rp_id = normalize_host_value(
        subdomain_mode
            .get("passkey_rp_id")
            .and_then(Value::as_str)
            .or_else(|| subdomain_mode.get("root_domain").and_then(Value::as_str))
            .unwrap_or(""),
    );
    if rp_id.is_empty() {
        return Err("Passkey parent-domain RP ID is required".to_string());
    }

    let auth_host = get_auth_host_mapping(config)
        .and_then(|mapping| {
            mapping
                .get("host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .or_else(|| {
            subdomain_mode
                .get("auth_host")
                .and_then(Value::as_str)
                .map(normalize_host_value)
        })
        .unwrap_or_default();

    if !auth_host.is_empty() && auth_host != rp_id && !auth_host.ends_with(&format!(".{rp_id}")) {
        return Err(format!(
            "Passkey auth host {auth_host} must match or belong to RP ID {rp_id}"
        ));
    }
    Ok(())
}

pub(super) fn normalize_host_mapping_locations_for_route(
    host: &str,
    value: Option<&Value>,
) -> Result<Vec<Value>, String> {
    let Some(items) = value.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut normalized = Vec::with_capacity(items.len());
    let mut seen = HashSet::new();

    for item in items {
        let object = item.as_object().cloned().unwrap_or_else(Map::new);
        let raw_path = object
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        if raw_path.is_empty() {
            return Err(format!("Host mapping {host} location path is required"));
        }
        if !raw_path.starts_with('/') {
            return Err(format!(
                "Host mapping {host} location path {raw_path} must start with /"
            ));
        }
        let path = clean_host_location_path(raw_path);
        if path == "/" {
            return Err(format!("Host mapping {host} location path / is reserved"));
        }
        if path.starts_with("/__") || path == "/s" || path == "/s/" {
            return Err(format!(
                "Host mapping {host} location path {path} is reserved"
            ));
        }
        let match_mode = if object
            .get("match")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "exact")
        {
            "exact"
        } else {
            "prefix"
        };
        let duplicate_key = format!("{match_mode}\0{path}");
        if !seen.insert(duplicate_key) {
            return Err(format!("Host mapping {host} has duplicate location {path}"));
        }

        let action = if object
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "response")
        {
            "response"
        } else {
            "proxy"
        };
        let target = if action == "proxy" {
            let target = object
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if target.is_empty() {
                return Err(format!(
                    "Host mapping {host} location {path} target is required"
                ));
            }
            if !is_supported_proxy_target_url(&target) {
                return Err(format!(
                    "Host mapping {host} location {path} target must be a supported HTTP/WebSocket URL"
                ));
            }
            target
        } else {
            String::new()
        };

        if action == "response" {
            validate_location_response(host, &path, object.get("response"))?;
        }

        normalized.push(json!({
            "path": path,
            "match": match_mode,
            "action": action,
            "target": target,
            "strip_path": action == "proxy" && object.get("strip_path").and_then(Value::as_bool).unwrap_or(true),
            "rewrite_html": action == "proxy" && object.get("rewrite_html").and_then(Value::as_bool).unwrap_or(true),
            "response": if action == "response" {
                normalize_location_response(object.get("response"))
            } else {
                normalize_location_response(None)
            },
        }));
    }

    Ok(normalized)
}

pub(super) fn validate_location_response(
    host: &str,
    path: &str,
    value: Option<&Value>,
) -> Result<(), String> {
    let object = value.and_then(Value::as_object);
    let status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    if !(100..=599).contains(&status) {
        return Err(format!(
            "Host mapping {host} location {path} response status is invalid"
        ));
    }

    let headers = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object);
    if let Some(headers) = headers {
        for raw_name in headers.keys() {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {raw_name} is invalid"
                ));
            }
            if forbidden_response_header(name) {
                return Err(format!(
                    "Host mapping {host} location {path} response header {name} is forbidden"
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn normalize_location_response(value: Option<&Value>) -> Value {
    let object = value.and_then(Value::as_object);
    let raw_status = object
        .and_then(|map| map.get("status"))
        .and_then(json_number_floor)
        .unwrap_or(200);
    let status = if (100..=599).contains(&raw_status) {
        raw_status
    } else {
        200
    };
    let content_type = object
        .and_then(|map| map.get("content_type"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE);

    let mut headers = Map::new();
    if let Some(header_map) = object
        .and_then(|map| map.get("headers"))
        .and_then(Value::as_object)
    {
        for (raw_name, raw_value) in header_map {
            let name = raw_name.trim();
            if !is_valid_http_header_name(name) || forbidden_response_header(name) {
                continue;
            }
            headers.insert(
                name.to_string(),
                Value::String(
                    raw_value
                        .as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| raw_value.to_string()),
                ),
            );
        }
    }

    json!({
        "status": status,
        "content_type": content_type,
        "headers": headers,
        "body": object
            .and_then(|map| map.get("body"))
            .and_then(Value::as_str)
            .unwrap_or(""),
    })
}
