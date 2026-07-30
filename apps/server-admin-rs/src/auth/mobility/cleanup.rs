use super::*;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LoginSessionRevocationOutcome {
    pub complete: bool,
}

pub async fn revoke_login_session(
    state: &AppState,
    session_id: &str,
    config: Option<&Value>,
    fallback_ip: &str,
    logout_source: &'static str,
) -> LoginSessionRevocationOutcome {
    let mut complete = true;
    let session = match state.store.get_session(session_id).await {
        Ok(session) => session,
        Err(error) => {
            tracing::warn!(%error, %session_id, %logout_source, "failed to load auth session during revocation");
            complete = false;
            None
        }
    };

    let loaded_config;
    let config = if let Some(config) = config {
        Some(config)
    } else if session.is_some() {
        loaded_config = match state.store.get_config().await {
            Ok(config) => Some(config),
            Err(error) => {
                tracing::warn!(%error, %session_id, %logout_source, "failed to load config during session revocation");
                complete = false;
                None
            }
        };
        loaded_config.as_ref()
    } else {
        None
    };

    if let Some(session) = session.as_ref()
        && let Err(error) = system_events::publish_auth_logout_event(
            state,
            json!({
                "session_id": session_id,
                "auth_method": session.method.clone(),
                "credential_id": session.credential_id.clone(),
                "credential_name": session.credential_name.clone(),
                "linked_totp_name": session.linked_totp_name.clone(),
                "session_comment": session.comment.clone(),
                "ip": session.ip.clone(),
                "ip_location": session.ip_location.clone(),
                "user_agent": session.user_agent.clone(),
                "login_time": session.login_time.clone(),
                "logout_source": logout_source,
            }),
        )
        .await
    {
        tracing::warn!(%error, %session_id, %logout_source, "failed to publish auth logout event");
    }

    if let Err(error) = destroy_session(state, session_id).await {
        tracing::warn!(%error, %session_id, %logout_source, "failed to destroy auth session state");
        complete = false;
    }
    if let Err(error) =
        revoke_custom_post_login_ip_grant(state, session.as_ref(), config, fallback_ip).await
    {
        tracing::warn!(%error, %session_id, %logout_source, "failed to revoke custom post-login IP grant");
        complete = false;
    }
    if let Err(error) = whitelist::sync_reverse_proxy_trusted_ips_required(state).await {
        tracing::warn!(%error, %session_id, %logout_source, "failed to confirm gateway trust revocation");
        complete = false;
    }

    LoginSessionRevocationOutcome { complete }
}

pub async fn revoke_custom_post_login_ip_grant(
    state: &AppState,
    session: Option<&LoginSession>,
    config: Option<&Value>,
    fallback_ip: &str,
) -> anyhow::Result<bool> {
    if !should_revoke_custom_post_login_ip_grant(session, config) {
        return Ok(false);
    }
    if let Some(record_id) = session
        .and_then(|session| session.post_login_ip_grant_record_id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return whitelist::remove_whitelist_record_by_id(state, record_id).await;
    }
    let ip = session
        .map(|session| session.ip.as_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_ip);
    whitelist::remove_whitelist_records_by_ip(state, ip, Some("auto")).await
}

pub fn should_revoke_custom_post_login_ip_grant(
    session: Option<&LoginSession>,
    config: Option<&Value>,
) -> bool {
    let Some(session) = session else {
        return false;
    };
    if session.grant_type.as_deref() == Some("login_ip_grant")
        && session.post_login_ip_grant_mode.as_deref() == Some("custom")
    {
        return true;
    }
    session
        .comment
        .as_deref()
        .is_some_and(is_auto_ip_grant_comment)
        && config.and_then(|config| {
            config
                .pointer("/auth_credential_settings/post_login_ip_grant_mode")
                .and_then(Value::as_str)
        }) == Some("custom")
}

pub async fn destroy_sessions_for_totp_credential(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<usize> {
    destroy_sessions_matching(state, |session| session.totp_id == totp_id).await
}

pub async fn destroy_sessions_for_auth_credential(
    state: &AppState,
    credential_id: &str,
) -> anyhow::Result<usize> {
    destroy_sessions_matching(state, |session| session.credential_id == credential_id).await
}

pub async fn destroy_sessions_for_auth_method(
    state: &AppState,
    auth_method: &str,
) -> anyhow::Result<usize> {
    destroy_sessions_matching(state, |session| {
        session.method.eq_ignore_ascii_case(auth_method)
    })
    .await
}

async fn destroy_sessions_matching(
    state: &AppState,
    matches: impl Fn(&LoginSession) -> bool,
) -> anyhow::Result<usize> {
    let sessions = state.store.list_login_sessions().await?;
    let mut destroyed = 0usize;
    for (session_id, session) in sessions {
        if !matches(&session) {
            continue;
        }
        if let Err(error) = destroy_session_state(state, &session_id).await {
            // A failed mutation can still have deleted authoritative session
            // state before a later storage step failed. Publish that partial
            // revocation before returning the original error.
            whitelist::sync_reverse_proxy_trusted_ips(state).await;
            return Err(error);
        }
        destroyed += 1;
    }
    if destroyed > 0 {
        whitelist::sync_reverse_proxy_trusted_ips_required(state).await?;
    }
    Ok(destroyed)
}

pub async fn destroy_session(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let result = destroy_session_state(state, session_id).await;
    whitelist::sync_reverse_proxy_trusted_ips(state).await;
    result
}

async fn destroy_session_state(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let lease = loop {
        if let Some(lease) = acquire_auth_mobility_session_mutation_lease(state, session_id).await?
        {
            break lease;
        }
        tracing::warn!(%session_id, "still waiting for auth mobility mutation lock during revocation");
    };
    let result = async {
        // The mutation lease excludes mobility writers. Remove grants and
        // secondary indexes before the authoritative Session so any failure is
        // fail-closed but remains discoverable by credential-based retries.
        let whitelist_ids = state
            .store
            .list_auth_mobility_session_whitelist_ids(session_id)
            .await?;
        whitelist::remove_whitelist_records_without_runtime_sync(state, &whitelist_ids).await?;
        state
            .store
            .destroy_auth_mobility_session(session_id)
            .await?;
        state.store.delete_session(session_id).await?;
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
    Ok(state
        .store
        .list_auth_mobility_session_whitelist_ids(session_id)
        .await?)
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
