use super::*;

pub(crate) fn is_request_exempt_from_scan(headers: &HeaderMap, uri: &Uri, config: &Value) -> bool {
    let forwarded_path = resolve_forwarded_path(headers, uri);
    if config
        .get("fnos_share_bypass")
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        == Some(true)
        && is_fnos_share_path(&forwarded_path)
    {
        return true;
    }

    if is_any_subdomain_routing_mode(config) {
        let matched = find_matching_host_mapping(
            &resolve_forwarded_host(headers, uri),
            config
                .get("host_mappings")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
        return is_public_host_mapping(matched);
    }

    let proxy_mappings = config
        .get("proxy_mappings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if let Some(mapping) = find_best_matching_proxy_mapping(&forwarded_path, proxy_mappings) {
        return mapping
            .get("use_auth")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            == false;
    }

    resolve_default_proxy_mapping(config, proxy_mappings)
        .and_then(|mapping| mapping.get("use_auth").and_then(Value::as_bool))
        == Some(false)
}

pub(crate) async fn is_blacklisted_for_preflight(
    state: &AppState,
    ip: &str,
) -> anyhow::Result<bool> {
    let clean_ip = normalize_scanner_ip(ip);
    if clean_ip.is_empty() || is_scanner_local_address(&clean_ip) {
        return Ok(false);
    }

    let settings = load_scanner_settings(state).await?;
    if !settings.enabled || is_scanner_exempt_ip(state, &clean_ip, &settings).await? {
        return Ok(false);
    }

    Ok(state.store.scanner_blacklist_exists(&clean_ip).await?)
}

pub(crate) async fn is_common_path_for_preflight(
    state: &AppState,
    path: &str,
) -> anyhow::Result<bool> {
    let clean_path = normalize_scanner_path(path);
    if is_known_subsonic_rest_path(path) {
        return Ok(true);
    }
    if clean_path == "/__auth__" || clean_path.starts_with("/__auth__/") {
        return Ok(true);
    }
    if clean_path == "/api/auth/passkey" || clean_path.starts_with("/api/auth/passkey/") {
        return Ok(true);
    }
    if clean_path == "/websocket" {
        return Ok(true);
    }
    if clean_path == "/api/admin/terminal" || clean_path.starts_with("/api/admin/terminal/") {
        return Ok(true);
    }
    if clean_path == "/cgi/ThirdParty" || clean_path.starts_with("/cgi/ThirdParty/") {
        return Ok(true);
    }
    if clean_path == "/assets/" || clean_path.starts_with("/assets/") {
        return Ok(true);
    }
    if clean_path == "/s/" || clean_path.starts_with("/s/") {
        return Ok(true);
    }

    const COMMON_PATHS: &[&str] = &[
        "/",
        "/index.html",
        "/robots.txt",
        "/sitemap.xml",
        "/favicon.ico",
        "/favicon.svg",
        "/api/auth/bootstrap",
        "/api/auth/captcha/config",
        "/api/auth/challenge",
        "/api/auth/login",
        "/api/auth/ip",
        "/api/auth/ip/location",
        "/api/auth/session",
        "/api/auth/verify",
        "/api/auth/passkey/status",
        "/trimcon",
        "/.well-known/ai-plugin.json",
        "/apple-touch-icon.png",
        "/manifest.json",
        "/login",
        "/locales/zh-CN/os.json",
        "/license/v1/device/baseInfo",
        "/locales/zh-CN/apps/setting.json",
        "/app-center/v1/check-update?language=zh-CN",
        "/sac/rpcproxy/v1/new-user-guide/status",
        "/locales/zh-CN/pages/login.json",
        "/static/bg/wallpaper-1.webp",
        "/api/config",
        "/identity/connect/token",
    ];
    if COMMON_PATHS.contains(&clean_path.as_str()) {
        return Ok(true);
    }

    let config = state.store.get_config().await?;
    Ok(config
        .get("proxy_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| {
            mapping
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|mapping_path| is_known_proxy_path(&clean_path, mapping_path))
        }))
}

pub(crate) async fn record_uncommon_path_for_preflight(
    state: &AppState,
    ip: &str,
    path: &str,
) -> anyhow::Result<ScannerPreflightRecordResult> {
    let clean_ip = normalize_scanner_ip(ip);
    if clean_ip.is_empty() || is_scanner_local_address(&clean_ip) {
        return Ok(ScannerPreflightRecordResult {
            hit_count: 0,
            blocked: false,
        });
    }

    let settings = load_scanner_settings(state).await?;
    if !settings.enabled || is_scanner_exempt_ip(state, &clean_ip, &settings).await? {
        return Ok(ScannerPreflightRecordResult {
            hit_count: 0,
            blocked: false,
        });
    }

    let now = time_utils::now_ms();
    let clean_path = normalize_scanner_path(path);
    let hit = json!({ "path": clean_path, "createdAt": now });
    let min_score = now - settings.window_seconds * 1000;
    let window_min_score = now - settings.window_minutes * 60 * 1000;
    let hit_count = state
        .store
        .record_scanner_suspicious_hit(
            &clean_ip,
            &hit,
            now,
            min_score,
            window_min_score,
            settings.window_seconds + 60,
        )
        .await?;

    if hit_count >= settings.threshold && !is_blacklisted_for_preflight(state, &clean_ip).await? {
        let hits = state
            .store
            .scanner_suspicious_hits_since(&clean_ip, window_min_score)
            .await?
            .into_iter()
            .filter(|value| {
                value.get("path").and_then(Value::as_str).is_some()
                    && value.get("createdAt").and_then(Value::as_i64).is_some()
            })
            .collect::<Vec<_>>();
        let ip_location = state
            .store
            .get_ip_location_cache(&clean_ip)
            .await?
            .and_then(|value| {
                value
                    .get("raw")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .filter(|value| !value.trim().is_empty());
        let mut record = json!({
            "ip": clean_ip,
            "blockedAt": now,
            "windowMinutes": settings.window_minutes,
            "threshold": settings.threshold,
            "hits": hits,
        });
        if let Some(location) = ip_location.clone()
            && let Some(object) = record.as_object_mut()
        {
            object.insert("ipLocation".to_string(), Value::String(location));
        }
        state
            .store
            .add_scanner_blacklist_record(&clean_ip, &record, now, settings.blacklist_ttl_seconds)
            .await?;
        let registered_location = ip_location::register_usage(
            state,
            &clean_ip,
            vec![format!("scanner-blacklist|{clean_ip}")],
        )
        .await
        .unwrap_or_default();
        let event_hits = record
            .get("hits")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|hit| {
                let path = hit.get("path").and_then(Value::as_str)?;
                let created_at = hit.get("createdAt").and_then(Value::as_i64)?;
                Some(json!({
                    "path": path,
                    "created_at": time_utils::iso_from_ms(created_at),
                }))
            })
            .collect::<Vec<_>>();
        let mut payload = json!({
            "ip": clean_ip,
            "blocked_at": time_utils::iso_from_ms(now),
            "window_minutes": settings.window_minutes,
            "threshold": settings.threshold,
            "hit_count": hit_count,
            "hits": event_hits,
        });
        if let Some(location) = ip_location
            .filter(|value| !value.trim().is_empty())
            .or_else(|| (!registered_location.trim().is_empty()).then_some(registered_location))
            && let Some(object) = payload.as_object_mut()
        {
            object.insert("ip_location".to_string(), Value::String(location));
        }
        if let Err(error) = system_events::publish_scanner_blocked_event(state, payload).await {
            tracing::warn!(%error, %clean_ip, "failed to publish scanner blocked event");
        }
        return Ok(ScannerPreflightRecordResult {
            hit_count,
            blocked: true,
        });
    }

    Ok(ScannerPreflightRecordResult {
        hit_count,
        blocked: false,
    })
}

pub(super) async fn is_scanner_exempt_ip(
    state: &AppState,
    ip: &str,
    settings: &ScannerSettings,
) -> anyhow::Result<bool> {
    if settings.common_location_exempt_enabled
        && common_auth_locations::is_common_auth_location_exempt_ip(state, ip).await?
    {
        return Ok(true);
    }
    Ok(is_scanner_cidr_exempt_ip(
        ip,
        &settings.cidr_exemption_cidrs,
    ))
}

pub(super) fn is_scanner_cidr_exempt_ip(ip: &str, cidrs: &[String]) -> bool {
    let normalized = http_utils::normalize_ip(ip);
    let Ok(ip) = normalized.parse::<IpAddr>() else {
        return false;
    };
    cidrs
        .iter()
        .filter_map(|cidr| cidr.trim().parse::<IpNet>().ok())
        .any(|network| network.contains(&ip))
}

pub(super) fn normalize_scanner_ip(ip: &str) -> String {
    ip.trim().to_string()
}

pub(super) fn is_scanner_local_address(ip: &str) -> bool {
    let mut candidate = ip.trim().to_ascii_lowercase();
    if candidate.is_empty() {
        return false;
    }

    if candidate.starts_with('[')
        && let Some(end) = candidate.find(']')
    {
        candidate = candidate[1..end].to_string();
    }

    if candidate == "localhost" || candidate.starts_with("localhost:") {
        return true;
    }
    if candidate == "::1" || candidate == "0:0:0:0:0:0:0:1" {
        return true;
    }
    if candidate.starts_with("fc") || candidate.starts_with("fd") || candidate.starts_with("fe80:")
    {
        return true;
    }

    if let Some(mapped) = candidate.strip_prefix("::ffff:") {
        return is_private_scanner_ipv4(mapped);
    }
    is_private_scanner_ipv4(&candidate)
}

pub(super) fn is_private_scanner_ipv4(candidate: &str) -> bool {
    let (ip, port) = candidate
        .split_once(':')
        .map_or((candidate, None), |(ip, port)| (ip, Some(port)));
    if port.is_some_and(|port| port.is_empty() || port.chars().any(|ch| !ch.is_ascii_digit())) {
        return false;
    }
    let octets = ip.split('.').collect::<Vec<_>>();
    if octets.len() != 4
        || octets
            .iter()
            .any(|octet| octet.is_empty() || octet.chars().any(|ch| !ch.is_ascii_digit()))
    {
        return false;
    }
    octets[0] == "10"
        || octets[0] == "127"
        || (octets[0] == "192" && octets[1] == "168")
        || (octets[0] == "172"
            && matches!(
                octets[1],
                "16" | "17"
                    | "18"
                    | "19"
                    | "20"
                    | "21"
                    | "22"
                    | "23"
                    | "24"
                    | "25"
                    | "26"
                    | "27"
                    | "28"
                    | "29"
                    | "30"
                    | "31"
            ))
}

pub(super) fn resolve_forwarded_host(headers: &HeaderMap, uri: &Uri) -> String {
    if let Some(value) = first_header_value(headers, "x-forwarded-host") {
        return normalize_scanner_host(&value);
    }
    if let Some(authority) = uri.authority() {
        return normalize_scanner_host(authority.as_str());
    }
    headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .map(normalize_scanner_host)
        .unwrap_or_default()
}

pub(super) fn resolve_forwarded_path(headers: &HeaderMap, uri: &Uri) -> String {
    if let Some(value) = first_header_value(headers, "x-forwarded-path") {
        return value;
    }
    uri.path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string())
}

pub(super) use crate::http_utils::first_header_value;

pub(super) fn normalize_scanner_host(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return String::new();
    }

    if let Ok(url) = Url::parse(&format!("https://{normalized}"))
        && let Some(host) = url.host_str()
    {
        return host.trim_end_matches('.').to_ascii_lowercase();
    }

    let mut host = normalized.as_str();
    if let Some((_, rest)) = host.split_once("://") {
        host = rest;
    }
    if let Some((_, rest)) = host.rsplit_once('@') {
        host = rest;
    }
    host = host.split('/').next().unwrap_or("").trim();
    if host.starts_with('[')
        && let Some(end) = host.find(']')
    {
        return host[1..end].trim_end_matches('.').to_string();
    }
    if let Some((without_port, port)) = host.rsplit_once(':')
        && !port.is_empty()
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        host = without_port;
    }
    host.trim_end_matches('.').to_string()
}

pub(super) fn normalize_scanner_path(path: &str) -> String {
    let clean = path
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("");
    if clean.is_empty() {
        return "/".to_string();
    }
    let mut normalized = if clean.starts_with('/') {
        clean.to_string()
    } else {
        format!("/{clean}")
    };
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

pub(super) fn parse_scanner_path(value: &str) -> Option<Url> {
    let base = Url::parse("http://127.0.0.1").ok()?;
    Url::options().base_url(Some(&base)).parse(value).ok()
}

pub(super) fn normalize_subsonic_rest_endpoint(path: &str) -> String {
    let clean = normalize_scanner_path(path);
    let Some(rest) = clean.strip_prefix("/rest/") else {
        return String::new();
    };
    if rest.contains('/') {
        return String::new();
    }
    let (endpoint, extension) = rest
        .split_once('.')
        .map_or((rest, None), |(endpoint, extension)| {
            (endpoint, Some(extension))
        });
    if extension.is_some_and(|value| !matches!(value, "view" | "json" | "xml")) {
        return String::new();
    }
    let endpoint = endpoint.to_ascii_lowercase();
    if endpoint
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_alphabetic())
        && endpoint.chars().all(|value| value.is_ascii_alphanumeric())
    {
        endpoint
    } else {
        String::new()
    }
}

pub(super) fn is_known_subsonic_rest_path(path: &str) -> bool {
    let Some(parsed) = parse_scanner_path(path) else {
        return false;
    };
    let endpoint = normalize_subsonic_rest_endpoint(parsed.path());
    !endpoint.is_empty() && SUBSONIC_REST_ENDPOINTS.contains(&endpoint.as_str())
}

pub(super) fn is_known_proxy_path(request_path: &str, mapping_path: &str) -> bool {
    let clean_request_path = normalize_scanner_path(request_path);
    let clean_mapping_path = normalize_scanner_path(mapping_path);
    clean_mapping_path == "/"
        || clean_request_path == clean_mapping_path
        || clean_request_path.starts_with(&format!("{clean_mapping_path}/"))
}

pub(super) fn is_fnos_share_path(path: &str) -> bool {
    let clean_path = normalize_scanner_path(path);
    clean_path == "/s" || clean_path.starts_with("/s/")
}

pub(super) fn find_best_matching_proxy_mapping<'a>(
    path: &str,
    mappings: &'a [Value],
) -> Option<&'a Value> {
    let clean_path = normalize_scanner_path(path);
    let mut best_match = None;
    let mut best_length = 0_usize;
    for mapping in mappings {
        let Some(mapping_path) = mapping.get("path").and_then(Value::as_str) else {
            continue;
        };
        if !is_known_proxy_path(&clean_path, mapping_path) {
            continue;
        }
        let length = normalize_scanner_path(mapping_path).len();
        if length > best_length {
            best_match = Some(mapping);
            best_length = length;
        }
    }
    best_match
}

pub(super) fn find_matching_host_mapping<'a>(
    host: &str,
    mappings: &'a [Value],
) -> Option<&'a Value> {
    let clean_host = normalize_scanner_host(host);
    if clean_host.is_empty() {
        return None;
    }
    mappings.iter().find(|mapping| {
        mapping
            .get("host")
            .and_then(Value::as_str)
            .map(normalize_scanner_host)
            == Some(clean_host.clone())
    })
}

pub(super) fn is_public_host_mapping(mapping: Option<&Value>) -> bool {
    mapping.is_some_and(|mapping| {
        mapping.get("use_auth").and_then(Value::as_bool) == Some(false)
            && mapping.get("access_mode").and_then(Value::as_str) != Some("strict_whitelist")
    })
}

pub(super) fn resolve_default_proxy_mapping<'a>(
    config: &Value,
    mappings: &'a [Value],
) -> Option<&'a Value> {
    let default_route = config
        .get("default_route")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "/__select__")?;
    mappings.iter().find(|mapping| {
        mapping
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| {
                normalize_scanner_path(path) == normalize_scanner_path(default_route)
            })
    })
}

pub(super) use crate::proxy_utils::is_any_subdomain_routing_mode;
