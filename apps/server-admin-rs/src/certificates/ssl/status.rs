use super::*;

pub(crate) async fn build_ssl_status(state: &AppState) -> anyhow::Result<Value> {
    let translator = Translator::from_state(state).await;
    build_ssl_status_with_translator(state, &translator).await
}

pub(super) async fn build_ssl_status_with_translator(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Value> {
    let config = state.storage.store.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let local_status = local_ssl_status(&ssl);
    let gateway = gateway_ssl_status(state, translator).await;
    let gateway_status = gateway.as_ref().ok().and_then(|value| value.clone());
    let gateway_error = gateway.as_ref().err().cloned();
    let gateway_mode = gateway_status
        .as_ref()
        .and_then(|value| value.get("deployment_mode").and_then(Value::as_str));
    let effective_mode = if gateway_mode == Some("multi_sni") {
        "multi_sni".to_string()
    } else {
        local_status
            .get("deploymentMode")
            .and_then(Value::as_str)
            .unwrap_or("single_active")
            .to_string()
    };
    let enabled = gateway_status
        .as_ref()
        .and_then(|value| value.get("enabled").and_then(Value::as_bool))
        .unwrap_or_else(|| {
            local_status
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(false)
        });

    let gateway_payload =
        build_gateway_status_payload(gateway_status.clone(), gateway_error, translator);

    let mut status = local_status;
    let mut certificates = status
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for certificate in &mut certificates {
        let certificate_domains = certificate_dns_names(certificate);
        certificate["coverage"] = build_subdomain_certificate_coverage(
            state.settings.auth_port,
            &config,
            &certificate_domains,
            translator,
        );
    }
    let active_certificate_domains = status
        .get("certInfo")
        .map(certificate_info_dns_names)
        .unwrap_or_default();
    let active_certificate_id = status
        .get("activeCertId")
        .and_then(Value::as_str)
        .map(str::to_string);
    let inventory_certificates = certificates
        .iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            Some(CertificateCoverageInput {
                id,
                certificate_domains: certificate_dns_names(certificate),
            })
        })
        .collect::<Vec<_>>();

    status["enabled"] = Value::Bool(enabled);
    status["configuredDeploymentMode"] = status
        .get("deploymentMode")
        .cloned()
        .unwrap_or_else(|| json!("single_active"));
    status["deploymentMode"] = json!(effective_mode);
    status["certificates"] = Value::Array(certificates);
    status["subdomain_coverage"] = build_subdomain_certificate_coverage(
        state.settings.auth_port,
        &config,
        &active_certificate_domains,
        translator,
    );
    status["library_coverage"] = build_subdomain_certificate_inventory_coverage(
        state.settings.auth_port,
        &config,
        &inventory_certificates,
        active_certificate_id.as_deref(),
        &effective_mode,
        translator,
    );
    status["gateway_status"] = gateway_payload;
    Ok(status)
}

pub(super) fn build_gateway_status_payload(
    gateway_status: Option<Value>,
    gateway_error: Option<String>,
    translator: &Translator,
) -> Value {
    if let Some(status) = gateway_status {
        json!({
            "enabled": status.get("enabled").and_then(Value::as_bool).unwrap_or(false),
            "deployment_mode": if status.get("deployment_mode").and_then(Value::as_str) == Some("multi_sni") { "multi_sni" } else { "single_active" },
            "certificates": status.get("certificates").cloned().unwrap_or_else(|| json!([])),
        })
    } else {
        json!({
            "enabled": false,
            "deployment_mode": "single_active",
            "certificates": [],
            "sync_error": gateway_error.unwrap_or_else(|| ssl_route_text(translator, "gatewayStatusReadFailed"))
        })
    }
}

pub(super) async fn gateway_ssl_status(
    state: &AppState,
    translator: &Translator,
) -> Result<Option<Value>, String> {
    match state.gateway.client.get_ssl_info().await {
        Ok((status, value)) if status.is_success() => {
            if value.get("success").and_then(Value::as_bool) == Some(false) {
                return Err(value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| ssl_route_text(translator, "gatewayStatusReadFailed")));
            }
            Ok(value.get("data").cloned().or(Some(value)))
        }
        Ok((status, value)) => Err(value
            .get("message")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}: {status}",
                    ssl_route_text(translator, "gatewayStatusReadFailed")
                )
            })),
        Err(error) => {
            tracing::warn!(%error, "failed to read gateway SSL status");
            Err(ssl_route_text(translator, "gatewayStatusReadFailed"))
        }
    }
}

pub(super) fn local_ssl_status(ssl: &Value) -> Value {
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            let mut object = Map::new();
            object.insert("id".to_string(), json!(id));
            object.insert(
                "label".to_string(),
                json!(
                    certificate
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                ),
            );
            object.insert(
                "source".to_string(),
                json!(normalize_certificate_source(
                    certificate.get("source").and_then(Value::as_str)
                )),
            );
            insert_optional_status_string(
                &mut object,
                "primary_domain",
                certificate.get("primary_domain"),
            );
            insert_optional_status_string(
                &mut object,
                "source_ref_id",
                certificate.get("source_ref_id"),
            );
            object.insert(
                "created_at".to_string(),
                json!(
                    certificate
                        .get("created_at")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                ),
            );
            object.insert(
                "updated_at".to_string(),
                json!(
                    certificate
                        .get("updated_at")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                ),
            );
            if let Some(cert_info) = certificate
                .get("cert")
                .and_then(Value::as_str)
                .and_then(parse_cert_info)
            {
                object.insert("certInfo".to_string(), cert_info);
            }
            object.insert("is_active".to_string(), json!(id == active_id));
            object.insert("coverage".to_string(), Value::Null);
            Some(Value::Object(object))
        })
        .collect::<Vec<_>>();
    let active = certificates
        .iter()
        .find(|item| item.get("is_active").and_then(Value::as_bool) == Some(true));
    let mut status = Map::new();
    status.insert("enabled".to_string(), json!(active.is_some()));
    if let Some(active_id) = active.and_then(|item| item.get("id").and_then(Value::as_str)) {
        status.insert("activeCertId".to_string(), json!(active_id));
    }
    status.insert(
        "deploymentMode".to_string(),
        json!(normalize_deployment_mode(
            ssl.get("deployment_mode").and_then(Value::as_str)
        )),
    );
    if let Some(cert_info) = active.and_then(|item| item.get("certInfo").cloned()) {
        status.insert("certInfo".to_string(), cert_info);
    }
    status.insert("certificates".to_string(), Value::Array(certificates));
    Value::Object(status)
}

fn insert_optional_status_string(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<&Value>,
) {
    if let Some(value) = value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        object.insert(key.to_string(), json!(value));
    }
}
