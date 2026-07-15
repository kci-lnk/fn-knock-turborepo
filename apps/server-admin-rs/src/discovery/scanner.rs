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
    cidr::{CidrError, CidrOperator, CidrRegionQuery, CidrSelection},
    common_auth_locations, http_utils,
    i18n::Translator,
    ip_location, response,
    state::AppState,
    system_events, time_utils,
};

const SCANNER_BASE_WINDOW_SECONDS: i64 = 5 * 60;
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
enum ScannerError {
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
}

#[derive(Clone, Copy)]
struct ScannerEnvDefaults {
    enabled: bool,
    window_minutes: i64,
    threshold: i64,
    blacklist_ttl_seconds: i64,
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
        .route("/api/admin/cidr/capabilities", get(get_cidr_capabilities))
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
mod handlers;
mod preflight;
mod settings;
mod utils;

use cidr_routes::{
    get_cidr_capabilities, get_cidr_cidrs, get_cidr_cities, get_cidr_provinces, get_cidr_selector,
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
use preflight::{
    is_scanner_local_address, normalize_scanner_host, normalize_subsonic_rest_endpoint,
};
#[cfg(test)]
use settings::scanner_settings_from_raw;

#[cfg(test)]
mod tests;
