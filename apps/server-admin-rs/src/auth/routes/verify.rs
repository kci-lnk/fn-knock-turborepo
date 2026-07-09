use super::*;

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
    if access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
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
    let config = state.store.get_config().await?;
    let locale = config
        .get("locale")
        .cloned()
        .unwrap_or_else(|| json!({ "default_locale": "zh-CN" }));
    let translator = translator_from_config(&config);
    let appearance = config
        .get("appearance")
        .cloned()
        .unwrap_or_else(|| json!({ "theme_color_preset": "default" }));
    let access = resolve_auth_access(state, headers, uri, &translator).await?;
    let client_ip = client_ip_for_auth(headers);
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
        "captcha": public_captcha_settings_from_config(state, &config, &translator),
        "passkey": passkey,
        "oidc": { "providers": oidc_providers }
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

pub(super) async fn resolve_auth_access(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    translator: &Translator,
) -> anyhow::Result<AuthAccess> {
    let client_ip = client_ip_for_auth(headers);
    let config = state.store.get_config().await?;
    let access_mode = requested_access_mode(headers);
    let normal_access =
        resolve_preflight_normal_access(state, headers, uri, &config, &client_ip, access_mode)
            .await?;
    if normal_access.authorized {
        let identity = inspect_auth_mobility_request(headers);
        if let Err(error) = auth_mobility::sync_trusted_request(
            state,
            &client_ip,
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
        if let Err(error) =
            common_auth_locations::record_recent_verified_ip(state, &client_ip).await
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
            set_cookies: Vec::new(),
            response_headers: normal_access.response_headers,
        });
    }
    if normal_access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
        let mut response_headers = normal_access.response_headers;
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
            set_cookies: Vec::new(),
            response_headers,
        });
    }

    let share_access = fnos_share_bypass::authorize(state, headers, uri, &config).await?;
    if share_access.authorized {
        return Ok(AuthAccess {
            authenticated: true,
            message: auth_route_text(translator, "authenticated"),
            grant_type: Some("fnos_share".to_string()),
            deny_reason: None,
            set_cookies: share_access.set_cookies,
            response_headers: share_access.response_headers,
        });
    }
    if !share_access.set_cookies.is_empty() || !share_access.response_headers.is_empty() {
        return Ok(AuthAccess {
            authenticated: false,
            message: auth_route_text(translator, "authenticationRequired"),
            grant_type: None,
            deny_reason: None,
            set_cookies: share_access.set_cookies,
            response_headers: share_access.response_headers,
        });
    }

    Ok(AuthAccess {
        authenticated: false,
        message: auth_route_text(translator, "authenticationRequired"),
        grant_type: None,
        deny_reason: None,
        set_cookies: Vec::new(),
        response_headers: Vec::new(),
    })
}

pub(super) async fn public_captcha_settings(state: &AppState) -> anyhow::Result<Value> {
    let config = state.store.get_config().await?;
    let translator = translator_from_config(&config);
    Ok(public_captcha_settings_from_config(
        state,
        &config,
        &translator,
    ))
}

pub(super) fn public_captcha_settings_from_config(
    state: &AppState,
    config: &Value,
    translator: &Translator,
) -> Value {
    let captcha = config.get("captcha").cloned().unwrap_or_else(|| {
        json!({
            "provider": "pow",
            "widget_mode": "normal",
            "pow": {},
            "turnstile": { "site_key": "", "secret_key": "" }
        })
    });
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
