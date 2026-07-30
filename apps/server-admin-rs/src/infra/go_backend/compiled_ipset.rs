use anyhow::Context;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde_json::{Value, json};

use crate::{cidr::CompiledIpSet, grpc_proto::CompiledIpSet as ProtoCompiledIpSet};

fn parse_compiled_ip_set(value: &Value) -> anyhow::Result<ProtoCompiledIpSet> {
    let policy = CompiledIpSet::from_transport_value(value).map_err(anyhow::Error::msg)?;
    let encoded = policy.to_config_value();
    let ipv4_ranges = URL_SAFE_NO_PAD
        .decode(
            encoded
                .get("ipv4_ranges")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .with_context(|| format!("encode compiled IP set {} ipv4_ranges", policy.id))?;
    let ipv6_ranges = URL_SAFE_NO_PAD
        .decode(
            encoded
                .get("ipv6_ranges")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .with_context(|| format!("encode compiled IP set {} ipv6_ranges", policy.id))?;
    Ok(ProtoCompiledIpSet {
        id: policy.id,
        format_version: policy.format_version,
        ipv4_ranges,
        ipv6_ranges,
    })
}

pub(super) fn parse_compiled_ip_sets(
    value: Option<&Value>,
) -> anyhow::Result<Vec<ProtoCompiledIpSet>> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(parse_compiled_ip_set)
        .collect()
}

pub(super) fn parse_optional_compiled_ip_set(
    value: Option<&Value>,
) -> anyhow::Result<Option<ProtoCompiledIpSet>> {
    let Some(value) = value.filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    if value
        .get("id")
        .and_then(Value::as_str)
        .is_none_or(|id| id.trim().is_empty())
    {
        return Ok(None);
    }
    parse_compiled_ip_set(value).map(Some)
}

pub(super) fn compiled_ip_set_to_json(policy: ProtoCompiledIpSet) -> Value {
    json!({
        "id": policy.id,
        "format_version": policy.format_version,
        "ipv4_ranges": URL_SAFE_NO_PAD.encode(policy.ipv4_ranges),
        "ipv6_ranges": URL_SAFE_NO_PAD.encode(policy.ipv6_ranges),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{compiled_ip_set_to_json, parse_optional_compiled_ip_set};
    use crate::cidr::compile_ip_set;

    #[test]
    fn compiled_ip_set_codec_round_trips_a_valid_policy() {
        let policy = compile_ip_set(["127.0.0.1/32", "2001:db8::/32"]).unwrap();
        let parsed = parse_optional_compiled_ip_set(Some(&policy.to_transport_value()))
            .unwrap()
            .unwrap();
        let mut expected = policy.to_config_value();
        expected["id"] = json!(policy.id);

        assert_eq!(compiled_ip_set_to_json(parsed), expected);
    }

    #[test]
    fn compiled_ip_set_codec_rejects_invalid_base64() {
        let result = parse_optional_compiled_ip_set(Some(&json!({
            "id": "ipset-v2:invalid",
            "format_version": 2,
            "ipv4_ranges": "***",
            "ipv6_ranges": ""
        })));

        assert!(result.is_err());
    }

    #[test]
    fn compiled_ip_set_codec_rejects_digest_mismatch() {
        let policy = compile_ip_set(["127.0.0.1/32"]).unwrap();
        let mut value = policy.to_transport_value();
        value["id"] = json!("ipset-v2:tampered");

        assert!(parse_optional_compiled_ip_set(Some(&value)).is_err());
    }
}
