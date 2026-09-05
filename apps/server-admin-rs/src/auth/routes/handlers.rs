use super::*;

pub(super) async fn auth_api_not_found() -> Response {
    let translator = Translator::new(crate::i18n::DEFAULT_LOCALE);
    response::error(
        StatusCode::NOT_FOUND,
        auth_route_text(&translator, "pathNotFound"),
    )
}

pub(super) async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<BootstrapQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let client_ip = client_ip_for_auth(&headers);
    enqueue_auth_ip_location(&state, &client_ip, "bootstrap");
    match build_auth_shell_data(&state, &headers, &uri, query.redirect_uri.as_deref(), true).await {
        Ok((mut data, access)) => {
            let mut clear_cookie = None;
            if let Ok(config) = state.storage.store.get_config().await
                && let Some((message, cookie)) =
                    consume_login_error_for_bootstrap(&state, &headers, &uri, &config).await
            {
                if let Some(oidc) = data.get_mut("oidc").and_then(Value::as_object_mut) {
                    oidc.insert("login_error".to_string(), Value::String(message));
                }
                clear_cookie = Some(cookie);
            }
            let mut response = with_auth_headers(response::ok(data).into_response());
            apply_auth_access_response_headers(response.headers_mut(), &access);
            if let Some(cookie) = clear_cookie
                && let Ok(value) = HeaderValue::from_str(&cookie)
            {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
            response
        }
        Err(error) => {
            tracing::warn!(%error, "failed to build auth bootstrap data");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadBootstrapFailed"),
            ))
        }
    }
}

pub(super) async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match build_auth_shell_data(&state, &headers, &uri, None, false).await {
        Ok((data, access)) => {
            if access.authenticated {
                let client_ip = client_ip_for_auth(&headers);
                enqueue_auth_ip_location(&state, &client_ip, "session");
            }
            let status = if access.authenticated {
                StatusCode::OK
            } else {
                StatusCode::UNAUTHORIZED
            };
            let mut response = with_auth_headers(
                (
                    status,
                    Json(ApiEnvelope {
                        success: access.authenticated,
                        code: None,
                        message: if access.authenticated {
                            None
                        } else {
                            Some(auth_route_text(&translator, "authenticationRequired"))
                        },
                        data: if access.authenticated {
                            Some(data)
                        } else {
                            None
                        },
                    }),
                )
                    .into_response(),
            );
            apply_auth_access_response_headers(response.headers_mut(), &access);
            response
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load auth session");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadSessionFailed"),
            ))
        }
    }
}

pub(super) async fn captcha_config(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match public_captcha_settings(&state).await {
        Ok(data) => with_auth_headers(response::ok(data).into_response()),
        Err(error) => {
            tracing::warn!(%error, "failed to load captcha config");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadCaptchaConfigFailed"),
            ))
        }
    }
}

pub(super) async fn challenge(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let translator = Translator::from_state(&state).await;
    let settings = match runtime_config::load_captcha_settings(&state).await {
        Ok(settings) => settings,
        Err(error) => {
            tracing::warn!(%error, "failed to load captcha settings for challenge");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "createCaptchaChallengeFailed"),
            ));
        }
    };
    if settings
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or("pow")
        != "pow"
    {
        return with_auth_headers(response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            captcha_text(&translator, "powNotEnabled"),
        ));
    }
    let Some(key) = state
        .settings
        .altcha_hmac_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return with_auth_headers(response::error(
            StatusCode::SERVICE_UNAVAILABLE,
            captcha_text(&translator, "powServerNotConfigured"),
        ));
    };

    let salt = hex::encode(random_bytes::<12>());
    let expires = time_utils::now_ms() / 1000 + 300;
    let salt_with_params = format!("{salt}?expires={expires}");
    let max_number = pow_max_number_for_request(&state, &settings, &headers).await;
    let secret_number = pow_secret_number_from_random(rand::random::<u32>(), max_number);
    let challenge = sha256_hex(format!("{salt_with_params}{secret_number}").as_bytes());
    let signature = hmac_sha256_hex(key.as_bytes(), challenge.as_bytes());

    with_auth_headers(
        Json(json!({
            "algorithm": "SHA-256",
            "challenge": challenge,
            "maxnumber": max_number,
            "salt": salt_with_params,
            "signature": signature
        }))
        .into_response(),
    )
}

pub(super) async fn ip(headers: HeaderMap) -> Response {
    with_auth_headers(response::ok(json!({ "ip": client_ip_for_auth(&headers) })).into_response())
}

pub(super) async fn ip_location(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let ip = client_ip_for_auth(&headers);
    match ip_location::ensure_ip_location_enqueued(&state, &ip).await {
        Ok(snapshot) => {
            let mut data = json!({
                "ip": ip,
                "location": snapshot.get("location").cloned().unwrap_or_else(|| Value::String(String::new())),
                "status": snapshot.get("status").cloned().unwrap_or_else(|| Value::String("skipped".to_string())),
                "attempts": snapshot.get("attempts").cloned().unwrap_or_else(|| json!(0)),
                "maxAttempts": snapshot.get("maxAttempts").cloned().unwrap_or_else(|| json!(0))
            });
            if let Some(error) = snapshot.get("error") {
                data["error"] = error.clone();
            }
            with_auth_headers(response::ok(data).into_response())
        }
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to enqueue auth IP location lookup");
            let translator = Translator::from_state(&state).await;
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                translator.t("server.ipLocationRoutes.enqueueFailed"),
            ))
        }
    }
}

pub(super) async fn oidc_providers(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match oidc_public_providers(&state).await {
        Ok(providers) => {
            with_auth_headers(response::ok(json!({ "providers": providers })).into_response())
        }
        Err(error) => {
            tracing::warn!(%error, "failed to load public OIDC providers");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadOidcProvidersFailed"),
            ))
        }
    }
}

pub(super) async fn oidc_client_metadata(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    Query(query): Query<OidcClientMetadataQuery>,
) -> Response {
    let Some(provider_id) = query
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|id| !id.is_empty())
    else {
        return response::error(StatusCode::BAD_REQUEST, "provider_id is required");
    };
    let provider = match oidc_get_provider(&state, provider_id).await {
        Ok(Some(provider))
            if provider.get("type").and_then(Value::as_str) == Some("fnknock_qq")
                && provider.get("enabled").and_then(Value::as_bool) == Some(true) =>
        {
            provider
        }
        _ => return response::error(StatusCode::NOT_FOUND, "QQ OIDC provider not found"),
    };
    let client_id = provider
        .pointer("/connection_config/client_id")
        .and_then(Value::as_str)
        .unwrap_or("fnknock-qq-public");
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(_) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load auth config",
            );
        }
    };
    let Some(base_url) = callback_base_url(&headers, &uri, &config) else {
        return response::error(
            StatusCode::BAD_REQUEST,
            "Unable to determine public auth URL",
        );
    };
    let callback_url = format!(
        "{}/api/auth/oidc/callback/{}",
        base_url.trim_end_matches('/'),
        crate::http_utils::url_encode_component(provider_id),
    );
    let mut response = Json(json!({
        "client_id": client_id,
        "redirect_uris": [callback_url],
        "software_id": "fn-knock",
    }))
    .into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

pub(super) async fn oidc_invite(
    State(state): State<AppState>,
    Query(query): Query<OidcInviteQuery>,
) -> Response {
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load config for OIDC invite");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadOidcInviteFailed"),
            ));
        }
    };
    let translator = translator_from_config(&config);
    let locale = config
        .get("locale")
        .cloned()
        .unwrap_or_else(|| json!({ "default_locale": "zh-CN" }));
    let appearance = config
        .get("appearance")
        .cloned()
        .unwrap_or_else(|| json!({ "theme_color_preset": "default" }));
    let Some(token) = query
        .token
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return with_auth_headers(
            (
                StatusCode::BAD_REQUEST,
                Json(ApiEnvelope {
                    success: false,
                    code: None,
                    message: Some(oidc_text(&translator, "inviteInvalid")),
                    data: Some(json!({ "locale": locale, "appearance": appearance })),
                }),
            )
                .into_response(),
        );
    };

    match oidc_inspect_invite(&state, token).await {
        Ok(Some(mut invite)) => {
            if let Some(object) = invite.as_object_mut() {
                object.insert("locale".to_string(), locale);
                object.insert("appearance".to_string(), appearance);
            }
            with_auth_headers(response::ok(invite).into_response())
        }
        Ok(None) => with_auth_headers(
            (
                StatusCode::NOT_FOUND,
                Json(ApiEnvelope {
                    success: false,
                    code: None,
                    message: Some(oidc_text(&translator, "inviteMissingExpiredUsed")),
                    data: Some(json!({ "locale": locale, "appearance": appearance })),
                }),
            )
                .into_response(),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to inspect OIDC invite");
            with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "inspectOidcInviteFailed"),
            ))
        }
    }
}

#[axum::debug_handler(state = AppState)]
pub(super) async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<LoginBody>,
) -> Response {
    let client_ip = client_ip_for_auth(&headers);
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config during login");
            let translator = Translator::from_state(&state).await;
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadAuthConfigFailed"),
            ));
        }
    };
    let translator = translator_from_config(&config);

    match state
        .storage
        .store
        .get_login_backoff_status(&tracking_ip)
        .await
    {
        Ok(status) if status.blocked => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            return with_auth_headers(backoff_login_response(
                &server_text_params(
                    &translator,
                    "tooManyAttemptsWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            ));
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to inspect auth login backoff");
        }
    }

    if let Err(message) = verify_captcha(&state, &body.captcha, &client_ip, &translator).await {
        return with_auth_headers(response::error(StatusCode::BAD_REQUEST, message));
    }

    let login_mode = match state.storage.store.get_auth_login_mode().await {
        Ok(mode) => mode,
        Err(error) => {
            tracing::warn!(%error, "failed to load auth login mode");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadLoginCredentialsFailed"),
            ));
        }
    };
    let Some(method) = login_method(&body) else {
        return with_auth_headers(response::error(
            StatusCode::BAD_REQUEST,
            auth_route_text(&translator, "loginMethodUnavailable"),
        ));
    };
    if method == AuthMethod::Ldap {
        if login_mode != AuthLoginMode::Totp {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                auth_route_text(&translator, "loginMethodUnavailable"),
            ));
        }
        return with_auth_headers(
            crate::ldap_auth::login(
                &state,
                &headers,
                &config,
                &translator,
                body.provider_id.as_deref().unwrap_or_default(),
                body.username.as_deref().unwrap_or_default(),
                body.password.as_deref().unwrap_or_default(),
                body.remember_me,
                body.redirect_uri.as_deref(),
                &client_ip,
                &tracking_ip,
            )
            .await,
        );
    }
    if method == AuthMethod::Password {
        if login_mode != AuthLoginMode::Password {
            return with_auth_headers(response::error(
                StatusCode::BAD_REQUEST,
                auth_route_text(&translator, "loginMethodUnavailable"),
            ));
        }
        return password_login(
            &state,
            &headers,
            &config,
            &translator,
            &body,
            &client_ip,
            &tracking_ip,
        )
        .await;
    }
    if login_mode != AuthLoginMode::Totp {
        return with_auth_headers(response::error(
            StatusCode::BAD_REQUEST,
            auth_route_text(&translator, "loginMethodUnavailable"),
        ));
    }
    let token = body.token.as_deref().unwrap_or("").trim();

    let totps = match state.storage.store.get_totps().await {
        Ok(totps) => totps,
        Err(error) => {
            tracing::warn!(%error, "failed to load TOTP credentials");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "loadLoginCredentialsFailed"),
            ));
        }
    };
    if totps.is_empty() {
        return with_auth_headers(response::error(
            StatusCode::BAD_REQUEST,
            server_text(&translator, "loginCredentialMissing"),
        ));
    }

    let Some(credential) = find_matching_totp(&totps, token) else {
        match state
            .storage
            .store
            .register_login_backoff_failure(&tracking_ip)
            .await
        {
            Ok(status) => {
                let retry_after = status.retry_after.unwrap_or(1).max(1);
                if let Err(error) = system_events::publish_auth_login_failure_event(
                    &state,
                    json!({
                        "ip": tracking_ip.clone(),
                        "attempts": status.attempts,
                        "retry_after_seconds": retry_after,
                        "blocked_until": status.blocked_until.map(time_utils::iso_from_ms),
                        "method": AuthMethod::Totp.as_session_str(),
                        "credential_name": "! Unknown TOTP",
                        "user_agent": user_agent(&headers),
                    }),
                )
                .await
                {
                    tracing::warn!(%error, %tracking_ip, "failed to publish auth login failure event");
                }
                return with_auth_headers(backoff_login_response(
                    &server_text_params(
                        &translator,
                        "invalidOtpWithRetry",
                        &[("seconds", retry_after.to_string())],
                    ),
                    retry_after,
                    status.blocked_until,
                ));
            }
            Err(error) => {
                tracing::warn!(%error, %tracking_ip, "failed to register auth login failure");
                return with_auth_headers(response::error(
                    StatusCode::TOO_MANY_REQUESTS,
                    server_text_params(
                        &translator,
                        "invalidOtpWithRetry",
                        &[("seconds", "1".to_string())],
                    ),
                ));
            }
        }
    };

    let credential_name = credential_name(&credential, &translator);
    let passkey_info = if config
        .pointer("/auth_credential_settings/passkey_bind_prompt_enabled")
        .and_then(Value::as_bool)
        == Some(false)
    {
        None
    } else {
        match build_passkey_bind_info(&state, &credential.id).await {
            Ok(value) => Some(value),
            Err(error) => {
                tracing::warn!(%error, totp_id = %credential.id, "failed to build passkey bind info");
                return with_auth_headers(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    auth_route_text(&translator, "createSessionFailed"),
                ));
            }
        }
    };
    let created = match auth_mobility::create_login_session(
        &state,
        &config,
        CreateLoginSessionInput {
            auth_method: AuthMethod::Totp.as_session_str().to_string(),
            auth_provider_name: None,
            credential_id: credential.id.clone(),
            credential_name: credential_name.clone(),
            totp_id: credential.id.clone(),
            linked_totp_name: None,
            totp_credential: Some(credential.clone()),
            client_ip: client_ip.clone(),
            user_agent: user_agent(&headers),
            remember_me: body.remember_me,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to create auth session");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(&translator, "createSessionFailed"),
            ));
        }
    };
    tracing::debug!(
        session_id = %created.session_id,
        grant_type = %created.grant_type,
        whitelist_record_id = ?created.whitelist_record_id,
        post_login_ip_grant_mode = ?created.post_login_ip_grant_mode,
        expires_at = %created.expires_at,
        session_comment = ?created.session_comment,
        "created auth session"
    );
    if created.ttl_seconds <= 0 {
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            auth_route_text(&translator, "createSessionFailed"),
        ));
    }
    if let Err(error) = state.storage.store.reset_login_backoff(&tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset auth login backoff after success");
    }

    let redirect_to = effective_login_redirect(
        &config,
        &headers,
        &created.grant_type,
        body.redirect_uri.as_deref(),
    );
    let cookie_domain = resolve_cookie_domain(&config, &headers);
    let cookie = cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        cookie_domain.as_deref(),
    );
    let cookie_header = match login_session_cookie_header(
        &state,
        &created.session_id,
        &cookie,
        &translator,
    )
    .await
    {
        Ok(value) => value,
        Err(response) => return response,
    };
    let mut data = json!({
        "run_type": config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        "grant_type": created.grant_type
    });
    if let Some(mut passkey_info) = passkey_info {
        if let Some(object) = passkey_info.as_object_mut() {
            object.remove("token");
        }
        data["passkey"] = passkey_info;
    }
    if let Some(redirect_to) = redirect_to {
        data["redirect_to"] = Value::String(redirect_to);
    }
    let mut response = (
        [(header::SET_COOKIE, cookie_header)],
        Json(ApiEnvelope {
            success: true,
            code: None,
            message: Some(auth_route_text(&translator, "loginSuccessful")),
            data: Some(data),
        }),
    )
        .into_response();
    apply_no_store_headers(response.headers_mut());
    response
}

fn login_method(body: &LoginBody) -> Option<AuthMethod> {
    if let Some(explicit) = body
        .method
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return AuthMethod::from_login_request(explicit);
    }
    if body
        .username
        .as_deref()
        .is_some_and(|value| !value.trim().is_empty())
        || body
            .password
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    {
        return Some(AuthMethod::Password);
    }
    Some(AuthMethod::Totp)
}

async fn login_session_cookie_header(
    state: &AppState,
    session_id: &str,
    cookie: &str,
    translator: &Translator,
) -> Result<HeaderValue, Response> {
    match HeaderValue::from_str(cookie) {
        Ok(value) => Ok(value),
        Err(error) => {
            tracing::warn!(%error, %session_id, "failed to build auth session cookie header");
            if let Err(error) = auth_mobility::destroy_session(state, session_id).await {
                tracing::warn!(%error, %session_id, "failed to destroy auth session after cookie header failure");
            }
            Err(with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(translator, "createSessionFailed"),
            )))
        }
    }
}

#[cfg(test)]
tokio::task_local! {
    static PASSWORD_LOGIN_HASH_BARRIER: (std::sync::Arc<tokio::sync::Notify>, std::sync::Arc<tokio::sync::Notify>);
}

async fn password_login(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
    body: &LoginBody,
    client_ip: &str,
    tracking_ip: &str,
) -> Response {
    let username = body.username.as_deref().unwrap_or("").trim();
    let password = body.password.as_deref().unwrap_or("");
    // Account creation accepts at most 128 UTF-8 bytes. Reject oversized login
    // input uniformly before account lookup or a real/dummy hash allocation.
    if password.len() > crate::auth::password::MAX_AUTH_PASSWORD_BYTES {
        return register_password_login_failure(state, headers, tracking_ip, translator).await;
    }
    let account = if username.is_empty() || password.is_empty() {
        None
    } else {
        match state
            .storage
            .store
            .get_auth_account_by_username(username)
            .await
        {
            Ok(account) => account,
            Err(error) => {
                tracing::warn!(%error, "failed to load auth account for password login");
                return with_auth_headers(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    auth_route_text(translator, "loadLoginCredentialsFailed"),
                ));
            }
        }
    };
    let mut verified_credential = None;
    let verified = if let Some(account) = account.as_ref() {
        match state
            .storage
            .store
            .get_auth_password_credential(&account.id)
            .await
        {
            Ok(Some(record)) if record.account_id == account.id => {
                #[cfg(test)]
                if let Ok((hashing, resume)) = PASSWORD_LOGIN_HASH_BARRIER.try_with(Clone::clone) {
                    hashing.notify_one();
                    resume.notified().await;
                }
                match crate::auth::password::verify_auth_password(password, &record).await {
                    Ok(value) => {
                        if value {
                            verified_credential = Some(record);
                        }
                        Ok(value)
                    }
                    Err(error) if crate::auth::password::is_password_hash_busy(&error) => {
                        Err(error)
                    }
                    Err(error) => {
                        tracing::warn!(%error, account_id = %account.id, "failed to verify auth account password");
                        consume_dummy_auth_password_hash(password)
                            .await
                            .map(|()| false)
                    }
                }
            }
            Ok(Some(_)) | Ok(None) => consume_dummy_auth_password_hash(password)
                .await
                .map(|()| false),
            Err(error) => {
                tracing::warn!(%error, account_id = %account.id, "failed to load auth account password credential");
                return with_auth_headers(response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    auth_route_text(translator, "loadLoginCredentialsFailed"),
                ));
            }
        }
    } else {
        consume_dummy_auth_password_hash(password)
            .await
            .map(|()| false)
    };
    let verified = match verified {
        Ok(verified) => verified,
        Err(error) => {
            // Real and unknown accounts share the same bounded hash service.
            // Overload must not turn into an account-dependent failure/backoff.
            tracing::warn!(%error, "password login hashing unavailable");
            let mut response = with_auth_headers(response::error(
                StatusCode::SERVICE_UNAVAILABLE,
                auth_route_text(translator, "verifyFailed"),
            ));
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from_static("3"));
            return response;
        }
    };
    let Some(account) = account.filter(|_| verified) else {
        return register_password_login_failure(state, headers, tracking_ip, translator).await;
    };

    // Password verification can wait in the hash pool while a management
    // request changes/deletes this account or switches login mode. Serialize
    // only session publication, then validate against the current credential.
    let _mutation = state.storage.store.auth_account_mutation_lock.lock().await;
    let account = match revalidate_password_login(
        state,
        &account,
        verified_credential
            .as_ref()
            .expect("verified password has a credential"),
    )
    .await
    {
        Ok(Some(account)) => account,
        Ok(None) => {
            return register_password_login_failure(state, headers, tracking_ip, translator).await;
        }
        Err(error) => {
            tracing::warn!(%error, account_id = %account.id, "failed to revalidate password login before session publication");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(translator, "loadLoginCredentialsFailed"),
            ));
        }
    };

    let totp_credential = match state.storage.store.get_totps().await {
        Ok(totps) => totps
            .into_iter()
            .find(|credential| credential.id == account.source_totp_id),
        Err(error) => {
            tracing::warn!(%error, account_id = %account.id, "failed to load source TOTP for password login");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(translator, "loadLoginCredentialsFailed"),
            ));
        }
    };
    let account_name = if account.display_name.trim().is_empty() {
        account.username.clone()
    } else {
        account.display_name.clone()
    };
    let (linked_totp_name, session_totp_credential, passkey_info) =
        if let Some(mut credential) = totp_credential {
            credential.comment = account_name.clone();
            credential.access_scopes = account.access_scopes.clone();
            credential.subdomain_access = account.subdomain_access.clone();
            let linked_totp_name = Some(credential_name(&credential, translator));
            (linked_totp_name, Some(credential), None::<Value>)
        } else {
            (
                None,
                Some(TotpCredential {
                    id: account.source_totp_id.clone(),
                    secret: String::new(),
                    comment: account_name.clone(),
                    created_at: String::new(),
                    access_scopes: account.access_scopes.clone(),
                    subdomain_access: account.subdomain_access.clone(),
                }),
                None::<Value>,
            )
        };
    let created = match auth_mobility::create_login_session(
        state,
        config,
        CreateLoginSessionInput {
            auth_method: AuthMethod::Password.as_session_str().to_string(),
            auth_provider_name: None,
            credential_id: account.id.clone(),
            credential_name: account_name.clone(),
            totp_id: account.source_totp_id.clone(),
            linked_totp_name,
            totp_credential: session_totp_credential,
            client_ip: client_ip.to_string(),
            user_agent: user_agent(headers),
            remember_me: body.remember_me,
        },
    )
    .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, account_id = %account.id, "failed to create password login session");
            return with_auth_headers(response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                auth_route_text(translator, "createSessionFailed"),
            ));
        }
    };
    if created.ttl_seconds <= 0 {
        return with_auth_headers(response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            auth_route_text(translator, "createSessionFailed"),
        ));
    }
    if let Err(error) = state.storage.store.reset_login_backoff(tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset password login backoff after success");
    }

    let redirect_to = effective_login_redirect(
        config,
        headers,
        &created.grant_type,
        body.redirect_uri.as_deref(),
    );
    let cookie_domain = resolve_cookie_domain(config, headers);
    let cookie = cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        cookie_domain.as_deref(),
    );
    let cookie_header =
        match login_session_cookie_header(state, &created.session_id, &cookie, translator).await {
            Ok(value) => value,
            Err(response) => return response,
        };
    let mut data = json!({
        "run_type": config.get("run_type").and_then(Value::as_i64).unwrap_or(3),
        "grant_type": created.grant_type
    });
    if let Some(mut passkey_info) = passkey_info {
        if let Some(object) = passkey_info.as_object_mut() {
            object.remove("token");
        }
        data["passkey"] = passkey_info;
    }
    if let Some(redirect_to) = redirect_to {
        data["redirect_to"] = Value::String(redirect_to);
    }
    let mut response = (
        [(header::SET_COOKIE, cookie_header)],
        Json(ApiEnvelope {
            success: true,
            code: None,
            message: Some(auth_route_text(translator, "loginSuccessful")),
            data: Some(data),
        }),
    )
        .into_response();
    apply_no_store_headers(response.headers_mut());
    response
}

/// Called while the account mutation lock is held, and the caller retains it
/// until session creation completes. Use current account permissions, never
/// the projection read before the expensive password verification.
async fn revalidate_password_login(
    state: &AppState,
    looked_up_account: &crate::store::AuthAccount,
    verified: &crate::store::AuthPasswordCredential,
) -> anyhow::Result<Option<crate::store::AuthAccount>> {
    if verified.account_id != looked_up_account.id
        || state.storage.store.get_auth_login_mode().await? != AuthLoginMode::Password
    {
        return Ok(None);
    }
    let snapshot = state
        .storage
        .store
        .get_auth_account_mutation_snapshot(&looked_up_account.id, None)
        .await?;
    if serde_json::to_value(snapshot.password.as_ref())? != serde_json::to_value(Some(verified))? {
        return Ok(None);
    }
    Ok(snapshot.accounts.into_iter().find(|account| {
        account.id == looked_up_account.id
            && account
                .username
                .eq_ignore_ascii_case(&looked_up_account.username)
    }))
}

async fn consume_dummy_auth_password_hash(password: &str) -> anyhow::Result<()> {
    crate::auth::password::consume_dummy_auth_password_hash(password).await
}

async fn register_password_login_failure(
    state: &AppState,
    headers: &HeaderMap,
    tracking_ip: &str,
    translator: &Translator,
) -> Response {
    match state
        .storage
        .store
        .register_login_backoff_failure(tracking_ip)
        .await
    {
        Ok(status) => {
            let retry_after = status.retry_after.unwrap_or(1).max(1);
            if let Err(error) = system_events::publish_auth_login_failure_event(
                state,
                json!({
                    "ip": tracking_ip,
                    "attempts": status.attempts,
                    "retry_after_seconds": retry_after,
                    "blocked_until": status.blocked_until.map(time_utils::iso_from_ms),
                    "method": AuthMethod::Password.as_session_str(),
                    "credential_name": "! Unknown Account",
                    "user_agent": user_agent(headers),
                }),
            )
            .await
            {
                tracing::warn!(%error, %tracking_ip, "failed to publish password login failure event");
            }
            with_auth_headers(backoff_login_response(
                &server_text_params(
                    translator,
                    "invalidPasswordWithRetry",
                    &[("seconds", retry_after.to_string())],
                ),
                retry_after,
                status.blocked_until,
            ))
        }
        Err(error) => {
            tracing::warn!(%error, %tracking_ip, "failed to register password login failure");
            with_auth_headers(response::error(
                StatusCode::TOO_MANY_REQUESTS,
                server_text_params(
                    translator,
                    "invalidPasswordWithRetry",
                    &[("seconds", "1".to_string())],
                ),
            ))
        }
    }
}

pub(crate) fn backoff_login_response(
    message: &str,
    retry_after: i64,
    blocked_until: Option<i64>,
) -> Response {
    let retry_after = retry_after.max(1);
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(json!({
            "success": false,
            "message": message,
            "retryAfter": retry_after,
            "blockedUntil": blocked_until
        })),
    )
        .into_response();
    response.headers_mut().insert(
        header::RETRY_AFTER,
        HeaderValue::from_str(&retry_after.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("1")),
    );
    apply_no_store_headers(response.headers_mut());
    response
}

pub(super) async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
) -> Response {
    let mut grant_revoke_failed =
        if let Err(error) = super::subdomain_grant::revoke(&state, &headers).await {
            tracing::warn!(%error, "failed to revoke subdomain rule grant on logout");
            true
        } else {
            false
        };
    let config = match state.storage.store.get_config().await {
        Ok(config) => Some(config),
        Err(error) => {
            tracing::warn!(%error, "failed to load config for logout");
            None
        }
    };
    let identity = inspect_auth_mobility_request(&headers);
    let session_id = identity.session_id;
    let client_ip = client_ip_for_auth(&headers);
    if let Some(session_id) = session_id.as_deref() {
        let outcome = auth_mobility::revoke_login_session(
            &state,
            session_id,
            config.as_ref(),
            &client_ip,
            "user_logout",
        )
        .await;
        if !outcome.complete {
            grant_revoke_failed = true;
        }
    } else {
        if let Err(error) =
            whitelist::remove_whitelist_records_by_ip(&state, &client_ip, Some("auto")).await
        {
            tracing::warn!(%error, %client_ip, "failed to remove auto whitelist records on logout without session");
            grant_revoke_failed = true;
        }
        if let Err(error) = whitelist::sync_reverse_proxy_trusted_ips_required(&state).await {
            tracing::warn!(%error, "failed to confirm gateway trust revocation on logout");
            grant_revoke_failed = true;
        }
    }

    let cookie_domains = resolve_cookie_clear_domains(config.as_ref(), &headers);
    let mut response_builder = Response::builder().status(if grant_revoke_failed {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::FOUND
    });
    if !grant_revoke_failed {
        response_builder =
            response_builder.header(header::LOCATION, post_logout_location(&headers, &uri));
    }
    let mut response = response_builder
        .body(axum::body::Body::empty())
        .unwrap_or_else(|_| Response::new(axum::body::Body::empty()));
    apply_no_store_headers(response.headers_mut());
    for domain in &cookie_domains {
        append_set_cookie_header(
            response.headers_mut(),
            cookies::session_clear_cookie(domain.as_deref()),
            "session clear cookie",
        );
        append_set_cookie_header(
            response.headers_mut(),
            cookies::fnos_share_clear_cookie(domain.as_deref()),
            "share clear cookie",
        );
        append_set_cookie_header(
            response.headers_mut(),
            cookies::fnos_share_access_code_clear_cookie(domain.as_deref()),
            "share access-code clear cookie",
        );
    }
    if cookies::read_cookie(&headers, super::subdomain_grant::COOKIE_NAME).is_some() {
        append_set_cookie_header(
            response.headers_mut(),
            super::subdomain_grant::clear_cookie(),
            "subdomain rule grant clear cookie",
        );
    }
    response
}

fn append_set_cookie_header(headers: &mut HeaderMap, cookie: String, context: &'static str) {
    match HeaderValue::from_str(&cookie) {
        Ok(value) => {
            headers.append(header::SET_COOKIE, value);
        }
        Err(error) => {
            tracing::warn!(%error, %context, "failed to build Set-Cookie header");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn password_test_account() -> crate::store::AuthAccount {
        crate::store::AuthAccount {
            id: "password-account".to_string(),
            username: "alice".to_string(),
            display_name: "alice".to_string(),
            source_totp_id: String::new(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            access_scopes: json!([]),
            subdomain_access: json!({"mode":"all", "hosts":[]}),
        }
    }

    #[tokio::test]
    async fn password_login_hash_wait_rejects_changed_password_account_and_mode() {
        let (_directory, state) =
            super::super::tests::auth_route_test_state("password-hash-revalidation").await;
        let account = password_test_account();
        let credential = crate::auth::password::make_auth_password_credential(
            &account.id,
            "original-password123",
            None,
        )
        .await
        .unwrap();
        for changed in ["password", "delete", "rename", "mode"] {
            state
                .storage
                .store
                .set_auth_accounts(std::slice::from_ref(&account))
                .await
                .unwrap();
            state
                .storage
                .store
                .set_auth_password_credential(&credential)
                .await
                .unwrap();
            state
                .storage
                .store
                .set_auth_login_mode(AuthLoginMode::Password)
                .await
                .unwrap();
            state
                .storage
                .store
                .reset_login_backoff("203.0.113.10")
                .await
                .unwrap();
            let hashing = std::sync::Arc::new(tokio::sync::Notify::new());
            let resume = std::sync::Arc::new(tokio::sync::Notify::new());
            let body = login_body(
                Some("password"),
                Some("alice"),
                Some("original-password123"),
            );
            let headers = HeaderMap::new();
            let config = state.storage.store.get_config().await.unwrap();
            let translator = Translator::from_state(&state).await;
            let login = PASSWORD_LOGIN_HASH_BARRIER.scope(
                (hashing.clone(), resume.clone()),
                password_login(
                    &state,
                    &headers,
                    &config,
                    &translator,
                    &body,
                    "203.0.113.10",
                    "203.0.113.10",
                ),
            );
            let mutation = async {
                hashing.notified().await;
                {
                    let _guard = state.storage.store.auth_account_mutation_lock.lock().await;
                    match changed {
                        "password" => {
                            let mut replacement = credential.clone();
                            replacement.hash = "changed-password".to_string();
                            state
                                .storage
                                .store
                                .set_auth_password_credential(&replacement)
                                .await
                                .unwrap();
                        }
                        "delete" => {
                            state.storage.store.set_auth_accounts(&[]).await.unwrap();
                        }
                        "rename" => {
                            let mut replacement = account.clone();
                            replacement.username = "renamed".to_string();
                            state
                                .storage
                                .store
                                .set_auth_accounts(&[replacement])
                                .await
                                .unwrap();
                        }
                        "mode" => {
                            state
                                .storage
                                .store
                                .set_auth_login_mode(AuthLoginMode::Totp)
                                .await
                                .unwrap();
                        }
                        _ => unreachable!(),
                    }
                }
                resume.notify_one();
            };
            let (response, ()) = tokio::time::timeout(std::time::Duration::from_secs(30), async {
                tokio::join!(login, mutation)
            })
            .await
            .unwrap();
            assert!(
                !response.status().is_success(),
                "accepted concurrent {changed}"
            );
            assert!(response.headers().get(header::SET_COOKIE).is_none());
            assert!(
                state
                    .storage
                    .store
                    .list_login_sessions()
                    .await
                    .unwrap()
                    .is_empty()
            );
        }
    }

    #[tokio::test]
    async fn password_login_revalidation_uses_current_permissions_and_checks_credential_identity() {
        let (_directory, state) =
            super::super::tests::auth_route_test_state("password-current-permissions").await;
        let account = password_test_account();
        let credential = crate::auth::password::make_auth_password_credential(
            &account.id,
            "original-password123",
            None,
        )
        .await
        .unwrap();
        state
            .storage
            .store
            .set_auth_accounts(std::slice::from_ref(&account))
            .await
            .unwrap();
        state
            .storage
            .store
            .set_auth_password_credential(&credential)
            .await
            .unwrap();
        state
            .storage
            .store
            .set_auth_login_mode(AuthLoginMode::Password)
            .await
            .unwrap();
        let _guard = state.storage.store.auth_account_mutation_lock.lock().await;
        let mut updated = account.clone();
        updated.access_scopes = json!(["docker_admin_panel"]);
        updated.subdomain_access = crate::store::normalize_totp_subdomain_access(
            json!({"mode":"custom", "hosts":["app.example.com"]}),
        );
        state
            .storage
            .store
            .set_auth_accounts(std::slice::from_ref(&updated))
            .await
            .unwrap();
        let current = revalidate_password_login(&state, &account, &credential)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(current.access_scopes, updated.access_scopes);
        assert_eq!(current.subdomain_access, updated.subdomain_access);
        let mut mismatched = credential.clone();
        mismatched.account_id = "other-id".to_string();
        assert!(
            revalidate_password_login(&state, &account, &mismatched)
                .await
                .unwrap()
                .is_none()
        );
    }

    fn login_body(
        method: Option<&str>,
        username: Option<&str>,
        password: Option<&str>,
    ) -> LoginBody {
        LoginBody {
            method: method.map(str::to_string),
            provider_id: None,
            token: None,
            username: username.map(str::to_string),
            password: password.map(str::to_string),
            captcha: CaptchaSubmission::Pow {
                proof: String::new(),
            },
            remember_me: false,
            redirect_uri: None,
        }
    }

    #[test]
    fn login_method_rejects_invalid_explicit_method() {
        let body = login_body(Some("magic"), Some("admin"), Some("password"));

        assert_eq!(login_method(&body), None);
    }

    #[test]
    fn login_method_honors_explicit_totp_over_password_fields() {
        let body = login_body(Some("totp"), Some("admin"), Some("password"));

        assert_eq!(login_method(&body), Some(AuthMethod::Totp));
    }

    #[test]
    fn login_method_infers_password_only_without_explicit_method() {
        let body = login_body(None, Some("admin"), Some("password"));

        assert_eq!(login_method(&body), Some(AuthMethod::Password));
    }
}
