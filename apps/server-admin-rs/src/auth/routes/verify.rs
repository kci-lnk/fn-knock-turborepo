use super::*;
use crate::auth::routes::subdomain_grant;
use crate::grpc_proto::SubdomainRuleMatch;

pub(super) async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match resolve_auth_access(&state, &headers, &uri, &translator).await {
        Ok(access) if access.authenticated => {
            let mut response = with_auth_headers(
                response::success_message(access.message.clone()).into_response(),
            );
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Ok(access) => {
            let status = auth_verify_denied_status(&access);
            let mut response = with_auth_headers(response::error(status, access.message.clone()));
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "auth verify failed");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "verifyFailed"),
            ))
        }
    }
}

pub(super) fn auth_verify_denied_status(access: &AuthAccess) -> StatusCode {
    if access.deny_reason.as_deref() == Some(subdomain_grant::RATE_LIMITED_ERROR) {
        StatusCode::TOO_MANY_REQUESTS
    } else if access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::UNAUTHORIZED
    }
}

pub(super) async fn build_auth_shell_data(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    redirect_uri: Option<&str>,
    include_redirect: bool,
) -> anyhow::Result<(Value, AuthAccess)> {
    let config = state.store.config_snapshot();
    let captcha_settings = runtime_config::load_captcha_settings(state).await?;
    let locale = config
        .get("locale")
        .cloned()
        .unwrap_or_else(|| json!({ "default_locale": "zh-CN" }));
    let translator = translator_from_config(&config);
    let appearance = config
        .get("appearance")
        .cloned()
        .unwrap_or_else(|| json!({ "theme_color_preset": "default" }));
    let auth_shell_headers =
        cookies::without_cookie(headers, cookies::SUBDOMAIN_RULE_GRANT_COOKIE_NAME);
    let mut access = resolve_auth_access(state, &auth_shell_headers, uri, &translator).await?;
    append_shared_session_cookie_for_auth_shell(state, &auth_shell_headers, &config, &mut access)
        .await?;
    let client_ip = client_ip_for_auth(&auth_shell_headers);
    let login_mode = state
        .store
        .get_auth_login_mode()
        .await
        .unwrap_or(AuthLoginMode::Totp);
    let oidc_providers = if login_mode.allows_totp_family() {
        oidc_public_providers(state).await.unwrap_or_default()
    } else {
        Vec::new()
    };
    let ldap_providers = if login_mode.allows_totp_family() {
        crate::ldap_auth::ldap_public_providers(state)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let passkey = public_passkey_status(state, headers, &config).await;
    let mut data = json!({
        "locale": locale,
        "appearance": appearance,
        "auth": {
            "authenticated": access.authenticated,
            "message": access.message,
            "grant_type": access.grant_type,
            "login_mode": login_mode.as_str()
        },
        "client": { "ip": client_ip },
        "captcha": public_captcha_settings_from_settings(state, &captcha_settings, &translator),
        "passkey": passkey,
        "oidc": { "providers": oidc_providers },
        "ldap": { "providers": ldap_providers }
    });

    if include_redirect {
        let redirect_to = if access.authenticated {
            effective_login_redirect(
                &config,
                headers,
                access.grant_type.as_deref().unwrap_or_default(),
                redirect_uri,
            )
        } else {
            resolve_shared_auth_login_redirect(&config, headers, redirect_uri)
        };
        if let Some(value) = redirect_to {
            data["redirect_to"] = Value::String(value);
        }
    }
    Ok((data, access))
}

#[derive(Debug)]
pub(super) struct AuthAccess {
    pub(super) authenticated: bool,
    pub(super) message: String,
    pub(super) grant_type: Option<String>,
    pub(super) deny_reason: Option<String>,
    pub(super) set_cookies: Vec<String>,
    pub(super) response_headers: Vec<(String, String)>,
}

async fn append_shared_session_cookie_for_auth_shell(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    access: &mut AuthAccess,
) -> anyhow::Result<()> {
    if !access.authenticated {
        return Ok(());
    }
    let Some(current_hostname) = resolve_request_hostname_from_headers(headers) else {
        return Ok(());
    };
    let Some(public_auth_url) =
        resolve_public_auth_base_url(config).and_then(|value| url::Url::parse(&value).ok())
    else {
        return Ok(());
    };
    if public_auth_url
        .host_str()
        .map(normalize_subdomain_access_host)
        .as_deref()
        != Some(current_hostname.as_str())
    {
        return Ok(());
    }

    let identity = inspect_auth_mobility_request(headers);
    let Some(session_id) = identity.session_id.as_deref() else {
        return Ok(());
    };
    let Some(session) = state.store.get_session(session_id).await? else {
        return Ok(());
    };
    let Some(expires_at) = session
        .expires_at
        .as_deref()
        .and_then(time_utils::parse_iso_ms)
    else {
        // Legacy sessions without a parseable absolute expiry remain valid,
        // but cannot be safely migrated with a client-side lifetime.
        return Ok(());
    };
    let remaining_ms = expires_at - time_utils::now_ms();
    if remaining_ms <= 0 {
        return Ok(());
    }
    let Some(cookie_domain) = resolve_cookie_domain(config, headers) else {
        return Ok(());
    };
    let max_age = remaining_ms.saturating_add(999).div_euclid(1000).max(1);
    access.set_cookies.push(cookies::session_cookie(
        session_id,
        max_age,
        Some(&cookie_domain),
    ));
    Ok(())
}

pub(super) async fn resolve_auth_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
) -> anyhow::Result<AuthAccess> {
    resolve_auth_access_with_routed_upstream(state, headers, uri, translator, None, None, None)
        .await
}

pub(super) async fn resolve_auth_access_with_routed_upstream(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<AuthAccess> {
    let config = state.store.config_snapshot();
    resolve_auth_access_with_routed_upstream_and_config(
        state,
        headers,
        uri,
        translator,
        config.as_ref(),
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn resolve_auth_access_with_routed_upstream_and_config(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
    config: &Value,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<AuthAccess> {
    let client_ip = client_ip_for_auth(headers);
    let access_mode = requested_access_mode(headers);
    let normal_access =
        resolve_preflight_normal_access(state, headers, uri, config, &client_ip, access_mode)
            .await?;

    resolve_auth_access_with_normal_access_and_rule_match(
        state,
        headers,
        uri,
        translator,
        config,
        &client_ip,
        &normal_access,
        None,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn resolve_auth_access_with_normal_access_and_rule_match(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
    config: &Value,
    client_ip: &str,
    normal_access: &PreflightNormalAccess,
    matched: Option<&SubdomainRuleMatch>,
    routed_upstream: Option<&str>,
    routed_upstream_host: Option<&str>,
    routed_upstream_route_id: Option<&str>,
) -> anyhow::Result<AuthAccess> {
    let invalid_session_cookies = if normal_access.invalid_session_cookie {
        resolve_cookie_clear_domains(Some(config), headers)
            .into_iter()
            .map(|domain| cookies::session_clear_cookie(domain.as_deref()))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if normal_access.authorized
        && normal_access
            .grant_type
            .as_deref()
            .is_some_and(is_ip_or_mobility_grant)
    {
        let grant = match subdomain_grant::authorize(state, headers, config, matched).await {
            Err(error) if subdomain_grant::is_rate_limited(&error) => {
                return Ok(rate_limited_access(invalid_session_cookies));
            }
            result => result?,
        };
        if let Some(grant) = grant {
            let mut set_cookies = invalid_session_cookies;
            if let Some(cookie) = grant.set_cookie.clone() {
                set_cookies.push(cookie);
            }
            return Ok(AuthAccess {
                authenticated: true,
                message: auth_route_text(translator, "authenticated"),
                grant_type: Some("subdomain_rule".to_string()),
                deny_reason: None,
                set_cookies,
                response_headers: rule_grant_headers(headers, &grant),
            });
        }
    }

    if normal_access.authorized {
        let identity = inspect_auth_mobility_request(headers);
        if let Err(error) = auth_mobility::sync_trusted_request(
            state,
            client_ip,
            auth_mobility::AuthMobilityRestoreIdentity {
                session_id: identity.session_id.as_deref(),
                fnos_token: identity.fnos_token.as_deref(),
                trim_media_token: identity.trim_media_token.as_deref(),
                app_binding: identity.app_binding,
            },
        )
        .await
        {
            tracing::warn!(%error, %client_ip, "failed to sync trusted auth mobility request");
        }
        if let Err(error) = common_auth_locations::record_recent_verified_ip(state, client_ip).await
        {
            tracing::debug!(%error, %client_ip, "failed to record recent verified auth IP");
        }
        let grant_type = normal_access.grant_type.clone();
        let message = match grant_type.as_deref() {
            Some("local_exempt") => auth_route_text(translator, "localNetworkAccessAllowed"),
            _ => auth_route_text(translator, "authenticated"),
        };
        return Ok(AuthAccess {
            authenticated: true,
            message,
            grant_type,
            deny_reason: None,
            set_cookies: invalid_session_cookies,
            response_headers: normal_access.response_headers.clone(),
        });
    }
    if normal_access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        let grant = match subdomain_grant::authorize(state, headers, config, matched).await {
            Err(error) if subdomain_grant::is_rate_limited(&error) => {
                return Ok(rate_limited_access(invalid_session_cookies));
            }
            result => result?,
        };
        if let Some(grant) = grant {
            let mut set_cookies = invalid_session_cookies;
            if let Some(cookie) = grant.set_cookie.clone() {
                set_cookies.push(cookie);
            }
            return Ok(AuthAccess {
                authenticated: true,
                message: auth_route_text(translator, "authenticated"),
                grant_type: Some("subdomain_rule_login".to_string()),
                deny_reason: None,
                set_cookies,
                response_headers: rule_grant_headers(headers, &grant),
            });
        }
        let mut response_headers = normal_access.response_headers.clone();
        if !response_headers
            .iter()
            .any(|(key, _)| key.eq_ignore_ascii_case(REAUTH_ACCESS_DENIED_HEADER))
        {
            response_headers.push((
                REAUTH_ACCESS_DENIED_HEADER.to_string(),
                REAUTH_SCOPE_DENIED.to_string(),
            ));
        }
        return Ok(AuthAccess {
            authenticated: false,
            message: "Access denied by credential scope".to_string(),
            grant_type: None,
            deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
            set_cookies: invalid_session_cookies,
            response_headers,
        });
    }

    let share_access = fnos_share_bypass::authorize(
        state,
        headers,
        uri,
        config,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await?;
    if share_access.authorized {
        let mut set_cookies = invalid_session_cookies;
        set_cookies.extend(share_access.set_cookies);
        return Ok(AuthAccess {
            authenticated: true,
            message: auth_route_text(translator, "authenticated"),
            grant_type: Some("fnos_share".to_string()),
            deny_reason: None,
            set_cookies,
            response_headers: share_access.response_headers,
        });
    }
    // A validated subdomain rule is an explicit, host-scoped auth decision.
    // It must be considered before an unrelated share-flow redirect, while a
    // successfully authorized share session above retains its existing
    // priority.
    let grant = match subdomain_grant::authorize(state, headers, config, matched).await {
        Err(error) if subdomain_grant::is_rate_limited(&error) => {
            return Ok(rate_limited_access(invalid_session_cookies));
        }
        result => result?,
    };
    if let Some(grant) = grant {
        let mut set_cookies = invalid_session_cookies;
        if let Some(cookie) = grant.set_cookie.clone() {
            set_cookies.push(cookie);
        }
        return Ok(AuthAccess {
            authenticated: true,
            message: auth_route_text(translator, "authenticated"),
            grant_type: Some("subdomain_rule".to_string()),
            deny_reason: None,
            set_cookies,
            response_headers: rule_grant_headers(headers, &grant),
        });
    }
    if !share_access.set_cookies.is_empty() || !share_access.response_headers.is_empty() {
        let mut set_cookies = invalid_session_cookies;
        set_cookies.extend(share_access.set_cookies);
        return Ok(AuthAccess {
            authenticated: false,
            message: auth_route_text(translator, "authenticationRequired"),
            grant_type: None,
            deny_reason: None,
            set_cookies,
            response_headers: share_access.response_headers,
        });
    }

    Ok(AuthAccess {
        authenticated: false,
        message: auth_route_text(translator, "authenticationRequired"),
        grant_type: None,
        deny_reason: None,
        set_cookies: invalid_session_cookies,
        response_headers: Vec::new(),
    })
}

pub(super) fn is_ip_or_mobility_grant(grant_type: &str) -> bool {
    matches!(
        grant_type,
        "login_ip_grant" | "fnos_fingerprint_session" | "session_migration"
    )
}

fn rule_grant_headers(
    request_headers: &HeaderMap,
    grant: &subdomain_grant::GrantAccess,
) -> Vec<(String, String)> {
    let host = resolve_request_hostname_from_headers(request_headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty());
    let mut headers = vec![
        (
            "X-Reauth-Auth-Rule-Group".to_string(),
            grant.group_id.clone(),
        ),
        (
            "X-Reauth-Auth-Grant-State".to_string(),
            grant.state.to_string(),
        ),
        (
            "X-Reauth-Auth-Cache-Max-Age".to_string(),
            grant.cache_max_age_seconds.to_string(),
        ),
        (
            "X-Reauth-Access-Mode".to_string(),
            "subdomain-rule".to_string(),
        ),
    ];
    // Keep the internal host picker from advertising unrelated subdomains
    // when a host-only temporary grant reaches /__select__.  This is a UI
    // scope restriction; the gateway/Rust host checks remain authoritative.
    if let Some(host) = host {
        headers.push((
            "X-Reauth-Subdomain-Access".to_string(),
            "custom".to_string(),
        ));
        headers.push(("X-Reauth-Allowed-Subdomain-Hosts".to_string(), host));
    }
    headers
}

fn rate_limited_access(set_cookies: Vec<String>) -> AuthAccess {
    AuthAccess {
        authenticated: false,
        message: "Too many temporary credentials requested".to_string(),
        grant_type: None,
        deny_reason: Some(subdomain_grant::RATE_LIMITED_ERROR.to_string()),
        set_cookies,
        response_headers: vec![("Retry-After".to_string(), "60".to_string())],
    }
}

pub(super) async fn public_captcha_settings(state: &AppState) -> anyhow::Result<Value> {
    let config = state.store.config_snapshot();
    let settings = runtime_config::load_captcha_settings(state).await?;
    let translator = translator_from_config(&config);
    Ok(public_captcha_settings_from_settings(
        state,
        &settings,
        &translator,
    ))
}

pub(super) fn public_captcha_settings_from_settings(
    state: &AppState,
    captcha: &Value,
    translator: &Translator,
) -> Value {
    let provider = captcha
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("pow");
    let site_key = captcha
        .pointer("/turnstile/site_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    let turnstile_secret = captcha
        .pointer("/turnstile/secret_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (available, unavailable_reason) = match provider {
        "pow"
            if state
                .settings
                .altcha_hmac_key
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .is_none() =>
        {
            (
                false,
                Some(captcha_text(translator, "powServerNotConfigured")),
            )
        }
        "turnstile" if site_key.trim().is_empty() || turnstile_secret.trim().is_empty() => (
            false,
            Some(captcha_text(translator, "turnstileNotConfigured")),
        ),
        "pow" | "turnstile" => (true, None),
        _ => (false, Some(captcha_text(translator, "providerUnavailable"))),
    };

    json!({
        "provider": provider,
        "widget_mode": "normal",
        "available": available,
        "unavailable_reason": unavailable_reason,
        "pow": {},
        "turnstile": { "site_key": site_key }
    })
}
