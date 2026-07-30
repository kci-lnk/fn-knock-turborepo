use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(crate) const COMPILED_IP_SET_FORMAT_VERSION: u32 = 1;
const DIGEST_DOMAIN: &[u8] = b"fnknock-ipset-v1\0";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct CompiledIpSet {
    pub(crate) id: String,
    pub(crate) format_version: u32,
    pub(crate) ipv4_ranges: Vec<u8>,
    pub(crate) ipv6_ranges: Vec<u8>,
    pub(crate) source_cidr_count: usize,
}

impl CompiledIpSet {
    pub(crate) fn ipv4_range_count(&self) -> usize {
        self.ipv4_ranges.len() / 8
    }

    pub(crate) fn ipv6_range_count(&self) -> usize {
        self.ipv6_ranges.len() / 32
    }

    pub(crate) fn range_count(&self) -> usize {
        self.ipv4_range_count() + self.ipv6_range_count()
    }

    pub(crate) fn to_config_value(&self) -> Value {
        json!({
            "format_version": self.format_version,
            "ipv4_ranges": URL_SAFE_NO_PAD.encode(&self.ipv4_ranges),
            "ipv6_ranges": URL_SAFE_NO_PAD.encode(&self.ipv6_ranges),
        })
    }

    pub(crate) fn from_config_value(id: &str, value: &Value) -> Result<Self, String> {
        let format_version = value
            .get("format_version")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or_default();
        if format_version != COMPILED_IP_SET_FORMAT_VERSION {
            return Err(format!(
                "unsupported compiled IP set format version {format_version}"
            ));
        }
        let ipv4_ranges = decode_ranges(value, "ipv4_ranges", 8)?;
        let ipv6_ranges = decode_ranges(value, "ipv6_ranges", 32)?;
        validate_sorted_ranges(&ipv4_ranges, 4)?;
        validate_sorted_ranges(&ipv6_ranges, 16)?;
        let expected_id = policy_id(&ipv4_ranges, &ipv6_ranges);
        if id != expected_id {
            return Err(format!(
                "compiled IP set digest mismatch: expected {expected_id}, got {id}"
            ));
        }
        Ok(Self {
            id: id.to_string(),
            format_version,
            ipv4_ranges,
            ipv6_ranges,
            source_cidr_count: 0,
        })
    }
}

pub(crate) fn compile_ip_set<I, S>(cidrs: I) -> Result<CompiledIpSet, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    let mut source_cidr_count = 0usize;
    for raw in cidrs {
        let raw = raw.as_ref().trim();
        if raw.is_empty() {
            continue;
        }
        let network = raw
            .parse::<IpNet>()
            .map_err(|error| format!("invalid CIDR {raw:?}: {error}"))?
            .trunc();
        source_cidr_count += 1;
        match network {
            IpNet::V4(network) => {
                let start = u32::from(network.network());
                let host_bits = 32 - network.prefix_len();
                let end = if host_bits == 32 {
                    u32::MAX
                } else {
                    start | ((1u32 << host_bits) - 1)
                };
                v4.push((start, end));
            }
            IpNet::V6(network) => {
                let start = u128::from(network.network());
                let host_bits = 128 - network.prefix_len();
                let end = if host_bits == 128 {
                    u128::MAX
                } else {
                    start | ((1u128 << host_bits) - 1)
                };
                v6.push((start, end));
            }
        }
    }

    let v4 = merge_ranges(v4);
    let v6 = merge_ranges(v6);
    let mut ipv4_ranges = Vec::with_capacity(v4.len() * 8);
    for (start, end) in v4 {
        ipv4_ranges.extend_from_slice(&start.to_be_bytes());
        ipv4_ranges.extend_from_slice(&end.to_be_bytes());
    }
    let mut ipv6_ranges = Vec::with_capacity(v6.len() * 32);
    for (start, end) in v6 {
        ipv6_ranges.extend_from_slice(&start.to_be_bytes());
        ipv6_ranges.extend_from_slice(&end.to_be_bytes());
    }
    Ok(CompiledIpSet {
        id: policy_id(&ipv4_ranges, &ipv6_ranges),
        format_version: COMPILED_IP_SET_FORMAT_VERSION,
        ipv4_ranges,
        ipv6_ranges,
        source_cidr_count,
    })
}

fn merge_ranges<T>(mut ranges: Vec<(T, T)>) -> Vec<(T, T)>
where
    T: Copy + Ord + num_traits::CheckedAdd + num_traits::One,
{
    ranges.sort_unstable();
    let mut merged: Vec<(T, T)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges {
        if let Some((_, previous_end)) = merged.last_mut() {
            let adjacent_or_overlapping = start <= *previous_end
                || previous_end
                    .checked_add(&T::one())
                    .is_some_and(|next| start <= next);
            if adjacent_or_overlapping {
                if end > *previous_end {
                    *previous_end = end;
                }
                continue;
            }
        }
        merged.push((start, end));
    }
    merged
}

fn policy_id(ipv4_ranges: &[u8], ipv6_ranges: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(DIGEST_DOMAIN);
    hasher.update(((ipv4_ranges.len() / 8) as u32).to_be_bytes());
    hasher.update(ipv4_ranges);
    hasher.update(((ipv6_ranges.len() / 32) as u32).to_be_bytes());
    hasher.update(ipv6_ranges);
    format!("ipset-v1:{}", hex::encode(hasher.finalize()))
}

fn decode_ranges(value: &Value, field: &str, stride: usize) -> Result<Vec<u8>, String> {
    let encoded = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("compiled IP set {field} is missing"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|error| format!("compiled IP set {field} is invalid base64url: {error}"))?;
    if decoded.len() % stride != 0 {
        return Err(format!(
            "compiled IP set {field} length {} is not divisible by {stride}",
            decoded.len()
        ));
    }
    Ok(decoded)
}

fn validate_sorted_ranges(bytes: &[u8], width: usize) -> Result<(), String> {
    let stride = width * 2;
    let mut previous_end: Option<&[u8]> = None;
    for pair in bytes.chunks_exact(stride) {
        let (start, end) = pair.split_at(width);
        if start > end {
            return Err("compiled IP set contains a reversed range".to_string());
        }
        if previous_end.is_some_and(|previous| previous >= start) {
            return Err("compiled IP set ranges are not sorted and disjoint".to_string());
        }
        previous_end = Some(end);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn compiles_exact_merged_ipv4_and_ipv6_ranges() {
        let compiled = compile_ip_set([
            "192.0.2.128/25",
            "192.0.2.0/25",
            "192.0.2.0/24",
            "2001:db8::/127",
            "2001:db8::2/127",
        ])
        .unwrap();
        assert_eq!(compiled.source_cidr_count, 5);
        assert_eq!(compiled.ipv4_range_count(), 1);
        assert_eq!(compiled.ipv6_range_count(), 1);
        assert_eq!(
            compiled.ipv4_ranges,
            [
                Ipv4Addr::new(192, 0, 2, 0).octets(),
                Ipv4Addr::new(192, 0, 2, 255).octets()
            ]
            .concat()
        );
        assert_eq!(
            compiled.ipv6_ranges,
            [
                Ipv6Addr::from(0x20010db8000000000000000000000000u128).octets(),
                Ipv6Addr::from(0x20010db8000000000000000000000003u128).octets()
            ]
            .concat()
        );
    }

    #[test]
    fn compiles_full_address_spaces_without_overflow() {
        let compiled = compile_ip_set(["0.0.0.0/0", "::/0"]).unwrap();
        assert_eq!(&compiled.ipv4_ranges[..4], &[0; 4]);
        assert_eq!(&compiled.ipv4_ranges[4..], &[255; 4]);
        assert_eq!(&compiled.ipv6_ranges[..16], &[0; 16]);
        assert_eq!(&compiled.ipv6_ranges[16..], &[255; 16]);
    }

    #[test]
    fn config_round_trip_validates_content_digest() {
        let compiled = compile_ip_set(["203.0.113.0/24", "2001:db8::/32"]).unwrap();
        let decoded =
            CompiledIpSet::from_config_value(&compiled.id, &compiled.to_config_value()).unwrap();
        assert_eq!(decoded.id, compiled.id);
        assert_eq!(decoded.ipv4_ranges, compiled.ipv4_ranges);
        assert_eq!(decoded.ipv6_ranges, compiled.ipv6_ranges);
    }

    #[test]
    fn policy_id_is_deterministic_for_equivalent_sets() {
        let left = compile_ip_set(["192.0.2.0/25", "192.0.2.128/25"]).unwrap();
        let right = compile_ip_set(["192.0.2.0/24"]).unwrap();
        assert_eq!(left.id, right.id);
        assert_eq!(left.ipv4_ranges, right.ipv4_ranges);
    }

    #[test]
    fn matches_cross_language_golden_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../packages/grpc-contracts/testdata/ipset-v1-golden.json"
        ))
        .unwrap();
        let cidrs = fixture["cidrs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let compiled = compile_ip_set(cidrs).unwrap();
        assert_eq!(compiled.id, fixture["id"].as_str().unwrap());
        assert_eq!(
            compiled.format_version as u64,
            fixture["format_version"].as_u64().unwrap()
        );
        assert_eq!(
            URL_SAFE_NO_PAD.encode(compiled.ipv4_ranges),
            fixture["ipv4_ranges"].as_str().unwrap()
        );
        assert_eq!(
            URL_SAFE_NO_PAD.encode(compiled.ipv6_ranges),
            fixture["ipv6_ranges"].as_str().unwrap()
        );
    }
}
