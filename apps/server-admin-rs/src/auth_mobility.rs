use std::collections::BTreeSet;

use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::time::{self, MissedTickBehavior};

use crate::{
    http_utils::normalize_ip,
    i18n::{DEFAULT_LOCALE, Translator},
    ip_location,
    redis_store::{LoginSession, TotpCredential},
    state::AppState,
    system_events, time_utils, whitelist,
};

const MAX_SESSION_ACTIVE_IPS: usize = 32;
const DEFAULT_SESSION_TTL_SECONDS: i64 = 24 * 3600;
const DEFAULT_REMEMBER_ME_TTL_SECONDS: i64 = 365 * 24 * 3600;
const DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS: i64 = 3600;
const DEFAULT_SESSION_IP_MOBILITY_WINDOW_SECONDS: i64 = 20 * 60;
const MAX_AUTH_TTL_SECONDS: i64 = 5 * 365 * 24 * 3600;
const AUTH_MOBILITY_MAINTENANCE_INTERVAL_SECONDS: u64 = 60;

#[derive(Clone, Debug)]
pub struct CreateLoginSessionInput {
    pub auth_method: String,
    pub auth_provider_name: Option<String>,
    pub credential_id: String,
    pub credential_name: String,
    pub totp_id: String,
    pub linked_totp_name: Option<String>,
    pub totp_credential: Option<TotpCredential>,
    pub client_ip: String,
    pub user_agent: String,
    pub remember_me: bool,
}

#[derive(Clone, Debug)]
pub struct CreatedLoginSession {
    pub session_id: String,
    pub ttl_seconds: i64,
    pub grant_type: String,
    pub expires_at: String,
    pub whitelist_record_id: Option<String>,
    pub post_login_ip_grant_mode: Option<String>,
    pub session_comment: Option<String>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuthMobilityRestoreIdentity<'a> {
    pub session_id: Option<&'a str>,
    pub fnos_token: Option<&'a str>,
    pub trim_media_token: Option<&'a str>,
    pub app_binding: Option<&'a str>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AuthMobilityRestoreResult {
    pub success: bool,
    pub grant_type: Option<&'static str>,
}

#[derive(Clone, Debug)]
struct AuthCredentialSettings {
    session_ttl_seconds: i64,
    remember_me_ttl_seconds: i64,
    post_login_ip_grant_mode: String,
    post_login_ip_grant_ttl_seconds: i64,
    session_ip_mobility_enabled: bool,
    session_ip_mobility_window_seconds: i64,
}

pub fn start_auth_mobility_tasks(state: AppState) {
    tokio::spawn(async move {
        let mut ticker = time::interval(std::time::Duration::from_secs(
            AUTH_MOBILITY_MAINTENANCE_INTERVAL_SECONDS,
        ));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            if let Err(error) = maintain_session_active_ips(&state).await {
                tracing::warn!(%error, "auth mobility active IP maintenance failed");
            }
        }
    });
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
        .redis
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

async fn refresh_proxy_session_binding(
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

async fn refresh_app_token_binding(
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

async fn restore_app_token_binding(
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

async fn restore_anonymous_fnos_app(state: &AppState, client_ip: &str) -> anyhow::Result<bool> {
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

async fn restore_trim_media_app(state: &AppState, client_ip: &str) -> anyhow::Result<bool> {
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

async fn restore_proxy_session(
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

async fn resolve_bootstrap_owner(
    state: &AppState,
    client_ip: &str,
) -> anyhow::Result<Option<(String, LoginSession)>> {
    let sessions = list_active_sessions_by_ip(state, client_ip).await?;
    Ok((sessions.len() == 1)
        .then(|| sessions.into_iter().next())
        .flatten())
}

async fn list_active_sessions_by_ip(
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

pub async fn destroy_sessions_for_totp_credential(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<usize> {
    let sessions = state.redis.list_login_sessions().await?;
    let mut destroyed = 0usize;
    for (session_id, session) in sessions {
        if session.totp_id != totp_id {
            continue;
        }
        destroy_session(state, &session_id).await?;
        state.redis.delete_session(&session_id).await?;
        destroyed += 1;
    }
    if destroyed > 0 {
        whitelist::sync_reverse_proxy_trusted_ips(state).await;
    }
    Ok(destroyed)
}

pub async fn destroy_session(state: &AppState, session_id: &str) -> anyhow::Result<()> {
    let whitelist_ids = state
        .redis
        .destroy_auth_mobility_session(session_id)
        .await?;
    for whitelist_id in whitelist_ids {
        if let Err(error) = whitelist::remove_whitelist_record_by_id(state, &whitelist_id).await {
            tracing::warn!(%error, %session_id, %whitelist_id, "failed to remove mobility whitelist record");
        }
    }
    Ok(())
}

pub async fn clear_auto_ip_grants_for_totp_credential(
    state: &AppState,
    totp_id: &str,
) -> anyhow::Result<bool> {
    let sessions = state.redis.list_login_sessions().await?;
    let mut changed = false;
    for (session_id, session) in sessions {
        if session.totp_id != totp_id {
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
                .redis
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
        .redis
        .get_auth_mobility_binding("proxy-session", session_id)
        .await?
        && let Some(record_id) = binding_whitelist_record_id(&binding)
    {
        record_ids.insert(record_id);
    }
    for detail in state
        .redis
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
    let sessions = state.redis.list_login_sessions().await?;
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

async fn cleanup_session_active_ip_state(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    preserve_legacy_single_slot: bool,
) -> anyhow::Result<()> {
    let details = state
        .redis
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
            let config = state.redis.get_config().await?;
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
            ensure_legacy_proxy_binding(state, session_id, session, &current_ip, &record.id)
                .await?;
            if session.post_login_ip_grant_record_id.as_deref() != Some(record.id.as_str()) {
                let mut updates = Map::new();
                updates.insert(
                    "postLoginIpGrantRecordId".to_string(),
                    Value::String(record.id.clone()),
                );
                state
                    .redis
                    .update_session_value(session_id, updates)
                    .await?;
            }
        }
    }

    state
        .redis
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

async fn ensure_legacy_proxy_binding(
    state: &AppState,
    session_id: &str,
    session: &LoginSession,
    current_ip: &str,
    whitelist_record_id: &str,
) -> anyhow::Result<()> {
    let expire_at = parse_iso_unix(session.expires_at.as_deref());
    let Some(ttl_seconds) = resolve_proxy_session_ttl(expire_at) else {
        return Ok(());
    };
    let existing = state
        .redis
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
    state
        .redis
        .save_auth_mobility_owned_binding(
            "proxy-session",
            session_id,
            &next_binding,
            session_id,
            ttl_seconds,
            Some(ttl_seconds),
        )
        .await?;
    state
        .redis
        .set_auth_mobility_whitelist_owner(whitelist_record_id, session_id, ttl_seconds)
        .await?;
    Ok(())
}

async fn maintain_session_active_ips(state: &AppState) -> anyhow::Result<bool> {
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

async fn register_login_session(
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

async fn record_browser_session_login(
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

struct RecordSessionActiveIpArgs<'a> {
    state: &'a AppState,
    session_id: &'a str,
    session: Option<&'a LoginSession>,
    client_ip: &'a str,
    source: &'a str,
    ip_location: Option<&'a str>,
    whitelist_record_id: Option<&'a str>,
    settings: Option<&'a AuthCredentialSettings>,
    sync_reason: &'a str,
    schedule_sync: bool,
}

async fn record_session_active_ip(
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

struct PruneOptions<'a> {
    keep_ip: Option<&'a str>,
    schedule_sync: bool,
}

async fn prune_session_active_ips(
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
struct ActiveIpDetail {
    ip: String,
    first_seen_at: i64,
    ip_location: Option<String>,
    whitelist_record_id: Option<String>,
}

fn parse_active_ip_detail(value: Value) -> Option<ActiveIpDetail> {
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

impl AuthCredentialSettings {
    fn from_config(config: &Value) -> Self {
        let raw = config
            .get("auth_credential_settings")
            .unwrap_or(&Value::Null);
        Self::from_raw_with_legacy(raw, legacy_auto_add_whitelist_on_login(config))
    }

    fn from_raw(raw: &Value) -> Self {
        Self::from_raw_with_legacy(raw, None)
    }

    fn from_raw_with_legacy(raw: &Value, legacy_auto_add_whitelist_on_login: Option<bool>) -> Self {
        let session_ttl_seconds = bounded_int_like_node(
            raw,
            "session_ttl_seconds",
            DEFAULT_SESSION_TTL_SECONDS,
            60,
            MAX_AUTH_TTL_SECONDS,
        );
        let remember_me_ttl_seconds = bounded_int_like_node(
            raw,
            "remember_me_ttl_seconds",
            DEFAULT_REMEMBER_ME_TTL_SECONDS,
            session_ttl_seconds,
            MAX_AUTH_TTL_SECONDS,
        );
        let post_login_ip_grant_mode =
            match raw.get("post_login_ip_grant_mode").and_then(Value::as_str) {
                Some("disabled") => "disabled",
                Some("custom") => "custom",
                Some("follow_session") => "follow_session",
                _ if legacy_auto_add_whitelist_on_login == Some(false) => "disabled",
                _ => "follow_session",
            }
            .to_string();
        let post_login_ip_grant_ttl_seconds = if post_login_ip_grant_mode == "custom" {
            bounded_int_like_node(
                raw,
                "post_login_ip_grant_ttl_seconds",
                DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS,
                60,
                MAX_AUTH_TTL_SECONDS,
            )
        } else {
            DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS
        };
        let session_ip_mobility_window_seconds = bounded_int_like_node(
            raw,
            "session_ip_mobility_window_seconds",
            DEFAULT_SESSION_IP_MOBILITY_WINDOW_SECONDS,
            60,
            24 * 3600,
        );
        Self {
            session_ttl_seconds,
            remember_me_ttl_seconds,
            post_login_ip_grant_mode,
            post_login_ip_grant_ttl_seconds,
            session_ip_mobility_enabled: raw
                .get("session_ip_mobility_enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            session_ip_mobility_window_seconds,
        }
    }
}

fn legacy_auto_add_whitelist_on_login(config: &Value) -> Option<bool> {
    config
        .pointer("/subdomain_mode/auto_add_whitelist_on_login")
        .and_then(Value::as_bool)
}

fn bounded_int_like_node(value: &Value, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    value
        .get(key)
        .and_then(parse_int_like_node)
        .unwrap_or(fallback)
        .clamp(min, max)
}

fn parse_int_like_node(value: &Value) -> Option<i64> {
    let raw = match value {
        Value::Null => return None,
        Value::Bool(true) => "true".to_string(),
        Value::Bool(false) => "false".to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => return None,
    };
    parse_int_prefix(raw.trim_start())
}

fn parse_int_prefix(value: &str) -> Option<i64> {
    let mut chars = value.chars().peekable();
    let mut sign = 1_i64;
    if let Some(next) = chars.peek().copied() {
        if next == '-' {
            sign = -1;
            chars.next();
        } else if next == '+' {
            chars.next();
        }
    }

    let digits = chars
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i64>().ok().map(|value| value * sign)
}

fn auto_ip_grant_comment(config: &Value) -> String {
    let locale = config
        .pointer("/locale/default_locale")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_LOCALE);
    Translator::new(locale).t("auth.autoIpGrantComment")
}

fn normalize_auto_ip_grant_comment(value: Option<&str>, config: &Value) -> Option<String> {
    let trimmed = value?.trim();
    if trimmed.is_empty() {
        return None;
    }
    if is_auto_ip_grant_comment(trimmed) {
        Some(auto_ip_grant_comment(config))
    } else {
        Some(trimmed.to_string())
    }
}

pub(crate) fn is_auto_ip_grant_comment(value: &str) -> bool {
    matches!(
        value.trim(),
        "登录后自动授权"
            | "登入後自動授權"
            | "Automatically authorized after sign-in"
            | "로그인 후 자동 승인됨"
            | "ログイン後自動認証"
            | "server.auth.autoIpGrantComment"
    )
}

fn is_totp_subdomain_access_restricted(value: &Value) -> bool {
    value.get("mode").and_then(Value::as_str) == Some("custom")
}

fn is_follow_session_auto_grant(session: &LoginSession) -> bool {
    session.grant_type.as_deref() == Some("login_ip_grant")
        && session.post_login_ip_grant_mode.as_deref() == Some("follow_session")
}

fn binding_owner_session_id(value: &Value) -> Option<String> {
    value
        .get("ownerSessionId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn binding_whitelist_record_id(value: &Value) -> Option<String> {
    value
        .get("whitelistRecordId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn clear_binding_owner_session(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.remove("ownerSessionId");
    }
}

fn set_binding_last_seen(value: &mut Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert("lastSeenAt".to_string(), json!(time_utils::now_iso()));
    }
}

fn build_or_update_mobility_binding(
    existing: Option<Value>,
    subject_type: &str,
    subject_key: &str,
    current_ip: &str,
    expire_at: Option<i64>,
    owner_session_id: Option<&str>,
    whitelist_record_id: Option<String>,
) -> Value {
    let now_iso = time_utils::now_iso();
    let mut value = existing
        .filter(Value::is_object)
        .unwrap_or_else(|| json!({}));
    let object = value.as_object_mut().expect("binding object");
    object.insert("version".to_string(), json!(1));
    object.insert(
        "subjectType".to_string(),
        Value::String(subject_type.to_string()),
    );
    object.insert(
        "subjectHash".to_string(),
        Value::String(auth_mobility_subject_hash(subject_type, subject_key)),
    );
    object.insert(
        "currentIp".to_string(),
        Value::String(current_ip.to_string()),
    );
    object.insert(
        "expireAt".to_string(),
        expire_at.map_or(Value::Null, |value| json!(value)),
    );
    object
        .entry("createdAt".to_string())
        .or_insert_with(|| Value::String(now_iso.clone()));
    object.insert("lastSeenAt".to_string(), Value::String(now_iso));
    if let Some(owner_session_id) = owner_session_id.filter(|value| !value.trim().is_empty()) {
        object.insert(
            "ownerSessionId".to_string(),
            Value::String(owner_session_id.to_string()),
        );
    } else {
        object.remove("ownerSessionId");
    }
    if let Some(whitelist_record_id) = whitelist_record_id.filter(|value| !value.trim().is_empty())
    {
        object.insert(
            "whitelistRecordId".to_string(),
            Value::String(whitelist_record_id),
        );
    } else {
        object.remove("whitelistRecordId");
    }
    value
}

fn normalized_or_trimmed_ip(value: &str) -> String {
    let normalized = normalize_ip(value);
    if normalized.is_empty() {
        value.trim().to_string()
    } else {
        normalized
    }
}

async fn cached_ip_location(state: &AppState, ip: &str) -> Option<String> {
    if ip.is_empty() {
        return None;
    }
    state
        .redis
        .get_ip_location_cache(ip)
        .await
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("raw")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

fn auth_mobility_subject_hash(subject_type: &str, subject_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{subject_type}:{subject_key}"));
    hex::encode(hasher.finalize())
}

fn mobility_login_event(ip: &str, ip_location: Option<&str>, happened_at: Option<&str>) -> Value {
    json!({
        "version": 1,
        "kind": "login",
        "happenedAt": happened_at.map(ToString::to_string).unwrap_or_else(time_utils::now_iso),
        "source": "login",
        "toIp": ip,
        "toIpLocation": ip_location.filter(|value| !value.trim().is_empty()),
    })
}

fn mobility_drift_event(
    source: &str,
    from_ip: &str,
    from_ip_location: Option<&str>,
    to_ip: &str,
    to_ip_location: Option<&str>,
) -> Value {
    json!({
        "version": 1,
        "kind": "drift",
        "happenedAt": time_utils::now_iso(),
        "source": normalize_drift_source(source),
        "fromIp": from_ip,
        "fromIpLocation": from_ip_location.filter(|value| !value.trim().is_empty()),
        "toIp": to_ip,
        "toIpLocation": to_ip_location.filter(|value| !value.trim().is_empty()),
    })
}

fn mobility_summary(events: &[Value]) -> Value {
    let drift_events = events
        .iter()
        .filter(|event| event.get("kind").and_then(Value::as_str) == Some("drift"))
        .collect::<Vec<_>>();
    let last_drift = drift_events.last().copied();
    json!({
        "hasHistory": !events.is_empty(),
        "driftCount": drift_events.len(),
        "lastDriftAt": last_drift
            .and_then(|event| event.get("happenedAt"))
            .and_then(Value::as_str),
        "lastDriftSource": last_drift
            .and_then(|event| event.get("source"))
            .and_then(Value::as_str),
    })
}

fn normalize_active_ip_source(value: &str) -> &str {
    match value {
        "login" | "proxy-session" | "fnos-token" | "session-refresh" | "browser-session" => value,
        _ => "session-refresh",
    }
}

fn normalize_drift_source(value: &str) -> &str {
    match value {
        "proxy-session" | "fnos-token" | "session-refresh" | "browser-session" => value,
        _ => "session-refresh",
    }
}

fn parse_iso_unix(value: Option<&str>) -> Option<i64> {
    value
        .and_then(time_utils::parse_iso_ms)
        .map(|ms| ms.div_euclid(1000))
}

fn resolve_proxy_session_ttl(expire_at: Option<i64>) -> Option<i64> {
    let remaining = expire_at? - now_seconds();
    (remaining > 0).then_some(remaining)
}

fn now_seconds() -> i64 {
    time_utils::now_ms().div_euclid(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_credential_settings_runtime_normalizes_like_node() {
        let settings = AuthCredentialSettings::from_config(&json!({
            "subdomain_mode": { "auto_add_whitelist_on_login": false },
            "auth_credential_settings": {
                "session_ttl_seconds": "59.8",
                "remember_me_ttl_seconds": "10",
                "post_login_ip_grant_ttl_seconds": "10",
                "session_ip_mobility_window_seconds": "90000"
            }
        }));
        assert_eq!(settings.session_ttl_seconds, 60);
        assert_eq!(settings.remember_me_ttl_seconds, 60);
        assert_eq!(settings.post_login_ip_grant_mode, "disabled");
        assert_eq!(
            settings.post_login_ip_grant_ttl_seconds,
            DEFAULT_POST_LOGIN_IP_GRANT_TTL_SECONDS
        );
        assert_eq!(settings.session_ip_mobility_window_seconds, 86_400);

        let custom = AuthCredentialSettings::from_raw(&json!({
            "session_ttl_seconds": 120,
            "remember_me_ttl_seconds": 240,
            "post_login_ip_grant_mode": "custom",
            "post_login_ip_grant_ttl_seconds": 3.14,
            "session_ip_mobility_window_seconds": "30"
        }));
        assert_eq!(custom.post_login_ip_grant_mode, "custom");
        assert_eq!(custom.post_login_ip_grant_ttl_seconds, 60);
        assert_eq!(custom.session_ip_mobility_window_seconds, 60);
    }

    #[test]
    fn mobility_binding_builder_preserves_and_clears_node_fields() {
        let original = json!({
            "createdAt": "2026-01-01T00:00:00.000Z",
            "ownerSessionId": "old-session",
            "whitelistRecordId": "old-whitelist",
            "custom": true
        });

        let owned = build_or_update_mobility_binding(
            Some(original),
            "fnos-token",
            "secret-token",
            "203.0.113.10",
            Some(1_800_000_000),
            Some("session-1"),
            Some("whitelist-1".to_string()),
        );

        assert_eq!(owned.get("version").and_then(Value::as_i64), Some(1));
        assert_eq!(
            owned.get("subjectType").and_then(Value::as_str),
            Some("fnos-token")
        );
        let expected_hash = auth_mobility_subject_hash("fnos-token", "secret-token");
        assert_eq!(
            owned.get("subjectHash").and_then(Value::as_str),
            Some(expected_hash.as_str())
        );
        assert_eq!(
            owned.get("currentIp").and_then(Value::as_str),
            Some("203.0.113.10")
        );
        assert_eq!(
            owned.get("expireAt").and_then(Value::as_i64),
            Some(1_800_000_000)
        );
        assert_eq!(
            owned.get("ownerSessionId").and_then(Value::as_str),
            Some("session-1")
        );
        assert_eq!(
            owned.get("whitelistRecordId").and_then(Value::as_str),
            Some("whitelist-1")
        );
        assert_eq!(
            owned.get("createdAt").and_then(Value::as_str),
            Some("2026-01-01T00:00:00.000Z")
        );
        assert_eq!(owned.get("custom").and_then(Value::as_bool), Some(true));
        assert!(owned.get("lastSeenAt").and_then(Value::as_str).is_some());

        let cleared = build_or_update_mobility_binding(
            Some(owned),
            "fnos-token",
            "secret-token",
            "203.0.113.11",
            None,
            None,
            None,
        );
        assert!(cleared.get("ownerSessionId").is_none());
        assert!(cleared.get("whitelistRecordId").is_none());
        assert!(cleared.get("expireAt").is_some_and(Value::is_null));
        assert_eq!(
            cleared.get("createdAt").and_then(Value::as_str),
            Some("2026-01-01T00:00:00.000Z")
        );

        let mut orphaned = cleared;
        clear_binding_owner_session(&mut orphaned);
        set_binding_last_seen(&mut orphaned);
        assert!(orphaned.get("ownerSessionId").is_none());
        assert!(orphaned.get("lastSeenAt").and_then(Value::as_str).is_some());
    }
}
