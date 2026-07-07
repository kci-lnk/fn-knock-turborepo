use super::*;

pub(super) async fn ensure_acme_data_migrated(state: &AppState) -> redis::RedisResult<()> {
    let existing = read_acme_applications_raw(state).await?;
    if !existing.is_empty() {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    }

    let Some(legacy) = read_legacy_settings(state).await? else {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    };
    let domains = legacy
        .get("domains")
        .and_then(Value::as_array)
        .map(|value| normalize_domain_list(value.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        state
            .redis
            .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
            .await?;
        return Ok(());
    }

    let now = now_node_iso();
    let primary_domain = domains[0].clone();
    let updated_at = legacy
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(normalize_timestamp)
        .unwrap_or_else(|| now.clone());
    let application = json!({
        "id": build_application_id(Some(&primary_domain)),
        "domains": domains,
        "primaryDomain": primary_domain,
        "dnsType": legacy.get("dnsType").and_then(Value::as_str).map(str::trim).unwrap_or(""),
        "credentials": normalize_string_record(legacy.get("credentials")),
        "renewEnabled": true,
        "createdAt": updated_at,
        "updatedAt": updated_at,
        "latestJobStatus": "idle",
    });

    let mut issued_certificates = Vec::new();
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    if let Some((cert, key)) = read_acme_cert_pair(state, primary_domain).await?
        && let Some(cert_info) = ssl::parse_cert_info(&cert)
    {
        issued_certificates.push(json!({
            "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
            "primaryDomain": primary_domain,
            "cert": cert,
            "key": key,
            "certInfo": cert_info,
            "createdAt": now,
            "updatedAt": now,
        }));
    }

    state
        .redis
        .set_json_value(ACME_APPLICATIONS_KEY, &Value::Array(vec![application]))
        .await?;
    state
        .redis
        .set_json_value(
            ACME_ISSUED_CERTIFICATES_KEY,
            &Value::Array(issued_certificates),
        )
        .await?;
    state
        .redis
        .set_string_value(ACME_MIGRATION_VERSION_KEY, "1")
        .await
}

pub(super) async fn read_acme_applications(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    ensure_acme_data_migrated(state).await?;
    let mut applications = read_acme_applications_raw(state).await?;
    applications.sort_by(|left, right| {
        right
            .get("updatedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(left.get("updatedAt").and_then(Value::as_str).unwrap_or(""))
    });
    Ok(applications)
}

pub(super) async fn read_acme_applications_raw(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    Ok(state
        .redis
        .get_json_value(ACME_APPLICATIONS_KEY)
        .await?
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(normalize_acme_application)
        .collect())
}

pub(super) async fn write_acme_applications(
    state: &AppState,
    applications: &[Value],
) -> redis::RedisResult<()> {
    state
        .redis
        .set_json_value(ACME_APPLICATIONS_KEY, &Value::Array(applications.to_vec()))
        .await
}

pub(super) async fn save_acme_application_with_effects(
    state: &AppState,
    t: &Translator,
    input: SaveAcmeApplicationInput,
) -> anyhow::Result<AcmeApplicationSaveOutcome> {
    ensure_acme_data_migrated(state).await?;
    let applications = read_acme_applications_raw(state).await?;
    let normalized_domains = normalize_domain_strings(input.domains);
    let primary_domain = normalized_domains.first().cloned().unwrap_or_default();
    let dns_type = input.dns_type.trim().to_string();
    if normalized_domains.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.domainsRequired"));
    }
    if dns_type.is_empty() {
        anyhow::bail!(t.t("server.redis.acme.dnsProviderRequired"));
    }

    let existing = input.id.as_ref().and_then(|id| {
        applications
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(id.as_str()))
            .cloned()
    });
    let duplicated = applications.iter().any(|item| {
        item.get("primaryDomain").and_then(Value::as_str) == Some(primary_domain.as_str())
            && item.get("id").and_then(Value::as_str)
                != existing
                    .as_ref()
                    .and_then(|value| value.get("id"))
                    .and_then(Value::as_str)
    });
    if duplicated {
        anyhow::bail!(t.t_params(
            "server.redis.acme.primaryDomainDuplicated",
            &[("primaryDomain", primary_domain.clone())]
        ));
    }

    let now = now_node_iso();
    let existing_id = existing
        .as_ref()
        .and_then(|value| value.get("id"))
        .and_then(Value::as_str);
    let id = existing_id
        .map(str::to_string)
        .or(input.id.filter(|value| !value.trim().is_empty()))
        .unwrap_or_else(|| build_application_id(None));
    let created_at = existing
        .as_ref()
        .and_then(|value| value.get("createdAt"))
        .and_then(Value::as_str)
        .unwrap_or(&now)
        .to_string();
    let mut application = Map::new();
    application.insert("id".to_string(), json!(id));
    if input.name_provided {
        if let Some(name) = input
            .name
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            application.insert("name".to_string(), json!(name));
        }
    } else if let Some(name) = existing
        .as_ref()
        .and_then(|value| value.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        application.insert("name".to_string(), json!(name));
    }
    application.insert("domains".to_string(), json!(normalized_domains));
    application.insert("primaryDomain".to_string(), json!(primary_domain));
    application.insert("dnsType".to_string(), json!(dns_type));
    application.insert("credentials".to_string(), input.credentials);
    application.insert(
        "renewEnabled".to_string(),
        json!(
            input
                .renew_enabled
                .or_else(|| {
                    existing
                        .as_ref()
                        .and_then(|value| value.get("renewEnabled"))
                        .and_then(Value::as_bool)
                })
                .unwrap_or(true)
        ),
    );
    application.insert("createdAt".to_string(), json!(created_at));
    application.insert("updatedAt".to_string(), json!(now));
    if let Some(existing) = existing.as_ref() {
        insert_optional_string(&mut application, "latestJobId", existing.get("latestJobId"));
        insert_optional_value(
            &mut application,
            "latestJobStatus",
            normalize_latest_job_status(existing.get("latestJobStatus")),
        );
        insert_optional_value(
            &mut application,
            "latestJobTrigger",
            normalize_job_trigger(existing.get("latestJobTrigger")),
        );
        insert_optional_string(&mut application, "latestJobAt", existing.get("latestJobAt"));
        insert_optional_string(&mut application, "lastError", existing.get("lastError"));
    } else {
        application.insert("latestJobStatus".to_string(), json!("idle"));
    }
    let application = Value::Object(application);
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let next_applications =
        std::iter::once(application.clone())
            .chain(applications.into_iter().filter(|item| {
                item.get("id").and_then(Value::as_str) != Some(application_id.as_str())
            }))
            .collect::<Vec<_>>();
    write_acme_applications(state, &next_applications).await?;

    let domain_changed = existing.as_ref().is_some_and(|previous| {
        previous.get("primaryDomain").and_then(Value::as_str)
            != application.get("primaryDomain").and_then(Value::as_str)
            || normalized_domain_signature(
                &previous
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            ) != normalized_domain_signature(
                &application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            )
    });
    if !domain_changed {
        return Ok(AcmeApplicationSaveOutcome {
            application,
            removed_library_certificate_count: 0,
            removed_active_library_certificate: false,
        });
    }

    let deleted_issued_certificate = delete_acme_issued_certificate(state, &application_id).await?;
    let previous_primary_domain = existing
        .as_ref()
        .and_then(|value| value.get("primaryDomain"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) = cleanup_acme_application_artifacts(
        state,
        &application_id,
        previous_primary_domain,
        deleted_issued_certificate.as_ref(),
    )
    .await?;
    Ok(AcmeApplicationSaveOutcome {
        application,
        removed_library_certificate_count: removed_count,
        removed_active_library_certificate: removed_active,
    })
}

pub(super) async fn resolve_legacy_application_for_mutation(
    state: &AppState,
    domains: &[String],
    t: &Translator,
) -> anyhow::Result<Option<Value>> {
    let applications = read_acme_applications(state).await?;
    let primary_domain = domains.first().map(String::as_str).unwrap_or("");
    if let Some(application) = applications.iter().find(|application| {
        application.get("primaryDomain").and_then(Value::as_str) == Some(primary_domain)
    }) {
        return Ok(Some(application.clone()));
    }
    if applications.len() == 1 {
        return Ok(applications.first().cloned());
    }
    if applications.len() > 1 {
        anyhow::bail!(t.t("server.redis.acme.multipleApplicationsUseNewApi"));
    }
    Ok(None)
}

pub(super) async fn delete_acme_application_internal(
    state: &AppState,
    id: &str,
) -> anyhow::Result<bool> {
    ensure_acme_data_migrated(state).await?;
    let applications = read_acme_applications_raw(state).await?;
    let Some(existing) = applications
        .iter()
        .find(|item| item.get("id").and_then(Value::as_str) == Some(id))
        .cloned()
    else {
        return Ok(false);
    };
    let next_applications = applications
        .into_iter()
        .filter(|item| item.get("id").and_then(Value::as_str) != Some(id))
        .collect::<Vec<_>>();
    write_acme_applications(state, &next_applications).await?;
    if next_applications.is_empty() {
        state.redis.delete_key(ACME_LEGACY_SETTINGS_KEY).await?;
    }
    let deleted_issued_certificate = delete_acme_issued_certificate(state, id).await?;
    let primary_domain = existing
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) = cleanup_acme_application_artifacts(
        state,
        id,
        primary_domain,
        deleted_issued_certificate.as_ref(),
    )
    .await?;
    sync_gateway_if_acme_library_removed(state, removed_active, removed_count).await?;
    Ok(true)
}

pub(super) async fn delete_acme_application_certificate_internal(
    state: &AppState,
    id: &str,
) -> anyhow::Result<bool> {
    let Some(application) = find_acme_application(state, id).await? else {
        return Ok(false);
    };
    let issued_certificate = delete_acme_issued_certificate(state, id).await?;
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let (removed_count, removed_active) =
        cleanup_acme_application_artifacts(state, id, primary_domain, issued_certificate.as_ref())
            .await?;
    sync_gateway_if_acme_library_removed(state, removed_active, removed_count).await?;
    Ok(true)
}

pub(super) async fn cleanup_acme_application_artifacts(
    state: &AppState,
    application_id: &str,
    primary_domain: &str,
    deleted_issued_certificate: Option<&Value>,
) -> anyhow::Result<(usize, bool)> {
    let (removed_by_ref, active_by_ref) =
        ssl::delete_acme_ssl_certificates(state, Some(application_id), None).await?;
    let (removed_by_domain, active_by_domain) =
        ssl::delete_acme_ssl_certificates(state, None, Some(primary_domain)).await?;
    let removed_domains = uniq_strings(
        [primary_domain].into_iter().chain(
            deleted_issued_certificate
                .and_then(|value| value.get("primaryDomain"))
                .and_then(Value::as_str),
        ),
    );
    remove_acme_domain_artifacts(state, &removed_domains).await?;
    Ok((
        removed_by_ref + removed_by_domain,
        active_by_ref || active_by_domain,
    ))
}

pub(super) async fn delete_acme_issued_certificate(
    state: &AppState,
    application_id: &str,
) -> redis::RedisResult<Option<Value>> {
    let issued_certificates = read_issued_certificates(state).await?;
    let mut deleted = None;
    let next = issued_certificates
        .into_iter()
        .filter(|item| {
            let should_delete =
                item.get("applicationId").and_then(Value::as_str) == Some(application_id);
            if should_delete {
                deleted = Some(item.clone());
            }
            !should_delete
        })
        .collect::<Vec<_>>();
    state
        .redis
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(next))
        .await?;
    Ok(deleted)
}

pub(super) async fn delete_acme_cert_pair(
    state: &AppState,
    domain: &str,
) -> redis::RedisResult<()> {
    state
        .redis
        .delete_key(&format!("{ACME_CERT_PREFIX}{domain}"))
        .await
}

pub(super) async fn remove_acme_domain_artifacts(
    state: &AppState,
    domains: &[String],
) -> anyhow::Result<()> {
    for domain in domains {
        delete_acme_cert_pair(state, domain).await?;
        let dir = state.settings.data_dir.join("ssl").join(domain);
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
