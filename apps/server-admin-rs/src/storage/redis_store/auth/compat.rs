use super::*;

pub(in crate::storage::redis_store) fn login_backoff_status_from_raw(
    requested_ip: &str,
    raw: Option<&str>,
    now_ms: i64,
) -> LoginBackoffStatus {
    let Some(raw) = raw else {
        return LoginBackoffStatus {
            ip: requested_ip.to_string(),
            attempts: 0,
            blocked: false,
            retry_after: None,
            blocked_until: None,
        };
    };
    let Ok(state) = serde_json::from_str::<LoginBackoffAttemptState>(raw) else {
        return LoginBackoffStatus {
            ip: requested_ip.to_string(),
            attempts: 0,
            blocked: false,
            retry_after: None,
            blocked_until: None,
        };
    };
    let blocked = state
        .blocked_until
        .is_some_and(|blocked_until| now_ms <= blocked_until);
    let retry_after = if blocked {
        state
            .blocked_until
            .map(|blocked_until| ((blocked_until - now_ms).max(1000) + 999) / 1000)
    } else {
        None
    };
    LoginBackoffStatus {
        ip: requested_ip.to_string(),
        attempts: state.attempts,
        blocked,
        retry_after,
        blocked_until: state.blocked_until,
    }
}
pub(in crate::storage::redis_store) fn normalize_totp_credentials(
    totps: &[TotpCredential],
) -> Vec<TotpCredential> {
    totps
        .iter()
        .filter_map(|credential| {
            normalize_totp_credential_value(
                &serde_json::to_value(credential).unwrap_or(Value::Null),
            )
        })
        .collect()
}

pub(in crate::storage::redis_store) fn normalize_totp_credentials_value(
    value: &Value,
) -> Vec<TotpCredential> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(normalize_totp_credential_value)
        .collect()
}

pub(in crate::storage::redis_store) fn normalize_totp_credential_value(
    value: &Value,
) -> Option<TotpCredential> {
    let object = value.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let secret = object
        .get("secret")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() || secret.is_empty() {
        return None;
    }
    let comment = object
        .get("comment")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let created_at = object
        .get("createdAt")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    Some(TotpCredential {
        id,
        secret,
        comment,
        created_at: if created_at.is_empty() {
            now_iso()
        } else {
            created_at
        },
        access_scopes: normalize_totp_access_scopes(
            object.get("access_scopes").cloned().unwrap_or(Value::Null),
        ),
        subdomain_access: normalize_totp_subdomain_access(
            object
                .get("subdomain_access")
                .cloned()
                .unwrap_or(Value::Null),
        ),
    })
}

pub(crate) fn normalize_totp_access_scopes(value: Value) -> Value {
    let mut scopes = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            let scope = js_string(item).trim().to_string();
            if scope == "docker_admin_panel"
                && !scopes
                    .iter()
                    .any(|existing: &Value| existing.as_str() == Some("docker_admin_panel"))
            {
                scopes.push(Value::String("docker_admin_panel".to_string()));
            }
        }
    }
    Value::Array(scopes)
}

pub(crate) fn normalize_totp_subdomain_access(value: Value) -> Value {
    let mode = value
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| *mode == "custom")
        .unwrap_or("all");
    if mode != "custom" {
        return json!({ "mode": "all", "hosts": [], "streams": [] });
    }
    let hosts = value
        .get("hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|host| normalize_totp_subdomain_access_host(&js_string(host)))
                .filter(|host| !host.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_totp_stream_access)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|(listen_port, protocol)| {
                    json!({
                        "protocol": protocol,
                        "listen_port": listen_port,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "mode": "custom", "hosts": hosts, "streams": streams })
}

pub(in crate::storage::redis_store) fn normalize_totp_stream_access(
    value: &Value,
) -> Option<(i64, String)> {
    let protocol = value
        .get("protocol")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    if protocol != "tcp" && protocol != "udp" {
        return None;
    }
    let listen_port = value.get("listen_port").and_then(Value::as_i64)?;
    (1..=65535)
        .contains(&listen_port)
        .then_some((listen_port, protocol))
}

pub(in crate::storage::redis_store) fn normalize_totp_subdomain_access_host(value: &str) -> String {
    let mut host = value.trim().to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE || host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_WOL_PAGE || host == TOTP_SUBDOMAIN_ACCESS_WOL_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_WOL_PAGE.to_string();
    }

    if let Ok(url) = if host.contains("://") {
        url::Url::parse(&host)
    } else {
        url::Url::parse(&format!("https://{host}"))
    } {
        host = url.host_str().unwrap_or("").to_string();
    } else {
        if let Some((_, rest)) = host.split_once("://") {
            host = rest.to_string();
        }
        if let Some((_, rest)) = host.rsplit_once('@') {
            host = rest.to_string();
        }
        host = host
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if host.starts_with('[') {
            if let Some(end) = host.find(']') {
                host = host[1..end].to_string();
            }
        } else if host.matches(':').count() == 1
            && let Some((without_port, _)) = host.rsplit_once(':')
        {
            host = without_port.to_string();
        }
    }

    host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.contains('*')
        || host
            .chars()
            .any(|value| value.is_whitespace() || value == ',')
    {
        return String::new();
    }
    host
}
