use std::{
    collections::{BTreeSet, HashMap},
    env,
    net::{Ipv4Addr, ToSocketAddrs},
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use get_if_addrs::{IfAddr, get_if_addrs};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{net::TcpStream, task::JoinSet, time::timeout};
use url::Url;

use crate::{i18n::Translator, response, runtime_profile, state::AppState};

mod analyzer;
mod handlers;
mod host_probe;
mod jobs;
mod network;
mod runner;
mod targets;

use analyzer::*;
use handlers::*;
use host_probe::*;
use jobs::*;
use network::*;
use runner::*;
use targets::*;

#[cfg(test)]
mod tests;

const MAX_SCAN_CIDRS: usize = 16;
const MAX_SCAN_HOSTS: u64 = 1024;
const DOCKER_DISCOVER_IP_HEADER: &str = "x-fn-knock-docker-discover-ip";
const DISCOVER_JOB_ACTIVE_TTL_MS: i64 = 30 * 60 * 1000;
const DISCOVER_JOB_DONE_TTL_MS: i64 = 5 * 60 * 1000;
const DISCOVER_JOB_MAX_ACTIVE: usize = 4;
const DISCOVER_JOB_MAX_RETAINED: usize = 64;
const LOOPBACK_DISCOVERY_CIDR: &str = "127.0.0.1/32";
const LOOPBACK_DISCOVERY_HOST: &str = "127.0.0.1";
const DISCOVERY_PORT_RANGE_START: u16 = 80;
const DISCOVERY_PORT_RANGE_END: u16 = 60_000;
const DISCOVERY_LIMITED_PORT_RANGE_END: u16 = 9_999;
const DISCOVERY_TIMEOUT_MS: u64 = 80;
const DISCOVERY_HTTP_TIMEOUT_MS: u64 = 2_000;
const DISCOVERY_HTTP_USER_AGENT: &str = "Fn-Knock-Scanner/1.0";
const NETWORK_MAX_CONCURRENT: usize = 64;
const NETWORK_HOST_CONCURRENCY: usize = 6;
const LOOPBACK_MAX_CONCURRENT: usize = 200;
const LOCAL_SELF_DISCOVERY_SKIP_PORTS: &[u16] = &[80];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DiscoveryPortRangeMode {
    Full,
    Limited,
}

#[derive(Clone, Copy)]
struct DiscoveryPortRange {
    start: u16,
    end: u16,
}

struct DiscoveryHostGroup {
    hosts: Vec<String>,
    mode: DiscoveryPortRangeMode,
    port_range: DiscoveryPortRange,
    skip_ports: Vec<u16>,
}

#[derive(Deserialize)]
struct DiscoverTargetsBody {
    #[serde(default)]
    custom_cidrs: Vec<String>,
    #[serde(default)]
    selected_cidrs: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoverJobBody {
    #[serde(default)]
    target_cidrs: Vec<String>,
}

#[derive(Deserialize)]
struct DiscoverJobQuery {
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct HostMappingProbeBody {
    hosts: Option<Vec<String>>,
}

#[derive(Clone)]
struct ParsedIpv4Cidr {
    cidr: String,
    first_host: u32,
    last_host: u32,
    host_count: u64,
}

struct DiscoverJob {
    id: String,
    cancel: Arc<AtomicBool>,
    created_at: i64,
    updated_at: i64,
    state: String,
    meta: Option<Value>,
    progress: Option<Value>,
    service_events: Vec<Value>,
    service_map: Vec<(String, Value)>,
    result: Option<Value>,
    error: Option<String>,
}

type DiscoverJobHandle = Arc<Mutex<DiscoverJob>>;

static DISCOVER_JOBS: OnceLock<Mutex<HashMap<String, DiscoverJobHandle>>> = OnceLock::new();

#[derive(Clone)]
struct DiscoveryProxyRule {
    path: String,
    rewrite_html: bool,
    use_root_mode: bool,
}

#[derive(Clone)]
struct DiscoveryAnalyzerRule {
    name: String,
    label: String,
    proxy: DiscoveryProxyRule,
    is_default: bool,
}

struct DiscoveryHttpResult {
    host: String,
    port: u16,
    status: u16,
    headers: HashMap<String, String>,
    body: String,
}

pub fn scan_asset_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/scan/discover-targets",
            get(get_discover_targets).post(save_discover_targets),
        )
        .route("/api/admin/scan/discover/jobs", post(start_discover_job))
        .route(
            "/api/admin/scan/discover/jobs/{job_id}",
            get(get_discover_job).delete(cancel_discover_job_route),
        )
        .route(
            "/api/admin/scan/host-mappings/probe",
            post(probe_host_mappings),
        )
}
