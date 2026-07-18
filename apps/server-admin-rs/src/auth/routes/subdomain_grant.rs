use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{normalize_subdomain_access_host, resolve_request_hostname_from_headers};
use crate::grpc_proto::SubdomainRuleMatch;
use crate::{auth::cookies, crypto_utils, state::AppState, time_utils};

pub(crate) const COOKIE_NAME: &str = "fn-knock-subdomain-rule-grant";
const KEY_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant:";
const ACTIVE_INDEX_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant_active:";
const RATE_KEY_PREFIX: &str = "fn_knock:auth:subdomain_rule_rate:";
const RENEWAL_GRANULARITY_SECONDS: i64 = 60;
const RATE_WINDOW_SECONDS: i64 = 60;
const RATE_PER_CLIENT: i64 = 10;
const RATE_PER_HOST: i64 = 1_000;
const MAX_ACTIVE_GRANTS_PER_HOST: i64 = 100_000;
pub(crate) const RATE_LIMITED_ERROR: &str = "subdomain_rule_rate_limited";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GrantRecord {
    host: String,
    policy_version: String,
    group_id: String,
    issued_at: i64,
    last_access_at: i64,
    hard_expires_at: i64,
}

#[derive(Debug, Clone)]
pub(crate) struct GrantAccess {
    pub(crate) set_cookie: Option<String>,
    pub(crate) group_id: String,
    pub(crate) state: &'static str,
    pub(crate) cache_max_age_seconds: i64,
}

pub(crate) fn is_rate_limited(error: &anyhow::Error) -> bool {
    error.root_cause().to_string() == RATE_LIMITED_ERROR
}

fn key(token: &str) -> String {
    format!("{KEY_PREFIX}{}", crypto_utils::sha256_hex_str(token))
}

fn active_index_key(host: &str) -> String {
    format!(
        "{ACTIVE_INDEX_PREFIX}{}",
        crypto_utils::sha256_hex_str(host)
    )
}

fn now_seconds() -> i64 {
    time_utils::now_ms().div_euclid(1000)
}

fn mapping_policy(config: &Value, host: &str, policy_version: &str, group_id: &str) -> bool {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|mapping| {
            normalize_subdomain_access_host(
                mapping.get("host").and_then(Value::as_str).unwrap_or(""),
            ) == host
        })
        .filter(|mapping| {
            mapping
                .get("use_auth")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        })
        .and_then(|mapping| mapping.get("advanced_auth"))
        .is_some_and(|policy| {
            policy
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
                && policy.get("policy_version").and_then(Value::as_str) == Some(policy_version)
                && policy
                    .get("groups")
                    .and_then(Value::as_array)
                    .is_some_and(|groups| {
                        groups
                            .iter()
                            .any(|group| group.get("id").and_then(Value::as_str) == Some(group_id))
                    })
        })
}

pub(crate) fn match_is_valid(
    config: &Value,
    current_host: &str,
    matched: &SubdomainRuleMatch,
) -> bool {
    let host = normalize_subdomain_access_host(&matched.host);
    let current_host = normalize_subdomain_access_host(current_host);
    host == current_host
        && !host.is_empty()
        && !matched.policy_version.trim().is_empty()
        && !matched.group_id.trim().is_empty()
        && mapping_policy(
            config,
            &current_host,
            matched.policy_version.trim(),
            matched.group_id.trim(),
        )
}

pub(crate) async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    matched: Option<&SubdomainRuleMatch>,
) -> anyhow::Result<Option<GrantAccess>> {
    if let Some(access) = authorize_existing(state, headers, config, true).await? {
        return Ok(Some(access));
    }

    let Some(host) = resolve_request_hostname_from_headers(headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let now = now_seconds();

    let Some(matched) = matched.filter(|value| match_is_valid(config, &host, value)) else {
        return Ok(None);
    };
    enforce_issue_rate_limit(state, headers, &host).await?;
    let (idle, hard) = policy_ttls(config, &host);
    let token =
        crate::crypto_utils::sha256_base64_url_no_pad(crate::crypto_utils::random_bytes::<32>());
    let record = GrantRecord {
        host: host.clone(),
        policy_version: matched.policy_version.trim().to_string(),
        group_id: matched.group_id.trim().to_string(),
        issued_at: now,
        last_access_at: now,
        hard_expires_at: now.saturating_add(hard),
    };
    let encoded = serde_json::to_string(&record)?;
    let grant_key = key(&token);
    let ttl = idle.min(hard).max(1);
    let stored = state
        .store
        .set_expiring_string_with_zset_limit(
            &grant_key,
            &encoded,
            ttl,
            &active_index_key(&host),
            now,
            now.saturating_add(ttl),
            MAX_ACTIVE_GRANTS_PER_HOST,
        )
        .await?;
    if !stored {
        anyhow::bail!(RATE_LIMITED_ERROR);
    }
    Ok(Some(GrantAccess {
        set_cookie: Some(cookies::build_cookie(
            COOKIE_NAME,
            &token,
            ttl,
            "/",
            None,
            true,
            cookie_secure(),
            "Lax",
        )),
        group_id: matched.group_id.trim().to_string(),
        state: "issued",
        cache_max_age_seconds: bounded_cache_max_age(idle.min(hard)),
    }))
}

/// Validate an already issued host-scoped grant without sliding it. Preflight
/// calls this read-only path because only the verify response is delivered
/// back to the browser with Set-Cookie headers.
pub(crate) async fn inspect_existing(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
) -> anyhow::Result<Option<GrantAccess>> {
    authorize_existing(state, headers, config, false).await
}

async fn authorize_existing(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    renew: bool,
) -> anyhow::Result<Option<GrantAccess>> {
    let Some(host) = resolve_request_hostname_from_headers(headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let Some(token) = cookies::read_cookie(headers, COOKIE_NAME).filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let grant_key = key(&token);
    let Some(raw) = state.store.get_string_value(&grant_key).await? else {
        return Ok(None);
    };
    let Ok(mut record) = serde_json::from_str::<GrantRecord>(&raw) else {
        if renew {
            let _ = state.store.delete_key(&grant_key).await;
        }
        return Ok(None);
    };
    let now = now_seconds();
    let idle = policy_ttls(config, &host).0;
    let elapsed_idle = now.saturating_sub(record.last_access_at);
    let valid = record.host == host
        && record.hard_expires_at > now
        && record.last_access_at > 0
        && elapsed_idle < idle
        && mapping_policy(config, &host, &record.policy_version, &record.group_id);
    if !valid {
        if renew {
            let _ = state
                .store
                .delete_string_and_zrem(&grant_key, &active_index_key(&record.host), &grant_key)
                .await;
        }
        return Ok(None);
    }

    let hard_remaining = record.hard_expires_at.saturating_sub(now);
    let idle_remaining = idle.saturating_sub(elapsed_idle);
    let should_renew = renew && elapsed_idle >= RENEWAL_GRANULARITY_SECONDS;
    let effective_remaining = if should_renew {
        hard_remaining.min(idle)
    } else {
        hard_remaining.min(idle_remaining)
    }
    .max(1);
    if should_renew {
        record.last_access_at = now;
        let encoded = serde_json::to_string(&record)?;
        let stored = state
            .store
            .set_expiring_string_with_zset_limit(
                &grant_key,
                &encoded,
                effective_remaining,
                &active_index_key(&record.host),
                now,
                now.saturating_add(effective_remaining),
                MAX_ACTIVE_GRANTS_PER_HOST,
            )
            .await?;
        if !stored {
            anyhow::bail!(RATE_LIMITED_ERROR);
        }
    }
    Ok(Some(GrantAccess {
        set_cookie: should_renew.then(|| {
            cookies::build_cookie(
                COOKIE_NAME,
                &token,
                effective_remaining,
                "/",
                None,
                true,
                cookie_secure(),
                "Lax",
            )
        }),
        group_id: record.group_id,
        state: if should_renew { "renewed" } else { "reused" },
        cache_max_age_seconds: bounded_cache_max_age(effective_remaining),
    }))
}

fn bounded_cache_max_age(remaining_seconds: i64) -> i64 {
    RENEWAL_GRANULARITY_SECONDS.min(remaining_seconds).max(1)
}

async fn enforce_issue_rate_limit(
    state: &AppState,
    headers: &HeaderMap,
    host: &str,
) -> anyhow::Result<()> {
    let client_ip = super::client_ip_for_auth(headers);
    let host_hash = crypto_utils::sha256_hex_str(host);
    let client_hash = crypto_utils::sha256_hex_str(&format!("{host}\n{client_ip}"));
    let host_key = format!("{RATE_KEY_PREFIX}host:{host_hash}");
    let client_key = format!("{RATE_KEY_PREFIX}client:{client_hash}");
    let host_count = state
        .store
        .increment_counter_with_ttl(&host_key, RATE_WINDOW_SECONDS)
        .await?;
    if host_count > RATE_PER_HOST {
        anyhow::bail!(RATE_LIMITED_ERROR);
    }
    let client_count = state
        .store
        .increment_counter_with_ttl(&client_key, RATE_WINDOW_SECONDS)
        .await?;
    if client_count > RATE_PER_CLIENT {
        anyhow::bail!(RATE_LIMITED_ERROR);
    }
    Ok(())
}

fn policy_ttls(config: &Value, host: &str) -> (i64, i64) {
    let policy = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|mapping| {
            normalize_subdomain_access_host(
                mapping.get("host").and_then(Value::as_str).unwrap_or(""),
            ) == host
        })
        .and_then(|mapping| mapping.get("advanced_auth"));
    let idle = policy
        .and_then(|p| p.get("idle_ttl_seconds"))
        .and_then(Value::as_i64)
        .unwrap_or(86_400);
    let hard = policy
        .and_then(|p| p.get("max_lifetime_seconds"))
        .and_then(Value::as_i64)
        .unwrap_or(2_592_000);
    (idle.max(300), hard.max(idle).max(300))
}

fn cookie_secure() -> bool {
    std::env::var("SESSION_COOKIE_SECURE")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true)
}

pub(crate) async fn revoke(state: &AppState, headers: &HeaderMap) -> anyhow::Result<()> {
    if let Some(token) = cookies::read_cookie(headers, COOKIE_NAME) {
        let grant_key = key(&token);
        let record = state
            .store
            .get_string_value(&grant_key)
            .await?
            .and_then(|raw| serde_json::from_str::<GrantRecord>(&raw).ok());
        if let Some(record) = record {
            state
                .store
                .delete_string_and_zrem(&grant_key, &active_index_key(&record.host), &grant_key)
                .await?;
        } else {
            state.store.delete_key(&grant_key).await?;
        }
    }
    Ok(())
}

pub(crate) fn clear_cookie() -> String {
    cookies::subdomain_rule_clear_cookie()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_cache_never_outlives_remaining_credential_time() {
        assert_eq!(bounded_cache_max_age(3_600), 60);
        assert_eq!(bounded_cache_max_age(17), 17);
        assert_eq!(bounded_cache_max_age(0), 1);
    }

    #[test]
    fn match_validation_is_host_policy_and_group_scoped() {
        let config = serde_json::json!({
            "host_mappings": [{
                "host": "APP.EXAMPLE.COM.",
                "use_auth": true,
                "advanced_auth": {
                    "enabled": true,
                    "policy_version": "version-1",
                    "groups": [{"id": "group-1", "conditions": []}]
                }
            }]
        });
        let matched = SubdomainRuleMatch {
            host: "app.example.com".to_string(),
            policy_version: "version-1".to_string(),
            group_id: "group-1".to_string(),
        };
        assert!(match_is_valid(&config, "app.example.com", &matched));
        assert!(!match_is_valid(&config, "other.example.com", &matched));
        let stale = SubdomainRuleMatch {
            policy_version: "stale".to_string(),
            ..matched
        };
        assert!(!match_is_valid(&config, "app.example.com", &stale));
    }
}
