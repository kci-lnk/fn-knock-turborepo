use super::*;

pub(super) async fn maintain_session_active_ips(state: &AppState) -> anyhow::Result<bool> {
    let config = state.store.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    if !settings.session_ip_mobility_enabled {
        return Ok(false);
    }

    let now = now_seconds();
    let sessions = state.store.list_login_sessions().await?;
    let mut changed = false;
    for (session_id, session) in sessions {
        let removed = prune_session_active_ips(
            state,
            &session_id,
            &session,
            &settings,
            now,
            PruneOptions {
                keep_ip: None,
                schedule_sync: false,
            },
        )
        .await?;
        changed = changed || removed > 0;
    }
    if changed {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(changed)
}

pub(super) async fn register_login_session(
    state: &AppState,
    session_id: &str,
    ip: &str,
    ip_location: Option<&str>,
    expire_at: Option<i64>,
) -> anyhow::Result<String> {
    let Some(active_detail) = record_session_active_ip(RecordSessionActiveIpArgs {
        state,
        session_id,
        session: None,
        client_ip: ip,
        source: "login",
        ip_location,
        whitelist_record_id: None,
        settings: None,
        sync_reason: "mobility-login-session",
        schedule_sync: true,
    })
    .await?
    else {
        anyhow::bail!("Login session was revoked during mobility registration");
    };
    let whitelist_record_id = active_detail
        .get("whitelistRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| anyhow::anyhow!("Follow-session mobility whitelist was not created"))?;
    let mut session_updates = Map::new();
    session_updates.insert(
        "postLoginIpGrantRecordId".to_string(),
        Value::String(whitelist_record_id.clone()),
    );
    if state
        .store
        .update_session_value(session_id, session_updates)
        .await?
        .is_none()
    {
        anyhow::bail!("Login session was revoked during mobility registration");
    }
    if !initialize_login_session_mobility_metadata(
        state,
        session_id,
        ip,
        ip_location,
        &whitelist_record_id,
        expire_at,
    )
    .await?
    {
        anyhow::bail!("Login session was revoked during mobility initialization");
    }
    Ok(whitelist_record_id)
}

pub(super) async fn initialize_login_session_mobility_metadata(
    state: &AppState,
    session_id: &str,
    ip: &str,
    ip_location: Option<&str>,
    whitelist_record_id: &str,
    expire_at: Option<i64>,
) -> anyhow::Result<bool> {
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(false);
    };
    let subject_hash = auth_mobility_subject_hash("proxy-session", session_id);
    let now_iso = time_utils::now_iso();
    let binding = json!({
        "version": 1,
        "subjectType": "proxy-session",
        "subjectHash": subject_hash,
        "currentIp": ip,
        "whitelistRecordId": &whitelist_record_id,
        "expireAt": expire_at,
        "ownerSessionId": session_id,
        "createdAt": now_iso,
        "lastSeenAt": now_iso,
    });
    let login_event = mobility_login_event(ip, ip_location, None);
    let summary = mobility_summary(std::slice::from_ref(&login_event));
    if !state
        .store
        .initialize_auth_mobility_login_session(
            session_id,
            &subject_hash,
            &binding,
            &login_event,
            &summary,
            whitelist_record_id,
            ttl_seconds,
        )
        .await?
    {
        return Ok(false);
    }
    Ok(true)
}

pub(super) async fn record_browser_session_login(
    state: &AppState,
    session_id: &str,
    ip: &str,
    ip_location: Option<&str>,
) -> anyhow::Result<()> {
    record_session_active_ip(RecordSessionActiveIpArgs {
        state,
        session_id,
        session: None,
        client_ip: ip,
        source: "login",
        ip_location,
        whitelist_record_id: None,
        settings: None,
        sync_reason: "browser-session-login",
        schedule_sync: true,
    })
    .await?;
    Ok(())
}

pub(super) async fn record_session_active_ip(
    args: RecordSessionActiveIpArgs<'_>,
) -> anyhow::Result<Option<Value>> {
    let settings = if let Some(settings) = args.settings {
        settings.clone()
    } else {
        let config = args.state.store.get_config().await?;
        AuthCredentialSettings::from_config(&config)
    };
    if !settings.session_ip_mobility_enabled {
        return Ok(None);
    }

    let normalized_ip = normalized_or_trimmed_ip(args.client_ip);
    if normalized_ip.is_empty() {
        return Ok(None);
    }
    let session = if let Some(session) = args.session {
        session.clone()
    } else {
        let Some(session) = args.state.store.get_session(args.session_id).await? else {
            return Ok(None);
        };
        session
    };
    let window_seconds = settings.session_ip_mobility_window_seconds;
    let now = now_seconds();
    let session_expire_at = parse_iso_unix(session.expires_at.as_deref());
    let storage_ttl = resolve_proxy_session_ttl(session_expire_at).unwrap_or(window_seconds);
    if storage_ttl <= 0 {
        return Ok(None);
    }

    let existing_value = args
        .state
        .store
        .get_auth_mobility_active_ip_detail(args.session_id, &normalized_ip)
        .await?;
    let existing = existing_value.clone().and_then(parse_active_ip_detail);
    if active_ip_detail_touch_is_fresh(existing.as_ref(), &session, now, window_seconds) {
        return Ok(existing_value);
    }

    let Some(lease) =
        acquire_auth_mobility_session_mutation_lease(args.state, args.session_id).await?
    else {
        anyhow::bail!("Timed out waiting for auth mobility session mutation lock");
    };
    let result = async {
        let Some(live_session) = args.state.store.get_session(args.session_id).await? else {
            return Ok(None);
        };
        record_session_active_ip_locked(&args, &settings, &normalized_ip, &live_session, &lease)
            .await
    }
    .await;
    if let Err(error) = lease.release().await {
        tracing::warn!(%error, session_id = args.session_id, "failed to release auth mobility session mutation lock");
    }
    result
}

async fn record_session_active_ip_locked(
    args: &RecordSessionActiveIpArgs<'_>,
    settings: &AuthCredentialSettings,
    normalized_ip: &str,
    session: &LoginSession,
    lease: &AuthMobilitySessionMutationLease,
) -> anyhow::Result<Option<Value>> {
    if !lease.ensure_valid().await? {
        return Ok(None);
    }
    let window_seconds = settings.session_ip_mobility_window_seconds;
    let now = now_seconds();
    let session_expire_at = parse_iso_unix(session.expires_at.as_deref());
    let storage_ttl = resolve_proxy_session_ttl(session_expire_at).unwrap_or(window_seconds);
    if storage_ttl <= 0 {
        return Ok(None);
    }

    let existing_value = args
        .state
        .store
        .get_auth_mobility_active_ip_detail(args.session_id, normalized_ip)
        .await?;
    let existing = existing_value.clone().and_then(parse_active_ip_detail);
    if active_ip_detail_touch_is_fresh(existing.as_ref(), session, now, window_seconds) {
        return Ok(existing_value);
    }

    prune_session_active_ips(
        args.state,
        args.session_id,
        session,
        settings,
        now,
        PruneOptions {
            keep_ip: None,
            schedule_sync: args.schedule_sync,
        },
    )
    .await?;

    let active_expire_at = std::cmp::min(
        session_expire_at.unwrap_or(now + window_seconds),
        now + window_seconds,
    );
    let mut whitelist_record_id = existing
        .as_ref()
        .and_then(|value| value.whitelist_record_id.clone())
        .or_else(|| args.whitelist_record_id.map(ToString::to_string));
    let mut auto_owner_key = None::<String>;
    let mut auto_owner_record_key = None::<String>;
    let mut deferred_whitelist = None::<whitelist::DeferredSessionAutoWhitelist>;
    let mut pending_whitelist_record_id = None::<String>;

    if is_follow_session_auto_grant(session) {
        let config = args.state.store.get_config().await?;
        let auto_comment = normalize_auto_ip_grant_comment(session.comment.as_deref(), &config)
            .unwrap_or_else(|| auto_ip_grant_comment(&config));
        let owner_key = format!(
            "auth-mobility:active-ip:{}:{normalized_ip}",
            args.session_id
        );
        let owner_record_key = whitelist::whitelist_auto_owner_record_key(&owner_key);
        let candidate_record_id = format!("whitelist:{}", uuid::Uuid::new_v4());
        if !args
            .state
            .store
            .add_auth_mobility_pending_whitelist(
                args.session_id,
                &candidate_record_id,
                &owner_record_key,
                storage_ttl,
            )
            .await?
        {
            return Ok(None);
        }
        let deferred = match whitelist::ensure_pending_session_auto_whitelist(
            args.state,
            &owner_key,
            normalized_ip,
            Some(active_expire_at),
            Some(auto_comment),
            whitelist_record_id.as_deref(),
            &candidate_record_id,
        )
        .await
        {
            Ok(deferred) => deferred,
            Err(error) => {
                rollback_active_ip_auto_whitelist(
                    args.state,
                    args.session_id,
                    normalized_ip,
                    Some(&owner_key),
                    Some(&candidate_record_id),
                    Some(&candidate_record_id),
                )
                .await;
                return Err(error);
            }
        };
        if deferred.record.id != candidate_record_id {
            if !args
                .state
                .store
                .add_auth_mobility_pending_whitelist(
                    args.session_id,
                    &deferred.record.id,
                    &owner_record_key,
                    storage_ttl,
                )
                .await?
            {
                rollback_active_ip_auto_whitelist(
                    args.state,
                    args.session_id,
                    normalized_ip,
                    Some(&owner_key),
                    Some(&deferred.record.id),
                    Some(&candidate_record_id),
                )
                .await;
                return Ok(None);
            }
            args.state
                .store
                .remove_auth_mobility_pending_whitelist(args.session_id, &candidate_record_id)
                .await?;
        }
        whitelist_record_id = Some(deferred.record.id.clone());
        pending_whitelist_record_id = Some(deferred.record.id.clone());
        deferred_whitelist = Some(deferred);
        auto_owner_key = Some(owner_key);
        auto_owner_record_key = Some(owner_record_key);
    } else {
        whitelist_record_id = None;
    }

    let detail = json!({
        "version": 1,
        "ip": normalized_ip,
        "firstSeenAt": existing
            .as_ref()
            .map(|value| value.first_seen_at)
            .unwrap_or(now),
        "lastSeenAt": now,
        "source": normalize_active_ip_source(args.source),
        "ipLocation": args
            .ip_location
            .filter(|value| !value.trim().is_empty())
            .or_else(|| existing.as_ref().and_then(|value| value.ip_location.as_deref())),
        "whitelistRecordId": whitelist_record_id,
        "autoWhitelistOwnerRecordKey": auto_owner_record_key,
    });
    if !lease.ensure_valid().await? {
        rollback_active_ip_auto_whitelist(
            args.state,
            args.session_id,
            normalized_ip,
            auto_owner_key.as_deref(),
            whitelist_record_id.as_deref(),
            pending_whitelist_record_id.as_deref(),
        )
        .await;
        return Ok(None);
    }
    let saved = args
        .state
        .store
        .save_auth_mobility_active_ip_detail(
            args.session_id,
            normalized_ip,
            now,
            &detail,
            storage_ttl,
        )
        .await;
    let saved = match saved {
        Ok(saved) => saved,
        Err(error) => {
            rollback_active_ip_auto_whitelist(
                args.state,
                args.session_id,
                normalized_ip,
                auto_owner_key.as_deref(),
                whitelist_record_id.as_deref(),
                pending_whitelist_record_id.as_deref(),
            )
            .await;
            return Err(error.into());
        }
    };
    if !saved {
        rollback_active_ip_auto_whitelist(
            args.state,
            args.session_id,
            normalized_ip,
            auto_owner_key.as_deref(),
            whitelist_record_id.as_deref(),
            pending_whitelist_record_id.as_deref(),
        )
        .await;
        return Ok(None);
    }
    if let Some(whitelist_record_id) = whitelist_record_id.as_deref() {
        let owner_saved = args
            .state
            .store
            .set_auth_mobility_whitelist_owner(whitelist_record_id, args.session_id, storage_ttl)
            .await;
        if !matches!(owner_saved, Ok(true)) {
            rollback_active_ip_auto_whitelist(
                args.state,
                args.session_id,
                normalized_ip,
                auto_owner_key.as_deref(),
                Some(whitelist_record_id),
                pending_whitelist_record_id.as_deref(),
            )
            .await;
            return match owner_saved {
                Ok(false) => Ok(None),
                Err(error) => Err(error.into()),
                Ok(true) => unreachable!(),
            };
        }
    }

    if let Some(deferred) = deferred_whitelist {
        if !lease.ensure_valid().await? {
            rollback_active_ip_auto_whitelist(
                args.state,
                args.session_id,
                normalized_ip,
                auto_owner_key.as_deref(),
                whitelist_record_id.as_deref(),
                pending_whitelist_record_id.as_deref(),
            )
            .await;
            return Ok(None);
        }
        if let Err(error) =
            whitelist::publish_deferred_session_auto_whitelist(args.state, deferred).await
        {
            rollback_active_ip_auto_whitelist(
                args.state,
                args.session_id,
                normalized_ip,
                auto_owner_key.as_deref(),
                whitelist_record_id.as_deref(),
                pending_whitelist_record_id.as_deref(),
            )
            .await;
            return Err(error);
        }
        if !lease.ensure_valid().await?
            || args
                .state
                .store
                .get_session(args.session_id)
                .await?
                .is_none()
        {
            rollback_active_ip_auto_whitelist(
                args.state,
                args.session_id,
                normalized_ip,
                auto_owner_key.as_deref(),
                whitelist_record_id.as_deref(),
                pending_whitelist_record_id.as_deref(),
            )
            .await;
            return Ok(None);
        }
        if let Some(record_id) = pending_whitelist_record_id.as_deref() {
            args.state
                .store
                .remove_auth_mobility_pending_whitelist(args.session_id, record_id)
                .await?;
        }
    }

    let removed_count = prune_session_active_ips(
        args.state,
        args.session_id,
        session,
        settings,
        now,
        PruneOptions {
            keep_ip: Some(normalized_ip),
            schedule_sync: false,
        },
    )
    .await?;
    if args.schedule_sync && (existing.is_none() || removed_count > 0) {
        tracing::debug!(
            reason = args.sync_reason,
            "syncing trusted IPs after active IP update"
        );
        whitelist::sync_reverse_proxy_trusted_ips(args.state).await;
    }

    Ok(Some(detail))
}

async fn rollback_active_ip_auto_whitelist(
    state: &AppState,
    session_id: &str,
    ip: &str,
    owner_key: Option<&str>,
    whitelist_record_id: Option<&str>,
    pending_whitelist_record_id: Option<&str>,
) {
    let _ = state
        .store
        .remove_auth_mobility_active_ips(session_id, &[ip.to_string()])
        .await;
    if let (Some(owner_key), Some(whitelist_record_id)) = (owner_key, whitelist_record_id) {
        let _ =
            whitelist::rollback_session_auto_whitelist(state, owner_key, whitelist_record_id).await;
    }
    if let Some(record_id) = pending_whitelist_record_id {
        let _ = state
            .store
            .remove_auth_mobility_pending_whitelist(session_id, record_id)
            .await;
    }
}

pub(super) async fn active_ip_touch_is_fresh_for_session(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    client_ip: &str,
    settings: &AuthCredentialSettings,
) -> anyhow::Result<bool> {
    if !settings.session_ip_mobility_enabled {
        return Ok(true);
    }
    let normalized_ip = normalized_or_trimmed_ip(client_ip);
    if normalized_ip.is_empty() {
        return Ok(true);
    }
    let window_seconds = settings.session_ip_mobility_window_seconds;
    let storage_ttl = resolve_proxy_session_ttl(parse_iso_unix(session.expires_at.as_deref()))
        .unwrap_or(window_seconds);
    if storage_ttl <= 0 {
        return Ok(true);
    }
    let existing = state
        .store
        .get_auth_mobility_active_ip_detail(session_id, &normalized_ip)
        .await?
        .and_then(parse_active_ip_detail);
    Ok(active_ip_detail_touch_is_fresh(
        existing.as_ref(),
        session,
        now_seconds(),
        window_seconds,
    ))
}

fn active_ip_detail_touch_is_fresh(
    existing: Option<&ActiveIpDetail>,
    session: &LoginSession,
    now: i64,
    window_seconds: i64,
) -> bool {
    let touch_interval = AUTH_ACTIVITY_TOUCH_MIN_INTERVAL_SECONDS.min((window_seconds / 2).max(1));
    let expected_whitelist = is_follow_session_auto_grant(session);
    existing.is_some_and(|detail| {
        mobility_touch_is_fresh(detail.last_seen_at, now, touch_interval)
            && detail.whitelist_record_id.is_some() == expected_whitelist
    })
}

pub(super) async fn prune_session_active_ips(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    settings: &AuthCredentialSettings,
    now: i64,
    options: PruneOptions<'_>,
) -> anyhow::Result<usize> {
    let cutoff = now - settings.session_ip_mobility_window_seconds;
    let keep_ip = options.keep_ip.map(normalized_or_trimmed_ip);
    let ips_to_remove = state
        .store
        .collect_auth_mobility_prune_targets(
            session_id,
            cutoff,
            keep_ip.as_deref(),
            MAX_SESSION_ACTIVE_IPS,
        )
        .await?;
    if ips_to_remove.is_empty() {
        return Ok(0);
    }
    let details = state
        .store
        .remove_auth_mobility_active_ips(session_id, &ips_to_remove)
        .await?;
    for detail in details.into_iter().filter_map(parse_active_ip_detail) {
        if let Some(whitelist_record_id) = detail.whitelist_record_id {
            whitelist::remove_whitelist_record_by_id(state, &whitelist_record_id).await?;
        }
    }
    if let Some(ttl) = resolve_proxy_session_ttl(parse_iso_unix(session.expires_at.as_deref())) {
        state
            .store
            .expire_auth_mobility_active_ip_keys(session_id, ttl)
            .await?;
    }
    if options.schedule_sync {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(ips_to_remove.len())
}

pub async fn effective_session_ips(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    settings_value: &Value,
) -> anyhow::Result<Vec<String>> {
    let settings = AuthCredentialSettings::from_config(settings_value);
    if !settings.session_ip_mobility_enabled {
        let ip = normalized_or_trimmed_ip(&session.ip);
        return Ok((!ip.is_empty()).then_some(ip).into_iter().collect());
    }
    let since = now_seconds() - settings.session_ip_mobility_window_seconds + 1;
    let mut ips = state
        .store
        .list_auth_mobility_recent_active_ip_details(session_id, since)
        .await?
        .into_iter()
        .filter_map(parse_active_ip_detail)
        .map(|detail| detail.ip)
        .collect::<Vec<_>>();
    ips.sort();
    ips.dedup();
    Ok(ips)
}

#[derive(Clone, Debug)]
pub(super) struct ActiveIpDetail {
    pub(super) ip: String,
    pub(super) first_seen_at: i64,
    pub(super) last_seen_at: i64,
    pub(super) ip_location: Option<String>,
    pub(super) whitelist_record_id: Option<String>,
}

pub(super) fn parse_active_ip_detail(value: Value) -> Option<ActiveIpDetail> {
    let ip = value
        .get("ip")
        .and_then(Value::as_str)
        .map(normalized_or_trimmed_ip)
        .filter(|value| !value.is_empty())?;
    Some(ActiveIpDetail {
        ip,
        first_seen_at: value
            .get("firstSeenAt")
            .and_then(Value::as_i64)
            .unwrap_or_else(now_seconds),
        last_seen_at: value
            .get("lastSeenAt")
            .and_then(Value::as_i64)
            .unwrap_or_default(),
        ip_location: value
            .get("ipLocation")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
        whitelist_record_id: value
            .get("whitelistRecordId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string),
    })
}
