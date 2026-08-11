use fn_knock_wol_protocol::{
    AckPacket, AckStatus, Command, MacAddress, PACKET_LEN, RequestPacket, decode_request,
    encode_ack, magic_packet,
};
use ipnet::IpNet;
use serde_json::json;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::UdpSocket,
    sync::{Mutex, OwnedSemaphorePermit, Semaphore},
    task::JoinSet,
    time,
};
use uuid::Uuid;

use crate::{state::AppState, time_utils};

use super::{
    secrets::{local_relay_secret_id, secret_store},
    store::LocalRelayConfig,
    store::load_local_relay_config,
};

const MAX_CLOCK_SKEW: Duration = Duration::from_secs(60);
const REPLAY_TTL_SECONDS: u64 = 120;
const REPLAY_MAX_ENTRIES: usize = 4096;
const RESTART_DELAY: Duration = Duration::from_secs(5);
const STATUS_CONCURRENCY: usize = 8;

pub(crate) fn start_wol_relay_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("wol-relay-supervisor", async move {
        relay_supervisor(task_state).await;
    });
}

async fn relay_supervisor(state: AppState) {
    let mut runtime_reload = state.wol.runtime_reload.subscribe();
    loop {
        if state.shutdown.is_cancelled() {
            set_status(&state, false, false, None, None).await;
            return;
        }

        match super::feature_enabled_for_state(&state).await {
            Ok(true) => {}
            Ok(false) => {
                set_status(&state, false, false, None, None).await;
                tokio::select! {
                    _ = state.shutdown.cancelled() => return,
                    _ = runtime_reload.changed() => continue,
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to load WoL feature configuration");
                set_status(&state, false, false, None, Some(error.to_string())).await;
                wait_for_reload_or_retry(&state, &mut runtime_reload).await;
                continue;
            }
        }

        let config = match load_local_relay_config(&state).await {
            Ok(config) => config,
            Err(error) => {
                tracing::warn!(%error, "failed to load built-in WoL Relay configuration");
                set_status(&state, false, false, None, Some(error.to_string())).await;
                wait_for_reload_or_retry(&state, &mut runtime_reload).await;
                continue;
            }
        };

        if !config.enabled {
            set_status(&state, false, false, None, None).await;
            tokio::select! {
                _ = state.shutdown.cancelled() => return,
                _ = state.wol.relay_reload.notified() => continue,
                _ = runtime_reload.changed() => continue,
            }
        }

        match run_listener(&state, &config, &mut runtime_reload).await {
            ListenerExit::Reload => continue,
            ListenerExit::Shutdown => {
                set_status(&state, true, false, None, None).await;
                return;
            }
            ListenerExit::Failed(error) => {
                tracing::warn!(%error, "built-in WoL Relay listener stopped");
                set_status(&state, true, false, None, Some(error)).await;
                wait_for_reload_or_retry(&state, &mut runtime_reload).await;
            }
        }
    }
}

async fn wait_for_reload_or_retry(
    state: &AppState,
    runtime_reload: &mut tokio::sync::watch::Receiver<u64>,
) {
    tokio::select! {
        _ = state.shutdown.cancelled() => {}
        _ = state.wol.relay_reload.notified() => {}
        _ = runtime_reload.changed() => {}
        _ = time::sleep(RESTART_DELAY) => {}
    }
}

enum ListenerExit {
    Reload,
    Shutdown,
    Failed(String),
}

async fn run_listener(
    state: &AppState,
    config: &LocalRelayConfig,
    runtime_reload: &mut tokio::sync::watch::Receiver<u64>,
) -> ListenerExit {
    let relay_id = match Uuid::parse_str(&config.relay_id) {
        Ok(value) => *value.as_bytes(),
        Err(_) => return ListenerExit::Failed("Relay ID is invalid".to_string()),
    };
    if config.key_version == 0 {
        return ListenerExit::Failed("Relay key version is invalid".to_string());
    }
    let psk = match secret_store(state)
        .read(&local_relay_secret_id(&config.relay_id), config.key_version)
    {
        Ok(Some(value)) if value.len() == 32 => value,
        Ok(_) => return ListenerExit::Failed("Relay PSK is not configured".to_string()),
        Err(error) => return ListenerExit::Failed(error),
    };
    let listen_ip = match config.listen_address.parse::<IpAddr>() {
        Ok(value) => value,
        Err(_) => return ListenerExit::Failed("Relay listen address is invalid".to_string()),
    };
    let listen_endpoint = SocketAddr::new(listen_ip, config.port);
    let allowed_sources = match parse_allowed_sources(&config.allowed_sources) {
        Ok(value) => value,
        Err(error) => return ListenerExit::Failed(error),
    };
    let broadcast_destinations = match parse_broadcast_destinations(&config.broadcast_destinations)
    {
        Ok(value) => value,
        Err(error) => return ListenerExit::Failed(error),
    };
    let listener = match UdpSocket::bind(listen_endpoint).await {
        Ok(value) => Arc::new(value),
        Err(error) => {
            return ListenerExit::Failed(format!(
                "failed to bind UDP listener on {listen_endpoint}: {error}"
            ));
        }
    };
    let broadcast_socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(value) => Arc::new(value),
        Err(error) => {
            return ListenerExit::Failed(format!("failed to open WoL broadcast socket: {error}"));
        }
    };
    if let Err(error) = broadcast_socket.set_broadcast(true) {
        return ListenerExit::Failed(format!("failed to enable UDP broadcast: {error}"));
    }
    let actual_endpoint = listener.local_addr().unwrap_or(listen_endpoint);
    set_status(state, true, true, Some(actual_endpoint.to_string()), None).await;
    tracing::info!(address = %actual_endpoint, relay_id = %config.relay_id, "built-in WoL Relay listening");

    let processor = Arc::new(Mutex::new(RelayProcessor::new(
        relay_id,
        config.key_version,
        psk,
    )));
    let status_semaphore = Arc::new(Semaphore::new(STATUS_CONCURRENCY));
    let mut workers = JoinSet::new();
    // The extra byte ensures an oversized datagram is not accepted after UDP truncation.
    let mut input = [0_u8; PACKET_LEN + 1];
    loop {
        tokio::select! {
            _ = state.shutdown.cancelled() => return ListenerExit::Shutdown,
            _ = state.wol.relay_reload.notified() => return ListenerExit::Reload,
            _ = runtime_reload.changed() => return ListenerExit::Reload,
            completed = workers.join_next(), if !workers.is_empty() => {
                if let Some(Err(error)) = completed {
                    tracing::warn!(%error, "built-in WoL Relay request worker failed");
                }
            }
            received = listener.recv_from(&mut input) => {
                let (length, source) = match received {
                    Ok(value) => value,
                    Err(error) => return ListenerExit::Failed(format!("failed to receive Relay datagram: {error}")),
                };
                if !source_allowed(source.ip(), &allowed_sources) {
                    continue;
                }
                let now = unix_seconds();
                let action = processor.lock().await.inspect(&input[..length], now);
                let Some(action) = action else {
                    // Authentication and identity failures are deliberately silent.
                    continue;
                };
                match action {
                    RelayAction::Cached(packet) => {
                        if let Err(error) = listener.send_to(&packet, source).await {
                            tracing::warn!(%error, source = %source, "failed to send cached signed WoL Relay acknowledgement");
                        }
                    }
                    RelayAction::New { request, forced_status } => {
                        let listener = Arc::clone(&listener);
                        let broadcast_socket = Arc::clone(&broadcast_socket);
                        let destinations = broadcast_destinations.clone();
                        let processor = Arc::clone(&processor);
                        let status_semaphore = Arc::clone(&status_semaphore);
                        workers.spawn(async move {
                            let _status_permit = if forced_status.is_none() && request.command == Command::Status {
                                try_acquire_status_permit(&status_semaphore)
                            } else {
                                None
                            };
                            let overloaded = forced_status.is_none()
                                && request.command == Command::Status
                                && _status_permit.is_none();
                            let effective_status = forced_status.or(
                                overloaded.then_some(AckStatus::TargetUnknown),
                            );
                            let (status, observed_ip) = match effective_status {
                                Some(status) => (status, request.target_ipv4),
                                None => process_request(&request, &broadcast_socket, &destinations).await,
                            };
                            let request_id = Uuid::from_bytes(request.request_id);
                            let target_mac = MacAddress::from_bytes(request.target_mac).ok();
                            tracing::info!(
                                %request_id,
                                source = %source,
                                target_mac = target_mac.map(|value| value.to_string()).unwrap_or_default(),
                                command = ?request.command,
                                status = ?status,
                                "processed authenticated built-in WoL Relay request"
                            );
                            let ack = processor.lock().await.finish(&request, status, observed_ip, unix_seconds());
                            if let Err(error) = listener.send_to(&ack, source).await {
                                tracing::warn!(%error, source = %source, "failed to send signed WoL Relay acknowledgement");
                            }
                        });
                    }
                }
            }
        }
    }
}

async fn process_request(
    request: &RequestPacket,
    socket: &UdpSocket,
    destinations: &[SocketAddr],
) -> (AckStatus, [u8; 4]) {
    if request.command == Command::Probe {
        return (AckStatus::Ok, request.target_ipv4);
    }
    let mac = match MacAddress::from_bytes(request.target_mac) {
        Ok(value) => value,
        Err(_) => return (AckStatus::InvalidTarget, request.target_ipv4),
    };
    if request.command == Command::Status {
        let result = super::probe::probe_device_bounded(
            mac,
            super::probe::ipv4_from_wire(request.target_ipv4),
        )
        .await;
        let status = match result.state {
            super::probe::DeviceProbeState::Online => AckStatus::TargetOnline,
            super::probe::DeviceProbeState::Offline => AckStatus::TargetOffline,
            super::probe::DeviceProbeState::Unknown => AckStatus::TargetUnknown,
        };
        return (status, super::probe::ipv4_to_wire(result.observed_ip));
    }
    let packet = magic_packet(mac);
    let mut delivered = 0_usize;
    for destination in destinations {
        match socket.send_to(&packet, destination).await {
            Ok(length) if length == packet.len() => delivered += 1,
            _ => {}
        }
    }
    (
        if delivered > 0 {
            AckStatus::Ok
        } else {
            AckStatus::BroadcastFailed
        },
        request.target_ipv4,
    )
}

fn try_acquire_status_permit(semaphore: &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
    Arc::clone(semaphore).try_acquire_owned().ok()
}

fn parse_allowed_sources(values: &[String]) -> Result<Vec<IpNet>, String> {
    values
        .iter()
        .map(|value| {
            value
                .trim()
                .parse::<IpNet>()
                .map_err(|_| format!("allowed source CIDR is invalid: {value}"))
        })
        .collect()
}

fn parse_broadcast_destinations(values: &[String]) -> Result<Vec<SocketAddr>, String> {
    if values.is_empty() {
        return Err("at least one broadcast destination is required".to_string());
    }
    values
        .iter()
        .map(|value| {
            let endpoint = value
                .trim()
                .parse::<SocketAddr>()
                .map_err(|_| format!("broadcast destination is invalid: {value}"))?;
            if !endpoint.is_ipv4() || endpoint.port() == 0 {
                return Err(format!(
                    "broadcast destination must be IPv4 with a port: {value}"
                ));
            }
            Ok(endpoint)
        })
        .collect()
}

fn source_allowed(source: IpAddr, allowed_sources: &[IpNet]) -> bool {
    allowed_sources.is_empty()
        || allowed_sources
            .iter()
            .any(|network| network.contains(&source))
}

async fn set_status(
    state: &AppState,
    enabled: bool,
    active: bool,
    listen_address: Option<String>,
    last_error: Option<String>,
) {
    *state.wol.relay_status.write().await = json!({
        "enabled": enabled,
        "active": active,
        "listenAddress": listen_address,
        "lastError": last_error,
        "updatedAt": time_utils::now_iso(),
    });
}

#[derive(Clone)]
struct CachedAck {
    created_at: u64,
    packet: Vec<u8>,
}

enum RelayAction {
    Cached(Vec<u8>),
    New {
        request: RequestPacket,
        forced_status: Option<AckStatus>,
    },
}

struct RelayProcessor {
    relay_id: [u8; 16],
    key_version: u32,
    psk: Vec<u8>,
    replay: HashMap<[u8; 16], CachedAck>,
    replay_order: VecDeque<[u8; 16]>,
    pending: HashSet<[u8; 16]>,
}

impl RelayProcessor {
    fn new(relay_id: [u8; 16], key_version: u32, psk: Vec<u8>) -> Self {
        Self {
            relay_id,
            key_version,
            psk,
            replay: HashMap::new(),
            replay_order: VecDeque::new(),
            pending: HashSet::new(),
        }
    }

    fn inspect(&mut self, input: &[u8], now: u64) -> Option<RelayAction> {
        let request = decode_request(input, &self.psk).ok()?;
        if request.relay_id != self.relay_id || request.key_version != self.key_version {
            return None;
        }
        self.prune_replay(now);
        if let Some(cached) = self.replay.get(&request.request_id) {
            return Some(RelayAction::Cached(cached.packet.clone()));
        }
        if !self.pending.insert(request.request_id) {
            return None;
        }
        Some(RelayAction::New {
            forced_status: (now.abs_diff(request.timestamp) > MAX_CLOCK_SKEW.as_secs())
                .then_some(AckStatus::ClockSkew),
            request,
        })
    }

    fn finish(
        &mut self,
        request: &RequestPacket,
        status: AckStatus,
        target_ipv4: [u8; 4],
        now: u64,
    ) -> Vec<u8> {
        self.pending.remove(&request.request_id);
        let packet = encode_ack(
            &AckPacket {
                command: request.command,
                status,
                relay_id: request.relay_id,
                key_version: request.key_version,
                timestamp: now,
                request_id: request.request_id,
                target_mac: request.target_mac,
                target_ipv4,
            },
            &self.psk,
        )
        .to_vec();
        self.replay_order.push_back(request.request_id);
        self.replay.insert(
            request.request_id,
            CachedAck {
                created_at: now,
                packet: packet.clone(),
            },
        );
        while self.replay.len() > REPLAY_MAX_ENTRIES {
            if let Some(oldest) = self.replay_order.pop_front() {
                self.replay.remove(&oldest);
            }
        }
        packet
    }

    fn prune_replay(&mut self, now: u64) {
        while let Some(request_id) = self.replay_order.front().copied() {
            let expired = self
                .replay
                .get(&request_id)
                .is_none_or(|entry| now.saturating_sub(entry.created_at) > REPLAY_TTL_SECONDS);
            if !expired {
                break;
            }
            self.replay_order.pop_front();
            self.replay.remove(&request_id);
        }
    }
}

fn unix_seconds() -> u64 {
    (time_utils::now_ms().max(0) / 1000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_knock_wol_protocol::{decode_ack, encode_request};

    fn request(request_id: [u8; 16], timestamp: u64) -> RequestPacket {
        RequestPacket {
            command: Command::Wake,
            relay_id: [1; 16],
            key_version: 1,
            timestamp,
            request_id,
            target_mac: [0x02, 1, 2, 3, 4, 5],
            target_ipv4: [192, 168, 1, 10],
        }
    }

    #[test]
    fn authenticates_identity_and_caches_signed_ack() {
        let psk = vec![7; 32];
        let mut processor = RelayProcessor::new([1; 16], 1, psk.clone());
        let packet = encode_request(&request([2; 16], 1_000), &psk);
        let RelayAction::New {
            request,
            forced_status,
        } = processor.inspect(&packet, 1_000).expect("valid request")
        else {
            panic!("first request must not be cached")
        };
        assert_eq!(forced_status, None);
        let first = processor.finish(&request, AckStatus::Ok, request.target_ipv4, 1_000);
        let RelayAction::Cached(second) = processor.inspect(&packet, 1_001).expect("replay") else {
            panic!("retransmission must use cached ACK")
        };
        assert_eq!(first, second);
        assert_eq!(decode_ack(&second, &psk).unwrap().status, AckStatus::Ok);

        let mut tampered = packet;
        tampered[10] ^= 1;
        assert!(processor.inspect(&tampered, 1_001).is_none());
    }

    #[test]
    fn rejects_wrong_identity_and_reports_clock_skew() {
        let psk = vec![8; 32];
        let mut processor = RelayProcessor::new([1; 16], 1, psk.clone());
        let wrong_identity = encode_request(
            &RequestPacket {
                relay_id: [9; 16],
                ..request([3; 16], 1_000)
            },
            &psk,
        );
        assert!(processor.inspect(&wrong_identity, 1_000).is_none());

        let stale = encode_request(&request([4; 16], 1), &psk);
        let RelayAction::New { forced_status, .. } =
            processor.inspect(&stale, 1_000).expect("signed request")
        else {
            panic!("request is not cached")
        };
        assert_eq!(forced_status, Some(AckStatus::ClockSkew));
    }

    #[tokio::test]
    async fn emits_exact_magic_packet_to_configured_destination() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let sender = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        sender.set_broadcast(true).unwrap();
        let (status, _) = process_request(
            &request([5; 16], 1_000),
            &sender,
            &[receiver.local_addr().unwrap()],
        )
        .await;
        assert_eq!(status, AckStatus::Ok);
        let mut packet = [0_u8; 103];
        let (length, _) = receiver.recv_from(&mut packet).await.unwrap();
        assert_eq!(length, 102);
        assert_eq!(&packet[..6], &[0xff; 6]);
    }

    #[test]
    fn status_capacity_is_rejected_instead_of_queued() {
        let semaphore = Arc::new(Semaphore::new(1));
        let permit = try_acquire_status_permit(&semaphore).expect("first status permit");
        assert!(try_acquire_status_permit(&semaphore).is_none());
        drop(permit);
        assert!(try_acquire_status_permit(&semaphore).is_some());
    }
}
