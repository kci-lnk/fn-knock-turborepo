pub(super) fn parse_limit(value: Option<&str>) -> usize {
    let parsed = value.and_then(parse_node_parse_int).unwrap_or(200);
    parsed.clamp(1, 1000) as usize
}

pub(super) fn parse_node_parse_int(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix_trim_start(value)
}
