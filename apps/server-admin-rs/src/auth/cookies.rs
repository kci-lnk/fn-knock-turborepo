use std::env;

use axum::http::HeaderMap;

pub const SESSION_COOKIE_NAME: &str = "x-go-reauth-proxy-session-id";
pub const ADMIN_PANEL_SESSION_COOKIE_NAME: &str = "fn-knock-admin-panel-session";
pub const FNOS_SHARE_SESSION_COOKIE_NAME: &str = "fn-knock-fnos-share-session";
pub const OIDC_LOGIN_ERROR_COOKIE_NAME: &str = "fn-knock-oidc-login-error";
pub const OIDC_FLOW_COOKIE_NAME: &str = "fn-knock-oidc-flow";

pub fn read_cookie(headers: &HeaderMap, name: &str) -> Option<String> {
    let cookie = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    for segment in cookie.split(';') {
        let trimmed = segment.trim();
        let (key, value) = match trimmed.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if key.trim() == name {
            return Some(percent_decode(value));
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
pub fn build_cookie(
    name: &str,
    value: &str,
    max_age: i64,
    path: &str,
    domain: Option<&str>,
    http_only: bool,
    secure: bool,
    same_site: &str,
) -> String {
    let mut parts = vec![
        format!("{name}={value}"),
        format!("Path={path}"),
        format!("SameSite={same_site}"),
        format!("Max-Age={max_age}"),
    ];
    if let Some(domain) = domain.filter(|value| !value.is_empty()) {
        parts.insert(2, format!("Domain={domain}"));
    }
    if http_only {
        parts.insert(2, "HttpOnly".to_string());
    }
    if secure {
        parts.push("Secure".to_string());
    }
    parts.join("; ")
}

pub fn session_cookie(session_id: &str, max_age: i64, domain: Option<&str>) -> String {
    build_cookie(
        SESSION_COOKIE_NAME,
        session_id,
        max_age,
        "/",
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn session_clear_cookie(domain: Option<&str>) -> String {
    build_cookie(
        SESSION_COOKIE_NAME,
        "",
        0,
        "/",
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn admin_panel_cookie(session_id: &str, max_age: i64, secure: bool) -> String {
    admin_panel_cookie_with_same_site(session_id, max_age, secure, session_cookie_same_site())
}

fn admin_panel_cookie_with_same_site(
    session_id: &str,
    max_age: i64,
    secure: bool,
    same_site: &str,
) -> String {
    build_cookie(
        ADMIN_PANEL_SESSION_COOKIE_NAME,
        session_id,
        max_age,
        "/",
        None,
        true,
        secure,
        same_site,
    )
}

pub fn admin_panel_clear_cookie(secure: bool) -> String {
    build_cookie(
        ADMIN_PANEL_SESSION_COOKIE_NAME,
        "",
        0,
        "/",
        None,
        true,
        secure,
        session_cookie_same_site(),
    )
}

pub fn fnos_share_clear_cookie(domain: Option<&str>) -> String {
    build_cookie(
        FNOS_SHARE_SESSION_COOKIE_NAME,
        "",
        0,
        "/s",
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn fnos_share_session_cookie(session_id: &str, max_age: i64, domain: Option<&str>) -> String {
    build_cookie(
        FNOS_SHARE_SESSION_COOKIE_NAME,
        session_id,
        max_age,
        "/s",
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn oidc_login_error_cookie(
    token: &str,
    max_age: i64,
    domain: Option<&str>,
    path: &str,
) -> String {
    build_cookie(
        OIDC_LOGIN_ERROR_COOKIE_NAME,
        token,
        max_age,
        path,
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn oidc_login_error_clear_cookie(domain: Option<&str>, path: &str) -> String {
    build_cookie(
        OIDC_LOGIN_ERROR_COOKIE_NAME,
        "",
        0,
        path,
        domain,
        true,
        session_cookie_secure(true),
        session_cookie_same_site(),
    )
}

pub fn oidc_flow_cookie(token: &str, max_age: i64, domain: Option<&str>, path: &str) -> String {
    build_cookie(
        OIDC_FLOW_COOKIE_NAME,
        token,
        max_age,
        path,
        domain,
        true,
        session_cookie_secure(true),
        "Lax",
    )
}

pub fn oidc_flow_clear_cookie(domain: Option<&str>, path: &str) -> String {
    build_cookie(
        OIDC_FLOW_COOKIE_NAME,
        "",
        0,
        path,
        domain,
        true,
        session_cookie_secure(true),
        "Lax",
    )
}

fn session_cookie_secure(default_value: bool) -> bool {
    match env::var("SESSION_COOKIE_SECURE")
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("0" | "false" | "no" | "off") => false,
        Some("1" | "true" | "yes" | "on") => true,
        _ => default_value,
    }
}

fn session_cookie_same_site() -> &'static str {
    session_cookie_same_site_from_raw(env::var("SESSION_COOKIE_SAMESITE").ok().as_deref())
}

fn session_cookie_same_site_from_raw(raw: Option<&str>) -> &'static str {
    match raw
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("strict") => "Strict",
        Some("none") => "None",
        _ => "Lax",
    }
}

pub(crate) fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return value.to_string();
            }
            let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            else {
                return value.to_string();
            };
            output.push((high << 4) | low);
            index += 3;
            continue;
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(output).unwrap_or_else(|_| value.to_string())
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue, header};

    use super::*;

    #[test]
    fn reads_percent_decoded_cookie_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("a=1; x-go-reauth-proxy-session-id=abc%20123"),
        );
        assert_eq!(
            read_cookie(&headers, SESSION_COOKIE_NAME).as_deref(),
            Some("abc 123")
        );
    }

    #[test]
    fn reads_cookie_like_node_when_segments_are_malformed() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("flag; x-go-reauth-proxy-session-id=abc"),
        );
        assert_eq!(
            read_cookie(&headers, SESSION_COOKIE_NAME).as_deref(),
            Some("abc")
        );
    }

    #[test]
    fn preserves_cookie_value_whitespace_like_node() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("x-go-reauth-proxy-session-id= abc"),
        );
        assert_eq!(
            read_cookie(&headers, SESSION_COOKIE_NAME).as_deref(),
            Some(" abc")
        );
    }

    #[test]
    fn leaves_malformed_percent_encoded_cookie_value_unchanged_like_node() {
        assert_eq!(percent_decode("abc%20def%zz"), "abc%20def%zz");
    }

    #[test]
    fn builds_admin_panel_cookie_with_secure_flag() {
        let cookie = admin_panel_cookie("sid", 60, true);
        assert!(cookie.contains("fn-knock-admin-panel-session=sid"));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Lax"));
    }

    #[test]
    fn admin_panel_cookie_uses_session_same_site_env_like_node() {
        assert_eq!(session_cookie_same_site_from_raw(Some("none")), "None");
        assert_eq!(
            session_cookie_same_site_from_raw(Some(" strict ")),
            "Strict"
        );

        let cookie = admin_panel_cookie_with_same_site("sid", 60, false, "None");
        assert!(cookie.contains("SameSite=None"));
        assert!(!cookie.contains("Secure"));
    }
}
