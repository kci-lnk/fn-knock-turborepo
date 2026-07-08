use super::*;

pub(crate) async fn sync_ssl_deployment_to_gateway(
    state: &AppState,
    config: Option<&Value>,
) -> anyhow::Result<()> {
    let owned_config;
    let config = match config {
        Some(config) => config,
        None => {
            owned_config = state.store.get_config().await?;
            &owned_config
        }
    };
    let deployment = build_gateway_ssl_deployment(config.get("ssl"));
    let certificates = deployment
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let (status, value) = if certificates.is_empty() {
        state.go_backend.clear_ssl().await?
    } else {
        state.go_backend.set_ssl_deployment(&deployment).await?
    };
    if !status.is_success() || value.get("success").and_then(Value::as_bool) == Some(false) {
        return Err(anyhow!(
            "{}",
            value.get("message").and_then(Value::as_str).unwrap_or("")
        ));
    }
    Ok(())
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
