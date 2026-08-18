use std::{
    collections::{BTreeSet, HashSet},
    env,
    net::IpAddr,
};

use axum::{
    Json,
    body::Bytes,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use url::Url;
use utoipa_axum::router::OpenApiRouter;

use crate::{
    cidr::{CidrError, CidrOperator, CidrRegionQuery, CidrSelection},
    common_auth_locations, http_utils,
    i18n::Translator,
    ip_location, response,
    state::AppState,
    system_events, time_utils,
};

const SCANNER_BASE_WINDOW_SECONDS: i64 = 5 * 60;
const SCANNER_EXEMPT_IPSET_KEY: &str = "scanner_exemptions";
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

fn localize_scanner_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    match message {
        "Invalid request body" => return scanner_text(translator, "invalidRequestBody"),
        "At least one IP is required" => return scanner_text(translator, "atLeastOneIpRequired"),
        "Record not found" => return scanner_text(translator, "recordNotFound"),
        "Invalid scanner path whitelist" => {
            return scanner_text(translator, "pathWhitelistInvalid");
        }
        "Path must not be empty" => return scanner_text(translator, "pathRequired"),
        "Path must be absolute" => return scanner_text(translator, "pathMustBeAbsolute"),
        "Path contains control characters" => {
            return scanner_text(translator, "pathContainsControlCharacters");
        }
        "IP is required" => return scanner_text(translator, "ipRequired"),
        "province is required" => return cidr_text(translator, "provinceRequired"),
        "CIDR operator filtering is unsupported" => {
            return cidr_text(translator, "operatorUnsupported");
        }
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
    crate::cidr::localize_error(translator, message)
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ScannerError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Cidr(String),
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
}

impl From<CidrError> for ScannerError {
    fn from(value: CidrError) -> Self {
        match value {
            CidrError::BadRequest(message) => Self::BadRequest(message),
            CidrError::Service(message) => Self::Cidr(message),
            CidrError::Storage(error) => Self::Storage(error),
        }
    }
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
    operator: Option<String>,
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

#[derive(Deserialize)]
struct UpdateScannerPathWhitelistBody {
    paths: Vec<String>,
}

#[derive(Deserialize)]
struct ScannerFalsePositiveBody {
    ip: String,
    path: String,
}

#[derive(Clone, Debug, Deserialize)]
struct ScannerCidrExemptionRegionBody {
    province: String,
    #[serde(default)]
    query_city: Option<String>,
    #[serde(default)]
    operator: Option<Value>,
}

type ScannerCidrExemptionSelection = CidrSelection;

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
    #[serde(
        rename = "cidrExemptionPolicyId",
        skip_serializing_if = "Option::is_none"
    )]
    cidr_exemption_policy_id: Option<String>,
    #[serde(rename = "cidrExemptionSourceCidrCount")]
    cidr_exemption_source_cidr_count: usize,
    #[serde(rename = "cidrExemptionRangeCount")]
    cidr_exemption_range_count: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ScannerPathWhitelist {
    paths: Vec<String>,
    default_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ScannerFalsePositiveResult {
    ip: String,
    path: String,
    added: bool,
    unblocked: bool,
}

pub(crate) struct ScannerPreflightPolicy {
    settings: ScannerSettings,
    path_whitelist: HashSet<String>,
    client_ip: String,
    ip_exempt: bool,
}

#[derive(Clone, Copy)]
struct ScannerEnvDefaults {
    enabled: bool,
    window_minutes: i64,
    threshold: i64,
    blacklist_ttl_seconds: i64,
}

pub fn scanner_routes() -> OpenApiRouter<AppState> {
    handlers::routes()
}

pub fn cidr_routes() -> OpenApiRouter<AppState> {
    cidr_routes::routes()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScannerPreflightRecordResult {
    pub hit_count: i64,
    pub blocked: bool,
}

mod cidr_routes;
mod handlers;
mod path_whitelist;
mod preflight;
mod settings;
mod utils;

use path_whitelist::*;
pub(crate) use preflight::{
    is_blacklisted_for_preflight, is_common_path_for_preflight, is_request_exempt_from_scan,
    load_preflight_policy, record_uncommon_path_for_preflight,
};
pub(crate) use settings::migrate_scanner_cidr_ipset_on_boot;
use settings::{load_scanner_settings, save_scanner_settings};
use utils::*;

#[cfg(test)]
use preflight::{
    is_scanner_local_address, normalize_scanner_host, normalize_subsonic_rest_endpoint,
};
#[cfg(test)]
use settings::scanner_settings_from_raw;

#[cfg(test)]
mod tests;
