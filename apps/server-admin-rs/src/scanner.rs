use std::{collections::BTreeSet, env, net::IpAddr};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use crate::{
    common_auth_locations, http_utils, i18n::Translator, ip_location, response, state::AppState,
    system_events, time_utils,
};

const SCANNER_BASE_WINDOW_SECONDS: i64 = 5 * 60;
const DEFAULT_CIDR_API_URL: &str = "https://cidr.fnknock.cn/api/v1";
const IP_LOCATION_API_SETTINGS_KEY: &str = "fn_knock:ip-location-api:settings";
const CIDR_CACHE_PREFIX: &str = "fn_knock:cidr";
const CIDR_SUCCESS_CACHE_TTL_SECONDS: usize = 30 * 24 * 60 * 60;
const CIDR_PROVINCE_WIDE_VALUE: &str = "__province_all__";
const CIDR_USER_AGENT: &str = "fn-knock-server-admin/1.0";
const CIDR_CITY_ONLY_PROVINCES: &[&str] = &["广东", "浙江"];
const SUBSONIC_REST_ENDPOINTS: &[&str] = &[
    "addchatmessage",
    "changeemail",
    "changepassword",
    "createbookmark",
    "createinternetradiostation",
    "createplaylist",
    "createpodcastchannel",
    "createshare",
    "createuser",
    "deletebookmark",
    "deleteinternetradiostation",
    "deleteplaylist",
    "deletepodcastchannel",
    "deletepodcastepisode",
    "deleteshare",
    "deleteuser",
    "download",
    "downloadpodcastepisode",
    "getalbum",
    "getalbuminfo",
    "getalbuminfo2",
    "getalbumlist",
    "getalbumlist2",
    "getartist",
    "getartistinfo",
    "getartistinfo2",
    "getartists",
    "getavatar",
    "getbookmarks",
    "getchatmessages",
    "getcoverart",
    "getgenres",
    "getindexes",
    "getinternetradiostations",
    "getlicense",
    "getlyrics",
    "getlyricsbysongid",
    "getmusicdirectory",
    "getmusicfolders",
    "getnewestpodcasts",
    "getnowplaying",
    "getplaylists",
    "getplaylist",
    "getplayqueue",
    "getpodcasts",
    "getrandomsongs",
    "getshares",
    "getsimilarsongs",
    "getsimilarsongs2",
    "getsong",
    "getsongsbygenre",
    "getstarred",
    "getstarred2",
    "gettopsongs",
    "getuser",
    "getusers",
    "getvideoinfo",
    "getvideos",
    "hls",
    "jukeboxcontrol",
    "ping",
    "refreshpodcasts",
    "saveplayqueue",
    "scrobble",
    "search2",
    "search3",
    "setrating",
    "star",
    "stream",
    "unstar",
    "updateinternetradiostation",
    "updateplaylist",
    "updateshare",
    "updateuser",
];

fn scanner_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.scanner.{key}"))
}

fn scanner_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.scanner.{key}"), params)
}

fn cidr_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.cidr.{key}"))
}

fn cidr_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.cidr.{key}"), params)
}

fn localize_scanner_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    match message {
        "Invalid request body" => return scanner_text(translator, "invalidRequestBody"),
        "At least one IP is required" => return scanner_text(translator, "atLeastOneIpRequired"),
        "Record not found" => return scanner_text(translator, "recordNotFound"),
        "province is required" => return cidr_text(translator, "provinceRequired"),
        _ => {}
    }

    if let Some(cidrs) = message.strip_prefix("Invalid CIDR exemptions: ") {
        return scanner_text_params(
            translator,
            "cidrExemptionsInvalid",
            &[("cidrs", cidrs.to_string())],
        );
    }

    localize_cidr_error(translator, message)
}

fn localize_cidr_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    if message.is_empty() {
        return cidr_text(translator, "serviceError");
    }
    match message {
        "CIDR service failed" => return cidr_text(translator, "serviceError"),
        "CIDR upstream response missing data" => {
            return cidr_text(translator, "upstreamUnexpected");
        }
        _ => {}
    }
    if let Some(detail) = message.strip_prefix("Invalid CIDR API URL: ") {
        return cidr_text_params(
            translator,
            "invalidApiUrl",
            &[("error", detail.to_string())],
        );
    }
    if let Some(status) = message.strip_prefix("CIDR upstream request failed: HTTP ") {
        return cidr_text_params(
            translator,
            "upstreamRequestFailed",
            &[("status", status.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("CIDR upstream request failed: ") {
        return cidr_text_params(
            translator,
            "upstreamRequestFailedGeneric",
            &[("error", detail.to_string())],
        );
    }
    if message.starts_with("CIDR upstream returned invalid JSON") {
        return cidr_text(translator, "invalidJson");
    }
    message.to_string()
}

#[derive(Debug, thiserror::Error)]
enum ScannerError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Cidr(String),
    #[error(transparent)]
    Redis(#[from] redis::RedisError),
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<String>,
    limit: Option<String>,
    search: Option<String>,
}

#[derive(Deserialize)]
struct CidrProvinceQuery {
    province: Option<String>,
}

#[derive(Deserialize)]
struct CidrCityQuery {
    province: String,
    city: Option<String>,
}

#[derive(Deserialize)]
struct UpdateScannerSettingsBody {
    enabled: bool,
    #[serde(rename = "windowMinutes")]
    window_minutes: f64,
    threshold: f64,
    #[serde(rename = "blacklistTtlSeconds")]
    blacklist_ttl_seconds: f64,
    #[serde(default, rename = "commonLocationExemptEnabled")]
    common_location_exempt_enabled: Option<bool>,
    #[serde(default, rename = "cidrExemptions")]
    cidr_exemptions: Option<Vec<String>>,
    #[serde(default, rename = "cidrExemptionRegions")]
    cidr_exemption_regions: Option<Vec<ScannerCidrExemptionRegionBody>>,
}

#[derive(Clone, Debug, Deserialize)]
struct ScannerCidrExemptionRegionBody {
    province: String,
    #[serde(default)]
    query_city: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct ScannerCidrExemptionRegionInput {
    province: String,
    query_city: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct ScannerCidrExemptionSelection {
    province: String,
    city: Option<String>,
    label: String,
    value: String,
    query_city: Option<String>,
    is_province_wide: bool,
    is_municipality: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
struct ScannerSettings {
    enabled: bool,
    #[serde(rename = "windowMinutes")]
    window_minutes: i64,
    threshold: i64,
    #[serde(rename = "windowSeconds")]
    window_seconds: i64,
    #[serde(rename = "blacklistTtlSeconds")]
    blacklist_ttl_seconds: i64,
    #[serde(rename = "commonLocationExemptEnabled")]
    common_location_exempt_enabled: bool,
    #[serde(rename = "cidrExemptions")]
    cidr_exemptions: Vec<String>,
    #[serde(rename = "cidrExemptionRegions")]
    cidr_exemption_regions: Vec<ScannerCidrExemptionSelection>,
    #[serde(rename = "cidrExemptionRegionCidrs")]
    cidr_exemption_region_cidrs: Vec<String>,
    #[serde(rename = "cidrExemptionCidrs")]
    cidr_exemption_cidrs: Vec<String>,
}

#[derive(Clone, Copy)]
struct ScannerEnvDefaults {
    enabled: bool,
    window_minutes: i64,
    threshold: i64,
    blacklist_ttl_seconds: i64,
}

struct ResolvedCidrLookup {
    selection: ScannerCidrExemptionSelection,
    cidrs: Vec<String>,
}

pub(crate) struct CidrRegionLookup {
    pub selection: Value,
    pub cidrs: Vec<String>,
}

pub fn scanner_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/scanner/settings",
            get(get_settings).post(update_settings),
        )
        .route(
            "/api/admin/scanner/blacklist",
            get(list_blacklist).delete(delete_blacklist),
        )
        .route(
            "/api/admin/scanner/blacklist/",
            get(list_blacklist).delete(delete_blacklist),
        )
        .route(
            "/api/admin/scanner/blacklist/{ip}",
            get(get_blacklist_record).delete(delete_blacklist_record),
        )
}

pub fn cidr_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/cidr/provinces", get(get_cidr_provinces))
        .route("/api/admin/cidr/cities", get(get_cidr_cities))
        .route("/api/admin/cidr/selector", get(get_cidr_selector))
        .route("/api/admin/cidr/cidrs", get(get_cidr_cidrs))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScannerPreflightRecordResult {
    pub hit_count: i64,
    pub blocked: bool,
}

pub(crate) fn is_request_exempt_from_scan(headers: &HeaderMap, uri: &Uri, config: &Value) -> bool {
    let forwarded_path = resolve_forwarded_path(headers, uri);
    if config
        .get("fnos_share_bypass")
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        == Some(true)
        && is_fnos_share_path(&forwarded_path)
    {
        return true;
    }

    if is_any_subdomain_routing_mode(config) {
        let matched = find_matching_host_mapping(
            &resolve_forwarded_host(headers, uri),
            config
                .get("host_mappings")
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or(&[]),
        );
        return is_public_host_mapping(matched);
    }

    let proxy_mappings = config
        .get("proxy_mappings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    if let Some(mapping) = find_best_matching_proxy_mapping(&forwarded_path, proxy_mappings) {
        return mapping
            .get("use_auth")
            .and_then(Value::as_bool)
            .unwrap_or(true)
            == false;
    }

    resolve_default_proxy_mapping(config, proxy_mappings)
        .and_then(|mapping| mapping.get("use_auth").and_then(Value::as_bool))
        == Some(false)
}

pub(crate) async fn is_blacklisted_for_preflight(
    state: &AppState,
    ip: &str,
) -> anyhow::Result<bool> {
    let clean_ip = normalize_scanner_ip(ip);
    if clean_ip.is_empty() || is_scanner_local_address(&clean_ip) {
        return Ok(false);
    }

    let settings = load_scanner_settings(state).await?;
    if !settings.enabled || is_scanner_exempt_ip(state, &clean_ip, &settings).await? {
        return Ok(false);
    }

    Ok(state.redis.scanner_blacklist_exists(&clean_ip).await?)
}

pub(crate) async fn is_common_path_for_preflight(
    state: &AppState,
    path: &str,
) -> anyhow::Result<bool> {
    let clean_path = normalize_scanner_path(path);
    if is_known_subsonic_rest_path(path) {
        return Ok(true);
    }
    if clean_path == "/__auth__" || clean_path.starts_with("/__auth__/") {
        return Ok(true);
    }
    if clean_path == "/api/auth/passkey" || clean_path.starts_with("/api/auth/passkey/") {
        return Ok(true);
    }
    if clean_path == "/websocket" {
        return Ok(true);
    }
    if clean_path == "/api/admin/terminal" || clean_path.starts_with("/api/admin/terminal/") {
        return Ok(true);
    }
    if clean_path == "/cgi/ThirdParty" || clean_path.starts_with("/cgi/ThirdParty/") {
        return Ok(true);
    }
    if clean_path == "/assets/" || clean_path.starts_with("/assets/") {
        return Ok(true);
    }
    if clean_path == "/s/" || clean_path.starts_with("/s/") {
        return Ok(true);
    }

    const COMMON_PATHS: &[&str] = &[
        "/",
        "/index.html",
        "/robots.txt",
        "/sitemap.xml",
        "/favicon.ico",
        "/favicon.svg",
        "/api/auth/bootstrap",
        "/api/auth/captcha/config",
        "/api/auth/challenge",
        "/api/auth/login",
        "/api/auth/ip",
        "/api/auth/ip/location",
        "/api/auth/session",
        "/api/auth/verify",
        "/api/auth/passkey/status",
        "/trimcon",
        "/.well-known/ai-plugin.json",
        "/apple-touch-icon.png",
        "/manifest.json",
        "/login",
        "/locales/zh-CN/os.json",
        "/license/v1/device/baseInfo",
        "/locales/zh-CN/apps/setting.json",
        "/app-center/v1/check-update?language=zh-CN",
        "/sac/rpcproxy/v1/new-user-guide/status",
        "/locales/zh-CN/pages/login.json",
        "/static/bg/wallpaper-1.webp",
        "/api/config",
        "/identity/connect/token",
    ];
    if COMMON_PATHS.contains(&clean_path.as_str()) {
        return Ok(true);
    }

    let config = state.redis.get_config().await?;
    Ok(config
        .get("proxy_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|mapping| {
            mapping
                .get("path")
                .and_then(Value::as_str)
                .is_some_and(|mapping_path| is_known_proxy_path(&clean_path, mapping_path))
        }))
}

pub(crate) async fn record_uncommon_path_for_preflight(
    state: &AppState,
    ip: &str,
    path: &str,
) -> anyhow::Result<ScannerPreflightRecordResult> {
    let clean_ip = normalize_scanner_ip(ip);
    if clean_ip.is_empty() || is_scanner_local_address(&clean_ip) {
        return Ok(ScannerPreflightRecordResult {
            hit_count: 0,
            blocked: false,
        });
    }

    let settings = load_scanner_settings(state).await?;
    if !settings.enabled || is_scanner_exempt_ip(state, &clean_ip, &settings).await? {
        return Ok(ScannerPreflightRecordResult {
            hit_count: 0,
            blocked: false,
        });
    }

    let now = time_utils::now_ms();
    let clean_path = normalize_scanner_path(path);
    let hit = json!({ "path": clean_path, "createdAt": now });
    let min_score = now - settings.window_seconds * 1000;
    let window_min_score = now - settings.window_minutes * 60 * 1000;
    let hit_count = state
        .redis
        .record_scanner_suspicious_hit(
            &clean_ip,
            &hit,
            now,
            min_score,
            window_min_score,
            settings.window_seconds + 60,
        )
        .await?;

    if hit_count >= settings.threshold && !is_blacklisted_for_preflight(state, &clean_ip).await? {
        let hits = state
            .redis
            .scanner_suspicious_hits_since(&clean_ip, window_min_score)
            .await?
            .into_iter()
            .filter(|value| {
                value.get("path").and_then(Value::as_str).is_some()
                    && value.get("createdAt").and_then(Value::as_i64).is_some()
            })
            .collect::<Vec<_>>();
        let ip_location = state
            .redis
            .get_ip_location_cache(&clean_ip)
            .await?
            .and_then(|value| {
                value
                    .get("raw")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .filter(|value| !value.trim().is_empty());
        let mut record = json!({
            "ip": clean_ip,
            "blockedAt": now,
            "windowMinutes": settings.window_minutes,
            "threshold": settings.threshold,
            "hits": hits,
        });
        if let Some(location) = ip_location.clone()
            && let Some(object) = record.as_object_mut()
        {
            object.insert("ipLocation".to_string(), Value::String(location));
        }
        state
            .redis
            .add_scanner_blacklist_record(&clean_ip, &record, now, settings.blacklist_ttl_seconds)
            .await?;
        let registered_location = ip_location::register_usage(
            state,
            &clean_ip,
            vec![format!("scanner-blacklist|{clean_ip}")],
        )
        .await
        .unwrap_or_default();
        let event_hits = record
            .get("hits")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|hit| {
                let path = hit.get("path").and_then(Value::as_str)?;
                let created_at = hit.get("createdAt").and_then(Value::as_i64)?;
                Some(json!({
                    "path": path,
                    "created_at": time_utils::iso_from_ms(created_at),
                }))
            })
            .collect::<Vec<_>>();
        let mut payload = json!({
            "ip": clean_ip,
            "blocked_at": time_utils::iso_from_ms(now),
            "window_minutes": settings.window_minutes,
            "threshold": settings.threshold,
            "hit_count": hit_count,
            "hits": event_hits,
        });
        if let Some(location) = ip_location
            .filter(|value| !value.trim().is_empty())
            .or_else(|| (!registered_location.trim().is_empty()).then_some(registered_location))
            && let Some(object) = payload.as_object_mut()
        {
            object.insert("ip_location".to_string(), Value::String(location));
        }
        if let Err(error) = system_events::publish_scanner_blocked_event(state, payload).await {
            tracing::warn!(%error, %clean_ip, "failed to publish scanner blocked event");
        }
        return Ok(ScannerPreflightRecordResult {
            hit_count,
            blocked: true,
        });
    }

    Ok(ScannerPreflightRecordResult {
        hit_count,
        blocked: false,
    })
}

async fn is_scanner_exempt_ip(
    state: &AppState,
    ip: &str,
    settings: &ScannerSettings,
) -> anyhow::Result<bool> {
    if settings.common_location_exempt_enabled
        && common_auth_locations::is_common_auth_location_exempt_ip(state, ip).await?
    {
        return Ok(true);
    }
    Ok(is_scanner_cidr_exempt_ip(
        ip,
        &settings.cidr_exemption_cidrs,
    ))
}

fn is_scanner_cidr_exempt_ip(ip: &str, cidrs: &[String]) -> bool {
    let normalized = http_utils::normalize_ip(ip);
    let Ok(ip) = normalized.parse::<IpAddr>() else {
        return false;
    };
    cidrs
        .iter()
        .filter_map(|cidr| cidr.trim().parse::<IpNet>().ok())
        .any(|network| network.contains(&ip))
}

fn normalize_scanner_ip(ip: &str) -> String {
    ip.trim().to_string()
}

fn is_scanner_local_address(ip: &str) -> bool {
    let mut candidate = ip.trim().to_ascii_lowercase();
    if candidate.is_empty() {
        return false;
    }

    if candidate.starts_with('[')
        && let Some(end) = candidate.find(']')
    {
        candidate = candidate[1..end].to_string();
    }

    if candidate == "localhost" || candidate.starts_with("localhost:") {
        return true;
    }
    if candidate == "::1" || candidate == "0:0:0:0:0:0:0:1" {
        return true;
    }
    if candidate.starts_with("fc") || candidate.starts_with("fd") || candidate.starts_with("fe80:")
    {
        return true;
    }

    if let Some(mapped) = candidate.strip_prefix("::ffff:") {
        return is_private_scanner_ipv4(mapped);
    }
    is_private_scanner_ipv4(&candidate)
}

fn is_private_scanner_ipv4(candidate: &str) -> bool {
    let (ip, port) = candidate
        .split_once(':')
        .map_or((candidate, None), |(ip, port)| (ip, Some(port)));
    if port.is_some_and(|port| port.is_empty() || port.chars().any(|ch| !ch.is_ascii_digit())) {
        return false;
    }
    let octets = ip.split('.').collect::<Vec<_>>();
    if octets.len() != 4
        || octets
            .iter()
            .any(|octet| octet.is_empty() || octet.chars().any(|ch| !ch.is_ascii_digit()))
    {
        return false;
    }
    octets[0] == "10"
        || octets[0] == "127"
        || (octets[0] == "192" && octets[1] == "168")
        || (octets[0] == "172"
            && matches!(
                octets[1],
                "16" | "17"
                    | "18"
                    | "19"
                    | "20"
                    | "21"
                    | "22"
                    | "23"
                    | "24"
                    | "25"
                    | "26"
                    | "27"
                    | "28"
                    | "29"
                    | "30"
                    | "31"
            ))
}

fn resolve_forwarded_host(headers: &HeaderMap, uri: &Uri) -> String {
    if let Some(value) = first_header_value(headers, "x-forwarded-host") {
        return normalize_scanner_host(&value);
    }
    if let Some(authority) = uri.authority() {
        return normalize_scanner_host(authority.as_str());
    }
    headers
        .get("host")
        .and_then(|value| value.to_str().ok())
        .map(normalize_scanner_host)
        .unwrap_or_default()
}

fn resolve_forwarded_path(headers: &HeaderMap, uri: &Uri) -> String {
    if let Some(value) = first_header_value(headers, "x-forwarded-path") {
        return value;
    }
    uri.path_and_query()
        .map(|value| value.as_str().to_string())
        .unwrap_or_else(|| "/".to_string())
}

fn first_header_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn normalize_scanner_host(value: &str) -> String {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return String::new();
    }

    if let Ok(url) = Url::parse(&format!("https://{normalized}"))
        && let Some(host) = url.host_str()
    {
        return host.trim_end_matches('.').to_ascii_lowercase();
    }

    let mut host = normalized.as_str();
    if let Some((_, rest)) = host.split_once("://") {
        host = rest;
    }
    if let Some((_, rest)) = host.rsplit_once('@') {
        host = rest;
    }
    host = host.split('/').next().unwrap_or("").trim();
    if host.starts_with('[')
        && let Some(end) = host.find(']')
    {
        return host[1..end].trim_end_matches('.').to_string();
    }
    if let Some((without_port, port)) = host.rsplit_once(':')
        && !port.is_empty()
        && port.chars().all(|ch| ch.is_ascii_digit())
    {
        host = without_port;
    }
    host.trim_end_matches('.').to_string()
}

fn normalize_scanner_path(path: &str) -> String {
    let clean = path
        .split('?')
        .next()
        .unwrap_or("")
        .split('#')
        .next()
        .unwrap_or("");
    if clean.is_empty() {
        return "/".to_string();
    }
    let mut normalized = if clean.starts_with('/') {
        clean.to_string()
    } else {
        format!("/{clean}")
    };
    if normalized.len() > 1 && normalized.ends_with('/') {
        normalized.pop();
    }
    normalized
}

fn parse_scanner_path(value: &str) -> Option<Url> {
    let base = Url::parse("http://127.0.0.1").ok()?;
    Url::options().base_url(Some(&base)).parse(value).ok()
}

fn normalize_subsonic_rest_endpoint(path: &str) -> String {
    let clean = normalize_scanner_path(path);
    let Some(rest) = clean.strip_prefix("/rest/") else {
        return String::new();
    };
    if rest.contains('/') {
        return String::new();
    }
    let (endpoint, extension) = rest
        .split_once('.')
        .map_or((rest, None), |(endpoint, extension)| {
            (endpoint, Some(extension))
        });
    if extension.is_some_and(|value| !matches!(value, "view" | "json" | "xml")) {
        return String::new();
    }
    let endpoint = endpoint.to_ascii_lowercase();
    if endpoint
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_alphabetic())
        && endpoint.chars().all(|value| value.is_ascii_alphanumeric())
    {
        endpoint
    } else {
        String::new()
    }
}

fn is_known_subsonic_rest_path(path: &str) -> bool {
    let Some(parsed) = parse_scanner_path(path) else {
        return false;
    };
    let endpoint = normalize_subsonic_rest_endpoint(parsed.path());
    !endpoint.is_empty() && SUBSONIC_REST_ENDPOINTS.contains(&endpoint.as_str())
}

fn is_known_proxy_path(request_path: &str, mapping_path: &str) -> bool {
    let clean_request_path = normalize_scanner_path(request_path);
    let clean_mapping_path = normalize_scanner_path(mapping_path);
    clean_mapping_path == "/"
        || clean_request_path == clean_mapping_path
        || clean_request_path.starts_with(&format!("{clean_mapping_path}/"))
}

fn is_fnos_share_path(path: &str) -> bool {
    let clean_path = normalize_scanner_path(path);
    clean_path == "/s" || clean_path.starts_with("/s/")
}

fn find_best_matching_proxy_mapping<'a>(path: &str, mappings: &'a [Value]) -> Option<&'a Value> {
    let clean_path = normalize_scanner_path(path);
    let mut best_match = None;
    let mut best_length = 0_usize;
    for mapping in mappings {
        let Some(mapping_path) = mapping.get("path").and_then(Value::as_str) else {
            continue;
        };
        if !is_known_proxy_path(&clean_path, mapping_path) {
            continue;
        }
        let length = normalize_scanner_path(mapping_path).len();
        if length > best_length {
            best_match = Some(mapping);
            best_length = length;
        }
    }
    best_match
}

fn find_matching_host_mapping<'a>(host: &str, mappings: &'a [Value]) -> Option<&'a Value> {
    let clean_host = normalize_scanner_host(host);
    if clean_host.is_empty() {
        return None;
    }
    mappings.iter().find(|mapping| {
        mapping
            .get("host")
            .and_then(Value::as_str)
            .map(normalize_scanner_host)
            == Some(clean_host.clone())
    })
}

fn is_public_host_mapping(mapping: Option<&Value>) -> bool {
    mapping.is_some_and(|mapping| {
        mapping.get("use_auth").and_then(Value::as_bool) == Some(false)
            && mapping.get("access_mode").and_then(Value::as_str) != Some("strict_whitelist")
    })
}

fn resolve_default_proxy_mapping<'a>(config: &Value, mappings: &'a [Value]) -> Option<&'a Value> {
    let default_route = config
        .get("default_route")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "/__select__")?;
    mappings.iter().find(|mapping| {
        mapping
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| {
                normalize_scanner_path(path) == normalize_scanner_path(default_route)
            })
    })
}

fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || (config.get("run_type").and_then(Value::as_i64) == Some(1)
            && config
                .get("reverse_proxy_submode")
                .and_then(Value::as_str)
                .unwrap_or("path")
                == "subdomain")
}

async fn get_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match load_scanner_settings(&state).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load scanner settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "settingsLoadFailed"),
            )
        }
    }
}

async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<UpdateScannerSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match save_scanner_settings(&state, body).await {
        Ok(settings) => response::ok(settings).into_response(),
        Err(ScannerError::BadRequest(message)) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(&translator, &message),
        ),
        Err(ScannerError::Cidr(message)) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_cidr_error(&translator, &message),
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to update scanner settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "settingsUpdateFailed"),
            )
        }
    }
}

pub(crate) async fn lookup_cidr_region(
    state: &AppState,
    province: &str,
    query_city: Option<&str>,
) -> Result<CidrRegionLookup, String> {
    let input = ScannerCidrExemptionRegionInput {
        province: province.to_string(),
        query_city: query_city.map(ToString::to_string),
    };
    let lookup = lookup_region_cidrs(state, &input)
        .await
        .map_err(|error| error.to_string())?;
    let selection = serde_json::to_value(&lookup.selection).map_err(|error| error.to_string())?;
    Ok(CidrRegionLookup {
        selection,
        cidrs: lookup.cidrs,
    })
}

async fn get_cidr_provinces(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(get_cidr_provinces_payload(&state).await, &translator)
}

async fn get_cidr_cities(
    State(state): State<AppState>,
    Query(query): Query<CidrCityQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(
        get_cidr_cities_payload(&state, &query.province, Some(&translator)).await,
        &translator,
    )
}

async fn get_cidr_selector(
    State(state): State<AppState>,
    Query(query): Query<CidrProvinceQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let result = async {
        let provinces = get_cidr_provinces_payload(&state).await?;
        let province = query
            .province
            .as_deref()
            .map(normalize_string)
            .filter(|value| !value.is_empty());
        let cities = match province {
            Some(province) => get_cidr_cities_payload(&state, &province, Some(&translator)).await?,
            None => Value::Null,
        };
        Ok(json!({ "provinces": provinces, "cities": cities }))
    }
    .await;
    cidr_response(result, &translator)
}

async fn get_cidr_cidrs(
    State(state): State<AppState>,
    Query(query): Query<CidrCityQuery>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    cidr_response(
        get_cidr_lookup_payload(
            &state,
            &query.province,
            query.city.as_deref(),
            Some(&translator),
        )
        .await,
        &translator,
    )
}

fn cidr_response(result: Result<Value, ScannerError>, translator: &Translator) -> Response {
    match result {
        Ok(payload) => response::ok(payload).into_response(),
        Err(ScannerError::BadRequest(message)) => response::error(
            StatusCode::BAD_REQUEST,
            localize_scanner_error(translator, &message),
        ),
        Err(ScannerError::Cidr(message)) => response::error(
            StatusCode::BAD_GATEWAY,
            localize_cidr_error(translator, &message),
        ),
        Err(error) => {
            tracing::warn!(%error, "CIDR route failed");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                cidr_text(translator, "serviceError"),
            )
        }
    }
}

async fn list_blacklist(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    let translator = Translator::from_state(&state).await;
    let page = parse_i64(query.page.as_deref(), 1);
    let limit = parse_i64(query.limit.as_deref(), 20);
    let search = query.search.as_deref().unwrap_or("");
    match state
        .redis
        .list_scanner_blacklist(page, limit, search)
        .await
    {
        Ok(data) => response::ok(data).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list scanner blacklist");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistLoadFailed"),
            )
        }
    }
}

async fn get_blacklist_record(State(state): State<AppState>, Path(ip): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.redis.get_scanner_blacklist_record(&ip).await {
        Ok(Some(record)) => response::ok(record).into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            scanner_text(&translator, "recordNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %ip, "failed to load scanner blacklist record");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordLoadFailed"),
            )
        }
    }
}

async fn delete_blacklist_record(
    State(state): State<AppState>,
    Path(ip): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let ips = sanitize_scanner_ips([ip]);
    match state.redis.remove_scanner_blacklist(&ips).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete scanner blacklist record");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordDeleteFailed"),
            )
        }
    }
}

async fn delete_blacklist(State(state): State<AppState>, body: Bytes) -> Response {
    let translator = Translator::from_state(&state).await;
    let ips = match parse_blacklist_delete_ips(&body) {
        Ok(ips) => ips,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_scanner_error(&translator, message),
            );
        }
    };
    if ips.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            scanner_text(&translator, "atLeastOneIpRequired"),
        );
    }
    match state.redis.remove_scanner_blacklist(&ips).await {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to delete scanner blacklist records");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                scanner_text(&translator, "blacklistRecordsDeleteFailed"),
            )
        }
    }
}

async fn load_scanner_settings(state: &AppState) -> Result<ScannerSettings, ScannerError> {
    let raw = state.redis.scanner_settings_raw().await?;
    Ok(scanner_settings_from_raw(
        raw.as_ref(),
        scanner_env_defaults(),
    ))
}

async fn save_scanner_settings(
    state: &AppState,
    body: UpdateScannerSettingsBody,
) -> Result<ScannerSettings, ScannerError> {
    let previous = load_scanner_settings(state).await?;
    let manual_cidrs = match body.cidr_exemptions {
        Some(cidrs) => validate_scanner_cidr_exemptions(cidrs)?,
        None => previous.cidr_exemptions.clone(),
    };
    let requested_regions = body
        .cidr_exemption_regions
        .map(dedupe_scanner_cidr_exemption_region_inputs);
    let previous_region_inputs = previous
        .cidr_exemption_regions
        .iter()
        .map(|item| ScannerCidrExemptionRegionInput {
            province: item.province.clone(),
            query_city: item.query_city.clone(),
        })
        .collect::<Vec<_>>();
    let reuse_region_resolution = requested_regions
        .as_ref()
        .is_none_or(|regions| scanner_cidr_region_keys_equal(regions, &previous_region_inputs));

    let (regions, region_cidrs) = if reuse_region_resolution {
        (
            previous.cidr_exemption_regions.clone(),
            previous.cidr_exemption_region_cidrs.clone(),
        )
    } else {
        let resolved =
            resolve_cidr_exemption_regions(state, requested_regions.as_deref().unwrap_or(&[]))
                .await?;
        (
            resolved
                .iter()
                .map(|item| item.selection.clone())
                .collect::<Vec<_>>(),
            normalize_scanner_cidr_exemptions_from_strings(
                resolved
                    .iter()
                    .flat_map(|item| item.cidrs.iter().cloned())
                    .collect::<Vec<_>>(),
            ),
        )
    };

    let effective_cidr_exemptions = normalize_scanner_cidr_exemptions_from_strings(
        region_cidrs
            .iter()
            .chain(manual_cidrs.iter())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let stored = json!({
        "enabled": body.enabled,
        "windowMinutes": floor_to_i64(body.window_minutes).max(1),
        "threshold": floor_to_i64(body.threshold).max(1),
        "blacklistTtlSeconds": floor_to_i64(body.blacklist_ttl_seconds).max(60),
        "commonLocationExemptEnabled": body.common_location_exempt_enabled == Some(true),
        "cidrExemptions": manual_cidrs,
        "cidrExemptionRegions": regions,
        "cidrExemptionRegionCidrs": region_cidrs,
        "cidrExemptionCidrs": effective_cidr_exemptions,
    });
    state.redis.save_scanner_settings(&stored).await?;
    load_scanner_settings(state).await
}

async fn resolve_cidr_exemption_regions(
    state: &AppState,
    regions: &[ScannerCidrExemptionRegionInput],
) -> Result<Vec<ResolvedCidrLookup>, ScannerError> {
    let mut resolved = Vec::new();
    for region in regions {
        resolved.push(lookup_region_cidrs(state, region).await?);
    }
    Ok(resolved)
}

async fn lookup_region_cidrs(
    state: &AppState,
    input: &ScannerCidrExemptionRegionInput,
) -> Result<ResolvedCidrLookup, ScannerError> {
    let province = normalize_string(&input.province);
    if province.is_empty() {
        return Err(ScannerError::BadRequest("province is required".to_string()));
    }
    let city = input
        .query_city
        .as_deref()
        .map(normalize_string)
        .filter(|value| !value.is_empty() && value != CIDR_PROVINCE_WIDE_VALUE);
    let cache_key = cidr_cache_key(&province, city.as_deref());

    let data = match state.redis.get_json_value(&cache_key).await? {
        Some(data) => data,
        None => {
            let data = fetch_cidr_data(state, &province, city.as_deref()).await?;
            state
                .redis
                .set_json_value_ex(&cache_key, &data, CIDR_SUCCESS_CACHE_TTL_SECONDS)
                .await?;
            data
        }
    };

    Ok(cidr_lookup_from_data(&province, city.as_deref(), &data))
}

async fn get_cidr_provinces_payload(state: &AppState) -> Result<Value, ScannerError> {
    let cache_key = format!("{CIDR_CACHE_PREFIX}:provinces");
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, "provinces", &[]).await?;
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(normalize_string)
                        .filter(|value| !value.is_empty())?;
                    let city_count = to_safe_i64(item.get("city_count"), 0);
                    let is_municipality = city_count <= 1;
                    Some(json!({
                        "name": name,
                        "cityCount": city_count,
                        "isMunicipality": is_municipality,
                        "hasChildren": !is_municipality,
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let options = items
        .iter()
        .map(|item| {
            let name = item.get("name").and_then(Value::as_str).unwrap_or("");
            json!({
                "label": name,
                "value": name,
                "cityCount": item.get("cityCount").and_then(Value::as_i64).unwrap_or(0),
                "isMunicipality": item.get("isMunicipality").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect::<Vec<_>>();
    let total = to_safe_i64(data.get("total"), items.len() as i64);

    Ok(json!({
        "items": items,
        "options": options,
        "total": total,
    }))
}

async fn get_cidr_cities_payload(
    state: &AppState,
    province_input: &str,
    translator: Option<&Translator>,
) -> Result<Value, ScannerError> {
    let province = normalize_required_province(province_input)?;
    let cache_key = format!(
        "{CIDR_CACHE_PREFIX}:cities:{}",
        percent_encode_uri_component(&province)
    );
    let path = format!(
        "provinces/{}/cities",
        percent_encode_uri_component(&province)
    );
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, &path, &[]).await?;
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .map(normalize_string)
                        .filter(|value| !value.is_empty())?;
                    Some(json!({
                        "name": name,
                        "ipv4Count": to_safe_i64(item.get("ipv4_count"), 0),
                        "ipv6Count": to_safe_i64(item.get("ipv6_count"), 0),
                    }))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or(province);
    let is_municipality = items.len() == 1
        && items
            .first()
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            == Some(resolved_province.as_str());
    let supports_province_wide =
        !is_municipality && !CIDR_CITY_ONLY_PROVINCES.contains(&resolved_province.as_str());

    let mut options = Vec::new();
    if supports_province_wide {
        options.push(json!({
            "label": province_wide_label(translator, &resolved_province),
            "value": CIDR_PROVINCE_WIDE_VALUE,
            "queryCity": Value::Null,
            "isProvinceWide": true,
            "isMunicipality": false,
            "ipv4Count": 0,
            "ipv6Count": 0,
        }));
    }
    for item in &items {
        let name = item.get("name").and_then(Value::as_str).unwrap_or("");
        options.push(json!({
            "label": name,
            "value": name,
            "queryCity": if is_municipality {
                Value::String(resolved_province.clone())
            } else {
                Value::String(name.to_string())
            },
            "isProvinceWide": false,
            "isMunicipality": is_municipality,
            "ipv4Count": item.get("ipv4Count").and_then(Value::as_i64).unwrap_or(0),
            "ipv6Count": item.get("ipv6Count").and_then(Value::as_i64).unwrap_or(0),
        }));
    }
    let default_value = if supports_province_wide {
        CIDR_PROVINCE_WIDE_VALUE.to_string()
    } else {
        items
            .first()
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string()
    };
    let total = cidr_cities_total(&data, items.len());

    Ok(json!({
        "province": resolved_province,
        "items": items,
        "options": options,
        "total": total,
        "isMunicipality": is_municipality,
        "supportsProvinceWide": supports_province_wide,
        "defaultValue": default_value,
    }))
}

async fn get_cidr_lookup_payload(
    state: &AppState,
    province_input: &str,
    city_input: Option<&str>,
    translator: Option<&Translator>,
) -> Result<Value, ScannerError> {
    let province = normalize_required_province(province_input)?;
    let city = city_input
        .map(normalize_string)
        .filter(|value| !value.is_empty() && value != CIDR_PROVINCE_WIDE_VALUE);
    let cache_key = cidr_cache_key(&province, city.as_deref());
    let mut query = vec![("province", province.as_str())];
    if let Some(city) = city.as_deref() {
        query.push(("city", city));
    }
    let data = get_cached_or_fetch_cidr_data(state, &cache_key, "cidrs", &query).await?;
    Ok(cidr_lookup_payload_from_data(
        &province,
        city.as_deref(),
        &data,
        translator,
    ))
}

async fn get_cached_or_fetch_cidr_data(
    state: &AppState,
    cache_key: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, ScannerError> {
    match state.redis.get_json_value(cache_key).await? {
        Some(data) => Ok(data),
        None => {
            let data = fetch_cidr_api_data(state, path, query).await?;
            state
                .redis
                .set_json_value_ex(cache_key, &data, CIDR_SUCCESS_CACHE_TTL_SECONDS)
                .await?;
            Ok(data)
        }
    }
}

async fn fetch_cidr_data(
    state: &AppState,
    province: &str,
    city: Option<&str>,
) -> Result<Value, ScannerError> {
    let mut query = vec![("province", province)];
    if let Some(city) = city {
        query.push(("city", city));
    }
    fetch_cidr_api_data(state, "cidrs", &query).await
}

async fn fetch_cidr_api_data(
    state: &AppState,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, ScannerError> {
    let base_url = resolve_cidr_api_base_url(state).await?;
    let clean_path = path.trim_start_matches('/');
    let mut url = Url::parse(&format!("{base_url}/{clean_path}"))
        .map_err(|error| ScannerError::Cidr(format!("Invalid CIDR API URL: {error}")))?;
    for (key, value) in query {
        if !value.trim().is_empty() {
            url.query_pairs_mut().append_pair(key, value.trim());
        }
    }

    let response = state
        .fallback_client
        .get(url.clone())
        .header(reqwest::header::USER_AGENT, CIDR_USER_AGENT)
        .send()
        .await
        .map_err(|error| ScannerError::Cidr(format!("CIDR upstream request failed: {error}")))?;
    let status = response.status();
    let raw_body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(ScannerError::Cidr(format!(
            "CIDR upstream request failed: HTTP {}",
            status.as_u16()
        )));
    }
    let payload: Value =
        serde_json::from_str(raw_body.trim_start_matches('\u{feff}')).map_err(|error| {
            ScannerError::Cidr(format!("CIDR upstream returned invalid JSON: {error}"))
        })?;
    if payload.get("code").and_then(Value::as_i64) != Some(0) {
        let message = payload
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("CIDR upstream returned unexpected payload");
        return Err(ScannerError::Cidr(message.to_string()));
    }
    payload
        .get("data")
        .cloned()
        .ok_or_else(|| ScannerError::Cidr("CIDR upstream response missing data".to_string()))
}

async fn resolve_cidr_api_base_url(state: &AppState) -> Result<String, ScannerError> {
    let settings = state
        .redis
        .get_json_value(IP_LOCATION_API_SETTINGS_KEY)
        .await?
        .unwrap_or_else(|| {
            json!({
                "cidr_mode": "online",
                "cidr_url": DEFAULT_CIDR_API_URL,
            })
        });
    let mode = settings
        .get("cidr_mode")
        .and_then(Value::as_str)
        .unwrap_or("online");
    let configured_url = settings
        .get("cidr_url")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let raw_url = if mode == "custom" {
        configured_url
    } else {
        DEFAULT_CIDR_API_URL
    };
    resolve_ip_location_api_base_url(raw_url)
}

fn resolve_ip_location_api_base_url(value: &str) -> Result<String, ScannerError> {
    let normalized = value.trim().trim_end_matches('/');
    let mut url = Url::parse(normalized)
        .map_err(|error| ScannerError::Cidr(format!("Invalid CIDR API URL: {error}")))?;
    let path = url.path().trim_end_matches('/').to_string();
    if path.is_empty() {
        url.set_path("/api/v1");
    } else {
        url.set_path(&path);
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn cidr_lookup_from_data(province: &str, city: Option<&str>, data: &Value) -> ResolvedCidrLookup {
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| province.to_string());
    let resolved_city = data
        .get("city")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .or_else(|| city.map(ToString::to_string));
    let ipv4 = json_string_array(data.pointer("/cidr_groups/4"));
    let ipv6 = json_string_array(data.pointer("/cidr_groups/6"));
    let is_municipality = resolved_city
        .as_deref()
        .is_some_and(|city| city == resolved_province);
    let is_province_wide = resolved_city.is_none();

    ResolvedCidrLookup {
        selection: ScannerCidrExemptionSelection {
            province: resolved_province.clone(),
            city: resolved_city.clone(),
            label: resolved_city
                .clone()
                .unwrap_or_else(|| format!("{resolved_province}全省")),
            value: resolved_city
                .clone()
                .unwrap_or_else(|| CIDR_PROVINCE_WIDE_VALUE.to_string()),
            query_city: resolved_city,
            is_province_wide,
            is_municipality,
        },
        cidrs: normalize_scanner_cidr_exemptions_from_strings(
            ipv4.into_iter().chain(ipv6).collect::<Vec<_>>(),
        ),
    }
}

fn cidr_lookup_payload_from_data(
    province: &str,
    city: Option<&str>,
    data: &Value,
    translator: Option<&Translator>,
) -> Value {
    let resolved_province = data
        .get("province")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| province.to_string());
    let resolved_city = data
        .get("city")
        .and_then(Value::as_str)
        .map(normalize_string)
        .filter(|value| !value.is_empty())
        .or_else(|| city.map(ToString::to_string));
    let ipv4 = json_array_values(data.pointer("/cidr_groups/4"));
    let ipv6 = json_array_values(data.pointer("/cidr_groups/6"));
    let ipv4_count = to_safe_i64(data.pointer("/counts/4"), ipv4.len() as i64);
    let ipv6_count = to_safe_i64(data.pointer("/counts/6"), ipv6.len() as i64);
    let is_municipality = resolved_city
        .as_deref()
        .is_some_and(|city| city == resolved_province);
    let is_province_wide = resolved_city.is_none();
    let label = resolved_city
        .clone()
        .unwrap_or_else(|| province_wide_label(translator, &resolved_province));
    let value = resolved_city
        .clone()
        .unwrap_or_else(|| CIDR_PROVINCE_WIDE_VALUE.to_string());

    json!({
        "province": resolved_province,
        "city": resolved_city,
        "selection": {
            "province": resolved_province,
            "city": resolved_city,
            "label": label,
            "value": value,
            "queryCity": resolved_city,
            "isProvinceWide": is_province_wide,
            "isMunicipality": is_municipality,
        },
        "cidrGroups": {
            "ipv4": ipv4,
            "ipv6": ipv6,
        },
        "counts": {
            "ipv4": ipv4_count,
            "ipv6": ipv6_count,
        },
        "totalCount": ipv4_count + ipv6_count,
    })
}

fn province_wide_label(translator: Option<&Translator>, province: &str) -> String {
    translator.map_or_else(
        || format!("{province}全省"),
        |translator| {
            cidr_text_params(
                translator,
                "provinceWideLabel",
                &[("province", province.to_string())],
            )
        },
    )
}

fn cidr_cities_total(data: &Value, item_count: usize) -> i64 {
    to_safe_i64(data.get("total"), item_count as i64)
}

fn scanner_settings_from_raw(raw: Option<&Value>, defaults: ScannerEnvDefaults) -> ScannerSettings {
    let mut enabled = defaults.enabled;
    let mut window_minutes = defaults.window_minutes;
    let mut threshold = defaults.threshold;
    let mut blacklist_ttl_seconds = defaults.blacklist_ttl_seconds;
    let mut common_location_exempt_enabled = false;
    let mut cidr_exemptions = Vec::new();
    let mut cidr_exemption_regions = Vec::new();
    let mut cidr_exemption_region_cidrs = Vec::new();
    let mut cidr_exemption_cidrs = Vec::new();

    if let Some(raw) = raw {
        if let Some(value) = raw.get("enabled").and_then(Value::as_bool) {
            enabled = value;
        }
        if let Some(value) = positive_i64(raw.get("windowMinutes")) {
            window_minutes = value;
        }
        if let Some(value) = positive_i64(raw.get("threshold")) {
            threshold = value;
        }
        if let Some(value) = positive_i64(raw.get("blacklistTtlSeconds")) {
            blacklist_ttl_seconds = value;
        }
        if let Some(value) = raw
            .get("commonLocationExemptEnabled")
            .and_then(Value::as_bool)
        {
            common_location_exempt_enabled = value;
        }
        cidr_exemptions = normalize_scanner_cidr_exemptions(raw.get("cidrExemptions"));
        cidr_exemption_regions =
            normalize_scanner_cidr_exemption_regions(raw.get("cidrExemptionRegions"));
        cidr_exemption_region_cidrs =
            normalize_scanner_cidr_exemptions(raw.get("cidrExemptionRegionCidrs"));
        cidr_exemption_cidrs = normalize_scanner_cidr_exemptions(raw.get("cidrExemptionCidrs"));
    }

    let effective_cidr_exemptions = if cidr_exemption_cidrs.is_empty() {
        normalize_scanner_cidr_exemptions_from_strings(
            cidr_exemption_region_cidrs
                .iter()
                .chain(cidr_exemptions.iter())
                .cloned()
                .collect::<Vec<_>>(),
        )
    } else {
        cidr_exemption_cidrs.clone()
    };

    ScannerSettings {
        enabled,
        window_minutes,
        threshold,
        window_seconds: SCANNER_BASE_WINDOW_SECONDS.max(window_minutes * 60),
        blacklist_ttl_seconds,
        common_location_exempt_enabled,
        cidr_exemptions,
        cidr_exemption_regions,
        cidr_exemption_region_cidrs,
        cidr_exemption_cidrs: effective_cidr_exemptions,
    }
}

fn scanner_env_defaults() -> ScannerEnvDefaults {
    let enabled_raw = env::var("SCANNER_ENABLED")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    ScannerEnvDefaults {
        enabled: enabled_raw == "true" || enabled_raw == "1",
        window_minutes: env_i64("SCANNER_WINDOW_MINUTES", 5),
        threshold: env_i64("SCANNER_THRESHOLD", 5),
        blacklist_ttl_seconds: env_i64("SCANNER_BLACKLIST_TTL_DAYS", 90) * 24 * 3600,
    }
}

fn parse_blacklist_delete_ips(body: &[u8]) -> Result<Vec<String>, &'static str> {
    let parsed = parse_json_body(body)?;
    if let Some(array) = parsed.as_array() {
        return Ok(array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect());
    }
    if let Some(array) = parsed.get("ips").and_then(Value::as_array) {
        return Ok(array
            .iter()
            .filter_map(Value::as_str)
            .map(ToString::to_string)
            .collect());
    }
    Ok(Vec::new())
}

fn parse_json_body(body: &[u8]) -> Result<Value, &'static str> {
    if body.is_empty() {
        return Ok(Value::Null);
    }
    let parsed = serde_json::from_slice::<Value>(body).map_err(|_| "Invalid request body")?;
    if let Some(inner) = parsed.as_str() {
        return serde_json::from_str(inner).map_err(|_| "Invalid request body");
    }
    Ok(parsed)
}

fn sanitize_scanner_ips(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() || !seen.insert(normalized.to_string()) {
            continue;
        }
        result.push(normalized.to_string());
    }
    result
}

fn validate_scanner_cidr_exemptions(values: Vec<String>) -> Result<Vec<String>, ScannerError> {
    let normalized = normalize_scanner_cidr_exemptions_from_strings(values);
    let invalid = normalized
        .iter()
        .filter(|cidr| !is_valid_cidr(cidr))
        .cloned()
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        return Err(ScannerError::BadRequest(format!(
            "Invalid CIDR exemptions: {}",
            invalid.join(", ")
        )));
    }
    Ok(normalized)
}

fn normalize_scanner_cidr_exemptions(value: Option<&Value>) -> Vec<String> {
    let values = value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    item.as_str()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| item.to_string())
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    normalize_scanner_cidr_exemptions_from_strings(values)
        .into_iter()
        .filter(|cidr| is_valid_cidr(cidr))
        .collect()
}

fn normalize_scanner_cidr_exemptions_from_strings(values: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let normalized = value.trim();
        if normalized.is_empty() {
            continue;
        }
        let key = normalized.to_ascii_lowercase();
        if seen.insert(key) {
            result.push(normalized.to_string());
        }
    }
    result
}

fn is_valid_cidr(value: &str) -> bool {
    let normalized = value.trim();
    let Some((address, prefix_raw)) = normalized.split_once('/') else {
        return false;
    };
    if address.trim().is_empty()
        || prefix_raw.trim().is_empty()
        || prefix_raw.trim().chars().any(|ch| !ch.is_ascii_digit())
    {
        return false;
    }
    let Ok(prefix) = prefix_raw.trim().parse::<u16>() else {
        return false;
    };
    match address.trim().parse::<IpAddr>() {
        Ok(IpAddr::V4(_)) => prefix <= 32,
        Ok(IpAddr::V6(_)) => prefix <= 128,
        Err(_) => false,
    }
}

fn normalize_scanner_cidr_exemption_regions(
    value: Option<&Value>,
) -> Vec<ScannerCidrExemptionSelection> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(normalize_scanner_cidr_exemption_selection)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn normalize_scanner_cidr_exemption_selection(
    value: &Value,
) -> Option<ScannerCidrExemptionSelection> {
    let province = normalize_string(value.get("province")?.as_str()?);
    let label = normalize_string(value.get("label")?.as_str()?);
    let value_label = normalize_string(value.get("value")?.as_str()?);
    if province.is_empty() || label.is_empty() || value_label.is_empty() {
        return None;
    }
    Some(ScannerCidrExemptionSelection {
        province,
        city: value
            .get("city")
            .and_then(Value::as_str)
            .map(normalize_string)
            .filter(|value| !value.is_empty()),
        label,
        value: value_label,
        query_city: value
            .get("query_city")
            .and_then(Value::as_str)
            .map(normalize_string)
            .filter(|value| !value.is_empty()),
        is_province_wide: value
            .get("is_province_wide")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        is_municipality: value
            .get("is_municipality")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    })
}

fn dedupe_scanner_cidr_exemption_region_inputs(
    values: Vec<ScannerCidrExemptionRegionBody>,
) -> Vec<ScannerCidrExemptionRegionInput> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for value in values {
        let province = normalize_string(&value.province);
        if province.is_empty() {
            continue;
        }
        let query_city = value
            .query_city
            .as_deref()
            .map(normalize_string)
            .filter(|value| !value.is_empty());
        let key = scanner_cidr_region_key(&province, query_city.as_deref());
        if seen.insert(key) {
            result.push(ScannerCidrExemptionRegionInput {
                province,
                query_city,
            });
        }
    }
    result
}

fn scanner_cidr_region_keys_equal(
    left: &[ScannerCidrExemptionRegionInput],
    right: &[ScannerCidrExemptionRegionInput],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            scanner_cidr_region_key(&left.province, left.query_city.as_deref())
                == scanner_cidr_region_key(&right.province, right.query_city.as_deref())
        })
}

fn scanner_cidr_region_key(province: &str, query_city: Option<&str>) -> String {
    format!("{}::{}", province.trim(), query_city.unwrap_or("").trim())
}

fn cidr_cache_key(province: &str, city: Option<&str>) -> String {
    let province = percent_encode_uri_component(province);
    match city {
        Some(city) => format!(
            "{CIDR_CACHE_PREFIX}:cidrs:{province}:{}",
            percent_encode_uri_component(city)
        ),
        None => format!("{CIDR_CACHE_PREFIX}:cidrs:{province}"),
    }
}

fn percent_encode_uri_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn json_string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn json_array_values(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

fn positive_i64(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    let parsed = value
        .as_i64()
        .or_else(|| value.as_f64().map(floor_to_i64))
        .or_else(|| value.as_str().and_then(|text| text.trim().parse().ok()))?;
    (parsed > 0).then_some(parsed)
}

fn floor_to_i64(value: f64) -> i64 {
    if value.is_finite() {
        value.floor() as i64
    } else {
        0
    }
}

fn parse_i64(value: Option<&str>, fallback: i64) -> i64 {
    value
        .and_then(|value| parse_i64_prefix(value.trim_start()))
        .unwrap_or(fallback)
}

fn parse_i64_prefix(value: &str) -> Option<i64> {
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
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<i64>().ok().map(|value| value * sign)
}

fn normalize_required_province(value: &str) -> Result<String, ScannerError> {
    let normalized = normalize_string(value);
    if normalized.is_empty() {
        return Err(ScannerError::BadRequest("province is required".to_string()));
    }
    Ok(normalized)
}

fn to_safe_i64(value: Option<&Value>, fallback: i64) -> i64 {
    let parsed = value.and_then(js_number_like_i64_floor).unwrap_or(fallback);
    parsed.max(0)
}

fn js_number_like_i64_floor(value: &Value) -> Option<i64> {
    let parsed = match value {
        Value::Null => 0.0,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(value) => value.as_f64()?,
        Value::String(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                0.0
            } else {
                trimmed.parse::<f64>().ok()?
            }
        }
        Value::Array(items) => match items.as_slice() {
            [] => 0.0,
            [item] => {
                let text = match item {
                    Value::Null => String::new(),
                    Value::Bool(value) => value.to_string(),
                    Value::Number(value) => value.to_string(),
                    Value::String(value) => value.clone(),
                    Value::Array(_) | Value::Object(_) => return None,
                };
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    0.0
                } else {
                    trimmed.parse::<f64>().ok()?
                }
            }
            _ => return None,
        },
        Value::Object(_) => return None,
    };
    parsed.is_finite().then(|| floor_to_i64(parsed))
}

fn env_i64(name: &str, fallback: i64) -> i64 {
    env::var(name)
        .ok()
        .and_then(|value| value.trim().parse::<i64>().ok())
        .unwrap_or(fallback)
}

fn normalize_string(value: &str) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ScannerEnvDefaults {
        ScannerEnvDefaults {
            enabled: false,
            window_minutes: 5,
            threshold: 5,
            blacklist_ttl_seconds: 90 * 24 * 3600,
        }
    }

    #[test]
    fn scanner_settings_preserve_node_defaults_and_effective_cidrs() {
        let raw = json!({
            "enabled": true,
            "windowMinutes": 2,
            "threshold": 3,
            "blacklistTtlSeconds": 120,
            "commonLocationExemptEnabled": true,
            "cidrExemptions": [" 10.0.0.0/8 ", "10.0.0.0/8", "bad"],
            "cidrExemptionRegions": [{
                "province": "广东",
                "city": null,
                "label": "广东全省",
                "value": "__province_all__",
                "query_city": null,
                "is_province_wide": true,
                "is_municipality": false
            }],
            "cidrExemptionRegionCidrs": ["1.1.1.0/24"]
        });

        let settings = scanner_settings_from_raw(Some(&raw), defaults());

        assert!(settings.enabled);
        assert_eq!(settings.window_seconds, SCANNER_BASE_WINDOW_SECONDS);
        assert_eq!(settings.cidr_exemptions, vec!["10.0.0.0/8"]);
        assert_eq!(
            settings.cidr_exemption_cidrs,
            vec!["1.1.1.0/24", "10.0.0.0/8"]
        );
    }

    #[test]
    fn validates_cidr_exemptions_without_canonicalizing_values() {
        assert_eq!(
            validate_scanner_cidr_exemptions(vec![
                " 2001:DB8::/32 ".to_string(),
                "2001:db8::/32".to_string(),
                "192.168.0.0/16".to_string(),
            ])
            .unwrap(),
            vec!["2001:DB8::/32", "192.168.0.0/16"]
        );
        assert!(validate_scanner_cidr_exemptions(vec!["10.0.0.0/33".to_string()]).is_err());
    }

    #[test]
    fn parses_blacklist_delete_body_shapes_like_node() {
        assert_eq!(
            parse_blacklist_delete_ips(br#"["1.1.1.1", 2, " 2.2.2.2 "]"#).unwrap(),
            vec!["1.1.1.1", " 2.2.2.2 "]
        );
        assert_eq!(
            parse_blacklist_delete_ips(br#"{"ips":["3.3.3.3","3.3.3.3"]}"#).unwrap(),
            vec!["3.3.3.3", "3.3.3.3"]
        );
        assert_eq!(
            parse_blacklist_delete_ips(br#"["   "]"#).unwrap(),
            vec!["   "]
        );
        assert!(parse_blacklist_delete_ips(br#""not-json""#).is_err());
    }

    #[test]
    fn scanner_query_int_parser_matches_node_parse_int_edges() {
        assert_eq!(parse_i64(None, 20), 20);
        assert_eq!(parse_i64(Some(""), 20), 20);
        assert_eq!(parse_i64(Some("   "), 20), 20);
        assert_eq!(parse_i64(Some("2x"), 20), 2);
        assert_eq!(parse_i64(Some("  +3.9"), 20), 3);
        assert_eq!(parse_i64(Some("-1"), 20), -1);
    }

    #[test]
    fn subsonic_rest_endpoint_parser_matches_node_route_regex() {
        assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.view"), "ping");
        assert_eq!(
            normalize_subsonic_rest_endpoint("/rest/getLicense.json"),
            "getlicense"
        );
        assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.xml"), "ping");
        assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping"), "ping");
        assert_eq!(normalize_subsonic_rest_endpoint("/rest/ping.bad"), "");
        assert_eq!(normalize_subsonic_rest_endpoint("/rest/foo/bar"), "");
        assert_eq!(
            normalize_subsonic_rest_endpoint("/rest/ping.view/extra"),
            ""
        );
    }

    #[test]
    fn scanner_local_address_detection_matches_node_regex_edges() {
        assert!(is_scanner_local_address("10.999.999.999"));
        assert!(is_scanner_local_address("10.0.0.1:7999"));
        assert!(!is_scanner_local_address("10.0.0.1:bad"));
        assert!(is_scanner_local_address("127.999.999.999"));
        assert!(is_scanner_local_address("192.168.999.999"));
        assert!(is_scanner_local_address("172.16.999.999"));
        assert!(is_scanner_local_address("172.31.999.999"));
        assert!(!is_scanner_local_address("172.32.0.1"));
        assert!(!is_scanner_local_address("172.016.0.1"));
        assert!(is_scanner_local_address("::ffff:10.999.999.999"));
        assert!(!is_scanner_local_address("::ffff:10.0.0.1:bad"));
    }

    #[test]
    fn scanner_host_normalization_preserves_node_fallback_port_rules() {
        assert_eq!(normalize_scanner_host("Example.COM:7999"), "example.com");
        assert_eq!(normalize_scanner_host("foo:bar"), "foo:bar");
        assert_eq!(normalize_scanner_host("foo:123abc"), "foo:123abc");
        assert_eq!(normalize_scanner_host("http://foo.example/path"), "http");
        assert_eq!(normalize_scanner_host("2001:db8::1"), "2001:db8:");
    }

    #[test]
    fn dedupes_region_inputs_by_province_and_query_city() {
        let regions = dedupe_scanner_cidr_exemption_region_inputs(vec![
            ScannerCidrExemptionRegionBody {
                province: " 广东 ".to_string(),
                query_city: None,
            },
            ScannerCidrExemptionRegionBody {
                province: "广东".to_string(),
                query_city: Some("".to_string()),
            },
            ScannerCidrExemptionRegionBody {
                province: "广东".to_string(),
                query_city: Some("深圳".to_string()),
            },
        ]);

        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].province, "广东");
        assert_eq!(regions[0].query_city, None);
        assert_eq!(regions[1].query_city.as_deref(), Some("深圳"));
    }

    #[test]
    fn builds_public_cidr_lookup_payload_with_camel_case_fields() {
        let payload = cidr_lookup_payload_from_data(
            "广东",
            Some("深圳"),
            &json!({
                "province": "广东",
                "city": "深圳",
                "cidr_groups": {
                    "4": ["1.1.1.0/24"],
                    "6": ["2001:db8::/32"]
                },
                "counts": {
                    "4": 10,
                    "6": 1
                }
            }),
            None,
        );

        assert_eq!(payload["selection"]["queryCity"], "深圳");
        assert_eq!(payload["selection"]["isProvinceWide"], false);
        assert_eq!(payload["cidrGroups"]["ipv4"][0], "1.1.1.0/24");
        assert_eq!(payload["counts"]["ipv4"], 10);
        assert_eq!(payload["totalCount"], 11);
    }

    #[test]
    fn cidr_safe_int_matches_node_number_coercion_edges() {
        assert_eq!(to_safe_i64(None, 9), 9);
        assert_eq!(to_safe_i64(Some(&json!("7.9")), 9), 7);
        assert_eq!(to_safe_i64(Some(&json!("")), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(null)), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(true)), 9), 1);
        assert_eq!(to_safe_i64(Some(&json!(false)), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(-3)), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!("bad")), 9), 9);
        assert_eq!(to_safe_i64(Some(&json!(["4.2"])), 9), 4);
        assert_eq!(to_safe_i64(Some(&json!([])), 9), 0);
        assert_eq!(to_safe_i64(Some(&json!(["1", "2"])), 9), 9);
    }

    #[test]
    fn cidr_cities_total_fallback_excludes_province_wide_option_like_node() {
        assert_eq!(cidr_cities_total(&json!({}), 2), 2);
        assert_eq!(cidr_cities_total(&json!({ "total": "7.9" }), 2), 7);
    }

    #[test]
    fn public_cidr_payload_localizes_province_wide_label_like_node() {
        let en = Translator::new("en-US");
        let payload = cidr_lookup_payload_from_data(
            "Guangdong",
            None,
            &json!({
                "province": "Guangdong",
                "cidr_groups": {
                    "4": [],
                    "6": []
                }
            }),
            Some(&en),
        );

        assert_eq!(province_wide_label(Some(&en), "Guangdong"), "All Guangdong");
        assert_eq!(payload["selection"]["label"], "All Guangdong");
        assert_eq!(payload["selection"]["value"], CIDR_PROVINCE_WIDE_VALUE);
    }

    #[test]
    fn public_cidr_payload_preserves_upstream_cidr_arrays_like_node() {
        let payload = cidr_lookup_payload_from_data(
            "广东",
            Some("深圳"),
            &json!({
                "province": "广东",
                "city": "深圳",
                "cidr_groups": {
                    "4": ["1.1.1.0/24", 123, null],
                    "6": []
                }
            }),
            None,
        );

        assert_eq!(payload["cidrGroups"]["ipv4"][0], "1.1.1.0/24");
        assert_eq!(payload["cidrGroups"]["ipv4"][1], 123);
        assert_eq!(payload["cidrGroups"]["ipv4"][2], Value::Null);
        assert_eq!(payload["counts"]["ipv4"], 3);
        assert_eq!(payload["totalCount"], 3);
    }

    #[test]
    fn resolves_cidr_api_base_url_like_node_helper() {
        assert_eq!(
            resolve_ip_location_api_base_url("https://example.test").unwrap(),
            "https://example.test/api/v1"
        );
        assert_eq!(
            resolve_ip_location_api_base_url("https://example.test/custom/").unwrap(),
            "https://example.test/custom"
        );
    }

    #[test]
    fn localizes_scanner_and_cidr_route_errors() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            localize_scanner_error(&translator, "Invalid request body"),
            "请求体不正确"
        );
        assert_eq!(
            localize_scanner_error(&translator, "At least one IP is required"),
            "请至少提供一个 IP"
        );
        assert_eq!(
            localize_scanner_error(&translator, "Invalid CIDR exemptions: 10.0.0.0/33"),
            "CIDR 豁免格式不正确：10.0.0.0/33"
        );
        assert_eq!(
            localize_cidr_error(&translator, "CIDR upstream request failed: HTTP 502"),
            "CIDR 上游请求失败 (502)"
        );
        assert_eq!(
            localize_cidr_error(&translator, "CIDR upstream response missing data"),
            "CIDR 上游返回异常"
        );
    }
}
