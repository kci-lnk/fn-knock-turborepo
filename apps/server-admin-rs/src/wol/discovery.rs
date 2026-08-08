use fn_knock_wol_protocol::MacAddress;
use get_if_addrs::{IfAddr, get_if_addrs};
use ipnet::Ipv4Net;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    process::Stdio,
    sync::{
        Arc, Mutex, MutexGuard, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::{process::Command, task::JoinSet, time};
use uuid::Uuid;

use crate::time_utils;

const MAX_NETWORKS: usize = 16;
const MAX_SCAN_HOSTS: usize = 4096;
const MAX_CONCURRENT_PROBES: usize = 256;
const PROBE_TIMEOUT: Duration = Duration::from_millis(1_500);
const COMPLETED_JOB_TTL_MS: i64 = 5 * 60 * 1000;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LocalNetwork {
    pub interface_name: String,
    pub address: String,
    pub cidr: String,
    pub scan_cidr: String,
    pub broadcast_address: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiscoveredDevice {
    pub ip: String,
    pub mac: String,
    pub interface_name: String,
    pub broadcast_address: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiscoveryProgress {
    pub scanned_hosts: usize,
    pub total_hosts: usize,
    pub found_devices: usize,
    pub current_host: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiscoveryResult {
    pub devices: Vec<DiscoveredDevice>,
    pub networks: Vec<LocalNetwork>,
    pub duration_ms: u64,
    pub method: &'static str,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DiscoveryJobStatus {
    pub job_id: String,
    pub state: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub networks: Vec<LocalNetwork>,
    pub progress: DiscoveryProgress,
    pub devices: Vec<DiscoveredDevice>,
    pub next_cursor: usize,
    pub result: Option<DiscoveryResult>,
    pub error: Option<String>,
}

pub(super) enum DiscoveryJobError {
    BadRequest(String),
    Conflict(String),
    Internal(String),
}

#[derive(Clone)]
struct ProbeTarget {
    ip: Ipv4Addr,
    interface_name: String,
    broadcast_address: String,
}

struct DiscoveryJob {
    id: String,
    cancel: Arc<AtomicBool>,
    created_at: i64,
    updated_at: i64,
    state: String,
    networks: Vec<LocalNetwork>,
    progress: DiscoveryProgress,
    devices: Vec<DiscoveredDevice>,
    result: Option<DiscoveryResult>,
    error: Option<String>,
}

type DiscoveryJobHandle = Arc<Mutex<DiscoveryJob>>;

static DISCOVERY_JOBS: OnceLock<Mutex<HashMap<String, DiscoveryJobHandle>>> = OnceLock::new();

pub(super) async fn start_discovery_job(
    target_cidrs: Vec<String>,
) -> Result<DiscoveryJobStatus, DiscoveryJobError> {
    let networks = tokio::task::spawn_blocking(move || networks_for_scan(&target_cidrs))
        .await
        .map_err(|error| DiscoveryJobError::Internal(format!("prepare LAN scan: {error}")))?
        .map_err(DiscoveryJobError::BadRequest)?;
    let targets = probe_targets(&networks).map_err(DiscoveryJobError::BadRequest)?;
    let now = time_utils::now_ms();
    let id = Uuid::new_v4().to_string();
    let job = Arc::new(Mutex::new(DiscoveryJob {
        id: id.clone(),
        cancel: Arc::new(AtomicBool::new(false)),
        created_at: now,
        updated_at: now,
        state: "queued".to_string(),
        networks,
        progress: DiscoveryProgress {
            scanned_hosts: 0,
            total_hosts: targets.len(),
            found_devices: 0,
            current_host: targets
                .first()
                .map(|target| target.ip.to_string())
                .unwrap_or_default(),
        },
        devices: Vec::new(),
        result: None,
        error: None,
    }));

    {
        let mut jobs = discovery_jobs_guard();
        retain_recent_jobs(&mut jobs, now);
        if jobs.values().any(|job| {
            let job = job_guard(job);
            matches!(job.state.as_str(), "queued" | "running")
        }) {
            return Err(DiscoveryJobError::Conflict(
                "A LAN discovery scan is already running".to_string(),
            ));
        }
        jobs.insert(id, job.clone());
    }

    let initial = job_status(&job, 0);
    tokio::spawn(run_discovery_job(job, targets));
    Ok(initial)
}

pub(super) fn get_discovery_job(id: &str, cursor: usize) -> Option<DiscoveryJobStatus> {
    let now = time_utils::now_ms();
    let mut jobs = discovery_jobs_guard();
    retain_recent_jobs(&mut jobs, now);
    jobs.get(id).map(|job| job_status(job, cursor))
}

pub(super) fn cancel_discovery_job(id: &str) -> Option<DiscoveryJobStatus> {
    let job = {
        let jobs = discovery_jobs_guard();
        jobs.get(id).cloned()
    }?;
    {
        let mut job = job_guard(&job);
        job.cancel.store(true, Ordering::SeqCst);
        if matches!(job.state.as_str(), "queued" | "running") {
            job.state = "cancelled".to_string();
            job.updated_at = time_utils::now_ms();
        }
    }
    Some(job_status(&job, 0))
}

pub(super) fn local_broadcast_addresses() -> Vec<Ipv4Addr> {
    let mut addresses = connected_networks()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|network| network.broadcast_address.parse().ok())
        .collect::<Vec<_>>();
    addresses.push(Ipv4Addr::BROADCAST);
    addresses.sort_unstable();
    addresses.dedup();
    addresses
}

async fn run_discovery_job(job: DiscoveryJobHandle, targets: Vec<ProbeTarget>) {
    let started = time::Instant::now();
    update_job(&job, |job| {
        if job.cancel.load(Ordering::SeqCst) {
            return false;
        }
        job.state = "running".to_string();
        true
    });
    if job_guard(&job).state != "running" {
        return;
    }

    let cancel = job_guard(&job).cancel.clone();
    let mut online = Vec::new();
    let mut probe_error = None;
    let mut jobs = JoinSet::new();
    let mut targets = targets.into_iter();

    loop {
        if cancel.load(Ordering::SeqCst) {
            jobs.abort_all();
            return;
        }
        while jobs.len() < MAX_CONCURRENT_PROBES {
            let Some(target) = targets.next() else {
                break;
            };
            jobs.spawn(async move {
                let result = probe_host(target.ip).await;
                (target, result)
            });
        }
        let Some(result) = jobs.join_next().await else {
            break;
        };
        match result {
            Ok((target, Ok(true))) => {
                online.push(target.clone());
                if let Some(device) = resolve_discovered_device(&target).await {
                    push_device(&job, device);
                }
                update_progress(&job, &target.ip.to_string());
            }
            Ok((target, Ok(false))) => update_progress(&job, &target.ip.to_string()),
            Ok((target, Err(error))) => {
                probe_error = Some(error);
                update_progress(&job, &target.ip.to_string());
            }
            Err(_) => update_progress(&job, ""),
        }
    }

    if cancel.load(Ordering::SeqCst) {
        return;
    }
    let had_online_devices = !online.is_empty();
    let neighbors = read_neighbor_table().await;
    for target in online {
        if let Some(mac) = neighbors.get(&target.ip) {
            push_device(
                &job,
                DiscoveredDevice {
                    ip: target.ip.to_string(),
                    mac: mac.clone(),
                    interface_name: target.interface_name,
                    broadcast_address: target.broadcast_address,
                },
            );
        }
    }

    if !had_online_devices && let Some(error) = probe_error {
        update_job(&job, |job| {
            job.state = "failed".to_string();
            job.error = Some(error);
        });
        return;
    }

    update_job(&job, |job| {
        let mut devices = job.devices.clone();
        devices.sort_by_key(|device| device.ip.parse::<Ipv4Addr>().ok());
        job.result = Some(DiscoveryResult {
            devices,
            networks: job.networks.clone(),
            duration_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
            method: "icmp-neighbor",
        });
        job.state = "completed".to_string();
    });
}

async fn resolve_discovered_device(target: &ProbeTarget) -> Option<DiscoveredDevice> {
    let neighbors = read_neighbor_table().await;
    neighbors.get(&target.ip).map(|mac| DiscoveredDevice {
        ip: target.ip.to_string(),
        mac: mac.clone(),
        interface_name: target.interface_name.clone(),
        broadcast_address: target.broadcast_address.clone(),
    })
}

fn push_device(job: &DiscoveryJobHandle, device: DiscoveredDevice) {
    update_job(job, |job| {
        if job.devices.iter().any(|current| current.mac == device.mac) {
            return;
        }
        job.devices.push(device);
        job.progress.found_devices = job.devices.len();
    });
}

fn update_progress(job: &DiscoveryJobHandle, current_host: &str) {
    update_job(job, |job| {
        job.progress.scanned_hosts = job.progress.scanned_hosts.saturating_add(1);
        job.progress.current_host = current_host.to_string();
    });
}

fn update_job<T>(job: &DiscoveryJobHandle, update: impl FnOnce(&mut DiscoveryJob) -> T) -> T {
    let mut job = job_guard(job);
    let result = update(&mut job);
    job.updated_at = time_utils::now_ms();
    result
}

fn job_status(job: &DiscoveryJobHandle, cursor: usize) -> DiscoveryJobStatus {
    let job = job_guard(job);
    let cursor = cursor.min(job.devices.len());
    DiscoveryJobStatus {
        job_id: job.id.clone(),
        state: job.state.clone(),
        created_at: job.created_at,
        updated_at: job.updated_at,
        networks: job.networks.clone(),
        progress: job.progress.clone(),
        devices: job.devices[cursor..].to_vec(),
        next_cursor: job.devices.len(),
        result: job.result.clone(),
        error: job.error.clone(),
    }
}

fn discovery_jobs() -> &'static Mutex<HashMap<String, DiscoveryJobHandle>> {
    DISCOVERY_JOBS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn discovery_jobs_guard() -> MutexGuard<'static, HashMap<String, DiscoveryJobHandle>> {
    discovery_jobs()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn job_guard(job: &DiscoveryJobHandle) -> MutexGuard<'_, DiscoveryJob> {
    job.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn retain_recent_jobs(jobs: &mut HashMap<String, DiscoveryJobHandle>, now: i64) {
    jobs.retain(|_, job| {
        let job = job_guard(job);
        matches!(job.state.as_str(), "queued" | "running")
            || now.saturating_sub(job.updated_at) <= COMPLETED_JOB_TTL_MS
    });
}

fn networks_for_scan(target_cidrs: &[String]) -> Result<Vec<LocalNetwork>, String> {
    let connected = connected_networks()?;
    let requested = target_cidrs
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    if requested.is_empty() {
        if connected.is_empty() {
            return Err("No usable local IPv4 network was detected".to_string());
        }
        return Ok(connected);
    }
    if requested.len() > MAX_NETWORKS {
        return Err(format!("At most {MAX_NETWORKS} scan networks are allowed"));
    }

    let mut networks = Vec::new();
    let mut seen = HashSet::new();
    for value in requested {
        let parsed = value
            .parse::<Ipv4Net>()
            .map_err(|_| format!("Scan network '{value}' must be an IPv4 CIDR"))?;
        if parsed.prefix_len() > 30 {
            return Err(format!(
                "Scan network '{}' must contain at least two usable hosts",
                parsed
            ));
        }
        if !is_lan_address(parsed.network()) {
            return Err(format!(
                "Scan network '{}' must be a private or shared LAN range",
                parsed
            ));
        }
        let normalized = parsed.to_string();
        if !seen.insert(normalized.clone()) {
            continue;
        }
        let matched = connected.iter().find(|network| {
            network
                .address
                .parse::<Ipv4Addr>()
                .ok()
                .is_some_and(|address| parsed.contains(&address))
        });
        networks.push(LocalNetwork {
            interface_name: matched
                .map(|network| network.interface_name.clone())
                .unwrap_or_else(|| "custom".to_string()),
            address: matched
                .map(|network| network.address.clone())
                .unwrap_or_default(),
            cidr: matched
                .map(|network| network.cidr.clone())
                .unwrap_or_else(|| normalized.clone()),
            scan_cidr: normalized,
            broadcast_address: parsed.broadcast().to_string(),
        });
    }
    Ok(networks)
}

fn connected_networks() -> Result<Vec<LocalNetwork>, String> {
    let mut networks = Vec::new();
    let interfaces = get_if_addrs().map_err(|error| error.to_string())?;
    for interface in interfaces {
        let IfAddr::V4(address) = interface.addr else {
            continue;
        };
        if address.ip.is_loopback()
            || address.ip.is_unspecified()
            || address.ip.is_multicast()
            || address.ip.is_link_local()
            || !is_lan_address(address.ip)
        {
            continue;
        }
        let Some(prefix) = netmask_prefix(address.netmask) else {
            continue;
        };
        if prefix > 30 {
            continue;
        }
        let mask = u32::from(address.netmask);
        let ip = u32::from(address.ip);
        let network = Ipv4Addr::from(ip & mask);
        let broadcast = address
            .broadcast
            .unwrap_or_else(|| Ipv4Addr::from((ip & mask) | !mask));
        let scan_prefix = prefix.max(24);
        let scan_mask = prefix_mask(scan_prefix);
        let scan_network = Ipv4Addr::from(ip & scan_mask);
        let value = LocalNetwork {
            interface_name: interface.name,
            address: address.ip.to_string(),
            cidr: format!("{network}/{prefix}"),
            scan_cidr: format!("{scan_network}/{scan_prefix}"),
            broadcast_address: broadcast.to_string(),
        };
        if !networks.iter().any(|current: &LocalNetwork| {
            current.interface_name == value.interface_name && current.cidr == value.cidr
        }) {
            networks.push(value);
        }
        if networks.len() == MAX_NETWORKS {
            break;
        }
    }
    Ok(networks)
}

fn probe_targets(networks: &[LocalNetwork]) -> Result<Vec<ProbeTarget>, String> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    let local_addresses = networks
        .iter()
        .filter_map(|network| network.address.parse::<Ipv4Addr>().ok())
        .collect::<HashSet<_>>();
    for network in networks {
        let parsed = network
            .scan_cidr
            .parse::<Ipv4Net>()
            .map_err(|_| format!("Invalid scan network '{}'", network.scan_cidr))?;
        let first = u32::from(parsed.network());
        let last = u32::from(parsed.broadcast());
        for raw in (first + 1)..last {
            let ip = Ipv4Addr::from(raw);
            if local_addresses.contains(&ip) || !seen.insert(ip) {
                continue;
            }
            targets.push(ProbeTarget {
                ip,
                interface_name: network.interface_name.clone(),
                broadcast_address: network.broadcast_address.clone(),
            });
            if targets.len() > MAX_SCAN_HOSTS {
                return Err(format!(
                    "The selected networks exceed the {MAX_SCAN_HOSTS}-host scan limit"
                ));
            }
        }
    }
    if targets.is_empty() {
        return Err("The selected networks contain no scan targets".to_string());
    }
    Ok(targets)
}

pub(super) async fn probe_host(ip: Ipv4Addr) -> Result<bool, String> {
    let mut command = Command::new("ping");
    command.kill_on_drop(true);
    command.stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(target_os = "windows")]
    command.args(["-n", "1", "-w", "1000", &ip.to_string()]);
    #[cfg(target_os = "macos")]
    command.args(["-n", "-c", "1", "-W", "1000", &ip.to_string()]);
    #[cfg(all(unix, not(target_os = "macos")))]
    command.args(["-n", "-c", "1", "-W", "1", &ip.to_string()]);
    #[cfg(not(any(unix, target_os = "windows")))]
    command.arg(ip.to_string());

    match time::timeout(PROBE_TIMEOUT, command.status()).await {
        Ok(Ok(status)) => Ok(status.success()),
        Ok(Err(error)) => Err(format!("start ICMP discovery probe: {error}")),
        Err(_) => Ok(false),
    }
}

pub(super) async fn read_neighbor_table() -> HashMap<Ipv4Addr, String> {
    read_neighbor_table_checked().await.unwrap_or_default()
}

pub(super) async fn read_neighbor_table_checked() -> Result<HashMap<Ipv4Addr, String>, String> {
    #[cfg(target_os = "linux")]
    if let Ok(content) = tokio::fs::read_to_string("/proc/net/arp").await {
        let parsed = parse_neighbor_table(&content);
        if !parsed.is_empty() {
            return Ok(parsed);
        }
    }

    let mut command = if cfg!(target_os = "windows") {
        let mut command = Command::new("arp");
        command.arg("-a");
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("arp");
        command.args(["-a", "-n"]);
        command
    } else {
        let mut command = Command::new("ip");
        command.args(["neigh", "show"]);
        command
    };
    command.kill_on_drop(true);
    match time::timeout(Duration::from_secs(2), command.output()).await {
        Ok(Ok(output)) if output.status.success() => Ok(parse_neighbor_table(
            &String::from_utf8_lossy(&output.stdout),
        )),
        Ok(Ok(output)) => Err(format!(
            "neighbor table command exited with status {}",
            output.status
        )),
        Ok(Err(error)) => Err(format!("start neighbor table command: {error}")),
        Err(_) => Err("neighbor table command timed out".to_string()),
    }
}

fn parse_neighbor_table(content: &str) -> HashMap<Ipv4Addr, String> {
    let mut neighbors = HashMap::new();
    for line in content.lines() {
        let fields = line
            .split(|character: char| {
                character.is_ascii_whitespace() || matches!(character, '(' | ')')
            })
            .filter(|field| !field.is_empty())
            .collect::<Vec<_>>();
        let Some(ip) = fields
            .iter()
            .find_map(|field| field.parse::<Ipv4Addr>().ok())
        else {
            continue;
        };
        let Some(mac) = fields
            .iter()
            .find_map(|field| field.parse::<MacAddress>().ok())
        else {
            continue;
        };
        neighbors.insert(ip, mac.to_string());
    }
    neighbors
}

fn netmask_prefix(netmask: Ipv4Addr) -> Option<u8> {
    let value = u32::from(netmask);
    let prefix = value.count_ones() as u8;
    (value == prefix_mask(prefix)).then_some(prefix)
}

fn prefix_mask(prefix: u8) -> u32 {
    if prefix == 0 {
        0
    } else {
        u32::MAX << (32 - prefix)
    }
}

fn is_lan_address(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    address.is_private() || (octets[0] == 100 && (64..=127).contains(&octets[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_contiguous_netmasks() {
        assert_eq!(netmask_prefix(Ipv4Addr::new(255, 255, 255, 0)), Some(24));
        assert_eq!(netmask_prefix(Ipv4Addr::new(255, 255, 254, 0)), Some(23));
        assert_eq!(netmask_prefix(Ipv4Addr::new(255, 0, 255, 0)), None);
    }

    #[test]
    fn limits_discovery_to_private_and_shared_lan_ranges() {
        assert!(is_lan_address(Ipv4Addr::new(192, 168, 31, 98)));
        assert!(is_lan_address(Ipv4Addr::new(100, 64, 0, 1)));
        assert!(!is_lan_address(Ipv4Addr::new(203, 0, 113, 10)));
    }

    #[test]
    fn accepts_custom_cidrs_and_preserves_automatic_fallback() {
        let custom = networks_for_scan(&["192.168.88.0/24".to_string()]).unwrap();
        assert_eq!(custom.len(), 1);
        assert_eq!(custom[0].scan_cidr, "192.168.88.0/24");
        assert_eq!(custom[0].broadcast_address, "192.168.88.255");

        let targets = probe_targets(&custom).unwrap();
        assert_eq!(targets.len(), 254);
    }

    #[test]
    fn rejects_public_and_oversized_custom_scans() {
        assert!(networks_for_scan(&["203.0.113.0/24".to_string()]).is_err());
        let networks = networks_for_scan(&["10.0.0.0/19".to_string()]).unwrap();
        assert!(probe_targets(&networks).is_err());
    }

    #[test]
    fn discovery_job_cursor_returns_only_new_devices() {
        let now = time_utils::now_ms();
        let job = Arc::new(Mutex::new(DiscoveryJob {
            id: "job-1".to_string(),
            cancel: Arc::new(AtomicBool::new(false)),
            created_at: now,
            updated_at: now,
            state: "running".to_string(),
            networks: Vec::new(),
            progress: DiscoveryProgress {
                scanned_hosts: 0,
                total_hosts: 2,
                found_devices: 0,
                current_host: String::new(),
            },
            devices: Vec::new(),
            result: None,
            error: None,
        }));
        for index in 1..=2 {
            push_device(
                &job,
                DiscoveredDevice {
                    ip: format!("192.168.1.{index}"),
                    mac: format!("02:00:00:00:00:{index:02X}"),
                    interface_name: "eth0".to_string(),
                    broadcast_address: "192.168.1.255".to_string(),
                },
            );
        }

        let status = job_status(&job, 1);
        assert_eq!(status.devices.len(), 1);
        assert_eq!(status.devices[0].ip, "192.168.1.2");
        assert_eq!(status.next_cursor, 2);
    }

    #[test]
    fn parses_linux_and_bsd_neighbor_formats() {
        let values = parse_neighbor_table(
            "IP address HW type Flags HW address Mask Device\n\
             192.168.31.10 0x1 0x2 aa:bb:cc:dd:ee:01 * eth0\n\
             ? (192.168.31.11) at aa-bb-cc-dd-ee-02 on en0",
        );
        assert_eq!(
            values[&Ipv4Addr::new(192, 168, 31, 10)],
            "AA:BB:CC:DD:EE:01"
        );
        assert_eq!(
            values[&Ipv4Addr::new(192, 168, 31, 11)],
            "AA:BB:CC:DD:EE:02"
        );
    }
}
