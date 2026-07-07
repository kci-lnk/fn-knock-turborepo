use super::*;

pub(super) async fn read_issued_certificates(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    ensure_acme_data_migrated(state).await?;
    Ok(state
        .redis
        .get_json_value(ACME_ISSUED_CERTIFICATES_KEY)
        .await?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_issued_certificate)
        .collect())
}

pub(super) async fn find_acme_application(
    state: &AppState,
    id: &str,
) -> redis::RedisResult<Option<Value>> {
    Ok(read_acme_applications(state)
        .await?
        .into_iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id)))
}

pub(super) async fn find_application_by_primary_domain(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<Value>> {
    let domain = domain.trim().to_ascii_lowercase();
    if domain.is_empty() {
        return Ok(None);
    }
    Ok(read_acme_applications(state)
        .await?
        .into_iter()
        .find(|item| {
            item.get("primaryDomain")
                .and_then(Value::as_str)
                .is_some_and(|value| value == domain)
        }))
}

pub(super) async fn get_acme_settings(state: &AppState) -> redis::RedisResult<Value> {
    let applications = read_acme_applications(state).await?;
    if let Some(application) = applications.first() {
        return Ok(json!({
            "domains": application.get("domains").cloned().unwrap_or_else(|| json!([])),
            "dnsType": application.get("dnsType").cloned().unwrap_or_else(|| json!("")),
            "credentials": application.get("credentials").cloned().unwrap_or_else(|| json!({})),
            "updatedAt": application.get("updatedAt").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(read_legacy_settings(state).await?.unwrap_or(Value::Null))
}

pub(super) async fn read_legacy_settings(state: &AppState) -> redis::RedisResult<Option<Value>> {
    let Some(value) = state.redis.get_json_value(ACME_LEGACY_SETTINGS_KEY).await? else {
        return Ok(None);
    };
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let domains = object
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let dns_type = object
        .get("dnsType")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("");
    if domains.is_empty() || dns_type.is_empty() {
        return Ok(None);
    }
    Ok(Some(json!({
        "domains": domains,
        "dnsType": dns_type,
        "credentials": normalize_string_record(object.get("credentials")),
        "updatedAt": object
            .get("updatedAt")
            .and_then(Value::as_str)
            .and_then(normalize_timestamp)
            .unwrap_or_else(time_utils::now_iso),
    })))
}

pub(super) async fn ensure_client_settings(state: &AppState) -> redis::RedisResult<Value> {
    if let Some(settings) = state
        .redis
        .get_json_value(ACME_CLIENT_SETTINGS_KEY)
        .await?
        .and_then(normalize_client_settings)
    {
        return Ok(settings);
    }
    let settings = json!({
        "certificateAuthority": default_certificate_authority(state),
        "updatedAt": time_utils::now_iso(),
    });
    state
        .redis
        .set_json_value(ACME_CLIENT_SETTINGS_KEY, &settings)
        .await?;
    Ok(settings)
}

pub(super) async fn status_certificate(state: &AppState) -> redis::RedisResult<Value> {
    let applications = read_acme_applications(state).await?;
    let issued = read_issued_certificates(state).await?;
    for application in applications {
        let Some(application_id) = application.get("id").and_then(Value::as_str) else {
            continue;
        };
        let Some(certificate) = issued.iter().find(|item| {
            item.get("applicationId").and_then(Value::as_str) == Some(application_id)
                && issued_certificate_compatible(&application, item)
        }) else {
            continue;
        };
        return Ok(json!({
            "primaryDomain": certificate.get("primaryDomain").cloned().unwrap_or(Value::Null),
            "info": certificate.get("certInfo").cloned().unwrap_or(Value::Null),
        }));
    }
    Ok(Value::Null)
}

pub(super) async fn get_certificate_for_domain(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<(String, String, String, Value)>> {
    if let Some(application) = find_application_by_primary_domain(state, domain).await? {
        let Some(application_id) = application.get("id").and_then(Value::as_str) else {
            return Ok(None);
        };
        if let Some(certificate) = read_issued_certificates(state)
            .await?
            .into_iter()
            .find(|item| {
                item.get("applicationId").and_then(Value::as_str) == Some(application_id)
                    && issued_certificate_compatible(&application, item)
            })
        {
            return Ok(Some((
                certificate
                    .get("primaryDomain")
                    .and_then(Value::as_str)
                    .unwrap_or(domain)
                    .to_string(),
                certificate
                    .get("cert")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                certificate
                    .get("key")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                certificate.get("certInfo").cloned().unwrap_or(Value::Null),
            )));
        }
        return Ok(None);
    }

    let domain = domain.trim().to_ascii_lowercase();
    let Some((cert, key)) = read_acme_cert_pair(state, &domain).await? else {
        return Ok(None);
    };
    let Some(info) = ssl::parse_cert_info(&cert) else {
        return Ok(None);
    };
    Ok(Some((domain, cert, key, info)))
}

pub(super) async fn get_usable_issued_certificate_for_application(
    state: &AppState,
    application: &Value,
) -> redis::RedisResult<Option<Value>> {
    let Some(application_id) = application.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    Ok(read_issued_certificates(state)
        .await?
        .into_iter()
        .find(|certificate| {
            certificate.get("applicationId").and_then(Value::as_str) == Some(application_id)
                && issued_certificate_compatible(application, certificate)
        }))
}

pub(super) async fn save_acme_certificate_to_library_by_application(
    state: &AppState,
    application: &Value,
    activate: bool,
    override_label: Option<&str>,
    t: &Translator,
) -> anyhow::Result<Value> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let issued = get_usable_issued_certificate_for_application(state, application)
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.noMatchingIssuedCertificate")))?;
    let primary_domain = issued
        .get("primaryDomain")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.acme.jobDataInvalid")))?;
    let cert = issued
        .get("cert")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.ssl.certNotFound")))?;
    let key = issued
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.redis.ssl.certNotFound")))?;
    let existing_id = issued
        .get("libraryCertificateId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty());
    let fallback_label = application
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(primary_domain);
    let saved = ssl::save_acme_certificate_to_library(
        state,
        existing_id,
        override_label.or(Some(fallback_label)),
        primary_domain,
        Some(application_id),
        cert,
        key,
        activate,
    )
    .await?;
    if let Some(certificate_id) = saved.get("id").and_then(Value::as_str) {
        link_issued_certificate_to_library(state, application_id, certificate_id).await?;
    }
    Ok(saved)
}

pub(super) async fn link_issued_certificate_to_library(
    state: &AppState,
    application_id: &str,
    library_certificate_id: &str,
) -> redis::RedisResult<Option<Value>> {
    let mut issued = read_issued_certificates(state).await?;
    let Some(index) = issued
        .iter()
        .position(|item| item.get("applicationId").and_then(Value::as_str) == Some(application_id))
    else {
        return Ok(None);
    };
    if let Some(object) = issued[index].as_object_mut() {
        object.insert(
            "libraryCertificateId".to_string(),
            json!(library_certificate_id),
        );
        object.insert("libraryLinkedAt".to_string(), json!(time_utils::now_iso()));
    }
    let linked = issued[index].clone();
    state
        .redis
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(issued))
        .await?;
    Ok(Some(linked))
}

pub(super) async fn sync_gateway_if_acme_library_touched(
    state: &AppState,
    certificate_id: &str,
) -> anyhow::Result<()> {
    let config = state.redis.get_config().await?;
    let should_sync = config
        .pointer("/ssl/active_cert_id")
        .and_then(Value::as_str)
        == Some(certificate_id)
        || config
            .pointer("/ssl/deployment_mode")
            .and_then(Value::as_str)
            == Some("multi_sni");
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}

pub(super) async fn read_acme_cert_pair(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<Option<(String, String)>> {
    let key = format!("{ACME_CERT_PREFIX}{domain}");
    let Some(value) = state.redis.get_json_value(&key).await? else {
        return Ok(None);
    };
    let cert = value
        .get("cert")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let key = value
        .get("key")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    Ok(cert.zip(key))
}

pub(super) async fn get_acme_job(state: &AppState, id: &str) -> redis::RedisResult<Option<Value>> {
    Ok(state
        .redis
        .get_json_value(&format!("{ACME_JOB_PREFIX}{id}"))
        .await?
        .and_then(normalize_acme_job))
}

pub(super) async fn get_acme_logs(
    state: &AppState,
    id: &str,
    limit: usize,
    order: &str,
) -> redis::RedisResult<Vec<Value>> {
    let mut logs = state
        .redis
        .list_log_buffer(
            &format!("{ACME_LOGS_PREFIX}{id}"),
            limit,
            MAX_ACME_LOG_LIMIT,
        )
        .await?
        .into_iter()
        .map(Value::String)
        .collect::<Vec<_>>();
    if order == "desc" {
        logs.reverse();
    }
    Ok(logs)
}

pub(super) async fn get_active_acme_runtime_lock(state: &AppState) -> redis::RedisResult<Value> {
    let Some(raw_lock) = state.redis.get_json_value(ACME_RUNTIME_LOCK_KEY).await? else {
        return Ok(json!({ "locked": false }));
    };
    let lock = normalize_runtime_lock(&raw_lock);
    if lock.get("locked").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({ "locked": false }));
    }
    let Some(job_id) = lock.get("jobId").and_then(Value::as_str) else {
        return Ok(json!({ "locked": false }));
    };
    let Some(job) = get_acme_job(state, job_id).await? else {
        return Ok(json!({ "locked": false }));
    };
    if matches!(
        job.get("status").and_then(Value::as_str),
        Some("succeeded" | "failed" | "stopped")
    ) {
        return Ok(json!({ "locked": false }));
    }
    Ok(lock)
}

pub(super) fn normalize_runtime_lock(value: &Value) -> Value {
    let Some(raw) = value.as_object() else {
        return json!({ "locked": false });
    };
    if raw.get("locked").and_then(Value::as_bool) != Some(true) {
        return json!({ "locked": false });
    }
    let mut object = Map::new();
    object.insert("locked".to_string(), json!(true));
    insert_optional_string(&mut object, "lockId", raw.get("lockId"));
    insert_optional_string(&mut object, "jobId", raw.get("jobId"));
    insert_optional_string(&mut object, "applicationId", raw.get("applicationId"));
    insert_optional_value(
        &mut object,
        "reason",
        normalize_job_trigger(raw.get("reason")),
    );
    insert_optional_string(&mut object, "startedAt", raw.get("startedAt"));
    insert_optional_string(&mut object, "heartbeatAt", raw.get("heartbeatAt"));
    insert_optional_string(&mut object, "expiresAt", raw.get("expiresAt"));
    Value::Object(object)
}

pub(super) fn find_library_certificate(
    ssl_status: &Value,
    application: &Value,
    issued_certificate: &Value,
) -> Option<Value> {
    let application_id = application.get("id").and_then(Value::as_str).unwrap_or("");
    let linked_id = issued_certificate
        .get("libraryCertificateId")
        .and_then(Value::as_str);
    ssl_status
        .get("certificates")
        .and_then(Value::as_array)?
        .iter()
        .find(|certificate| {
            certificate.get("source").and_then(Value::as_str) == Some("acme")
                && (certificate
                    .get("source_ref_id")
                    .and_then(Value::as_str)
                    .is_some_and(|value| value == application_id)
                    || linked_id.is_some_and(|linked_id| {
                        certificate.get("id").and_then(Value::as_str) == Some(linked_id)
                    }))
        })
        .cloned()
}

pub(super) fn build_latest_job_summary(application: &Value, latest_job: Option<&Value>) -> Value {
    if let Some(job) = latest_job {
        let mut object = Map::new();
        object.insert(
            "id".to_string(),
            job.get("id").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "status".to_string(),
            job.get("status").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "trigger".to_string(),
            job.get("trigger")
                .cloned()
                .unwrap_or_else(|| json!("manual_request")),
        );
        object.insert(
            "createdAt".to_string(),
            job.get("startedAt")
                .or_else(|| job.get("createdAt"))
                .or_else(|| application.get("updatedAt"))
                .cloned()
                .unwrap_or(Value::Null),
        );
        insert_optional_string(&mut object, "message", job.get("message"));
        return Value::Object(object);
    }

    let Some(latest_job_id) = application.get("latestJobId").and_then(Value::as_str) else {
        return Value::Null;
    };
    let mut object = Map::new();
    object.insert("id".to_string(), json!(latest_job_id));
    object.insert(
        "status".to_string(),
        application
            .get("latestJobStatus")
            .cloned()
            .unwrap_or_else(|| json!("idle")),
    );
    object.insert(
        "trigger".to_string(),
        application
            .get("latestJobTrigger")
            .cloned()
            .unwrap_or_else(|| json!("manual_request")),
    );
    object.insert(
        "createdAt".to_string(),
        application
            .get("latestJobAt")
            .or_else(|| application.get("updatedAt"))
            .cloned()
            .unwrap_or(Value::Null),
    );
    insert_optional_string(&mut object, "message", application.get("lastError"));
    Value::Object(object)
}

pub(super) fn provider_label(t: &Translator, dns_type: &str) -> String {
    let normalized =
        normalize_acme_dns_type(dns_type).unwrap_or_else(|| dns_type.trim().to_string());
    if normalized.is_empty() {
        return "DNS".to_string();
    }
    acme_dns_providers(t)
        .into_iter()
        .find(|provider| {
            provider.get("dnsType").and_then(Value::as_str) == Some(normalized.as_str())
        })
        .and_then(|provider| {
            provider
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or(normalized)
}

pub(super) fn build_subdomain_certificate_recommendation(
    state: &AppState,
    config: &Value,
    t: &Translator,
) -> Value {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let auth_host = auth_host_mapping(state, config)
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
                .map(normalize_domain_name)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let all_hosts = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|mappings| {
            uniq_strings(
                mappings
                    .iter()
                    .filter_map(|mapping| mapping.get("host").and_then(Value::as_str)),
            )
        })
        .unwrap_or_default();

    let mut mode = "manual";
    let mut summary = t.t("server.subdomainMode.recommendationMissingBase");
    let mut warnings = Vec::<String>::new();
    let mut recommended_domains = Vec::<String>::new();

    if !root_domain.is_empty() {
        mode = "wildcard_parent";
        recommended_domains = uniq_strings([root_domain.as_str(), &format!("*.{root_domain}")]);
        summary = t.t_params(
            "server.subdomainMode.recommendationWildcardSummary",
            &[("rootDomain", root_domain.clone())],
        );
        if !auth_host.is_empty()
            && !is_requirement_covered_by_certificate_domains(&auth_host, &recommended_domains)
        {
            recommended_domains = uniq_strings(
                recommended_domains
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(auth_host.as_str())),
            );
            warnings.push(t.t_params(
                "server.subdomainMode.authOutOfRootWarning",
                &[
                    ("authHost", auth_host.clone()),
                    ("rootDomain", root_domain.clone()),
                ],
            ));
        }
    } else if !auth_host.is_empty() {
        mode = "single_host";
        recommended_domains = vec![auth_host.clone()];
        summary = t.t_params(
            "server.subdomainMode.recommendationSingleHostSummary",
            &[("authHost", auth_host.clone())],
        );
        warnings.push(t.t("server.subdomainMode.wildcardSuggestion"));
    } else {
        warnings.push(t.t("server.subdomainMode.configureRootOrAuth"));
    }

    if auth_host.is_empty() {
        warnings.push(t.t("server.subdomainMode.authMissingWarning"));
    }

    let covered_hosts = all_hosts
        .iter()
        .filter(|host| is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_hosts = all_hosts
        .iter()
        .filter(|host| !is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();

    if !uncovered_hosts.is_empty() && !recommended_domains.is_empty() {
        warnings.push(t.t_params(
            "server.subdomainMode.uncoveredHostMappingsWarning",
            &[("count", uncovered_hosts.len().to_string())],
        ));
    }

    json!({
        "mode": mode,
        "root_domain": if root_domain.is_empty() { Value::Null } else { json!(root_domain) },
        "auth_host": if auth_host.is_empty() { Value::Null } else { json!(auth_host) },
        "recommended_domains": recommended_domains,
        "covered_hosts": covered_hosts,
        "uncovered_hosts": uncovered_hosts,
        "warnings": warnings,
        "can_autofill": !recommended_domains.is_empty(),
        "summary": summary,
    })
}

pub(super) fn auth_host_mapping(state: &AppState, config: &Value) -> Option<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| is_auth_service_mapping(state, mapping))
        .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
}

pub(super) fn is_auth_service_mapping(state: &AppState, mapping: &Value) -> bool {
    if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
        return true;
    }
    let target = mapping.get("target").and_then(Value::as_str).unwrap_or("");
    parse_target_port(target) == Some(state.settings.auth_port)
}

pub(super) fn parse_target_port(target: &str) -> Option<u16> {
    let parsed = url::Url::parse(target.trim()).ok()?;
    if let Some(port) = parsed.port() {
        return Some(port);
    }
    match parsed.scheme() {
        "https" | "wss" => Some(443),
        "http" | "ws" => Some(80),
        _ => None,
    }
}

pub(super) fn uniq_strings<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let normalized = normalize_domain_name(value);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        output.push(normalized);
    }
    output
}

pub(super) fn normalize_domain_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

pub(super) fn is_wildcard_domain(value: &str) -> bool {
    normalize_domain_name(value).starts_with("*.")
}

pub(super) fn strip_wildcard_prefix(value: &str) -> String {
    let normalized = normalize_domain_name(value);
    normalized
        .strip_prefix("*.")
        .unwrap_or(normalized.as_str())
        .to_string()
}

pub(super) fn does_pattern_cover_concrete_host(concrete_host: &str, pattern: &str) -> bool {
    let normalized_host = normalize_domain_name(concrete_host);
    let normalized_pattern = normalize_domain_name(pattern);
    if normalized_host.is_empty()
        || normalized_pattern.is_empty()
        || is_wildcard_domain(&normalized_host)
    {
        return false;
    }
    if !is_wildcard_domain(&normalized_pattern) {
        return normalized_host == normalized_pattern;
    }
    let suffix = strip_wildcard_prefix(&normalized_pattern);
    if suffix.is_empty() || !normalized_host.ends_with(&format!(".{suffix}")) {
        return false;
    }
    let label = &normalized_host[..normalized_host.len() - suffix.len() - 1];
    !label.is_empty() && !label.contains('.')
}

pub(super) fn is_requirement_covered_by_certificate_domains(
    requirement: &str,
    certificate_domains: &[String],
) -> bool {
    let requirement = normalize_domain_name(requirement);
    if requirement.is_empty() {
        return false;
    }
    if is_wildcard_domain(&requirement) {
        return certificate_domains
            .iter()
            .any(|domain| normalize_domain_name(domain) == requirement);
    }
    certificate_domains
        .iter()
        .any(|domain| does_pattern_cover_concrete_host(&requirement, domain))
}
