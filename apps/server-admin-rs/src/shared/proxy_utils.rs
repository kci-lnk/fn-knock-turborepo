use serde_json::Value;

pub(crate) fn is_reverse_proxy_subdomain_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(1)
        && config
            .get("reverse_proxy_submode")
            .and_then(Value::as_str)
            .unwrap_or("path")
            == "subdomain"
}

pub(crate) fn is_any_subdomain_routing_mode(config: &Value) -> bool {
    config.get("run_type").and_then(Value::as_i64) == Some(3)
        || is_reverse_proxy_subdomain_mode(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn detects_subdomain_routing_modes() {
        assert!(is_any_subdomain_routing_mode(&json!({ "run_type": 3 })));
        assert!(is_any_subdomain_routing_mode(&json!({
            "run_type": 1,
            "reverse_proxy_submode": "subdomain"
        })));
        assert!(!is_any_subdomain_routing_mode(&json!({
            "run_type": 1,
            "reverse_proxy_submode": "path"
        })));
    }
}
