use super::*;

pub(super) async fn build_authorization_url(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
    provider_id: &str,
    mode: &str,
    redirect_uri: Option<&str>,
    invite_token: Option<&str>,
    remember_me: bool,
) -> Result<AuthorizationBuild, String> {
    ensure_oidc_login_mode(state, translator).await?;
    let provider = oidc_get_provider(state, provider_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "providerUnavailable"))?;
    if provider.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err(oidc_text(translator, "providerUnavailable"));
    }
    oidc_provider_ready_with_translator(&provider, translator)?;
    let mut invite_token_hash = None;
    if mode == "bind" {
        let token = invite_token
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| oidc_text(translator, "inviteInvalid"))?;
        let invite = oidc_inspect_invite(state, token)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| oidc_text(translator, "inviteExpired"))?;
        if let Some(invite_provider_id) = invite.get("provider_id").and_then(Value::as_str)
            && invite_provider_id != provider_id
        {
            return Err(oidc_text(translator, "inviteProviderNotAllowed"));
        }
        invite_token_hash = Some(hash_oidc_token(token));
    }

    let callback_url = build_callback_url(provider_id, headers, uri, config, translator)?;
    let state_token = create_public_token();
    let state_hash = hash_oidc_token(&state_token);
    let protocol = provider
        .get("protocol")
        .and_then(Value::as_str)
        .unwrap_or("oidc");
    let nonce = (protocol == "oidc").then(create_public_token);
    let code_verifier = (protocol == "oidc").then(create_pkce_verifier);
    let safe_redirect_uri = crate::auth::safe_redirect(config, headers, redirect_uri);
    let mut auth_state = Map::new();
    auth_state.insert("state_hash".to_string(), Value::String(state_hash.clone()));
    auth_state.insert("mode".to_string(), Value::String(mode.to_string()));
    auth_state.insert(
        "provider_id".to_string(),
        Value::String(provider_id.to_string()),
    );
    if let Some(redirect_uri) = safe_redirect_uri {
        auth_state.insert("redirect_uri".to_string(), Value::String(redirect_uri));
    }
    if let Some(invite_token_hash) = invite_token_hash {
        auth_state.insert(
            "invite_token_hash".to_string(),
            Value::String(invite_token_hash),
        );
    }
    if let Some(code_verifier) = code_verifier.as_deref() {
        auth_state.insert(
            "code_verifier".to_string(),
            Value::String(code_verifier.to_string()),
        );
    }
    if let Some(nonce) = nonce.as_deref() {
        auth_state.insert("nonce".to_string(), Value::String(nonce.to_string()));
    }
    auth_state.insert("remember_me".to_string(), Value::Bool(remember_me));
    let client_ip = client_ip_for_headers(headers);
    if !client_ip.is_empty() {
        auth_state.insert("client_ip".to_string(), Value::String(client_ip));
    }
    auth_state.insert(
        "created_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    auth_state.insert(
        "expires_at".to_string(),
        Value::String(time_utils::iso_after_seconds(OIDC_STATE_TTL_SECONDS as i64)),
    );
    oidc_save_state(state, &Value::Object(auth_state), OIDC_STATE_TTL_SECONDS)
        .await
        .map_err(|error| error.to_string())?;

    let authorization_url = if protocol == "oidc" {
        build_standard_oidc_authorization_url(
            &provider,
            &callback_url,
            &state_token,
            nonce.as_deref().unwrap_or(""),
            code_verifier.as_deref().unwrap_or(""),
            translator,
        )
        .await?
    } else {
        build_oauth_profile_authorization_url(&provider, &callback_url, &state_token, translator)?
    };
    Ok(AuthorizationBuild {
        authorization_url,
        flow_token: state_hash,
        max_age: OIDC_STATE_TTL_SECONDS,
    })
}

pub(super) async fn resolve_callback(
    state: &AppState,
    headers: &HeaderMap,
    uri: &Uri,
    config: &Value,
    translator: &Translator,
    provider_id: &str,
    code: &str,
    state_token: &str,
    flow_token: Option<&str>,
) -> Result<CallbackResolved, String> {
    if !oidc_flow_token_valid(state_token, flow_token) {
        return Err(oidc_text(translator, "callbackStateExpired"));
    }
    let state_hash = hash_oidc_token(state_token);
    let auth_state = oidc_consume_state(state, &state_hash)
        .await
        .map_err(|error| error.to_string())?
        .filter(|value| value.get("provider_id").and_then(Value::as_str) == Some(provider_id))
        .ok_or_else(|| oidc_text(translator, "callbackStateExpired"))?;
    ensure_oidc_login_mode(state, translator).await?;
    let provider = oidc_get_provider(state, provider_id)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "providerUnavailable"))?;
    if provider.get("enabled").and_then(Value::as_bool) != Some(true) {
        return Err(oidc_text(translator, "providerUnavailable"));
    }
    let callback_url = build_callback_url(provider_id, headers, uri, config, translator)?;
    let profile = if provider.get("protocol").and_then(Value::as_str) == Some("oauth2_profile") {
        resolve_oauth_profile_callback(state, &provider, code, &callback_url, translator).await?
    } else {
        resolve_standard_oidc_callback(
            state,
            &provider,
            code,
            &callback_url,
            &auth_state,
            translator,
        )
        .await?
    };
    let subject_key = build_subject_key(provider_id, &profile.issuer, &profile.subject);
    if auth_state.get("mode").and_then(Value::as_str) == Some("bind") {
        return bind_profile_and_resolve_login(
            state,
            provider,
            profile,
            subject_key,
            auth_state,
            translator,
        )
        .await;
    }
    let Some(mut binding) = oidc_get_binding_by_subject(state, &subject_key)
        .await
        .map_err(|error| error.to_string())?
    else {
        return Err(oidc_text(translator, "accountNotBoundCannotLogin"));
    };
    update_binding_profile_fields(&mut binding, &profile);
    if let Some(object) = binding.as_object_mut() {
        object.insert(
            "last_used_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
    }
    oidc_save_binding(state, &binding)
        .await
        .map_err(|error| error.to_string())?;
    Ok(CallbackResolved {
        state: auth_state,
        provider,
        binding,
        profile,
    })
}
