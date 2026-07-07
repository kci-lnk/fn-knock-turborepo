use super::*;

pub(super) async fn save_ssl_certificate(
    state: &AppState,
    input: SaveCertificateBody,
    activate: bool,
) -> anyhow::Result<Value> {
    validate_ssl_cert(&input.cert, &input.key)?;
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let mut certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let cert = input.cert.trim().to_string();
    let key = input.key.trim().to_string();
    let id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            certificates
                .iter()
                .find(|item| {
                    item.get("cert").and_then(Value::as_str) == Some(cert.as_str())
                        && item.get("key").and_then(Value::as_str) == Some(key.as_str())
                })
                .and_then(|item| item.get("id").and_then(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_else(|| build_ssl_certificate_id(&cert, &key));
    let existing = certificates
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
        .cloned();
    let now = now_node_iso();
    let source = normalize_certificate_source(input.source.as_deref());
    let primary_domain = input
        .primary_domain
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let source_ref_id = input
        .source_ref_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let label = input
        .label
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            existing
                .as_ref()
                .and_then(|item| item.get("label").and_then(Value::as_str))
                .map(str::to_string)
        })
        .unwrap_or_else(|| default_certificate_label(source, primary_domain.as_deref()));
    let created_at = existing
        .as_ref()
        .and_then(|item| item.get("created_at").and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&now)
        .to_string();
    let mut next = Map::new();
    next.insert("id".to_string(), json!(id));
    next.insert("label".to_string(), json!(label));
    next.insert("source".to_string(), json!(source));
    if let Some(primary_domain) = primary_domain {
        next.insert("primary_domain".to_string(), json!(primary_domain));
    }
    if let Some(source_ref_id) = source_ref_id {
        next.insert("source_ref_id".to_string(), json!(source_ref_id));
    }
    next.insert("cert".to_string(), json!(cert));
    next.insert("key".to_string(), json!(key));
    next.insert("created_at".to_string(), json!(created_at));
    next.insert("updated_at".to_string(), json!(now));
    let next = Value::Object(next);
    certificates.retain(|item| {
        item.get("id").and_then(Value::as_str) != next.get("id").and_then(Value::as_str)
    });
    certificates.insert(0, next.clone());
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(certificates);
    if activate {
        let id = next.get("id").and_then(Value::as_str).unwrap_or("");
        next_ssl = mirror_active_ssl_certificate(&next_ssl, Some(id));
    }
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok(next)
}

pub(crate) async fn save_acme_certificate_to_library(
    state: &AppState,
    id: Option<&str>,
    label: Option<&str>,
    primary_domain: &str,
    source_ref_id: Option<&str>,
    cert: &str,
    key: &str,
    activate: bool,
) -> anyhow::Result<Value> {
    let normalized_domain = primary_domain.trim().to_ascii_lowercase();
    let normalized_ref = source_ref_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut resolved_id = id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    if resolved_id.is_none() {
        resolved_id = find_acme_ssl_certificate(state, normalized_ref, Some(&normalized_domain))
            .await?
            .and_then(|certificate| {
                certificate
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
    }

    save_ssl_certificate(
        state,
        SaveCertificateBody {
            id: resolved_id,
            label: label
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string),
            source: Some("acme".to_string()),
            primary_domain: Some(normalized_domain),
            source_ref_id: normalized_ref.map(str::to_string),
            cert: cert.to_string(),
            key: key.to_string(),
            activate: Some(activate),
        },
        activate,
    )
    .await
}

pub(crate) async fn get_acme_ssl_certificate_by_source_ref(
    state: &AppState,
    source_ref_id: &str,
) -> anyhow::Result<Option<Value>> {
    find_acme_ssl_certificate(state, Some(source_ref_id), None).await
}

pub(crate) async fn active_ssl_certificate_id(state: &AppState) -> anyhow::Result<Option<String>> {
    let config = state.redis.get_config().await?;
    Ok(normalize_ssl_config(config.get("ssl"))
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string))
}

pub(crate) async fn auto_select_certificate_for_subdomain(
    state: &AppState,
    translator: &Translator,
) -> anyhow::Result<Option<Value>> {
    let config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_certificate_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let deployment_mode =
        normalize_deployment_mode(ssl.get("deployment_mode").and_then(Value::as_str));
    let inventory_certificates = certificates
        .iter()
        .filter_map(|certificate| {
            let id = certificate.get("id").and_then(Value::as_str)?.to_string();
            Some(CertificateCoverageInput {
                id,
                certificate_domains: certificate
                    .get("cert")
                    .and_then(Value::as_str)
                    .and_then(parse_cert_info)
                    .map(|info| certificate_info_dns_names(&info))
                    .unwrap_or_default(),
            })
        })
        .collect::<Vec<_>>();
    let coverage = build_subdomain_certificate_inventory_coverage(
        state.settings.auth_port,
        &config,
        &inventory_certificates,
        active_certificate_id.as_deref(),
        deployment_mode,
        translator,
    );
    if coverage.get("can_auto_activate").and_then(Value::as_bool) != Some(true) {
        return Ok(None);
    }
    let Some(suggested_certificate_id) = coverage
        .get("suggested_certificate_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return Ok(None);
    };

    let Some(candidate) =
        set_active_ssl_certificate(state, Some(suggested_certificate_id.as_str())).await?
    else {
        return Ok(None);
    };

    match sync_ssl_deployment_to_gateway(state, None).await {
        Ok(()) => Ok(Some(json!({
            "applied": true,
            "certificate_id": candidate.get("id").and_then(Value::as_str).unwrap_or(""),
            "label": candidate.get("label").and_then(Value::as_str).unwrap_or(""),
            "message": translator.t("server.admin.subdomainMode.sslAutoSelected")
        }))),
        Err(error) => {
            let _ = set_active_ssl_certificate(state, active_certificate_id.as_deref()).await;
            let _ = sync_ssl_deployment_to_gateway(state, None).await;
            let detail = error.to_string();
            let message = if detail.trim().is_empty() {
                translator.t("server.admin.subdomainMode.sslAutoSelectionSyncFailed")
            } else {
                detail
            };
            Ok(Some(json!({
                "applied": false,
                "certificate_id": candidate.get("id").and_then(Value::as_str).unwrap_or(""),
                "label": candidate.get("label").and_then(Value::as_str).unwrap_or(""),
                "message": message
            })))
        }
    }
}

pub(super) async fn find_acme_ssl_certificate(
    state: &AppState,
    source_ref_id: Option<&str>,
    primary_domain: Option<&str>,
) -> anyhow::Result<Option<Value>> {
    let config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let normalized_ref = source_ref_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_domain = primary_domain
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    Ok(ssl
        .get("certificates")
        .and_then(Value::as_array)
        .and_then(|certificates| {
            certificates
                .iter()
                .find(|certificate| {
                    certificate.get("source").and_then(Value::as_str) == Some("acme")
                        && (normalized_ref.is_some_and(|id| {
                            certificate.get("source_ref_id").and_then(Value::as_str) == Some(id)
                        }) || normalized_domain.as_deref().is_some_and(|domain| {
                            certificate
                                .get("primary_domain")
                                .and_then(Value::as_str)
                                .is_some_and(|value| value.trim().eq_ignore_ascii_case(domain))
                        }))
                })
                .cloned()
        }))
}

pub(super) async fn activate_ssl_certificate(state: &AppState, id: &str) -> anyhow::Result<bool> {
    Ok(set_active_ssl_certificate(state, Some(id)).await?.is_some())
}

pub(super) async fn set_active_ssl_certificate(
    state: &AppState,
    id: Option<&str>,
) -> anyhow::Result<Option<Value>> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let normalized_id = id.map(str::trim).filter(|value| !value.is_empty());
    let candidate = normalized_id.and_then(|id| {
        ssl.get("certificates")
            .and_then(Value::as_array)?
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
            .cloned()
    });
    if normalized_id.is_some() && candidate.is_none() {
        return Ok(None);
    }
    config["ssl"] = mirror_active_ssl_certificate(&ssl, normalized_id);
    state.redis.save_config(&config).await?;
    Ok(candidate)
}

pub(super) async fn delete_ssl_certificate(
    state: &AppState,
    id: &str,
) -> anyhow::Result<(bool, bool)> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let removed = certificates
        .iter()
        .any(|item| item.get("id").and_then(Value::as_str) == Some(id));
    if !removed {
        return Ok((false, false));
    }
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(
        certificates
            .into_iter()
            .filter(|item| item.get("id").and_then(Value::as_str) != Some(id))
            .collect(),
    );
    let removed_active = active_id == id;
    next_ssl = mirror_active_ssl_certificate(
        &next_ssl,
        if removed_active {
            None
        } else {
            Some(&active_id)
        },
    );
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok((true, removed_active))
}

pub(crate) async fn delete_acme_ssl_certificates(
    state: &AppState,
    application_id: Option<&str>,
    primary_domain: Option<&str>,
) -> anyhow::Result<(usize, bool)> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let normalized_application_id = application_id
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let normalized_primary_domain = primary_domain
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty());

    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for certificate in certificates {
        let is_acme = certificate.get("source").and_then(Value::as_str) == Some("acme");
        let matches_ref = normalized_application_id
            .is_some_and(|id| certificate.get("source_ref_id").and_then(Value::as_str) == Some(id));
        let matches_domain = normalized_primary_domain.as_deref().is_some_and(|domain| {
            certificate
                .get("primary_domain")
                .and_then(Value::as_str)
                .is_some_and(|value| value.trim().eq_ignore_ascii_case(domain))
        });
        if is_acme && (matches_ref || matches_domain) {
            removed.push(certificate);
        } else {
            kept.push(certificate);
        }
    }

    if removed.is_empty() {
        return Ok((0, false));
    }

    let removed_active = removed
        .iter()
        .any(|item| item.get("id").and_then(Value::as_str) == Some(active_id.as_str()));
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(kept);
    next_ssl = mirror_active_ssl_certificate(
        &next_ssl,
        if removed_active {
            None
        } else {
            Some(&active_id)
        },
    );
    config["ssl"] = next_ssl;
    state.redis.save_config(&config).await?;
    Ok((removed.len(), removed_active))
}

pub(super) async fn clear_ssl_certificate_library(state: &AppState) -> anyhow::Result<()> {
    let mut config = state.redis.get_config().await?;
    let mut ssl = normalize_ssl_config(config.get("ssl"));
    ssl["certificates"] = json!([]);
    config["ssl"] = mirror_active_ssl_certificate(&ssl, None);
    state.redis.save_config(&config).await?;
    Ok(())
}

pub(super) async fn clear_active_ssl(state: &AppState) -> anyhow::Result<()> {
    let mut config = state.redis.get_config().await?;
    let ssl = normalize_ssl_config(config.get("ssl"));
    config["ssl"] = mirror_active_ssl_certificate(&ssl, None);
    state.redis.save_config(&config).await?;
    Ok(())
}
