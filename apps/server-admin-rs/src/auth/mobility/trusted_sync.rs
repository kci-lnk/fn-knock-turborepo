use super::*;

pub async fn sync_browser_session_ip(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
    source: &str,
) -> anyhow::Result<Option<LoginSession>> {
    let Some(session) = state.storage.store.get_session(session_id).await? else {
        return Ok(None);
    };
    sync_browser_session_ip_current(state, session_id, &session, client_ip, source).await
}

pub(crate) async fn sync_browser_session_ip_with_session(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    client_ip: &str,
    source: &str,
) -> anyhow::Result<Option<LoginSession>> {
    let config = state.storage.store.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    let normalized_client_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_client_ip.is_empty() {
        return Ok(Some(session.clone()));
    }

    let session_needs_update = session.ip.trim() != normalized_client_ip
        || (session
            .ip_location
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
            && cached_ip_location(state, &normalized_client_ip)
                .await
                .is_some());
    if !session_needs_update
        && active_ip_touch_is_fresh_for_session(
            state,
            session_id,
            session,
            &normalized_client_ip,
            &settings,
        )
        .await?
    {
        // A borrowed preflight snapshot is safe only while this path is
        // provably read-only. Any slow path below reloads the live session so a
        // concurrent logout cannot recreate mobility/whitelist state.
        return Ok(Some(session.clone()));
    }

    let Some(live_session) = state.storage.store.get_session(session_id).await? else {
        return Ok(None);
    };
    sync_browser_session_ip_current(
        state,
        session_id,
        &live_session,
        &normalized_client_ip,
        source,
    )
    .await
}

async fn sync_browser_session_ip_current(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    client_ip: &str,
    source: &str,
) -> anyhow::Result<Option<LoginSession>> {
    let config = state.storage.store.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    let normalized_client_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_client_ip.is_empty() {
        return Ok(Some(session.clone()));
    }

    let previous_ip = session.ip.clone();
    let previous_ip_location = session.ip_location.clone();
    let normalized_previous_ip = normalized_or_trimmed_ip(&previous_ip);
    let ip_changed =
        !normalized_previous_ip.is_empty() && normalized_previous_ip != normalized_client_ip;
    // Preserve the previous canonicalization behavior once, then stop rewriting
    // the record after the stored representation matches the normalized IP.
    let ip_needs_update = previous_ip.trim() != normalized_client_ip;
    let location_needs_enrichment = session
        .ip_location
        .as_deref()
        .is_none_or(|value| value.trim().is_empty());
    let next_ip_location = if ip_needs_update || location_needs_enrichment {
        cached_ip_location(state, &normalized_client_ip).await
    } else {
        None
    };
    let mut updates = Map::new();
    if ip_needs_update {
        updates.insert(
            "ip".to_string(),
            Value::String(normalized_client_ip.clone()),
        );
    }
    if let Some(ip_location) = next_ip_location
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .filter(|value| session.ip_location.as_deref() != Some(*value))
    {
        updates.insert(
            "ipLocation".to_string(),
            Value::String(ip_location.to_string()),
        );
    }
    let updated = if updates.is_empty() {
        session.clone()
    } else {
        let Some(updated) = state
            .storage
            .store
            .update_session_value(session_id, updates)
            .await?
        else {
            // update_session_value never recreates a missing session. Treat a
            // concurrent deletion as revocation and stop before mobility writes.
            return Ok(None);
        };
        serde_json::from_value::<LoginSession>(updated)
            .map_err(|error| anyhow::anyhow!("updated login session is invalid: {error}"))?
    };

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
        if !state
            .storage
            .store
            .append_auth_mobility_timeline_event(
                session_id,
                &drift_event,
                Some(&seed_login_event),
                resolve_proxy_session_ttl(parse_iso_unix(session.expires_at.as_deref())),
            )
            .await?
        {
            return Ok(None);
        }
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
    }

    // Single-IP sessions feed the gateway trusted-client runtime. Refresh it
    // after any accepted canonical IP write, including a silent repair from a
    // legacy invalid/loopback value to a valid remote address.
    if ip_needs_update && !settings.session_ip_mobility_enabled {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }

    let active_ip = record_session_active_ip(RecordSessionActiveIpArgs {
        state,
        session_id,
        session: Some(&updated),
        client_ip: &normalized_client_ip,
        source,
        ip_location: updated.ip_location.as_deref(),
        whitelist_record_id: None,
        settings: Some(&settings),
        sync_reason: "browser-session-ip-refresh",
        schedule_sync: true,
    })
    .await?;
    if settings.session_ip_mobility_enabled && active_ip.is_none() {
        return Ok(None);
    }

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
    let Some(session) = state.storage.store.get_session(session_id).await? else {
        return Ok(());
    };
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(());
    }
    let binding = state
        .storage
        .store
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?;
    if let Some(binding) = binding {
        if mobility_binding_touch_is_fresh(&binding, &normalized_ip, session_id, now_seconds()) {
            sync_browser_session_ip_current(
                state,
                session_id,
                &session,
                &normalized_ip,
                "session-refresh",
            )
            .await?;
            return Ok(());
        }
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
        if !state
            .storage
            .store
            .save_auth_mobility_binding_keep_ttl(
                "proxy-session",
                session_id,
                &next_binding,
                session_id,
            )
            .await?
        {
            return Ok(());
        }
        sync_browser_session_ip_current(
            state,
            session_id,
            &session,
            &normalized_ip,
            "session-refresh",
        )
        .await?;
        return Ok(());
    }

    let config = state.storage.store.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    if settings.session_ip_mobility_enabled {
        sync_browser_session_ip_current(
            state,
            session_id,
            &session,
            &normalized_ip,
            "session-refresh",
        )
        .await?;
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
        .storage
        .store
        .get_auth_mobility_binding(subject_type, subject_key)
        .await?;

    if let Some(session_id) = session_id.filter(|value| !value.trim().is_empty()) {
        let Some(session) = state.storage.store.get_session(session_id).await? else {
            return Ok(());
        };
        if let Some(existing_owner_id) = binding.as_ref().and_then(binding_owner_session_id)
            && existing_owner_id != session_id
        {
            if state
                .storage
                .store
                .get_session(&existing_owner_id)
                .await?
                .is_some()
            {
                return Ok(());
            }
            if let Some(mut orphaned) = binding.take() {
                clear_binding_owner_session(&mut orphaned);
                set_binding_last_seen(&mut orphaned);
                if !state
                    .storage
                    .store
                    .save_auth_mobility_orphaned_binding(
                        subject_type,
                        subject_key,
                        &orphaned,
                        &existing_owner_id,
                    )
                    .await?
                {
                    return Ok(());
                }
                binding = Some(orphaned);
            }
        }
        let expire_at = parse_iso_unix(session.expires_at.as_deref());
        let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
            return Ok(());
        };
        if binding.as_ref().is_some_and(|value| {
            mobility_binding_touch_is_fresh(value, &normalized_ip, session_id, now_seconds())
        }) {
            return Ok(());
        }
        let next_binding = build_or_update_mobility_binding(
            binding,
            subject_type,
            subject_key,
            &normalized_ip,
            expire_at,
            Some(session_id),
            None,
        );
        if !state
            .storage
            .store
            .save_auth_mobility_owned_binding(
                subject_type,
                subject_key,
                &next_binding,
                session_id,
                ttl_seconds,
                resolve_proxy_session_ttl(expire_at),
            )
            .await?
        {
            return Ok(());
        }
        return Ok(());
    }

    if let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id) {
        if let Some(owner_session) = state.storage.store.get_session(&owner_session_id).await? {
            let expire_at = parse_iso_unix(owner_session.expires_at.as_deref());
            let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
                return Ok(());
            };
            if binding.as_ref().is_some_and(|value| {
                mobility_binding_touch_is_fresh(
                    value,
                    &normalized_ip,
                    &owner_session_id,
                    now_seconds(),
                )
            }) {
                return Ok(());
            }
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
            if !state
                .storage
                .store
                .save_auth_mobility_owned_binding(
                    subject_type,
                    subject_key,
                    &next_binding,
                    &owner_session_id,
                    ttl_seconds,
                    resolve_proxy_session_ttl(expire_at),
                )
                .await?
            {
                return Ok(());
            }
            return Ok(());
        }
        if let Some(mut orphaned) = binding.take() {
            clear_binding_owner_session(&mut orphaned);
            set_binding_last_seen(&mut orphaned);
            if !state
                .storage
                .store
                .save_auth_mobility_orphaned_binding(
                    subject_type,
                    subject_key,
                    &orphaned,
                    &owner_session_id,
                )
                .await?
            {
                return Ok(());
            }
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
    if !state
        .storage
        .store
        .save_auth_mobility_owned_binding(
            subject_type,
            subject_key,
            &next_binding,
            &owner_session_id,
            ttl_seconds,
            resolve_proxy_session_ttl(expire_at),
        )
        .await?
    {
        return Ok(());
    }
    Ok(())
}
