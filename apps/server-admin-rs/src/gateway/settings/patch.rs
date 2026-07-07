use super::*;

pub(super) fn apply_gateway_patch(config: &mut Value, patch: &Map<String, Value>) {
    let object = ensure_object(config);

    if patch.contains_key("auth_cache_ttl_seconds")
        || patch.contains_key("auth_cache_unauthorized_ttl_seconds")
    {
        let mut subdomain = object
            .get("subdomain_mode")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        if let Some(value) = patch.get("auth_cache_ttl_seconds") {
            subdomain.insert(
                "auth_cache_ttl_seconds".to_string(),
                Value::Number(normalize_cache_ttl(Some(value), 1).into()),
            );
        }
        if let Some(value) = patch.get("auth_cache_unauthorized_ttl_seconds") {
            subdomain.insert(
                "auth_cache_unauthorized_ttl_seconds".to_string(),
                Value::Number(normalize_cache_ttl(Some(value), 1).into()),
            );
        }
        object.insert("subdomain_mode".to_string(), Value::Object(subdomain));
    }

    if let Some(value) = patch.get("reverse_proxy_throttle") {
        let previous = object
            .get("reverse_proxy_throttle")
            .cloned()
            .unwrap_or_else(default_reverse_proxy_throttle);
        object.insert(
            "reverse_proxy_throttle".to_string(),
            normalize_reverse_proxy_throttle(&merge_objects(&previous, value)),
        );
    }

    if let Some(value) = patch.get("portal") {
        let previous = object
            .get("gateway_portal")
            .cloned()
            .unwrap_or_else(default_gateway_portal);
        object.insert(
            "gateway_portal".to_string(),
            normalize_gateway_portal(&merge_objects(&previous, value)),
        );
    }

    if let Some(value) = patch.get("crawler_blocker") {
        let previous = object
            .get("gateway_crawler_blocker")
            .cloned()
            .unwrap_or_else(default_gateway_crawler_blocker);
        let mut merged = merge_objects(&previous, value);
        ensure_object(&mut merged).insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert(
            "gateway_crawler_blocker".to_string(),
            normalize_gateway_crawler_blocker(&merged),
        );
    }
}
