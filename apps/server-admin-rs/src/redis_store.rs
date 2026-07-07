use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashMap, HashSet},
    str::FromStr,
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ipnet::IpNet;
use redis::{
    AsyncCommands,
    aio::ConnectionManager,
    streams::{StreamRangeReply, StreamReadOptions, StreamReadReply},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

use crate::{
    http_utils::normalize_ip,
    time_utils::{iso_after_seconds, now_iso},
};

#[derive(Clone)]
pub struct RedisStore {
    manager: ConnectionManager,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TotpCredential {
    pub id: String,
    pub secret: String,
    #[serde(default)]
    pub comment: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: String,
    #[serde(default)]
    pub access_scopes: Value,
    #[serde(default)]
    pub subdomain_access: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginSession {
    #[serde(rename = "totpId")]
    pub totp_id: String,
    pub method: String,
    #[serde(rename = "credentialId")]
    pub credential_id: String,
    #[serde(rename = "credentialName")]
    pub credential_name: String,
    #[serde(rename = "linkedTotpName", skip_serializing_if = "Option::is_none")]
    pub linked_totp_name: Option<String>,
    #[serde(rename = "grantType", skip_serializing_if = "Option::is_none")]
    pub grant_type: Option<String>,
    #[serde(
        default,
        rename = "postLoginIpGrantMode",
        skip_serializing_if = "Option::is_none"
    )]
    pub post_login_ip_grant_mode: Option<String>,
    #[serde(
        default,
        rename = "postLoginIpGrantRecordId",
        skip_serializing_if = "Option::is_none"
    )]
    pub post_login_ip_grant_record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    pub ip: String,
    #[serde(rename = "userAgent")]
    pub user_agent: String,
    #[serde(rename = "loginTime")]
    pub login_time: String,
    #[serde(rename = "expiresAt", skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
    #[serde(rename = "ipLocation", skip_serializing_if = "Option::is_none")]
    pub ip_location: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerAdminPasswordRecord {
    pub algorithm: String,
    pub salt: String,
    pub hash: String,
    pub n: u32,
    pub r: u32,
    pub p: u32,
    pub key_length: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DockerAdminSessionRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    #[serde(default)]
    pub ttl_seconds: i64,
    pub ip: String,
    pub user_agent: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginAttemptRecord {
    pub ip: String,
    pub attempts: u32,
    pub last_attempt_at: String,
    pub blocked_until: i64,
}

#[derive(Clone, Debug, Serialize)]
pub struct DockerAdminResetSummary {
    pub password_cleared: bool,
    pub sessions_cleared: usize,
    pub login_failures_cleared: usize,
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

#[derive(Clone, Debug, Serialize)]
pub struct LoginBackoffStatus {
    pub ip: String,
    pub attempts: i64,
    pub blocked: bool,
    #[serde(skip_serializing_if = "Option::is_none", rename = "retryAfter")]
    pub retry_after: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "blockedUntil")]
    pub blocked_until: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhitelistRecord {
    pub id: String,
    pub ip: String,
    #[serde(default = "default_whitelist_target_type", rename = "targetType")]
    pub target_type: String,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    #[serde(default = "default_whitelist_source")]
    pub source: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: i64,
    #[serde(default = "default_whitelist_status")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "ipLocation")]
    pub ip_location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolvedTargets")]
    pub resolved_targets: Option<Vec<String>>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        rename = "checkIntervalMinutes"
    )]
    pub check_interval_minutes: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastCheckedAt")]
    pub last_checked_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "lastResolvedAt")]
    pub last_resolved_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolveStatus")]
    pub resolve_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "resolveMessage")]
    pub resolve_message: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhitelistConcreteTarget {
    #[serde(rename = "recordId")]
    pub record_id: String,
    #[serde(rename = "recordTarget")]
    pub record_target: String,
    #[serde(rename = "recordTargetType")]
    pub record_target_type: String,
    pub source: String,
    pub target: String,
    #[serde(rename = "targetType")]
    pub target_type: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WhitelistRegionInput {
    pub province: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_city: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WhitelistRegionGroupRecord {
    pub id: String,
    #[serde(default)]
    pub regions: Vec<WhitelistRegionInput>,
    #[serde(default)]
    pub cidrs: Vec<String>,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    #[serde(default = "default_whitelist_source")]
    pub source: String,
    #[serde(default, rename = "createdAt")]
    pub created_at: i64,
    #[serde(default, rename = "updatedAt")]
    pub updated_at: i64,
    #[serde(default = "default_whitelist_status")]
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct WhitelistRegionGroupSummary {
    pub id: String,
    pub regions: Vec<WhitelistRegionInput>,
    #[serde(rename = "expireAt")]
    pub expire_at: Option<i64>,
    pub source: String,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    #[serde(rename = "updatedAt")]
    pub updated_at: i64,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(rename = "cidrCount")]
    pub cidr_count: usize,
}

impl WhitelistRecord {
    pub fn target_type(&self) -> &str {
        match self.target_type.as_str() {
            "cidr" => "cidr",
            "cname" => "cname",
            _ => "ip",
        }
    }

    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn concrete_targets(&self) -> Vec<WhitelistConcreteTarget> {
        match self.target_type() {
            "cidr" => vec![self.concrete_target(&self.ip, "cidr")],
            "cname" => self
                .resolved_targets
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|target| {
                    let normalized = normalize_ip(&target);
                    (!normalized.is_empty()).then_some(normalized)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|target| self.concrete_target(&target, "ip"))
                .collect(),
            _ => vec![self.concrete_target(&self.ip, "ip")],
        }
    }

    fn concrete_target(&self, target: &str, target_type: &str) -> WhitelistConcreteTarget {
        WhitelistConcreteTarget {
            record_id: self.id.clone(),
            record_target: self.ip.clone(),
            record_target_type: self.target_type().to_string(),
            source: if self.source == "auto" {
                "auto".to_string()
            } else {
                "manual".to_string()
            },
            target: target.to_string(),
            target_type: target_type.to_string(),
        }
    }
}

impl WhitelistRegionGroupRecord {
    pub fn is_active(&self) -> bool {
        self.status == "active"
    }

    pub fn summary(&self) -> WhitelistRegionGroupSummary {
        WhitelistRegionGroupSummary {
            id: self.id.clone(),
            regions: self.regions.clone(),
            expire_at: self.expire_at,
            source: self.source.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            status: self.status.clone(),
            comment: self.comment.clone(),
            cidr_count: self.cidrs.len(),
        }
    }

    pub fn concrete_targets(&self) -> Vec<WhitelistConcreteTarget> {
        self.cidrs
            .iter()
            .map(|cidr| WhitelistConcreteTarget {
                record_id: self.id.clone(),
                record_target: self.id.clone(),
                record_target_type: "cidr".to_string(),
                source: self.source.clone(),
                target: cidr.clone(),
                target_type: "cidr".to_string(),
            })
            .collect()
    }
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
            Some(WhitelistRegionInput {
                province,
                query_city: (!query_city.is_empty()).then_some(query_city),
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
    let trimmed = value.trim_start();
    let mut chars = trimmed.chars().peekable();
    let mut sign = 1i64;
    if matches!(chars.peek(), Some('+')) {
        chars.next();
    } else if matches!(chars.peek(), Some('-')) {
        chars.next();
        sign = -1;
    }
    let digits = chars
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i64>().ok().map(|value| value * sign)
}

#[derive(Clone, Debug, PartialEq)]
pub struct TrafficDeltaPoint {
    pub ts: i64,
    pub delta: f64,
}

#[derive(Clone, Debug)]
pub struct TrafficSnapshotRecord {
    pub host: Option<String>,
    pub total_in: f64,
    pub total_out: f64,
    pub error_5xx: f64,
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
const AUTH_MOBILITY_PREFIX: &str = "fn_knock:auth_mobility";
const REVERSE_PROXY_TRUSTED_IPS_RUNTIME: &str = "fn_knock:reverse-proxy:trusted-ips:runtime";
const EVENTS_STREAM_KEY: &str = "fn_knock:events:stream";
const EVENTS_INDEX_KEY: &str = "fn_knock:events:index";
const EVENTS_DATA_PREFIX: &str = "fn_knock:events:data:";
const EVENTS_DEDUPE_PREFIX: &str = "fn_knock:events:dedupe:";
const EVENTS_STREAM_ID_PREFIX: &str = "fn_knock:events:stream-id:";
const NOTIFICATION_RUNTIME_LAST_STREAM_KEY: &str = "fn_knock:notifications:runtime:last-stream-id";
const NOTIFICATION_RUNTIME_LOCK_PREFIX: &str = "fn_knock:notifications:runtime:lock:";
const NOTIFICATION_RUNTIME_COOLDOWN_PREFIX: &str = "fn_knock:notifications:runtime:cooldown:";
const NOTIFICATION_RUNTIME_WINDOW_PREFIX: &str = "fn_knock:notifications:runtime:window:";
const NOTIFICATION_DELIVERIES_READY_KEY: &str = "fn_knock:notifications:deliveries:ready";
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
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE: &str = "__builtin_select__";
const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH: &str = "/__select__";
const EVENT_LIST_SCAN_CHUNK_SIZE: isize = 200;
const EVENT_CLEAR_CHUNK_SIZE: usize = 500;
const MAX_EVENT_RETENTION_DAYS: i64 = 90;
const LOGIN_BACKOFF_REGISTER_FAILURE_SCRIPT: &str = r#"
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

fn auth_mobility_active_ip_details_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:active_ip_details:{session_id}")
}

fn auth_mobility_active_ip_zset_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:active_ips:{session_id}")
}

fn auth_mobility_binding_key(subject_type: &str, subject_hash: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:binding:{subject_type}:{subject_hash}")
}

fn auth_mobility_session_index_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:session:{session_id}")
}

fn auth_mobility_summary_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:summary:{session_id}")
}

fn auth_mobility_timeline_key(session_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:timeline:{session_id}")
}

fn auth_mobility_whitelist_owner_key(whitelist_record_id: &str) -> String {
    format!("{AUTH_MOBILITY_PREFIX}:whitelist:{whitelist_record_id}:session")
}

fn auth_mobility_subject_hash(subject_type: &str, subject_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{subject_type}:{subject_key}"));
    hex::encode(hasher.finalize())
}

fn traffic_scope_segment(user_id: &str, host: Option<&str>) -> String {
    let host = host.map(str::trim).filter(|value| !value.is_empty());
    match host {
        Some(host) => {
            let encoded: String = url::form_urlencoded::byte_serialize(host.as_bytes()).collect();
            format!("{user_id}:host:{encoded}")
        }
        None => user_id.to_string(),
    }
}

fn traffic_key(user_id: &str, direction: &str, host: Option<&str>) -> String {
    format!(
        "fn_knock:traffic:{}:{}",
        traffic_scope_segment(user_id, host),
        direction
    )
}

fn traffic_last_total_key(user_id: &str, direction: &str, host: Option<&str>) -> String {
    format!(
        "fn_knock:traffic:last:{}:{}",
        traffic_scope_segment(user_id, host),
        direction
    )
}

fn error5xx_key(user_id: &str, host: Option<&str>) -> String {
    format!(
        "fn_knock:errors:{}:5xx",
        traffic_scope_segment(user_id, host)
    )
}

fn error5xx_last_total_key(user_id: &str, host: Option<&str>) -> String {
    format!(
        "fn_knock:errors:last:{}:5xx",
        traffic_scope_segment(user_id, host)
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

impl RedisStore {
    pub async fn connect(redis_url: &str) -> redis::RedisResult<Self> {
        let client = redis::Client::open(redis_url)?;
        let manager = client.get_connection_manager().await?;
        Ok(Self { manager })
    }

    fn conn(&self) -> ConnectionManager {
        self.manager.clone()
    }

    pub async fn ping(&self) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        redis::cmd("PING").query_async(&mut conn).await
    }

    pub async fn get_json_value(&self, key: &str) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn get_string_value(&self, key: &str) -> redis::RedisResult<Option<String>> {
        let mut conn = self.conn();
        conn.get(key).await
    }

    pub async fn set_string_value_with_optional_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: Option<i64>,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        if let Some(ttl_seconds) = ttl_seconds.filter(|value| *value > 0) {
            let _: () = conn.set_ex(key, value, ttl_seconds as u64).await?;
        } else {
            let _: () = conn.set(key, value).await?;
        }
        Ok(())
    }

    pub async fn set_string_value(&self, key: &str, value: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set(key, value).await
    }

    pub async fn set_key_if_not_exists_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn delete_key_if_value(&self, key: &str, value: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let _: i64 = redis::cmd("EVAL")
            .arg(
                r#"
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                    return redis.call('DEL', KEYS[1])
                end
                return 0
                "#,
            )
            .arg(1)
            .arg(key)
            .arg(value)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn delete_key(&self, key: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(key).await
    }

    pub async fn delete_key_count(&self, key: &str) -> redis::RedisResult<usize> {
        let mut conn = self.conn();
        conn.del(key).await
    }

    pub async fn mget_string_values(
        &self,
        keys: &[String],
    ) -> redis::RedisResult<Vec<Option<String>>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.conn();
        redis::cmd("MGET").arg(keys).query_async(&mut conn).await
    }

    pub async fn consume_json_value(&self, key: &str) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = redis::cmd("EVAL")
            .arg(
                r#"
local value = redis.call("GET", KEYS[1])
if not value then
  return nil
end
redis.call("DEL", KEYS[1])
return value
"#,
            )
            .arg(1)
            .arg(key)
            .query_async(&mut conn)
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn hgetall_string_map(
        &self,
        key: &str,
    ) -> redis::RedisResult<HashMap<String, String>> {
        let mut conn = self.conn();
        conn.hgetall(key).await
    }

    pub async fn replace_hash_string_map(
        &self,
        key: &str,
        values: &HashMap<String, String>,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        if values.is_empty() {
            conn.del(key).await
        } else {
            let mut pipe = redis::pipe();
            pipe.del(key).ignore();
            pipe.hset_multiple(key, &values.iter().collect::<Vec<_>>())
                .ignore();
            let _: () = pipe.query_async(&mut conn).await?;
            Ok(())
        }
    }

    pub async fn smembers_strings(&self, key: &str) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(key).await
    }

    pub async fn sadd_string_member(&self, key: &str, member: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.sadd(key, member).await
    }

    pub async fn srem_string_member(&self, key: &str, member: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.srem(key, member).await
    }

    pub async fn srem_string_members(
        &self,
        key: &str,
        members: &[String],
    ) -> redis::RedisResult<()> {
        if members.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(key, members).await
    }

    pub async fn zrevrange_strings(&self, key: &str) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.zrevrange(key, 0, -1).await
    }

    pub async fn zadd_string_member(
        &self,
        key: &str,
        member: &str,
        score: i64,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.zadd(key, member, score).await
    }

    pub async fn zrem_string_member(&self, key: &str, member: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.zrem(key, member).await
    }

    pub async fn zadd_trim_count_expire(
        &self,
        key: &str,
        member: &str,
        score: i64,
        min_score: i64,
        ttl_seconds: usize,
    ) -> redis::RedisResult<i64> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(key, member, score).ignore();
        pipe.zrembyscore(key, 0, min_score - 1).ignore();
        pipe.expire(key, ttl_seconds.max(1) as i64).ignore();
        pipe.zcard(key);
        let values: Vec<i64> = pipe.query_async(&mut conn).await?;
        Ok(values.into_iter().next().unwrap_or_default())
    }

    pub async fn set_string_and_zadd(
        &self,
        data_key: &str,
        value: &str,
        index_key: &str,
        member: &str,
        score: i64,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set(data_key, value)
            .ignore()
            .zadd(index_key, member, score)
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_string_and_zrem(
        &self,
        data_key: &str,
        index_key: &str,
        member: &str,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.del(data_key).ignore().zrem(index_key, member).ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn save_expiring_string_and_sadd(
        &self,
        data_key: &str,
        value: &str,
        ttl_seconds: usize,
        set_key: &str,
        member: &str,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let ttl = ttl_seconds.max(1);
        let mut pipe = redis::pipe();
        pipe.set_ex(data_key, value, ttl as u64)
            .ignore()
            .sadd(set_key, member)
            .ignore()
            .expire(set_key, ttl as i64)
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_string_and_srem(
        &self,
        data_key: &str,
        set_key: &str,
        member: &str,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.del(data_key).ignore().srem(set_key, member).ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn delete_keys(&self, keys: &[String]) -> redis::RedisResult<()> {
        if keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.del(keys).await
    }

    pub async fn delete_keys_count(&self, keys: &[String]) -> redis::RedisResult<usize> {
        if keys.is_empty() {
            return Ok(0);
        }
        let mut conn = self.conn();
        conn.del(keys).await
    }

    pub async fn scan_keys(&self, prefix: &str, count: usize) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys = BTreeSet::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{prefix}*"))
                .arg("COUNT")
                .arg(count.max(1))
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        let mut keys = keys.into_iter().collect::<Vec<_>>();
        keys.sort_by(|left, right| node_locale_compare_ordering(left, right));
        Ok(keys)
    }

    pub async fn clear_keys_by_prefix(
        &self,
        prefix: &str,
        count: usize,
    ) -> redis::RedisResult<usize> {
        let keys = self.scan_keys(prefix, count).await?;
        let mut deleted = 0;
        for chunk in keys.chunks(200) {
            deleted += self.delete_keys_count(chunk).await?;
        }
        Ok(deleted)
    }

    pub async fn append_log_buffer(
        &self,
        key: &str,
        lines: &[String],
        ttl_seconds: usize,
        max_len: usize,
    ) -> redis::RedisResult<()> {
        if lines.is_empty() {
            return Ok(());
        }
        let seq_key = format!("{key}:seq");
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.cmd("RPUSH")
            .arg(key)
            .arg(lines)
            .ignore()
            .cmd("LTRIM")
            .arg(key)
            .arg(-(max_len.max(1) as i64))
            .arg(-1)
            .ignore()
            .cmd("INCRBY")
            .arg(&seq_key)
            .arg(lines.len() as i64)
            .ignore()
            .cmd("EXPIRE")
            .arg(key)
            .arg(ttl_seconds.max(1))
            .ignore()
            .cmd("EXPIRE")
            .arg(&seq_key)
            .arg(ttl_seconds.max(1))
            .ignore();
        pipe.query_async(&mut conn).await
    }

    pub async fn list_log_buffer(
        &self,
        key: &str,
        limit: usize,
        max_len: usize,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        let safe_limit = limit.max(1).min(max_len.max(1)) as i64;
        conn.lrange(key, -(safe_limit as isize), -1).await
    }

    pub async fn clear_log_buffer(&self, key: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let seq_key = format!("{key}:seq");
        conn.del(&[key, seq_key.as_str()]).await
    }

    pub async fn poll_log_buffer(
        &self,
        key: &str,
        cursor: Option<&str>,
    ) -> redis::RedisResult<Value> {
        let mut conn = self.conn();
        let seq_key = format!("{key}:seq");
        let total_len: i64 = conn.llen(key).await?;
        let raw_seq: Option<String> = conn.get(&seq_key).await?;
        let total_seq = raw_seq
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0)
            .unwrap_or(total_len);
        let retained_start_seq = (total_seq - total_len).max(0);
        let requested_cursor = cursor
            .and_then(|value| value.parse::<i64>().ok())
            .filter(|value| *value >= 0);
        let reset =
            requested_cursor.is_some_and(|value| value < retained_start_seq || value > total_seq);
        let from = if requested_cursor.is_none() || reset {
            0
        } else {
            (requested_cursor.unwrap_or(0) - retained_start_seq).max(0)
        };
        let items: Vec<String> = if total_len > 0 && from < total_len {
            conn.lrange(key, from as isize, -1).await?
        } else {
            Vec::new()
        };
        Ok(json!({
            "cursor": total_seq,
            "reset": reset,
            "items": items
        }))
    }

    pub async fn export_redis_backup_entry(&self, key: &str) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let value_type: String = redis::cmd("TYPE").arg(key).query_async(&mut conn).await?;
        if value_type == "none" {
            return Ok(None);
        }
        let ttl_ms: i64 = redis::cmd("PTTL").arg(key).query_async(&mut conn).await?;
        let ttl = if ttl_ms > 0 {
            Value::Number(ttl_ms.into())
        } else {
            Value::Null
        };

        match value_type.as_str() {
            "string" => {
                let value: Option<String> = conn.get(key).await?;
                Ok(value.map(|value| {
                    json!({
                        "key": key,
                        "type": "string",
                        "ttl_ms": ttl,
                        "value": value,
                    })
                }))
            }
            "hash" => {
                let value: HashMap<String, String> = conn.hgetall(key).await?;
                Ok(Some(json!({
                    "key": key,
                    "type": "hash",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "list" => {
                let value: Vec<String> = conn.lrange(key, 0, -1).await?;
                Ok(Some(json!({
                    "key": key,
                    "type": "list",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "set" => {
                let mut value: Vec<String> = conn.smembers(key).await?;
                value.sort_by(|left, right| node_locale_compare_ordering(left, right));
                Ok(Some(json!({
                    "key": key,
                    "type": "set",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "zset" => {
                let pairs: Vec<(String, f64)> = redis::cmd("ZRANGE")
                    .arg(key)
                    .arg(0)
                    .arg(-1)
                    .arg("WITHSCORES")
                    .query_async(&mut conn)
                    .await?;
                let value = pairs
                    .into_iter()
                    .map(|(member, score)| json!({ "member": member, "score": score }))
                    .collect::<Vec<_>>();
                Ok(Some(json!({
                    "key": key,
                    "type": "zset",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            "stream" => {
                let response: Vec<(String, Vec<String>)> = redis::cmd("XRANGE")
                    .arg(key)
                    .arg("-")
                    .arg("+")
                    .query_async(&mut conn)
                    .await?;
                let value = response
                    .into_iter()
                    .filter(|(_, fields)| !fields.is_empty() && fields.len() % 2 == 0)
                    .map(|(id, fields)| json!({ "id": id, "fields": fields }))
                    .collect::<Vec<_>>();
                Ok(Some(json!({
                    "key": key,
                    "type": "stream",
                    "ttl_ms": ttl,
                    "value": value,
                })))
            }
            _ => Ok(Some(json!({
                "key": key,
                "type": value_type,
                "ttl_ms": ttl,
                "value": Value::Null,
            }))),
        }
    }

    pub async fn restore_redis_backup_entries(&self, entries: &[Value]) -> redis::RedisResult<()> {
        const PIPELINE_BATCH_SIZE: usize = 100;

        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        let mut batched_commands = 0usize;

        for entry in entries {
            let key = entry.get("key").and_then(Value::as_str).unwrap_or("");
            let value_type = entry.get("type").and_then(Value::as_str).unwrap_or("");
            let ttl_ms = entry
                .get("ttl_ms")
                .and_then(Value::as_i64)
                .filter(|value| *value > 0);
            if key.is_empty() {
                continue;
            }

            match value_type {
                "string" => {
                    let command = pipe
                        .cmd("SET")
                        .arg(key)
                        .arg(entry.get("value").and_then(Value::as_str).unwrap_or(""));
                    if let Some(ttl_ms) = ttl_ms {
                        command.arg("PX").arg(ttl_ms);
                    }
                    command.ignore();
                    batched_commands += 1;
                }
                "hash" => {
                    if let Some(object) = entry.get("value").and_then(Value::as_object)
                        && !object.is_empty()
                    {
                        let pairs = object
                            .iter()
                            .filter_map(|(field, value)| {
                                value.as_str().map(|text| (field.as_str(), text))
                            })
                            .collect::<Vec<_>>();
                        if pairs.is_empty() {
                            continue;
                        }
                        pipe.cmd("HSET").arg(key);
                        for (field, value) in pairs {
                            pipe.arg(field).arg(value);
                        }
                        pipe.ignore();
                        batched_commands += 1;
                    }
                }
                "list" => {
                    if let Some(items) = entry.get("value").and_then(Value::as_array)
                        && !items.is_empty()
                    {
                        pipe.cmd("RPUSH").arg(key);
                        for item in items {
                            pipe.arg(item.as_str().unwrap_or(""));
                        }
                        pipe.ignore();
                        batched_commands += 1;
                    }
                }
                "set" => {
                    if let Some(items) = entry.get("value").and_then(Value::as_array)
                        && !items.is_empty()
                    {
                        pipe.cmd("SADD").arg(key);
                        for item in items {
                            pipe.arg(item.as_str().unwrap_or(""));
                        }
                        pipe.ignore();
                        batched_commands += 1;
                    }
                }
                "zset" => {
                    if let Some(items) = entry.get("value").and_then(Value::as_array)
                        && !items.is_empty()
                    {
                        pipe.cmd("ZADD").arg(key);
                        for item in items {
                            pipe.arg(item.get("score").and_then(Value::as_f64).unwrap_or(0.0))
                                .arg(item.get("member").and_then(Value::as_str).unwrap_or(""));
                        }
                        pipe.ignore();
                        batched_commands += 1;
                    }
                }
                "stream" => {
                    if let Some(items) = entry.get("value").and_then(Value::as_array) {
                        for item in items {
                            let id = item.get("id").and_then(Value::as_str).unwrap_or("*");
                            let Some(fields) = item.get("fields").and_then(Value::as_array) else {
                                continue;
                            };
                            if fields.is_empty() || fields.len() % 2 != 0 {
                                continue;
                            }
                            pipe.cmd("XADD").arg(key).arg(id);
                            for field in fields {
                                pipe.arg(field.as_str().unwrap_or(""));
                            }
                            pipe.ignore();
                            batched_commands += 1;
                            if batched_commands >= PIPELINE_BATCH_SIZE {
                                pipe.query_async::<()>(&mut conn).await?;
                                pipe = redis::pipe();
                                batched_commands = 0;
                            }
                        }
                    }
                }
                _ => {}
            }

            if ttl_ms.is_some() && !matches!(value_type, "none" | "string") {
                pipe.cmd("PEXPIRE").arg(key).arg(ttl_ms.unwrap()).ignore();
                batched_commands += 1;
            }

            if batched_commands >= PIPELINE_BATCH_SIZE {
                pipe.query_async::<()>(&mut conn).await?;
                pipe = redis::pipe();
                batched_commands = 0;
            }
        }

        if batched_commands > 0 {
            pipe.query_async::<()>(&mut conn).await?;
        }
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn set_json_value(&self, key: &str, value: &Value) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set(
            key,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
        )
        .await
    }

    pub async fn set_json_value_ex(
        &self,
        key: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            key,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn set_json_value_nx_ex(
        &self,
        key: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(serialized)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn set_json_lock_if_owned_ex(
        &self,
        key: &str,
        lock_id: &str,
        value: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
local raw = redis.call("GET", KEYS[1])
if not raw then
  return 0
end
local ok, decoded = pcall(cjson.decode, raw)
if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
  return 0
end
redis.call("SET", KEYS[1], ARGV[2], "EX", tonumber(ARGV[3]))
return 1
"#,
            )
            .arg(1)
            .arg(key)
            .arg(lock_id)
            .arg(serialized)
            .arg(ttl_seconds.max(1).to_string())
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn delete_lock_if_owned(&self, key: &str, lock_id: &str) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
local raw = redis.call("GET", KEYS[1])
if not raw then
  return 0
end
local ok, decoded = pcall(cjson.decode, raw)
if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
  return 0
end
redis.call("DEL", KEYS[1])
return 1
"#,
            )
            .arg(1)
            .arg(key)
            .arg(lock_id)
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn get_config(&self) -> redis::RedisResult<Value> {
        Ok(self
            .get_json_value("fn_knock:config")
            .await?
            .unwrap_or_else(default_config))
    }

    #[allow(dead_code)]
    pub async fn save_config(&self, value: &Value) -> redis::RedisResult<()> {
        self.set_json_value("fn_knock:config", value).await
    }

    pub async fn locale(&self) -> redis::RedisResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("locale")
            .cloned()
            .unwrap_or_else(|| json!({ "default_locale": "zh-CN" })))
    }

    pub async fn appearance(&self) -> redis::RedisResult<Value> {
        let config = self.get_config().await?;
        Ok(config
            .get("appearance")
            .cloned()
            .unwrap_or_else(|| json!({ "theme_color_preset": "default" })))
    }

    #[allow(dead_code)]
    pub async fn captcha_public_settings(&self) -> redis::RedisResult<Value> {
        let config = self.get_config().await?;
        let settings = config.get("captcha").cloned().unwrap_or_else(|| {
            json!({
                "provider": "pow",
                "widget_mode": "normal",
                "pow": {},
                "turnstile": { "site_key": "", "secret_key": "" }
            })
        });
        let provider = settings
            .get("provider")
            .and_then(Value::as_str)
            .unwrap_or("pow");
        let site_key = settings
            .pointer("/turnstile/site_key")
            .and_then(Value::as_str)
            .unwrap_or("");
        Ok(json!({
            "provider": provider,
            "widget_mode": "normal",
            "available": true,
            "unavailable_reason": null,
            "pow": {},
            "turnstile": { "site_key": site_key }
        }))
    }

    pub async fn get_totps(&self) -> redis::RedisResult<Vec<TotpCredential>> {
        let raw: Option<String> = {
            let mut conn = self.conn();
            conn.get("fn_knock:totps").await?
        };
        let Some(raw) = raw else {
            let old_secret: Option<String> = {
                let mut conn = self.conn();
                conn.get("fn_knock:totp_secret").await?
            };
            let Some(old_secret) = old_secret.filter(|value| !value.is_empty()) else {
                return Ok(Vec::new());
            };
            let legacy = TotpCredential {
                id: "legacy-totp-id".to_string(),
                secret: old_secret,
                comment: "默认凭据".to_string(),
                created_at: now_iso(),
                access_scopes: Value::Array(Vec::new()),
                subdomain_access: normalize_totp_subdomain_access(Value::Null),
            };
            self.set_totps(std::slice::from_ref(&legacy)).await?;
            {
                let mut conn = self.conn();
                let _: () = conn.del("fn_knock:totp_secret").await?;
            }
            let mut passkeys = self.get_passkeys().await?;
            let mut passkeys_modified = false;
            for passkey in &mut passkeys {
                let missing_totp_id = passkey
                    .get("totpId")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .is_none();
                if missing_totp_id && let Some(object) = passkey.as_object_mut() {
                    object.insert("totpId".to_string(), Value::String(legacy.id.clone()));
                    passkeys_modified = true;
                }
            }
            if passkeys_modified {
                self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                    .await?;
            }
            let normalized = normalize_totp_credentials(std::slice::from_ref(&legacy));
            return Ok(normalized);
        };
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::Null);
        Ok(normalize_totp_credentials_value(&value))
    }

    pub async fn set_totps(&self, totps: &[TotpCredential]) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let normalized = normalize_totp_credentials(totps);
        conn.set(
            "fn_knock:totps",
            serde_json::to_string(&normalized).unwrap_or_default(),
        )
        .await
    }

    pub async fn add_totp(&self, credential: TotpCredential) -> redis::RedisResult<()> {
        let mut totps = self.get_totps().await?;
        if let Some(credential) = normalize_totp_credential_value(
            &serde_json::to_value(credential).unwrap_or(Value::Null),
        ) {
            totps.push(credential);
        }
        self.set_totps(&totps).await
    }

    pub async fn update_totp_comment(
        &self,
        id: &str,
        comment: String,
    ) -> redis::RedisResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.comment = comment.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_access_scopes(
        &self,
        id: &str,
        access_scopes: Value,
    ) -> redis::RedisResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_access_scopes(access_scopes);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.access_scopes = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn update_totp_subdomain_access(
        &self,
        id: &str,
        subdomain_access: Value,
    ) -> redis::RedisResult<Option<TotpCredential>> {
        let mut totps = self.get_totps().await?;
        let normalized = normalize_totp_subdomain_access(subdomain_access);
        let mut updated = None;
        for credential in &mut totps {
            if credential.id == id {
                credential.subdomain_access = normalized.clone();
                updated = Some(credential.clone());
                break;
            }
        }
        if updated.is_some() {
            self.set_totps(&totps).await?;
        }
        Ok(updated)
    }

    pub async fn delete_totp(&self, id: &str) -> redis::RedisResult<bool> {
        let mut totps = self.get_totps().await?;
        let original_len = totps.len();
        totps.retain(|credential| credential.id != id);
        if totps.len() == original_len {
            return Ok(false);
        }
        self.set_totps(&totps).await?;
        let mut passkeys = self.get_passkeys().await?;
        let passkeys_original_len = passkeys.len();
        passkeys.retain(|passkey| passkey.get("totpId").and_then(Value::as_str) != Some(id));
        if passkeys.len() != passkeys_original_len {
            self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
                .await?;
        }
        Ok(true)
    }

    pub async fn set_nonce_if_not_exists(
        &self,
        nonce: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let key = format!("fn_knock:nonce:{nonce}");
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds)
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn set_lock_if_not_exists(
        &self,
        lock_name: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let key = format!("fn_knock:lock:{lock_name}");
        let result: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn get_login_backoff_status(
        &self,
        ip: &str,
    ) -> redis::RedisResult<LoginBackoffStatus> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(login_backoff_key(ip)).await?;
        Ok(login_backoff_status_from_raw(
            ip,
            raw.as_deref(),
            crate::time_utils::now_ms(),
        ))
    }

    pub async fn register_login_backoff_failure(
        &self,
        ip: &str,
    ) -> redis::RedisResult<LoginBackoffStatus> {
        let now = crate::time_utils::now_ms();
        let mut conn = self.conn();
        let result: Vec<i64> = redis::cmd("EVAL")
            .arg(LOGIN_BACKOFF_REGISTER_FAILURE_SCRIPT)
            .arg(1)
            .arg(login_backoff_key(ip))
            .arg(ip)
            .arg(now)
            .arg(LOGIN_BACKOFF_TTL_SECONDS)
            .arg(2000)
            .arg(3_600_000)
            .arg("0.4")
            .query_async(&mut conn)
            .await?;
        let attempts = result.first().copied().unwrap_or_default();
        let retry_after = result.get(1).copied().unwrap_or_default().max(0);
        let blocked_until = result.get(2).copied();
        Ok(LoginBackoffStatus {
            ip: ip.to_string(),
            attempts,
            blocked: blocked_until.is_some_and(|until| now <= until),
            retry_after: (retry_after > 0).then_some(retry_after),
            blocked_until,
        })
    }

    pub async fn reset_login_backoff(&self, ip: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(login_backoff_key(ip)).await
    }

    pub async fn list_blocked_login_backoffs(&self) -> redis::RedisResult<Vec<LoginBackoffStatus>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys = Vec::<String>::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{LOGIN_BACKOFF_PREFIX}*"))
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(keys.clone())
            .query_async(&mut conn)
            .await?;
        let now = crate::time_utils::now_ms();
        let mut items = Vec::new();
        for (key, raw) in keys.into_iter().zip(values) {
            let ip = key
                .strip_prefix(LOGIN_BACKOFF_PREFIX)
                .unwrap_or(&key)
                .to_string();
            let status = login_backoff_status_from_raw(&ip, raw.as_deref(), now);
            if status.blocked {
                items.push(status);
            }
        }
        items.sort_by(|left, right| {
            right
                .retry_after
                .unwrap_or_default()
                .cmp(&left.retry_after.unwrap_or_default())
        });
        Ok(items)
    }

    pub async fn add_session(
        &self,
        session_id: &str,
        session: &LoginSession,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        conn.set_ex(
            key,
            serde_json::to_string(session).unwrap_or_default(),
            ttl_seconds as u64,
        )
        .await
    }

    pub async fn get_session(&self, session_id: &str) -> redis::RedisResult<Option<LoginSession>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn delete_session(&self, session_id: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        conn.del(key).await
    }

    pub async fn list_login_sessions(&self) -> redis::RedisResult<Vec<(String, LoginSession)>> {
        let values = self.list_session_values().await?;
        Ok(values
            .into_iter()
            .filter_map(|(id, value)| {
                serde_json::from_value::<LoginSession>(value)
                    .ok()
                    .map(|data| (id, data))
            })
            .collect())
    }

    pub async fn list_session_values(&self) -> redis::RedisResult<Vec<(String, Value)>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut keys: Vec<String> = Vec::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg("fn_knock:session:*")
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            keys.extend(batch);
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(keys.clone())
            .query_async(&mut conn)
            .await?;
        let mut sessions = Vec::new();
        for (key, raw) in keys.into_iter().zip(values) {
            let Some(raw) = raw else {
                continue;
            };
            if let Ok(data) = serde_json::from_str::<Value>(&raw) {
                let id = key
                    .strip_prefix("fn_knock:session:")
                    .unwrap_or(&key)
                    .to_string();
                sessions.push((id, data));
            }
        }
        sessions.sort_by(|(_a_id, a), (_b_id, b)| {
            let at = a.get("loginTime").and_then(Value::as_str).unwrap_or("");
            let bt = b.get("loginTime").and_then(Value::as_str).unwrap_or("");
            bt.cmp(at)
        });
        Ok(sessions)
    }

    pub async fn get_session_value(&self, session_id: &str) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(key).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn update_session_value(
        &self,
        session_id: &str,
        updates: Map<String, Value>,
    ) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let key = format!("fn_knock:session:{session_id}");
        let raw: Option<String> = conn.get(&key).await?;
        let ttl: i64 = conn.ttl(&key).await?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let mut current: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
        let Some(object) = current.as_object_mut() else {
            return Ok(None);
        };
        for (key, value) in updates {
            object.insert(key, value);
        }
        let serialized = serde_json::to_string(&current).unwrap_or_default();
        if ttl > 0 {
            let _: () = conn.set_ex(&key, serialized, ttl as u64).await?;
        } else {
            let _: () = conn.set(&key, serialized).await?;
        }
        Ok(Some(current))
    }

    pub async fn initialize_auth_mobility_login_session(
        &self,
        session_id: &str,
        subject_hash: &str,
        binding: &Value,
        login_event: &Value,
        summary: &Value,
        whitelist_record_id: &str,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let ttl_seconds = ttl_seconds.max(1) as u64;
        let binding_key = auth_mobility_binding_key("proxy-session", subject_hash);
        let session_index_key = auth_mobility_session_index_key(session_id);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            &binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.set_ex(
            auth_mobility_timeline_key(session_id),
            serde_json::to_string(&vec![login_event.clone()]).unwrap_or_else(|_| "[]".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.set_ex(
            auth_mobility_summary_key(session_id),
            serde_json::to_string(summary).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds,
        )
        .ignore();
        pipe.sadd(&session_index_key, &binding_key).ignore();
        pipe.expire(&session_index_key, ttl_seconds as i64).ignore();
        pipe.set_ex(
            auth_mobility_whitelist_owner_key(whitelist_record_id),
            session_id,
            ttl_seconds,
        )
        .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn get_auth_mobility_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
    ) -> redis::RedisResult<Option<Value>> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        self.get_json_value(&auth_mobility_binding_key(subject_type, &subject_hash))
            .await
    }

    pub async fn save_auth_mobility_binding_with_ttl(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        conn.set_ex(
            binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn save_auth_mobility_owned_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        owner_session_id: &str,
        binding_ttl_seconds: i64,
        session_index_ttl_seconds: Option<i64>,
    ) -> redis::RedisResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            &binding_key,
            serde_json::to_string(binding).unwrap_or_else(|_| "{}".to_string()),
            binding_ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.sadd(
            auth_mobility_session_index_key(owner_session_id),
            &binding_key,
        )
        .ignore();
        if let Some(ttl) = session_index_ttl_seconds.filter(|ttl| *ttl > 0) {
            pipe.expire(auth_mobility_session_index_key(owner_session_id), ttl)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn save_auth_mobility_orphaned_binding(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
        previous_owner_session_id: &str,
    ) -> redis::RedisResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let ttl: i64 = {
            let mut conn = self.conn();
            conn.ttl(&binding_key).await?
        };
        self.set_json_value_preserve_ttl(&binding_key, binding, ttl)
            .await?;
        let mut conn = self.conn();
        conn.srem(
            auth_mobility_session_index_key(previous_owner_session_id),
            binding_key,
        )
        .await
    }

    pub async fn save_auth_mobility_binding_keep_ttl(
        &self,
        subject_type: &str,
        subject_key: &str,
        binding: &Value,
    ) -> redis::RedisResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let ttl: i64 = {
            let mut conn = self.conn();
            conn.ttl(&binding_key).await?
        };
        self.set_json_value_preserve_ttl(&binding_key, binding, ttl)
            .await
    }

    pub async fn add_auth_mobility_session_binding(
        &self,
        owner_session_id: &str,
        subject_type: &str,
        subject_key: &str,
        session_index_ttl_seconds: Option<i64>,
    ) -> redis::RedisResult<()> {
        let subject_hash = auth_mobility_subject_hash(subject_type, subject_key);
        let binding_key = auth_mobility_binding_key(subject_type, &subject_hash);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.sadd(
            auth_mobility_session_index_key(owner_session_id),
            binding_key,
        )
        .ignore();
        if let Some(ttl) = session_index_ttl_seconds.filter(|ttl| *ttl > 0) {
            pipe.expire(auth_mobility_session_index_key(owner_session_id), ttl)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn list_auth_mobility_session_binding_keys(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(auth_mobility_session_index_key(session_id))
            .await
    }

    pub async fn remove_auth_mobility_session_bindings(
        &self,
        session_id: &str,
        binding_keys: &[String],
    ) -> redis::RedisResult<()> {
        if binding_keys.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(auth_mobility_session_index_key(session_id), binding_keys)
            .await
    }

    pub async fn append_auth_mobility_timeline_event(
        &self,
        session_id: &str,
        event: &Value,
        seed_login_event: Option<&Value>,
        fallback_ttl_seconds: Option<i64>,
    ) -> redis::RedisResult<()> {
        let timeline_key = auth_mobility_timeline_key(session_id);
        let summary_key = auth_mobility_summary_key(session_id);
        let (current_events, timeline_ttl) = self.get_json_value_with_ttl(&timeline_key).await?;
        let (stored_summary, summary_ttl) = self.get_json_value_with_ttl(&summary_key).await?;
        let events = current_events
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let mut next_events = if events.is_empty() {
            seed_login_event
                .cloned()
                .into_iter()
                .chain(std::iter::once(event.clone()))
                .collect::<Vec<_>>()
        } else {
            events
                .iter()
                .cloned()
                .chain(std::iter::once(event.clone()))
                .collect::<Vec<_>>()
        };
        limit_mobility_timeline_events(&mut next_events, 100);
        let next_summary =
            next_mobility_summary_from_event(&events, stored_summary, event, seed_login_event);
        let ttl = [
            timeline_ttl,
            summary_ttl,
            fallback_ttl_seconds.unwrap_or_default(),
        ]
        .into_iter()
        .filter(|value| *value > 0)
        .max()
        .unwrap_or_default();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        let serialized_events =
            serde_json::to_string(&next_events).unwrap_or_else(|_| "[]".to_string());
        let serialized_summary =
            serde_json::to_string(&next_summary).unwrap_or_else(|_| "{}".to_string());
        if ttl > 0 {
            pipe.set_ex(&timeline_key, serialized_events, ttl as u64)
                .ignore();
            pipe.set_ex(&summary_key, serialized_summary, ttl as u64)
                .ignore();
        } else {
            pipe.set(&timeline_key, serialized_events).ignore();
            pipe.set(&summary_key, serialized_summary).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn get_auth_mobility_active_ip_detail(
        &self,
        session_id: &str,
        ip: &str,
    ) -> redis::RedisResult<Option<Value>> {
        self.hget_json_value(&auth_mobility_active_ip_details_key(session_id), ip)
            .await
    }

    pub async fn list_auth_mobility_active_ip_details(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Vec<Value>> {
        let mut conn = self.conn();
        let raws: Vec<String> = conn
            .hvals(auth_mobility_active_ip_details_key(session_id))
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .collect())
    }

    pub async fn clear_auth_mobility_active_ip_session(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<()> {
        let keys = vec![
            auth_mobility_active_ip_zset_key(session_id),
            auth_mobility_active_ip_details_key(session_id),
        ];
        self.delete_keys(&keys).await
    }

    pub async fn save_auth_mobility_active_ip_detail(
        &self,
        session_id: &str,
        ip: &str,
        score: i64,
        detail: &Value,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let zset_key = auth_mobility_active_ip_zset_key(session_id);
        let detail_key = auth_mobility_active_ip_details_key(session_id);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(&zset_key, ip, score).ignore();
        pipe.hset(
            &detail_key,
            ip,
            serde_json::to_string(detail).unwrap_or_else(|_| "{}".to_string()),
        )
        .ignore();
        pipe.expire(&zset_key, ttl_seconds).ignore();
        pipe.expire(&detail_key, ttl_seconds).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn list_auth_mobility_recent_active_ip_details(
        &self,
        session_id: &str,
        since: i64,
    ) -> redis::RedisResult<Vec<Value>> {
        let mut conn = self.conn();
        let ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(auth_mobility_active_ip_zset_key(session_id))
            .arg(since)
            .arg("+inf")
            .query_async(&mut conn)
            .await?;
        if ips.is_empty() {
            return Ok(Vec::new());
        }
        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(auth_mobility_active_ip_details_key(session_id))
            .arg(ips)
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect())
    }

    pub async fn collect_auth_mobility_prune_targets(
        &self,
        session_id: &str,
        cutoff: i64,
        keep_ip: Option<&str>,
        max_entries: usize,
    ) -> redis::RedisResult<Vec<String>> {
        let zset_key = auth_mobility_active_ip_zset_key(session_id);
        let mut conn = self.conn();
        let expired_ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(&zset_key)
            .arg(0)
            .arg(cutoff)
            .query_async(&mut conn)
            .await?;
        let all_ips: Vec<String> = conn.zrange(&zset_key, 0, -1).await?;
        let mut remove_ips = expired_ips.into_iter().collect::<BTreeSet<_>>();
        let remaining_ips = all_ips
            .into_iter()
            .filter(|ip| !remove_ips.contains(ip))
            .collect::<Vec<_>>();
        let overflow_count = remaining_ips.len().saturating_sub(max_entries);
        if overflow_count > 0 {
            let keep_ip = keep_ip.unwrap_or("");
            for ip in remaining_ips
                .into_iter()
                .filter(|ip| ip != keep_ip)
                .take(overflow_count)
            {
                remove_ips.insert(ip);
            }
        }
        Ok(remove_ips.into_iter().collect())
    }

    pub async fn remove_auth_mobility_active_ips(
        &self,
        session_id: &str,
        ips: &[String],
    ) -> redis::RedisResult<Vec<Value>> {
        if ips.is_empty() {
            return Ok(Vec::new());
        }
        let detail_key = auth_mobility_active_ip_details_key(session_id);
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(&detail_key)
            .arg(ips)
            .query_async(&mut conn)
            .await?;
        let details = raws
            .into_iter()
            .filter_map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect::<Vec<_>>();
        let mut pipe = redis::pipe();
        pipe.zrem(auth_mobility_active_ip_zset_key(session_id), ips)
            .ignore();
        pipe.hdel(detail_key, ips).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(details)
    }

    pub async fn expire_auth_mobility_active_ip_keys(
        &self,
        session_id: &str,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.expire(auth_mobility_active_ip_zset_key(session_id), ttl_seconds)
            .ignore();
        pipe.expire(auth_mobility_active_ip_details_key(session_id), ttl_seconds)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn set_auth_mobility_whitelist_owner(
        &self,
        whitelist_record_id: &str,
        session_id: &str,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            auth_mobility_whitelist_owner_key(whitelist_record_id),
            session_id,
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn destroy_auth_mobility_session(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let session_index_key = auth_mobility_session_index_key(session_id);
        let active_details_key = auth_mobility_active_ip_details_key(session_id);
        let active_zset_key = auth_mobility_active_ip_zset_key(session_id);
        let timeline_key = auth_mobility_timeline_key(session_id);
        let summary_key = auth_mobility_summary_key(session_id);
        let proxy_hash = auth_mobility_subject_hash("proxy-session", session_id);
        let proxy_binding_key = auth_mobility_binding_key("proxy-session", &proxy_hash);

        let mut conn = self.conn();
        let mut binding_keys: Vec<String> = conn.smembers(&session_index_key).await?;
        if !binding_keys.iter().any(|key| key == &proxy_binding_key) {
            binding_keys.push(proxy_binding_key.clone());
        }
        let binding_raws: Vec<Option<String>> = if binding_keys.is_empty() {
            Vec::new()
        } else {
            redis::cmd("MGET")
                .arg(binding_keys.clone())
                .query_async(&mut conn)
                .await?
        };
        let active_details: HashMap<String, String> = conn.hgetall(&active_details_key).await?;
        let mut whitelist_ids = BTreeSet::new();
        for raw in binding_raws.into_iter().flatten() {
            if let Ok(value) = serde_json::from_str::<Value>(&raw)
                && let Some(id) = value.get("whitelistRecordId").and_then(Value::as_str)
                && !id.trim().is_empty()
            {
                whitelist_ids.insert(id.to_string());
            }
        }
        for raw in active_details.into_values() {
            if let Ok(value) = serde_json::from_str::<Value>(&raw)
                && let Some(id) = value.get("whitelistRecordId").and_then(Value::as_str)
                && !id.trim().is_empty()
            {
                whitelist_ids.insert(id.to_string());
            }
        }

        let mut delete_keys = vec![
            active_details_key,
            active_zset_key,
            timeline_key,
            summary_key,
            session_index_key,
        ];
        delete_keys.extend(binding_keys);
        delete_keys.extend(
            whitelist_ids
                .iter()
                .map(|id| auth_mobility_whitelist_owner_key(id)),
        );
        let mut pipe = redis::pipe();
        pipe.del(delete_keys).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(whitelist_ids.into_iter().collect())
    }

    pub async fn get_passkeys(&self) -> redis::RedisResult<Vec<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get("fn_knock:passkeys").await?;
        let Some(raw) = raw else {
            return Ok(Vec::new());
        };
        Ok(serde_json::from_str::<Vec<Value>>(&raw).unwrap_or_default())
    }

    pub async fn delete_passkey(&self, id: &str) -> redis::RedisResult<bool> {
        let mut passkeys = self.get_passkeys().await?;
        let original_len = passkeys.len();
        passkeys.retain(|passkey| passkey.get("id").and_then(Value::as_str) != Some(id));
        if passkeys.len() == original_len {
            return Ok(false);
        }
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await?;
        Ok(true)
    }

    pub async fn add_passkey(&self, passkey: &Value) -> redis::RedisResult<()> {
        let mut passkeys = self.get_passkeys().await?;
        passkeys.push(passkey.clone());
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await
    }

    pub async fn update_passkey_counter(
        &self,
        id: &str,
        counter: u32,
        last_used_at: &str,
        backup_eligible: Option<bool>,
        backup_state: Option<bool>,
    ) -> redis::RedisResult<bool> {
        let mut passkeys = self.get_passkeys().await?;
        let mut found = false;
        for passkey in &mut passkeys {
            if passkey.get("id").and_then(Value::as_str) != Some(id) {
                continue;
            }
            if let Some(object) = passkey.as_object_mut() {
                object.insert("counter".to_string(), json!(counter));
                object.insert("lastUsedAt".to_string(), json!(last_used_at));
                if let Some(value) = backup_eligible {
                    object.insert("backupEligible".to_string(), json!(value));
                    object.insert("backup_eligible".to_string(), json!(value));
                }
                if let Some(value) = backup_state {
                    object.insert("backupState".to_string(), json!(value));
                    object.insert("backup_state".to_string(), json!(value));
                }
                found = true;
            }
        }
        if !found {
            return Ok(false);
        }
        self.set_json_value("fn_knock:passkeys", &Value::Array(passkeys))
            .await?;
        Ok(true)
    }

    pub async fn set_passkey_challenge(
        &self,
        challenge: &str,
        challenge_type: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            format!("fn_knock:passkey:challenge:{challenge}"),
            challenge_type,
            ttl_seconds.max(1) as u64,
        )
        .await
    }

    pub async fn consume_passkey_challenge(
        &self,
        challenge: &str,
        challenge_type: &str,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: i64 = redis::cmd("EVAL")
            .arg(
                r#"
local value = redis.call("GET", KEYS[1])
if value == ARGV[1] then
  redis.call("DEL", KEYS[1])
  return 1
end
return 0
"#,
            )
            .arg(1)
            .arg(format!("fn_knock:passkey:challenge:{challenge}"))
            .arg(challenge_type)
            .query_async(&mut conn)
            .await?;
        Ok(result == 1)
    }

    pub async fn create_passkey_bind_token(
        &self,
        totp_id: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<String> {
        let token = hex::encode(rand::random::<[u8; 24]>());
        let mut conn = self.conn();
        let _: () = conn
            .set_ex(
                format!("fn_knock:passkey:bind:{token}"),
                totp_id,
                ttl_seconds.max(1) as u64,
            )
            .await?;
        Ok(token)
    }

    pub async fn is_passkey_bind_token_valid(&self, token: &str) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let value: Option<String> = conn.get(format!("fn_knock:passkey:bind:{token}")).await?;
        Ok(value.is_some())
    }

    pub async fn consume_passkey_bind_token(
        &self,
        token: &str,
    ) -> redis::RedisResult<Option<String>> {
        let mut conn = self.conn();
        redis::cmd("EVAL")
            .arg(
                r#"
local value = redis.call("GET", KEYS[1])
if not value then
  return nil
end
redis.call("DEL", KEYS[1])
return value
"#,
            )
            .arg(1)
            .arg(format!("fn_knock:passkey:bind:{token}"))
            .query_async(&mut conn)
            .await
    }

    pub async fn set_passkey_state(
        &self,
        challenge: &str,
        state: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        self.set_json_value_ex(
            &format!("fn_knock:passkey:state:{challenge}"),
            state,
            ttl_seconds,
        )
        .await
    }

    pub async fn consume_passkey_state(
        &self,
        challenge: &str,
    ) -> redis::RedisResult<Option<Value>> {
        self.consume_json_value(&format!("fn_knock:passkey:state:{challenge}"))
            .await
    }

    pub async fn scanner_settings_raw(&self) -> redis::RedisResult<Option<Value>> {
        self.get_json_value(SCANNER_SETTINGS_KEY).await
    }

    pub async fn save_scanner_settings(&self, value: &Value) -> redis::RedisResult<()> {
        self.set_json_value(SCANNER_SETTINGS_KEY, value).await
    }

    pub async fn list_scanner_blacklist(
        &self,
        page: i64,
        limit: i64,
        search: &str,
    ) -> redis::RedisResult<Value> {
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 200);
        let start = (safe_page - 1) * safe_limit;
        let end = start + safe_limit - 1;
        let search = search.trim();
        let total;
        let mut ips = Vec::<String>::new();

        if search.is_empty() {
            let mut conn = self.conn();
            total = conn.zcard(SCANNER_BLACKLIST_INDEX_KEY).await?;
            if total > 0 {
                ips = conn
                    .zrevrange(SCANNER_BLACKLIST_INDEX_KEY, start as isize, end as isize)
                    .await?;
            }
        } else {
            let chunk_size = 200_i64.max(safe_limit * 5);
            let mut matched_count = 0_i64;
            let mut offset = 0_i64;

            loop {
                let mut conn = self.conn();
                let chunk: Vec<String> = conn
                    .zrevrange(
                        SCANNER_BLACKLIST_INDEX_KEY,
                        offset as isize,
                        (offset + chunk_size - 1) as isize,
                    )
                    .await?;
                if chunk.is_empty() {
                    break;
                }
                offset += chunk.len() as i64;

                for ip in chunk {
                    if !ip.contains(search) {
                        continue;
                    }
                    if matched_count >= start && ips.len() < safe_limit as usize {
                        ips.push(ip);
                    }
                    matched_count += 1;
                }
            }

            total = matched_count;
        }

        let items = self.scanner_blacklist_records_by_ips(&ips).await?;
        Ok(json!({ "total": total, "items": items }))
    }

    pub async fn get_scanner_blacklist_record(
        &self,
        ip: &str,
    ) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(scanner_blacklist_data_key(ip)).await?;
        Ok(raw.and_then(|value| scanner_blacklist_record_from_raw(ip, &value)))
    }

    pub async fn scanner_blacklist_exists(&self, ip: &str) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let exists: i64 = conn.exists(scanner_blacklist_data_key(ip)).await?;
        Ok(exists == 1)
    }

    pub async fn record_scanner_suspicious_hit(
        &self,
        ip: &str,
        hit: &Value,
        now_ms: i64,
        min_score_ms: i64,
        window_min_score_ms: i64,
        ttl_seconds: i64,
    ) -> redis::RedisResult<i64> {
        let key = scanner_suspicious_key(ip);
        let serialized = serde_json::to_string(hit).unwrap_or_else(|_| "{}".to_string());
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.zadd(&key, serialized, now_ms).ignore();
        pipe.zrembyscore(&key, 0, min_score_ms).ignore();
        pipe.expire(&key, ttl_seconds.max(1)).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        redis::cmd("ZCOUNT")
            .arg(&key)
            .arg(window_min_score_ms)
            .arg("+inf")
            .query_async(&mut conn)
            .await
    }

    pub async fn scanner_suspicious_hits_since(
        &self,
        ip: &str,
        min_score_ms: i64,
    ) -> redis::RedisResult<Vec<Value>> {
        let key = scanner_suspicious_key(ip);
        let mut conn = self.conn();
        let raws: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(&key)
            .arg(min_score_ms)
            .arg("+inf")
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .filter_map(|raw| serde_json::from_str::<Value>(&raw).ok())
            .collect())
    }

    pub async fn add_scanner_blacklist_record(
        &self,
        ip: &str,
        record: &Value,
        blocked_at_ms: i64,
        ttl_seconds: i64,
    ) -> redis::RedisResult<()> {
        let ttl_seconds = ttl_seconds.max(1);
        let index_min_score = blocked_at_ms - ttl_seconds * 1000;
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            scanner_blacklist_data_key(ip),
            serde_json::to_string(record).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zadd(SCANNER_BLACKLIST_INDEX_KEY, ip, blocked_at_ms)
            .ignore();
        pipe.zrembyscore(SCANNER_BLACKLIST_INDEX_KEY, 0, index_min_score)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;

        let current_ttl: i64 = conn.ttl(SCANNER_BLACKLIST_INDEX_KEY).await?;
        if current_ttl == -2 || current_ttl == -1 || current_ttl < ttl_seconds {
            let _: () = conn
                .expire(SCANNER_BLACKLIST_INDEX_KEY, ttl_seconds)
                .await?;
        }
        Ok(())
    }

    pub async fn remove_scanner_blacklist(&self, ips: &[String]) -> redis::RedisResult<()> {
        let clean_ips = sanitize_scanner_ips(ips);
        if clean_ips.is_empty() {
            return Ok(());
        }

        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        for ip in &clean_ips {
            pipe.del(scanner_blacklist_data_key(ip)).ignore();
            pipe.del(scanner_suspicious_key(ip)).ignore();
        }
        pipe.zrem(SCANNER_BLACKLIST_INDEX_KEY, clean_ips).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    async fn scanner_blacklist_records_by_ips(
        &self,
        ips: &[String],
    ) -> redis::RedisResult<Vec<Value>> {
        if ips.is_empty() {
            return Ok(Vec::new());
        }

        let keys = ips
            .iter()
            .map(|ip| scanner_blacklist_data_key(ip))
            .collect::<Vec<_>>();
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET").arg(keys).query_async(&mut conn).await?;
        let mut records = Vec::new();
        let mut stale_ips = Vec::new();

        for (ip, raw) in ips.iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ips.push(ip.clone());
                continue;
            };
            match scanner_blacklist_record_from_raw(ip, &raw) {
                Some(record) => records.push(record),
                None => stale_ips.push(ip.clone()),
            }
        }

        if !stale_ips.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(SCANNER_BLACKLIST_INDEX_KEY, stale_ips).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }

        Ok(records)
    }

    pub async fn get_ip_location_cache(&self, ip: &str) -> redis::RedisResult<Option<Value>> {
        self.get_json_value(&ip_location_cache_key(ip)).await
    }

    pub async fn get_ip_location_state(&self, ip: &str) -> redis::RedisResult<Option<Value>> {
        self.get_json_value(&ip_location_state_key(ip)).await
    }

    pub async fn set_ip_location_state(
        &self,
        ip: &str,
        state: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        self.set_json_value_ex(&ip_location_state_key(ip), state, ttl_seconds)
            .await
    }

    pub async fn enqueue_ip_location(
        &self,
        ip: &str,
        state: &Value,
        next_attempt_at_ms: i64,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            ip_location_state_key(ip),
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.zadd(IP_LOCATION_QUEUE_KEY, ip, next_attempt_at_ms)
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn due_ip_location_ips(
        &self,
        now_ms: i64,
        limit: usize,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        redis::cmd("ZRANGEBYSCORE")
            .arg(IP_LOCATION_QUEUE_KEY)
            .arg(0)
            .arg(now_ms)
            .arg("LIMIT")
            .arg(0)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await
    }

    pub async fn acquire_ip_location_lock(
        &self,
        ip: &str,
        now_ms: i64,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(ip_location_lock_key(ip))
            .arg(now_ms)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_ip_location_lock(&self, ip: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(ip_location_lock_key(ip)).await
    }

    pub async fn remove_ip_location_queue_entry(&self, ip: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.zrem(IP_LOCATION_QUEUE_KEY, ip).await
    }

    pub async fn complete_ip_location_lookup(
        &self,
        ip: &str,
        result: &Value,
        state: &Value,
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.set_ex(
            ip_location_cache_key(ip),
            serde_json::to_string(result).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.set_ex(
            ip_location_state_key(ip),
            serde_json::to_string(state).unwrap_or_else(|_| "{}".to_string()),
            ttl_seconds.max(1) as u64,
        )
        .ignore();
        pipe.zrem(IP_LOCATION_QUEUE_KEY, ip).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn ip_location_references(&self, ip: &str) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.smembers(ip_location_refs_key(ip)).await
    }

    pub async fn add_ip_location_references(
        &self,
        ip: &str,
        refs: &[String],
        ttl_seconds: usize,
    ) -> redis::RedisResult<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let key = ip_location_refs_key(ip);
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.sadd(&key, refs).ignore();
        pipe.ttl(&key);
        let values: Vec<i64> = pipe.query_async(&mut conn).await?;
        let ttl = values.into_iter().next().unwrap_or_default();
        if ttl == -1 || ttl > ttl_seconds as i64 {
            let _: () = conn.expire(key, ttl_seconds.max(1) as i64).await?;
        }
        Ok(())
    }

    pub async fn remove_ip_location_references(
        &self,
        ip: &str,
        refs: &[String],
    ) -> redis::RedisResult<()> {
        if refs.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.srem(ip_location_refs_key(ip), refs).await
    }

    pub async fn record_recent_auth_ip(&self, ip: &str, now: i64) -> redis::RedisResult<()> {
        let expire_at = now + RECENT_AUTH_IPS_TTL_SECONDS;
        let mut conn = self.conn();
        let expired_ips: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(RECENT_AUTH_IPS_ZSET_KEY)
            .arg(0)
            .arg(now)
            .query_async(&mut conn)
            .await?;
        let raw_detail: Option<String> = conn.hget(RECENT_AUTH_IPS_DETAILS_KEY, ip).await?;
        let detail = raw_detail
            .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
            .unwrap_or_else(|| json!({}));
        let first_seen_at = detail
            .get("firstSeenAt")
            .and_then(Value::as_i64)
            .unwrap_or(now);
        let seen_count = detail
            .get("seenCount")
            .and_then(Value::as_i64)
            .unwrap_or_default()
            .max(0)
            + 1;
        let next_detail = json!({
            "firstSeenAt": first_seen_at,
            "lastSeenAt": now,
            "seenCount": seen_count.max(1),
        });
        let mut pipe = redis::pipe();
        pipe.zadd(RECENT_AUTH_IPS_ZSET_KEY, ip, expire_at).ignore();
        pipe.zrembyscore(RECENT_AUTH_IPS_ZSET_KEY, 0, now).ignore();
        pipe.hset(
            RECENT_AUTH_IPS_DETAILS_KEY,
            ip,
            serde_json::to_string(&next_detail).unwrap_or_else(|_| "{}".to_string()),
        )
        .ignore();
        let expired_ips = expired_ips
            .into_iter()
            .filter(|expired_ip| expired_ip != ip)
            .collect::<Vec<_>>();
        if !expired_ips.is_empty() {
            pipe.hdel(RECENT_AUTH_IPS_DETAILS_KEY, expired_ips).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn is_recent_auth_ip_active(&self, ip: &str, now: i64) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let score: Option<i64> = conn.zscore(RECENT_AUTH_IPS_ZSET_KEY, ip).await.ok();
        Ok(score.is_some_and(|expires_at| expires_at > now))
    }

    pub async fn list_recent_auth_ips_with_scores(
        &self,
        now: i64,
        limit: usize,
    ) -> redis::RedisResult<Vec<Value>> {
        let mut conn = self.conn();
        let raw: Vec<String> = redis::cmd("ZREVRANGEBYSCORE")
            .arg(RECENT_AUTH_IPS_ZSET_KEY)
            .arg("+inf")
            .arg(now + 1)
            .arg("WITHSCORES")
            .arg("LIMIT")
            .arg(0)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await?;
        let mut entries = Vec::new();
        let mut seen = BTreeSet::new();
        for pair in raw.chunks(2) {
            let Some(ip) = pair.first().map(String::as_str) else {
                continue;
            };
            let expires_at = pair
                .get(1)
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or_default();
            if ip.trim().is_empty() || expires_at <= now || !seen.insert(ip.to_string()) {
                continue;
            }
            entries.push((ip.to_string(), expires_at));
        }
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let detail_values: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(RECENT_AUTH_IPS_DETAILS_KEY)
            .arg(entries.iter().map(|(ip, _)| ip).collect::<Vec<_>>())
            .query_async(&mut conn)
            .await?;
        Ok(entries
            .into_iter()
            .zip(detail_values)
            .map(|((ip, expires_at), raw_detail)| {
                let detail = raw_detail
                    .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
                    .unwrap_or_else(|| json!({}));
                let fallback_last_seen_at = (expires_at - RECENT_AUTH_IPS_TTL_SECONDS).max(0);
                let last_seen_at = detail
                    .get("lastSeenAt")
                    .and_then(Value::as_i64)
                    .unwrap_or(fallback_last_seen_at);
                json!({
                    "ip": ip,
                    "expiresAt": expires_at,
                    "lastSeenAt": last_seen_at,
                    "firstSeenAt": detail
                        .get("firstSeenAt")
                        .and_then(Value::as_i64)
                        .unwrap_or(last_seen_at),
                    "seenCount": detail
                        .get("seenCount")
                        .and_then(Value::as_i64)
                        .unwrap_or(1)
                        .max(1),
                })
            })
            .collect())
    }

    pub async fn get_json_value_with_ttl(
        &self,
        key: &str,
    ) -> redis::RedisResult<(Option<Value>, i64)> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(key).await?;
        let ttl: i64 = conn.ttl(key).await?;
        Ok((raw.and_then(|value| serde_json::from_str(&value).ok()), ttl))
    }

    pub async fn set_json_value_preserve_ttl(
        &self,
        key: &str,
        value: &Value,
        ttl: i64,
    ) -> redis::RedisResult<()> {
        let serialized = serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string());
        let mut conn = self.conn();
        if ttl > 0 {
            let _: () = conn.set_ex(key, serialized, ttl as u64).await?;
        } else {
            let _: () = conn.set(key, serialized).await?;
        }
        Ok(())
    }

    pub async fn hget_json_value(
        &self,
        key: &str,
        field: &str,
    ) -> redis::RedisResult<Option<Value>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(key, field).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn hset_json_value(
        &self,
        key: &str,
        field: &str,
        value: &Value,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.hset(
            key,
            field,
            serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string()),
        )
        .await
    }

    pub async fn list_traffic_points(
        &self,
        user_id: &str,
        direction: &str,
        from_sec: i64,
        to_sec: i64,
        host: Option<&str>,
    ) -> redis::RedisResult<Vec<TrafficDeltaPoint>> {
        let key = traffic_key(user_id, direction, host);
        let mut conn = self.conn();
        let members: Vec<String> = conn.zrangebyscore(key, from_sec, to_sec).await?;
        Ok(parse_traffic_points(&members))
    }

    pub async fn list_error5xx_points(
        &self,
        user_id: &str,
        from_sec: i64,
        to_sec: i64,
        host: Option<&str>,
    ) -> redis::RedisResult<Vec<TrafficDeltaPoint>> {
        let key = error5xx_key(user_id, host);
        let mut conn = self.conn();
        let members: Vec<String> = conn.zrangebyscore(key, from_sec, to_sec).await?;
        Ok(parse_traffic_points(&members))
    }

    pub async fn record_traffic_snapshot(
        &self,
        user_id: &str,
        records: &[TrafficSnapshotRecord],
        now_sec: i64,
        keep_seconds: i64,
    ) -> redis::RedisResult<(f64, f64, f64)> {
        if records.is_empty() {
            return Ok((0.0, 0.0, 0.0));
        }

        let keep_seconds = keep_seconds.clamp(60, 365 * 24 * 3600);
        let expire_before_sec = now_sec - keep_seconds;
        let mut last_keys = Vec::with_capacity(records.len() * 3);
        for record in records {
            last_keys.push(traffic_last_total_key(
                user_id,
                "in",
                record.host.as_deref(),
            ));
            last_keys.push(traffic_last_total_key(
                user_id,
                "out",
                record.host.as_deref(),
            ));
            last_keys.push(error5xx_last_total_key(user_id, record.host.as_deref()));
        }

        let mut conn = self.conn();
        let last_values: Vec<Option<String>> = redis::cmd("MGET")
            .arg(last_keys)
            .query_async(&mut conn)
            .await?;

        let mut pipe = redis::pipe();
        let mut global_delta_in = 0.0;
        let mut global_delta_out = 0.0;
        let mut global_delta_5xx = 0.0;

        for (index, record) in records.iter().enumerate() {
            let offset = index * 3;
            let last_in = last_values
                .get(offset)
                .and_then(|value| parse_finite(value));
            let last_out = last_values
                .get(offset + 1)
                .and_then(|value| parse_finite(value));
            let last_5xx = last_values
                .get(offset + 2)
                .and_then(|value| parse_finite(value));
            let delta_in = compute_counter_delta(record.total_in, last_in);
            let delta_out = compute_counter_delta(record.total_out, last_out);
            let delta_5xx = compute_counter_delta(record.error_5xx, last_5xx);

            if record.host.is_none() {
                global_delta_in = delta_in;
                global_delta_out = delta_out;
                global_delta_5xx = delta_5xx;
            }

            let key_in = traffic_key(user_id, "in", record.host.as_deref());
            let key_out = traffic_key(user_id, "out", record.host.as_deref());
            let key_5xx = error5xx_key(user_id, record.host.as_deref());

            pipe.set(
                traffic_last_total_key(user_id, "in", record.host.as_deref()),
                finite_number_string(record.total_in),
            )
            .ignore();
            pipe.set(
                traffic_last_total_key(user_id, "out", record.host.as_deref()),
                finite_number_string(record.total_out),
            )
            .ignore();
            pipe.set(
                error5xx_last_total_key(user_id, record.host.as_deref()),
                finite_number_string(record.error_5xx),
            )
            .ignore();

            pipe.zadd(&key_in, traffic_member(now_sec, delta_in), now_sec)
                .ignore();
            pipe.zadd(&key_out, traffic_member(now_sec, delta_out), now_sec)
                .ignore();
            pipe.zadd(&key_5xx, traffic_member(now_sec, delta_5xx), now_sec)
                .ignore();
            pipe.sadd(TRAFFIC_KEY_INDEX, &key_in).ignore();
            pipe.sadd(TRAFFIC_KEY_INDEX, &key_out).ignore();
            pipe.sadd(ERROR5XX_KEY_INDEX, &key_5xx).ignore();
            pipe.zrembyscore(&key_in, 0, expire_before_sec).ignore();
            pipe.zrembyscore(&key_out, 0, expire_before_sec).ignore();
            pipe.zrembyscore(&key_5xx, 0, expire_before_sec).ignore();
        }

        let _: () = pipe.query_async(&mut conn).await?;
        Ok((global_delta_in, global_delta_out, global_delta_5xx))
    }

    pub async fn cleanup_traffic_metrics(&self, keep_seconds: i64) -> redis::RedisResult<usize> {
        let keep_seconds = keep_seconds.clamp(60, 365 * 24 * 3600);
        let expire_before_sec = chrono_like_now_seconds() - keep_seconds;
        let mut conn = self.conn();
        let traffic_keys: Vec<String> = conn.smembers(TRAFFIC_KEY_INDEX).await?;
        let error_keys: Vec<String> = conn.smembers(ERROR5XX_KEY_INDEX).await?;
        let keys = traffic_keys
            .into_iter()
            .chain(error_keys.into_iter())
            .filter(|key| !key.trim().is_empty())
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if keys.is_empty() {
            return Ok(0);
        }
        let mut pipe = redis::pipe();
        for key in &keys {
            pipe.zrembyscore(key, 0, expire_before_sec).ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(keys.len())
    }

    pub async fn get_whitelist_record(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(WHITELIST_RECORDS, id).await?;
        Ok(raw.and_then(|value| deserialize_whitelist_record(&value)))
    }

    pub async fn list_whitelist_records(&self) -> redis::RedisResult<Vec<WhitelistRecord>> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrevrange(WHITELIST_RECORD_ORDER, 0, -1).await?;
        if ids.is_empty() {
            return self.rebuild_whitelist_indexes().await;
        }

        let raws: Vec<Option<String>> = redis::cmd("HMGET")
            .arg(WHITELIST_RECORDS)
            .arg(ids.clone())
            .query_async(&mut conn)
            .await?;
        let mut records = Vec::new();
        let mut stale_ids = Vec::new();
        let mut stale_ip_targets = BTreeSet::new();
        for (id, raw) in ids.into_iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id);
                continue;
            };
            let Some(record) = deserialize_whitelist_record(&raw) else {
                stale_ids.push(id);
                continue;
            };
            if record.is_active() {
                records.push(record);
            } else {
                for target in whitelist_stale_ip_index_targets(&record) {
                    stale_ip_targets.insert(target);
                }
                stale_ids.push(id);
            }
        }
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WHITELIST_RECORD_ORDER, stale_ids.clone())
                .ignore();
            pipe.zrem(WHITELIST_EXPIRY, stale_ids.clone()).ignore();
            pipe.srem(WHITELIST_CIDR_RECORDS, stale_ids.clone())
                .ignore();
            for ip in stale_ip_targets {
                pipe.srem(whitelist_ip_records_key(&ip), stale_ids.clone())
                    .ignore();
            }
            let _: () = pipe.query_async(&mut conn).await?;
        }
        records.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(records)
    }

    pub async fn insert_whitelist_record(
        &self,
        record: &WhitelistRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            &record.id,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .ignore();
        pipe.zadd(WHITELIST_RECORD_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &record.id, expire_at).ignore();
        }
        queue_whitelist_indexes(&mut pipe, record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn replace_whitelist_record(
        &self,
        previous: &WhitelistRecord,
        next: &WhitelistRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            &next.id,
            serde_json::to_string(next).unwrap_or_default(),
        )
        .ignore();
        if let Some(expire_at) = next.expire_at {
            pipe.zadd(WHITELIST_EXPIRY, &next.id, expire_at).ignore();
        } else {
            pipe.zrem(WHITELIST_EXPIRY, &next.id).ignore();
        }
        queue_remove_whitelist_indexes(&mut pipe, previous);
        queue_whitelist_indexes(&mut pipe, next);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn delete_whitelist_record(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRecord>> {
        let Some(record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hdel(WHITELIST_RECORDS, id).ignore();
        pipe.hdel(WHITELIST_DELETED, id).ignore();
        pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
        pipe.zrem(WHITELIST_EXPIRY, id).ignore();
        queue_remove_whitelist_indexes(&mut pipe, &record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn expire_whitelist_record(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRecord>> {
        let Some(record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "expired".to_string();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_RECORD_ORDER, id).ignore();
        pipe.zrem(WHITELIST_EXPIRY, id).ignore();
        queue_remove_whitelist_indexes(&mut pipe, &record);
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn update_whitelist_comment(
        &self,
        id: &str,
        comment: String,
    ) -> redis::RedisResult<Option<WhitelistRecord>> {
        let Some(mut record) = self.get_whitelist_record(id).await? else {
            return Ok(None);
        };
        record.comment = Some(comment);
        let mut conn = self.conn();
        let _: () = conn
            .hset(
                WHITELIST_RECORDS,
                id,
                serde_json::to_string(&record).unwrap_or_default(),
            )
            .await?;
        Ok(Some(record))
    }

    pub async fn find_whitelist_records_by_target(
        &self,
        target: &str,
        target_type: &str,
        source: Option<&str>,
    ) -> redis::RedisResult<Vec<WhitelistRecord>> {
        let records = self.list_whitelist_records().await?;
        let mut matched = records
            .into_iter()
            .filter(|record| {
                if let Some(source) = source {
                    if record.source != source {
                        return false;
                    }
                }
                match target_type {
                    "cidr" => record.target_type() == "cidr" && record.ip == target,
                    "cname" => record.target_type() == "cname" && record.ip == target,
                    _ => record.target_type() == "ip" && record.ip == target,
                }
            })
            .collect::<Vec<_>>();
        matched.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(matched)
    }

    pub async fn get_whitelist_region_group(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.hget(WHITELIST_REGION_GROUP_RECORDS, id).await?;
        Ok(raw.and_then(|value| deserialize_whitelist_region_group(&value)))
    }

    pub async fn list_whitelist_region_groups(
        &self,
    ) -> redis::RedisResult<Vec<WhitelistRegionGroupRecord>> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrevrange(WHITELIST_REGION_GROUP_ORDER, 0, -1).await?;
        let mut stale_ids = Vec::new();
        let mut records = if ids.is_empty() {
            let all: HashMap<String, String> = conn.hgetall(WHITELIST_REGION_GROUP_RECORDS).await?;
            all.into_values()
                .filter_map(|raw| deserialize_whitelist_region_group(&raw))
                .filter(WhitelistRegionGroupRecord::is_active)
                .collect::<Vec<_>>()
        } else {
            let raws: Vec<Option<String>> = redis::cmd("HMGET")
                .arg(WHITELIST_REGION_GROUP_RECORDS)
                .arg(ids.clone())
                .query_async(&mut conn)
                .await?;
            let mut records = Vec::new();
            for (id, raw) in ids.into_iter().zip(raws) {
                let Some(raw) = raw else {
                    stale_ids.push(id);
                    continue;
                };
                let Some(record) = deserialize_whitelist_region_group(&raw) else {
                    stale_ids.push(id);
                    continue;
                };
                if record.is_active() {
                    records.push(record);
                } else {
                    stale_ids.push(id);
                }
            }
            records
        };
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WHITELIST_REGION_GROUP_ORDER, stale_ids.clone())
                .ignore();
            pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, stale_ids).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        records.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(records)
    }

    pub async fn insert_whitelist_region_group(
        &self,
        record: &WhitelistRegionGroupRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            &record.id,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .ignore();
        pipe.zadd(WHITELIST_REGION_GROUP_ORDER, &record.id, record.created_at)
            .ignore();
        if let Some(expire_at) = record.expire_at {
            pipe.zadd(WHITELIST_REGION_GROUP_EXPIRY, &record.id, expire_at)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn delete_whitelist_region_group(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRegionGroupRecord>> {
        let Some(record) = self.get_whitelist_region_group(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "deleted".to_string();
        next.updated_at = chrono_like_now_seconds();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn expire_whitelist_region_group(
        &self,
        id: &str,
    ) -> redis::RedisResult<Option<WhitelistRegionGroupRecord>> {
        let Some(record) = self.get_whitelist_region_group(id).await? else {
            return Ok(None);
        };
        if !record.is_active() {
            return Ok(None);
        }
        let mut next = record.clone();
        next.status = "expired".to_string();
        next.updated_at = chrono_like_now_seconds();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();
        pipe.hset(
            WHITELIST_REGION_GROUP_RECORDS,
            id,
            serde_json::to_string(&next).unwrap_or_default(),
        )
        .ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_ORDER, id).ignore();
        pipe.zrem(WHITELIST_REGION_GROUP_EXPIRY, id).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(Some(record))
    }

    pub async fn cleanup_whitelist_concrete_targets(
        &self,
        targets: &[WhitelistConcreteTarget],
    ) -> redis::RedisResult<Vec<WhitelistConcreteTarget>> {
        let active_records = self.list_whitelist_records().await?;
        let active_region_targets = self.list_whitelist_region_group_concrete_targets().await?;
        let mut removed = Vec::new();
        let mut conn = self.conn();
        let mut pipe = redis::pipe();

        for target in unique_concrete_targets(targets) {
            let still_active = active_records.iter().any(|record| {
                record.concrete_targets().iter().any(|candidate| {
                    candidate.target == target.target && candidate.target_type == target.target_type
                })
            });
            if still_active {
                continue;
            }
            if target.target_type == "cidr"
                && active_region_targets.iter().any(|candidate| {
                    candidate.target.eq_ignore_ascii_case(&target.target)
                        && candidate.target_type == "cidr"
                })
            {
                continue;
            }

            if target.target_type == "cidr" {
                pipe.srem(WHITELIST_CIDR_RECORDS, &target.record_id)
                    .ignore();
            } else {
                pipe.srem(WHITELIST_IPS, &target.target).ignore();
                pipe.del(whitelist_ip_records_key(&target.target)).ignore();
            }
            removed.push(target);
        }

        if !removed.is_empty() {
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(removed)
    }

    pub async fn list_whitelist_active_concrete_targets(
        &self,
    ) -> redis::RedisResult<Vec<WhitelistConcreteTarget>> {
        let now = chrono_like_now_seconds();
        let mut targets = Vec::new();
        for record in self.list_whitelist_records().await? {
            if !record.is_active() {
                continue;
            }
            if record.expire_at.is_some_and(|expire_at| expire_at <= now) {
                continue;
            }
            targets.extend(record.concrete_targets());
        }
        targets.extend(self.list_whitelist_region_group_concrete_targets().await?);
        Ok(targets)
    }

    pub async fn save_reverse_proxy_trusted_ips_runtime(
        &self,
        runtime: &Value,
    ) -> redis::RedisResult<()> {
        self.set_json_value(REVERSE_PROXY_TRUSTED_IPS_RUNTIME, runtime)
            .await
    }

    pub async fn append_system_event(
        &self,
        event: &Value,
        retention_days: i64,
    ) -> redis::RedisResult<()> {
        let event_id = event.get("id").and_then(Value::as_str).unwrap_or("");
        if event_id.trim().is_empty() {
            return Ok(());
        }
        let now = crate::time_utils::now_ms();
        let retention_days = retention_days.clamp(1, MAX_EVENT_RETENTION_DAYS);
        let retention_ms = retention_days * 86_400 * 1000;
        let happened_at_ms = event
            .get("happened_at")
            .and_then(Value::as_str)
            .and_then(crate::time_utils::parse_iso_ms)
            .unwrap_or(now);
        let cutoff_timestamp = now - retention_ms;
        let expires_at_ms = happened_at_ms + retention_ms;
        let ttl_seconds = ((expires_at_ms - now).max(1000) + 999) / 1000;
        let serialized = serde_json::to_string(event).unwrap_or_default();

        let mut conn = self.conn();
        let stream_id: String = redis::cmd("XADD")
            .arg(EVENTS_STREAM_KEY)
            .arg("*")
            .arg("event")
            .arg(&serialized)
            .query_async(&mut conn)
            .await?;

        let mut pipe = redis::pipe();
        pipe.set_ex(
            system_event_data_key(event_id),
            &serialized,
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zadd(EVENTS_INDEX_KEY, event_id, happened_at_ms)
            .ignore();
        pipe.set_ex(
            system_event_stream_id_key(event_id),
            stream_id,
            ttl_seconds as u64,
        )
        .ignore();
        pipe.zrembyscore(EVENTS_INDEX_KEY, 0, cutoff_timestamp)
            .ignore();
        pipe.cmd("XTRIM")
            .arg(EVENTS_STREAM_KEY)
            .arg("MINID")
            .arg("~")
            .arg(format!("{cutoff_timestamp}-0"))
            .ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn latest_system_event_stream_id(&self) -> redis::RedisResult<Option<String>> {
        let mut conn = self.conn();
        let reply: StreamRangeReply = conn.xrevrange_count(EVENTS_STREAM_KEY, "+", "-", 1).await?;
        Ok(reply.ids.first().map(|entry| entry.id.clone()))
    }

    pub async fn read_system_event_stream_after(
        &self,
        last_id: &str,
        count: usize,
    ) -> redis::RedisResult<Vec<(String, Value)>> {
        let mut conn = self.conn();
        let options = StreamReadOptions::default().count(count.max(1));
        let reply: Option<StreamReadReply> = conn
            .xread_options(&[EVENTS_STREAM_KEY], &[last_id], &options)
            .await?;
        let mut events = Vec::new();
        let Some(reply) = reply else {
            return Ok(events);
        };
        for key in reply.keys {
            for stream_id in key.ids {
                let Some(raw_event) = stream_id.get::<String>("event") else {
                    continue;
                };
                if let Ok(event) = serde_json::from_str::<Value>(&raw_event) {
                    events.push((stream_id.id, event));
                }
            }
        }
        Ok(events)
    }

    pub async fn get_notification_last_stream_id(&self) -> redis::RedisResult<Option<String>> {
        self.get_string_value(NOTIFICATION_RUNTIME_LAST_STREAM_KEY)
            .await
    }

    pub async fn set_notification_last_stream_id(&self, id: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set(NOTIFICATION_RUNTIME_LAST_STREAM_KEY, id).await
    }

    pub async fn acquire_notification_runtime_lease(
        &self,
        name: &str,
        token: &str,
        ttl_seconds: usize,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(notification_runtime_lock_key(name))
            .arg(token)
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_notification_runtime_lease(
        &self,
        name: &str,
        token: &str,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let _: i64 = redis::cmd("EVAL")
            .arg(
                r#"
                if redis.call('GET', KEYS[1]) == ARGV[1] then
                    return redis.call('DEL', KEYS[1])
                end
                return 0
                "#,
            )
            .arg(1)
            .arg(notification_runtime_lock_key(name))
            .arg(token)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn append_notification_window_hit(
        &self,
        rule_id: &str,
        group_key: &str,
        event_id: &str,
        happened_at_ms: i64,
        window_seconds: i64,
    ) -> redis::RedisResult<i64> {
        let key = notification_window_key(rule_id, group_key);
        let window_ms = window_seconds.max(1) * 1000;
        let start_score = (happened_at_ms - window_ms).max(0);
        let mut conn = self.conn();
        let _: () = conn.zadd(&key, event_id, happened_at_ms).await?;
        let _: () = conn
            .zrembyscore(&key, 0, start_score.saturating_sub(1))
            .await?;
        let _: () = conn.expire(&key, (window_seconds * 2).max(60)).await?;
        conn.zcount(&key, start_score, happened_at_ms).await
    }

    pub async fn get_notification_cooldown_until(
        &self,
        rule_id: &str,
        group_key: &str,
    ) -> redis::RedisResult<Option<String>> {
        self.get_string_value(&notification_cooldown_key(rule_id, group_key))
            .await
    }

    pub async fn set_notification_cooldown_until(
        &self,
        rule_id: &str,
        group_key: &str,
        until: &str,
        cooldown_seconds: i64,
    ) -> redis::RedisResult<()> {
        if cooldown_seconds <= 0 {
            return Ok(());
        }
        let mut conn = self.conn();
        conn.set_ex(
            notification_cooldown_key(rule_id, group_key),
            until,
            cooldown_seconds as u64,
        )
        .await
    }

    pub async fn enqueue_notification_delivery(
        &self,
        id: &str,
        ready_at_ms: i64,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.zadd(NOTIFICATION_DELIVERIES_READY_KEY, id, ready_at_ms)
            .await
    }

    pub async fn pull_ready_notification_delivery_ids(
        &self,
        limit: usize,
        now_ms: i64,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        let ids: Vec<String> = redis::cmd("EVAL")
            .arg(
                r#"
                local ids = redis.call(
                    'ZRANGEBYSCORE',
                    KEYS[1],
                    '-inf',
                    ARGV[1],
                    'LIMIT',
                    0,
                    tonumber(ARGV[2])
                )
                if #ids == 0 then
                    return ids
                end
                redis.call('ZREM', KEYS[1], unpack(ids))
                return ids
                "#,
            )
            .arg(1)
            .arg(NOTIFICATION_DELIVERIES_READY_KEY)
            .arg(now_ms)
            .arg(limit.max(1))
            .query_async(&mut conn)
            .await?;
        Ok(ids.into_iter().filter(|id| !id.trim().is_empty()).collect())
    }

    pub async fn acquire_system_event_dedupe(
        &self,
        key: &str,
        ttl_seconds: i64,
    ) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let result: Option<String> = redis::cmd("SET")
            .arg(format!("{EVENTS_DEDUPE_PREFIX}{key}"))
            .arg("1")
            .arg("EX")
            .arg(ttl_seconds.max(1))
            .arg("NX")
            .query_async(&mut conn)
            .await?;
        Ok(result.as_deref() == Some("OK"))
    }

    pub async fn release_system_event_dedupe(&self, key: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{EVENTS_DEDUPE_PREFIX}{key}")).await
    }

    pub async fn list_system_events(
        &self,
        page: i64,
        limit: i64,
        search: &str,
        event_type: Option<&str>,
        level: Option<&str>,
        source: Option<&str>,
    ) -> redis::RedisResult<Value> {
        let safe_page = page.max(1);
        let safe_limit = limit.clamp(1, 100);
        let has_filter = !search.trim().is_empty()
            || event_type.is_some()
            || level.is_some()
            || source.is_some();

        if !has_filter {
            let start = (safe_page - 1) * safe_limit;
            loop {
                let mut conn = self.conn();
                let total: i64 = conn.zcard(EVENTS_INDEX_KEY).await?;
                if total == 0 {
                    return Ok(json!({ "events": [], "total": 0 }));
                }
                let ids: Vec<String> = conn
                    .zrevrange(
                        EVENTS_INDEX_KEY,
                        start as isize,
                        (start + safe_limit - 1) as isize,
                    )
                    .await?;
                if ids.is_empty() {
                    return Ok(json!({ "events": [], "total": total }));
                }
                let (events, stale_ids) = self.system_events_by_ids(&ids).await?;
                if !stale_ids.is_empty() {
                    self.remove_stale_system_event_ids(&stale_ids).await?;
                    continue;
                }
                return Ok(json!({ "events": events, "total": total }));
            }
        }

        let page_start = (safe_page - 1) * safe_limit;
        let mut matched_total = 0_i64;
        let mut offset = 0_isize;
        let mut events = Vec::new();
        let mut all_stale_ids = Vec::new();

        loop {
            let mut conn = self.conn();
            let ids: Vec<String> = conn
                .zrevrange(
                    EVENTS_INDEX_KEY,
                    offset,
                    offset + EVENT_LIST_SCAN_CHUNK_SIZE - 1,
                )
                .await?;
            if ids.is_empty() {
                break;
            }
            offset += ids.len() as isize;

            let (batch_events, stale_ids) = self.system_events_by_ids(&ids).await?;
            all_stale_ids.extend(stale_ids);
            for event in batch_events {
                if !system_event_matches_filters(&event, search, event_type, level, source) {
                    continue;
                }
                if matched_total >= page_start && events.len() < safe_limit as usize {
                    events.push(event);
                }
                matched_total += 1;
            }
        }

        if !all_stale_ids.is_empty() {
            self.remove_stale_system_event_ids(&all_stale_ids).await?;
        }
        Ok(json!({ "events": events, "total": matched_total }))
    }

    pub async fn list_system_events_by_range(
        &self,
        from_ms: i64,
        to_ms: i64,
        types: &[&str],
    ) -> redis::RedisResult<Vec<(Value, i64)>> {
        let mut conn = self.conn();
        let pairs: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(EVENTS_INDEX_KEY)
            .arg(from_ms.max(0))
            .arg(to_ms.max(from_ms))
            .arg("WITHSCORES")
            .query_async(&mut conn)
            .await?;
        if pairs.is_empty() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let mut scores = Vec::new();
        for pair in pairs.chunks(2) {
            let Some(id) = pair.first() else {
                continue;
            };
            let score = pair
                .get(1)
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|value| value.is_finite())
                .map(|value| value as i64)
                .unwrap_or_default();
            ids.push(id.clone());
            scores.push(score);
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }

        let allowed_types = types
            .iter()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .collect::<HashSet<_>>();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let mut events = Vec::new();
        let mut stale_ids = Vec::new();
        for ((id, score), raw) in ids.into_iter().zip(scores).zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id);
                continue;
            };
            match serde_json::from_str::<Value>(&raw) {
                Ok(event) => {
                    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
                    if allowed_types.is_empty() || allowed_types.contains(event_type) {
                        events.push((event, score));
                    }
                }
                Err(_) => stale_ids.push(id),
            }
        }
        if !stale_ids.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(EVENTS_INDEX_KEY, stale_ids).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(events)
    }

    pub async fn count_waf_logs_for_buckets(
        &self,
        bucket_starts: &[i64],
        to_ms: i64,
    ) -> redis::RedisResult<(i64, Vec<i64>)> {
        if bucket_starts.is_empty() {
            return Ok((0, Vec::new()));
        }
        let step = if bucket_starts.len() > 1 {
            (bucket_starts[1] - bucket_starts[0]).max(1)
        } else {
            1
        };
        let mut total = 0_i64;
        let mut counts = vec![0_i64; bucket_starts.len()];
        let mut conn = self.conn();

        for (bucket_index, bucket_start) in bucket_starts.iter().enumerate() {
            let bucket_end = if bucket_index == bucket_starts.len() - 1 {
                to_ms
            } else {
                to_ms.min(bucket_start + step)
            };
            let end_arg = if bucket_index == bucket_starts.len() - 1 {
                bucket_end.to_string()
            } else {
                format!("({bucket_end}")
            };
            for date in waf_log_dates_for_range(*bucket_start, bucket_end) {
                let count: i64 = redis::cmd("ZCOUNT")
                    .arg(waf_log_date_key(&date))
                    .arg(*bucket_start)
                    .arg(&end_arg)
                    .query_async(&mut conn)
                    .await?;
                counts[bucket_index] += count;
                total += count;
            }
        }

        Ok((total, counts))
    }

    pub async fn persist_waf_events(
        &self,
        events: &[Value],
        retention_days: i64,
    ) -> redis::RedisResult<()> {
        if events.is_empty() {
            return Ok(());
        }
        let ttl_seconds = retention_days.clamp(1, 365) * 86_400;
        let mut touched_dates = BTreeSet::new();
        let mut pipe = redis::pipe();
        let mut operations = 0_usize;

        for event in events {
            let Some(trace_id) = event.get("trace_id").and_then(Value::as_str) else {
                continue;
            };
            if trace_id.trim().is_empty() {
                continue;
            }
            let score = waf_log_event_score(event);
            let date = crate::time_utils::local_date_from_ms(score);
            let action = event.get("action").and_then(Value::as_str).unwrap_or("log");
            let serialized = serde_json::to_string(event).unwrap_or_default();

            touched_dates.insert(date.clone());
            pipe.set_ex(waf_log_event_key(trace_id), serialized, ttl_seconds as u64)
                .ignore();
            pipe.zadd(waf_log_date_key(&date), trace_id, score).ignore();
            pipe.expire(waf_log_date_key(&date), ttl_seconds).ignore();
            pipe.cmd("HINCRBY")
                .arg(waf_log_stats_key(&date))
                .arg("events")
                .arg(1)
                .ignore();
            pipe.cmd("HINCRBY")
                .arg(waf_log_stats_key(&date))
                .arg(format!("action:{action}"))
                .arg(1)
                .ignore();
            pipe.expire(waf_log_stats_key(&date), ttl_seconds).ignore();
            operations += 6;
        }

        for date in touched_dates {
            pipe.zadd(WAF_LOG_DATES_INDEX_KEY, &date, waf_log_date_score(&date))
                .ignore();
            operations += 1;
        }

        if operations > 0 {
            let _: () = pipe.query_async(&mut self.conn()).await?;
        }
        Ok(())
    }

    pub async fn list_waf_log_dates(&self, today: &str) -> redis::RedisResult<Vec<String>> {
        let migrated = self
            .get_string_value(WAF_LOG_DATES_INDEX_MIGRATED_KEY)
            .await?;
        if migrated.is_none() {
            return self.scan_waf_log_dates_and_backfill_index(today).await;
        }

        let mut conn = self.conn();
        let indexed_dates: Vec<String> = conn.zrevrange(WAF_LOG_DATES_INDEX_KEY, 0, -1).await?;
        if indexed_dates.is_empty() {
            return Ok(vec![today.to_string()]);
        }

        let mut dates = BTreeSet::new();
        dates.insert(today.to_string());
        let mut stale_dates = Vec::new();
        for date in indexed_dates
            .into_iter()
            .filter(|date| is_waf_log_date(date))
        {
            let count: i64 = conn.zcard(waf_log_date_key(&date)).await?;
            if count > 0 {
                dates.insert(date);
            } else {
                stale_dates.push(date);
            }
        }
        if !stale_dates.is_empty() {
            let mut pipe = redis::pipe();
            pipe.zrem(WAF_LOG_DATES_INDEX_KEY, stale_dates).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }

        Ok(descending_strings(dates))
    }

    async fn scan_waf_log_dates_and_backfill_index(
        &self,
        today: &str,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        let mut cursor = "0".to_string();
        let mut dates = BTreeSet::new();
        loop {
            let (next_cursor, batch): (String, Vec<String>) = redis::cmd("SCAN")
                .arg(&cursor)
                .arg("MATCH")
                .arg(format!("{WAF_LOG_DATE_PREFIX}*"))
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;
            for key in batch {
                let date = key.strip_prefix(WAF_LOG_DATE_PREFIX).unwrap_or("");
                if is_waf_log_date(date) {
                    dates.insert(date.to_string());
                }
            }
            if next_cursor == "0" {
                break;
            }
            cursor = next_cursor;
        }

        let mut pipe = redis::pipe();
        for date in &dates {
            pipe.zadd(WAF_LOG_DATES_INDEX_KEY, date, waf_log_date_score(date))
                .ignore();
        }
        pipe.set(WAF_LOG_DATES_INDEX_MIGRATED_KEY, "1").ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        dates.insert(today.to_string());
        Ok(descending_strings(dates))
    }

    pub async fn waf_log_date_total(&self, date: &str) -> redis::RedisResult<i64> {
        let mut conn = self.conn();
        conn.zcard(waf_log_date_key(date)).await
    }

    pub async fn waf_log_ids_desc(
        &self,
        date: &str,
        start: isize,
        end: isize,
    ) -> redis::RedisResult<Vec<String>> {
        let mut conn = self.conn();
        conn.zrevrange(waf_log_date_key(date), start, end).await
    }

    pub async fn waf_log_events_by_ids(
        &self,
        ids: &[String],
    ) -> redis::RedisResult<Vec<Option<Value>>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| waf_log_event_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        Ok(raws
            .into_iter()
            .map(|raw| raw.and_then(|value| serde_json::from_str::<Value>(&value).ok()))
            .collect())
    }

    pub async fn get_waf_log_event(&self, trace_id: &str) -> redis::RedisResult<Option<Value>> {
        self.get_json_value(&waf_log_event_key(trace_id)).await
    }

    pub async fn remove_waf_log_stale_ids(
        &self,
        date: &str,
        ids: &[String],
    ) -> redis::RedisResult<()> {
        let unique_ids = unique_non_empty_strings(ids);
        if unique_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        for chunk in unique_ids.chunks(500) {
            let mut pipe = redis::pipe();
            pipe.zrem(waf_log_date_key(date), chunk).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
        }
        Ok(())
    }

    pub async fn delete_waf_log_date(&self, date: &str) -> redis::RedisResult<bool> {
        let mut conn = self.conn();
        let mut deleted_count = 0usize;
        loop {
            let ids: Vec<String> = conn.zrange(waf_log_date_key(date), 0, 499).await?;
            if ids.is_empty() {
                break;
            }
            let mut pipe = redis::pipe();
            for id in &ids {
                pipe.del(waf_log_event_key(id)).ignore();
            }
            pipe.zrem(waf_log_date_key(date), ids.clone()).ignore();
            let _: () = pipe.query_async(&mut conn).await?;
            deleted_count += ids.len();
        }

        let mut pipe = redis::pipe();
        pipe.del(waf_log_date_key(date)).ignore();
        pipe.del(waf_log_stats_key(date)).ignore();
        pipe.zrem(WAF_LOG_DATES_INDEX_KEY, date).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(deleted_count > 0)
    }

    pub async fn delete_system_events(&self, ids: &[String]) -> redis::RedisResult<()> {
        let unique_ids = unique_non_empty_strings(ids);
        if unique_ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let stream_ids: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                unique_ids
                    .iter()
                    .map(|id| system_event_stream_id_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let valid_stream_ids = stream_ids.into_iter().flatten().collect::<Vec<_>>();
        let mut pipe = redis::pipe();
        pipe.del(
            unique_ids
                .iter()
                .map(|id| system_event_data_key(id))
                .collect::<Vec<_>>(),
        )
        .ignore();
        pipe.del(
            unique_ids
                .iter()
                .map(|id| system_event_stream_id_key(id))
                .collect::<Vec<_>>(),
        )
        .ignore();
        pipe.zrem(EVENTS_INDEX_KEY, unique_ids.clone()).ignore();
        if !valid_stream_ids.is_empty() {
            pipe.cmd("XDEL")
                .arg(EVENTS_STREAM_KEY)
                .arg(valid_stream_ids)
                .ignore();
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(())
    }

    pub async fn clear_system_events(&self) -> redis::RedisResult<usize> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrange(EVENTS_INDEX_KEY, 0, -1).await?;
        let mut pipe = redis::pipe();
        for batch in ids.chunks(EVENT_CLEAR_CHUNK_SIZE) {
            pipe.del(
                batch
                    .iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .ignore();
            pipe.del(
                batch
                    .iter()
                    .map(|id| system_event_stream_id_key(id))
                    .collect::<Vec<_>>(),
            )
            .ignore();
        }
        pipe.del(EVENTS_INDEX_KEY).ignore();
        pipe.del(EVENTS_STREAM_KEY).ignore();
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(ids.len())
    }

    async fn system_events_by_ids(
        &self,
        ids: &[String],
    ) -> redis::RedisResult<(Vec<Value>, Vec<String>)> {
        if ids.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut conn = self.conn();
        let raws: Vec<Option<String>> = redis::cmd("MGET")
            .arg(
                ids.iter()
                    .map(|id| system_event_data_key(id))
                    .collect::<Vec<_>>(),
            )
            .query_async(&mut conn)
            .await?;
        let mut events = Vec::new();
        let mut stale_ids = Vec::new();
        for (id, raw) in ids.iter().zip(raws) {
            let Some(raw) = raw else {
                stale_ids.push(id.clone());
                continue;
            };
            match serde_json::from_str::<Value>(&raw) {
                Ok(event) => events.push(event),
                Err(_) => stale_ids.push(id.clone()),
            }
        }
        Ok((events, stale_ids))
    }

    async fn remove_stale_system_event_ids(&self, ids: &[String]) -> redis::RedisResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn();
        let _: () = redis::cmd("ZREM")
            .arg(EVENTS_INDEX_KEY)
            .arg(ids)
            .query_async(&mut conn)
            .await?;
        Ok(())
    }

    async fn rebuild_whitelist_indexes(&self) -> redis::RedisResult<Vec<WhitelistRecord>> {
        let mut conn = self.conn();
        let all: HashMap<String, String> = conn.hgetall(WHITELIST_RECORDS).await?;
        let existing_ips: Vec<String> = conn.smembers(WHITELIST_IPS).await.unwrap_or_default();
        let mut records = Vec::new();

        for raw in all.values() {
            let Some(record) = deserialize_whitelist_record(raw) else {
                continue;
            };
            if record.is_active() {
                records.push(record);
            }
        }
        records.sort_by(|left, right| right.created_at.cmp(&left.created_at));

        let mut pipe = redis::pipe();
        pipe.del(WHITELIST_RECORD_ORDER).ignore();
        pipe.del(WHITELIST_EXPIRY).ignore();
        pipe.del(WHITELIST_IPS).ignore();
        pipe.del(WHITELIST_CIDR_RECORDS).ignore();
        for ip in existing_ips {
            pipe.del(whitelist_ip_records_key(&ip)).ignore();
        }
        for record in &records {
            pipe.zadd(WHITELIST_RECORD_ORDER, &record.id, record.created_at)
                .ignore();
            if let Some(expire_at) = record.expire_at {
                pipe.zadd(WHITELIST_EXPIRY, &record.id, expire_at).ignore();
            }
            queue_whitelist_indexes(&mut pipe, record);
        }
        let _: () = pipe.query_async(&mut conn).await?;
        Ok(records)
    }

    async fn list_whitelist_region_group_concrete_targets(
        &self,
    ) -> redis::RedisResult<Vec<WhitelistConcreteTarget>> {
        let mut conn = self.conn();
        let ids: Vec<String> = conn.zrevrange(WHITELIST_REGION_GROUP_ORDER, 0, -1).await?;
        let raws: Vec<String> = if ids.is_empty() {
            let all: HashMap<String, String> = conn.hgetall(WHITELIST_REGION_GROUP_RECORDS).await?;
            all.into_values().collect()
        } else {
            let values: Vec<Option<String>> = redis::cmd("HMGET")
                .arg(WHITELIST_REGION_GROUP_RECORDS)
                .arg(ids)
                .query_async(&mut conn)
                .await?;
            values.into_iter().flatten().collect()
        };

        let now = chrono_like_now_seconds();
        let mut targets = Vec::new();
        for raw in raws {
            let Some(record) = serde_json::from_str::<Value>(&raw).ok() else {
                continue;
            };
            if record
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("active")
                != "active"
            {
                continue;
            }
            if record
                .get("expireAt")
                .and_then(Value::as_i64)
                .is_some_and(|expire_at| expire_at <= now)
            {
                continue;
            }
            let id = record
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim();
            if id.is_empty() {
                continue;
            }
            let Some(cidrs) = record.get("cidrs").and_then(Value::as_array) else {
                continue;
            };
            for cidr in cidrs.iter().filter_map(Value::as_str) {
                let target = cidr.trim();
                if target.is_empty() {
                    continue;
                }
                targets.push(WhitelistConcreteTarget {
                    record_id: id.to_string(),
                    record_target: id.to_string(),
                    record_target_type: "cidr".to_string(),
                    source: "manual".to_string(),
                    target: target.to_string(),
                    target_type: "cidr".to_string(),
                });
            }
        }
        Ok(targets)
    }

    pub async fn docker_admin_password(
        &self,
    ) -> redis::RedisResult<Option<DockerAdminPasswordRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn.get(DOCKER_ADMIN_PASSWORD_KEY).await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_password(
        &self,
        record: &DockerAdminPasswordRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set(
            DOCKER_ADMIN_PASSWORD_KEY,
            serde_json::to_string(record).unwrap_or_default(),
        )
        .await
    }

    pub async fn docker_admin_session(
        &self,
        session_id: &str,
    ) -> redis::RedisResult<Option<DockerAdminSessionRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_session(
        &self,
        record: &DockerAdminSessionRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        let ttl = crate::time_utils::parse_iso_ms(&record.expires_at)
            .map(|expires_ms| ((expires_ms - crate::time_utils::now_ms()).max(1000) / 1000) as u64)
            .unwrap_or(record.ttl_seconds.max(1) as u64);
        conn.set_ex(
            format!("{DOCKER_ADMIN_SESSION_PREFIX}{}", record.id),
            serde_json::to_string(record).unwrap_or_default(),
            ttl,
        )
        .await
    }

    pub async fn delete_docker_admin_session(&self, session_id: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_SESSION_PREFIX}{session_id}"))
            .await
    }

    pub async fn docker_admin_login_attempt(
        &self,
        ip: &str,
    ) -> redis::RedisResult<Option<LoginAttemptRecord>> {
        let mut conn = self.conn();
        let raw: Option<String> = conn
            .get(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await?;
        Ok(raw.and_then(|value| serde_json::from_str(&value).ok()))
    }

    pub async fn set_docker_admin_login_attempt(
        &self,
        record: &LoginAttemptRecord,
    ) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.set_ex(
            format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{}", record.ip),
            serde_json::to_string(record).unwrap_or_default(),
            3600,
        )
        .await
    }

    pub async fn reset_docker_admin_login_attempt(&self, ip: &str) -> redis::RedisResult<()> {
        let mut conn = self.conn();
        conn.del(format!("{DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX}{ip}"))
            .await
    }

    pub async fn clear_docker_admin_sessions(&self) -> redis::RedisResult<usize> {
        self.clear_keys_by_prefix(DOCKER_ADMIN_SESSION_PREFIX, 200)
            .await
    }

    pub async fn clear_docker_admin_login_failures(&self) -> redis::RedisResult<usize> {
        self.clear_keys_by_prefix(DOCKER_ADMIN_LOGIN_BACKOFF_PREFIX, 200)
            .await
    }

    pub async fn reset_docker_admin_password_state(
        &self,
    ) -> redis::RedisResult<DockerAdminResetSummary> {
        let password_deleted = self.delete_key_count(DOCKER_ADMIN_PASSWORD_KEY).await?;
        let sessions_cleared = self.clear_docker_admin_sessions().await?;
        let login_failures_cleared = self.clear_docker_admin_login_failures().await?;

        Ok(DockerAdminResetSummary {
            password_cleared: password_deleted > 0,
            sessions_cleared,
            login_failures_cleared,
        })
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

pub fn default_config() -> Value {
    let gateway_config_dir = default_gateway_config_dir();
    let waf_rules_dir = format!("{}/waf", gateway_config_dir.trim_end_matches('/'));

    let subdomain_mode = json!({
        "root_domain": "",
        "auth_host": "",
        "auth_target": "http://127.0.0.1:7997",
        "cookie_domain": "",
        "edge_client_ip_enabled": false,
        "aliyun_esa_enabled": false,
        "tencent_edgeone_enabled": false,
        "public_auth_base_url": "",
        "public_http_port": 0,
        "public_https_port": 0,
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
        "default_access_mode": "login_first",
        "auto_add_whitelist_on_login": true,
        "passkey_rp_mode": "auth_host",
        "passkey_rp_id": ""
    });
    let ssl = json!({
        "cert": "",
        "key": "",
        "active_cert_id": "",
        "deployment_mode": "single_active",
        "certificates": []
    });
    let fnos_share_bypass = json!({
        "enabled": false,
        "upstream_timeout_ms": 2500,
        "validation_cache_ttl_seconds": 30,
        "validation_lock_ttl_seconds": 5,
        "session_ttl_seconds": 300
    });
    let fnos_port_icon_hijack = json!({
        "enabled": false,
        "updated_at": null
    });
    let fnos_network_tuning = json!({
        "bbr_enabled": false,
        "mtu_probing_enabled": false,
        "previous_tcp_congestion_control": null,
        "previous_default_qdisc": null,
        "previous_tcp_mtu_probing": null,
        "updated_at": null,
        "last_error": null
    });
    let gateway_logging = json!({
        "enabled": false,
        "max_days": 7
    });
    let waf = json!({
        "enabled": false,
        "system_rules_auto_update_enabled": true,
        "common_location_exempt_enabled": false,
        "mode": "blocking",
        "active_bundle_id": "local",
        "rules_dir": waf_rules_dir,
        "paranoia_level": 1,
        "executing_paranoia_level": 1,
        "inbound_anomaly_threshold": 5,
        "outbound_anomaly_threshold": 4,
        "request_body_access": true,
        "request_body_limit_bytes": 131072,
        "request_body_in_memory_limit_bytes": 65536,
        "response_body_access": false,
        "disabled_hosts": [],
        "disabled_path_prefixes": [],
        "log_retention_days": 7,
        "drain_interval_seconds": 2,
        "updated_at": null
    });
    let reverse_proxy_throttle = json!({
        "enabled": true,
        "requests_per_second": 100,
        "burst": 200,
        "block_seconds": 30
    });
    let gateway_visibility = json!({
        "enabled": false,
        "selections": [],
        "custom_cidrs": []
    });
    let gateway_proxy_headers = json!({ "disabled_hosts": [] });
    let gateway_host_response = json!({ "disabled_hosts": [] });
    let gateway_crawler_blocker = json!({
        "enabled": false,
        "updated_at": null
    });
    let gateway_portal = json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "icon_drag_mode": "corners"
    });
    let appearance = json!({ "theme_color_preset": "default" });
    let dashboard_display = json!({ "show_entry_status_module": true });
    let auto_https = json!({ "enabled": false });
    let smart_connect = json!({
        "enabled": false,
        "selected_ipv4": ""
    });
    let scan_discovery = json!({
        "custom_cidrs": [],
        "selected_cidrs": []
    });
    let locale = json!({ "default_locale": "zh-CN" });
    let auth_credential_settings = json!({
        "session_ttl_seconds": 86400,
        "remember_me_ttl_seconds": 31536000,
        "post_login_ip_grant_mode": "follow_session",
        "post_login_ip_grant_ttl_seconds": 3600,
        "session_ip_mobility_enabled": false,
        "session_ip_mobility_window_seconds": 1200,
        "passkey_bind_prompt_enabled": true
    });
    let event_system = json!({
        "enabled": true,
        "retention_days": 30,
        "rules": {
            "login_failure": { "enabled": true },
            "ip_drift": { "enabled": true },
            "scanner_blocked": { "enabled": true },
            "ddns_update": { "enabled": true },
            "gateway_throttle_block": { "enabled": true },
            "waf_blocked": { "enabled": true },
            "app_update_available": { "enabled": true },
            "frp_tunnel": { "enabled": true },
            "cloudflared_tunnel": { "enabled": true },
            "ssh_login_success": { "enabled": true },
            "ssh_login_failure": { "enabled": true },
            "ssh_ip_blocked": { "enabled": true },
            "cpu_alert": {
                "enabled": true,
                "threshold_percent": 80,
                "recover_percent": 60,
                "sample_interval_seconds": 5,
                "sustain_seconds": 30
            },
            "memory_alert": {
                "enabled": true,
                "threshold_percent": 80,
                "recover_percent": 60,
                "sample_interval_seconds": 5,
                "sustain_seconds": 30
            }
        }
    });
    let terminal_feature = json!({
        "enabled": false,
        "default_cwd": "~",
        "max_sessions": 3,
        "idle_timeout_seconds": 86400,
        "resume_backend": "tmux",
        "allow_mobile_toolbar": true,
        "dangerously_run_as_current_user": true
    });
    let ssh_security = json!({
        "enabled": false,
        "window_minutes": 10,
        "failed_login_threshold": 5,
        "block_duration_value": 1,
        "block_duration_unit": "day",
        "allowed_regions": [],
        "custom_cidrs": [],
        "configured_at": null,
        "updated_at": null
    });

    json!({
        "run_type": 3,
        "reverse_proxy_submode": "host",
        "auto_manage_firewall": true,
        "whitelist_ips": [],
        "proxy_mappings": [],
        "host_mappings": [],
        "stream_mappings": [],
        "subdomain_mode": subdomain_mode,
        "ssl": ssl,
        "default_route": "/__select__",
        "default_tunnel": "frp",
        "fnos_share_bypass": fnos_share_bypass,
        "fnos_port_icon_hijack": fnos_port_icon_hijack,
        "fnos_network_tuning": fnos_network_tuning,
        "gateway_logging": gateway_logging,
        "waf": waf,
        "reverse_proxy_throttle": reverse_proxy_throttle,
        "gateway_visibility": gateway_visibility,
        "gateway_proxy_headers": gateway_proxy_headers,
        "gateway_host_response": gateway_host_response,
        "gateway_crawler_blocker": gateway_crawler_blocker,
        "gateway_portal": gateway_portal,
        "appearance": appearance,
        "dashboard_display": dashboard_display,
        "auto_https": auto_https,
        "smart_connect": smart_connect,
        "scan_discovery": scan_discovery,
        "locale": locale,
        "auth_credential_settings": auth_credential_settings,
        "event_system": event_system,
        "terminal_feature": terminal_feature,
        "ssh_security": ssh_security
    })
}

fn default_gateway_config_dir() -> String {
    std::env::var("FN_KNOCK_GATEWAY_CONFIG_DIR")
        .or_else(|_| std::env::var("GATEWAY_CONFIG_DIR"))
        .or_else(|_| std::env::var("FN_KNOCK_DATA_DIR"))
        .unwrap_or_else(|_| "/tmp/fn-knock".to_string())
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
        return json!({ "mode": "all", "hosts": [] });
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
    json!({ "mode": "custom", "hosts": hosts })
}

fn normalize_totp_subdomain_access_host(value: &str) -> String {
    let mut host = value.trim().to_ascii_lowercase();
    if host.is_empty() {
        return String::new();
    }
    if host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE || host == TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH {
        return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE.to_string();
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
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        comment: None,
        ip: ip.to_string(),
        user_agent: user_agent.to_string(),
        login_time: now_iso(),
        expires_at: Some(iso_after_seconds(ttl_seconds)),
        ip_location: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sorts_backup_strings_like_node_locale_compare() {
        let mut values = [
            "fn_knock:a",
            "fn_knock:Z",
            "fn_knock:A",
            "fn_knock:z",
            "fn_knock:_",
            "fn_knock:-",
            "fn_knock:2",
            "fn_knock:10",
            "fn_knock:á",
            "fn_knock:ä",
        ]
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();

        values.sort_by(|left, right| node_locale_compare_ordering(left, right));

        assert_eq!(
            values,
            vec![
                "fn_knock:_",
                "fn_knock:-",
                "fn_knock:10",
                "fn_knock:2",
                "fn_knock:a",
                "fn_knock:A",
                "fn_knock:á",
                "fn_knock:ä",
                "fn_knock:z",
                "fn_knock:Z",
            ]
        );
        assert_eq!(node_locale_compare_ordering("a", "Z"), Ordering::Less);
        assert_eq!(node_locale_compare_ordering("😀", "0"), Ordering::Less);
        assert_eq!(node_locale_compare_ordering("中", "z"), Ordering::Greater);
    }

    #[test]
    fn default_config_top_level_keys_match_node_default_config() {
        let config = default_config();
        let keys = config
            .as_object()
            .expect("default config is object")
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = [
            "run_type",
            "reverse_proxy_submode",
            "auto_manage_firewall",
            "whitelist_ips",
            "proxy_mappings",
            "host_mappings",
            "stream_mappings",
            "subdomain_mode",
            "ssl",
            "default_route",
            "default_tunnel",
            "fnos_share_bypass",
            "fnos_port_icon_hijack",
            "fnos_network_tuning",
            "gateway_logging",
            "waf",
            "reverse_proxy_throttle",
            "gateway_visibility",
            "gateway_proxy_headers",
            "gateway_host_response",
            "gateway_crawler_blocker",
            "gateway_portal",
            "appearance",
            "dashboard_display",
            "auto_https",
            "smart_connect",
            "scan_discovery",
            "auth_credential_settings",
            "event_system",
            "terminal_feature",
            "ssh_security",
            "locale",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();

        assert_eq!(keys, expected);
    }

    #[test]
    fn default_config_includes_node_runtime_feature_defaults() {
        let config = default_config();

        assert_eq!(
            config.pointer("/event_system/rules/cpu_alert/enabled"),
            Some(&json!(true))
        );
        assert_eq!(
            config.pointer("/event_system/rules/cpu_alert/threshold_percent"),
            Some(&json!(80))
        );
        assert_eq!(
            config.pointer("/event_system/rules/memory_alert/sample_interval_seconds"),
            Some(&json!(5))
        );
        assert_eq!(
            config.pointer("/terminal_feature/idle_timeout_seconds"),
            Some(&json!(86400))
        );
        assert_eq!(
            config.pointer("/gateway_portal/display_style"),
            Some(&json!("title"))
        );
        assert_eq!(
            config.pointer("/waf/system_rules_auto_update_enabled"),
            Some(&json!(true))
        );
    }

    #[test]
    fn normalizes_totp_access_scopes_like_node() {
        assert_eq!(
            normalize_totp_access_scopes(json!([
                " docker_admin_panel ",
                "other",
                "docker_admin_panel"
            ])),
            json!(["docker_admin_panel"])
        );
        assert_eq!(normalize_totp_access_scopes(json!("nope")), json!([]));
    }

    #[test]
    fn normalizes_totp_credentials_like_node_store() {
        let credentials = normalize_totp_credentials_value(&json!([
            {
                "id": " one ",
                "secret": " SECRET ",
                "comment": "  Comment  ",
                "createdAt": "",
                "access_scopes": [" docker_admin_panel "],
                "subdomain_access": { "mode": "custom", "hosts": ["Example.com."] }
            },
            { "id": "", "secret": "NOPE" }
        ]));
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].id, "one");
        assert_eq!(credentials[0].secret, "SECRET");
        assert_eq!(credentials[0].comment, "Comment");
        assert!(crate::time_utils::parse_iso_ms(&credentials[0].created_at).is_some());
        assert_eq!(credentials[0].access_scopes, json!(["docker_admin_panel"]));
        assert_eq!(
            credentials[0].subdomain_access,
            json!({ "mode": "custom", "hosts": ["example.com"] })
        );
    }

    #[test]
    fn normalizes_totp_subdomain_access_like_node() {
        assert_eq!(
            normalize_totp_subdomain_access(json!({
                "mode": "custom",
                "hosts": [
                    "HTTPS://Example.COM:8443/path?q=1",
                    "example.com.",
                    "/__select__",
                    "*.bad.test",
                    "bad host"
                ]
            })),
            json!({
                "mode": "custom",
                "hosts": ["__builtin_select__", "example.com"]
            })
        );
        assert_eq!(
            normalize_totp_subdomain_access(json!({ "mode": "all", "hosts": ["example.com"] })),
            json!({ "mode": "all", "hosts": [] })
        );
    }

    #[test]
    fn cname_whitelist_concrete_targets_normalize_dedupe_and_sort_ips() {
        let record = WhitelistRecord {
            id: "whitelist:1".to_string(),
            ip: "example.com".to_string(),
            target_type: "cname".to_string(),
            expire_at: None,
            source: "manual".to_string(),
            created_at: 1,
            status: "active".to_string(),
            comment: None,
            ip_location: None,
            resolved_targets: Some(vec![
                " 192.0.2.1 ".to_string(),
                "not-an-ip".to_string(),
                "2001:DB8::1".to_string(),
                "192.0.2.1".to_string(),
            ]),
            check_interval_minutes: None,
            last_checked_at: None,
            last_resolved_at: None,
            resolve_status: None,
            resolve_message: None,
        };
        let targets = record
            .concrete_targets()
            .into_iter()
            .map(|target| target.target)
            .collect::<Vec<_>>();
        assert_eq!(targets, vec!["192.0.2.1", "2001:DB8::1"]);
    }

    #[test]
    fn stale_whitelist_cleanup_targets_match_node_indexes() {
        let mut record = WhitelistRecord {
            id: "whitelist:1".to_string(),
            ip: "example.com".to_string(),
            target_type: "cname".to_string(),
            expire_at: None,
            source: "manual".to_string(),
            created_at: 1,
            status: "expired".to_string(),
            comment: None,
            ip_location: None,
            resolved_targets: Some(vec![
                "192.0.2.1".to_string(),
                "bad".to_string(),
                "192.0.2.1".to_string(),
                "2001:DB8::1".to_string(),
            ]),
            check_interval_minutes: None,
            last_checked_at: None,
            last_resolved_at: None,
            resolve_status: None,
            resolve_message: None,
        };
        assert_eq!(
            whitelist_stale_ip_index_targets(&record),
            vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()]
        );

        record.target_type = "cidr".to_string();
        record.ip = "192.0.2.0/24".to_string();
        record.resolved_targets = None;
        assert!(whitelist_stale_ip_index_targets(&record).is_empty());
    }

    #[test]
    fn deserializes_whitelist_records_like_node_store() {
        let record = deserialize_whitelist_record(
            r#"{
                "id": " whitelist:legacy ",
                "ip": "Example.COM.",
                "expireAt": "123abc",
                "createdAt": "456.9",
                "resolvedTargets": [" 192.0.2.1 ", "bad", "2001:DB8::1", "192.0.2.1"],
                "checkIntervalMinutes": "10m",
                "lastCheckedAt": "",
                "resolveStatus": "nope",
                "resolveMessage": " resolved "
            }"#,
        )
        .unwrap();
        assert_eq!(record.id, "whitelist:legacy");
        assert_eq!(record.ip, "example.com");
        assert_eq!(record.target_type, "cname");
        assert_eq!(record.expire_at, Some(123));
        assert_eq!(record.created_at, 456);
        assert_eq!(record.source, "manual");
        assert_eq!(record.status, "active");
        assert_eq!(
            record.resolved_targets,
            Some(vec!["192.0.2.1".to_string(), "2001:DB8::1".to_string()])
        );
        assert_eq!(record.check_interval_minutes, Some(10));
        assert_eq!(record.last_checked_at, None);
        assert_eq!(record.resolve_status.as_deref(), Some("pending"));
        assert_eq!(record.resolve_message.as_deref(), Some("resolved"));
    }

    #[test]
    fn deserializes_whitelist_region_groups_like_node_store() {
        let group = deserialize_whitelist_region_group(
            r#"{
                "id": " whitelist-region:legacy ",
                "regions": [
                    { "province": 440000, "query_city": true },
                    { "province": "广东", "query_city": "" },
                    { "province": " ", "query_city": "ignored" },
                    null
                ],
                "cidrs": [" 192.0.2.0/24 ", 123, null],
                "expireAt": "0x10",
                "createdAt": true,
                "updatedAt": "456.9",
                "status": "nope",
                "source": "auto",
                "comment": null
            }"#,
        )
        .unwrap();
        assert_eq!(group.id, "whitelist-region:legacy");
        assert_eq!(
            group.regions,
            vec![
                WhitelistRegionInput {
                    province: "440000".to_string(),
                    query_city: Some("true".to_string())
                },
                WhitelistRegionInput {
                    province: "广东".to_string(),
                    query_city: None
                }
            ]
        );
        assert_eq!(
            group.cidrs,
            vec!["192.0.2.0/24".to_string(), "123".to_string()]
        );
        assert_eq!(group.expire_at, Some(16));
        assert_eq!(group.created_at, 1);
        assert_eq!(group.updated_at, 456);
        assert_eq!(group.status, "active");
        assert_eq!(group.source, "manual");
        assert_eq!(group.comment.as_deref(), Some(""));
    }

    #[test]
    fn reads_login_backoff_status_like_node_store() {
        let status = login_backoff_status_from_raw(
            "203.0.113.10",
            Some(r#"{"ip":"ignored","attempts":-2,"blockedUntil":1100}"#),
            1000,
        );
        assert_eq!(status.ip, "203.0.113.10");
        assert_eq!(status.attempts, -2);
        assert!(status.blocked);
        assert_eq!(status.retry_after, Some(1));
        assert_eq!(status.blocked_until, Some(1100));

        let expired = login_backoff_status_from_raw(
            "203.0.113.10",
            Some(r#"{"ip":"ignored","attempts":3,"blockedUntil":999}"#),
            1000,
        );
        assert_eq!(expired.attempts, 3);
        assert!(!expired.blocked);
        assert_eq!(expired.retry_after, None);
    }

    #[test]
    fn docker_admin_session_record_accepts_legacy_missing_ttl() {
        let record: DockerAdminSessionRecord = serde_json::from_str(
            r#"{
                "id": "session-1",
                "created_at": "2026-01-01T00:00:00.000Z",
                "updated_at": "2026-01-01T00:00:00.000Z",
                "expires_at": "2026-01-01T12:00:00.000Z",
                "ip": "203.0.113.10",
                "user_agent": "ua"
            }"#,
        )
        .expect("legacy docker admin session");

        assert_eq!(record.ttl_seconds, 0);
    }

    #[test]
    fn traffic_scope_matches_node_uri_encoding() {
        assert_eq!(traffic_scope_segment("global", None), "global");
        assert_eq!(traffic_scope_segment("", None), "");
        assert_eq!(traffic_scope_segment(" user ", None), " user ");
        assert_eq!(
            traffic_scope_segment("global", Some("example.com")),
            "global:host:example.com"
        );
        assert_eq!(
            traffic_scope_segment(" user ", Some("example.com")),
            " user :host:example.com"
        );
        assert_eq!(
            traffic_scope_segment("u", Some("[2001:db8::1]")),
            "u:host:%5B2001%3Adb8%3A%3A1%5D"
        );
    }

    #[test]
    fn system_event_search_uses_unicode_lowercase_like_node() {
        let event = json!({
            "id": "evt_unicode",
            "type": "FN_EVENT_AUTH_LOGIN_SUCCESS",
            "source": "SERVER_ADMIN",
            "level": "INFO",
            "happened_at": "2026-07-07T00:00:00.000Z",
            "payload": {
                "credential_name": "Älice"
            }
        });

        assert!(system_event_matches_filters(
            &event, "älice", None, None, None
        ));
    }

    #[test]
    fn parses_traffic_members_and_ignores_invalid_values() {
        assert_eq!(
            parse_traffic_points(&[
                "10:5".to_string(),
                "bad".to_string(),
                "11:nope".to_string(),
                "12:0".to_string()
            ]),
            vec![
                TrafficDeltaPoint { ts: 10, delta: 5.0 },
                TrafficDeltaPoint { ts: 12, delta: 0.0 }
            ]
        );
    }

    #[test]
    fn counter_delta_handles_first_sample_and_resets() {
        assert_eq!(compute_counter_delta(100.0, None), 100.0);
        assert_eq!(compute_counter_delta(120.0, Some(100.0)), 20.0);
        assert_eq!(compute_counter_delta(12.0, Some(100.0)), 12.0);
        assert_eq!(compute_counter_delta(-1.0, Some(100.0)), 0.0);
    }

    #[test]
    fn waf_log_dates_include_neighboring_utc_days() {
        let dates = waf_log_dates_for_range(1_704_067_200_000, 1_704_153_600_000);
        assert!(dates.contains(&"2023-12-31".to_string()));
        assert!(dates.contains(&"2024-01-01".to_string()));
        assert!(dates.contains(&"2024-01-02".to_string()));
        assert!(dates.contains(&"2024-01-03".to_string()));
    }
}
