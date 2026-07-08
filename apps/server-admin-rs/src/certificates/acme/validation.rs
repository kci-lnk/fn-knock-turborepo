use super::*;

pub(super) async fn apply_acme_dns_provider_patches(
    state: &AppState,
    dns_type: &str,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    if dns_type != "dns_duckdns" {
        return Ok(());
    }
    const DEFAULT_API: &str = "https://www.duckdns.org/update";
    const PROXY_API: &str = "https://duckdns.fnknock.cn/update";
    let script_path = acme_home_dir(state).join("dnsapi/dns_duckdns.sh");
    let content = tokio::fs::read_to_string(&script_path).await.map_err(|_| {
        anyhow::anyhow!(t.t_params(
            "server.acmePatches.duckdns.scriptMissing",
            &[("path", script_path.to_string_lossy().to_string())],
        ))
    })?;
    if content.contains(PROXY_API) || !content.contains(DEFAULT_API) {
        return Ok(());
    }
    let updated = content.replace(DEFAULT_API, PROXY_API);
    if updated != content {
        tokio::fs::write(&script_path, updated).await?;
        append_acme_log(
            state,
            job_id,
            &t.t_params(
                "server.acmePatches.duckdns.proxyApplied",
                &[
                    ("from", DEFAULT_API.to_string()),
                    ("to", PROXY_API.to_string()),
                ],
            ),
        )
        .await
        .ok();
    }
    Ok(())
}

pub(super) async fn read_replayable_json_body(
    req: Request<Body>,
    t: &Translator,
) -> Result<(Value, Request<Body>), Response> {
    let (parts, body) = req.into_parts();
    let bytes = match to_bytes(body, MAX_ACME_BODY_BYTES).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to read ACME request body");
            return Err(response::error(
                StatusCode::BAD_REQUEST,
                acme_route_text(t, "invalidRequestBody"),
            ));
        }
    };
    let replayable = Request::from_parts(parts, Body::from(bytes.clone()));
    let value = parse_json_bytes(&bytes, t)?;
    Ok((value, replayable))
}

pub(super) fn parse_json_bytes(bytes: &Bytes, t: &Translator) -> Result<Value, Response> {
    if bytes.is_empty() {
        return Err(response::error(
            StatusCode::BAD_REQUEST,
            acme_route_text(t, "invalidRequestBody"),
        ));
    }
    serde_json::from_slice(bytes).map_err(|_| {
        response::error(
            StatusCode::BAD_REQUEST,
            acme_route_text(t, "invalidRequestBody"),
        )
    })
}

pub(super) fn submit_now_requested(value: &Value) -> bool {
    value.get("submitNow").and_then(Value::as_bool) == Some(true)
}

pub(super) fn build_pending_acme_application_for_update(
    existing: &Value,
    body: &Value,
    normalized: &NormalizedAcmeRequest,
) -> Value {
    let mut application = existing.as_object().cloned().unwrap_or_default();
    if body.get("name").is_some() {
        if let Some(name) = body
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            application.insert("name".to_string(), json!(name));
        } else {
            application.remove("name");
        }
    }
    application.insert("domains".to_string(), json!(normalized.domains.clone()));
    application.insert(
        "primaryDomain".to_string(),
        json!(
            normalized
                .domains
                .first()
                .cloned()
                .or_else(|| {
                    existing
                        .get("primaryDomain")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default()
        ),
    );
    application.insert("dnsType".to_string(), json!(normalized.dns_type.clone()));
    application.insert("credentials".to_string(), normalized.credentials.clone());
    if let Some(renew_enabled) = body.get("renewEnabled").and_then(Value::as_bool) {
        application.insert("renewEnabled".to_string(), json!(renew_enabled));
    }
    Value::Object(application)
}

pub(super) fn validate_acme_request(
    input: &Value,
    t: &Translator,
) -> Result<NormalizedAcmeRequest, String> {
    let domains = input
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_valid_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        return Err(t.t("server.acmeRoutes.domainsInvalid"));
    }

    let dns_type = input
        .get("dnsType")
        .or_else(|| input.get("provider"))
        .and_then(value_to_trimmed_string)
        .and_then(|value| normalize_acme_dns_type(&value))
        .ok_or_else(|| t.t("server.acmeRoutes.dnsTypeRequired"))?;
    let provider = acme_dns_providers(t)
        .into_iter()
        .find(|provider| provider.get("dnsType").and_then(Value::as_str) == Some(dns_type.as_str()))
        .ok_or_else(|| t.t("server.acmeRoutes.unsupportedDnsProvider"))?;
    let credentials =
        filter_acme_credentials_for_provider(&provider, &dns_type, input.get("credentials"));
    if !credential_scheme_satisfied(&provider, &credentials) {
        return Err(t.t_params(
            "server.acmeRoutes.missingDnsCredentials",
            &[("requirements", format_credential_requirements(&provider, t))],
        ));
    }

    Ok(NormalizedAcmeRequest {
        domains,
        dns_type,
        credentials: Value::Object(credentials),
    })
}

pub(super) fn normalize_valid_domain_list<'a>(
    values: impl Iterator<Item = &'a Value>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let Some(domain) = value_to_trimmed_string(value).map(|value| value.to_ascii_lowercase())
        else {
            continue;
        };
        if !is_valid_acme_domain(&domain) || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

pub(super) fn is_valid_acme_domain(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || value.len() > 253
        || value.contains("..")
        || value.starts_with('.')
        || value.ends_with('.')
        || value.contains('/')
        || value.chars().any(char::is_whitespace)
    {
        return false;
    }
    let host = value.strip_prefix("*.").unwrap_or(value);
    if host.starts_with("*.") || host.contains('*') {
        return false;
    }
    let labels = host.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return false;
    }
    labels.iter().all(|label| {
        !label.is_empty()
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

pub(super) fn value_to_trimmed_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
    .filter(|value| !value.is_empty())
}

pub(super) fn normalize_acme_dns_type(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let lower = value.to_ascii_lowercase();
    let aliased = match lower.as_str() {
        "aliyun" => "dns_ali",
        "cloudflare" => "dns_cf",
        "dnspod" => "dns_dp",
        "tencentcloud" => "dns_tencent",
        "duckdns" => "dns_duckdns",
        "google" | "gcloud" | "dns_google" => "dns_gcloud",
        "huaweicloud" | "huawei" => "dns_huaweicloud",
        "netlify" => "dns_netlify",
        _ => "",
    };
    if !aliased.is_empty() {
        return Some(aliased.to_string());
    }
    if lower.starts_with("dns_")
        && lower
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Some(lower);
    }
    None
}

pub(super) fn normalize_acme_env_vars(
    dns_type: &str,
    credentials: Option<&Value>,
) -> Map<String, Value> {
    let mut record = normalize_string_record(credentials)
        .as_object()
        .cloned()
        .unwrap_or_default();
    if dns_type == "dns_netlify"
        && !record.contains_key("NETLIFY_ACCESS_TOKEN")
        && let Some(value) = record.get("NETLIFY_TOKEN").cloned()
    {
        record.insert("NETLIFY_ACCESS_TOKEN".to_string(), value);
    }
    record
}

pub(super) fn filter_acme_credentials_for_provider(
    provider: &Value,
    dns_type: &str,
    credentials: Option<&Value>,
) -> Map<String, Value> {
    let normalized = normalize_string_record(credentials);
    let mut record = normalized.as_object().cloned().unwrap_or_default();
    if dns_type == "dns_netlify"
        && !record.contains_key("NETLIFY_ACCESS_TOKEN")
        && let Some(value) = record.get("NETLIFY_TOKEN").cloned()
    {
        record.insert("NETLIFY_ACCESS_TOKEN".to_string(), value);
    }
    let allowed_keys = provider_credential_keys(provider);
    record
        .into_iter()
        .filter(|(key, _)| allowed_keys.contains(key))
        .collect()
}

pub(super) fn provider_credential_keys(provider: &Value) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    if let Some(schemes) = provider.get("credentialSchemes").and_then(Value::as_array) {
        for scheme in schemes {
            if let Some(fields) = scheme.get("fields").and_then(Value::as_array) {
                for field in fields {
                    if let Some(key) = field.get("key").and_then(Value::as_str) {
                        keys.insert(key.to_string());
                    }
                }
            }
        }
    }
    keys
}

pub(super) fn credential_scheme_satisfied(
    provider: &Value,
    credentials: &Map<String, Value>,
) -> bool {
    provider
        .get("credentialSchemes")
        .and_then(Value::as_array)
        .is_some_and(|schemes| {
            schemes.iter().any(|scheme| {
                scheme
                    .get("fields")
                    .and_then(Value::as_array)
                    .is_some_and(|fields| {
                        fields
                            .iter()
                            .filter(|field| {
                                field.get("required").and_then(Value::as_bool) != Some(false)
                            })
                            .all(|field| {
                                field
                                    .get("key")
                                    .and_then(Value::as_str)
                                    .and_then(|key| credentials.get(key))
                                    .and_then(Value::as_str)
                                    .is_some_and(|value| !value.trim().is_empty())
                            })
                    })
            })
        })
}

pub(super) fn format_credential_requirements(provider: &Value, t: &Translator) -> String {
    let schemes = provider
        .get("credentialSchemes")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if schemes.len() == 1 {
        return required_credential_keys(&schemes[0]).join(", ");
    }
    schemes
        .iter()
        .map(|scheme| {
            let required = required_credential_keys(scheme).join(", ");
            let optional = optional_credential_keys(scheme);
            let suffix = if optional.is_empty() {
                String::new()
            } else {
                t.t_params(
                    "server.acmeDnsProviders.requirements.optionalSuffix",
                    &[("keys", optional.join(", "))],
                )
            };
            format!(
                "{}: {required}{suffix}",
                scheme
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("Credentials")
            )
        })
        .collect::<Vec<_>>()
        .join(&t.t("server.acmeDnsProviders.requirements.orSeparator"))
}

pub(super) fn required_credential_keys(scheme: &Value) -> Vec<String> {
    scheme
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) != Some(false))
        .filter_map(|field| field.get("key").and_then(Value::as_str).map(str::to_string))
        .collect()
}

pub(super) fn optional_credential_keys(scheme: &Value) -> Vec<String> {
    scheme
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) == Some(false))
        .filter_map(|field| field.get("key").and_then(Value::as_str).map(str::to_string))
        .collect()
}

pub(super) fn normalize_domain_strings(values: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut domains = Vec::new();
    for value in values {
        let domain = normalize_domain_name(&value);
        if domain.is_empty() || !seen.insert(domain.clone()) {
            continue;
        }
        domains.push(domain);
    }
    domains
}

pub(super) async fn sync_gateway_if_acme_library_removed(
    state: &AppState,
    removed_active: bool,
    removed_count: usize,
) -> anyhow::Result<()> {
    if !removed_active && removed_count == 0 {
        return Ok(());
    }
    let config = state.store.get_config().await?;
    let should_sync = removed_active
        || (removed_count > 0
            && config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni"));
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}
