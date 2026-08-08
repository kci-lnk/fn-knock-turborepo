use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::{i18n::Translator, response, state::AppState, time_utils};

type HmacSha256 = Hmac<Sha256>;

fn hmac_text(translator: &Translator, key: &str) -> String {
    let translation_key = format!("server.hmac.{key}");
    match key {
        "missingHeaders" => translator.t_with_fallback(
            &translation_key,
            "Missing Required Security Headers (x-timestamp, x-nonce, x-signature)",
        ),
        "invalidTimestampFormat" => {
            translator.t_with_fallback(&translation_key, "Invalid Timestamp Format")
        }
        "invalidNonceLength" => {
            translator.t_with_fallback(&translation_key, "Invalid Nonce Length")
        }
        _ => translator.t(&translation_key),
    }
}

async fn hmac_error(state: &AppState, status: StatusCode, key: &str) -> Response {
    let translator = Translator::from_state(state).await;
    response::error(status, hmac_text(&translator, key))
}

pub async fn hmac_middleware(
    State(state): State<AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    if !requires_hmac(req.uri().path()) {
        return next.run(req).await;
    }

    let secret = state.settings.hmac_secret.trim();
    if secret.is_empty() {
        return next.run(req).await;
    }

    let headers = match parse_hmac_headers(req.headers()) {
        Ok(headers) => headers,
        Err((status, key)) => return hmac_error(&state, status, key).await,
    };

    if (time_utils::now_ms() - headers.timestamp_ms).abs() > 5 * 60 * 1000 {
        return hmac_error(&state, StatusCode::UNAUTHORIZED, "timestampExpired").await;
    }

    let mut mac = match HmacSha256::new_from_slice(secret.as_bytes()) {
        Ok(mac) => mac,
        Err(_) => return hmac_error(&state, StatusCode::INTERNAL_SERVER_ERROR, "invalidKey").await,
    };
    mac.update(format!("{}:{}", headers.timestamp, headers.nonce).as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    if expected
        .as_bytes()
        .ct_eq(headers.signature.as_bytes())
        .unwrap_u8()
        != 1
    {
        return hmac_error(&state, StatusCode::UNAUTHORIZED, "invalidSignature").await;
    }

    match state
        .store
        .set_nonce_if_not_exists(&headers.nonce, 600)
        .await
    {
        Ok(true) => next.run(req).await,
        Ok(false) => hmac_error(&state, StatusCode::UNAUTHORIZED, "nonceReused").await,
        Err(error) => {
            tracing::warn!(%error, "failed to store HMAC nonce");
            hmac_error(
                &state,
                StatusCode::INTERNAL_SERVER_ERROR,
                "nonceVerifyFailed",
            )
            .await
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedHmacHeaders {
    timestamp: String,
    timestamp_ms: i64,
    nonce: String,
    signature: String,
}

fn parse_hmac_headers(
    headers: &HeaderMap,
) -> Result<ParsedHmacHeaders, (StatusCode, &'static str)> {
    let Some(timestamp) = header_value(headers, "x-timestamp") else {
        return Err((StatusCode::UNAUTHORIZED, "missingHeaders"));
    };
    let Some(nonce) = header_value(headers, "x-nonce") else {
        return Err((StatusCode::UNAUTHORIZED, "missingHeaders"));
    };
    let Some(signature) = header_value(headers, "x-signature") else {
        return Err((StatusCode::UNAUTHORIZED, "missingHeaders"));
    };
    let Some(timestamp_ms) = parse_js_parse_int_radix_10(&timestamp) else {
        return Err((StatusCode::BAD_REQUEST, "invalidTimestampFormat"));
    };
    if nonce.len() < 8 {
        return Err((StatusCode::BAD_REQUEST, "invalidNonceLength"));
    }

    Ok(ParsedHmacHeaders {
        timestamp,
        timestamp_ms,
        nonce,
        signature: signature.to_ascii_lowercase(),
    })
}

fn header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

use crate::node_compat::parse_i64_prefix_trim_start as parse_js_parse_int_radix_10;

fn requires_hmac(path: &str) -> bool {
    let normalized = normalize_auth_api_path(path);
    if !normalized.starts_with("/api") {
        return false;
    }

    const IGNORED_PATHS: &[&str] = &[
        "/api/auth/challenge",
        "/api/auth/verify",
        "/api/auth/logout",
        "/api/auth/preflight",
        "/api/auth/oidc/bind",
        "/api/auth/oidc/bind/",
        "/api/auth/oidc/client-metadata",
        "/api/internal/system-events",
    ];
    const IGNORED_PATH_PREFIXES: &[&str] = &["/api/auth/oidc/callback/"];

    if IGNORED_PATHS.contains(&normalized.as_str()) {
        return false;
    }
    if IGNORED_PATH_PREFIXES
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
    {
        return false;
    }

    true
}

fn normalize_auth_api_path(path: &str) -> String {
    if path.starts_with("/auth/api") {
        return path["/auth".len()..].to_string();
    }
    if path.starts_with("/__auth__/api") {
        return path["/__auth__".len()..].to_string();
    }
    path.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localizes_hmac_errors() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            hmac_text(&translator, "missingTimestamp"),
            "缺少 HMAC 时间戳"
        );
        assert_eq!(hmac_text(&translator, "nonceReused"), "HMAC nonce 已被使用");
        assert_eq!(
            hmac_text(&translator, "missingHeaders"),
            "Missing Required Security Headers (x-timestamp, x-nonce, x-signature)"
        );
    }

    #[test]
    fn hmac_header_parse_int_and_statuses_match_node_middleware() {
        assert_eq!(parse_js_parse_int_radix_10("123abc"), Some(123));
        assert_eq!(parse_js_parse_int_radix_10("+123abc"), Some(123));
        assert_eq!(parse_js_parse_int_radix_10("-5ms"), Some(-5));
        assert_eq!(parse_js_parse_int_radix_10("0x10"), Some(0));
        assert_eq!(parse_js_parse_int_radix_10("abc"), None);

        let mut headers = HeaderMap::new();
        headers.insert("x-timestamp", "123abc".parse().unwrap());
        headers.insert("x-nonce", "12345678".parse().unwrap());
        headers.insert("x-signature", "ABCDEF".parse().unwrap());
        assert_eq!(
            parse_hmac_headers(&headers).expect("headers"),
            ParsedHmacHeaders {
                timestamp: "123abc".to_string(),
                timestamp_ms: 123,
                nonce: "12345678".to_string(),
                signature: "abcdef".to_string(),
            }
        );

        headers.remove("x-signature");
        assert_eq!(
            parse_hmac_headers(&headers).expect_err("missing signature"),
            (StatusCode::UNAUTHORIZED, "missingHeaders")
        );

        headers.insert("x-signature", "abcdef".parse().unwrap());
        headers.insert("x-timestamp", "abc".parse().unwrap());
        assert_eq!(
            parse_hmac_headers(&headers).expect_err("invalid timestamp"),
            (StatusCode::BAD_REQUEST, "invalidTimestampFormat")
        );

        headers.insert("x-timestamp", "123".parse().unwrap());
        headers.insert("x-nonce", "1234567".parse().unwrap());
        assert_eq!(
            parse_hmac_headers(&headers).expect_err("short nonce"),
            (StatusCode::BAD_REQUEST, "invalidNonceLength")
        );
    }

    #[test]
    fn hmac_path_exemptions_match_node_middleware() {
        assert!(!requires_hmac("/api/auth/challenge"));
        assert!(!requires_hmac("/api/auth/verify"));
        assert!(!requires_hmac("/api/auth/logout"));
        assert!(!requires_hmac("/api/auth/preflight"));
        assert!(!requires_hmac("/api/auth/oidc/bind"));
        assert!(!requires_hmac("/api/auth/oidc/bind/"));
        assert!(!requires_hmac("/api/auth/oidc/client-metadata"));
        assert!(!requires_hmac("/api/auth/oidc/callback/provider-1"));
        assert!(!requires_hmac("/api/internal/system-events"));

        assert!(requires_hmac("/api/auth/challenge/"));
        assert!(requires_hmac("/api/auth/verify/"));
        assert!(requires_hmac("/api/auth/logout/"));
        assert!(requires_hmac("/api/auth/preflight/"));
        assert!(requires_hmac("/api/auth/oidc/bind/foo"));
        assert!(requires_hmac("/api/auth/oidc/client-metadata/"));
        assert!(requires_hmac("/api/auth/oidc/callback"));
        assert!(requires_hmac("/api/internal/system-events/"));
        assert!(requires_hmac("/api/auth/oidc/providers"));
        assert!(requires_hmac("/api/auth/oidc/providers/"));
        assert!(requires_hmac("/api/auth/session"));
        assert!(requires_hmac("/api/auth/wol/targets"));
        assert!(requires_hmac("/api/auth/wol/targets/device-1/wake"));
        assert!(requires_hmac("/api/admin/config"));
    }

    #[test]
    fn hmac_normalizes_auth_mount_prefixes_like_node() {
        assert!(requires_hmac("/auth/api"));
        assert!(requires_hmac("/__auth__/api"));
        assert!(!requires_hmac("/auth/api/auth/oidc/bind"));
        assert!(!requires_hmac("/auth/api/auth/oidc/client-metadata"));
        assert!(!requires_hmac(
            "/__auth__/api/auth/oidc/callback/provider-1"
        ));
        assert!(requires_hmac("/auth/api/auth/verify/"));
        assert!(requires_hmac("/auth/api/auth/oidc/providers"));
        assert!(requires_hmac("/__auth__/api/auth/session"));
        assert!(requires_hmac("/__auth__/api/auth/wol/targets"));
        assert!(!requires_hmac("/auth"));
        assert!(!requires_hmac("/__auth__/index.html"));
    }
}
