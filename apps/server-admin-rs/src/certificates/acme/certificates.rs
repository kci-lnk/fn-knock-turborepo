use super::*;

pub(super) fn acme_home_dir(state: &AppState) -> PathBuf {
    if crate::runtime_profile::deployment_target(state) == "windows" {
        return state.settings.data_dir.join("acme");
    }
    state.settings.data_dir.join(".acme.sh")
}

pub(super) fn acme_data_dir_name(state: &AppState, domain: &str) -> String {
    acme_data_dir_name_for_target(
        domain,
        crate::runtime_profile::deployment_target(state) == "windows",
    )
}

pub(super) fn acme_data_dir_name_for_target(domain: &str, is_windows: bool) -> String {
    let normalized = normalize_domain_name(domain);
    if is_windows {
        return normalized
            .strip_prefix("*.")
            .map(|suffix| format!("wildcard_{suffix}"))
            .unwrap_or(normalized);
    }
    normalized
}

pub(super) fn acme_issued_storage_domain(state: &AppState, application: &Value) -> String {
    acme_issued_storage_domain_for_target(
        application,
        crate::runtime_profile::deployment_target(state) == "windows",
    )
}

pub(super) fn acme_issued_storage_domain_for_target(
    application: &Value,
    is_windows: bool,
) -> String {
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .unwrap_or_default();
    if !is_windows {
        return primary_domain;
    }
    application
        .get("domains")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(normalize_domain_name)
        .find(|domain| !domain.starts_with("*."))
        .unwrap_or(primary_domain)
}

pub(super) async fn save_acme_issued_cert_from_fs(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<Value> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
    let primary_domain = application
        .get("primaryDomain")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
    let issued_storage_domain = acme_issued_storage_domain(state, application);
    install_acme_cert_to_data_dir(state, primary_domain, &issued_storage_domain, job_id).await?;
    let (cert, key) = read_acme_cert_pair_from_fs(state, primary_domain)
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButCertReadFailed")))?;
    let cert_info = ssl::parse_cert_info(&cert)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButCertReadFailed")))?;
    let issued = save_acme_issued_certificate(
        state,
        application_id,
        primary_domain,
        &cert,
        &key,
        cert_info,
    )
    .await?;
    Ok(issued)
}

pub(super) async fn install_acme_cert_to_data_dir(
    state: &AppState,
    primary_domain: &str,
    issued_storage_domain: &str,
    job_id: &str,
) -> anyhow::Result<()> {
    let normalized = normalize_domain_name(primary_domain);
    let data_dir_name = acme_data_dir_name(state, &normalized);
    let source_domain = normalize_domain_name(issued_storage_domain);
    let domain_dir = state.settings.data_dir.join("ssl").join(&data_dir_name);
    tokio::fs::create_dir_all(&domain_dir).await?;
    let installed_key_path = domain_dir.join(format!("{data_dir_name}.key"));
    let installed_fullchain_path = domain_dir.join("fullchain.cer");
    let executable = acme_executable_path(state);
    if !executable.is_file() {
        return Ok(());
    }
    let candidates = [
        (
            acme_home_dir(state).join(format!("{source_domain}_ecc")),
            true,
        ),
        (acme_home_dir(state).join(&source_domain), false),
        (
            legacy_acme_home_dir().join(format!("{source_domain}_ecc")),
            true,
        ),
        (legacy_acme_home_dir().join(&source_domain), false),
    ];
    let mut variants = candidates
        .iter()
        .filter(|(path, _)| path.exists())
        .map(|(_, use_ecc)| *use_ecc)
        .collect::<Vec<_>>();
    if variants.is_empty() {
        variants = vec![true, false];
    }
    variants.sort();
    variants.dedup();

    let mut last_error = None;
    for use_ecc in variants {
        let mut args = vec![
            "--home".to_string(),
            acme_home_dir(state).to_string_lossy().to_string(),
            "--config-home".to_string(),
            acme_home_dir(state).to_string_lossy().to_string(),
            "--install-cert".to_string(),
            "-d".to_string(),
            source_domain.clone(),
            "--key-file".to_string(),
            installed_key_path.to_string_lossy().to_string(),
            "--fullchain-file".to_string(),
            installed_fullchain_path.to_string_lossy().to_string(),
        ];
        if use_ecc {
            args.push("--ecc".to_string());
        }
        let result = run_acme_command(state, args, None).await?;
        if result.exit_code == 0 {
            return Ok(());
        }
        let message = format!(
            "[acme][install-cert] {} install failed (exit {}): {}",
            if use_ecc { "ECC" } else { "RSA" },
            result.exit_code,
            command_output_brief(&result.stdout, &result.stderr).trim_start_matches(": ")
        );
        append_acme_log(state, job_id, &message).await.ok();
        last_error = Some(message);
    }
    if read_acme_cert_pair_from_fs(state, &normalized)
        .await?
        .is_some()
    {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        last_error.unwrap_or_else(|| "failed to install ACME certificate files".to_string())
    )
}

pub(super) async fn read_acme_cert_pair_from_fs(
    state: &AppState,
    domain: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let normalized = normalize_domain_name(domain);
    let data_dir_name = acme_data_dir_name(state, &normalized);
    let candidates = [
        state.settings.data_dir.join("ssl").join(&data_dir_name),
        acme_home_dir(state).join(format!("{normalized}_ecc")),
        acme_home_dir(state).join(&normalized),
    ];
    for dir in candidates {
        let key_path = if dir == state.settings.data_dir.join("ssl").join(&data_dir_name) {
            dir.join(format!("{data_dir_name}.key"))
        } else {
            dir.join(format!("{normalized}.key"))
        };
        let cert_paths = [
            dir.join("fullchain.cer"),
            dir.join(format!("{normalized}.cer")),
        ];
        let Ok(key) = tokio::fs::read_to_string(&key_path).await else {
            continue;
        };
        for cert_path in cert_paths {
            if let Ok(cert) = tokio::fs::read_to_string(&cert_path).await
                && !cert.trim().is_empty()
                && !key.trim().is_empty()
            {
                return Ok(Some((cert, key)));
            }
        }
    }
    Ok(None)
}

pub(super) async fn save_acme_issued_certificate(
    state: &AppState,
    application_id: &str,
    primary_domain: &str,
    cert: &str,
    key: &str,
    cert_info: Value,
) -> anyhow::Result<Value> {
    let mut issued = read_issued_certificates(state).await?;
    let existing = issued
        .iter()
        .find(|item| item.get("applicationId").and_then(Value::as_str) == Some(application_id))
        .cloned();
    let now = now_node_iso();
    let mut next = Map::new();
    next.insert("applicationId".to_string(), json!(application_id));
    next.insert(
        "primaryDomain".to_string(),
        json!(normalize_domain_name(primary_domain)),
    );
    next.insert("cert".to_string(), json!(cert.trim()));
    next.insert("key".to_string(), json!(key.trim()));
    next.insert("certInfo".to_string(), cert_info);
    next.insert(
        "createdAt".to_string(),
        existing
            .as_ref()
            .and_then(|value| value.get("createdAt"))
            .cloned()
            .unwrap_or_else(|| json!(now.clone())),
    );
    next.insert("updatedAt".to_string(), json!(now));
    if let Some(value) = existing
        .as_ref()
        .and_then(|value| value.get("libraryCertificateId"))
        .and_then(Value::as_str)
    {
        next.insert("libraryCertificateId".to_string(), json!(value));
    }
    if let Some(value) = existing
        .as_ref()
        .and_then(|value| value.get("libraryLinkedAt"))
        .and_then(Value::as_str)
    {
        next.insert("libraryLinkedAt".to_string(), json!(value));
    }
    let next = Value::Object(next);
    issued.retain(|item| item.get("applicationId").and_then(Value::as_str) != Some(application_id));
    issued.insert(0, next.clone());
    state
        .store
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(issued))
        .await?;
    state
        .store
        .set_json_value(
            &format!(
                "{ACME_CERT_PREFIX}{}",
                normalize_domain_name(primary_domain)
            ),
            &json!({ "cert": cert.trim(), "key": key.trim() }),
        )
        .await?;
    Ok(next)
}

pub(super) async fn restore_acme_issued_certificate_snapshot(
    state: &AppState,
    application_id: &str,
    current_primary_domain: &str,
    previous: Option<&Value>,
) -> anyhow::Result<()> {
    let mut issued = read_issued_certificates(state).await?;
    issued.retain(|item| item.get("applicationId").and_then(Value::as_str) != Some(application_id));
    if let Some(previous) = previous {
        issued.insert(0, previous.clone());
    }
    state
        .store
        .set_json_value(ACME_ISSUED_CERTIFICATES_KEY, &Value::Array(issued))
        .await?;

    let current_primary_domain = normalize_domain_name(current_primary_domain);
    let previous_primary_domain = previous
        .and_then(|certificate| certificate.get("primaryDomain"))
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .unwrap_or_default();
    let applications = read_acme_applications(state).await?;
    let domain_is_reused = |domain: &str| {
        applications.iter().any(|application| {
            application.get("id").and_then(Value::as_str) != Some(application_id)
                && application
                    .get("primaryDomain")
                    .and_then(Value::as_str)
                    .is_some_and(|value| normalize_domain_name(value) == domain)
        })
    };
    if !current_primary_domain.is_empty()
        && current_primary_domain != previous_primary_domain
        && !domain_is_reused(&current_primary_domain)
    {
        remove_acme_domain_artifacts(state, &[current_primary_domain]).await?;
    }

    let Some(previous) = previous else {
        return Ok(());
    };
    let cert = previous
        .get("cert")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let key = previous
        .get("key")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if previous_primary_domain.is_empty() || cert.is_empty() || key.is_empty() {
        return Ok(());
    }
    if domain_is_reused(&previous_primary_domain) {
        return Ok(());
    }
    state
        .store
        .set_json_value(
            &format!("{ACME_CERT_PREFIX}{previous_primary_domain}"),
            &json!({ "cert": cert, "key": key }),
        )
        .await?;
    let data_dir_name = acme_data_dir_name(state, &previous_primary_domain);
    let domain_dir = state.settings.data_dir.join("ssl").join(&data_dir_name);
    tokio::fs::create_dir_all(&domain_dir).await?;
    tokio::fs::write(domain_dir.join(format!("{data_dir_name}.key")), key).await?;
    tokio::fs::write(domain_dir.join("fullchain.cer"), cert).await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum AcmeLibraryUpdateKind {
    Replaced,
    Added,
}

#[derive(Debug)]
pub(super) struct PreparedAcmeLibraryUpdate {
    pub(super) previous_ssl: Option<Value>,
    pub(super) next_config: Value,
    pub(super) should_sync_gateway: bool,
    pub(super) kind: AcmeLibraryUpdateKind,
}

#[derive(Debug)]
pub(super) enum AcmeSslRollbackOutcome {
    RestoredPrevious(Value),
    PreservedConcurrent(Value),
}

#[async_trait::async_trait]
pub(super) trait AcmeSslDeployment {
    async fn sync(&mut self, state: &AppState, config: &Value) -> anyhow::Result<()>;
}

struct GatewayAcmeSslDeployment;

#[async_trait::async_trait]
impl AcmeSslDeployment for GatewayAcmeSslDeployment {
    async fn sync(&mut self, state: &AppState, config: &Value) -> anyhow::Result<()> {
        ssl::sync_ssl_deployment_to_gateway(state, Some(config)).await
    }
}

pub(super) fn replacement_library_certificate(
    config: &Value,
    application_id: &str,
    issued_certificate: &Value,
) -> Option<Value> {
    let normalized_ssl = ssl::normalize_ssl_config(config.get("ssl"));
    let certificates = normalized_ssl
        .get("certificates")
        .and_then(Value::as_array)?;
    certificates
        .iter()
        .find(|certificate| {
            certificate.get("source").and_then(Value::as_str) == Some("acme")
                && certificate.get("source_ref_id").and_then(Value::as_str) == Some(application_id)
        })
        .or_else(|| {
            let library_certificate_id = issued_certificate
                .get("libraryCertificateId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())?;
            certificates.iter().find(|certificate| {
                certificate.get("source").and_then(Value::as_str) == Some("acme")
                    && certificate.get("id").and_then(Value::as_str) == Some(library_certificate_id)
            })
        })
        .cloned()
}

pub(super) async fn prepare_acme_library_after_issue(
    state: &AppState,
    application: &Value,
    t: &Translator,
) -> anyhow::Result<PreparedAcmeLibraryUpdate> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
    let issued_certificate = get_usable_issued_certificate_for_application(state, application)
        .await?
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.noMatchingIssuedCertificate")))?;
    let previous_config = state.store.get_config().await?;
    let previous_ssl = previous_config.get("ssl").cloned();
    let normalized_previous_ssl = ssl::normalize_ssl_config(previous_config.get("ssl"));
    let linked =
        replacement_library_certificate(&previous_config, application_id, &issued_certificate);
    if let Some(linked_id) = linked
        .as_ref()
        .and_then(|certificate| certificate.get("id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        // The source reference is authoritative when older issued metadata
        // still points at a stale library ID. Repair the link before saving so
        // the replacement cannot be inserted as a duplicate certificate.
        link_issued_certificate_to_library(state, application_id, linked_id).await?;
    }
    let active_id = normalized_previous_ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let (kind, should_activate, label) = if let Some(linked_certificate) = linked {
        let linked_id = linked_certificate
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("");
        (
            AcmeLibraryUpdateKind::Replaced,
            active_id == Some(linked_id),
            linked_certificate
                .get("label")
                .and_then(Value::as_str)
                .map(str::to_string),
        )
    } else {
        (
            AcmeLibraryUpdateKind::Added,
            false,
            application
                .get("name")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| application.get("primaryDomain").and_then(Value::as_str))
                .map(str::to_string),
        )
    };

    save_acme_certificate_to_library_by_application(
        state,
        application,
        should_activate,
        label.as_deref(),
        t,
    )
    .await?;
    let next_config = state.store.get_config().await?;
    let should_sync_gateway = should_activate
        || next_config
            .pointer("/ssl/deployment_mode")
            .and_then(Value::as_str)
            == Some("multi_sni");

    Ok(PreparedAcmeLibraryUpdate {
        previous_ssl,
        next_config,
        should_sync_gateway,
        kind,
    })
}

pub(super) async fn restore_ssl_after_failed_acme_deployment(
    state: &AppState,
    expected_ssl: Option<&Value>,
    previous_ssl: Option<&Value>,
) -> anyhow::Result<AcmeSslRollbackOutcome> {
    match state
        .store
        .compare_and_set_ssl_config(expected_ssl, previous_ssl)
        .await?
    {
        Some(restored_config) => {
            crate::fnos_certificate_sync::notify_certificate_library_changed(state);
            Ok(AcmeSslRollbackOutcome::RestoredPrevious(restored_config))
        }
        None => Ok(AcmeSslRollbackOutcome::PreservedConcurrent(
            state.store.get_config().await?,
        )),
    }
}

pub(super) async fn rollback_acme_ssl_and_sync_gateway(
    state: &AppState,
    previous_ssl: Option<&Value>,
    next_config: &Value,
) -> anyhow::Result<AcmeSslRollbackOutcome> {
    rollback_acme_ssl_and_sync_gateway_with(
        state,
        previous_ssl,
        next_config,
        &mut GatewayAcmeSslDeployment,
    )
    .await
}

pub(super) async fn rollback_acme_ssl_and_sync_gateway_with<D: AcmeSslDeployment>(
    state: &AppState,
    previous_ssl: Option<&Value>,
    next_config: &Value,
    deployment: &mut D,
) -> anyhow::Result<AcmeSslRollbackOutcome> {
    let outcome =
        restore_ssl_after_failed_acme_deployment(state, next_config.get("ssl"), previous_ssl)
            .await?;
    let config = match &outcome {
        AcmeSslRollbackOutcome::RestoredPrevious(config)
        | AcmeSslRollbackOutcome::PreservedConcurrent(config) => config,
    };
    deployment.sync(state, config).await?;
    sync_newer_ssl_deployment_until_current(state, config, deployment).await?;
    Ok(outcome)
}

async fn sync_newer_ssl_deployment_until_current<D: AcmeSslDeployment>(
    state: &AppState,
    deployed_config: &Value,
    deployment: &mut D,
) -> anyhow::Result<()> {
    let mut deployed_ssl = deployed_config.get("ssl").cloned();
    for _ in 0..32 {
        let current_config = state.store.get_config().await?;
        if current_config.get("ssl") == deployed_ssl.as_ref() {
            return Ok(());
        }
        deployment.sync(state, &current_config).await?;
        deployed_ssl = current_config.get("ssl").cloned();
    }
    anyhow::bail!("SSL configuration changed too frequently while synchronizing the gateway")
}

pub(super) async fn sync_ssl_deployment_with_rollback(
    state: &AppState,
    previous_ssl: Option<&Value>,
    next_config: &Value,
) -> anyhow::Result<()> {
    sync_ssl_deployment_with_rollback_using(
        state,
        previous_ssl,
        next_config,
        &mut GatewayAcmeSslDeployment,
    )
    .await
}

pub(super) async fn sync_ssl_deployment_with_rollback_using<D: AcmeSslDeployment>(
    state: &AppState,
    previous_ssl: Option<&Value>,
    next_config: &Value,
    deployment: &mut D,
) -> anyhow::Result<()> {
    let Err(deployment_error) = deployment.sync(state, next_config).await else {
        return sync_newer_ssl_deployment_until_current(state, next_config, deployment).await;
    };

    match rollback_acme_ssl_and_sync_gateway_with(state, previous_ssl, next_config, deployment)
        .await
    {
        Ok(AcmeSslRollbackOutcome::RestoredPrevious(_)) => {
            anyhow::bail!(
                "failed to deploy renewed ACME certificate: {deployment_error}; \
                 restored and reapplied the previous SSL configuration"
            );
        }
        Ok(AcmeSslRollbackOutcome::PreservedConcurrent(_)) => {
            anyhow::bail!(
                "failed to deploy renewed ACME certificate: {deployment_error}; \
                 preserved and applied a newer concurrent SSL configuration"
            );
        }
        Err(rollback_error) => {
            anyhow::bail!(
                "failed to deploy renewed ACME certificate: {deployment_error}; \
                 failed to restore or reapply a safe SSL configuration: {rollback_error}"
            );
        }
    }
}

pub(super) async fn sync_acme_library_after_issue(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let prepared = match prepare_acme_library_after_issue(state, application, t).await {
        Ok(prepared) => prepared,
        Err(error) => {
            let message = t.t_params(
                "server.acmeJobRunner.addToLibraryFailed",
                &[("message", error.to_string())],
            );
            append_acme_log(state, job_id, &message).await.ok();
            anyhow::bail!(message);
        }
    };

    if acme_job_is_stopped(state, job_id).await? {
        restore_ssl_after_failed_acme_deployment(
            state,
            prepared.next_config.get("ssl"),
            prepared.previous_ssl.as_ref(),
        )
        .await?;
        anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
    }

    if prepared.should_sync_gateway {
        sync_ssl_deployment_with_rollback(
            state,
            prepared.previous_ssl.as_ref(),
            &prepared.next_config,
        )
        .await?;
        if acme_job_is_stopped(state, job_id).await? {
            rollback_acme_ssl_and_sync_gateway(
                state,
                prepared.previous_ssl.as_ref(),
                &prepared.next_config,
            )
            .await?;
            anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
        }
    }

    let message = match (prepared.kind, prepared.should_sync_gateway) {
        (AcmeLibraryUpdateKind::Replaced, true) => {
            t.t("server.acmeJobRunner.linkedLibrarySyncedGateway")
        }
        (AcmeLibraryUpdateKind::Replaced, false) => {
            t.t("server.acmeJobRunner.linkedLibraryUpdated")
        }
        (AcmeLibraryUpdateKind::Added, true) => {
            t.t("server.acmeJobRunner.addedToLibraryAndSyncedGateway")
        }
        (AcmeLibraryUpdateKind::Added, false) => t.t("server.acmeJobRunner.addedToLibrary"),
    };
    append_acme_log(state, job_id, &message).await.ok();
    Ok(())
}

pub(super) async fn clear_acme_domain_working_state(
    state: &AppState,
    primary_domain: &str,
) -> anyhow::Result<()> {
    let normalized = normalize_domain_name(primary_domain);
    if normalized.is_empty() {
        return Ok(());
    }
    let working_dir_name = acme_data_dir_name(state, &normalized);
    for dir in [
        acme_home_dir(state).join(&working_dir_name),
        acme_home_dir(state).join(format!("{working_dir_name}_ecc")),
    ] {
        match tokio::fs::remove_dir_all(&dir).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}
