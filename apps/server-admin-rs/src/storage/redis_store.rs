use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{
        Arc, Mutex as StdMutex,
        atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering},
    },
};

use crate::storage::redis_compat as redis;
use arc_swap::ArcSwap;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ipnet::IpNet;
use redis::{
    ConnectionManager,
    streams::{StreamRangeReply, StreamReadOptions, StreamReadReply},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use crate::storage::typed_config::TypedConfigRepository;
use crate::storage::typed_docker_admin::TypedDockerAdminRepository;
use crate::storage::typed_event_dedupe::TypedEventDedupeRepository;
use crate::storage::typed_events::TypedEventRepository;
use crate::storage::typed_fnos_share::TypedFnosShareRepository;
use crate::storage::typed_hmac_nonce::TypedHmacNonceRepository;
use crate::storage::typed_identity_runtime::TypedIdentityRuntimeRepository;
use crate::storage::typed_login_backoff::TypedLoginBackoffRepository;
use crate::storage::typed_mobility::TypedMobilityRepository;
use crate::storage::typed_notification_runtime::TypedNotificationRuntimeRepository;
use crate::storage::typed_notifications::TypedNotificationRepository;
use crate::storage::typed_passkey_runtime::TypedPasskeyRuntimeRepository;
use crate::storage::typed_subdomain_grant::TypedSubdomainGrantRepository;
use crate::storage::typed_subdomain_rate_limit::TypedSubdomainRateLimitRepository;
use crate::storage::typed_whitelist::{TypedWhitelistDocument, TypedWhitelistRepository};
use crate::storage::typed_whitelist_runtime::TypedWhitelistRuntimeRepository;
use crate::storage::typed_wol_cooldown::TypedWolCooldownRepository;
use crate::{
    auth_mobility_keys::{
        active_ip_details_key as auth_mobility_active_ip_details_key,
        active_ip_zset_key as auth_mobility_active_ip_zset_key,
        binding_key as auth_mobility_binding_key,
        session_index_key as auth_mobility_session_index_key,
        session_pending_whitelist_key as auth_mobility_session_pending_whitelist_key,
        subject_hash as auth_mobility_subject_hash, summary_key as auth_mobility_summary_key,
        timeline_key as auth_mobility_timeline_key,
        whitelist_owner_key as auth_mobility_whitelist_owner_key,
    },
    http_utils::normalize_ip,
    time_utils::{iso_after_seconds, now_iso},
};

mod auth;
mod config;
mod core;
mod discovery;
mod docker_admin;
mod events;
mod notifications;
mod traffic;
mod types;
mod waf_logs;
mod whitelist;

pub use config::default_config;
pub(crate) use core::{LdapBindingClaim, OidcBindingClaim, OwnedBindingDelete, OwnedBindingUpdate};
pub use types::*;

const CONFIG_KEY: &str = "fn_knock:config";
const HOST_MAPPINGS_GENERATION_KEY: &str = "fn_knock:config:host_mappings:generation";
pub(crate) const CONFIG_GENERATION_MARKER: &str = "__fn_knock_internal_host_mappings_generation";

pub(crate) fn strip_internal_config_metadata(config: &mut Value) {
    if let Some(object) = config.as_object_mut() {
        object.remove(CONFIG_GENERATION_MARKER);
    }
}

pub(crate) fn referenced_host_ipset_policy_ids<'a>(
    mappings: impl IntoIterator<Item = &'a Value>,
) -> BTreeSet<String> {
    let mut referenced = BTreeSet::new();
    for mapping in mappings {
        if let Some(id) = mapping
            .pointer("/visibility/policy_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            referenced.insert(id.to_string());
        }
        for condition in mapping
            .pointer("/advanced_auth/groups")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("conditions").and_then(Value::as_array))
            .flatten()
        {
            if let Some(id) = condition
                .get("policy_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                referenced.insert(id.to_string());
            }
        }
    }
    referenced
}

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct Store {
    manager: ConnectionManager,
    path: PathBuf,
    config_snapshot: Arc<ArcSwap<Value>>,
    config_snapshot_revision: Arc<StdMutex<u64>>,
    typed_config: TypedConfigRepository,
    typed_config_primary_bootstrapped: Arc<AtomicBool>,
    typed_config_shadow_healthy: Arc<AtomicBool>,
    typed_config_shadow_mismatches: Arc<AtomicU64>,
    typed_docker_admin: TypedDockerAdminRepository,
    typed_docker_admin_shadow_healthy: Arc<AtomicBool>,
    typed_docker_admin_shadow_mismatches: Arc<AtomicU64>,
    typed_event_dedupe: TypedEventDedupeRepository,
    typed_event_dedupe_shadow_healthy: Arc<AtomicBool>,
    typed_event_dedupe_shadow_mismatches: Arc<AtomicU64>,
    typed_events: TypedEventRepository,
    typed_events_shadow_healthy: Arc<AtomicBool>,
    typed_events_shadow_mismatches: Arc<AtomicU64>,
    typed_fnos_share: TypedFnosShareRepository,
    typed_fnos_share_shadow_healthy: Arc<AtomicBool>,
    typed_fnos_share_shadow_mismatches: Arc<AtomicU64>,
    typed_hmac_nonce: TypedHmacNonceRepository,
    typed_hmac_nonce_shadow_healthy: Arc<AtomicBool>,
    typed_hmac_nonce_shadow_mismatches: Arc<AtomicU64>,
    typed_identity_runtime: TypedIdentityRuntimeRepository,
    typed_identity_runtime_shadow_healthy: Arc<AtomicBool>,
    typed_identity_runtime_shadow_mismatches: Arc<AtomicU64>,
    typed_login_backoff: TypedLoginBackoffRepository,
    typed_login_backoff_shadow_healthy: Arc<AtomicBool>,
    typed_login_backoff_shadow_mismatches: Arc<AtomicU64>,
    typed_mobility: TypedMobilityRepository,
    typed_mobility_shadow_healthy: Arc<AtomicBool>,
    typed_mobility_shadow_mismatches: Arc<AtomicU64>,
    typed_notification_runtime: TypedNotificationRuntimeRepository,
    typed_notification_runtime_shadow_healthy: Arc<AtomicBool>,
    typed_notification_runtime_shadow_mismatches: Arc<AtomicU64>,
    typed_notifications: TypedNotificationRepository,
    typed_passkey_runtime: TypedPasskeyRuntimeRepository,
    typed_passkey_runtime_shadow_healthy: Arc<AtomicBool>,
    typed_passkey_runtime_shadow_mismatches: Arc<AtomicU64>,
    typed_subdomain_grant: TypedSubdomainGrantRepository,
    typed_subdomain_grant_shadow_healthy: Arc<AtomicBool>,
    typed_subdomain_grant_shadow_mismatches: Arc<AtomicU64>,
    typed_subdomain_rate_limit: TypedSubdomainRateLimitRepository,
    typed_subdomain_rate_limit_shadow_healthy: Arc<AtomicBool>,
    typed_subdomain_rate_limit_shadow_mismatches: Arc<AtomicU64>,
    typed_whitelist: TypedWhitelistRepository,
    typed_whitelist_shadow_healthy: Arc<AtomicBool>,
    typed_whitelist_shadow_mismatches: Arc<AtomicU64>,
    typed_whitelist_runtime: TypedWhitelistRuntimeRepository,
    typed_whitelist_runtime_shadow_healthy: Arc<AtomicBool>,
    typed_whitelist_runtime_shadow_mismatches: Arc<AtomicU64>,
    typed_wol_cooldown: TypedWolCooldownRepository,
    typed_wol_cooldown_shadow_healthy: Arc<AtomicBool>,
    typed_wol_cooldown_shadow_mismatches: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct TypedConfigShadowStatus {
    pub(crate) phase: &'static str,
    pub(crate) healthy: bool,
    pub(crate) mismatch_count: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct LoginBackoffAttemptState {
    ip: String,
    attempts: i64,
    #[serde(default, rename = "lastAttempt")]
    last_attempt: i64,
    #[serde(default, rename = "blockedUntil")]
    blocked_until: Option<i64>,
}

fn deserialize_whitelist_region_group(raw: &str) -> Option<WhitelistRegionGroupRecord> {
    let parsed = serde_json::from_str::<Value>(raw).ok()?;
    let object = parsed.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }

    let regions = object
        .get("regions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|region| {
            let object = region.as_object()?;
            let province = object
                .get("province")
                .map(js_string)
                .unwrap_or_default()
                .trim()
                .to_string();
            if province.is_empty() {
                return None;
            }
            let query_city = object
                .get("query_city")
                .map(js_string)
                .unwrap_or_default()
                .trim()
                .to_string();
            let operator = crate::cidr::CidrOperator::parse_value(object.get("operator"))
                .ok()
                .flatten();
            Some(WhitelistRegionInput {
                province,
                query_city: (!query_city.is_empty()).then_some(query_city),
                operator,
            })
        })
        .collect::<Vec<_>>();
    let cidrs = object
        .get("cidrs")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(js_string)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let policy_id = object
        .get("policyId")
        .or_else(|| object.get("policy_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
        .to_string();
    let policy = object
        .get("policy")
        .filter(|value| !value.is_null())
        .cloned();
    let source_cidr_count = object
        .get("sourceCidrCount")
        .or_else(|| object.get("source_cidr_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(cidrs.len());
    let range_count = object
        .get("rangeCount")
        .or_else(|| object.get("range_count"))
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default();
    let created_at = js_finite_number(object.get("createdAt"))
        .map(|value| value.trunc() as i64)
        .unwrap_or(0);
    let updated_at = js_finite_number(object.get("updatedAt"))
        .map(|value| value.trunc() as i64)
        .unwrap_or(0);
    let expire_at = match object.get("expireAt") {
        None | Some(Value::Null) => None,
        value => js_finite_number(value).map(|value| value.trunc() as i64),
    };
    let status = match object.get("status").and_then(Value::as_str) {
        Some("deleted") => "deleted",
        Some("expired") => "expired",
        _ => "active",
    };
    let comment = object.contains_key("comment").then(|| {
        object
            .get("comment")
            .map(js_string)
            .unwrap_or_default()
            .trim()
            .to_string()
    });

    Some(WhitelistRegionGroupRecord {
        id,
        regions,
        cidrs,
        policy_id,
        policy,
        source_cidr_count,
        range_count,
        expire_at,
        source: "manual".to_string(),
        created_at,
        updated_at,
        status: status.to_string(),
        comment,
    })
}

fn deserialize_whitelist_record(raw: &str) -> Option<WhitelistRecord> {
    let parsed = serde_json::from_str::<Value>(raw).ok()?;
    let object = parsed.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }

    let raw_target = object
        .get("ip")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let target_type = match object.get("targetType").and_then(Value::as_str) {
        Some("cidr") => "cidr",
        Some("cname") => "cname",
        _ => infer_whitelist_target_type(&raw_target)?,
    };
    let normalized_target = normalize_whitelist_target(&raw_target, target_type)?;

    let source = if object.get("source").and_then(Value::as_str) == Some("auto") {
        "auto"
    } else {
        "manual"
    };
    let status = match object.get("status").and_then(Value::as_str) {
        Some("pending") => "pending",
        Some("expired") => "expired",
        Some("deleted") => "deleted",
        _ => "active",
    };
    let created_at = object
        .get("createdAt")
        .map(js_string)
        .as_deref()
        .and_then(parse_int_like_js)
        .unwrap_or(0);
    let expire_at = optional_whitelist_timestamp(object.get("expireAt"));
    let comment = object
        .get("comment")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let ip_location = (target_type == "ip")
        .then(|| {
            object
                .get("ipLocation")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
        .flatten();
    let resolved_targets = (target_type == "cname")
        .then(|| normalize_whitelist_resolved_targets(object.get("resolvedTargets")));
    let check_interval_minutes = (target_type == "cname")
        .then(|| normalize_whitelist_cname_check_interval(object.get("checkIntervalMinutes")));
    let last_checked_at = optional_whitelist_timestamp(object.get("lastCheckedAt"));
    let last_resolved_at = optional_whitelist_timestamp(object.get("lastResolvedAt"));
    let resolve_status = match object.get("resolveStatus").and_then(Value::as_str) {
        Some("resolved") => Some("resolved".to_string()),
        Some("empty") => Some("empty".to_string()),
        Some("error") => Some("error".to_string()),
        Some("pending") => Some("pending".to_string()),
        _ if target_type == "cname" => Some("pending".to_string()),
        _ => None,
    };
    let resolve_message = object
        .get("resolveMessage")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);

    Some(WhitelistRecord {
        id,
        ip: normalized_target,
        target_type: target_type.to_string(),
        expire_at,
        source: source.to_string(),
        created_at,
        status: status.to_string(),
        comment,
        ip_location,
        resolved_targets,
        check_interval_minutes,
        last_checked_at,
        last_resolved_at,
        resolve_status,
        resolve_message,
    })
}

fn infer_whitelist_target_type(value: &str) -> Option<&'static str> {
    if normalize_whitelist_cidr(value).is_some() {
        return Some("cidr");
    }
    if !normalize_ip(value).is_empty() {
        return Some("ip");
    }
    if normalize_whitelist_domain(value).is_some() {
        return Some("cname");
    }
    None
}

fn normalize_whitelist_target(value: &str, target_type: &str) -> Option<String> {
    match target_type {
        "cidr" => normalize_whitelist_cidr(value),
        "cname" => normalize_whitelist_domain(value),
        _ => {
            let normalized = normalize_ip(value);
            (!normalized.is_empty()).then_some(normalized)
        }
    }
}

fn normalize_whitelist_cidr(value: &str) -> Option<String> {
    let parsed = IpNet::from_str(value.trim()).ok()?;
    Some(match parsed {
        IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
        IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
    })
}

fn normalize_whitelist_domain(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains("..") {
        return None;
    }
    let ascii = idna::domain_to_ascii(&trimmed).ok()?;
    if ascii.is_empty() || ascii.len() > 253 {
        return None;
    }
    let labels = ascii.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return None;
    }
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return None;
        }
    }
    Some(ascii)
}

fn normalize_whitelist_resolved_targets(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let normalized = normalize_ip(js_string(item).trim());
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn optional_whitelist_timestamp(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    let string_value = js_string(value);
    if string_value.is_empty() {
        return None;
    }
    parse_int_like_js(&string_value)
}

fn normalize_whitelist_cname_check_interval(value: Option<&Value>) -> i64 {
    parse_int_like_js(&value.map(js_string).unwrap_or_default())
        .unwrap_or(5)
        .clamp(1, 24 * 60)
}

fn parse_int_like_js(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix_trim_start(value)
}

const WHITELIST_RECORDS: &str = "fn_knock:whitelist:records";
const WHITELIST_RECORD_ORDER: &str = "fn_knock:whitelist:record_order";
const WHITELIST_EXPIRY: &str = "fn_knock:whitelist:expiry";
const WHITELIST_IPS: &str = "fn_knock:whitelist:ips";
const WHITELIST_CIDR_RECORDS: &str = "fn_knock:whitelist:cidr_records";
const WHITELIST_DELETED: &str = "fn_knock:whitelist:deleted";
const WHITELIST_REGION_GROUP_RECORDS: &str = "fn_knock:whitelist:region_groups:records";
const WHITELIST_REGION_GROUP_ORDER: &str = "fn_knock:whitelist:region_groups:order";
const WHITELIST_REGION_GROUP_EXPIRY: &str = "fn_knock:whitelist:region_groups:expiry";
const GATEWAY_TRUSTED_CLIENT_IPS_RUNTIME: &str = "fn_knock:gateway:trusted-client-ips:runtime";
const REVERSE_PROXY_TRUSTED_IPS_RUNTIME: &str = "fn_knock:reverse-proxy:trusted-ips:runtime";
const EVENTS_STREAM_KEY: &str = "fn_knock:events:stream";
const EVENTS_INDEX_KEY: &str = "fn_knock:events:index";
const EVENTS_DATA_PREFIX: &str = "fn_knock:events:data:";
const EVENTS_DEDUPE_PREFIX: &str = crate::storage::typed_event_dedupe::DEDUPE_PREFIX;
const EVENTS_STREAM_ID_PREFIX: &str = "fn_knock:events:stream-id:";
const NOTIFICATION_RUNTIME_LAST_STREAM_KEY: &str = "fn_knock:notifications:runtime:last-stream-id";
const NOTIFICATION_RUNTIME_LOCK_PREFIX: &str = "fn_knock:notifications:runtime:lock:";
const NOTIFICATION_RUNTIME_COOLDOWN_PREFIX: &str = "fn_knock:notifications:runtime:cooldown:";
const NOTIFICATION_RUNTIME_WINDOW_PREFIX: &str = "fn_knock:notifications:runtime:window:";
const NOTIFICATION_DELIVERIES_READY_KEY: &str = "fn_knock:notifications:deliveries:ready";
const NOTIFICATION_DELIVERY_QUEUE_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const TRAFFIC_KEY_INDEX: &str = "fn_knock:traffic:keys";
const ERROR5XX_KEY_INDEX: &str = "fn_knock:errors:5xx:keys";
const WAF_LOG_DATE_PREFIX: &str = "fn_knock:waf:logs:";
const WAF_LOG_EVENT_PREFIX: &str = "fn_knock:waf:log:";
const WAF_LOG_STATS_PREFIX: &str = "fn_knock:waf:stats:";
const WAF_LOG_DATES_INDEX_KEY: &str = "fn_knock:waf:logs:dates";
const WAF_LOG_DATES_INDEX_MIGRATED_KEY: &str = "fn_knock:waf:logs:dates:migrated";
const LOGIN_BACKOFF_PREFIX: &str = "fn_knock:login_backoff:";
const LOGIN_BACKOFF_TTL_SECONDS: i64 = 3600;
const SCANNER_SUSPICIOUS_PREFIX: &str = "fn_knock:scanner:suspicious:";
const SCANNER_BLACKLIST_INDEX_KEY: &str = "fn_knock:scanner:blacklist:index";
const SCANNER_BLACKLIST_DATA_PREFIX: &str = "fn_knock:scanner:blacklist:data:";
const SCANNER_SETTINGS_KEY: &str = "fn_knock:scanner:settings";
const IP_LOCATION_PREFIX: &str = "fn_knock:ip_location";
const IP_LOCATION_QUEUE_KEY: &str = "fn_knock:ip_location:queue";
const RECENT_AUTH_IPS_ZSET_KEY: &str = "fn_knock:recent_auth_ips:zset";
const RECENT_AUTH_IPS_DETAILS_KEY: &str = "fn_knock:recent_auth_ips:details";
const RECENT_AUTH_IPS_TTL_SECONDS: i64 = 30 * 24 * 3600;
const DOCKER_ADMIN_PASSWORD_KEY: &str = "fn_knock:docker_admin:password:v1";
const DOCKER_ADMIN_SESSION_PREFIX: &str = "fn_knock:docker_admin:session:v1:";
const DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX: &str = "fn_knock:docker_admin:login_backoff:v1:";
const DOCKER_ADMIN_LOGIN_BACKOFF_TTL_SECONDS: i64 = 3_600;
const DOCKER_ADMIN_LOGIN_BACKOFF_BASE_DELAY_MS: i64 = 2_000;
const DOCKER_ADMIN_LOGIN_BACKOFF_MAX_DELAY_MS: i64 = 15 * 60 * 1_000;
const DOCKER_ADMIN_REGISTER_LOGIN_FAILURE_SCRIPT: &str = r#"
-- fn-knock:eval:docker-admin-login-backoff:v1
local key = KEYS[1]
local ip = ARGV[1]
local now = tonumber(ARGV[2])
local nowIso = ARGV[3]
local ttlSeconds = tonumber(ARGV[4])
local baseDelay = tonumber(ARGV[5])
local maxDelay = tonumber(ARGV[6])

local attempts = 0
local raw = redis.call('GET', key)
if raw then
  local ok, decoded = pcall(cjson.decode, raw)
  if ok and type(decoded) == 'table' and tonumber(decoded.attempts) then
    attempts = tonumber(decoded.attempts)
  end
end
attempts = attempts + 1
local exponent = math.min(math.max(attempts - 1, 0), 30)
local backoffMs = math.min(baseDelay * math.pow(2, exponent), maxDelay)
local blockedUntil = now + backoffMs
redis.call('SET', key, cjson.encode({
  ip = ip,
  attempts = attempts,
  last_attempt_at = nowIso,
  blocked_until = blockedUntil,
}), 'EX', ttlSeconds)
return { attempts, math.floor((backoffMs + 999) / 1000), blockedUntil }
"#;
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE: &str = "__builtin_select__";
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH: &str = "/__select__";
const TOTP_SUBDOMAIN_ACCESS_WOL_PAGE: &str = "__builtin_wol__";
const TOTP_SUBDOMAIN_ACCESS_WOL_PAGE_PATH: &str = "/__wol__";
const EVENT_LIST_SCAN_CHUNK_SIZE: isize = 200;
const EVENT_CLEAR_CHUNK_SIZE: usize = 500;
const MAX_EVENT_RETENTION_DAYS: i64 = 90;
const LOGIN_BACKOFF_REGISTER_FAILURE_SCRIPT: &str = r#"
-- fn-knock:eval:login-backoff:v1
local key = KEYS[1]
local ip = ARGV[1]
local now = tonumber(ARGV[2])
local ttlSeconds = tonumber(ARGV[3])
local baseDelay = tonumber(ARGV[4])
local maxDelay = tonumber(ARGV[5])
local jitterFactor = tonumber(ARGV[6])

local attempts = 0
local raw = redis.call('GET', key)
if raw then
  local ok, decoded = pcall(cjson.decode, raw)
  if ok and type(decoded) == 'table' and tonumber(decoded.attempts) then
    attempts = tonumber(decoded.attempts)
  end
end

attempts = attempts + 1

local expDelay = math.pow(2, attempts - 1) * baseDelay
local seed = ip .. ':' .. tostring(attempts) .. ':' .. tostring(now)
local hash = 0
for i = 1, #seed do
  hash = (hash * 33 + string.byte(seed, i)) % 1000003
end
local ratio = (hash % 10000) / 10000
local jitter = ((ratio * 2) - 1) * (expDelay * jitterFactor)
local backoffMs = math.floor(expDelay + jitter)
if backoffMs < 0 then
  backoffMs = 0
end
if backoffMs > maxDelay then
  backoffMs = maxDelay
end

local blockedUntil = now + backoffMs
local nextState = cjson.encode({
  ip = ip,
  attempts = attempts,
  lastAttempt = now,
  blockedUntil = blockedUntil,
})

redis.call('SET', key, nextState, 'EX', ttlSeconds)
return {attempts, math.ceil(backoffMs / 1000), blockedUntil}
"#;

fn default_whitelist_target_type() -> String {
    "ip".to_string()
}

fn default_whitelist_source() -> String {
    "manual".to_string()
}

fn default_whitelist_status() -> String {
    "active".to_string()
}

fn whitelist_ip_records_key(ip: &str) -> String {
    format!("fn_knock:whitelist:ip_records:{ip}")
}

fn traffic_scope_segment(user_id: &str, host: Option<&str>, stream: Option<&str>) -> String {
    let host = host.map(str::trim).filter(|value| !value.is_empty());
    let stream = stream.map(str::trim).filter(|value| !value.is_empty());
    match (host, stream) {
        (Some(host), _) => {
            let encoded = crate::http_utils::url_encode_component(host);
            format!("{user_id}:host:{encoded}")
        }
        (None, Some(stream)) => {
            let encoded = crate::http_utils::url_encode_component(stream);
            format!("{user_id}:stream:{encoded}")
        }
        (None, None) => user_id.to_string(),
    }
}

fn traffic_key(user_id: &str, direction: &str, host: Option<&str>, stream: Option<&str>) -> String {
    format!(
        "fn_knock:traffic:{}:{}",
        traffic_scope_segment(user_id, host, stream),
        direction
    )
}

fn traffic_last_total_key(
    user_id: &str,
    direction: &str,
    host: Option<&str>,
    stream: Option<&str>,
) -> String {
    format!(
        "fn_knock:traffic:last:{}:{}",
        traffic_scope_segment(user_id, host, stream),
        direction
    )
}

fn error5xx_key(user_id: &str, host: Option<&str>, stream: Option<&str>) -> String {
    format!(
        "fn_knock:errors:{}:5xx",
        traffic_scope_segment(user_id, host, stream)
    )
}

fn error5xx_last_total_key(user_id: &str, host: Option<&str>, stream: Option<&str>) -> String {
    format!(
        "fn_knock:errors:last:{}:5xx",
        traffic_scope_segment(user_id, host, stream)
    )
}

fn scanner_suspicious_key(ip: &str) -> String {
    format!("{SCANNER_SUSPICIOUS_PREFIX}{ip}")
}

fn scanner_blacklist_data_key(ip: &str) -> String {
    format!("{SCANNER_BLACKLIST_DATA_PREFIX}{ip}")
}

fn sanitize_scanner_ips(ips: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut clean_ips = Vec::new();
    for ip in ips {
        let clean = ip.trim();
        if clean.is_empty() || !seen.insert(clean.to_string()) {
            continue;
        }
        clean_ips.push(clean.to_string());
    }
    clean_ips
}

fn scanner_blacklist_record_from_raw(ip: &str, raw: &str) -> Option<Value> {
    let mut record = serde_json::from_str::<Value>(raw).ok()?;
    let object = record.as_object_mut()?;
    let missing_ip = object
        .get("ip")
        .and_then(Value::as_str)
        .is_none_or(|value| value.trim().is_empty());
    if missing_ip {
        object.insert("ip".to_string(), Value::String(ip.to_string()));
    }
    Some(record)
}

fn ip_location_cache_key(ip: &str) -> String {
    format!("{IP_LOCATION_PREFIX}:cache:{ip}")
}

fn ip_location_state_key(ip: &str) -> String {
    format!("{IP_LOCATION_PREFIX}:state:{ip}")
}

fn ip_location_refs_key(ip: &str) -> String {
    format!("{IP_LOCATION_PREFIX}:refs:{ip}")
}

fn ip_location_lock_key(ip: &str) -> String {
    format!("{IP_LOCATION_PREFIX}:lock:{ip}")
}

impl Store {
    pub async fn connect(sqlite_path: impl AsRef<Path>) -> crate::storage::StorageResult<Self> {
        let path = sqlite_path.as_ref().to_path_buf();
        let manager = ConnectionManager::open(&path).await?;
        let typed_config = TypedConfigRepository::new(manager.clone());
        typed_config.initialize().await?;
        let typed_docker_admin = TypedDockerAdminRepository::new(manager.clone());
        typed_docker_admin.initialize().await?;
        let typed_event_dedupe = TypedEventDedupeRepository::new(manager.clone());
        typed_event_dedupe.initialize().await?;
        let typed_events = TypedEventRepository::new(manager.clone());
        typed_events.initialize().await?;
        let typed_fnos_share = TypedFnosShareRepository::new(manager.clone());
        typed_fnos_share.initialize().await?;
        let typed_hmac_nonce = TypedHmacNonceRepository::new(manager.clone());
        typed_hmac_nonce.initialize().await?;
        let typed_identity_runtime = TypedIdentityRuntimeRepository::new(manager.clone());
        typed_identity_runtime.initialize().await?;
        let typed_login_backoff = TypedLoginBackoffRepository::new(manager.clone());
        typed_login_backoff.initialize().await?;
        let typed_mobility = TypedMobilityRepository::new(manager.clone());
        typed_mobility.initialize().await?;
        let typed_notification_runtime = TypedNotificationRuntimeRepository::new(manager.clone());
        typed_notification_runtime.initialize().await?;
        let typed_notifications = TypedNotificationRepository::new(manager.clone());
        typed_notifications.initialize().await?;
        let typed_passkey_runtime = TypedPasskeyRuntimeRepository::new(manager.clone());
        typed_passkey_runtime.initialize().await?;
        let typed_subdomain_grant = TypedSubdomainGrantRepository::new(manager.clone());
        typed_subdomain_grant.initialize().await?;
        let typed_subdomain_rate_limit = TypedSubdomainRateLimitRepository::new(manager.clone());
        typed_subdomain_rate_limit.initialize().await?;
        let typed_whitelist = TypedWhitelistRepository::new(manager.clone());
        typed_whitelist.initialize().await?;
        let typed_whitelist_runtime = TypedWhitelistRuntimeRepository::new(manager.clone());
        typed_whitelist_runtime.initialize().await?;
        let typed_wol_cooldown = TypedWolCooldownRepository::new(manager.clone());
        typed_wol_cooldown.initialize().await?;
        let store = Self {
            manager,
            path,
            config_snapshot: Arc::new(ArcSwap::from_pointee(default_config())),
            config_snapshot_revision: Arc::new(StdMutex::new(0)),
            typed_config,
            typed_config_primary_bootstrapped: Arc::new(AtomicBool::new(false)),
            typed_config_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_config_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_docker_admin,
            typed_docker_admin_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_docker_admin_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_event_dedupe,
            typed_event_dedupe_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_event_dedupe_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_events,
            typed_events_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_events_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_fnos_share,
            typed_fnos_share_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_fnos_share_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_hmac_nonce,
            typed_hmac_nonce_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_hmac_nonce_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_identity_runtime,
            typed_identity_runtime_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_identity_runtime_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_login_backoff,
            typed_login_backoff_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_login_backoff_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_mobility,
            typed_mobility_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_mobility_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_notification_runtime,
            typed_notification_runtime_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_notification_runtime_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_notifications,
            typed_passkey_runtime,
            typed_passkey_runtime_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_passkey_runtime_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_subdomain_grant,
            typed_subdomain_grant_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_subdomain_grant_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_subdomain_rate_limit,
            typed_subdomain_rate_limit_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_subdomain_rate_limit_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_whitelist,
            typed_whitelist_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_whitelist_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_whitelist_runtime,
            typed_whitelist_runtime_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_whitelist_runtime_shadow_mismatches: Arc::new(AtomicU64::new(0)),
            typed_wol_cooldown,
            typed_wol_cooldown_shadow_healthy: Arc::new(AtomicBool::new(true)),
            typed_wol_cooldown_shadow_mismatches: Arc::new(AtomicU64::new(0)),
        };
        store.typed_docker_admin.rebuild_from_legacy().await?;
        store.typed_event_dedupe.rebuild_from_legacy().await?;
        store.rebuild_typed_system_events_from_legacy().await?;
        store.typed_fnos_share.rebuild_from_legacy().await?;
        store.typed_hmac_nonce.rebuild_from_legacy().await?;
        store.typed_identity_runtime.rebuild_from_legacy().await?;
        store.typed_login_backoff.rebuild_from_legacy().await?;
        store.typed_mobility.rebuild_from_legacy().await?;
        store
            .typed_notification_runtime
            .rebuild_from_legacy()
            .await?;
        store
            .rebuild_typed_notification_documents_from_legacy()
            .await?;
        store
            .rebuild_typed_notification_history_from_legacy()
            .await?;
        store.typed_passkey_runtime.rebuild_from_legacy().await?;
        store.typed_subdomain_grant.rebuild_from_legacy().await?;
        store
            .typed_subdomain_rate_limit
            .rebuild_from_legacy()
            .await?;
        store.rebuild_typed_whitelist_from_legacy().await?;
        store.typed_whitelist_runtime.rebuild_from_legacy().await?;
        store.typed_wol_cooldown.rebuild_from_legacy().await?;
        store.refresh_config_snapshot().await?;
        Ok(store)
    }

    pub(crate) async fn prepare_for_system_update(
        &self,
        backup_path: impl AsRef<Path>,
    ) -> crate::storage::StorageResult<()> {
        self.manager
            .prepare_for_system_update(backup_path.as_ref())
            .await
    }

    pub(crate) async fn checkpoint_for_shutdown(&self) -> crate::storage::StorageResult<()> {
        self.manager.checkpoint_for_shutdown().await
    }

    pub(crate) async fn cancel_system_update(&self) -> crate::storage::StorageResult<()> {
        self.manager.cancel_system_update().await
    }

    fn conn(&self) -> ConnectionManager {
        self.manager.clone()
    }

    #[allow(dead_code)]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn config_snapshot(&self) -> Arc<Value> {
        self.config_snapshot.load_full()
    }

    pub async fn refresh_config_snapshot(&self) -> crate::storage::StorageResult<()> {
        let (config, revision) = self.reconcile_typed_config_from_legacy().await?;
        self.publish_config_snapshot(config, revision);
        Ok(())
    }

    fn publish_config_snapshot(&self, config: Value, revision: u64) {
        let mut published_revision = self
            .config_snapshot_revision
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if revision < *published_revision {
            tracing::debug!(
                revision,
                current_revision = *published_revision,
                "ignored stale config snapshot publication"
            );
            return;
        }
        self.config_snapshot.store(Arc::new(config));
        *published_revision = revision;
        self.typed_config_shadow_healthy
            .store(true, AtomicOrdering::Release);
    }

    pub(crate) fn typed_config_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "typed_primary",
            healthy: self
                .typed_config_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_config_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_mobility_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "dual_write_shadow",
            healthy: self
                .typed_mobility_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_mobility_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_login_backoff_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_login_backoff_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_login_backoff_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_docker_admin_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_docker_admin_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_docker_admin_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_event_dedupe_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_event_dedupe_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_event_dedupe_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_identity_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_identity_runtime_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_identity_runtime_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_fnos_share_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_fnos_share_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_fnos_share_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_hmac_nonce_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_hmac_nonce_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_hmac_nonce_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_subdomain_rate_limit_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_subdomain_rate_limit_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_subdomain_rate_limit_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_subdomain_grant_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_subdomain_grant_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_subdomain_grant_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_wol_cooldown_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_wol_cooldown_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_wol_cooldown_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_whitelist_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_whitelist_runtime_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_whitelist_runtime_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_notification_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_notification_runtime_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_notification_runtime_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    pub(crate) fn typed_passkey_runtime_shadow_status(&self) -> TypedConfigShadowStatus {
        TypedConfigShadowStatus {
            phase: "legacy_primary_shadow",
            healthy: self
                .typed_passkey_runtime_shadow_healthy
                .load(AtomicOrdering::Acquire),
            mismatch_count: self
                .typed_passkey_runtime_shadow_mismatches
                .load(AtomicOrdering::Acquire),
        }
    }

    #[cfg(test)]
    pub(crate) fn typed_config_shadow_mismatch_count(&self) -> u64 {
        self.typed_config_shadow_status().mismatch_count
    }
}

fn queue_whitelist_indexes(pipe: &mut redis::Pipeline, record: &WhitelistRecord) {
    match record.target_type() {
        "cidr" => {
            pipe.sadd(WHITELIST_CIDR_RECORDS, &record.id).ignore();
        }
        "cname" => {
            for target in record.concrete_targets() {
                if target.target_type == "ip" {
                    pipe.sadd(WHITELIST_IPS, &target.target).ignore();
                    pipe.sadd(whitelist_ip_records_key(&target.target), &record.id)
                        .ignore();
                }
            }
        }
        _ => {
            pipe.sadd(WHITELIST_IPS, &record.ip).ignore();
            pipe.sadd(whitelist_ip_records_key(&record.ip), &record.id)
                .ignore();
        }
    }
}

fn queue_remove_whitelist_indexes(pipe: &mut redis::Pipeline, record: &WhitelistRecord) {
    match record.target_type() {
        "cidr" => {
            pipe.srem(WHITELIST_CIDR_RECORDS, &record.id).ignore();
        }
        "cname" => {
            for target in record.concrete_targets() {
                if target.target_type == "ip" {
                    pipe.srem(whitelist_ip_records_key(&target.target), &record.id)
                        .ignore();
                }
            }
        }
        _ => {
            pipe.srem(whitelist_ip_records_key(&record.ip), &record.id)
                .ignore();
        }
    }
}

fn whitelist_stale_ip_index_targets(record: &WhitelistRecord) -> Vec<String> {
    let mut targets = Vec::new();
    for target in record.concrete_targets() {
        if target.target_type != "ip" || targets.iter().any(|value| value == &target.target) {
            continue;
        }
        targets.push(target.target);
    }
    targets
}

fn limit_mobility_timeline_events(events: &mut Vec<Value>, max_events: usize) {
    if events.len() <= max_events {
        return;
    }
    let first_is_login = events
        .first()
        .and_then(|event| event.get("kind"))
        .and_then(Value::as_str)
        == Some("login");
    if first_is_login {
        let first = events.first().cloned();
        let tail_count = max_events.saturating_sub(1);
        let tail = events
            .iter()
            .skip(events.len().saturating_sub(tail_count))
            .cloned()
            .collect::<Vec<_>>();
        events.clear();
        if let Some(first) = first {
            events.push(first);
        }
        events.extend(tail);
    } else {
        let tail = events
            .iter()
            .skip(events.len().saturating_sub(max_events))
            .cloned()
            .collect::<Vec<_>>();
        *events = tail;
    }
}

fn build_mobility_summary(events: &[Value]) -> Value {
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
            .and_then(Value::as_str)
    })
}

fn next_mobility_summary_from_event(
    events: &[Value],
    stored_summary: Option<Value>,
    event: &Value,
    seed_login_event: Option<&Value>,
) -> Value {
    let baseline = stored_summary.unwrap_or_else(|| {
        if events.is_empty() {
            let seeded = seed_login_event.cloned().into_iter().collect::<Vec<_>>();
            build_mobility_summary(&seeded)
        } else {
            build_mobility_summary(events)
        }
    });

    if event.get("kind").and_then(Value::as_str) != Some("drift") {
        return baseline;
    }

    let drift_count = baseline
        .get("driftCount")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + 1;
    json!({
        "hasHistory": true,
        "driftCount": drift_count,
        "lastDriftAt": event
            .get("happenedAt")
            .and_then(Value::as_str)
            .unwrap_or(""),
        "lastDriftSource": event
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("session-refresh")
    })
}

fn unique_concrete_targets(targets: &[WhitelistConcreteTarget]) -> Vec<WhitelistConcreteTarget> {
    let mut unique = Vec::new();
    for target in targets {
        if unique.iter().any(|candidate: &WhitelistConcreteTarget| {
            candidate.target == target.target && candidate.target_type == target.target_type
        }) {
            continue;
        }
        unique.push(target.clone());
    }
    unique
}

fn unique_non_empty_strings(values: &[String]) -> Vec<String> {
    let mut unique = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() || unique.iter().any(|item: &String| item == normalized) {
            continue;
        }
        unique.push(normalized.to_string());
    }
    unique
}

fn system_event_data_key(id: &str) -> String {
    format!("{EVENTS_DATA_PREFIX}{id}")
}

fn system_event_stream_id_key(id: &str) -> String {
    format!("{EVENTS_STREAM_ID_PREFIX}{id}")
}

fn notification_runtime_lock_key(name: &str) -> String {
    format!("{NOTIFICATION_RUNTIME_LOCK_PREFIX}{name}")
}

fn notification_cooldown_key(rule_id: &str, group_key: &str) -> String {
    format!(
        "{NOTIFICATION_RUNTIME_COOLDOWN_PREFIX}{rule_id}:{}",
        encode_notification_key_part(group_key)
    )
}

fn notification_window_key(rule_id: &str, group_key: &str) -> String {
    format!(
        "{NOTIFICATION_RUNTIME_WINDOW_PREFIX}{rule_id}:{}",
        encode_notification_key_part(group_key)
    )
}

fn encode_notification_key_part(value: &str) -> String {
    URL_SAFE_NO_PAD.encode(if value.is_empty() { "empty" } else { value })
}

fn waf_log_date_key(date: &str) -> String {
    format!("{WAF_LOG_DATE_PREFIX}{date}")
}

fn waf_log_event_key(trace_id: &str) -> String {
    format!("{WAF_LOG_EVENT_PREFIX}{trace_id}")
}

fn waf_log_stats_key(date: &str) -> String {
    format!("{WAF_LOG_STATS_PREFIX}{date}")
}

fn waf_log_event_score(event: &Value) -> i64 {
    event
        .get("time")
        .and_then(Value::as_str)
        .and_then(crate::time_utils::parse_iso_ms)
        .unwrap_or_else(crate::time_utils::now_ms)
}

fn is_waf_log_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

fn descending_strings(values: BTreeSet<String>) -> Vec<String> {
    values.into_iter().rev().collect()
}

fn waf_log_date_score(date: &str) -> i64 {
    let mut parts = date.split('-');
    let year = parts.next().and_then(|value| value.parse::<i32>().ok());
    let month = parts.next().and_then(|value| value.parse::<u8>().ok());
    let day = parts.next().and_then(|value| value.parse::<u8>().ok());
    let (Some(year), Some(month), Some(day)) = (year, month, day) else {
        return 0;
    };
    let Ok(month) = time::Month::try_from(month) else {
        return 0;
    };
    let Ok(date) = time::Date::from_calendar_date(year, month, day) else {
        return 0;
    };
    date.with_time(time::Time::MIDNIGHT)
        .assume_utc()
        .unix_timestamp()
        * 1000
}

fn login_backoff_key(ip: &str) -> String {
    format!("{LOGIN_BACKOFF_PREFIX}{ip}")
}

fn system_event_matches_filters(
    event: &Value,
    search: &str,
    event_type: Option<&str>,
    level: Option<&str>,
    source: Option<&str>,
) -> bool {
    if event_type.is_some_and(|value| event.get("type").and_then(Value::as_str) != Some(value)) {
        return false;
    }
    if level.is_some_and(|value| event.get("level").and_then(Value::as_str) != Some(value)) {
        return false;
    }
    if source.is_some_and(|value| event.get("source").and_then(Value::as_str) != Some(value)) {
        return false;
    }

    let keyword = search.trim().to_lowercase();
    if keyword.is_empty() {
        return true;
    }

    let mut haystack = String::new();
    for key in ["id", "type", "source", "level", "happened_at", "dedupe_key"] {
        if let Some(value) = event.get(key).and_then(Value::as_str) {
            haystack.push_str(value);
            haystack.push(' ');
        }
    }
    if let Some(subject) = event.get("subject").and_then(Value::as_object) {
        for key in ["kind", "id"] {
            if let Some(value) = subject.get(key).and_then(Value::as_str) {
                haystack.push_str(value);
                haystack.push(' ');
            }
        }
    }

    if let Some(tags) = event.get("tags").and_then(Value::as_array) {
        for tag in tags.iter().filter_map(Value::as_str) {
            haystack.push_str(tag);
            haystack.push(' ');
        }
    }
    if let Some(payload) = event.get("payload") {
        haystack.push_str(&serde_json::to_string(payload).unwrap_or_default());
    }

    haystack.to_lowercase().contains(&keyword)
}

fn chrono_like_now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn parse_finite(value: &Option<String>) -> Option<f64> {
    let parsed = value.as_ref()?.parse::<f64>().ok()?;
    parsed.is_finite().then_some(parsed)
}

fn compute_counter_delta(current_total: f64, last_total: Option<f64>) -> f64 {
    if !current_total.is_finite() || current_total < 0.0 {
        return 0.0;
    }
    let Some(last_total) = last_total else {
        return current_total;
    };
    if !last_total.is_finite() || last_total < 0.0 {
        return current_total;
    }
    if current_total >= last_total {
        current_total - last_total
    } else {
        current_total
    }
}

fn finite_number_string(value: f64) -> String {
    if !value.is_finite() || value <= 0.0 {
        return "0".to_string();
    }
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn traffic_member(ts: i64, delta: f64) -> String {
    format!("{ts}:{}", finite_number_string(delta))
}

fn parse_traffic_points(members: &[String]) -> Vec<TrafficDeltaPoint> {
    let mut points = Vec::new();
    for member in members {
        let Some((ts, delta)) = member.split_once(':') else {
            continue;
        };
        let Ok(ts) = ts.parse::<i64>() else {
            continue;
        };
        let Ok(delta) = delta.parse::<f64>() else {
            continue;
        };
        if !delta.is_finite() {
            continue;
        }
        points.push(TrafficDeltaPoint { ts, delta });
    }
    points
}

fn login_backoff_status_from_raw(
    requested_ip: &str,
    raw: Option<&str>,
    now_ms: i64,
) -> LoginBackoffStatus {
    let Some(raw) = raw else {
        return LoginBackoffStatus {
            ip: requested_ip.to_string(),
            attempts: 0,
            blocked: false,
            retry_after: None,
            blocked_until: None,
        };
    };
    let Ok(state) = serde_json::from_str::<LoginBackoffAttemptState>(raw) else {
        return LoginBackoffStatus {
            ip: requested_ip.to_string(),
            attempts: 0,
            blocked: false,
            retry_after: None,
            blocked_until: None,
        };
    };
    let blocked = state
        .blocked_until
        .is_some_and(|blocked_until| now_ms <= blocked_until);
    let retry_after = if blocked {
        state
            .blocked_until
            .map(|blocked_until| ((blocked_until - now_ms).max(1000) + 999) / 1000)
    } else {
        None
    };
    LoginBackoffStatus {
        ip: requested_ip.to_string(),
        attempts: state.attempts,
        blocked,
        retry_after,
        blocked_until: state.blocked_until,
    }
}

fn waf_log_dates_for_range(from_ms: i64, to_ms: i64) -> Vec<String> {
    const DAY_MS: i64 = 86_400_000;
    let start_day = (from_ms.max(0).div_euclid(DAY_MS) - 1).max(0);
    let end_day = to_ms.max(from_ms).div_euclid(DAY_MS) + 1;
    let mut dates = BTreeSet::new();
    for day in start_day..=end_day {
        let timestamp = day.saturating_mul(DAY_MS).div_euclid(1000);
        if let Ok(date_time) = time::OffsetDateTime::from_unix_timestamp(timestamp) {
            let date = date_time.date();
            dates.insert(format!(
                "{:04}-{:02}-{:02}",
                date.year(),
                u8::from(date.month()),
                date.day()
            ));
        }
    }
    dates.into_iter().collect()
}

fn normalize_totp_credentials(totps: &[TotpCredential]) -> Vec<TotpCredential> {
    totps
        .iter()
        .filter_map(|credential| {
            normalize_totp_credential_value(
                &serde_json::to_value(credential).unwrap_or(Value::Null),
            )
        })
        .collect()
}

fn normalize_totp_credentials_value(value: &Value) -> Vec<TotpCredential> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(normalize_totp_credential_value)
        .collect()
}

fn normalize_totp_credential_value(value: &Value) -> Option<TotpCredential> {
    let object = value.as_object()?;
    let id = object
        .get("id")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let secret = object
        .get("secret")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    if id.is_empty() || secret.is_empty() {
        return None;
    }
    let comment = object
        .get("comment")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    let created_at = object
        .get("createdAt")
        .map(js_string)
        .unwrap_or_default()
        .trim()
        .to_string();
    Some(TotpCredential {
        id,
        secret,
        comment,
        created_at: if created_at.is_empty() {
            now_iso()
        } else {
            created_at
        },
        access_scopes: normalize_totp_access_scopes(
            object.get("access_scopes").cloned().unwrap_or(Value::Null),
        ),
        subdomain_access: normalize_totp_subdomain_access(
            object
                .get("subdomain_access")
                .cloned()
                .unwrap_or(Value::Null),
        ),
    })
}

pub(crate) fn normalize_totp_access_scopes(value: Value) -> Value {
    let mut scopes = Vec::new();
    if let Some(items) = value.as_array() {
        for item in items {
            let scope = js_string(item).trim().to_string();
            if scope == "docker_admin_panel"
                && !scopes
                    .iter()
                    .any(|existing: &Value| existing.as_str() == Some("docker_admin_panel"))
            {
                scopes.push(Value::String("docker_admin_panel".to_string()));
            }
        }
    }
    Value::Array(scopes)
}

pub(crate) fn normalize_totp_subdomain_access(value: Value) -> Value {
    let mode = value
        .get("mode")
        .and_then(Value::as_str)
        .filter(|mode| *mode == "custom")
        .unwrap_or("all");
    if mode != "custom" {
        return json!({ "mode": "all", "hosts": [], "streams": [] });
    }
    let hosts = value
        .get("hosts")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|host| normalize_totp_subdomain_access_host(&js_string(host)))
                .filter(|host| !host.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(Value::String)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let streams = value
        .get("streams")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_totp_stream_access)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|(listen_port, protocol)| {
                    json!({
                        "protocol": protocol,
                        "listen_port": listen_port,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    json!({ "mode": "custom", "hosts": hosts, "streams": streams })
}

fn normalize_totp_stream_access(value: &Value) -> Option<(i64, String)> {
    let protocol = value
        .get("protocol")
        .and_then(Value::as_str)?
        .trim()
        .to_ascii_lowercase();
    if protocol != "tcp" && protocol != "udp" {
        return None;
    }
    let listen_port = value.get("listen_port").and_then(Value::as_i64)?;
    (1..=65535)
        .contains(&listen_port)
        .then_some((listen_port, protocol))
}

fn normalize_totp_subdomain_access_host(value: &str) -> String {
    let mut host = value.trim().to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE || host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_WOL_PAGE || host == TOTP_SUBDOMAIN_ACCESS_WOL_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_WOL_PAGE.to_string();
    }

    if let Ok(url) = if host.contains("://") {
        url::Url::parse(&host)
    } else {
        url::Url::parse(&format!("https://{host}"))
    } {
        host = url.host_str().unwrap_or("").to_string();
    } else {
        if let Some((_, rest)) = host.split_once("://") {
            host = rest.to_string();
        }
        if let Some((_, rest)) = host.rsplit_once('@') {
            host = rest.to_string();
        }
        host = host
            .split(['/', '?', '#'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        if host.starts_with('[') {
            if let Some(end) = host.find(']') {
                host = host[1..end].to_string();
            }
        } else if host.matches(':').count() == 1
            && let Some((without_port, _)) = host.rsplit_once(':')
        {
            host = without_port.to_string();
        }
    }

    host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty()
        || host.contains('*')
        || host
            .chars()
            .any(|value| value.is_whitespace() || value == ',')
    {
        return String::new();
    }
    host
}

fn js_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn js_finite_number(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(number) => number.as_f64()?,
        Value::String(value) => js_number_from_string(value)?,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Array(items) => {
            js_number_from_string(&items.iter().map(js_string).collect::<Vec<_>>().join(","))?
        }
        Value::Object(_) => return None,
    };
    number.is_finite().then_some(number)
}

fn js_number_from_string(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };

    match radix_value {
        Some(value) => Some(value),
        None => trimmed.parse::<f64>().ok(),
    }
}

pub(crate) fn node_locale_compare_ordering(left: &str, right: &str) -> Ordering {
    if left == right {
        return Ordering::Equal;
    }

    let mut left_chars = left.chars();
    let mut right_chars = right.chars();

    loop {
        match (left_chars.next(), right_chars.next()) {
            (Some(left), Some(right)) if left == right => {}
            (Some(left), Some(right)) => {
                let left_key = node_locale_char_key(left);
                let right_key = node_locale_char_key(right);
                let ordering = left_key.cmp(&right_key);
                return if ordering == Ordering::Equal {
                    left.cmp(&right)
                } else {
                    ordering
                };
            }
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return Ordering::Equal,
        }
    }
}

fn node_locale_char_key(value: char) -> (u16, u16, u16, u32) {
    if let Some(rank) = node_ascii_punctuation_rank(value) {
        return (rank, 0, 0, value as u32);
    }

    if value.is_ascii_digit() {
        return (100 + value as u16 - b'0' as u16, 0, 0, value as u32);
    }

    if value.is_ascii_alphabetic() {
        let lower = value.to_ascii_lowercase();
        let letter = lower as u16 - b'a' as u16;
        let case = if value.is_ascii_lowercase() { 0 } else { 1 };
        return (200 + letter, 0, case, value as u32);
    }

    if let Some((letter, accent, case)) = node_latin_accent_key(value) {
        return (200 + letter, accent, case, value as u32);
    }

    if matches!(
        value as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    ) {
        return (400, 0, 0, value as u32);
    }

    (50, 0, 0, value as u32)
}

fn node_ascii_punctuation_rank(value: char) -> Option<u16> {
    match value {
        ' ' => Some(0),
        '_' => Some(1),
        '-' => Some(2),
        ',' => Some(3),
        ';' => Some(4),
        ':' => Some(5),
        '!' => Some(6),
        '?' => Some(7),
        '.' => Some(8),
        '\'' => Some(9),
        '"' => Some(10),
        '(' => Some(11),
        ')' => Some(12),
        '[' => Some(13),
        ']' => Some(14),
        '{' => Some(15),
        '}' => Some(16),
        '@' => Some(17),
        '*' => Some(18),
        '/' => Some(19),
        '\\' => Some(20),
        '&' => Some(21),
        '#' => Some(22),
        '%' => Some(23),
        '`' => Some(24),
        '^' => Some(25),
        '+' => Some(26),
        '<' => Some(27),
        '=' => Some(28),
        '>' => Some(29),
        '|' => Some(30),
        '~' => Some(31),
        '$' => Some(32),
        _ => None,
    }
}

fn node_latin_accent_key(value: char) -> Option<(u16, u16, u16)> {
    let case = if value.is_lowercase() { 0 } else { 1 };
    let (letter, accent) = match value {
        'á' | 'Á' => (0, 1),
        'å' | 'Å' => (0, 2),
        'ä' | 'Ä' => (0, 3),
        'à' | 'À' => (0, 4),
        'â' | 'Â' => (0, 5),
        'ã' | 'Ã' => (0, 6),
        'ā' | 'Ā' => (0, 7),
        'ă' | 'Ă' => (0, 8),
        'ą' | 'Ą' => (0, 9),
        'ç' | 'Ç' => (2, 1),
        'é' | 'É' => (4, 1),
        'è' | 'È' => (4, 2),
        'ê' | 'Ê' => (4, 3),
        'ë' | 'Ë' => (4, 4),
        'í' | 'Í' => (8, 1),
        'ì' | 'Ì' => (8, 2),
        'î' | 'Î' => (8, 3),
        'ï' | 'Ï' => (8, 4),
        'ñ' | 'Ñ' => (13, 1),
        'ó' | 'Ó' => (14, 1),
        'ò' | 'Ò' => (14, 2),
        'ô' | 'Ô' => (14, 3),
        'ö' | 'Ö' => (14, 4),
        'õ' | 'Õ' => (14, 5),
        'ú' | 'Ú' => (20, 1),
        'ù' | 'Ù' => (20, 2),
        'û' | 'Û' => (20, 3),
        'ü' | 'Ü' => (20, 4),
        'ý' | 'Ý' => (24, 1),
        'ÿ' | 'Ÿ' => (24, 2),
        _ => return None,
    };
    Some((letter, accent, case))
}

#[allow(dead_code)]
pub fn new_login_session(
    totp_id: &str,
    credential_name: &str,
    ip: &str,
    user_agent: &str,
    ttl_seconds: i64,
) -> LoginSession {
    LoginSession {
        totp_id: totp_id.to_string(),
        method: "TOTP".to_string(),
        credential_id: totp_id.to_string(),
        credential_name: credential_name.to_string(),
        linked_totp_name: None,
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        stream_access_expires_at: None,
        comment: None,
        ip: ip.to_string(),
        user_agent: user_agent.to_string(),
        login_time: now_iso(),
        expires_at: Some(iso_after_seconds(ttl_seconds)),
        ip_location: None,
    }
}
