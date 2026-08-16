use super::*;

pub(crate) async fn sync_ssl_deployment_to_gateway(
    state: &AppState,
    config: Option<&Value>,
) -> anyhow::Result<()> {
    const MAX_CONVERGENCE_ATTEMPTS: usize = 32;

    let _guard = state.gateway.ssl_deployment_lock.lock().await;
    let mut target = match config {
        Some(config) => config.clone(),
        None => state.storage.store.get_config().await?,
    };
    for _ in 0..MAX_CONVERGENCE_ATTEMPTS {
        // A caller may have prepared `target` before waiting for the
        // deployment lock. Never send that stale snapshot to the gateway:
        // first converge it to the latest persisted SSL state, then apply.
        // This prevents an older manual/ACME request from briefly replacing a
        // newer Certd deployment while both requests are completing.
        let current = state.storage.store.get_config().await?;
        target = prefer_persisted_ssl_configuration(&target, current);
        apply_ssl_deployment_to_gateway(state, &target).await?;
        let current = state.storage.store.get_config().await?;
        if same_ssl_configuration(&target, &current) {
            return Ok(());
        }
        target = current;
    }
    Err(anyhow!(
        "SSL configuration changed too frequently while synchronizing the gateway"
    ))
}

async fn apply_ssl_deployment_to_gateway(state: &AppState, config: &Value) -> anyhow::Result<()> {
    let deployment = build_gateway_ssl_deployment(config.get("ssl"));
    let certificates = deployment
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (status, value) = if certificates.is_empty() {
        state.gateway.client.clear_ssl().await?
    } else {
        state.gateway.client.set_ssl_deployment(&deployment).await?
    };
    if !status.is_success() || value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(anyhow!(
            "{}",
            value.get("message").and_then(Value::as_str).unwrap_or("")
        ));
    }
    Ok(())
}

fn same_ssl_configuration(left: &Value, right: &Value) -> bool {
    normalize_ssl_config(left.get("ssl")) == normalize_ssl_config(right.get("ssl"))
}

fn prefer_persisted_ssl_configuration(requested: &Value, persisted: Value) -> Value {
    if same_ssl_configuration(requested, &persisted) {
        requested.clone()
    } else {
        persisted
    }
}

pub(super) fn build_gateway_ssl_deployment(ssl: Option<&Value>) -> Value {
    let ssl = normalize_ssl_config(ssl);
    let deployment_mode =
        normalize_deployment_mode(ssl.get("deployment_mode").and_then(Value::as_str));
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let active = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()))
        .cloned();
    if deployment_mode != "multi_sni" {
        return json!({
            "deployment_mode": "single_active",
            "certificates": active.as_ref().map(|certificate| gateway_certificate_payload(certificate, true)).into_iter().collect::<Vec<_>>()
        });
    }
    let mut ordered = Vec::new();
    let mut seen = BTreeSet::new();
    if let Some(active) = active.clone() {
        if let Some(id) = active.get("id").and_then(Value::as_str) {
            seen.insert(id.to_string());
        }
        ordered.push(active.clone());
    }
    for certificate in certificates {
        let id = certificate.get("id").and_then(Value::as_str).unwrap_or("");
        if !id.is_empty() && seen.insert(id.to_string()) {
            ordered.push(certificate);
        }
    }
    json!({
        "deployment_mode": "multi_sni",
        "certificates": ordered.iter().enumerate().map(|(index, certificate)| {
            let is_default = if active.is_some() {
                certificate.get("id").and_then(Value::as_str) == Some(active_id.as_str())
            } else {
                index == 0
            };
            gateway_certificate_payload(certificate, is_default)
        }).collect::<Vec<_>>()
    })
}

pub(super) fn gateway_certificate_payload(certificate: &Value, is_default: bool) -> Value {
    json!({
        "id": certificate.get("id").and_then(Value::as_str).unwrap_or(""),
        "label": certificate.get("label").and_then(Value::as_str).unwrap_or(""),
        "cert": certificate.get("cert").and_then(Value::as_str).unwrap_or(""),
        "key": certificate.get("key").and_then(Value::as_str).unwrap_or(""),
        "is_default": is_default
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssl_convergence_ignores_unrelated_config_and_detects_ssl_changes() {
        let first = json!({
            "ssl": { "deployment_mode": "single_active", "certificates": [] },
            "unrelated": 1
        });
        let same_ssl = json!({
            "ssl": { "deployment_mode": "single_active", "certificates": [] },
            "unrelated": 2
        });
        let changed_ssl = json!({
            "ssl": { "deployment_mode": "multi_sni", "certificates": [] },
            "unrelated": 2
        });
        assert!(same_ssl_configuration(&first, &same_ssl));
        assert!(!same_ssl_configuration(&first, &changed_ssl));
        assert_eq!(
            prefer_persisted_ssl_configuration(&first, same_ssl),
            first,
            "unrelated configuration changes must not replace the requested SSL snapshot",
        );
        assert_eq!(
            prefer_persisted_ssl_configuration(&first, changed_ssl.clone()),
            changed_ssl,
            "a newer persisted SSL snapshot must win before the gateway call",
        );
    }
}
