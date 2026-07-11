use super::*;

pub(super) fn find_matching_totp(
    credentials: &[TotpCredential],
    token: &str,
) -> Option<TotpCredential> {
    credentials
        .iter()
        .find(|credential| verify_totp_token(&credential.secret, token).unwrap_or(false))
        .cloned()
}

pub(crate) fn verify_totp_token(secret: &str, token: &str) -> anyhow::Result<bool> {
    let secret = Secret::Encoded(secret.trim().replace(' ', ""));
    let bytes = secret.to_bytes()?;
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes)?;
    Ok(totp.check_current(token)?)
}

pub(crate) fn safe_redirect(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> Option<String> {
    let raw_value = redirect_uri?;
    if is_unsafe_redirect_reference(raw_value) {
        return None;
    }
    let value = raw_value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('/') {
        let base = url::Url::parse("http://127.0.0.1").ok()?;
        let target = base.join(value).ok()?;
        if is_post_logout_redirect(&target) {
            return Some(relative_url(&normalize_post_logout_redirect_target(
                &target,
            )));
        }
        return Some(value.to_string());
    }

    let mut target = url::Url::parse(value).ok()?;
    if !matches!(target.scheme(), "http" | "https") {
        return None;
    }
    if is_post_logout_redirect(&target) {
        target = normalize_post_logout_redirect_target(&target);
    }

    if let (Some(proto), Some(host)) = (
        Some(resolve_forwarded_proto(headers)),
        resolve_forwarded_host(headers),
    ) && let Ok(current_origin) = url::Url::parse(&format!("{proto}://{host}"))
        && same_origin(&target, &current_origin)
    {
        return Some(target.to_string());
    }

    let target_host = target.host_str().map(normalize_subdomain_access_host)?;
    if target_host.is_empty() {
        return None;
    }

    if let Some(root_domain) = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_subdomain_access_host)
        .filter(|value| !value.is_empty())
        && host_within_domain(&target_host, &root_domain)
    {
        return Some(target.to_string());
    }

    let configured = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_subdomain_access_host)
        .any(|host| !host.is_empty() && host == target_host);
    if configured {
        return Some(target.to_string());
    }

    if let Some(auth_base_url) = resolve_public_auth_base_url(config)
        && let Ok(auth_base_url) = url::Url::parse(&auth_base_url)
        && same_origin(&target, &auth_base_url)
    {
        return Some(target.to_string());
    }

    None
}

fn is_unsafe_redirect_reference(value: &str) -> bool {
    if value.contains('\\') || value.chars().any(|character| character.is_ascii_control()) {
        return true;
    }
    let value = value.trim();
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    let Some(second) = chars.next() else {
        return false;
    };
    first == '/' && second == '/'
}

pub(crate) fn effective_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    grant_type: &str,
    redirect_uri: Option<&str>,
) -> Option<String> {
    let redirect_to = safe_redirect(config, headers, redirect_uri)?;
    if grant_type == "browser_session"
        && !can_browser_session_reach_redirect_uri(config, headers, Some(&redirect_to))
    {
        return None;
    }
    Some(redirect_to)
}

pub(crate) fn resolve_cookie_domain(config: &Value, headers: &HeaderMap) -> Option<String> {
    let request_host = resolve_request_hostname_from_headers(headers);
    resolve_cookie_domain_for_request_host(config, request_host.as_deref())
}

pub(crate) fn resolve_cookie_clear_domains(
    config: Option<&Value>,
    headers: &HeaderMap,
) -> Vec<Option<String>> {
    // A host-only cookie and Domain cookies with the same name can coexist.
    // Clear every scope that this deployment (including older releases) may
    // have used, rather than relying on Cookie header ordering.
    let mut domains = vec![None];
    let mut seen = BTreeSet::new();
    let mut push_domain = |raw: &str| {
        let domain = normalize_subdomain_access_host(raw)
            .trim_start_matches('.')
            .to_string();
        if domain.is_empty() || domain.parse::<IpAddr>().is_ok() || !seen.insert(domain.clone()) {
            return;
        }
        domains.push(Some(domain));
    };

    if let Some(config) = config
        && let Some(domain) = resolve_cookie_domain(config, headers)
    {
        push_domain(&domain);
    }
    if let Some(hostname) = resolve_request_hostname_from_headers(headers) {
        push_domain(&hostname);
    }
    domains
}

pub(super) fn resolve_shared_auth_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> Option<String> {
    if !is_any_subdomain_routing_mode(config) {
        return None;
    }
    let shared_auth_base_url = resolve_public_auth_base_url(config)?;
    let shared_auth_url = url::Url::parse(&shared_auth_base_url).ok()?;
    let shared_auth_hostname = shared_auth_url
        .host_str()
        .map(normalize_subdomain_access_host)?;
    if resolve_request_hostname_from_headers(headers).as_deref()
        == Some(shared_auth_hostname.as_str())
    {
        // Forwarded scheme/port can reflect the internal proxy hop rather
        // than the public listener. Host identity is sufficient to prevent a
        // shared auth page from redirecting to itself in that case.
        return None;
    }
    let request_proto = resolve_forwarded_proto(headers);
    let request_host = resolve_forwarded_host(headers)?;
    let current_origin = format!("{request_proto}://{request_host}");
    if let Ok(current_origin_url) = url::Url::parse(&current_origin) {
        if same_origin(&shared_auth_url, &current_origin_url) {
            return None;
        }
    } else {
        return None;
    }

    let shared_auth_host = shared_auth_url.host_str()?;
    if !can_browser_session_reach_redirect_uri_for_host(
        config,
        Some(shared_auth_host),
        Some(&current_origin),
    ) {
        return None;
    }

    let safe_redirect_uri = safe_redirect(config, headers, redirect_uri);
    build_shared_auth_login_url(&shared_auth_base_url, safe_redirect_uri.as_deref())
}

pub(super) fn build_shared_auth_login_url(
    auth_base_url: &str,
    redirect_uri: Option<&str>,
) -> Option<String> {
    let base = auth_base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    let mut login_url = url::Url::parse(&format!("{base}/#/login")).ok()?;
    if let Some(redirect_uri) = redirect_uri.filter(|value| !value.trim().is_empty()) {
        login_url
            .query_pairs_mut()
            .append_pair("redirect_uri", redirect_uri);
    }
    Some(login_url.to_string())
}

pub(super) fn can_browser_session_reach_redirect_uri(
    config: &Value,
    headers: &HeaderMap,
    redirect_uri: Option<&str>,
) -> bool {
    can_browser_session_reach_redirect_uri_for_host(
        config,
        resolve_request_hostname_from_headers(headers).as_deref(),
        redirect_uri,
    )
}

pub(super) fn can_browser_session_reach_redirect_uri_for_host(
    config: &Value,
    request_host: Option<&str>,
    redirect_uri: Option<&str>,
) -> bool {
    let raw = redirect_uri.map(str::trim).unwrap_or_default();
    if raw.is_empty() || raw.starts_with('/') {
        return true;
    }
    let Ok(target) = url::Url::parse(raw) else {
        return false;
    };
    let Some(target_host) = target.host_str().map(normalize_subdomain_access_host) else {
        return false;
    };
    if target_host.is_empty() {
        return false;
    }
    if let Some(cookie_domain) = resolve_cookie_domain_for_request_host(config, request_host) {
        return host_within_domain(&target_host, &cookie_domain);
    }
    request_host
        .map(normalize_subdomain_access_host)
        .filter(|host| !host.is_empty())
        .is_some_and(|host| host == target_host)
}

pub(super) fn resolve_cookie_domain_for_request_host(
    config: &Value,
    request_host: Option<&str>,
) -> Option<String> {
    let request_host = request_host
        .map(normalize_subdomain_access_host)
        .unwrap_or_default();
    let can_use =
        |candidate: &str| request_host.is_empty() || host_within_domain(&request_host, candidate);
    if let Some(domain) = config
        .pointer("/subdomain_mode/cookie_domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        && can_use(domain)
    {
        return Some(domain.to_string());
    }
    if let Ok(domain) = env::var("SESSION_COOKIE_DOMAIN") {
        let domain = domain.trim().to_string();
        if !domain.is_empty() && can_use(&domain) {
            return Some(domain);
        }
    }
    if is_any_subdomain_routing_mode(config)
        && let Some(root_domain) = config
            .pointer("/subdomain_mode/root_domain")
            .and_then(Value::as_str)
            .map(normalize_subdomain_access_host)
            .filter(|value| !value.is_empty())
        && !request_host.is_empty()
        && host_within_domain(&request_host, &root_domain)
    {
        return Some(root_domain);
    }
    None
}

pub(crate) fn resolve_public_auth_base_url(config: &Value) -> Option<String> {
    if !is_reverse_proxy_subdomain_mode(config)
        && let Some(explicit) = config
            .pointer("/subdomain_mode/public_auth_base_url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| apply_public_port_to_base_url(value, config))
    {
        return Some(explicit);
    }
    let auth_host = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                if item
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(is_auth_service_target)
                {
                    item.get("host").and_then(Value::as_str)
                } else {
                    None
                }
            })
        })
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    format_derived_public_auth_base_url(auth_host, config)
}

pub(super) fn is_auth_service_target(target: &str) -> bool {
    let Ok(parsed) = url::Url::parse(target.trim()) else {
        return false;
    };
    if !matches!(parsed.scheme(), "http" | "https" | "ws" | "wss")
        || parsed.host_str().is_none_or(|host| host.trim().is_empty())
    {
        return false;
    }
    parsed.port_or_known_default() == Some(resolve_auth_service_port())
}

pub(super) fn resolve_auth_service_port() -> u16 {
    env::var("AUTH_PORT")
        .ok()
        .and_then(|value| value.trim().parse::<u16>().ok())
        .filter(|port| *port > 0)
        .unwrap_or(7997)
}

pub(super) fn apply_public_port_to_base_url(raw_base_url: &str, config: &Value) -> Option<String> {
    let trimmed = raw_base_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }
    let Ok(mut parsed) = url::Url::parse(trimmed) else {
        return Some(trimmed.to_string());
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return Some(trimmed.to_string());
    }
    if parsed.port().is_none()
        && let Some(port) =
            resolve_public_port_for_scheme(config, parsed.scheme(), trimmed, true, false)
        && !is_default_scheme_port(parsed.scheme(), port)
    {
        let _ = parsed.set_port(Some(port));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let path = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(if path.is_empty() { "/" } else { &path });
    Some(parsed.to_string().trim_end_matches('/').to_string())
}

pub(super) fn format_derived_public_auth_base_url(host: &str, config: &Value) -> Option<String> {
    let host = host.trim().to_ascii_lowercase();
    if host.is_empty() {
        return None;
    }
    let scheme = "https";
    let public_base = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some(port) = resolve_auth_public_port_for_scheme(config, scheme, public_base, true)
        && !is_default_scheme_port(scheme, port)
    {
        return Some(format!("{scheme}://{host}:{port}"));
    }
    Some(format!("{scheme}://{host}"))
}

pub(super) fn parse_explicit_url_port(raw_url: &str, scheme: &str) -> Option<u16> {
    let parsed = url::Url::parse(raw_url.trim()).ok()?;
    if parsed.scheme() != scheme {
        return None;
    }
    parsed.port()
}

pub(super) fn resolve_configured_public_port(
    config: &Value,
    scheme: &str,
    allow_reverse_proxy_configured_port: bool,
) -> Option<u16> {
    if is_reverse_proxy_subdomain_mode(config) && !allow_reverse_proxy_configured_port {
        return None;
    }
    let pointer = if scheme == "https" {
        "/subdomain_mode/public_https_port"
    } else {
        "/subdomain_mode/public_http_port"
    };
    config
        .pointer(pointer)
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(raw) => raw.trim().parse::<i64>().ok(),
            _ => None,
        })
        .filter(|port| *port > 0 && *port <= u16::MAX as i64)
        .map(|port| port as u16)
}

pub(super) fn resolve_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
    allow_reverse_proxy_configured_port: bool,
) -> Option<u16> {
    if let Some(port) = parse_explicit_url_port(raw_public_base_url, scheme) {
        return Some(port);
    }
    if let Some(port) =
        resolve_configured_public_port(config, scheme, allow_reverse_proxy_configured_port)
    {
        return Some(port);
    }
    if should_omit_public_access_entry_port(config) || !gateway_fallback {
        return None;
    }
    resolve_public_gateway_port(config)
}

pub(super) fn resolve_auth_public_port_for_scheme(
    config: &Value,
    scheme: &str,
    raw_public_base_url: &str,
    gateway_fallback: bool,
) -> Option<u16> {
    resolve_public_port_for_scheme(config, scheme, raw_public_base_url, gateway_fallback, true)
}

pub(super) use crate::system_info::resolve_public_gateway_port_u16 as resolve_public_gateway_port;

pub(super) fn is_default_scheme_port(scheme: &str, port: u16) -> bool {
    (scheme == "https" && port == 443) || (scheme == "http" && port == 80)
}

pub(super) use crate::proxy_utils::{
    is_any_subdomain_routing_mode, is_reverse_proxy_subdomain_mode,
};

pub(super) fn is_cloudflared_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    is_reverse_proxy_subdomain_mode(config)
        && config
            .get("default_tunnel")
            .and_then(Value::as_str)
            .unwrap_or("frp")
            == "cloudflared"
}

pub(super) fn should_omit_public_access_entry_port(config: &Value) -> bool {
    is_cloudflared_reverse_proxy_subdomain_mode(config)
        || (config.get("run_type").and_then(Value::as_i64) == Some(3)
            && config
                .pointer("/subdomain_mode/edge_client_ip_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            && (config
                .pointer("/subdomain_mode/aliyun_esa_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                || config
                    .pointer("/subdomain_mode/tencent_edgeone_enabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)))
}

pub(super) fn resolve_forwarded_proto(headers: &HeaderMap) -> String {
    let proto = parse_forwarded_header_proto(headers)
        .or_else(|| first_header_value(headers, "x-forwarded-proto"))
        .or_else(|| first_header_value(headers, "x-forwarded-scheme"))
        .or_else(|| first_header_value(headers, "x-original-proto"))
        .or_else(|| first_header_value(headers, "x-original-scheme"))
        .unwrap_or_else(|| "http".to_string());
    let proto = proto.trim().trim_end_matches(':').to_ascii_lowercase();
    if matches!(proto.as_str(), "http" | "https") {
        proto
    } else {
        "https".to_string()
    }
}

pub(super) fn resolve_forwarded_host(headers: &HeaderMap) -> Option<String> {
    parse_forwarded_header_host(headers)
        .or_else(|| first_header_value(headers, "x-forwarded-host"))
        .or_else(|| first_header_value(headers, "x-original-host"))
        .or_else(|| first_header_value(headers, "host"))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn resolve_request_hostname_from_headers(headers: &HeaderMap) -> Option<String> {
    resolve_forwarded_host(headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty())
}

pub(super) fn parse_forwarded_header_proto(headers: &HeaderMap) -> Option<String> {
    crate::http_utils::forwarded_header_value(headers, "proto")
}

pub(super) fn host_within_domain(host: &str, domain: &str) -> bool {
    let host = normalize_subdomain_access_host(host);
    let domain = normalize_subdomain_access_host(domain)
        .trim_start_matches('.')
        .to_string();
    !host.is_empty()
        && !domain.is_empty()
        && (host == domain || host.ends_with(&format!(".{domain}")))
}

pub(super) fn same_origin(left: &url::Url, right: &url::Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str().map(normalize_subdomain_access_host)
            == right.host_str().map(normalize_subdomain_access_host)
        && left.port_or_known_default() == right.port_or_known_default()
}

pub(super) fn is_post_logout_redirect(target: &url::Url) -> bool {
    target
        .query_pairs()
        .any(|(key, value)| key == "logged_out" && value == "1")
        && is_logged_out_login_path(target.path())
}

pub(super) fn is_logged_out_login_path(pathname: &str) -> bool {
    let normalized = normalize_pathname(pathname);
    matches!(
        normalized.as_str(),
        "/login" | "/auth/login" | "/__auth__/login"
    )
}

pub(super) fn normalize_post_logout_redirect_target(target: &url::Url) -> url::Url {
    let mut normalized = target.clone();
    normalized.set_path(match normalize_pathname(target.path()).as_str() {
        "/auth/login" => "/auth/",
        "/__auth__/login" => "/__auth__/",
        _ => "/",
    });
    let pairs = target
        .query_pairs()
        .filter(|(key, _)| key != "logged_out")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    normalized.set_query(None);
    if !pairs.is_empty() {
        let mut query = normalized.query_pairs_mut();
        for (key, value) in pairs {
            query.append_pair(&key, &value);
        }
    }
    normalized.set_fragment(None);
    normalized
}
