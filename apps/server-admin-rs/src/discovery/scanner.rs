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
    Storage(#[from] crate::storage::StorageError),
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

mod cidr_routes;
mod cidr_service;
mod handlers;
mod preflight;
mod settings;
mod utils;

pub(crate) use cidr_routes::lookup_cidr_region;
use cidr_routes::{get_cidr_cidrs, get_cidr_cities, get_cidr_provinces, get_cidr_selector};
use cidr_service::{
    get_cidr_cities_payload, get_cidr_lookup_payload, get_cidr_provinces_payload,
    lookup_region_cidrs, resolve_cidr_exemption_regions,
};
use handlers::{
    delete_blacklist, delete_blacklist_record, get_blacklist_record, get_settings, list_blacklist,
    update_settings,
};
pub(crate) use preflight::{
    is_blacklisted_for_preflight, is_common_path_for_preflight, is_request_exempt_from_scan,
    record_uncommon_path_for_preflight,
};
use settings::{load_scanner_settings, save_scanner_settings};
use utils::*;

#[cfg(test)]
use cidr_service::{
    cidr_cities_total, cidr_lookup_payload_from_data, province_wide_label,
    resolve_ip_location_api_base_url,
};
#[cfg(test)]
use preflight::{
    is_scanner_local_address, normalize_scanner_host, normalize_subsonic_rest_endpoint,
};
#[cfg(test)]
use settings::scanner_settings_from_raw;

#[cfg(test)]
mod tests;
