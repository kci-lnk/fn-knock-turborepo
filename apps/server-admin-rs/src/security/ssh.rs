use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{BufRead, BufReader, Read},
    net::IpAddr,
    path::Path,
    process::Command,
    time::Duration,
};

use axum::{
    Json, Router,
    body::Bytes,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use flate2::read::GzDecoder;
use ipnet::IpNet;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use tokio::time as tokio_time;
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{
    cidr::{CidrOperator, CidrRegionQuery, CompiledIpSet, compile_ip_set},
    http_utils::{is_private_or_local_ip, normalize_ip},
    i18n::Translator,
    ip_location, response, runtime_profile,
    state::AppState,
    system_events, time_utils,
};

const RUNTIME_KEY: &str = "fn_knock:ssh_security:runtime";
const SSH_ALLOWED_IPSET_KEY: &str = "ssh_allowed";
const BLOCKS_INDEX_KEY: &str = "fn_knock:ssh_security:blocks:index";
const BLOCK_DATA_PREFIX: &str = "fn_knock:ssh_security:blocks:data:";
const FAILURES_PREFIX: &str = "fn_knock:ssh_security:failures:";
const PROCESSED_PREFIX: &str = "fn_knock:ssh_security:processed:";
const SSH_FIREWALL_CHAIN: &str = "FN-KNOCK-SSH";
const PROCESSED_TTL_SECONDS: i64 = 7 * 24 * 3600;
const STARTUP_BACKFILL_LOG_LIMIT: usize = 2000;
const SUCCESS_LOG_COALESCE_WINDOW_MS: i64 = 30 * 1000;
const SSH_SECURITY_TICK_SECONDS: u64 = 10;
const AUTH_LOG_CANDIDATES: &[&str] = &[
    "/var/log/auth.log",
    "/var/log/auth.log.1",
    "/var/log/auth.log.1.gz",
];

fn ssh_security_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sshSecurity.{key}"))
}

fn ssh_security_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.sshSecurity.{key}"), params)
}

fn ssh_security_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sshSecurity.routes.{key}"))
}

fn ssh_security_route_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.sshSecurity.routes.{key}"), params)
}

#[derive(Deserialize)]
struct ListQuery {
    page: Option<String>,
    limit: Option<String>,
    search: Option<String>,
    outcome: Option<String>,
}

pub fn ssh_security_routes() -> Router<AppState> {
    ssh_security_openapi_routes().into()
}

pub(crate) fn ssh_security_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_config))
        .routes(routes!(update_config))
        .routes(routes!(sync_firewall))
        .routes(routes!(clear_firewall))
        .routes(routes!(login_logs))
        .routes(routes!(list_blocks))
        .routes(routes!(delete_blocks))
        .routes(routes!(get_block))
        .routes(routes!(delete_block))
}

pub fn start_ssh_security_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("ssh-security-maintenance", async move {
        tokio::select! {
            _ = task_state.shutdown.cancelled() => return,
            result = ssh_security_maintenance_tick(&task_state) => {
                if let Err(error) = result {
                    tracing::warn!(%error, "SSH security boot sync failed");
                }
            }
        }

        let mut active = match ssh_security_config_active(&task_state).await {
            Ok(active) => Some(active),
            Err(error) => {
                tracing::debug!(%error, "failed to read SSH security maintenance state");
                None
            }
        };
        loop {
            let reload = tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = task_state.ssh_security_maintenance_notify.notified() => true,
                _ = tokio_time::sleep(Duration::from_secs(SSH_SECURITY_TICK_SECONDS)),
                    if active != Some(false) => false,
            };
            if reload || active.is_none() {
                active = match ssh_security_config_active(&task_state).await {
                    Ok(active) => Some(active),
                    Err(error) => {
                        tracing::debug!(%error, "failed to read SSH security maintenance state");
                        None
                    }
                };
            }
            if active != Some(true) {
                continue;
            }
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                result = ssh_security_maintenance_tick(&task_state) => {
                    if let Err(error) = result {
                        tracing::debug!(%error, "SSH security maintenance tick failed");
                    }
                }
            }
        }
    });
}

async fn ssh_security_config_active(state: &AppState) -> crate::storage::StorageResult<bool> {
    let config = load_config(state).await?;
    let runtime = load_runtime(state).await?;
    Ok(config.get("enabled").and_then(Value::as_bool) == Some(true)
        && runtime.get("enabled").and_then(Value::as_bool) == Some(true))
}

#[derive(Debug)]
enum SshError {
    BadRequest(String),
    Runtime(String),
    Storage(crate::storage::StorageError),
}

impl From<crate::storage::StorageError> for SshError {
    fn from(value: crate::storage::StorageError) -> Self {
        Self::Storage(value)
    }
}

struct ResolvedAllowedRegions {
    selections: Value,
    policy: CompiledIpSet,
}

struct SshAvailability {
    available: bool,
    reason: String,
    log_source: &'static str,
}

struct FirewallPolicyResult {
    allowed_cidrs: usize,
    blocked_ips: usize,
    ports: Vec<i64>,
}

mod availability;
mod blocks;
mod config;
mod handlers;
mod log_sources;
mod login_logs;
mod maintenance;
mod utils;

use availability::*;
use blocks::*;
use handlers::*;
use log_sources::*;
use login_logs::*;
use maintenance::*;
use utils::*;

pub(crate) use config::migrate_ssh_ipset_on_boot;
pub(crate) use config::normalize_config;
use config::{
    compact_runtime, load_config, load_runtime, policy_from_runtime, ssh_security_details,
    update_ssh_security_config,
};

#[cfg(test)]
mod tests;
