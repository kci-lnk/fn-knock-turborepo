use fn_knock_wol_protocol::MacAddress;
use std::{
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    time::Duration,
};
use tokio::time;

use super::discovery::{probe_host, read_neighbor_table_checked};

const DEVICE_PROBE_TIMEOUT: Duration = Duration::from_millis(1_650);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DeviceProbeState {
    Online,
    Offline,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DeviceProbeResult {
    pub state: DeviceProbeState,
    pub observed_ip: Option<Ipv4Addr>,
    pub error: Option<String>,
}

pub(super) async fn probe_device_bounded(
    mac: MacAddress,
    preferred_ip: Option<Ipv4Addr>,
) -> DeviceProbeResult {
    probe_device_candidates_bounded(mac, preferred_ip).await
}

pub(super) async fn probe_device_candidates_bounded(
    mac: MacAddress,
    preferred_ips: impl IntoIterator<Item = Ipv4Addr>,
) -> DeviceProbeResult {
    let preferred_ips = preferred_ips.into_iter().collect::<Vec<_>>();
    let fallback_ip = preferred_ips.first().copied();
    match time::timeout(
        DEVICE_PROBE_TIMEOUT,
        probe_device_candidates(mac, preferred_ips),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => probe_timeout_result(fallback_ip),
    }
}

pub(super) async fn probe_device_candidates(
    mac: MacAddress,
    preferred_ips: impl IntoIterator<Item = Ipv4Addr>,
) -> DeviceProbeResult {
    let before = read_neighbor_table_checked().await;
    let expected_mac = mac.to_string();
    let empty_neighbors = HashMap::new();
    let candidates = ordered_candidates(
        preferred_ips,
        before.as_ref().unwrap_or(&empty_neighbors),
        &expected_mac,
    );

    if candidates.is_empty() {
        return DeviceProbeResult {
            state: DeviceProbeState::Unknown,
            observed_ip: None,
            error: Some(
                before
                    .err()
                    .unwrap_or_else(|| "No IPv4 address is available for this target".to_string()),
            ),
        };
    }
    let fallback_ip = candidates.first().copied();

    let mut last_error = None;
    let mut completed_probe = false;
    let mut probe_failed = false;
    for ip in candidates {
        match probe_host(ip).await {
            Ok(false) => completed_probe = true,
            Ok(true) => {
                let after = match read_neighbor_table_checked().await {
                    Ok(after) => after,
                    Err(error) => {
                        probe_failed = true;
                        last_error = Some(error);
                        continue;
                    }
                };
                completed_probe = true;
                let mac_matches = after.get(&ip) == Some(&expected_mac);
                if mac_matches {
                    return DeviceProbeResult {
                        state: DeviceProbeState::Online,
                        observed_ip: Some(ip),
                        error: None,
                    };
                }
            }
            Err(error) => {
                probe_failed = true;
                last_error = Some(error);
            }
        }
    }

    DeviceProbeResult {
        state: if probe_failed {
            DeviceProbeState::Unknown
        } else if completed_probe {
            DeviceProbeState::Offline
        } else {
            DeviceProbeState::Unknown
        },
        observed_ip: fallback_ip,
        error: last_error,
    }
}

fn probe_timeout_result(fallback_ip: Option<Ipv4Addr>) -> DeviceProbeResult {
    DeviceProbeResult {
        state: DeviceProbeState::Unknown,
        observed_ip: fallback_ip,
        error: Some("Device probe pipeline timed out".to_string()),
    }
}

fn ordered_candidates(
    preferred_ips: impl IntoIterator<Item = Ipv4Addr>,
    neighbors: &std::collections::HashMap<Ipv4Addr, String>,
    expected_mac: &str,
) -> Vec<Ipv4Addr> {
    let mut seen = HashSet::new();
    preferred_ips
        .into_iter()
        .chain(
            neighbors
                .iter()
                .filter_map(|(ip, mac)| (mac == expected_mac).then_some(*ip)),
        )
        .filter(|ip| seen.insert(*ip))
        .collect()
}

pub(super) fn ipv4_to_wire(value: Option<Ipv4Addr>) -> [u8; 4] {
    value.map(|address| address.octets()).unwrap_or([0; 4])
}

pub(super) fn ipv4_from_wire(value: [u8; 4]) -> Option<Ipv4Addr> {
    (value != [0; 4]).then(|| Ipv4Addr::from(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn configured_and_observed_addresses_precede_neighbor_candidates() {
        let configured = Ipv4Addr::new(192, 0, 2, 10);
        let observed = Ipv4Addr::new(192, 0, 2, 11);
        let neighbor = Ipv4Addr::new(192, 0, 2, 12);
        let duplicate = configured;
        let neighbors = HashMap::from([
            (neighbor, "02:11:22:33:44:55".to_string()),
            (duplicate, "02:11:22:33:44:55".to_string()),
            (
                Ipv4Addr::new(192, 0, 2, 99),
                "02:aa:bb:cc:dd:ee".to_string(),
            ),
        ]);

        assert_eq!(
            ordered_candidates([configured, observed], &neighbors, "02:11:22:33:44:55",),
            vec![configured, observed, neighbor]
        );
    }

    #[test]
    fn bounded_probe_timeout_is_unknown_instead_of_offline() {
        let fallback = Ipv4Addr::new(192, 0, 2, 10);
        let timeout = probe_timeout_result(Some(fallback));
        assert_eq!(timeout.state, DeviceProbeState::Unknown);
        assert_eq!(timeout.observed_ip, Some(fallback));
    }
}
