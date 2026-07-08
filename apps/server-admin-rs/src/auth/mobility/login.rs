use super::*;

pub async fn create_login_session(
    state: &AppState,
    config: &Value,
    input: CreateLoginSessionInput,
) -> anyhow::Result<CreatedLoginSession> {
    let session_id = hex::encode(rand::random::<[u8; 16]>());
    let settings = AuthCredentialSettings::from_config(config);
    let ttl_seconds = if input.remember_me {
        settings.remember_me_ttl_seconds
    } else {
        settings.session_ttl_seconds
    };
    let now = now_seconds();
    let expire_at = now + ttl_seconds;
    let expires_at = time_utils::iso_after_seconds(ttl_seconds);
    let normalized_client_ip = normalized_or_trimmed_ip(&input.client_ip);
    let client_ip_for_session = if normalized_client_ip.is_empty() {
        "unknown".to_string()
    } else {
        normalized_client_ip.clone()
    };
    let ip_location = cached_ip_location(state, &normalized_client_ip).await;
    let auto_comment = auto_ip_grant_comment(config);
    let credential_restricted = input.totp_credential.as_ref().is_some_and(|credential| {
        is_totp_subdomain_access_restricted(&credential.subdomain_access)
    });
    let effective_post_login_mode = if credential_restricted {
        "disabled".to_string()
    } else {
        settings.post_login_ip_grant_mode.clone()
    };

    let mut whitelist_record_id = None::<String>;
    let mut session_comment = None::<String>;
    let mut grant_type = "browser_session".to_string();

    if !normalized_client_ip.is_empty() && effective_post_login_mode != "disabled" {
        let grant_expire_at = if effective_post_login_mode == "custom" {
            now + settings.post_login_ip_grant_ttl_seconds
        } else {
            expire_at
        };
        let record = if effective_post_login_mode == "follow_session"
            && settings.session_ip_mobility_enabled
        {
            whitelist::ensure_session_auto_whitelist(
                state,
                &format!("auth-mobility:login:{session_id}:{normalized_client_ip}"),
                &normalized_client_ip,
                Some(grant_expire_at),
                Some(auto_comment.clone()),
                None,
            )
            .await?
        } else {
            whitelist::add_auto_whitelist_record(
                state,
                &normalized_client_ip,
                Some(grant_expire_at),
                Some(auto_comment.clone()),
            )
            .await?
        };
        whitelist_record_id = Some(record.id.clone());
        session_comment = Some(
            normalize_auto_ip_grant_comment(record.comment.as_deref(), config)
                .unwrap_or(auto_comment.clone()),
        );
        grant_type = "login_ip_grant".to_string();
    }

    let session = LoginSession {
        totp_id: input.totp_id.clone(),
        method: input.auth_method.clone(),
        credential_id: input.credential_id.clone(),
        credential_name: input.credential_name.clone(),
        linked_totp_name: input.linked_totp_name.clone(),
        grant_type: Some(grant_type.clone()),
        post_login_ip_grant_mode: (grant_type == "login_ip_grant")
            .then(|| effective_post_login_mode.clone()),
        post_login_ip_grant_record_id: (grant_type == "login_ip_grant")
            .then(|| whitelist_record_id.clone())
            .flatten(),
        comment: session_comment.clone(),
        ip: client_ip_for_session.clone(),
        user_agent: input.user_agent.clone(),
        login_time: time_utils::now_iso(),
        expires_at: Some(expires_at.clone()),
        ip_location: ip_location.clone(),
    };
    state
        .store
        .add_session(&session_id, &session, ttl_seconds)
        .await?;

    if !normalized_client_ip.is_empty()
        && whitelist_record_id.is_some()
        && effective_post_login_mode == "follow_session"
    {
        register_login_session(
            state,
            &session_id,
            &normalized_client_ip,
            ip_location.as_deref(),
            whitelist_record_id.as_deref().unwrap_or_default(),
            Some(expire_at),
        )
        .await?;
    } else if !normalized_client_ip.is_empty() {
        record_browser_session_login(
            state,
            &session_id,
            &normalized_client_ip,
            ip_location.as_deref(),
        )
        .await?;
    }

    whitelist::sync_reverse_proxy_trusted_ips(state).await;
    if let Err(error) = system_events::publish_auth_login_success_event(
        state,
        json!({
            "session_id": session_id,
            "auth_method": input.auth_method,
            "auth_provider_name": input.auth_provider_name,
            "credential_id": input.credential_id,
            "credential_name": input.credential_name,
            "linked_totp_name": input.linked_totp_name,
            "session_comment": session_comment,
            "grant_type": grant_type,
            "post_login_ip_grant_mode": if grant_type == "login_ip_grant" {
                Some(effective_post_login_mode.clone())
            } else {
                None::<String>
            },
            "whitelist_record_id": whitelist_record_id,
            "ip": client_ip_for_session,
            "ip_location": ip_location,
            "user_agent": input.user_agent,
            "remember_me": input.remember_me,
            "expires_at": expires_at,
        }),
    )
    .await
    {
        tracing::warn!(%error, "failed to publish auth login success event");
    }

    Ok(CreatedLoginSession {
        session_id,
        ttl_seconds,
        grant_type,
        expires_at,
        whitelist_record_id,
        post_login_ip_grant_mode: (session.grant_type.as_deref() == Some("login_ip_grant"))
            .then(|| effective_post_login_mode),
        session_comment,
    })
}
