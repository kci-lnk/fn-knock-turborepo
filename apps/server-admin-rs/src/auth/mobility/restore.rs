use super::*;

pub async fn try_restore_access(
    state: &AppState,
    client_ip: &str,
    identity: AuthMobilityRestoreIdentity<'_>,
) -> anyhow::Result<AuthMobilityRestoreResult> {
    if let Some(token) = identity.fnos_token.filter(|value| !value.trim().is_empty())
        && restore_app_token_binding(state, "fnos-token", token, client_ip, "fnos-token").await?
    {
        return Ok(AuthMobilityRestoreResult {
            success: true,
            grant_type: Some("fnos_fingerprint_session"),
        });
    }

    if let Some(token) = identity
        .trim_media_token
        .filter(|value| !value.trim().is_empty())
        && restore_app_token_binding(state, "trim-media-token", token, client_ip, "fnos-token")
            .await?
    {
        return Ok(AuthMobilityRestoreResult {
            success: true,
            grant_type: Some("fnos_fingerprint_session"),
        });
    }

    match identity.app_binding {
        Some("fnos-app") if restore_anonymous_fnos_app(state, client_ip).await? => {
            return Ok(AuthMobilityRestoreResult {
                success: true,
                grant_type: Some("fnos_fingerprint_session"),
            });
        }
        Some("trim-media-app") if restore_trim_media_app(state, client_ip).await? => {
            return Ok(AuthMobilityRestoreResult {
                success: true,
                grant_type: Some("fnos_fingerprint_session"),
            });
        }
        _ => {}
    }

    if let Some(session_id) = identity.session_id.filter(|value| !value.trim().is_empty())
        && restore_proxy_session(state, session_id, client_ip).await?
    {
        return Ok(AuthMobilityRestoreResult {
            success: true,
            grant_type: Some("session_migration"),
        });
    }

    Ok(AuthMobilityRestoreResult {
        success: false,
        grant_type: None,
    })
}

pub(super) async fn restore_app_token_binding(
    state: &AppState,
    subject_type: &str,
    subject_key: &str,
    client_ip: &str,
    sync_source: &str,
) -> anyhow::Result<bool> {
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(false);
    }

    let mut binding = state
        .redis
        .get_auth_mobility_binding(subject_type, subject_key)
        .await?;
    if let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id)
        && state.redis.get_session(&owner_session_id).await?.is_none()
    {
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

    if binding
        .as_ref()
        .and_then(binding_owner_session_id)
        .is_none()
    {
        let Some((owner_session_id, owner_session)) =
            resolve_bootstrap_owner(state, &normalized_ip).await?
        else {
            return Ok(false);
        };
        let expire_at = parse_iso_unix(owner_session.expires_at.as_deref());
        let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
            return Ok(false);
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
        binding = Some(next_binding);
    }

    let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id) else {
        return Ok(false);
    };
    let Some(owner_session) = state.redis.get_session(&owner_session_id).await? else {
        return Ok(false);
    };
    let expire_at = parse_iso_unix(owner_session.expires_at.as_deref());
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(false);
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
        .save_auth_mobility_binding_with_ttl(subject_type, subject_key, &next_binding, ttl_seconds)
        .await?;

    let updated_session =
        sync_browser_session_ip(state, &owner_session_id, &normalized_ip, sync_source).await?;
    if let Some(updated_session) = updated_session {
        let session_ttl =
            resolve_proxy_session_ttl(parse_iso_unix(updated_session.expires_at.as_deref()));
        state
            .redis
            .add_auth_mobility_session_binding(
                &owner_session_id,
                subject_type,
                subject_key,
                session_ttl,
            )
            .await?;
    }

    Ok(true)
}

pub(super) async fn restore_anonymous_fnos_app(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<bool> {
    let Some((owner_session_id, _owner_session)) =
        resolve_bootstrap_owner(state, client_ip).await?
    else {
        return Ok(false);
    };
    let _ = ip_location::register_usage(
        state,
        client_ip,
        vec![
            format!("session|{owner_session_id}"),
            format!("session-timeline|{owner_session_id}"),
        ],
    )
    .await;
    Ok(true)
}

pub(super) async fn restore_trim_media_app(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<bool> {
    let sessions = list_active_sessions_by_ip(state, client_ip).await?;
    if sessions.is_empty() {
        return Ok(false);
    }

    let mut references = Vec::new();
    for (session_id, _session) in sessions {
        for reference in [
            format!("session|{session_id}"),
            format!("session-timeline|{session_id}"),
        ] {
            if !references.iter().any(|value| value == &reference) {
                references.push(reference);
            }
        }
    }
    let _ = ip_location::register_usage(state, client_ip, references).await;
    Ok(true)
}

pub(super) async fn restore_proxy_session(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
) -> anyhow::Result<bool> {
    let Some(session) = state.redis.get_session(session_id).await? else {
        return Ok(false);
    };
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(false);
    }

    let config = state.redis.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    let binding = state
        .redis
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?;

    if settings.session_ip_mobility_enabled {
        if normalized_or_trimmed_ip(&session.ip) == normalized_ip {
            return Ok(false);
        }
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
        }
        sync_browser_session_ip(state, session_id, &normalized_ip, "proxy-session").await?;
        return Ok(true);
    }

    let Some(binding) = binding else {
        return Ok(false);
    };
    let Some(whitelist_record_id) = binding_whitelist_record_id(&binding) else {
        return Ok(false);
    };
    let Some(moved_record) =
        whitelist::move_record_to_ip(state, &whitelist_record_id, &normalized_ip).await?
    else {
        return Ok(false);
    };
    let next_binding = build_or_update_mobility_binding(
        Some(binding),
        "proxy-session",
        session_id,
        &normalized_ip,
        moved_record
            .expire_at
            .or_else(|| parse_iso_unix(session.expires_at.as_deref())),
        Some(session_id),
        Some(whitelist_record_id),
    );
    state
        .redis
        .save_auth_mobility_binding_keep_ttl("proxy-session", session_id, &next_binding)
        .await?;
    sync_browser_session_ip(state, session_id, &normalized_ip, "proxy-session").await?;
    Ok(true)
}

pub(super) async fn resolve_bootstrap_owner(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Option<(String, LoginSession)>> {
    let sessions = list_active_sessions_by_ip(state, client_ip).await?;
    Ok((sessions.len() == 1)
        .then(|| sessions.into_iter().next())
        .flatten())
}

pub(super) async fn list_active_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(Vec::new());
    }
    let config = state.redis.get_config().await?;
    let mut owners = Vec::new();
    for (session_id, session) in state.redis.list_login_sessions().await? {
        let ips = effective_session_ips(state, &session_id, &session, &config).await?;
        if ips.iter().any(|ip| ip == &normalized_ip) {
            owners.push((session_id, session));
        }
    }
    Ok(owners)
}
