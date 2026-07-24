use std::{
    future::Future,
    io::{self, Write},
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use axum::{
    Router,
    body::Body,
    extract::{Json, State, rejection::JsonRejection},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, write::DeflateEncoder};
use serde_json::{Value, json};
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
    process::Command,
};
use uuid::Uuid;

use crate::{
    app_version::{APP_BACKUP_IMPORT_MIN_VERSION, APP_BACKUP_SCHEMA_VERSION, APP_LOCAL_VERSION},
    i18n::Translator,
    proxy_config, response, runtime_config, ssl,
    state::AppState,
    store::node_locale_compare_ordering,
    system_monitor, time_utils, waf,
};

const KNOCK_BACKUP_PREFIX: &str = "fn_knock:";
const KNOCK_BACKUP_EXTENSION: &str = ".knock";
const KNOCK_BACKUP_JSON_FILENAME: &str = "fn-knock-backup.json";
const KNOCK_BACKUP_PASSWORD: &str = "890eced0-4561-4044-8d6b-def83b5c6016";
const OPENWRT_APK_COMMAND: &str = "apk";
const OPENWRT_OPKG_COMMAND: &str = "opkg";
const DEBIAN_APT_GET_PATH: &str = "/usr/bin/apt-get";
const BACKUP_DIRECTORY_NAME: &str = "backup";
const AUTOMATIC_BACKUP_CONFIG_KEY: &str = "fn_knock:config:backup:automatic";
const AUTOMATIC_BACKUP_RUNTIME_KEY: &str = "fn_knock:config:backup:automatic:runtime";
const AUTOMATIC_BACKUP_DIRECTORY: [&str; 2] = ["backups", "automatic"];
const AUTOMATIC_BACKUP_DEFAULT_INTERVAL_HOURS: i64 = 24;
const AUTOMATIC_BACKUP_DEFAULT_RETENTION_DAYS: i64 = 7;
const AUTOMATIC_BACKUP_MIN_INTERVAL_HOURS: i64 = 1;
const AUTOMATIC_BACKUP_MAX_INTERVAL_HOURS: i64 = 24 * 365;
const AUTOMATIC_BACKUP_MIN_RETENTION_DAYS: i64 = 1;
const AUTOMATIC_BACKUP_MAX_RETENTION_DAYS: i64 = 3650;
const AUTOMATIC_BACKUP_RECHECK_SECONDS: u64 = 60;
const MAX_BACKUP_DIRECTORY_SCAN_DEPTH: usize = 5;
const MAX_BACKUP_DIRECTORY_FILES: usize = 500;
const MAX_BACKUP_ARCHIVE_SIZE: usize = 128 * 1024 * 1024;
const SCAN_COUNT: usize = 200;
const MAINTENANCE_BACKUP_ERROR_MARKER: &str = "__maintenance_backup_error";

static ARCHIVE_COMMANDS_READY: AtomicBool = AtomicBool::new(false);

const BACKUP_EXCLUDED_KEY_PREFIXES: &[&str] = &[
    "fn_knock:acme:job:",
    "fn_knock:acme:logs:",
    "fn_knock:auth_log_data:",
    "fn_knock:auth_logs:",
    "fn_knock:auth_mobility:",
    // Per-host temporary grants are revocable runtime credentials, never
    // backup material.  Keeping this prefix excluded also prevents an
    // imported archive from resurrecting a grant issued before restore.
    "fn_knock:auth:subdomain_rule_grant:",
    // The per-host expiry index contains runtime credential metadata and may
    // otherwise restore orphan members without their excluded grant records.
    "fn_knock:auth:subdomain_rule_grant_active:",
    // A short-lived earlier implementation wrote cookie-capability issue
    // slots. They are obsolete runtime tombstones and must not survive backup.
    "fn_knock:auth:subdomain_rule_issue_slot:",
    // Sliding-window issuance counters are runtime state as well.
    "fn_knock:auth:subdomain_rule_rate:",
    "fn_knock:backoff:",
    "fn_knock:cidr:",
    "fn_knock:cloudflared:logs",
    "fn_knock:common_auth_locations:runtime",
    "fn_knock:config:backup:",
    "fn_knock:docker_admin:login_backoff:",
    "fn_knock:docker_admin:session:",
    "fn_knock:errors:",
    "fn_knock:events:",
    "fn_knock:fnos-share:session:",
    "fn_knock:fnos-share:validation:",
    "fn_knock:gateway:",
    "fn_knock:ip_location:",
    "fn_knock:lock:",
    "fn_knock:login_backoff:",
    "fn_knock:nonce:",
    "fn_knock:notifications:deliveries:",
    "fn_knock:notifications:runtime:",
    "fn_knock:notifications:triggers:",
    "fn_knock:oidc:invite:",
    "fn_knock:oidc:login_error:",
    "fn_knock:oidc:state:",
    "fn_knock:passkey:bind:",
    "fn_knock:passkey:challenge:",
    "fn_knock:recent_auth_ips:",
    "fn_knock:reverse-proxy:",
    "fn_knock:scanner:blacklist:",
    "fn_knock:scanner:suspicious:",
    "fn_knock:session:",
    "fn_knock:smart-connect:runtime",
    "fn_knock:ssh_security:",
    "fn_knock:terminal:",
    "fn_knock:traffic:",
    "fn_knock:tunnel:runtime",
    "fn_knock:ui:",
    "fn_knock:update:",
    "fn_knock:waf:log:",
    "fn_knock:waf:logs:",
    "fn_knock:waf:stats:",
    "fn_knock:welcome-guide:",
];

pub fn maintenance_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/maintenance/backup/export", get(export_backup))
        .route(
            "/api/admin/maintenance/backup/automatic",
            get(get_automatic_backup_details).put(update_automatic_backup_config),
        )
        .route(
            "/api/admin/maintenance/backup/automatic/files",
            get(list_automatic_backup_files),
        )
        .route(
            "/api/admin/maintenance/backup/files",
            get(list_backup_files),
        )
        .route(
            "/api/admin/maintenance/backup/export/fnos",
            post(export_backup_to_directory),
        )
        .route("/api/admin/maintenance/backup/import", post(import_backup))
        .route(
            "/api/admin/maintenance/backup/import/automatic",
            post(import_backup_from_automatic_directory),
        )
        .route(
            "/api/admin/maintenance/backup/import/fnos",
            post(import_backup_from_directory),
        )
        .route("/api/admin/maintenance/data/clear", post(clear_all_data))
}

pub fn start_automatic_backup_tasks(state: AppState) {
    spawn_automatic_backup_task(state);
}

#[derive(serde::Deserialize)]
struct ImportBackupBody {
    filename: Option<String>,
    archive_base64: String,
}

#[derive(serde::Deserialize)]
struct ImportBackupFromDirectoryBody {
    path: String,
}

#[derive(serde::Deserialize)]
struct UpdateAutomaticBackupBody {
    enabled: bool,
    interval_hours: i64,
    retention_days: i64,
}

#[derive(serde::Deserialize)]
struct ClearAllDataBody {
    confirmation: String,
}

struct BackupArchive {
    buffer: Vec<u8>,
    exported_at: String,
    filename: String,
}

#[derive(Debug)]
struct BackupImportError {
    status: StatusCode,
    message: String,
}

impl BackupImportError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

mod automatic;
mod commands;
mod directory;
mod export;
mod i18n;
mod import;
mod paths;
mod routes;
mod sync;
mod zip;

use automatic::*;
use commands::*;
use directory::*;
use export::*;
use i18n::*;
use import::*;
use paths::*;
use routes::*;
use sync::*;
use zip::*;

#[cfg(test)]
mod tests;
