use super::*;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub(crate) struct FirewallPortRange {
    pub start: i64,
    pub end: i64,
}

fn parse_range(value: &Value) -> Result<FirewallPortRange, &'static str> {
    let start = value
        .get("start")
        .and_then(Value::as_i64)
        .ok_or("portIntegerRequired")?;
    let end = value
        .get("end")
        .and_then(Value::as_i64)
        .ok_or("portIntegerRequired")?;
    if !(1..=65535).contains(&start) || !(1..=65535).contains(&end) {
        return Err("portOutOfRange");
    }
    if start >= end {
        return Err("rangeOrder");
    }
    Ok(FirewallPortRange { start, end })
}

pub(super) fn parse_firewall_additional_ranges(
    body: &Value,
) -> Result<Option<Vec<FirewallPortRange>>, &'static str> {
    let Some(value) = body.get("ranges") else {
        return Ok(None);
    };
    let items = value.as_array().ok_or("rangesArrayRequired")?;
    if items.len() > MAX_FIREWALL_ADDITIONAL_PORTS {
        return Err("tooManyPorts");
    }
    let mut ranges = items
        .iter()
        .map(parse_range)
        .collect::<Result<Vec<_>, _>>()?;
    ranges.sort();
    Ok(Some(ranges))
}

pub(super) fn validate_firewall_port_selection(
    ports: &[i64],
    ranges: &[FirewallPortRange],
) -> Result<(), &'static str> {
    if ports.len() + ranges.len() > MAX_FIREWALL_ADDITIONAL_PORTS {
        return Err("tooManyPorts");
    }
    let mut sorted = ranges.to_vec();
    sorted.sort();
    if sorted.windows(2).any(|pair| pair[1].start <= pair[0].end)
        || sorted.iter().any(|range| {
            ports
                .iter()
                .any(|port| (range.start..=range.end).contains(port))
        })
    {
        return Err("overlap");
    }
    Ok(())
}

// Persisted/backup data is constrained without ever broadening the allowed ports.
// Invalid or overlapping ranges are discarded; legacy single ports take priority.
pub(crate) fn normalize_firewall_additional_port_ranges(
    value: Option<&Value>,
    ports: &[i64],
) -> Vec<FirewallPortRange> {
    let candidates = value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| parse_range(item).ok())
        .collect::<BTreeSet<_>>();
    let mut ranges: Vec<FirewallPortRange> = Vec::new();
    for range in candidates {
        if ranges.len() + ports.len() >= MAX_FIREWALL_ADDITIONAL_PORTS {
            break;
        }
        if ranges
            .last()
            .is_some_and(|previous| range.start <= previous.end)
            || ports
                .iter()
                .any(|port| (range.start..=range.end).contains(port))
        {
            continue;
        }
        ranges.push(range);
    }
    ranges
}

pub(crate) fn configured_firewall_port_ranges(config: &Value) -> Vec<FirewallPortRange> {
    normalize_firewall_additional_port_ranges(
        config.get("firewall_additional_port_ranges"),
        &normalize_firewall_additional_ports(config.get("firewall_additional_ports")),
    )
}

pub(super) fn effective_firewall_port_ranges(
    config: &Value,
    run_type: i64,
) -> Vec<FirewallPortRange> {
    if run_type == 1 {
        Vec::new()
    } else {
        configured_firewall_port_ranges(config)
    }
}
