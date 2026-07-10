use super::*;

async fn publish_login_auto_whitelist(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
    expire_at: i64,
    comment: String,
    whitelist_record_id: &str,
) -> anyhow::Result<()> {
    let Some(lease) = acquire_auth_mobility_session_mutation_lease(state, session_id).await? else {
        anyhow::bail!("Timed out waiting for login whitelist publication lock");
    };
    let owner_key = format!("auth-mobility:login:{session_id}:{client_ip}");
    let owner_record_key = whitelist::whitelist_auto_owner_record_key(&owner_key);
    let ttl_seconds = (expire_at - now_seconds()).max(1);
    let result = async {
        if state.store.get_session(session_id).await?.is_none() || !lease.ensure_valid().await? {
            anyhow::bail!("Login session was revoked before whitelist publication");
        }
        if !state
            .store
            .add_auth_mobility_pending_whitelist(
                session_id,
                whitelist_record_id,
                &owner_record_key,
                ttl_seconds,
            )
            .await?
        {
            anyhow::bail!("Login session was revoked before whitelist publication");
        }
        let deferred = whitelist::ensure_pending_session_auto_whitelist(
            state,
            &owner_key,
            client_ip,
            Some(expire_at),
            Some(comment),
            None,
            whitelist_record_id,
        )
        .await?;
        if deferred.record.id != whitelist_record_id {
            anyhow::bail!("Login whitelist owner resolved to an unexpected record");
        }
        if !lease.ensure_valid().await? {
            anyhow::bail!("Login whitelist publication lease was lost");
        }
        whitelist::publish_deferred_session_auto_whitelist(state, deferred).await?;
        if !lease.ensure_valid().await? || state.store.get_session(session_id).await?.is_none() {
            anyhow::bail!("Login session was revoked during whitelist publication");
        }
        // Keep this entry as the logout-enumerable owner index. Active-IP
        // grants move to the detail index; standalone/custom login grants do
        // not have another crash-safe reverse mapping.
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = whitelist::rollback_session_auto_whitelist(state, &owner_key, whitelist_record_id)
            .await;
        let _ = state
            .store
            .remove_auth_mobility_pending_whitelist(session_id, whitelist_record_id)
            .await;
    }
    if let Err(error) = lease.release().await {
        tracing::warn!(%error, %session_id, "failed to release login whitelist publication lock");
    }
    result
}

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
    let mut post_login_grant_expire_at = None::<i64>;
    let mut session_comment = None::<String>;
    let mut grant_type = "browser_session".to_string();
    let session_access_scopes = input.totp_credential.as_ref().map(|credential| {
        crate::store::normalize_totp_access_scopes(credential.access_scopes.clone())
    });
    let session_subdomain_access = input.totp_credential.as_ref().map(|credential| {
        crate::store::normalize_totp_subdomain_access(credential.subdomain_access.clone())
    });

    if !normalized_client_ip.is_empty() && effective_post_login_mode != "disabled" {
        let grant_expire_at = if effective_post_login_mode == "custom" {
            now + settings.post_login_ip_grant_ttl_seconds
        } else {
            expire_at
        };
        post_login_grant_expire_at = Some(grant_expire_at);
        if effective_post_login_mode == "follow_session" && settings.session_ip_mobility_enabled {
            // The live-session active-IP transaction creates this grant as a
            // pending record after Session exists. Publishing it here would
            // leave an orphan if login initialization is cancelled or revoked.
        } else {
            whitelist_record_id = Some(format!("whitelist:{}", uuid::Uuid::new_v4()));
        }
        session_comment = Some(auto_comment.clone());
        grant_type = "login_ip_grant".to_string();
    }

    let session = LoginSession {
        totp_id: input.totp_id.clone(),
        method: input.auth_method.clone(),
        credential_id: input.credential_id.clone(),
        credential_name: input.credential_name.clone(),
        linked_totp_name: input.linked_totp_name.clone(),
        access_scopes: session_access_scopes,
        subdomain_access: session_subdomain_access,
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
    if let Err(error) = state
        .store
        .add_session(&session_id, &session, ttl_seconds)
        .await
    {
        if let Some(record_id) = whitelist_record_id.as_deref() {
            let _ = whitelist::remove_whitelist_record_by_id(state, record_id).await;
        }
        return Err(error.into());
    }

    let registration = async {
        if !normalized_client_ip.is_empty()
            && effective_post_login_mode == "follow_session"
            && settings.session_ip_mobility_enabled
        {
            whitelist_record_id = Some(
                register_login_session(
                    state,
                    &session_id,
                    &normalized_client_ip,
                    ip_location.as_deref(),
                    Some(expire_at),
                )
                .await?,
            );
        } else if !normalized_client_ip.is_empty() {
            if let (Some(record_id), Some(grant_expire_at)) =
                (whitelist_record_id.as_deref(), post_login_grant_expire_at)
            {
                publish_login_auto_whitelist(
                    state,
                    &session_id,
                    &normalized_client_ip,
                    grant_expire_at,
                    auto_comment.clone(),
                    record_id,
                )
                .await?;
                if effective_post_login_mode == "follow_session"
                    && !initialize_login_session_mobility_metadata(
                        state,
                        &session_id,
                        &normalized_client_ip,
                        ip_location.as_deref(),
                        record_id,
                        Some(expire_at),
                    )
                    .await?
                {
                    anyhow::bail!("Login session was revoked during mobility initialization");
                }
            }
            record_browser_session_login(
                state,
                &session_id,
                &normalized_client_ip,
                ip_location.as_deref(),
            )
            .await?;
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if let Err(error) = registration {
        if let Err(cleanup_error) = destroy_session(state, &session_id).await {
            tracing::warn!(%cleanup_error, %session_id, "failed to rollback partially initialized login session");
        }
        if let Some(record_id) = whitelist_record_id.as_deref()
            && let Err(cleanup_error) =
                whitelist::remove_whitelist_record_by_id(state, record_id).await
        {
            tracing::warn!(%cleanup_error, %session_id, %record_id, "failed to rollback login whitelist record");
        }
        return Err(error);
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
            .then_some(effective_post_login_mode),
        session_comment,
    })
}
