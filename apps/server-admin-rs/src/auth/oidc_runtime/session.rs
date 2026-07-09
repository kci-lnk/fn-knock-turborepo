use super::*;

pub(super) async fn bind_profile_and_resolve_login(
    state: &AppState,
    provider: Value,
    profile: ExternalProfile,
    subject_key: String,
    auth_state: Value,
    translator: &Translator,
) -> Result<CallbackResolved, String> {
    let invite_hash = auth_state
        .get("invite_token_hash")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "bindStateInvalid"))?;
    let invite = state
        .store
        .get_json_value(&format!("fn_knock:oidc:invite:{invite_hash}"))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "inviteExpired"))?;
    if let Some(invite_provider) = invite.get("provider_id").and_then(Value::as_str)
        && invite_provider != provider.get("id").and_then(Value::as_str).unwrap_or("")
    {
        return Err(oidc_text(translator, "bindProviderMismatch"));
    }
    let totp_id = invite
        .get("totp_id")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "inviteTotpMissing"))?;
    let totps = state
        .store
        .get_totps()
        .await
        .map_err(|error| error.to_string())?;
    if !totps.iter().any(|totp| totp.id == totp_id) {
        return Err(oidc_text(translator, "inviteTotpMissing"));
    }
    let existing = oidc_get_binding_by_subject(state, &subject_key)
        .await
        .map_err(|error| error.to_string())?;
    if let Some(existing) = existing.as_ref()
        && existing.get("totp_id").and_then(Value::as_str) != Some(totp_id)
    {
        return Err(oidc_text(translator, "accountAlreadyBoundOtherTotp"));
    }
    oidc_consume_invite(state, invite_hash)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| oidc_text(translator, "inviteUsed"))?;
    if let Some(mut binding) = existing {
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
        return Ok(CallbackResolved {
            state: auth_state,
            provider,
            binding,
            profile,
        });
    }
    let now = time_utils::now_iso();
    let binding = json!({
        "id": create_oidc_id("oidc_binding"),
        "provider_id": provider.get("id").and_then(Value::as_str).unwrap_or(""),
        "provider_type": provider.get("type").and_then(Value::as_str).unwrap_or("custom_oidc"),
        "totp_id": totp_id,
        "issuer": profile.issuer.clone(),
        "subject": profile.subject.clone(),
        "subject_key": subject_key,
        "display_name": profile.display_name.clone(),
        "email": profile.email.clone(),
        "email_verified": profile.email_verified,
        "avatar_url": profile.avatar_url.clone(),
        "created_at": now,
        "updated_at": now,
        "last_used_at": now
    });
    let saved = oidc_save_binding_if_subject_available(state, &binding)
        .await
        .map_err(|error| error.to_string())?;
    if !saved {
        return Err(oidc_text(translator, "accountAlreadyBoundOtherTotp"));
    }
    Ok(CallbackResolved {
        state: auth_state,
        provider,
        binding,
        profile,
    })
}

pub(super) async fn create_oidc_session_response(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    resolved: &CallbackResolved,
    _translator: &Translator,
    redirect_to: &str,
    flow_clear_cookie: Option<String>,
) -> anyhow::Result<Response> {
    let client_ip = client_ip_for_headers(headers);
    let provider_name = resolved
        .provider
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let credential_name = resolved
        .profile
        .display_name
        .as_deref()
        .or(resolved.profile.email.as_deref())
        .or(provider_name)
        .unwrap_or("External Account");
    let totp_id = resolved
        .binding
        .get("totp_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let totp_credentials = state.store.get_totps().await?;
    let totp_credential = totp_credentials
        .iter()
        .find(|totp| totp.id == totp_id)
        .cloned();
    let linked_totp_name = totp_credential
        .as_ref()
        .map(|totp| totp.comment.clone())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let credential_id = resolved
        .binding
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let remember_me = resolved
        .state
        .get("remember_me")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let created = auth_mobility::create_login_session(
        state,
        config,
        CreateLoginSessionInput {
            auth_method: AuthMethod::Oidc.as_session_str().to_string(),
            auth_provider_name: provider_name.map(str::to_string),
            credential_id,
            credential_name: credential_name.to_string(),
            totp_id: totp_id.to_string(),
            linked_totp_name,
            totp_credential,
            client_ip: client_ip.clone(),
            user_agent: user_agent(headers),
            remember_me,
        },
    )
    .await?;
    let tracking_ip = normalize_auth_failure_tracking_ip(&client_ip);
    if let Err(error) = state.store.reset_login_backoff(&tracking_ip).await {
        tracing::warn!(%error, %tracking_ip, "failed to reset OIDC login backoff");
    }
    let domain = resolve_cookie_domain(config, headers);
    let mut cookies = vec![cookies::session_cookie(
        &created.session_id,
        created.ttl_seconds,
        domain.as_deref(),
    )];
    if let Some(flow_clear_cookie) = flow_clear_cookie {
        cookies.push(flow_clear_cookie);
    }
    let final_redirect_to = crate::auth::effective_login_redirect(
        config,
        headers,
        &created.grant_type,
        Some(redirect_to),
    )
    .unwrap_or_else(|| "/".to_string());
    Ok(redirect_response(&final_redirect_to, cookies))
}
