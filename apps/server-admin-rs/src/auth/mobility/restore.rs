use super::*;

pub async fn try_restore_access(
    state: &AppState,
    client_ip: &str,
    identity: AuthMobilityRestoreIdentity<'_>,
) -> anyhow::Result<AuthMobilityRestoreResult> {
    let _phase = crate::auth::diagnostics::enter("mobility_restore");
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
        .storage
        .store
        .get_auth_mobility_binding_for_authorization(subject_type, subject_key)
        .await?;
    if let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id)
        && state
            .storage
            .store
            .get_session(&owner_session_id)
            .await?
            .is_none()
        && let Some(mut orphaned) = binding.take()
    {
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
            return Ok(false);
        }
        binding = Some(orphaned);
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
            return Ok(false);
        }
        binding = Some(next_binding);
    }

    let Some(owner_session_id) = binding.as_ref().and_then(binding_owner_session_id) else {
        return Ok(false);
    };
    let Some(owner_session) = state.storage.store.get_session(&owner_session_id).await? else {
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
        return Ok(false);
    }

    if sync_browser_session_ip(state, &owner_session_id, &normalized_ip, sync_source)
        .await?
        .is_none()
    {
        return Ok(false);
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
    let Some(session) = state.storage.store.get_session(session_id).await? else {
        return Ok(false);
    };
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(false);
    }
    let config = crate::auth::request_context::config(state);
    let settings = AuthCredentialSettings::from_config(&config);
    // The common same-IP path needs no binding or primary-executor work.
    if settings.session_ip_mobility_enabled && session_ip_matches(&session, &normalized_ip) {
        return Ok(false);
    }
    let binding = state
        .storage
        .store
        .get_auth_mobility_binding_for_authorization("proxy-session", session_id)
        .await?;
    if !settings.session_ip_mobility_enabled {
        let Some(binding) = binding.as_ref() else {
            return Ok(false);
        };
        if binding_whitelist_record_id(binding).is_none() {
            return Ok(false);
        }
        if session_ip_matches(&session, &normalized_ip)
            && mobility_binding_touch_is_fresh(binding, &normalized_ip, session_id, now_seconds())
        {
            return Ok(true);
        }
    }
    // A request snapshot only drives read-only shortcuts. Re-read configuration
    // before the slow path can mutate IP ownership or publish a whitelist.
    let config = state.storage.store.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);

    if settings.session_ip_mobility_enabled {
        if session_ip_matches(&session, &normalized_ip) {
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
                return Ok(false);
            }
        }
        return Ok(
            sync_browser_session_ip(state, session_id, &normalized_ip, "proxy-session")
                .await?
                .is_some(),
        );
    }

    let Some(binding) = binding else {
        return Ok(false);
    };
    if binding_whitelist_record_id(&binding).is_none() {
        return Ok(false);
    }
    if session_ip_matches(&session, &normalized_ip)
        && mobility_binding_touch_is_fresh(&binding, &normalized_ip, session_id, now_seconds())
    {
        // Keep the historical session_migration grant while avoiding a
        // no-op whitelist/binding/session rewrite for an already-current IP.
        return Ok(true);
    }
    restore_single_ip_proxy_session(state, session_id, &normalized_ip).await
}

async fn restore_single_ip_proxy_session(
    state: &AppState,
    session_id: &str,
    normalized_ip: &str,
) -> anyhow::Result<bool> {
    let Some(lease) = acquire_auth_mobility_session_mutation_lease(state, session_id).await? else {
        anyhow::bail!("Timed out waiting for auth mobility session mutation lock");
    };
    let result = async {
        if !lease.ensure_valid().await? {
            return Ok(false);
        }
        let Some(session) = state.storage.store.get_session(session_id).await? else {
            return Ok(false);
        };
        let Some(binding) = state
            .storage
            .store
            .get_auth_mobility_binding_for_authorization("proxy-session", session_id)
            .await?
        else {
            return Ok(false);
        };
        let Some(whitelist_record_id) = binding_whitelist_record_id(&binding) else {
            return Ok(false);
        };
        if session_ip_matches(&session, normalized_ip)
            && mobility_binding_touch_is_fresh(&binding, normalized_ip, session_id, now_seconds())
        {
            return Ok(true);
        }
        let Some(moved_record) =
            whitelist::move_record_to_ip(state, &whitelist_record_id, normalized_ip).await?
        else {
            return Ok(false);
        };
        if !lease.ensure_valid().await? {
            return Ok(false);
        }
        let next_binding = build_or_update_mobility_binding(
            Some(binding),
            "proxy-session",
            session_id,
            normalized_ip,
            moved_record
                .expire_at
                .or_else(|| parse_iso_unix(session.expires_at.as_deref())),
            Some(session_id),
            Some(whitelist_record_id),
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
            let _ = whitelist::remove_whitelist_record_by_id(state, &moved_record.id).await;
            return Ok(false);
        }
        Ok(
            sync_browser_session_ip(state, session_id, normalized_ip, "proxy-session")
                .await?
                .is_some(),
        )
    }
    .await;
    if let Err(error) = lease.release().await {
        tracing::warn!(%error, %session_id, "failed to release single-IP session restore lock");
    }
    result
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

pub async fn list_active_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    list_sessions_by_ip(state, client_ip, false).await
}

pub(crate) async fn list_stream_access_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    list_sessions_by_ip(state, client_ip, true).await
}

async fn list_sessions_by_ip(
    state: &AppState,
    client_ip: &str,
    stream: bool,
) -> anyhow::Result<Vec<(String, LoginSession)>> {
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(Vec::new());
    }
    let config = crate::auth::request_context::config(state);
    let settings = AuthCredentialSettings::from_config(&config);
    let window = settings
        .session_ip_mobility_enabled
        .then_some(settings.session_ip_mobility_window_seconds);
    let candidates = state
        .storage
        .store
        .list_auth_session_ip_candidates(window)
        .await?;
    let mut owners = Vec::new();
    for (session_id, session, details) in candidates {
        if !session_ip_candidate_matches(&session, details, &normalized_ip, &settings, stream) {
            continue;
        }
        if let Some(session) =
            confirm_session_ip_candidate(state, &session_id, &normalized_ip, &settings, stream)
                .await?
        {
            owners.push((session_id, session));
        }
    }
    Ok(owners)
}

fn session_ip_candidate_matches(
    session: &LoginSession,
    details: Vec<Value>,
    client_ip: &str,
    settings: &AuthCredentialSettings,
    stream: bool,
) -> bool {
    let matches = |ip: &str| {
        if stream {
            stream_access_ip_matches(ip, client_ip)
        } else {
            normalized_or_trimmed_ip(ip) == client_ip
        }
    };
    // Streams retain canonical IP access for the grant lifetime; HTTP with
    // mobility enabled only recognizes addresses in the active window.
    if (stream || !settings.session_ip_mobility_enabled) && matches(&session.ip) {
        return true;
    }
    settings.session_ip_mobility_enabled
        && details
            .into_iter()
            .filter_map(parse_active_ip_detail)
            .any(|detail| matches(&detail.ip))
}

pub(super) async fn confirm_session_ip_candidate(
    state: &AppState,
    session_id: &str,
    client_ip: &str,
    settings: &AuthCredentialSettings,
    stream: bool,
) -> anyhow::Result<Option<LoginSession>> {
    // Do not authorize from a candidate snapshot: logout, expiry and drift can
    // invalidate it while other candidates are being inspected.
    let Some(session) = state.storage.store.get_session(session_id).await? else {
        return Ok(None);
    };
    if crate::auth::login_session_has_expired(&session) {
        return Ok(None);
    }
    if session_ip_candidate_matches(&session, Vec::new(), client_ip, settings, stream) {
        return Ok(Some(session));
    }
    let details = if settings.session_ip_mobility_enabled {
        state
            .storage
            .store
            .get_auth_mobility_recent_ip_details_for_authorization(
                session_id,
                settings.session_ip_mobility_window_seconds,
            )
            .await?
    } else {
        Vec::new()
    };
    Ok(
        session_ip_candidate_matches(&session, details, client_ip, settings, stream)
            .then_some(session),
    )
}

fn stream_access_ip_matches(left: &str, right: &str) -> bool {
    let left = normalized_or_trimmed_ip(left);
    let right = normalized_or_trimmed_ip(right);
    if left.is_empty() || right.is_empty() {
        return false;
    }
    match (
        left.parse::<std::net::IpAddr>(),
        right.parse::<std::net::IpAddr>(),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}
