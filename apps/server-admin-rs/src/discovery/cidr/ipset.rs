use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    sync::{Arc, Mutex},
};

use arc_swap::ArcSwap;

const LEGACY_COMPILED_IP_SET_FORMAT_VERSION: u32 = 1;
pub(crate) const COMPILED_IP_SET_FORMAT_VERSION: u32 = 2;
const V1_DIGEST_DOMAIN: &[u8] = b"fnknock-ipset-v1\0";
const V2_DIGEST_DOMAIN: &[u8] = b"fnknock-ipset-v2\0";
const MAX_CANONICAL_RANGE_BYTES: u64 = 64 * 1024 * 1024;

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
        let (ipv4_ranges, ipv6_ranges) = self
            .encoded_ranges()
            .expect("canonical compiled IP ranges must be encodable");
        json!({
            "format_version": self.format_version,
            "ipv4_ranges": URL_SAFE_NO_PAD.encode(ipv4_ranges),
            "ipv6_ranges": URL_SAFE_NO_PAD.encode(ipv6_ranges),
        })
    }

    pub(crate) fn to_transport_value(&self) -> Value {
        let mut value = self
            .to_config_value()
            .as_object()
            .cloned()
            .unwrap_or_default();
        value.insert("id".to_string(), Value::String(self.id.clone()));
        value.insert(
            "source_cidr_count".to_string(),
            json!(self.source_cidr_count),
        );
        value.insert("range_count".to_string(), json!(self.range_count()));
        Value::Object(value)
    }

    pub(crate) fn from_transport_value(value: &Value) -> Result<Self, String> {
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "compiled IP set id is missing".to_string())?;
        let mut policy = Self::from_config_value(id, value)?;
        policy.source_cidr_count = value
            .get("source_cidr_count")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_default();
        Ok(policy)
    }

    pub(crate) fn from_config_value(id: &str, value: &Value) -> Result<Self, String> {
        let format_version = value
            .get("format_version")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or_default();
        let encoded_ipv4 = decode_base64url(value, "ipv4_ranges")?;
        let encoded_ipv6 = decode_base64url(value, "ipv6_ranges")?;
        let (ipv4_ranges, ipv6_ranges) = match format_version {
            LEGACY_COMPILED_IP_SET_FORMAT_VERSION => (encoded_ipv4, encoded_ipv6),
            COMPILED_IP_SET_FORMAT_VERSION => (
                decompress_ranges(&encoded_ipv4, "ipv4_ranges", 8)?,
                decompress_ranges(&encoded_ipv6, "ipv6_ranges", 32)?,
            ),
            _ => {
                return Err(format!(
                    "unsupported compiled IP set format version {format_version}"
                ));
            }
        };
        validate_range_length(&ipv4_ranges, "ipv4_ranges", 8)?;
        validate_range_length(&ipv6_ranges, "ipv6_ranges", 32)?;
        validate_sorted_ranges(&ipv4_ranges, 4)?;
        validate_sorted_ranges(&ipv6_ranges, 16)?;
        let expected_id = policy_id(format_version, &ipv4_ranges, &ipv6_ranges);
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

    /// Re-encodes a validated legacy policy without re-fetching or expanding
    /// the original CIDR source list.
    pub(crate) fn into_current_format(mut self) -> Self {
        self.format_version = COMPILED_IP_SET_FORMAT_VERSION;
        self.id = policy_id(
            COMPILED_IP_SET_FORMAT_VERSION,
            &self.ipv4_ranges,
            &self.ipv6_ranges,
        );
        self
    }

    fn encoded_ranges(&self) -> Result<(Vec<u8>, Vec<u8>), String> {
        match self.format_version {
            LEGACY_COMPILED_IP_SET_FORMAT_VERSION => {
                Ok((self.ipv4_ranges.clone(), self.ipv6_ranges.clone()))
            }
            COMPILED_IP_SET_FORMAT_VERSION => Ok((
                compress_ranges(&self.ipv4_ranges)?,
                compress_ranges(&self.ipv6_ranges)?,
            )),
            version => Err(format!(
                "unsupported compiled IP set format version {version}"
            )),
        }
    }

    pub(crate) fn contains(&self, address: IpAddr) -> bool {
        match address {
            IpAddr::V4(address) => contains_packed_range(&self.ipv4_ranges, &address.octets(), 4),
            IpAddr::V6(address) => contains_packed_range(&self.ipv6_ranges, &address.octets(), 16),
        }
    }

    pub(crate) fn contains_cidr(&self, cidr: &str) -> bool {
        let Ok(network) = cidr.trim().parse::<IpNet>().map(|network| network.trunc()) else {
            return false;
        };
        match network {
            IpNet::V4(network) => {
                let start = u32::from(network.network());
                let host_bits = 32 - network.prefix_len();
                let end = if host_bits == 32 {
                    u32::MAX
                } else {
                    start | ((1u32 << host_bits) - 1)
                };
                contains_packed_cidr_range(
                    &self.ipv4_ranges,
                    &start.to_be_bytes(),
                    &end.to_be_bytes(),
                    4,
                )
            }
            IpNet::V6(network) => {
                let start = u128::from(network.network());
                let host_bits = 128 - network.prefix_len();
                let end = if host_bits == 128 {
                    u128::MAX
                } else {
                    start | ((1u128 << host_bits) - 1)
                };
                contains_packed_cidr_range(
                    &self.ipv6_ranges,
                    &start.to_be_bytes(),
                    &end.to_be_bytes(),
                    16,
                )
            }
        }
    }

    /// Converts canonical disjoint ranges into the smallest exact CIDR cover.
    /// This is intentionally kept off request paths and is used only where an
    /// external firewall API still requires textual prefixes.
    pub(crate) fn to_cidrs(&self) -> Vec<String> {
        let mut cidrs = Vec::new();
        for pair in self.ipv4_ranges.chunks_exact(8) {
            let start = u32::from_be_bytes(pair[..4].try_into().expect("four byte IPv4 start"));
            let end = u32::from_be_bytes(pair[4..].try_into().expect("four byte IPv4 end"));
            append_ipv4_cidrs(start, end, &mut cidrs);
        }
        for pair in self.ipv6_ranges.chunks_exact(32) {
            let start =
                u128::from_be_bytes(pair[..16].try_into().expect("sixteen byte IPv6 start"));
            let end = u128::from_be_bytes(pair[16..].try_into().expect("sixteen byte IPv6 end"));
            append_ipv6_cidrs(start, end, &mut cidrs);
        }
        cidrs
    }
}

#[derive(Clone)]
pub(crate) struct IpSetRegistry {
    snapshot: Arc<ArcSwap<HashMap<String, Arc<CompiledIpSet>>>>,
    write_lock: Arc<Mutex<()>>,
}

impl Default for IpSetRegistry {
    fn default() -> Self {
        Self {
            snapshot: Arc::new(ArcSwap::from_pointee(HashMap::new())),
            write_lock: Arc::new(Mutex::new(())),
        }
    }
}

impl IpSetRegistry {
    pub(crate) fn get(&self, key: &str) -> Option<Arc<CompiledIpSet>> {
        self.snapshot.load().get(key).cloned()
    }

    pub(crate) fn publish(&self, key: impl Into<String>, policy: Option<CompiledIpSet>) {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut next = (**self.snapshot.load()).clone();
        let key = key.into();
        if let Some(policy) = policy {
            next.insert(key, Arc::new(policy));
        } else {
            next.remove(&key);
        }
        self.snapshot.store(Arc::new(next));
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

    Ok(compiled_from_ranges(v4, v6, source_cidr_count))
}

pub(crate) fn union_ip_sets<'a, I>(sets: I) -> CompiledIpSet
where
    I: IntoIterator<Item = &'a CompiledIpSet>,
{
    let mut v4 = Vec::new();
    let mut v6 = Vec::new();
    let mut source_cidr_count = 0usize;
    for set in sets {
        source_cidr_count = source_cidr_count.saturating_add(set.source_cidr_count);
        for pair in set.ipv4_ranges.chunks_exact(8) {
            v4.push((
                u32::from_be_bytes(pair[..4].try_into().expect("four byte IPv4 start")),
                u32::from_be_bytes(pair[4..].try_into().expect("four byte IPv4 end")),
            ));
        }
        for pair in set.ipv6_ranges.chunks_exact(32) {
            v6.push((
                u128::from_be_bytes(pair[..16].try_into().expect("sixteen byte IPv6 start")),
                u128::from_be_bytes(pair[16..].try_into().expect("sixteen byte IPv6 end")),
            ));
        }
    }
    compiled_from_ranges(v4, v6, source_cidr_count)
}

fn compiled_from_ranges(
    v4: Vec<(u32, u32)>,
    v6: Vec<(u128, u128)>,
    source_cidr_count: usize,
) -> CompiledIpSet {
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
    CompiledIpSet {
        id: policy_id(COMPILED_IP_SET_FORMAT_VERSION, &ipv4_ranges, &ipv6_ranges),
        format_version: COMPILED_IP_SET_FORMAT_VERSION,
        ipv4_ranges,
        ipv6_ranges,
        source_cidr_count,
    }
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

fn policy_id(format_version: u32, ipv4_ranges: &[u8], ipv6_ranges: &[u8]) -> String {
    let (domain, prefix) = match format_version {
        LEGACY_COMPILED_IP_SET_FORMAT_VERSION => (V1_DIGEST_DOMAIN, "ipset-v1:"),
        COMPILED_IP_SET_FORMAT_VERSION => (V2_DIGEST_DOMAIN, "ipset-v2:"),
        _ => unreachable!("policy ID requested for unsupported format"),
    };
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(((ipv4_ranges.len() / 8) as u32).to_be_bytes());
    hasher.update(ipv4_ranges);
    hasher.update(((ipv6_ranges.len() / 32) as u32).to_be_bytes());
    hasher.update(ipv6_ranges);
    format!("{prefix}{}", hex::encode(hasher.finalize()))
}

fn decode_base64url(value: &Value, field: &str) -> Result<Vec<u8>, String> {
    let encoded = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("compiled IP set {field} is missing"))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(encoded)
        .map_err(|error| format!("compiled IP set {field} is invalid base64url: {error}"))?;
    Ok(decoded)
}

fn validate_range_length(decoded: &[u8], field: &str, stride: usize) -> Result<(), String> {
    if !decoded.len().is_multiple_of(stride) {
        return Err(format!(
            "compiled IP set {field} length {} is not divisible by {stride}",
            decoded.len()
        ));
    }
    Ok(())
}

fn compress_ranges(ranges: &[u8]) -> Result<Vec<u8>, String> {
    if ranges.is_empty() {
        return Ok(Vec::new());
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(ranges)
        .map_err(|error| format!("compress compiled IP ranges: {error}"))?;
    encoder
        .finish()
        .map_err(|error| format!("finish compressed IP ranges: {error}"))
}

fn decompress_ranges(encoded: &[u8], field: &str, stride: usize) -> Result<Vec<u8>, String> {
    if encoded.is_empty() {
        return Ok(Vec::new());
    }
    let mut decoder = ZlibDecoder::new(encoded);
    let mut decoded = Vec::new();
    decoder
        .by_ref()
        .take(MAX_CANONICAL_RANGE_BYTES + 1)
        .read_to_end(&mut decoded)
        .map_err(|error| format!("compiled IP set {field} is invalid zlib: {error}"))?;
    if decoded.len() as u64 > MAX_CANONICAL_RANGE_BYTES {
        return Err(format!(
            "compiled IP set {field} exceeds the {} byte decompression limit",
            MAX_CANONICAL_RANGE_BYTES
        ));
    }
    validate_range_length(&decoded, field, stride)?;
    Ok(decoded)
}

fn validate_sorted_ranges(bytes: &[u8], width: usize) -> Result<(), String> {
    let stride = width * 2;
    let mut previous_end: Option<Vec<u8>> = None;
    for pair in bytes.chunks_exact(stride) {
        let (start, end) = pair.split_at(width);
        if start > end {
            return Err("compiled IP set contains a reversed range".to_string());
        }
        if let Some(previous) = previous_end.as_deref()
            && (previous >= start || bytes_are_adjacent(previous, start))
        {
            return Err(
                "compiled IP set ranges are not sorted, disjoint, and non-adjacent".to_string(),
            );
        }
        previous_end = Some(end.to_vec());
    }
    Ok(())
}

fn bytes_are_adjacent(previous_end: &[u8], next_start: &[u8]) -> bool {
    let mut next = previous_end.to_vec();
    for byte in next.iter_mut().rev() {
        let (value, overflow) = byte.overflowing_add(1);
        *byte = value;
        if !overflow {
            return next == next_start;
        }
    }
    false
}

fn contains_packed_range(ranges: &[u8], address: &[u8], width: usize) -> bool {
    let stride = width * 2;
    let count = ranges.len() / stride;
    let mut lower = 0usize;
    let mut upper = count;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let offset = middle * stride;
        let end = &ranges[offset + width..offset + stride];
        if end < address {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    if lower >= count {
        return false;
    }
    let offset = lower * stride;
    &ranges[offset..offset + width] <= address
}

fn contains_packed_cidr_range(ranges: &[u8], start: &[u8], end: &[u8], width: usize) -> bool {
    let stride = width * 2;
    let count = ranges.len() / stride;
    let mut lower = 0usize;
    let mut upper = count;
    while lower < upper {
        let middle = lower + (upper - lower) / 2;
        let offset = middle * stride;
        let range_end = &ranges[offset + width..offset + stride];
        if range_end < start {
            lower = middle + 1;
        } else {
            upper = middle;
        }
    }
    if lower >= count {
        return false;
    }
    let offset = lower * stride;
    &ranges[offset..offset + width] <= start && &ranges[offset + width..offset + stride] >= end
}

fn append_ipv4_cidrs(mut start: u32, end: u32, cidrs: &mut Vec<String>) {
    loop {
        let alignment_bits = start.trailing_zeros();
        let remaining_bits = if start == 0 && end == u32::MAX {
            32
        } else {
            (u64::from(end) - u64::from(start) + 1).ilog2()
        };
        let host_bits = alignment_bits.min(remaining_bits);
        cidrs.push(format!("{}/{}", Ipv4Addr::from(start), 32 - host_bits));
        if host_bits == 32 {
            break;
        }
        let block_size = 1u64 << host_bits;
        let next = u64::from(start) + block_size;
        if next > u64::from(end) {
            break;
        }
        start = next as u32;
    }
}

fn append_ipv6_cidrs(mut start: u128, end: u128, cidrs: &mut Vec<String>) {
    loop {
        let alignment_bits = start.trailing_zeros();
        let remaining_bits = if start == 0 && end == u128::MAX {
            128
        } else {
            (end - start + 1).ilog2()
        };
        let host_bits = alignment_bits.min(remaining_bits);
        cidrs.push(format!("{}/{}", Ipv6Addr::from(start), 128 - host_bits));
        if host_bits == 128 {
            break;
        }
        let block_size = 1u128 << host_bits;
        let Some(next) = start.checked_add(block_size) else {
            break;
        };
        if next > end {
            break;
        }
        start = next;
    }
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
    fn cidr_containment_requires_one_range_to_cover_the_entire_network() {
        let discontinuous = compile_ip_set([
            "10.0.0.0/32",
            "10.0.0.255/32",
            "2001:db8::/128",
            "2001:db8::ff/128",
        ])
        .unwrap();
        assert!(!discontinuous.contains_cidr("10.0.0.0/24"));
        assert!(!discontinuous.contains_cidr("2001:db8::/120"));

        let covering = compile_ip_set(["10.0.0.0/24", "2001:db8::/120"]).unwrap();
        assert!(covering.contains_cidr("10.0.0.64/26"));
        assert!(covering.contains_cidr("2001:db8::80/121"));
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
    fn minimal_cidr_cover_round_trips_ranges() {
        let compiled = compile_ip_set([
            "192.0.2.1/32",
            "192.0.2.2/31",
            "192.0.2.4/30",
            "2001:db8::1/128",
            "2001:db8::2/127",
            "2001:db8::4/126",
        ])
        .unwrap();
        let cidrs = compiled.to_cidrs();
        let round_trip = compile_ip_set(&cidrs).unwrap();
        assert_eq!(compiled.ipv4_ranges, round_trip.ipv4_ranges);
        assert_eq!(compiled.ipv6_ranges, round_trip.ipv6_ranges);
        assert!(cidrs.len() <= 6);
    }

    #[test]
    fn policy_id_is_deterministic_for_equivalent_sets() {
        let left = compile_ip_set(["192.0.2.0/25", "192.0.2.128/25"]).unwrap();
        let right = compile_ip_set(["192.0.2.0/24"]).unwrap();
        assert_eq!(left.id, right.id);
        assert_eq!(left.ipv4_ranges, right.ipv4_ranges);
    }

    #[test]
    fn decodes_legacy_cross_language_golden_fixture_and_reencodes_v2() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../packages/grpc-contracts/testdata/ipset-v1-golden.json"
        ))
        .unwrap();
        let legacy = json!({
            "format_version": fixture["format_version"],
            "ipv4_ranges": fixture["ipv4_ranges"],
            "ipv6_ranges": fixture["ipv6_ranges"],
        });
        let decoded =
            CompiledIpSet::from_config_value(fixture["id"].as_str().unwrap(), &legacy).unwrap();
        assert_eq!(decoded.format_version, 1);

        let cidrs = fixture["cidrs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let compiled = compile_ip_set(cidrs).unwrap();
        assert_eq!(
            compiled.id,
            "ipset-v2:045f9d04abff90c133eb8992fa305f5638cd320ac8b895f1e23f601d0b68c8ce"
        );
        let round_trip =
            CompiledIpSet::from_config_value(&compiled.id, &compiled.to_config_value()).unwrap();
        assert_eq!(round_trip.ipv4_ranges, decoded.ipv4_ranges);
        assert_eq!(round_trip.ipv6_ranges, decoded.ipv6_ranges);
    }

    #[test]
    fn v2_encoding_matches_cross_language_golden_fixture() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../../packages/grpc-contracts/testdata/ipset-v2-golden.json"
        ))
        .unwrap();
        let cidrs = fixture["cidrs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>();
        let compiled = compile_ip_set(cidrs).unwrap();
        assert_eq!(compiled.id, fixture["id"]);
        assert_eq!(compiled.format_version, fixture["format_version"]);
        assert_eq!(
            URL_SAFE_NO_PAD.encode(&compiled.ipv4_ranges),
            fixture["canonical_ipv4_ranges"]
        );
        assert_eq!(
            URL_SAFE_NO_PAD.encode(&compiled.ipv6_ranges),
            fixture["canonical_ipv6_ranges"]
        );
    }
}
