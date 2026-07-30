use super::subdomain_grant;
use super::*;

const EXPIRED_SESSION_BACKGROUND_CLEANUP_TIMEOUT: std::time::Duration =
    std::time::Duration::from_secs(12);
const EXPIRED_SESSION_CLEANUP_MARKER_TTL_SECONDS: usize = 30;

pub(super) async fn preflight(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());

    if let Err(error) = apply_preflight_behavior(&state, &headers, &uri, &mut response).await {
        let client_ip = client_ip_for_auth(&headers);
        let forwarded_path = preflight_forwarded_path(&headers);
        tracing::warn!(%error, %client_ip, %forwarded_path, "auth preflight failed");
    }
    response
}

pub(super) async fn apply_preflight_behavior(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    response: &mut Response,
) -> anyhow::Result<()> {
    apply_preflight_behavior_with_routed_upstream(state, headers, uri, response, None, None, None)
        .await
}

pub(super) async fn apply_preflight_behavior_with_routed_upstream(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    response: &mut Response,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<()> {
    let config = state.store.config_snapshot();
    apply_preflight_behavior_with_routed_upstream_and_config(
        state,
        headers,
        uri,
        response,
        config.as_ref(),
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn apply_preflight_behavior_with_routed_upstream_and_config(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    response: &mut Response,
    config: &Value,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<()> {
    let client_ip = client_ip_for_auth(headers);
    let access_mode = requested_access_mode(headers);
    let normal_access =
        resolve_preflight_normal_access(state, headers, uri, config, &client_ip, access_mode)
            .await?;

    apply_preflight_behavior_with_normal_access(
        state,
        headers,
        uri,
        response,
        config,
        &client_ip,
        access_mode,
        &normal_access,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn apply_preflight_behavior_with_normal_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    response: &mut Response,
    config: &Value,
    client_ip: &str,
    access_mode: RequestedAccessMode,
    normal_access: &PreflightNormalAccess,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<()> {
    let forwarded_path = preflight_forwarded_path(headers);
    let mut share_decision_handled = false;
    let strict_whitelist_denied = if access_mode == RequestedAccessMode::StrictWhitelist {
        match has_preflight_whitelist_access(state, client_ip).await {
            Ok(allowed) => !allowed,
            Err(error) => {
                tracing::warn!(
                    %error,
                    %client_ip,
                    "strict whitelist lookup failed; denying preflight"
                );
                true
            }
        }
    } else {
        false
    };
    let active_rule_access = if !normal_access.authorized && !strict_whitelist_denied {
        subdomain_grant::has_valid_probe(state, headers, config)
            || subdomain_grant::inspect_existing(state, headers, config)
                .await?
                .is_some()
    } else {
        false
    };

    if normal_access.invalid_session_cookie {
        for domain in resolve_cookie_clear_domains(Some(config), headers) {
            if let Ok(value) =
                HeaderValue::from_str(&cookies::session_clear_cookie(domain.as_deref()))
            {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
        }
    }

    if strict_whitelist_denied {
        response
            .headers_mut()
            .insert("X-Option", HeaderValue::from_static("Deny"));
    } else if active_rule_access {
        // A previously issued grant or a valid short-lived cookie probe is
        // already bound to this host and policy. Verify will reuse the grant
        // or exchange the probe without relying on the current rule inputs.
    } else if normal_access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        insert_preflight_headers(response, &normal_access.response_headers);
        response.headers_mut().insert(
            REAUTH_ACCESS_DENIED_HEADER,
            HeaderValue::from_static(REAUTH_SCOPE_DENIED),
        );
    } else if !normal_access.authorized {
        let decision = fnos_share_bypass::resolve_preflight(
            state,
            headers,
            uri,
            config,
            routed_upstream,
            routed_upstream_host,
            routed_upstream_route_id,
        )
        .await?;
        share_decision_handled = decision.handled;
        if let Some(location) = decision.redirect_location {
            insert_header_value(response, "X-Reauth-Redirect-Location", &location);
        }
    }

    if !normal_access.authorized
        && config.get("run_type").and_then(Value::as_i64).unwrap_or(0) != 0
        && !scanner::is_request_exempt_from_scan(headers, uri, config)
    {
        if scanner::is_blacklisted_for_preflight(state, client_ip).await? {
            response
                .headers_mut()
                .insert("X-Option", HeaderValue::from_static("Deny"));
        } else if !state
            .store
            .is_recent_auth_ip_active(client_ip, time_utils::now_ms() / 1000)
            .await?
            && !share_decision_handled
            && !forwarded_path.is_empty()
            && !scanner::is_common_path_for_preflight(state, &forwarded_path).await?
        {
            let _ = scanner::record_uncommon_path_for_preflight(state, client_ip, &forwarded_path)
                .await?;
        }
    }

    Ok(())
}

pub(super) fn insert_preflight_headers(response: &mut Response, values: &[(String, String)]) {
    for (key, value) in values {
        insert_header_value(response, key, value);
    }
}

pub(super) fn insert_header_value(response: &mut Response, key: &str, value: &str) {
    if let Ok(header_value) = HeaderValue::from_str(value) {
        response.headers_mut().insert(
            axum::http::HeaderName::from_bytes(key.as_bytes())
                .unwrap_or_else(|_| axum::http::HeaderName::from_static("x-ignored-invalid")),
            header_value,
        );
    }
}

pub(super) fn apply_auth_access_response_headers(headers: &mut HeaderMap, access: &AuthAccess) {
    for cookie in &access.set_cookies {
        if let Ok(value) = HeaderValue::from_str(cookie) {
            headers.append(header::SET_COOKIE, value);
        }
    }
    for (key, value) in &access.response_headers {
        if let Ok(header_value) = HeaderValue::from_str(value) {
            headers.insert(
                axum::http::HeaderName::from_bytes(key.as_bytes())
                    .unwrap_or_else(|_| axum::http::HeaderName::from_static("x-ignored-invalid")),
                header_value,
            );
        }
    }
}

pub(super) fn preflight_forwarded_path(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-path")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .trim()
        .to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RequestedAccessMode {
    LoginFirst,
    StrictWhitelist,
}

pub(super) fn requested_access_mode(headers: &HeaderMap) -> RequestedAccessMode {
    headers
        .get("x-reauth-access-mode")
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.trim().eq_ignore_ascii_case("strict_whitelist"))
        .map(|_| RequestedAccessMode::StrictWhitelist)
        .unwrap_or(RequestedAccessMode::LoginFirst)
}

#[cfg(test)]
pub(super) fn is_strict_whitelist_request(headers: &HeaderMap) -> bool {
    requested_access_mode(headers) == RequestedAccessMode::StrictWhitelist
}

#[derive(Debug, Default)]
pub(super) struct PreflightNormalAccess {
    pub(super) authorized: bool,
    pub(super) grant_type: Option<String>,
    pub(super) deny_reason: Option<String>,
    pub(super) invalid_session_cookie: bool,
    pub(super) response_headers: Vec<(String, String)>,
}

#[derive(Debug)]
pub(super) struct SessionSubdomainAccessDecision {
    pub(super) protected_host: bool,
    pub(super) allowed: bool,
    pub(super) response_headers: Vec<(String, String)>,
}

#[derive(Debug, Default)]
pub(super) struct AuthMobilityRequestIdentity {
    pub(super) session_id: Option<String>,
    pub(super) fnos_token: Option<String>,
    pub(super) trim_media_token: Option<String>,
    pub(super) app_binding: Option<&'static str>,
}

impl AuthMobilityRequestIdentity {
    fn has_app_mobility_signal(&self) -> bool {
        self.fnos_token.is_some() || self.trim_media_token.is_some() || self.app_binding.is_some()
    }
}

#[derive(Debug)]
pub(super) struct MobilitySubdomainAccessDecision {
    pub(super) protected_host: bool,
    pub(super) has_owner_session: bool,
    pub(super) allowed: bool,
    pub(super) response_headers: Vec<(String, String)>,
}

pub(super) async fn resolve_preflight_normal_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    client_ip: &str,
    access_mode: RequestedAccessMode,
) -> anyhow::Result<PreflightNormalAccess> {
    let identity = inspect_auth_mobility_request(headers);
    let mut invalid_session_cookie = false;
    let browser_session = if let Some(session_id) = identity.session_id.as_deref() {
        match state.store.get_session(session_id).await? {
            Some(session) if login_session_has_expired(&session) => {
                invalid_session_cookie = true;
                revoke_expired_presented_session(state, session_id, &session, config).await;
                None
            }
            Some(session) => Some((session_id.to_string(), session)),
            None => {
                invalid_session_cookie = true;
                None
            }
        }
    } else {
        None
    };

    if http_utils::is_private_or_local_ip(client_ip) {
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("local_exempt".to_string()),
            invalid_session_cookie,
            ..Default::default()
        });
    }

    if has_preflight_whitelist_access_from_sources(state, client_ip, Some(&["manual"])).await? {
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("manual_whitelist".to_string()),
            invalid_session_cookie,
            ..Default::default()
        });
    }

    let mut session_scope_headers = Vec::new();
    if let Some((_session_id, session)) = browser_session.as_ref() {
        let scope = resolve_session_subdomain_access(state, headers, uri, config, session).await?;
        if !scope.allowed {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                invalid_session_cookie,
                response_headers: scope.response_headers,
                ..Default::default()
            });
        }
        session_scope_headers = scope.response_headers;
    }

    if identity.has_app_mobility_signal() {
        let mobility =
            resolve_mobility_subdomain_access(state, headers, uri, config, client_ip, &identity)
                .await?;
        if mobility.protected_host && mobility.has_owner_session && !mobility.allowed {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                invalid_session_cookie,
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
        session_scope_headers = mobility.response_headers;
    }

    let restored = auth_mobility::try_restore_access(
        state,
        client_ip,
        auth_mobility::AuthMobilityRestoreIdentity {
            session_id: identity.session_id.as_deref(),
            fnos_token: identity.fnos_token.as_deref(),
            trim_media_token: identity.trim_media_token.as_deref(),
            app_binding: identity.app_binding,
        },
    )
    .await?;
    if restored.success {
        let mobility =
            resolve_mobility_subdomain_access(state, headers, uri, config, client_ip, &identity)
                .await?;
        if mobility.protected_host && (!mobility.has_owner_session || !mobility.allowed) {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                invalid_session_cookie,
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
        if access_mode != RequestedAccessMode::StrictWhitelist {
            return Ok(PreflightNormalAccess {
                authorized: true,
                grant_type: restored
                    .grant_type
                    .map(ToString::to_string)
                    .or_else(|| Some("browser_session".to_string())),
                invalid_session_cookie,
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
    }

    if let Some((session_id, session)) = browser_session.as_ref() {
        if let Err(error) = auth_mobility::sync_browser_session_ip_with_session(
            state,
            session_id,
            session,
            client_ip,
            "browser-session",
        )
        .await
        {
            tracing::warn!(%error, %session_id, "failed to sync browser session IP");
        }

        if access_mode != RequestedAccessMode::StrictWhitelist {
            return Ok(PreflightNormalAccess {
                authorized: true,
                grant_type: Some("browser_session".to_string()),
                invalid_session_cookie,
                response_headers: session_scope_headers,
                ..Default::default()
            });
        }
    }

    if has_preflight_whitelist_access_from_sources(state, client_ip, Some(&["auto"])).await? {
        let mobility =
            resolve_auto_ip_subdomain_access(state, headers, uri, config, client_ip).await?;
        if mobility.protected_host && (!mobility.has_owner_session || !mobility.allowed) {
            return Ok(PreflightNormalAccess {
                authorized: false,
                deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                invalid_session_cookie,
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
        return Ok(PreflightNormalAccess {
            authorized: true,
            grant_type: Some("login_ip_grant".to_string()),
            invalid_session_cookie,
            response_headers: mobility.response_headers,
            ..Default::default()
        });
    }

    if access_mode != RequestedAccessMode::StrictWhitelist && identity.has_app_mobility_signal() {
        let mobility =
            resolve_mobility_subdomain_access(state, headers, uri, config, client_ip, &identity)
                .await?;
        if has_resolvable_auth_mobility_access(state, client_ip, &identity).await? {
            if mobility.protected_host && (!mobility.has_owner_session || !mobility.allowed) {
                return Ok(PreflightNormalAccess {
                    authorized: false,
                    deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
                    invalid_session_cookie,
                    response_headers: mobility.response_headers,
                    ..Default::default()
                });
            }
            return Ok(PreflightNormalAccess {
                authorized: true,
                grant_type: Some("fnos_fingerprint_session".to_string()),
                invalid_session_cookie,
                response_headers: mobility.response_headers,
                ..Default::default()
            });
        }
    }

    Ok(PreflightNormalAccess {
        authorized: false,
        invalid_session_cookie,
        ..Default::default()
    })
}

pub(crate) fn login_session_has_expired(session: &LoginSession) -> bool {
    session
        .expires_at
        .as_deref()
        .and_then(time_utils::parse_iso_ms)
        .is_some_and(|expires_at| expires_at <= time_utils::now_ms())
}

pub(crate) async fn revoke_expired_presented_session(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    config: &Value,
) {
    // Revoke the authoritative key before touching lease-protected secondary
    // state. Writers recheck this key before publication, so authorization
    // fails closed immediately even if another request holds the mobility
    // mutation lease indefinitely.
    if let Err(error) = state.store.delete_session(session_id).await {
        tracing::warn!(%error, %session_id, "failed to delete expired auth session authority");
    }

    let cleanup_marker_key = format!(
        "fn_knock:auth:expired_session_cleanup:{}",
        crate::crypto_utils::sha256_hex_str(session_id)
    );
    let should_start_cleanup = match state
        .store
        .set_json_value_nx_ex(
            &cleanup_marker_key,
            &json!({
                "sessionId": session_id,
                "createdAt": time_utils::now_iso(),
            }),
            EXPIRED_SESSION_CLEANUP_MARKER_TTL_SECONDS,
        )
        .await
    {
        Ok(created) => created,
        Err(error) => {
            tracing::warn!(%error, %session_id, "failed to deduplicate expired session cleanup");
            true
        }
    };
    if !should_start_cleanup {
        return;
    }

    let state = state.clone();
    let session_id = session_id.to_string();
    let session = session.clone();
    let config = config.clone();
    tokio::spawn(async move {
        let cleanup = async {
            if let Err(error) = auth_mobility::destroy_session(&state, &session_id).await {
                tracing::warn!(%error, %session_id, "failed to destroy expired auth session state");
            }
            if let Err(error) = revoke_custom_post_login_ip_grant(
                &state,
                Some(&session),
                Some(&config),
                &session.ip,
            )
            .await
            {
                tracing::warn!(%error, %session_id, "failed to revoke expired session IP grant");
            }
            whitelist::sync_reverse_proxy_trusted_ips(&state).await;
        };
        if tokio::time::timeout(EXPIRED_SESSION_BACKGROUND_CLEANUP_TIMEOUT, cleanup)
            .await
            .is_err()
        {
            tracing::warn!(%session_id, "timed out cleaning expired auth session secondary state");
        }
        if let Err(error) = state.store.delete_key(&cleanup_marker_key).await {
            tracing::debug!(%error, %session_id, "failed to remove expired session cleanup marker");
        }
    });
}

pub(super) async fn has_preflight_whitelist_access(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<bool> {
    has_preflight_whitelist_access_from_sources(state, client_ip, None).await
}

pub(super) async fn has_preflight_whitelist_access_from_sources(
    state: &AppState,
    client_ip: &str,
    sources: Option<&[&str]>,
) -> anyhow::Result<bool> {
    let normalized_ip = http_utils::normalize_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(false);
    }
    if http_utils::is_private_or_local_ip(&normalized_ip) {
        return Ok(true);
    }

    let client_ip = normalized_ip.parse::<IpAddr>()?;
    Ok(whitelist::whitelist_snapshot_contains(
        state, client_ip, sources,
    ))
}

#[cfg(test)]
pub(super) fn whitelist_target_matches_ip(
    target: &str,
    target_type: &str,
    client_ip: IpAddr,
) -> bool {
    if target_type == "cidr" {
        return target
            .trim()
            .parse::<IpNet>()
            .is_ok_and(|network| network.contains(&client_ip));
    }

    http_utils::normalize_ip(target)
        .parse::<IpAddr>()
        .is_ok_and(|target_ip| target_ip == client_ip)
}

pub(super) fn inspect_auth_mobility_request(headers: &HeaderMap) -> AuthMobilityRequestIdentity {
    let forwarded_pathname = normalize_forwarded_pathname(
        headers
            .get("x-forwarded-path")
            .and_then(|value| value.to_str().ok()),
    );
    let cookie_header = headers
        .get(header::COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let session_id = parse_auth_mobility_cookie_value(cookie_header, cookies::SESSION_COOKIE_NAME);
    let fnos_token = parse_auth_mobility_cookie_value(cookie_header, "fnos-token");
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let app_binding = resolve_auth_mobility_app_binding(
        user_agent,
        &forwarded_pathname,
        cookie_header,
        &fnos_token,
    );
    let trim_media_token = if app_binding == Some("trim-media-app") {
        ["authorization", "accesstoken", "access-token"]
            .iter()
            .filter_map(|name| headers.get(*name).and_then(|value| value.to_str().ok()))
            .find_map(parse_auth_mobility_header_token)
    } else {
        None
    };
    AuthMobilityRequestIdentity {
        session_id,
        fnos_token,
        trim_media_token,
        app_binding,
    }
}

pub(super) fn parse_auth_mobility_cookie_value(cookie_header: &str, name: &str) -> Option<String> {
    let mut last_value = None;
    for segment in cookie_header.split(';') {
        let (raw_key, raw_value) = match segment.split_once('=') {
            Some(pair) => pair,
            None => continue,
        };
        if raw_key.trim() != name {
            continue;
        }
        let value = raw_value.trim().trim_matches('"');
        if value.is_empty() {
            continue;
        }
        last_value = Some(cookies::percent_decode(value));
    }
    last_value
}

pub(super) async fn has_resolvable_auth_mobility_access(
    state: &AppState,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<bool> {
    if let Some(token) = identity.fnos_token.as_deref()
        && let Some((_owner_id, owner_session)) =
            auth_mobility_binding_owner_session(state, "fnos-token", token).await?
        && auth_mobility_session_has_remaining_ttl(&owner_session)
    {
        return Ok(true);
    }
    if let Some(token) = identity.trim_media_token.as_deref()
        && let Some((_owner_id, owner_session)) =
            auth_mobility_binding_owner_session(state, "trim-media-token", token).await?
        && auth_mobility_session_has_remaining_ttl(&owner_session)
    {
        return Ok(true);
    }
    match identity.app_binding {
        Some("fnos-app") => Ok(list_auth_mobility_owner_sessions_by_ip(state, client_ip)
            .await?
            .len()
            == 1),
        Some("trim-media-app") => Ok(!list_auth_mobility_owner_sessions_by_ip(state, client_ip)
            .await?
            .is_empty()),
        _ => Ok(false),
    }
}

pub(super) async fn resolve_mobility_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<MobilitySubdomainAccessDecision> {
    let owners = resolve_auth_mobility_owner_sessions(state, client_ip, identity).await?;
    resolve_owner_sessions_subdomain_access(state, headers, uri, config, owners).await
}

async fn resolve_auto_ip_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    client_ip: &str,
) -> anyhow::Result<MobilitySubdomainAccessDecision> {
    let owners = list_auth_mobility_owner_sessions_by_ip(state, client_ip).await?;
    resolve_owner_sessions_subdomain_access(state, headers, uri, config, owners).await
}

async fn resolve_owner_sessions_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    owners: Vec<(String, LoginSession)>,
) -> anyhow::Result<MobilitySubdomainAccessDecision> {
    let host = normalize_subdomain_access_host(&resolve_request_subdomain_access_key(headers, uri));
    if host.is_empty() {
        return Ok(MobilitySubdomainAccessDecision {
            protected_host: false,
            has_owner_session: false,
            allowed: true,
            response_headers: Vec::new(),
        });
    }

    if owners.is_empty() {
        let protected_host = is_protected_subdomain_auth_host(&host, config);
        return Ok(MobilitySubdomainAccessDecision {
            protected_host,
            has_owner_session: false,
            allowed: !protected_host,
            response_headers: Vec::new(),
        });
    }

    let mut protected_host = false;
    let mut denied_response_headers = Vec::new();
    for (_owner_session_id, owner_session) in owners {
        let decision =
            resolve_session_subdomain_access(state, headers, uri, config, &owner_session).await?;
        protected_host |= decision.protected_host;
        if decision.protected_host && decision.allowed {
            return Ok(MobilitySubdomainAccessDecision {
                protected_host: true,
                has_owner_session: true,
                allowed: true,
                response_headers: decision.response_headers,
            });
        }
        if decision.protected_host && !decision.allowed && denied_response_headers.is_empty() {
            denied_response_headers = decision.response_headers;
        }
    }

    Ok(MobilitySubdomainAccessDecision {
        protected_host,
        has_owner_session: true,
        allowed: !protected_host,
        response_headers: denied_response_headers,
    })
}

pub(super) async fn resolve_auth_mobility_owner_sessions(
    state: &AppState,
    client_ip: &str,
    identity: &AuthMobilityRequestIdentity,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let mut owners = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(session_id) = identity.session_id.as_deref()
        && let Some(session) = state.store.get_session(session_id).await?
        && seen.insert(session_id.to_string())
    {
        owners.push((session_id.to_string(), session));
    }
    if let Some(token) = identity.fnos_token.as_deref()
        && let Some(owner) = auth_mobility_binding_owner_session(state, "fnos-token", token).await?
        && seen.insert(owner.0.clone())
    {
        owners.push(owner);
    }
    if let Some(token) = identity.trim_media_token.as_deref()
        && let Some(owner) =
            auth_mobility_binding_owner_session(state, "trim-media-token", token).await?
        && seen.insert(owner.0.clone())
    {
        owners.push(owner);
    }
    match identity.app_binding {
        Some("fnos-app") => {
            let sessions = list_auth_mobility_owner_sessions_by_ip(state, client_ip).await?;
            if sessions.len() == 1
                && let Some(owner) = sessions.into_iter().next()
                && seen.insert(owner.0.clone())
            {
                owners.push(owner);
            }
        }
        Some("trim-media-app") => {
            for owner in list_auth_mobility_owner_sessions_by_ip(state, client_ip).await? {
                if seen.insert(owner.0.clone()) {
                    owners.push(owner);
                }
            }
        }
        _ => {}
    }
    Ok(owners)
}

pub(super) async fn auth_mobility_binding_owner_session(
    state: &AppState,
    subject_type: &str,
    subject_key: &str,
) -> anyhow::Result<Option<(String, LoginSession)>> {
    let Some(binding) = state
        .store
        .get_auth_mobility_binding(subject_type, subject_key)
        .await?
    else {
        return Ok(None);
    };
    let owner_session_id = binding
        .get("ownerSessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(owner_session_id) = owner_session_id else {
        return Ok(None);
    };
    Ok(state
        .store
        .get_session(owner_session_id)
        .await?
        .map(|session| (owner_session_id.to_string(), session)))
}

pub(super) fn auth_mobility_session_has_remaining_ttl(session: &LoginSession) -> bool {
    let Some(expires_at) = session.expires_at.as_deref() else {
        return false;
    };
    let Some(expire_ms) = time_utils::parse_iso_ms(expires_at) else {
        return false;
    };
    expire_ms.div_euclid(1000) > time_utils::now_ms().div_euclid(1000)
}

pub(super) async fn list_auth_mobility_owner_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let normalized_ip = http_utils::normalize_ip(client_ip);
    let target_ip = if normalized_ip.is_empty() {
        client_ip.trim().to_string()
    } else {
        normalized_ip
    };
    if target_ip.is_empty() {
        return Ok(Vec::new());
    }

    let config = state.store.config_snapshot();
    let mut owners = Vec::new();
    for (session_id, session) in state.store.list_login_sessions().await? {
        let ips =
            auth_mobility::effective_session_ips(state, &session_id, &session, &config).await?;
        if ips.iter().any(|ip| ip == &target_ip) {
            owners.push((session_id, session));
        }
    }
    Ok(owners)
}

pub(super) fn normalize_forwarded_pathname(raw_path: Option<&str>) -> String {
    let value = raw_path.map(str::trim).unwrap_or("");
    if value.is_empty() {
        return String::new();
    }
    let base = url::Url::parse("http://localhost").ok();
    if let Some(base) = base
        && let Ok(parsed) = url::Url::options().base_url(Some(&base)).parse(value)
    {
        return parsed.path().to_string();
    }
    let pathname = value.split('?').next().unwrap_or("");
    if pathname.is_empty() {
        String::new()
    } else if pathname.starts_with('/') {
        pathname.to_string()
    } else {
        format!("/{pathname}")
    }
}

pub(super) fn resolve_auth_mobility_app_binding(
    user_agent: &str,
    forwarded_pathname: &str,
    cookie_header: &str,
    fnos_token: &Option<String>,
) -> Option<&'static str> {
    let normalized_user_agent = user_agent.trim().to_ascii_lowercase();
    if normalized_user_agent.contains("com.trim.media") {
        return Some("trim-media-app");
    }

    let is_fnos_app_user_agent = normalized_user_agent.contains("com.trim.app")
        || normalized_user_agent.contains("dart:io")
        || normalized_user_agent.contains("flutter/");
    let is_fnos_app_path = forwarded_pathname == "/trimcon" || forwarded_pathname == "/websocket";
    let has_relay_cookie = cookie_header.to_ascii_lowercase().contains("mode=relay");
    (is_fnos_app_path && (is_fnos_app_user_agent || has_relay_cookie || fnos_token.is_some()))
        .then_some("fnos-app")
}

pub(super) fn parse_auth_mobility_header_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("bearer ")
        .or_else(|| lower.strip_prefix("token "))
    {
        let start = trimmed.len() - rest.len();
        let token = trimmed[start..].trim();
        return (!token.is_empty()).then(|| token.to_string());
    }
    Some(trimmed.to_string())
}

pub(super) async fn resolve_session_subdomain_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    session: &LoginSession,
) -> anyhow::Result<SessionSubdomainAccessDecision> {
    let credential = session_auth_credential(state, session).await?;
    let host = resolve_request_subdomain_access_key(headers, uri);
    let normalized_host = normalize_subdomain_access_host(&host);
    let protected_host = is_protected_subdomain_auth_host(&normalized_host, config);
    let allowed = if !protected_host {
        true
    } else {
        credential.as_ref().is_some_and(|credential| {
            is_host_allowed_by_totp_subdomain_access(&credential.subdomain_access, &normalized_host)
        })
    };

    let mut response_headers = build_session_credential_response_headers(session);
    if let Some(credential) = credential.as_ref() {
        response_headers.extend(build_credential_subdomain_access_response_headers(
            credential,
        ));
    }

    Ok(SessionSubdomainAccessDecision {
        protected_host,
        allowed,
        response_headers,
    })
}

pub(super) async fn session_auth_credential(
    state: &AppState,
    session: &LoginSession,
) -> anyhow::Result<Option<TotpCredential>> {
    if AuthMethod::Password.matches_session_str(&session.method) {
        return Ok(state
            .store
            .get_auth_account(&session.credential_id)
            .await?
            .map(password_account_to_credential));
    }
    Ok(state
        .store
        .get_totps()
        .await?
        .into_iter()
        .find(|credential| credential.id == session.totp_id))
}

fn password_account_to_credential(account: crate::store::AuthAccount) -> TotpCredential {
    TotpCredential {
        id: account.source_totp_id,
        secret: String::new(),
        comment: if account.display_name.trim().is_empty() {
            account.username
        } else {
            account.display_name
        },
        created_at: account.created_at,
        access_scopes: crate::store::normalize_totp_access_scopes(account.access_scopes),
        subdomain_access: crate::store::normalize_totp_subdomain_access(account.subdomain_access),
    }
}

pub(super) fn resolve_request_subdomain_access_key(headers: &HeaderMap, uri: &Uri) -> String {
    let pathname = resolve_forwarded_request_pathname(headers, uri);
    if pathname == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
    }
    if is_auth_service_request_pathname(&pathname) {
        return String::new();
    }
    resolve_request_hostname(headers, uri)
}

pub(super) fn resolve_forwarded_request_pathname(headers: &HeaderMap, uri: &Uri) -> String {
    let raw_path = headers
        .get("x-forwarded-path")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uri.path().to_string());
    if raw_path.is_empty() {
        return String::new();
    }
    let base = url::Url::parse("https://fn-knock.internal").ok();
    if let Some(base) = base
        && let Ok(parsed) = url::Url::options().base_url(Some(&base)).parse(&raw_path)
    {
        return parsed.path().to_string();
    }
    raw_path.split(['?', '#']).next().unwrap_or("").to_string()
}

pub(super) fn is_auth_service_request_pathname(pathname: &str) -> bool {
    ["/__auth__", "/auth", "/api/auth"]
        .iter()
        .any(|prefix| pathname == *prefix || pathname.starts_with(&format!("{prefix}/")))
}

pub(super) fn resolve_request_hostname(headers: &HeaderMap, uri: &Uri) -> String {
    extract_hostname(
        parse_forwarded_header_host(headers)
            .or_else(|| first_header_value(headers, "x-forwarded-host"))
            .or_else(|| first_header_value(headers, "x-original-host"))
            .or_else(|| first_header_value(headers, "host"))
            .or_else(|| {
                uri.authority()
                    .map(|authority| authority.as_str().to_string())
            })
            .as_deref()
            .unwrap_or(""),
    )
}

pub(super) fn parse_forwarded_header_host(headers: &HeaderMap) -> Option<String> {
    crate::http_utils::forwarded_header_value(headers, "host")
}

pub(super) use crate::http_utils::first_header_value;

pub(super) fn extract_hostname(value: &str) -> String {
    normalize_subdomain_access_host(value)
}

pub(super) fn normalize_subdomain_access_host(value: &str) -> String {
    let mut host = value.trim().to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE || host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
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

pub(super) fn is_protected_subdomain_auth_host(host: &str, config: &Value) -> bool {
    if host.is_empty() {
        return false;
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE {
        return true;
    }
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| {
            mapping.get("service_role").and_then(Value::as_str) != Some("auth")
                && mapping.get("use_auth").and_then(Value::as_bool) == Some(true)
                && mapping
                    .get("host")
                    .and_then(Value::as_str)
                    .map(normalize_subdomain_access_host)
                    == Some(host.to_string())
        })
}

pub(super) fn is_host_allowed_by_totp_subdomain_access(access: &Value, host: &str) -> bool {
    let mode = access.get("mode").and_then(Value::as_str).unwrap_or("all");
    if mode != "custom" {
        return true;
    }
    !host.is_empty()
        && access
            .get("hosts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(normalize_subdomain_access_host)
            .any(|candidate| candidate == host)
}

pub(super) fn is_stream_allowed_by_totp_subdomain_access(
    access: &Value,
    protocol: &str,
    listen_port: i32,
) -> bool {
    let mode = access.get("mode").and_then(Value::as_str).unwrap_or("all");
    if mode != "custom" {
        return true;
    }
    let protocol = protocol.trim().to_ascii_lowercase();
    access
        .get("streams")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|stream| {
            stream
                .get("protocol")
                .and_then(Value::as_str)
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(&protocol))
                && stream.get("listen_port").and_then(Value::as_i64) == Some(i64::from(listen_port))
        })
}

pub(super) fn build_credential_subdomain_access_response_headers(
    credential: &TotpCredential,
) -> Vec<(String, String)> {
    let mode = credential
        .subdomain_access
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("all");
    if mode != "custom" {
        return Vec::new();
    }
    let hosts = credential
        .subdomain_access
        .get("hosts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_subdomain_access_host)
        .filter(|host| !host.is_empty() && host != TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE)
        .collect::<Vec<_>>()
        .join(",");
    vec![
        (
            REAUTH_SUBDOMAIN_ACCESS_HEADER.to_string(),
            REAUTH_SUBDOMAIN_ACCESS_CUSTOM.to_string(),
        ),
        (REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER.to_string(), hosts),
    ]
}

pub(super) fn build_session_credential_response_headers(
    session: &LoginSession,
) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_ID_HEADER,
        &session.credential_id,
    );
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_NAME_HEADER,
        if session.credential_name.trim().is_empty() {
            session.comment.as_deref().unwrap_or("")
        } else {
            &session.credential_name
        },
    );
    push_credential_header(
        &mut headers,
        REAUTH_CREDENTIAL_METHOD_HEADER,
        &session.method,
    );
    push_credential_header(&mut headers, REAUTH_LINKED_TOTP_ID_HEADER, &session.totp_id);
    push_credential_header(
        &mut headers,
        REAUTH_LINKED_TOTP_NAME_HEADER,
        session.linked_totp_name.as_deref().unwrap_or(""),
    );
    headers
}

pub(super) fn push_credential_header(headers: &mut Vec<(String, String)>, key: &str, value: &str) {
    let normalized = normalize_credential_header_value(value);
    if normalized.is_empty() {
        return;
    }
    headers.push((
        key.to_string(),
        format!(
            "{AUTH_IDENTITY_HEADER_ENCODING_PREFIX}{}",
            URL_SAFE_NO_PAD.encode(normalized.as_bytes())
        ),
    ));
}

pub(super) fn normalize_credential_header_value(value: &str) -> String {
    value
        .replace(['\r', '\n'], " ")
        .trim()
        .chars()
        .take(AUTH_IDENTITY_HEADER_MAX_LENGTH)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_account_credential_uses_current_account_permissions() {
        let credential = password_account_to_credential(crate::store::AuthAccount {
            id: "account-a".to_string(),
            username: "alice".to_string(),
            display_name: String::new(),
            source_totp_id: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-02T00:00:00Z".to_string(),
            access_scopes: json!(["docker_admin_panel", "other"]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": ["HTTPS://App.Example.com/path", "__builtin_select__"],
                "streams": [
                    { "protocol": "TCP", "listen_port": 2222 },
                    { "protocol": "udp", "listen_port": 0 }
                ]
            }),
        });

        assert_eq!(credential.id, "");
        assert_eq!(credential.comment, "alice");
        assert_eq!(credential.access_scopes, json!(["docker_admin_panel"]));
        assert_eq!(
            credential.subdomain_access,
            json!({
                "mode": "custom",
                "hosts": ["__builtin_select__", "app.example.com"],
                "streams": [{ "protocol": "tcp", "listen_port": 2222 }]
            })
        );
        assert!(is_stream_allowed_by_totp_subdomain_access(
            &credential.subdomain_access,
            "TCP",
            2222
        ));
        assert!(!is_stream_allowed_by_totp_subdomain_access(
            &credential.subdomain_access,
            "udp",
            2222
        ));
    }
}
