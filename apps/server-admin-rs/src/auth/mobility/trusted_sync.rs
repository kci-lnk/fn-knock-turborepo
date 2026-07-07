use super::*;

pub async fn sync_browser_session_ip(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
    source: &str,
) -> anyhow::Result<Option<LoginSession>> {
    let Some(session) = state.redis.get_session(session_id).await? else {
        return Ok(None);
    };
    let config = state.redis.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    let normalized_client_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_client_ip.is_empty() {
        return Ok(Some(session));
    }

    let previous_ip = session.ip.clone();
    let previous_ip_location = session.ip_location.clone();
    let normalized_previous_ip = normalized_or_trimmed_ip(&previous_ip);
    let ip_changed =
        !normalized_previous_ip.is_empty() && normalized_previous_ip != normalized_client_ip;
    let next_ip_location = cached_ip_location(state, &normalized_client_ip).await;
    let mut updates = Map::new();
    updates.insert(
        "ip".to_string(),
        Value::String(normalized_client_ip.clone()),
    );
    if ip_changed {
        if let Some(ip_location) = next_ip_location
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            updates.insert(
                "ipLocation".to_string(),
                Value::String(ip_location.to_string()),
            );
        }
    } else if let Some(ip_location) = next_ip_location
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        updates.insert(
            "ipLocation".to_string(),
            Value::String(ip_location.to_string()),
        );
    }
    let updated = state
        .redis
        .update_session_value(session_id, updates)
        .await?
        .and_then(|value| serde_json::from_value::<LoginSession>(value).ok())
        .unwrap_or_else(|| {
            let mut fallback = session.clone();
            fallback.ip = normalized_client_ip.clone();
            if let Some(ip_location) = next_ip_location.clone() {
                fallback.ip_location = Some(ip_location);
            }
            fallback
        });

    if ip_changed && !settings.session_ip_mobility_enabled {
        let seed_login_event = mobility_login_event(
            &previous_ip,
            previous_ip_location.as_deref(),
            Some(&session.login_time),
        );
        let drift_event = mobility_drift_event(
            source,
            &previous_ip,
            previous_ip_location.as_deref(),
            &normalized_client_ip,
            next_ip_location.as_deref(),
        );
        state
            .redis
            .append_auth_mobility_timeline_event(
                session_id,
                &drift_event,
                Some(&seed_login_event),
                resolve_proxy_session_ttl(parse_iso_unix(session.expires_at.as_deref())),
            )
            .await?;
        if let Err(error) = system_events::publish_auth_session_ip_drift_event(
            state,
            json!({
                "session_id": session_id,
                "auth_method": session.method,
                "credential_id": session.credential_id,
                "credential_name": session.credential_name,
                "linked_totp_name": session.linked_totp_name,
                "session_comment": session.comment,
                "drift_source": source,
                "from_ip": previous_ip,
                "from_ip_location": previous_ip_location,
                "to_ip": normalized_client_ip,
                "to_ip_location": next_ip_location,
                "login_time": session.login_time,
            }),
        )
        .await
        {
            tracing::warn!(%error, %session_id, "failed to publish auth session IP drift event");
        }
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }

    record_session_active_ip(RecordSessionActiveIpArgs {
        state,
        session_id,
        session: Some(&updated),
        client_ip: &normalized_client_ip,
        source,
        ip_location: next_ip_location.as_deref(),
        whitelist_record_id: None,
        settings: Some(&settings),
        sync_reason: "browser-session-ip-refresh",
        schedule_sync: true,
    })
    .await?;

    Ok(Some(updated))
}

pub async fn sync_trusted_request(
    state: &AppState,
    client_ip: &str,
    identity: AuthMobilityRestoreIdentity<'_>,
) -> anyhow::Result<()> {
    if let Some(session_id) = identity.session_id.filter(|value| !value.trim().is_empty()) {
        refresh_proxy_session_binding(state, session_id, client_ip).await?;
    }
    if let Some(token) = identity.fnos_token.filter(|value| !value.trim().is_empty()) {
        refresh_app_token_binding(state, "fnos-token", token, client_ip, identity.session_id)
            .await?;
    }
    if let Some(token) = identity
        .trim_media_token
        .filter(|value| !value.trim().is_empty())
    {
        refresh_app_token_binding(
            state,
            "trim-media-token",
            token,
            client_ip,
            identity.session_id,
        )
        .await?;
    }
    Ok(())
}

pub(super) async fn refresh_proxy_session_binding(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
) -> anyhow::Result<()> {
    let Some(session) = state.redis.get_session(session_id).await? else {
        return Ok(());
    };
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(());
    }
    let binding = state
        .redis
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?;
    if let Some(binding) = binding {
        let whitelist_record_id = binding_whitelist_record_id(&binding);
        let next_binding = build_or_update_mobility_binding(
            Some(binding),
            "proxy-session",
            session_id,
            &normalized_ip,
            parse_iso_unix(session.expires_at.as_deref()),
            Some(session_id),
            whitelist_record_id,
        );
        state
            .redis
            .save_auth_mobility_binding_keep_ttl("proxy-session", session_id, &next_binding)
            .await?;
        sync_browser_session_ip(state, session_id, &normalized_ip, "session-refresh").await?;
        return Ok(());
    }

    let config = state.redis.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    if settings.session_ip_mobility_enabled {
        sync_browser_session_ip(state, session_id, &normalized_ip, "session-refresh").await?;
    }
    Ok(())
}

pub(super) async fn refresh_app_token_binding(
    state: &AppState,
    subject_type: &str,
    subject_key: &str,
    client_ip: &str,
    session_id: Option<&str>,
) -> anyhow::Result<()> {
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(());
    }

    let mut binding = state
        .redis
        .get_auth_mobility_binding(subject_type, subject_key)
        .await?;

    if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
        let Some(session) = state.redis.get_session(session_id).await? else {
            return Ok(());
        };
        if let Some(existing_owner_id) = binding.as_ref().and_then(binding_owner_session_id)
            && existing_owner_id != session_id
        {
            if state.redis.get_session(&existing_owner_id).await?.is_some() {
                return Ok(());
            }
            if let Some(mut orphaned) = binding.take() {
                clear_binding_owner_session(&mut orphaned);
                set_binding_last_seen(&mut orphaned);
                state
                    .redis
                    .save_auth_mobility_orphaned_binding(
                        subject_type,
                        subject_key,
                        &orphaned,
                        &existing_owner_id,
                    )
                    .await?;
                binding = Some(orphaned);
            }
        }
        let expire_at = parse_iso_unix(session.expires_at.as_deref());
        let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
            return Ok(());
        };
        let next_binding = build_or_update_mobility_binding(
            binding,
            subject_type,
            subject_key,
            &normalized_ip,
            expire_at,
            Some(session_id),
            None,
        );
        state
            .redis
            .save_auth_mobility_owned_binding(
                subject_type,
                subject_key,
                &next_binding,
                session_id,
                ttl_seconds,
                resolve_proxy_session_ttl(expire_at),
            )
            .await?;
        return Ok(());
    }

    if let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id) {
        if let Some(owner_session) = state.redis.get_session(&owner_session_id).await? {
            let expire_at = parse_iso_unix(owner_session.expires_at.as_deref());
            let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
                return Ok(());
            };
            let whitelist_record_id = binding.as_ref().and_then(binding_whitelist_record_id);
            let next_binding = build_or_update_mobility_binding(
                binding,
                subject_type,
                subject_key,
                &normalized_ip,
                expire_at,
                Some(&owner_session_id),
                whitelist_record_id,
            );
            state
                .redis
                .save_auth_mobility_owned_binding(
                    subject_type,
                    subject_key,
                    &next_binding,
                    &owner_session_id,
                    ttl_seconds,
                    resolve_proxy_session_ttl(expire_at),
                )
                .await?;
            return Ok(());
        }
        if let Some(mut orphaned) = binding.take() {
            clear_binding_owner_session(&mut orphaned);
            set_binding_last_seen(&mut orphaned);
            state
                .redis
                .save_auth_mobility_orphaned_binding(
                    subject_type,
                    subject_key,
                    &orphaned,
                    &owner_session_id,
                )
                .await?;
            binding = Some(orphaned);
        }
    }

    let Some((owner_session_id, owner_session)) =
        resolve_bootstrap_owner(state, &normalized_ip).await?
    else {
        return Ok(());
    };
    let expire_at = parse_iso_unix(owner_session.expires_at.as_deref());
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(());
    };
    let next_binding = build_or_update_mobility_binding(
        binding,
        subject_type,
        subject_key,
        &normalized_ip,
        expire_at,
        Some(&owner_session_id),
        None,
    );
    state
        .redis
        .save_auth_mobility_owned_binding(
            subject_type,
            subject_key,
            &next_binding,
            &owner_session_id,
            ttl_seconds,
            resolve_proxy_session_ttl(expire_at),
        )
        .await?;
    Ok(())
}
