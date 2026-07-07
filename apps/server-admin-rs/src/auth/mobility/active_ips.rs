use super::*;

pub(super) async fn maintain_session_active_ips(state: &AppState) -> anyhow::Result<bool> {
    let config = state.redis.get_config().await?;
    let settings = AuthCredentialSettings::from_config(&config);
    if !settings.session_ip_mobility_enabled {
        return Ok(false);
    }

    let now = now_seconds();
    let sessions = state.redis.list_login_sessions().await?;
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
    whitelist_record_id: &str,
    expire_at: Option<i64>,
) -> anyhow::Result<()> {
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(());
    };
    let subject_hash = auth_mobility_subject_hash("proxy-session", session_id);
    let now_iso = time_utils::now_iso();
    let binding = json!({
        "version": 1,
        "subjectType": "proxy-session",
        "subjectHash": subject_hash,
        "currentIp": ip,
        "whitelistRecordId": whitelist_record_id,
        "expireAt": expire_at,
        "ownerSessionId": session_id,
        "createdAt": now_iso,
        "lastSeenAt": now_iso,
    });
    let login_event = mobility_login_event(ip, ip_location, None);
    let summary = mobility_summary(&[login_event.clone()]);
    state
        .redis
        .initialize_auth_mobility_login_session(
            session_id,
            &subject_hash,
            &binding,
            &login_event,
            &summary,
            whitelist_record_id,
            ttl_seconds,
        )
        .await?;
    record_session_active_ip(RecordSessionActiveIpArgs {
        state,
        session_id,
        session: None,
        client_ip: ip,
        source: "login",
        ip_location,
        whitelist_record_id: Some(whitelist_record_id),
        settings: None,
        sync_reason: "mobility-login-session",
        schedule_sync: true,
    })
    .await?;
    Ok(())
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
        let config = args.state.redis.get_config().await?;
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
        let Some(session) = args.state.redis.get_session(args.session_id).await? else {
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

    prune_session_active_ips(
        args.state,
        args.session_id,
        &session,
        &settings,
        now,
        PruneOptions {
            keep_ip: None,
            schedule_sync: args.schedule_sync,
        },
    )
    .await?;

    let existing = args
        .state
        .redis
        .get_auth_mobility_active_ip_detail(args.session_id, &normalized_ip)
        .await?
        .and_then(parse_active_ip_detail);
    let active_expire_at = std::cmp::min(
        session_expire_at.unwrap_or(now + window_seconds),
        now + window_seconds,
    );
    let mut whitelist_record_id = existing
        .as_ref()
        .and_then(|value| value.whitelist_record_id.clone())
        .or_else(|| args.whitelist_record_id.map(ToString::to_string));

    if is_follow_session_auto_grant(&session) {
        let config = args.state.redis.get_config().await?;
        let auto_comment = normalize_auto_ip_grant_comment(session.comment.as_deref(), &config)
            .unwrap_or_else(|| auto_ip_grant_comment(&config));
        let record = whitelist::ensure_session_auto_whitelist(
            args.state,
            &format!(
                "auth-mobility:active-ip:{}:{normalized_ip}",
                args.session_id
            ),
            &normalized_ip,
            Some(active_expire_at),
            Some(auto_comment),
            whitelist_record_id.as_deref(),
        )
        .await?;
        whitelist_record_id = Some(record.id.clone());
        args.state
            .redis
            .set_auth_mobility_whitelist_owner(&record.id, args.session_id, storage_ttl)
            .await?;
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
    });
    args.state
        .redis
        .save_auth_mobility_active_ip_detail(
            args.session_id,
            &normalized_ip,
            now,
            &detail,
            storage_ttl,
        )
        .await?;

    let removed_count = prune_session_active_ips(
        args.state,
        args.session_id,
        &session,
        &settings,
        now,
        PruneOptions {
            keep_ip: Some(&normalized_ip),
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
        .redis
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
        .redis
        .remove_auth_mobility_active_ips(session_id, &ips_to_remove)
        .await?;
    for detail in details.into_iter().filter_map(parse_active_ip_detail) {
        if let Some(whitelist_record_id) = detail.whitelist_record_id {
            whitelist::remove_whitelist_record_by_id(state, &whitelist_record_id).await?;
        }
    }
    if let Some(ttl) = resolve_proxy_session_ttl(parse_iso_unix(session.expires_at.as_deref())) {
        state
            .redis
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
        .redis
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
