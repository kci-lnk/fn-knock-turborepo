use axum::http::{HeaderMap, header};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use subtle::ConstantTimeEq;

use super::{normalize_subdomain_access_host, resolve_request_hostname_from_headers};
use crate::grpc_proto::SubdomainRuleMatch;
use crate::{auth::cookies, crypto_utils, state::AppState, time_utils};

pub(crate) const COOKIE_NAME: &str = "fn-knock-subdomain-rule-grant";
const KEY_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant:";
const ACTIVE_INDEX_PREFIX: &str = "fn_knock:auth:subdomain_rule_grant_active:";
const RATE_KEY_PREFIX: &str = "fn_knock:auth:subdomain_rule_rate:";
const RENEWAL_GRANULARITY_SECONDS: i64 = 60;
const RATE_WINDOW_SECONDS: i64 = 60;
const PROBE_PREFIX: &str = "p1";
const PROBE_TTL_SECONDS: i64 = 300;
const PROBE_MAX_VALUE_BYTES: usize = 1_024;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProbeClaims {
    host_hash: String,
    policy_hash: String,
    group_hash: String,
    client_hash: String,
    issued_at: i64,
    expires_at: i64,
    nonce: String,
}

#[derive(Debug, Clone)]
struct ValidatedProbe {
    matched: SubdomainRuleMatch,
    issued_at: i64,
    value: String,
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

fn request_has_upgrade(headers: &HeaderMap) -> bool {
    headers
        .get_all(header::UPGRADE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| !value.trim().is_empty())
}

fn transient_access(matched: &SubdomainRuleMatch, set_cookie: Option<String>) -> GrantAccess {
    GrantAccess {
        set_cookie,
        group_id: matched.group_id.trim().to_string(),
        state: "transient",
        cache_max_age_seconds: 0,
    }
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

fn probe_client_hash(headers: &HeaderMap, host: &str) -> String {
    crypto_utils::sha256_hex_str(&format!("{host}\n{}", super::client_ip_for_auth(headers)))
}

fn presented_probe_value(headers: &HeaderMap) -> Option<String> {
    cookies::read_cookie(headers, COOKIE_NAME).filter(|value| {
        value.len() <= PROBE_MAX_VALUE_BYTES && value.starts_with(&format!("{PROBE_PREFIX}."))
    })
}

fn probe_signature(secret: &str, payload: &str) -> Vec<u8> {
    crypto_utils::hmac_sha256_bytes(
        secret.as_bytes(),
        format!("fn-knock-subdomain-rule-probe-v1\0{payload}").as_bytes(),
    )
}

fn build_probe_cookie(
    state: &AppState,
    headers: &HeaderMap,
    matched: &SubdomainRuleMatch,
) -> Option<String> {
    let secret = state.settings.internal_rpc_token.trim();
    let host = normalize_subdomain_access_host(&matched.host);
    if secret.is_empty()
        || host.is_empty()
        || matched.policy_version.trim().is_empty()
        || matched.group_id.trim().is_empty()
    {
        return None;
    }
    let now = now_seconds();
    let claims = ProbeClaims {
        host_hash: crypto_utils::sha256_hex_str(&host),
        policy_hash: crypto_utils::sha256_hex_str(matched.policy_version.trim()),
        group_hash: crypto_utils::sha256_hex_str(matched.group_id.trim()),
        client_hash: probe_client_hash(headers, &host),
        issued_at: now,
        expires_at: now.saturating_add(PROBE_TTL_SECONDS),
        nonce: crypto_utils::sha256_base64_url_no_pad(crypto_utils::random_bytes::<16>()),
    };
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).ok()?);
    let signature = URL_SAFE_NO_PAD.encode(probe_signature(secret, &payload));
    let value = format!("{PROBE_PREFIX}.{payload}.{signature}");
    (value.len() <= PROBE_MAX_VALUE_BYTES).then(|| {
        cookies::build_cookie(
            COOKIE_NAME,
            &value,
            PROBE_TTL_SECONDS,
            "/",
            None,
            true,
            cookie_secure(),
            "Lax",
        )
    })
}

fn probe_match_from_claims(
    config: &Value,
    current_host: &str,
    claims: &ProbeClaims,
) -> Option<SubdomainRuleMatch> {
    let mapping = config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| {
            normalize_subdomain_access_host(
                mapping.get("host").and_then(Value::as_str).unwrap_or(""),
            ) == current_host
        })?;
    if !mapping
        .get("use_auth")
        .and_then(Value::as_bool)
        .unwrap_or(true)
    {
        return None;
    }
    let policy = mapping.get("advanced_auth")?;
    if !policy
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }
    let policy_version = policy.get("policy_version").and_then(Value::as_str)?.trim();
    if crypto_utils::sha256_hex_str(policy_version) != claims.policy_hash {
        return None;
    }
    let group_id = policy
        .get("groups")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|group| group.get("id").and_then(Value::as_str))
        .map(str::trim)
        .find(|group_id| crypto_utils::sha256_hex_str(group_id) == claims.group_hash)?;
    Some(SubdomainRuleMatch {
        host: current_host.to_string(),
        policy_version: policy_version.to_string(),
        group_id: group_id.to_string(),
    })
}

fn validate_probe(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    current_host: &str,
) -> Option<ValidatedProbe> {
    let secret = state.settings.internal_rpc_token.trim();
    if secret.is_empty() {
        return None;
    }
    let value = presented_probe_value(headers)?;
    let mut parts = value.split('.');
    if parts.next()? != PROBE_PREFIX {
        return None;
    }
    let payload = parts.next()?;
    let signature = URL_SAFE_NO_PAD.decode(parts.next()?).ok()?;
    if parts.next().is_some() || signature.len() != 32 {
        return None;
    }
    let expected = probe_signature(secret, payload);
    if !bool::from(expected.as_slice().ct_eq(signature.as_slice())) {
        return None;
    }
    let claims =
        serde_json::from_slice::<ProbeClaims>(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    let now = now_seconds();
    if claims.issued_at > now
        || claims.expires_at <= now
        || claims.expires_at < claims.issued_at
        || claims.expires_at.saturating_sub(claims.issued_at) > PROBE_TTL_SECONDS
        || claims.nonce.is_empty()
        || claims.nonce.len() > 64
        || claims.host_hash != crypto_utils::sha256_hex_str(current_host)
        || claims.client_hash != probe_client_hash(headers, current_host)
    {
        return None;
    }
    let matched = probe_match_from_claims(config, current_host, &claims)?;
    Some(ValidatedProbe {
        matched,
        issued_at: claims.issued_at,
        value,
    })
}

fn probe_grant_token(state: &AppState, probe_value: &str) -> String {
    URL_SAFE_NO_PAD.encode(crypto_utils::hmac_sha256_bytes(
        state.settings.internal_rpc_token.trim().as_bytes(),
        format!("fn-knock-subdomain-rule-grant-v1\0{probe_value}").as_bytes(),
    ))
}

pub(crate) fn has_valid_probe(state: &AppState, headers: &HeaderMap, config: &Value) -> bool {
    resolve_request_hostname_from_headers(headers)
        .map(|host| normalize_subdomain_access_host(&host))
        .filter(|host| !host.is_empty())
        .is_some_and(|host| validate_probe(state, headers, config, &host).is_some())
}

pub(crate) async fn authorize(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    matched: Option<&SubdomainRuleMatch>,
) -> anyhow::Result<Option<GrantAccess>> {
    let transient_upgrade = request_has_upgrade(headers);
    let presented_probe = presented_probe_value(headers).is_some();
    let existing_error = if presented_probe {
        None
    } else {
        match authorize_existing(state, headers, config, !transient_upgrade).await {
            Ok(Some(access)) => return Ok(Some(access)),
            Ok(None) => None,
            Err(error) => Some(error),
        }
    };

    let Some(host) = resolve_request_hostname_from_headers(headers)
        .map(|value| normalize_subdomain_access_host(&value))
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    let current_match = matched.filter(|value| match_is_valid(config, &host, value));
    let probe = validate_probe(state, headers, config, &host);
    let effective_match = probe.as_ref().map(|probe| &probe.matched).or(current_match);
    let Some(effective_match) = effective_match else {
        if let Some(error) = existing_error {
            return Err(error);
        }
        return Ok(None);
    };
    if let Some(error) = existing_error {
        tracing::warn!(%error, %host, "subdomain rule credential lookup failed; using transient access");
        return Ok(Some(transient_access(
            effective_match,
            current_match.and_then(|matched| build_probe_cookie(state, headers, matched)),
        )));
    }
    if transient_upgrade {
        // Upgrade handshakes are authorized for this request only. A 101
        // response is not a reliable cookie-delivery channel, and persisting a
        // fresh grant on every reconnect would consume the issuance limiter.
        return Ok(Some(transient_access(effective_match, None)));
    }

    let Some(probe) = probe else {
        // A matching request is already authorized. Send only a signed,
        // host-scoped capability probe; persistent storage is created after a
        // later request proves that the client actually returns cookies.
        return Ok(Some(transient_access(
            effective_match,
            build_probe_cookie(state, headers, effective_match),
        )));
    };

    let now = now_seconds();
    let (idle, hard) = policy_ttls(config, &host);
    let hard_expires_at = probe.issued_at.saturating_add(hard);
    let ttl = idle.min(hard_expires_at.saturating_sub(now)).max(1);
    if let Err(error) = enforce_issue_rate_limit(state, headers, &host).await {
        tracing::warn!(%error, %host, "subdomain rule persistent issue unavailable; using transient access");
        return Ok(Some(transient_access(&probe.matched, None)));
    }
    let token = probe_grant_token(state, &probe.value);
    let record = GrantRecord {
        host: host.clone(),
        policy_version: probe.matched.policy_version.trim().to_string(),
        group_id: probe.matched.group_id.trim().to_string(),
        issued_at: probe.issued_at,
        last_access_at: now,
        hard_expires_at,
    };
    let encoded = serde_json::to_string(&record)?;
    let grant_key = key(&token);
    let stored = match state
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
        .await
    {
        Ok(stored) => stored,
        Err(error) => {
            tracing::warn!(%error, %host, "subdomain rule credential storage failed; using transient access");
            return Ok(Some(transient_access(&probe.matched, None)));
        }
    };
    if !stored {
        // Capacity protection applies to persistent credential state, not to
        // the already validated rule decision for this individual request.
        return Ok(Some(transient_access(&probe.matched, None)));
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
        group_id: probe.matched.group_id.trim().to_string(),
        state: "issued",
        cache_max_age_seconds: bounded_cache_max_age(ttl),
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
    let Some(token) = cookies::read_cookie(headers, COOKIE_NAME)
        .filter(|value| !value.is_empty() && !value.starts_with(&format!("{PROBE_PREFIX}.")))
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

    async fn test_state(label: &str) -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = format!("subdomain-grant-{label}");
        let state = AppState::new(settings).await.expect("auth test state");
        (directory, state)
    }

    fn cookie_header(set_cookie: &str) -> axum::http::HeaderValue {
        axum::http::HeaderValue::from_str(set_cookie.split(';').next().unwrap_or_default())
            .expect("cookie header")
    }

    fn test_config(groups: Value) -> Value {
        serde_json::json!({
            "host_mappings": [{
                "host": "app.example.com",
                "use_auth": true,
                "advanced_auth": {
                    "enabled": true,
                    "idle_ttl_seconds": 86_400,
                    "max_lifetime_seconds": 2_592_000,
                    "policy_version": "version-1",
                    "groups": groups
                }
            }]
        })
    }

    fn test_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-host",
            axum::http::HeaderValue::from_static("app.example.com"),
        );
        headers.insert(
            "x-real-ip",
            axum::http::HeaderValue::from_static("203.0.113.20"),
        );
        headers
    }

    fn test_match(group_id: impl Into<String>) -> SubdomainRuleMatch {
        SubdomainRuleMatch {
            host: "app.example.com".to_string(),
            policy_version: "version-1".to_string(),
            group_id: group_id.into(),
        }
    }

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

    #[tokio::test]
    async fn cookie_less_matches_remain_stateless_until_probe_is_returned() {
        let (_directory, state) = test_state("cookie-capability-probe").await;
        let config = test_config(serde_json::json!([{
            "id": "group-1", "conditions": []
        }]));
        let matched = test_match("group-1");
        let mut headers = test_headers();
        headers.insert(
            header::USER_AGENT,
            axum::http::HeaderValue::from_static("com.trim.app.ios/1.32.3"),
        );

        let first = authorize(&state, &headers, &config, Some(&matched))
            .await
            .expect("first authorization")
            .expect("first rule access");
        assert_eq!(first.state, "transient");
        let probe_cookie = first.set_cookie.expect("cookie capability probe");
        assert!(probe_cookie.starts_with(&format!("{COOKIE_NAME}={PROBE_PREFIX}.")));
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            0
        );
        assert_eq!(
            state
                .store
                .count_keys_by_prefix(RATE_KEY_PREFIX)
                .await
                .unwrap(),
            0
        );

        let second = authorize(&state, &headers, &config, Some(&matched))
            .await
            .expect("second authorization")
            .expect("second rule access");
        assert_eq!(second.state, "transient");
        assert!(second.set_cookie.is_some());
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            0
        );

        let mut browser_headers = headers.clone();
        browser_headers.insert(header::COOKIE, cookie_header(&probe_cookie));
        let issued = authorize(&state, &browser_headers, &config, None)
            .await
            .expect("probe exchange")
            .expect("probe rule access");
        assert_eq!(issued.state, "issued");
        let grant_cookie = issued.set_cookie.expect("persistent grant cookie");
        assert!(!grant_cookie.contains(&format!("={PROBE_PREFIX}.")));
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            1
        );

        browser_headers.insert(header::COOKIE, cookie_header(&grant_cookie));
        let reused = authorize(&state, &browser_headers, &config, None)
            .await
            .expect("grant reuse")
            .expect("grant access");
        assert_eq!(reused.state, "reused");
    }

    #[tokio::test]
    async fn varying_user_agents_without_cookies_create_no_runtime_state() {
        let (_directory, state) = test_state("varying-user-agents").await;
        let config = test_config(serde_json::json!([{
            "id": "group-1", "conditions": []
        }]));
        let matched = test_match("group-1");
        for index in 0..200 {
            let mut headers = test_headers();
            headers.insert(
                header::USER_AGENT,
                axum::http::HeaderValue::from_str(&format!(
                    "com.trim.app.ios/1.32.3 variant-{index}"
                ))
                .unwrap(),
            );
            let access = authorize(&state, &headers, &config, Some(&matched))
                .await
                .expect("rule authorization")
                .expect("rule access");
            assert_eq!(access.state, "transient");
            assert!(access.set_cookie.is_some());
        }
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            0
        );
        assert_eq!(
            state
                .store
                .count_keys_by_prefix(RATE_KEY_PREFIX)
                .await
                .unwrap(),
            0
        );
        assert!(
            state
                .store
                .scan_keys("fn_knock:auth:subdomain_rule_issue_slot:", 10)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn concurrent_probe_replay_uses_one_persistent_grant_key() {
        let (_directory, state) = test_state("concurrent-probe-replay").await;
        let config = test_config(serde_json::json!([{
            "id": "group-1", "conditions": []
        }]));
        let matched = test_match("group-1");
        let headers = test_headers();
        let probe_cookie = authorize(&state, &headers, &config, Some(&matched))
            .await
            .unwrap()
            .unwrap()
            .set_cookie
            .expect("probe cookie");
        let mut probe_headers = headers;
        probe_headers.insert(header::COOKIE, cookie_header(&probe_cookie));

        let mut tasks = tokio::task::JoinSet::new();
        for _ in 0..20 {
            let state = state.clone();
            let config = config.clone();
            let headers = probe_headers.clone();
            tasks.spawn(async move {
                authorize(&state, &headers, &config, None)
                    .await
                    .expect("rule authorization")
                    .expect("rule access")
            });
        }

        let mut issued = 0;
        let mut transient = 0;
        while let Some(result) = tasks.join_next().await {
            match result.expect("authorization task").state {
                "issued" => issued += 1,
                "transient" => transient += 1,
                state => panic!("unexpected grant state {state}"),
            }
        }
        assert_eq!(issued, RATE_PER_CLIENT);
        assert_eq!(transient, 20 - RATE_PER_CLIENT);
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            1
        );
    }

    #[tokio::test]
    async fn persistent_issue_limit_degrades_to_transient_access() {
        let (_directory, state) = test_state("rate-limit-fallback").await;
        let groups = (0..=RATE_PER_CLIENT)
            .map(|index| serde_json::json!({"id": format!("group-{index}"), "conditions": []}))
            .collect::<Vec<_>>();
        let config = test_config(Value::Array(groups));
        let headers = test_headers();

        for index in 0..=RATE_PER_CLIENT {
            let matched = test_match(format!("group-{index}"));
            let probe_cookie = authorize(&state, &headers, &config, Some(&matched))
                .await
                .unwrap()
                .unwrap()
                .set_cookie
                .expect("probe cookie");
            let mut probe_headers = headers.clone();
            probe_headers.insert(header::COOKIE, cookie_header(&probe_cookie));
            let access = authorize(&state, &probe_headers, &config, None)
                .await
                .expect("rule authorization")
                .expect("rule access");
            if index < RATE_PER_CLIENT {
                assert_eq!(access.state, "issued");
            } else {
                assert_eq!(access.state, "transient");
                assert!(access.set_cookie.is_none());
            }
        }
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            RATE_PER_CLIENT
        );
    }

    #[tokio::test]
    async fn probe_is_bound_to_trusted_client_ip_and_logout_allows_reissue() {
        let (_directory, state) = test_state("probe-binding-and-reissue").await;
        let config = test_config(serde_json::json!([{
            "id": "group-1", "conditions": []
        }]));
        let matched = test_match("group-1");
        let headers = test_headers();
        let probe_cookie = authorize(&state, &headers, &config, Some(&matched))
            .await
            .unwrap()
            .unwrap()
            .set_cookie
            .expect("probe cookie");

        let mut tampered_pair = probe_cookie
            .split(';')
            .next()
            .expect("probe cookie pair")
            .to_string();
        let last = tampered_pair.pop().expect("probe signature byte");
        tampered_pair.push(if last == 'A' { 'B' } else { 'A' });
        let mut tampered = headers.clone();
        tampered.insert(
            header::COOKIE,
            axum::http::HeaderValue::from_str(&tampered_pair).expect("tampered cookie header"),
        );
        assert!(
            authorize(&state, &tampered, &config, None)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            0
        );

        let mut other_client = headers.clone();
        other_client.insert(
            "x-real-ip",
            axum::http::HeaderValue::from_static("203.0.113.21"),
        );
        other_client.insert(header::COOKIE, cookie_header(&probe_cookie));
        assert!(
            authorize(&state, &other_client, &config, None)
                .await
                .unwrap()
                .is_none()
        );

        let mut probe_headers = headers.clone();
        probe_headers.insert(header::COOKIE, cookie_header(&probe_cookie));
        let grant_cookie = authorize(&state, &probe_headers, &config, None)
            .await
            .unwrap()
            .unwrap()
            .set_cookie
            .expect("grant cookie");
        let mut grant_headers = headers.clone();
        grant_headers.insert(header::COOKIE, cookie_header(&grant_cookie));
        revoke(&state, &grant_headers).await.expect("revoke grant");
        assert_eq!(
            state.store.count_keys_by_prefix(KEY_PREFIX).await.unwrap(),
            0
        );

        let next_probe = authorize(&state, &headers, &config, Some(&matched))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(next_probe.state, "transient");
        assert!(next_probe.set_cookie.is_some());
    }
}
