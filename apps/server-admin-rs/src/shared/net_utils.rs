use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, Ipv6Addr},
};

use get_if_addrs::{IfAddr, Interface, get_if_addrs};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PrivateIpv4Candidate {
    pub(crate) interface: String,
    pub(crate) address: Ipv4Addr,
    pub(crate) netmask: Ipv4Addr,
    pub(crate) prefix: Option<u8>,
}

pub(crate) fn ipv4_prefix_len(mask: Ipv4Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

pub(crate) fn ipv6_prefix_len(mask: Ipv6Addr) -> u32 {
    mask.octets().iter().map(|byte| byte.count_ones()).sum()
}

pub(crate) fn ipv4_prefix_len_checked(mask: Ipv4Addr) -> Option<u8> {
    let mask = u32::from(mask);
    let prefix = mask.leading_ones() as u8;
    let expected = if prefix == 0 {
        0
    } else {
        u32::MAX.checked_shl((32 - prefix) as u32).unwrap_or(0)
    };
    (mask == expected).then_some(prefix)
}

pub(crate) fn is_private_ipv4(ip: Ipv4Addr) -> bool {
    let [first, second, _, _] = ip.octets();
    first == 10 || (first == 172 && (16..=31).contains(&second)) || (first == 192 && second == 168)
}

pub(crate) fn is_excluded_local_interface(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    lower == "lo"
        || lower.starts_with("docker")
        || is_docker_generated_bridge_name(&lower)
        || lower.starts_with("veth")
        || lower.starts_with("tailscale")
        || lower.starts_with("zt")
        || lower.starts_with("tun")
        || lower.starts_with("tap")
        || lower.starts_with("wg")
        || lower.starts_with("gre")
        || lower.starts_with("ipip")
        || lower.starts_with("sit")
        || lower.starts_with("vxlan")
        || lower.starts_with("genev")
        || lower.starts_with("erspan")
        || lower.starts_with("ip6tnl")
        || lower.starts_with("ip6gre")
}

fn is_docker_generated_bridge_name(name: &str) -> bool {
    let Some(suffix) = name.strip_prefix("br-") else {
        return false;
    };
    (12..=64).contains(&suffix.len())
        && suffix
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

pub(crate) fn list_private_ipv4_candidates() -> Vec<PrivateIpv4Candidate> {
    get_if_addrs()
        .map(collect_private_ipv4_candidates)
        .unwrap_or_default()
}

fn collect_private_ipv4_candidates(
    interfaces: impl IntoIterator<Item = Interface>,
) -> Vec<PrivateIpv4Candidate> {
    let mut output = Vec::new();
    for interface in interfaces {
        if interface.is_loopback() || is_excluded_local_interface(&interface.name) {
            continue;
        }
        let IfAddr::V4(address) = interface.addr else {
            continue;
        };
        if !is_private_ipv4(address.ip) {
            continue;
        }
        output.push(PrivateIpv4Candidate {
            interface: interface.name,
            address: address.ip,
            netmask: address.netmask,
            prefix: ipv4_prefix_len_checked(address.netmask),
        });
    }
    output.sort_by(|left, right| {
        left.interface
            .cmp(&right.interface)
            .then_with(|| left.address.cmp(&right.address))
    });
    let mut seen = BTreeSet::new();
    output.retain(|candidate| seen.insert(candidate.address));
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ipv4_interface(name: &str, address: [u8; 4], netmask: [u8; 4]) -> Interface {
        Interface {
            name: name.to_string(),
            addr: IfAddr::V4(get_if_addrs::Ifv4Addr {
                ip: Ipv4Addr::from(address),
                netmask: Ipv4Addr::from(netmask),
                prefixlen: ipv4_prefix_len(Ipv4Addr::from(netmask)) as u8,
                broadcast: None,
            }),
            index: None,
            oper_status: get_if_addrs::IfOperStatus::Up,
            is_p2p: false,
            #[cfg(windows)]
            adapter_name: String::new(),
        }
    }

    #[test]
    fn counts_ipv4_and_ipv6_mask_bits_like_existing_callers() {
        assert_eq!(ipv4_prefix_len(Ipv4Addr::new(255, 255, 255, 0)), 24);
        assert_eq!(
            ipv6_prefix_len("ffff:ffff:ffff:ffff::".parse::<Ipv6Addr>().unwrap()),
            64
        );
    }

    #[test]
    fn validates_ipv4_prefix_masks() {
        assert_eq!(
            ipv4_prefix_len_checked(Ipv4Addr::new(255, 255, 254, 0)),
            Some(23)
        );
        assert_eq!(ipv4_prefix_len_checked(Ipv4Addr::new(255, 0, 255, 0)), None);
    }

    #[test]
    fn recognizes_only_rfc1918_ipv4_addresses() {
        for address in ["10.0.0.1", "172.16.0.1", "172.31.255.254", "192.168.1.1"] {
            assert!(is_private_ipv4(address.parse().unwrap()), "{address}");
        }
        for address in [
            "127.0.0.1",
            "100.64.0.1",
            "169.254.1.1",
            "172.32.0.1",
            "8.8.8.8",
        ] {
            assert!(!is_private_ipv4(address.parse().unwrap()), "{address}");
        }
    }

    #[test]
    fn excludes_ephemeral_interfaces_but_keeps_lan_bridges() {
        for name in ["br0", "br-lan", "bond0", "en0", "eth0", "ovs-system"] {
            assert!(!is_excluded_local_interface(name), "{name}");
        }
        for name in [
            "lo",
            "docker0",
            "br-0123456789ab",
            "veth1234",
            "tailscale0",
            "wg0",
            "gre0",
            "gretap0",
            "ipip0",
            "sit0",
            "vxlan100",
            "genev_sys_6081",
            "erspan0",
            "ip6tnl0",
            "ip6gre0",
        ] {
            assert!(is_excluded_local_interface(name), "{name}");
        }
    }

    #[test]
    fn private_candidates_filter_dedupe_and_sort_stably() {
        let candidates = collect_private_ipv4_candidates([
            ipv4_interface("en1", [192, 168, 1, 20], [255, 255, 255, 0]),
            ipv4_interface("en0", [192, 168, 1, 10], [255, 255, 254, 0]),
            ipv4_interface("en0", [192, 168, 1, 2], [255, 255, 255, 0]),
            ipv4_interface("bond0", [192, 168, 1, 20], [255, 255, 255, 0]),
            ipv4_interface("docker0", [172, 17, 0, 1], [255, 255, 0, 0]),
            ipv4_interface("en2", [100, 64, 0, 1], [255, 192, 0, 0]),
        ]);

        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (
                    candidate.interface.as_str(),
                    candidate.address,
                    candidate.prefix
                ))
                .collect::<Vec<_>>(),
            vec![
                ("bond0", Ipv4Addr::new(192, 168, 1, 20), Some(24)),
                ("en0", Ipv4Addr::new(192, 168, 1, 2), Some(24)),
                ("en0", Ipv4Addr::new(192, 168, 1, 10), Some(23)),
            ]
        );
    }
}
