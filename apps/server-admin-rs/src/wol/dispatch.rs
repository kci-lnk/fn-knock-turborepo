use fn_knock_wol_protocol::{
    AckPacket, AckStatus, Command, MacAddress, PACKET_LEN, RequestPacket, decode_ack,
    encode_request, magic_packet,
};
use std::{
    net::{Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{net::UdpSocket, time};
use uuid::Uuid;

use super::store::RelayRecord;

const ACK_TIMEOUT: Duration = Duration::from_millis(750);
// A relay-side ICMP timeout is about 1.5 seconds. Two 900 ms receive windows
// leave enough room for that result while retaining one authenticated retry.
const STATUS_ACK_TIMEOUT: Duration = Duration::from_millis(900);
const MAX_ATTEMPTS: u8 = 3;
const STATUS_MAX_ATTEMPTS: u8 = 2;

#[derive(Debug)]
pub(super) enum DispatchError {
    Network {
        message: String,
        request_id: String,
        attempts: u8,
        latency_ms: u64,
    },
    Timeout {
        request_id: String,
        attempts: u8,
        latency_ms: u64,
    },
    Relay {
        status: AckStatus,
        request_id: String,
        attempts: u8,
        latency_ms: u64,
    },
}

impl DispatchError {
    pub(super) fn request_id(&self) -> &str {
        match self {
            Self::Network { request_id, .. }
            | Self::Timeout { request_id, .. }
            | Self::Relay { request_id, .. } => request_id,
        }
    }

    pub(super) fn attempts(&self) -> u8 {
        match self {
            Self::Network { attempts, .. }
            | Self::Timeout { attempts, .. }
            | Self::Relay { attempts, .. } => *attempts,
        }
    }

    pub(super) fn latency_ms(&self) -> u64 {
        match self {
            Self::Network { latency_ms, .. }
            | Self::Timeout { latency_ms, .. }
            | Self::Relay { latency_ms, .. } => *latency_ms,
        }
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DispatchResult {
    pub request_id: String,
    pub relay_id: Option<String>,
    pub delivery_mode: &'static str,
    pub status: &'static str,
    pub attempts: u8,
    pub latency_ms: u64,
    pub acknowledged_at: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RemoteStatusResult {
    pub state: super::probe::DeviceProbeState,
    pub observed_ip: Option<Ipv4Addr>,
}

pub(super) async fn dispatch(
    relay: &RelayRecord,
    psk: &[u8],
    command: Command,
    target_mac: Option<MacAddress>,
) -> Result<DispatchResult, DispatchError> {
    let request_id = Uuid::new_v4();
    let request_id_string = request_id.to_string();
    let started = time::Instant::now();
    let address = relay.address.parse().map_err(|_| DispatchError::Network {
        message: "relay address is invalid".to_string(),
        request_id: request_id_string.clone(),
        attempts: 0,
        latency_ms: elapsed_ms(started),
    })?;
    let endpoint = SocketAddr::new(address, relay.port);
    let bind = if endpoint.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let socket = UdpSocket::bind(bind)
        .await
        .map_err(|error| DispatchError::Network {
            message: error.to_string(),
            request_id: request_id_string.clone(),
            attempts: 0,
            latency_ms: elapsed_ms(started),
        })?;
    socket
        .connect(endpoint)
        .await
        .map_err(|error| DispatchError::Network {
            message: error.to_string(),
            request_id: request_id_string.clone(),
            attempts: 0,
            latency_ms: elapsed_ms(started),
        })?;

    let relay_id = Uuid::parse_str(&relay.id).map_err(|_| DispatchError::Network {
        message: "relay ID is invalid".to_string(),
        request_id: request_id_string.clone(),
        attempts: 0,
        latency_ms: elapsed_ms(started),
    })?;
    let target_mac_bytes = target_mac.map(|mac| *mac.as_bytes()).unwrap_or([0; 6]);
    let request = RequestPacket {
        command,
        relay_id: *relay_id.as_bytes(),
        key_version: relay.key_version,
        timestamp: unix_seconds(),
        request_id: *request_id.as_bytes(),
        target_mac: target_mac_bytes,
        target_ipv4: [0; 4],
    };
    let packet = encode_request(&request, psk);
    let mut response = [0_u8; PACKET_LEN];

    for attempt in 1..=MAX_ATTEMPTS {
        socket
            .send(&packet)
            .await
            .map_err(|error| DispatchError::Network {
                message: error.to_string(),
                request_id: request_id_string.clone(),
                attempts: attempt,
                latency_ms: elapsed_ms(started),
            })?;
        let valid_ack = time::timeout(ACK_TIMEOUT, async {
            loop {
                let length = match socket.recv(&mut response).await {
                    Ok(length) => length,
                    Err(_) => return None,
                };
                if let Ok(ack) = decode_ack(&response[..length], psk)
                    && ack_matches(&ack, &request)
                {
                    return Some(ack);
                }
            }
        })
        .await
        .ok()
        .flatten();
        if let Some(ack) = valid_ack {
            if ack.status != AckStatus::Ok {
                return Err(DispatchError::Relay {
                    status: ack.status,
                    request_id: request_id_string,
                    attempts: attempt,
                    latency_ms: elapsed_ms(started),
                });
            }
            return Ok(DispatchResult {
                request_id: request_id_string,
                relay_id: Some(relay.id.clone()),
                delivery_mode: "relay",
                status: if command == Command::Wake {
                    "broadcasted"
                } else {
                    "ready"
                },
                attempts: attempt,
                latency_ms: elapsed_ms(started),
                acknowledged_at: crate::time_utils::now_iso(),
            });
        }
    }
    Err(DispatchError::Timeout {
        request_id: request_id_string,
        attempts: MAX_ATTEMPTS,
        latency_ms: elapsed_ms(started),
    })
}

pub(super) async fn dispatch_status(
    relay: &RelayRecord,
    psk: &[u8],
    target_mac: MacAddress,
    preferred_ip: Option<Ipv4Addr>,
) -> Result<RemoteStatusResult, DispatchError> {
    let request_id = Uuid::new_v4();
    let request_id_string = request_id.to_string();
    let started = time::Instant::now();
    let address = relay.address.parse().map_err(|_| DispatchError::Network {
        message: "relay address is invalid".to_string(),
        request_id: request_id_string.clone(),
        attempts: 0,
        latency_ms: elapsed_ms(started),
    })?;
    let endpoint = SocketAddr::new(address, relay.port);
    let socket = UdpSocket::bind(if endpoint.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .await
    .map_err(|error| DispatchError::Network {
        message: error.to_string(),
        request_id: request_id_string.clone(),
        attempts: 0,
        latency_ms: elapsed_ms(started),
    })?;
    socket
        .connect(endpoint)
        .await
        .map_err(|error| DispatchError::Network {
            message: error.to_string(),
            request_id: request_id_string.clone(),
            attempts: 0,
            latency_ms: elapsed_ms(started),
        })?;
    let relay_id = Uuid::parse_str(&relay.id).map_err(|_| DispatchError::Network {
        message: "relay ID is invalid".to_string(),
        request_id: request_id_string.clone(),
        attempts: 0,
        latency_ms: elapsed_ms(started),
    })?;
    let request = RequestPacket {
        command: Command::Status,
        relay_id: *relay_id.as_bytes(),
        key_version: relay.key_version,
        timestamp: unix_seconds(),
        request_id: *request_id.as_bytes(),
        target_mac: *target_mac.as_bytes(),
        target_ipv4: super::probe::ipv4_to_wire(preferred_ip),
    };
    let packet = encode_request(&request, psk);
    let mut response = [0_u8; PACKET_LEN];

    for attempt in 1..=STATUS_MAX_ATTEMPTS {
        socket
            .send(&packet)
            .await
            .map_err(|error| DispatchError::Network {
                message: error.to_string(),
                request_id: request_id_string.clone(),
                attempts: attempt,
                latency_ms: elapsed_ms(started),
            })?;
        let valid_ack = time::timeout(STATUS_ACK_TIMEOUT, async {
            loop {
                let length = socket.recv(&mut response).await.ok()?;
                if let Ok(ack) = decode_ack(&response[..length], psk)
                    && ack_matches(&ack, &request)
                {
                    return Some(ack);
                }
            }
        })
        .await
        .ok()
        .flatten();
        if let Some(ack) = valid_ack {
            let state = match ack.status {
                AckStatus::TargetOnline => super::probe::DeviceProbeState::Online,
                AckStatus::TargetOffline => super::probe::DeviceProbeState::Offline,
                AckStatus::TargetUnknown => super::probe::DeviceProbeState::Unknown,
                status => {
                    return Err(DispatchError::Relay {
                        status,
                        request_id: request_id_string,
                        attempts: attempt,
                        latency_ms: elapsed_ms(started),
                    });
                }
            };
            return Ok(RemoteStatusResult {
                state,
                observed_ip: super::probe::ipv4_from_wire(ack.target_ipv4),
            });
        }
    }
    Err(DispatchError::Timeout {
        request_id: request_id_string,
        attempts: STATUS_MAX_ATTEMPTS,
        latency_ms: elapsed_ms(started),
    })
}

pub(super) async fn dispatch_local(
    target_mac: MacAddress,
    broadcast_address: Option<Ipv4Addr>,
) -> Result<DispatchResult, DispatchError> {
    let mut destinations = broadcast_address
        .into_iter()
        .chain(super::discovery::local_broadcast_addresses())
        .map(|address| SocketAddr::from((address, 9)))
        .collect::<Vec<_>>();
    destinations.sort_unstable();
    destinations.dedup();
    dispatch_local_to(target_mac, &destinations).await
}

async fn dispatch_local_to(
    target_mac: MacAddress,
    destinations: &[SocketAddr],
) -> Result<DispatchResult, DispatchError> {
    let request_id = Uuid::new_v4().to_string();
    let started = time::Instant::now();
    let socket = UdpSocket::bind("0.0.0.0:0")
        .await
        .map_err(|error| DispatchError::Network {
            message: error.to_string(),
            request_id: request_id.clone(),
            attempts: 0,
            latency_ms: elapsed_ms(started),
        })?;
    socket
        .set_broadcast(true)
        .map_err(|error| DispatchError::Network {
            message: error.to_string(),
            request_id: request_id.clone(),
            attempts: 0,
            latency_ms: elapsed_ms(started),
        })?;
    let packet = magic_packet(target_mac);
    let mut delivered = 0_u8;
    let mut last_error = None;
    for destination in destinations {
        match socket.send_to(&packet, destination).await {
            Ok(length) if length == packet.len() => delivered = delivered.saturating_add(1),
            Ok(length) => {
                last_error = Some(format!("only {length} of {} bytes were sent", packet.len()))
            }
            Err(error) => last_error = Some(error.to_string()),
        }
    }
    if delivered == 0 {
        return Err(DispatchError::Network {
            message: last_error
                .unwrap_or_else(|| "no local broadcast destination is available".to_string()),
            request_id,
            attempts: 1,
            latency_ms: elapsed_ms(started),
        });
    }
    Ok(DispatchResult {
        request_id,
        relay_id: None,
        delivery_mode: "local",
        status: "broadcasted",
        attempts: 1,
        latency_ms: elapsed_ms(started),
        acknowledged_at: crate::time_utils::now_iso(),
    })
}

fn elapsed_ms(started: time::Instant) -> u64 {
    started.elapsed().as_millis().try_into().unwrap_or(u64::MAX)
}

fn ack_matches(ack: &AckPacket, request: &RequestPacket) -> bool {
    ack.command == request.command
        && ack.relay_id == request.relay_id
        && ack.key_version == request.key_version
        && ack.request_id == request.request_id
        && ack.target_mac == request.target_mac
        && (request.command == Command::Status || ack.target_ipv4 == request.target_ipv4)
}

fn unix_seconds() -> u64 {
    (crate::time_utils::now_ms().max(0) / 1000) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use fn_knock_wol_protocol::{AckPacket, decode_request, encode_ack};

    fn relay(port: u16) -> RelayRecord {
        RelayRecord {
            id: "11111111-1111-4111-8111-111111111111".to_string(),
            name: "test relay".to_string(),
            address: "127.0.0.1".to_string(),
            port,
            enabled: true,
            key_version: 7,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    fn ack_for(request: &RequestPacket, status: AckStatus) -> AckPacket {
        AckPacket {
            command: request.command,
            status,
            relay_id: request.relay_id,
            key_version: request.key_version,
            timestamp: unix_seconds(),
            request_id: request.request_id,
            target_mac: request.target_mac,
            target_ipv4: request.target_ipv4,
        }
    }

    #[test]
    fn ack_must_match_request_identity() {
        let request = RequestPacket {
            command: Command::Probe,
            relay_id: [1; 16],
            key_version: 1,
            timestamp: 1,
            request_id: [2; 16],
            target_mac: [0; 6],
            target_ipv4: [0; 4],
        };
        let mut ack = AckPacket {
            command: request.command,
            status: AckStatus::Ok,
            relay_id: request.relay_id,
            key_version: request.key_version,
            timestamp: 2,
            request_id: request.request_id,
            target_mac: request.target_mac,
            target_ipv4: request.target_ipv4,
        };
        assert!(ack_matches(&ack, &request));
        ack.request_id[0] ^= 1;
        assert!(!ack_matches(&ack, &request));
        assert_eq!(encode_ack(&ack, &[3; 32]).len(), PACKET_LEN);
    }

    #[tokio::test]
    async fn accepts_only_authenticated_ack_from_connected_endpoint() {
        let psk = [9_u8; 32];
        let listener = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let configured_port = listener.local_addr().unwrap().port();
        let spoof = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        tokio::spawn(async move {
            let mut buffer = [0_u8; PACKET_LEN];
            let (length, peer) = listener.recv_from(&mut buffer).await.unwrap();
            let request = decode_request(&buffer[..length], &psk).unwrap();
            let forged = encode_ack(&ack_for(&request, AckStatus::BroadcastFailed), &psk);
            spoof.send_to(&forged, peer).await.unwrap();
            let wrong_signature = encode_ack(&ack_for(&request, AckStatus::Ok), &[8_u8; 32]);
            listener.send_to(&wrong_signature, peer).await.unwrap();
            let valid = encode_ack(&ack_for(&request, AckStatus::Ok), &psk);
            listener.send_to(&valid, peer).await.unwrap();
        });

        let result = dispatch(&relay(configured_port), &psk, Command::Probe, None)
            .await
            .unwrap();
        assert_eq!(result.status, "ready");
        assert_eq!(result.attempts, 1);
    }

    #[tokio::test]
    async fn local_delivery_sends_magic_packet_without_a_relay() {
        let receiver = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let mac = "02:11:22:33:44:55".parse::<MacAddress>().unwrap();
        let result = dispatch_local_to(mac, &[receiver.local_addr().unwrap()])
            .await
            .unwrap();
        assert_eq!(result.delivery_mode, "local");
        assert_eq!(result.relay_id, None);

        let mut packet = [0_u8; 103];
        let (length, _) = time::timeout(Duration::from_secs(1), receiver.recv_from(&mut packet))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(length, 102);
        assert_eq!(&packet[..6], &[0xff; 6]);
        assert_eq!(&packet[6..12], mac.as_bytes());
    }

    #[tokio::test]
    async fn maps_authenticated_relay_failure_and_timeout() {
        let psk = [7_u8; 32];
        let listener = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let configured_port = listener.local_addr().unwrap().port();
        tokio::spawn(async move {
            let mut buffer = [0_u8; PACKET_LEN];
            let (length, peer) = listener.recv_from(&mut buffer).await.unwrap();
            let request = decode_request(&buffer[..length], &psk).unwrap();
            let failed = encode_ack(&ack_for(&request, AckStatus::BroadcastFailed), &psk);
            listener.send_to(&failed, peer).await.unwrap();
        });
        assert!(matches!(
            dispatch(
                &relay(configured_port),
                &psk,
                Command::Wake,
                "02:11:22:33:44:55".parse().ok()
            )
            .await,
            Err(DispatchError::Relay {
                status: AckStatus::BroadcastFailed,
                ..
            })
        ));

        let unused = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let unused_port = unused.local_addr().unwrap().port();
        assert!(matches!(
            dispatch(&relay(unused_port), &psk, Command::Probe, None).await,
            Err(DispatchError::Timeout { attempts: 3, .. })
                | Err(DispatchError::Network {
                    attempts: 1..=3,
                    ..
                })
        ));
    }
}
