use super::*;

pub(super) async fn probe_configured_host_mappings(
    mappings: Vec<Value>,
    hosts: Vec<String>,
) -> Vec<Value> {
    let requested_hosts = if hosts.is_empty() {
        None
    } else {
        Some(
            hosts
                .into_iter()
                .map(|host| normalize_host_key(&host))
                .filter(|host| !host.is_empty())
                .collect::<BTreeSet<_>>(),
        )
    };
    let mut target_cache = HashMap::<String, Value>::new();
    let mut results = Vec::new();
    for mapping in mappings {
        let host = mapping
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let target = mapping
            .get("target")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if host.is_empty()
            || target.is_empty()
            || is_auth_service_target(&target)
            || requested_hosts
                .as_ref()
                .is_some_and(|set| !set.contains(&normalize_host_key(&host)))
        {
            continue;
        }
        let target_key = normalize_probe_url(&target).unwrap_or_else(|| target.clone());
        let probe = if let Some(cached) = target_cache.get(&target_key) {
            cached.clone()
        } else {
            let result = probe_host_mapping_target(&target).await;
            target_cache.insert(target_key, result.clone());
            result
        };
        let mut result = serde_json::Map::new();
        result.insert("host".to_string(), json!(host));
        result.insert("target".to_string(), json!(target));
        if let Some(object) = probe.as_object() {
            for (key, value) in object {
                result.insert(key.clone(), value.clone());
            }
        }
        results.push(Value::Object(result));
    }
    results
}

pub(super) async fn probe_host_mapping_target(target: &str) -> Value {
    let started = Instant::now();
    let Some(url) = normalize_probe_url(target) else {
        return json!({
            "status": "unsupported",
            "error": "Only http:// and https:// targets can be probed",
            "latencyMs": 0
        });
    };
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(2500))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return json!({
                "status": "stale",
                "error": error.to_string(),
                "latencyMs": elapsed_ms(started)
            });
        }
    };

    for method in [reqwest::Method::HEAD, reqwest::Method::GET] {
        let is_get = method == reqwest::Method::GET;
        match client
            .request(method, url.as_str())
            .header("User-Agent", "fn-knock-host-mapping-probe/1.0")
            .header("Connection", "close")
            .send()
            .await
        {
            Ok(response) => {
                return json!({
                    "status": "online",
                    "httpStatus": response.status().as_u16(),
                    "latencyMs": elapsed_ms(started)
                });
            }
            Err(error) if is_get => {
                return json!({
                    "status": "stale",
                    "error": error.to_string(),
                    "latencyMs": elapsed_ms(started)
                });
            }
            Err(_) => {}
        }
    }
    json!({
        "status": "stale",
        "error": "Probe failed",
        "latencyMs": elapsed_ms(started)
    })
}

pub(super) fn elapsed_ms(started: Instant) -> i64 {
    started.elapsed().as_millis().min(i64::MAX as u128) as i64
}

pub(super) fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub(super) fn normalize_probe_url(target: &str) -> Option<String> {
    let url = Url::parse(target.trim()).ok()?;
    matches!(url.scheme(), "http" | "https").then(|| url.to_string())
}

pub(super) fn normalize_host_key(value: &str) -> String {
    let lower = value.trim().to_lowercase();
    let without_scheme = strip_alpha_scheme(&lower);
    without_scheme
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn strip_alpha_scheme(value: &str) -> &str {
    let Some((scheme, rest)) = value.split_once("://") else {
        return value;
    };
    if !scheme.is_empty() && scheme.chars().all(|ch| ch.is_ascii_alphabetic()) {
        rest
    } else {
        value
    }
}

pub(super) fn is_auth_service_target(target: &str) -> bool {
    let Ok(url) = Url::parse(target.trim()) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    let port = url.port_or_known_default().unwrap_or(0);
    port == resolve_env_port_with_fallback("AUTH_PORT", 7997)
}
