use super::*;

pub(crate) use crate::http_utils::{apply_no_store_headers, user_agent};

pub(super) fn normalize_pathname(pathname: &str) -> String {
    let pathname = pathname.trim();
    if pathname.is_empty() {
        return "/".to_string();
    }
    let pathname = if pathname.starts_with('/') {
        pathname.to_string()
    } else {
        format!("/{pathname}")
    };
    let normalized = pathname.trim_end_matches('/').to_string();
    if normalized.is_empty() {
        "/".to_string()
    } else {
        normalized
    }
}

pub(super) fn relative_url(target: &url::Url) -> String {
    let mut value = target.path().to_string();
    if let Some(query) = target.query() {
        value.push('?');
        value.push_str(query);
    }
    if let Some(fragment) = target.fragment() {
        value.push('#');
        value.push_str(fragment);
    }
    value
}

pub(super) fn enqueue_auth_ip_location(state: &AppState, ip: &str, context: &'static str) {
    if ip.trim().is_empty() {
        return;
    }
    let state = state.clone();
    let ip = ip.to_string();
    tokio::spawn(async move {
        if let Err(error) =
            ip_location::ensure_ip_locations_enqueued(&state, vec![ip.clone()]).await
        {
            tracing::warn!(%error, %ip, %context, "failed to enqueue auth IP location lookup");
        }
    });
}

pub(crate) fn client_ip_for_auth(headers: &HeaderMap) -> String {
    http_utils::get_client_ip(headers)
}

pub(super) fn credential_name(credential: &TotpCredential, translator: &Translator) -> String {
    let name = credential.comment.trim();
    if name.is_empty() {
        auth_route_text(translator, "unknownTotp")
    } else {
        name.to_string()
    }
}

pub(super) fn post_logout_location(headers: &HeaderMap, uri: &Uri) -> String {
    let base = resolve_auth_ui_base_prefix(headers, uri);
    format!("{base}/login?logged_out=1")
}

pub(crate) fn resolve_auth_ui_base_prefix(headers: &HeaderMap, uri: &Uri) -> &'static str {
    for pathname in [
        Some(uri.path().to_string()),
        header_pathname(headers, "x-forwarded-path"),
        header_pathname(headers, header::REFERER.as_str()),
    ]
    .into_iter()
    .flatten()
    {
        if pathname == "/__auth__" || pathname.starts_with("/__auth__/") {
            return "/__auth__";
        }
        if pathname == "/auth" || pathname.starts_with("/auth/") {
            return "/auth";
        }
    }
    ""
}

pub(super) fn header_pathname(headers: &HeaderMap, name: &str) -> Option<String> {
    let value = headers.get(name)?.to_str().ok()?;
    let base = url::Url::parse("http://127.0.0.1").ok()?;
    base.join(value).ok().map(|url| url.path().to_string())
}

pub(crate) fn with_auth_headers(mut response: Response) -> Response {
    apply_no_store_headers(response.headers_mut());
    response
}

pub(super) fn parse_pow_expires(salt: &str) -> Option<i64> {
    let query = salt.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=')
            && key == "expires"
        {
            return value.parse::<i64>().ok();
        }
    }
    None
}

pub(super) use crate::crypto_utils::{
    hmac_sha256_hex, random_bytes, sha256_hex_bytes as sha256_hex,
};

#[allow(dead_code)]
pub(super) fn _method_is_head(method: &Method) -> bool {
    method == Method::HEAD
}
