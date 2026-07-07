use super::*;

pub(super) fn update_binding_profile_fields(binding: &mut Value, profile: &ExternalProfile) {
    if let Some(object) = binding.as_object_mut() {
        if let Some(value) = profile
            .display_name
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            object.insert("display_name".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = profile.email.as_deref().filter(|value| !value.is_empty()) {
            object.insert("email".to_string(), Value::String(value.to_string()));
        }
        if let Some(value) = profile.email_verified {
            object.insert("email_verified".to_string(), Value::Bool(value));
        }
        if let Some(value) = profile
            .avatar_url
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            object.insert("avatar_url".to_string(), Value::String(value.to_string()));
        }
    }
}

pub(super) fn extra_auth_params(config: &Map<String, Value>) -> Vec<(String, String)> {
    config
        .get("extra_auth_params")
        .and_then(Value::as_object)
        .map(|extra| {
            extra
                .iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn provider_config<'a>(
    provider: &'a Value,
    translator: &Translator,
) -> Result<&'a Map<String, Value>, String> {
    provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))
}

pub(super) fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn string_field_from_value<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn scopes(config: &Map<String, Value>, fallback: &[&str]) -> Vec<String> {
    let values = config
        .get("scopes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if values.is_empty() {
        fallback.iter().map(|value| (*value).to_string()).collect()
    } else {
        values
    }
}

pub(super) fn optional_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(|value| match value {
            Value::String(value) => Some(value.trim().to_string()),
            Value::Number(value) => Some(value.to_string()),
            _ => None,
        })
        .filter(|value| !value.is_empty())
}

pub(super) fn value_truthy(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(value)) => *value,
        Some(Value::String(value)) => !value.is_empty(),
        Some(Value::Number(value)) => value.as_i64().unwrap_or_default() != 0,
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

pub(super) fn build_callback_url(
    provider_id: &str,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
) -> Result<String, String> {
    if let Some(base) = public_auth_base_url(config) {
        return Ok(format!(
            "{}/api/auth/oidc/callback/{}",
            base.trim_end_matches('/'),
            encode_query(provider_id)
        ));
    }
    let origin = request_origin(headers, uri, translator)?;
    let prefix = auth_api_prefix(uri.path());
    Ok(format!(
        "{origin}{prefix}/api/auth/oidc/callback/{}",
        encode_query(provider_id)
    ))
}

pub(super) fn build_login_redirect(
    config: &Value,
    headers: &HeaderMap,
    path: &str,
    redirect_uri: Option<&str>,
) -> String {
    let prefix = configured_auth_view_prefix(config, headers, path);
    let mut location = format!("{prefix}/login");
    if let Some(redirect_uri) = redirect_uri.filter(|value| !value.trim().is_empty()) {
        location.push('?');
        location.push_str("redirect_uri=");
        location.push_str(&encode_query(redirect_uri));
    }
    location
}

pub(super) fn resolve_oidc_cookie_path(config: &Value, headers: &HeaderMap, path: &str) -> String {
    configured_auth_view_prefix(config, headers, path)
        .trim_end_matches('/')
        .to_string()
        .if_empty("/")
}

pub(super) fn configured_auth_view_prefix(
    config: &Value,
    _headers: &HeaderMap,
    path: &str,
) -> String {
    if let Some(prefix) = auth_view_prefix(path) {
        return prefix.to_string();
    }
    if let Some(base_url) = public_auth_base_url(config)
        && let Ok(url) = Url::parse(&base_url)
    {
        let path = url.path().trim_end_matches('/');
        if !path.is_empty() && path != "/" {
            return path.to_string();
        }
    }
    String::new()
}

pub(super) fn auth_view_prefix(path: &str) -> Option<&'static str> {
    if path == "/__auth__" || path.starts_with("/__auth__/") {
        Some("/__auth__")
    } else if path == "/auth" || path.starts_with("/auth/") {
        Some("/auth")
    } else {
        None
    }
}

pub(super) fn auth_api_prefix(path: &str) -> &'static str {
    if path.starts_with("/__auth__/api/auth/") {
        "/__auth__"
    } else if path.starts_with("/auth/api/auth/") {
        "/auth"
    } else {
        ""
    }
}

pub(super) fn request_origin(
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
) -> Result<String, String> {
    let trust_forwarded = env_bool("OIDC_TRUST_FORWARDED_HEADERS", false)
        || env_bool("AUTH_TRUST_FORWARDED_HEADERS", false);
    let request_proto = uri.scheme_str().unwrap_or("http");
    let proto = if trust_forwarded {
        first_header(headers, "x-forwarded-proto")
    } else {
        None
    }
    .unwrap_or_else(|| request_proto.to_string())
    .trim()
    .trim_end_matches(':')
    .to_ascii_lowercase();
    let host = if trust_forwarded {
        first_header(headers, "x-forwarded-host")
    } else {
        None
    }
    .or_else(|| first_header(headers, "host"))
    .or_else(|| {
        uri.authority()
            .map(|authority| authority.as_str().to_string())
    })
    .ok_or_else(|| oidc_text(translator, "callbackUrlBuildFailed"))?;
    if (proto != "http" && proto != "https")
        || host
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, ',' | '/' | '?' | '#' | '\\' | '@'))
    {
        return Err(oidc_text(translator, "callbackUrlBuildFailed"));
    }
    Ok(format!("{proto}://{host}"))
}

pub(super) fn public_auth_base_url(config: &Value) -> Option<String> {
    crate::auth::resolve_public_auth_base_url(config)
}

pub(super) fn resolve_cookie_domain(config: &Value, headers: &HeaderMap) -> Option<String> {
    crate::auth::resolve_cookie_domain(config, headers)
}

pub(super) fn first_header(headers: &HeaderMap, name: &str) -> Option<String> {
    crate::http_utils::first_header_value(headers, name)
}

pub(super) fn client_ip_for_headers(headers: &HeaderMap) -> String {
    let ip = get_client_ip(headers);
    if ip.is_empty() {
        "127.0.0.1".to_string()
    } else {
        ip
    }
}

pub(super) fn user_agent(headers: &HeaderMap) -> String {
    headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.trim().chars().take(512).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "Unknown".to_string())
}

pub(super) fn locale_code(config: &Value) -> String {
    config
        .pointer("/locale/default_locale")
        .and_then(Value::as_str)
        .unwrap_or("zh-CN")
        .to_string()
}

pub(super) fn oidc_flow_token_valid(state: &str, flow_token: Option<&str>) -> bool {
    let expected = hash_oidc_token(state);
    let Some(flow_token) = flow_token.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    expected.as_bytes().ct_eq(flow_token.as_bytes()).unwrap_u8() == 1
}

pub(super) fn create_oidc_id(prefix: &str) -> String {
    format!("{prefix}_{}", hex::encode(rand::random::<[u8; 10]>()))
}

pub(super) fn create_public_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

pub(super) fn create_pkce_verifier() -> String {
    create_public_token()
}

pub(super) fn create_pkce_challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub(super) fn hash_oidc_token(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}

pub(super) fn build_subject_key(provider_id: &str, issuer: &str, subject: &str) -> String {
    hex::encode(Sha256::digest(format!(
        "{provider_id}\0{issuer}\0{subject}"
    )))
}

pub(super) fn normalize_login_error_message(message: &str, translator: &Translator) -> String {
    let message = message.trim();
    if message.is_empty() {
        oidc_text(translator, "loginFailedRetry")
    } else {
        message.chars().take(500).collect()
    }
}

pub(super) fn apply_no_store_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store, no-cache, max-age=0, must-revalidate"),
    );
    headers.insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    headers.insert(header::EXPIRES, HeaderValue::from_static("0"));
    headers.insert(
        "CDN-Cache-Control",
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert("Surrogate-Control", HeaderValue::from_static("no-store"));
}

pub(super) fn append_set_cookie(headers: &mut HeaderMap, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        headers.append(header::SET_COOKIE, value);
    }
}

pub(super) fn encode_query(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

pub(super) fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(super) fn env_bool(name: &str, fallback: bool) -> bool {
    crate::node_compat::env_bool(name, fallback)
}

trait EmptyFallback {
    fn if_empty(self, fallback: &str) -> String;
}

impl EmptyFallback for String {
    fn if_empty(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}
