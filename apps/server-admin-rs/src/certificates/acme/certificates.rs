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

pub(super) async fn sync_acme_library_after_issue(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let application_id = application
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
    let linked = ssl::get_acme_ssl_certificate_by_source_ref(state, application_id).await?;
    if let Some(linked_certificate) = linked {
        let linked_id = linked_certificate
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let label = linked_certificate
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string);
        let active_id = ssl::active_ssl_certificate_id(state).await?;
        let should_activate = active_id.as_deref() == Some(linked_id.as_str());
        save_acme_certificate_to_library_by_application(
            state,
            application,
            should_activate,
            label.as_deref(),
            t,
        )
        .await?;
        let config = state.store.get_config().await?;
        let should_sync = should_activate
            || config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni");
        if should_sync {
            ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
        }
        let message = if should_sync {
            t.t("server.acmeJobRunner.linkedLibrarySyncedGateway")
        } else {
            t.t("server.acmeJobRunner.linkedLibraryUpdated")
        };
        append_acme_log(state, job_id, &message).await.ok();
        return Ok(());
    }

    let label = application
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| application.get("primaryDomain").and_then(Value::as_str));
    match save_acme_certificate_to_library_by_application(state, application, false, label, t).await
    {
        Ok(_) => {
            let config = state.store.get_config().await?;
            if config
                .pointer("/ssl/deployment_mode")
                .and_then(Value::as_str)
                == Some("multi_sni")
            {
                ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
                append_acme_log(
                    state,
                    job_id,
                    &t.t("server.acmeJobRunner.addedToLibraryAndSyncedGateway"),
                )
                .await
                .ok();
            } else {
                append_acme_log(state, job_id, &t.t("server.acmeJobRunner.addedToLibrary"))
                    .await
                    .ok();
            }
            Ok(())
        }
        Err(error) => {
            let message = t.t_params(
                "server.acmeJobRunner.addToLibraryFailed",
                &[("message", error.to_string())],
            );
            append_acme_log(state, job_id, &message).await.ok();
            anyhow::bail!(message)
        }
    }
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
