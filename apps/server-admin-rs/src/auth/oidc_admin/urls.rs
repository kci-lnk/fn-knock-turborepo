use axum::http::{HeaderMap, Uri};
use serde_json::Value;

pub(super) fn callback_base_url(headers: &HeaderMap, uri: &Uri, config: &Value) -> Option<String> {
    public_auth_base_url(config).or_else(|| callback_origin(headers, uri))
}

pub(super) fn callback_origin(headers: &HeaderMap, uri: &Uri) -> Option<String> {
    let trust_forwarded = crate::node_compat::env_bool("OIDC_TRUST_FORWARDED_HEADERS", false)
        || crate::node_compat::env_bool("AUTH_TRUST_FORWARDED_HEADERS", false);
    let request_proto = uri.scheme_str().unwrap_or("http");
    let proto = if trust_forwarded {
        first_header(headers, "x-forwarded-proto")
    } else {
        None
    }
    .unwrap_or_else(|| request_proto.to_string());
    let proto = proto.trim().trim_end_matches(':').to_ascii_lowercase();
    if proto != "http" && proto != "https" {
        return None;
    }

    let host = if trust_forwarded {
        first_header(headers, "x-forwarded-host")
    } else {
        None
    }
    .or_else(|| first_header(headers, "host"))
    .or_else(|| {
        uri.authority()
            .map(|authority| authority.as_str().to_string())
    })?;
    if host
        .chars()
        .any(|ch| ch.is_whitespace() || matches!(ch, ',' | '/' | '?' | '#' | '\\' | '@'))
    {
        return None;
    }
    Some(format!("{proto}://{host}"))
}

pub(super) fn invite_base_url(headers: &HeaderMap, uri: &Uri, config: &Value) -> Option<String> {
    callback_base_url(headers, uri, config)
}

pub(super) use crate::auth::resolve_public_auth_base_url as public_auth_base_url;

pub(super) use crate::http_utils::first_header_value as first_header;
