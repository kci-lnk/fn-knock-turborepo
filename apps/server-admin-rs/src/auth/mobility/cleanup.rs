use super::*;

pub async fn destroy_sessions_for_totp_credential(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<usize> {
    let sessions = state.store.list_login_sessions().await?;
    let mut destroyed = 0usize;
    for (session_id, session) in sessions {
        if session.totp_id != totp_id {
            continue;
        }
        destroy_session(state, &session_id).await?;
        state.store.delete_session(&session_id).await?;
        destroyed += 1;
    }
    if destroyed > 0 {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(destroyed)
}

pub async fn destroy_sessions_for_auth_credential(
    state: &AppState,
    credential_id: &str,
) -> anyhow::Result<usize> {
    let sessions = state.store.list_login_sessions().await?;
    let mut destroyed = 0usize;
    for (session_id, session) in sessions {
        if session.credential_id != credential_id {
            continue;
        }
        destroy_session(state, &session_id).await?;
        state.store.delete_session(&session_id).await?;
        destroyed += 1;
    }
    if destroyed > 0 {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(destroyed)
}

pub async fn destroy_sessions_for_auth_method(
    state: &AppState,
    auth_method: &str,
) -> anyhow::Result<usize> {
    let sessions = state.store.list_login_sessions().await?;
    let mut destroyed = 0usize;
    for (session_id, session) in sessions {
        if !session.method.eq_ignore_ascii_case(auth_method) {
            continue;
        }
        destroy_session(state, &session_id).await?;
        state.store.delete_session(&session_id).await?;
        destroyed += 1;
    }
    if destroyed > 0 {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(destroyed)
}

pub async fn destroy_session(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let lease = loop {
        if let Some(lease) = acquire_auth_mobility_session_mutation_lease(state, session_id).await?
        {
            break lease;
        }
        tracing::warn!(%session_id, "still waiting for auth mobility mutation lock during revocation");
    };
    let result = async {
        // With the mutation lease held, publication and revocation are ordered:
        // state committed before this delete is collected below, while later
        // writers recheck the missing authoritative Session and fail closed.
        state.store.delete_session(session_id).await?;
        let whitelist_ids = state
            .store
            .destroy_auth_mobility_session(session_id)
            .await?;
        for whitelist_id in whitelist_ids {
            if let Err(error) =
                whitelist::remove_whitelist_record_by_id(state, &whitelist_id).await
            {
                tracing::warn!(%error, %session_id, %whitelist_id, "failed to remove mobility whitelist record");
            }
        }
        Ok(())
    }
    .await;
    if let Err(error) = lease.release().await {
        tracing::warn!(%error, %session_id, "failed to release auth mobility session mutation lock after revocation");
    }
    result
}

pub async fn clear_auto_ip_grants_for_totp_credential(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<bool> {
    clear_auto_ip_grants_for_matching_sessions(state, |session| session.totp_id == totp_id).await
}

pub async fn clear_auto_ip_grants_for_auth_credential(
    state: &AppState,
    credential_id: &str,
) -> anyhow::Result<bool> {
    clear_auto_ip_grants_for_matching_sessions(state, |session| {
        session.credential_id == credential_id
    })
    .await
}

pub async fn reconcile_stream_access_grants_for_totp_credential(
    state: &AppState,
    totp_id: &str,
    credential_access: &Value,
) -> anyhow::Result<bool> {
    reconcile_stream_access_grants_for_matching_sessions(
        state,
        |session| session.totp_id == totp_id,
        |_| Some(credential_access.clone()),
        None,
    )
    .await
}

pub async fn reconcile_stream_access_grants_for_auth_credential(
    state: &AppState,
    credential_id: &str,
    credential_access: &Value,
) -> anyhow::Result<bool> {
    reconcile_stream_access_grants_for_matching_sessions(
        state,
        |session| session.credential_id == credential_id,
        |_| Some(credential_access.clone()),
        None,
    )
    .await
}

pub async fn reconcile_all_stream_access_grants(
    state: &AppState,
    settings_value: &Value,
) -> anyhow::Result<bool> {
    let totps = state.store.get_totps().await?;
    let accounts = state.store.get_auth_accounts().await?;
    reconcile_stream_access_grants_for_matching_sessions(
        state,
        |_| true,
        |session| {
            if crate::auth::mode::AuthMethod::Password.matches_session_str(&session.method) {
                accounts
                    .iter()
                    .find(|account| account.id == session.credential_id)
                    .map(|account| account.subdomain_access.clone())
            } else {
                totps
                    .iter()
                    .find(|credential| credential.id == session.totp_id)
                    .map(|credential| credential.subdomain_access.clone())
            }
        },
        Some(settings_value),
    )
    .await
}

async fn reconcile_stream_access_grants_for_matching_sessions<M, A>(
    state: &AppState,
    matches_session: M,
    credential_access_for_session: A,
    settings_value: Option<&Value>,
) -> anyhow::Result<bool>
where
    M: Fn(&LoginSession) -> bool,
    A: Fn(&LoginSession) -> Option<Value>,
{
    let settings = match settings_value {
        Some(value) => AuthCredentialSettings::from_raw(value),
        None => {
            let config = state.store.get_config().await?;
            AuthCredentialSettings::from_config(&config)
        }
    };
    let sessions = state.store.list_login_sessions().await?;
    let mut changed = false;
    for (session_id, session) in sessions {
        if !matches_session(&session) {
            continue;
        }
        let next_expires_at = credential_access_for_session(&session).and_then(|access| {
            session
                .expires_at
                .as_deref()
                .and_then(|expires_at| stream_access_expires_at(&settings, expires_at, &access))
        });
        if session.stream_access_expires_at == next_expires_at {
            continue;
        }
        let mut updates = Map::new();
        updates.insert(
            "streamAccessExpiresAt".to_string(),
            next_expires_at.map(Value::String).unwrap_or(Value::Null),
        );
        if state
            .store
            .update_session_value(&session_id, updates)
            .await?
            .is_some()
        {
            changed = true;
        }
    }
    Ok(changed)
}

async fn clear_auto_ip_grants_for_matching_sessions<F>(
    state: &AppState,
    matches_session: F,
) -> anyhow::Result<bool>
where
    F: Fn(&LoginSession) -> bool,
{
    let sessions = state.store.list_login_sessions().await?;
    let mut changed = false;
    for (session_id, session) in sessions {
        if !matches_session(&session) {
            continue;
        }

        let mut whitelist_record_ids = list_session_whitelist_record_ids(state, &session_id)
            .await?
            .into_iter()
            .collect::<BTreeSet<_>>();
        if let Some(record_id) = session
            .post_login_ip_grant_record_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            whitelist_record_ids.insert(record_id.to_string());
        }

        for record_id in whitelist_record_ids {
            changed = whitelist::remove_whitelist_record_by_id(state, &record_id).await? || changed;
        }

        if session.grant_type.as_deref() == Some("login_ip_grant")
            || session.post_login_ip_grant_mode.is_some()
            || session.post_login_ip_grant_record_id.is_some()
        {
            let mut updates = Map::new();
            updates.insert(
                "grantType".to_string(),
                Value::String("browser_session".to_string()),
            );
            updates.insert("postLoginIpGrantMode".to_string(), Value::Null);
            updates.insert("postLoginIpGrantRecordId".to_string(), Value::Null);
            state
                .store
                .update_session_value(&session_id, updates)
                .await?;
            changed = true;
        }
    }
    if changed {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(changed)
}

pub async fn list_session_whitelist_record_ids(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<String>> {
    let mut record_ids = BTreeSet::new();
    if let Some(binding) = state
        .store
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?
        && let Some(record_id) = binding_whitelist_record_id(&binding)
    {
        record_ids.insert(record_id);
    }
    for detail in state
        .store
        .list_auth_mobility_active_ip_details(session_id)
        .await?
        .into_iter()
        .filter_map(parse_active_ip_detail)
    {
        if let Some(record_id) = detail.whitelist_record_id {
            record_ids.insert(record_id);
        }
    }
    Ok(record_ids.into_iter().collect())
}

pub async fn reconcile_session_ip_mobility_policy(
    state: &AppState,
    previous_settings: &Value,
    next_settings: &Value,
    schedule_sync: bool,
) -> anyhow::Result<()> {
    let previous = AuthCredentialSettings::from_raw(previous_settings);
    let next = AuthCredentialSettings::from_raw(next_settings);
    let sessions = state.store.list_login_sessions().await?;
    if !next.session_ip_mobility_enabled {
        for (session_id, session) in sessions {
            cleanup_session_active_ip_state(state, &session_id, &session, true).await?;
        }
        if schedule_sync {
            whitelist::sync_reverse_proxy_trusted_ips(state).await;
        }
        return Ok(());
    }

    let should_seed_current_ip =
        !previous.session_ip_mobility_enabled && next.session_ip_mobility_enabled;
    let now = now_seconds();
    for (session_id, session) in sessions {
        let current_ip = normalized_or_trimmed_ip(&session.ip);
        if should_seed_current_ip && !current_ip.is_empty() {
            record_session_active_ip(RecordSessionActiveIpArgs {
                state,
                session_id: &session_id,
                session: Some(&session),
                client_ip: &current_ip,
                source: "session-refresh",
                ip_location: session.ip_location.as_deref(),
                whitelist_record_id: session.post_login_ip_grant_record_id.as_deref(),
                settings: Some(&next),
                sync_reason: "session-ip-mobility-reconcile",
                schedule_sync,
            })
            .await?;
        }
        prune_session_active_ips(
            state,
            &session_id,
            &session,
            &next,
            now,
            PruneOptions {
                keep_ip: None,
                schedule_sync,
            },
        )
        .await?;
    }
    if schedule_sync {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(())
}

pub(super) async fn cleanup_session_active_ip_state(
    state: &AppState,
    session_id: &str,
    _session: &LoginSession,
    preserve_legacy_single_slot: bool,
) -> anyhow::Result<()> {
    let Some(lease) = acquire_auth_mobility_session_mutation_lease(state, session_id).await? else {
        anyhow::bail!("Timed out waiting for auth mobility session mutation lock");
    };
    let result = async {
        let Some(live_session) = state.store.get_session(session_id).await? else {
            return Ok(());
        };
        cleanup_session_active_ip_state_locked(
            state,
            session_id,
            &live_session,
            preserve_legacy_single_slot,
        )
        .await
    }
    .await;
    if let Err(error) = lease.release().await {
        tracing::warn!(%error, %session_id, "failed to release auth mobility session mutation lock after policy cleanup");
    }
    result
}

async fn cleanup_session_active_ip_state_locked(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    preserve_legacy_single_slot: bool,
) -> anyhow::Result<()> {
    let details = state
        .store
        .list_auth_mobility_active_ip_details(session_id)
        .await?
        .into_iter()
        .filter_map(parse_active_ip_detail)
        .collect::<Vec<_>>();
    let mut preserve_record_id = None::<String>;

    if preserve_legacy_single_slot && is_follow_session_auto_grant(session) {
        let current_ip = normalized_or_trimmed_ip(&session.ip);
        if !current_ip.is_empty() {
            let current_detail = details.iter().find(|detail| detail.ip == current_ip);
            let existing_record_id = session
                .post_login_ip_grant_record_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
                .or_else(|| current_detail.and_then(|detail| detail.whitelist_record_id.clone()));
            let config = state.store.get_config().await?;
            let auto_comment = normalize_auto_ip_grant_comment(session.comment.as_deref(), &config)
                .unwrap_or_else(|| auto_ip_grant_comment(&config));
            let record = whitelist::ensure_session_auto_whitelist(
                state,
                &format!("auth-mobility:legacy:{session_id}"),
                &current_ip,
                parse_iso_unix(session.expires_at.as_deref()),
                Some(auto_comment),
                existing_record_id.as_deref(),
            )
            .await?;
            preserve_record_id = Some(record.id.clone());
            if !ensure_legacy_proxy_binding(state, session_id, session, &current_ip, &record.id)
                .await?
            {
                whitelist::remove_whitelist_record_by_id(state, &record.id).await?;
                return Ok(());
            }
            if session.post_login_ip_grant_record_id.as_deref() != Some(record.id.as_str()) {
                let mut updates = Map::new();
                updates.insert(
                    "postLoginIpGrantRecordId".to_string(),
                    Value::String(record.id.clone()),
                );
                state
                    .store
                    .update_session_value(session_id, updates)
                    .await?;
            }
        }
    }

    state
        .store
        .clear_auth_mobility_active_ip_session(session_id)
        .await?;
    for detail in details {
        let Some(record_id) = detail.whitelist_record_id else {
            continue;
        };
        if preserve_record_id.as_deref() == Some(record_id.as_str()) {
            continue;
        }
        whitelist::remove_whitelist_record_by_id(state, &record_id).await?;
    }
    Ok(())
}

pub(super) async fn ensure_legacy_proxy_binding(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    current_ip: &str,
    whitelist_record_id: &str,
) -> anyhow::Result<bool> {
    let expire_at = parse_iso_unix(session.expires_at.as_deref());
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(false);
    };
    let existing = state
        .store
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?;
    let next_binding = build_or_update_mobility_binding(
        existing,
        "proxy-session",
        session_id,
        current_ip,
        expire_at,
        Some(session_id),
        Some(whitelist_record_id.to_string()),
    );
    if !state
        .store
        .save_auth_mobility_owned_binding(
            "proxy-session",
            session_id,
            &next_binding,
            session_id,
            ttl_seconds,
            Some(ttl_seconds),
        )
        .await?
    {
        return Ok(false);
    }
    if !state
        .store
        .set_auth_mobility_whitelist_owner(whitelist_record_id, session_id, ttl_seconds)
        .await?
    {
        return Ok(false);
    }
    Ok(true)
}
