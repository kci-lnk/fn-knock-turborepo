use super::*;

pub(super) async fn run_acme_auto_renew_once(state: AppState) -> anyhow::Result<()> {
    let acquired = state
        .redis
        .set_lock_if_not_exists("acme-renew", acme_renew_lock_ttl_seconds())
        .await?;
    if !acquired {
        return Ok(());
    }

    let t = Translator::from_state(&state).await;
    let install_state = current_acme_install_state(&state, &t).await;
    if install_state.get("status").and_then(Value::as_str) != Some("installed") {
        return Ok(());
    }
    let active_lock = get_active_acme_runtime_lock(&state).await?;
    if active_lock.get("locked").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }

    let threshold_seconds = acme_renew_days() * 24 * 60 * 60;
    let mut renewable = Vec::new();
    for application in read_acme_applications(&state).await? {
        if application.get("renewEnabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let Some(certificate) =
            get_usable_issued_certificate_for_application(&state, &application).await?
        else {
            continue;
        };
        let Some(valid_to) = certificate
            .pointer("/certInfo/validTo")
            .and_then(Value::as_str)
            .and_then(parse_rfc3339_unix_timestamp)
        else {
            continue;
        };
        if valid_to - time_utils::now_ms() / 1000 > threshold_seconds {
            continue;
        }
        renewable.push((valid_to, application));
    }
    renewable.sort_by_key(|(valid_to, _)| *valid_to);

    for (_, application) in renewable {
        match start_acme_application_job(state.clone(), application, "auto_renew", t.clone()).await
        {
            Ok((job, _lock)) => {
                if wait_for_acme_job_completion(&state, &job).await? == Some("stopped".to_string())
                {
                    return Ok(());
                }
            }
            Err(error) => {
                if error.to_string() == t.t("server.acmeJobRunner.activeTaskRunning") {
                    return Ok(());
                }
                tracing::warn!(%error, "failed to start ACME auto-renew job");
            }
        }
    }

    if let Err(error) = reconcile_acme_ssl_deployment(&state).await {
        tracing::warn!(%error, "failed to reconcile ACME SSL deployment after auto-renew");
    }
    Ok(())
}

pub(super) async fn reconcile_acme_ssl_deployment(state: &AppState) -> anyhow::Result<()> {
    let applications = read_acme_applications(state).await?;
    let t = Translator::from_state(state).await;
    let mut config = state.redis.get_config().await?;
    let mut deployment_changed = false;

    for application in applications {
        if application.get("renewEnabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }

        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if application_id.is_empty() {
            continue;
        }

        let result: anyhow::Result<bool> = async {
            let Some(issued_certificate) =
                get_usable_issued_certificate_for_application(state, &application).await?
            else {
                return Ok(false);
            };
            let linked_certificate =
                ssl::get_acme_ssl_certificate_by_source_ref(state, &application_id).await?;
            let library_matches_issued = linked_certificate.as_ref().is_some_and(|certificate| {
                same_pem(
                    certificate.get("cert").and_then(Value::as_str),
                    issued_certificate.get("cert").and_then(Value::as_str),
                ) && same_pem(
                    certificate.get("key").and_then(Value::as_str),
                    issued_certificate.get("key").and_then(Value::as_str),
                )
            });
            if library_matches_issued {
                return Ok(false);
            }

            let linked_id = linked_certificate
                .as_ref()
                .and_then(|certificate| certificate.get("id").and_then(Value::as_str))
                .map(str::to_string);
            let should_activate = linked_id.as_deref().is_some_and(|id| {
                config
                    .pointer("/ssl/active_cert_id")
                    .and_then(Value::as_str)
                    == Some(id)
            });
            let label = linked_certificate
                .as_ref()
                .and_then(|certificate| certificate.get("label").and_then(Value::as_str))
                .map(str::to_string)
                .or_else(|| {
                    application
                        .get("name")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .or_else(|| {
                    application
                        .get("primaryDomain")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });

            save_acme_certificate_to_library_by_application(
                state,
                &application,
                should_activate,
                label.as_deref(),
                &t,
            )
            .await?;
            config = state.redis.get_config().await?;
            Ok(should_activate
                || config
                    .pointer("/ssl/deployment_mode")
                    .and_then(Value::as_str)
                    == Some("multi_sni"))
        }
        .await;

        match result {
            Ok(changed) => deployment_changed |= changed,
            Err(error) => {
                let domain = application
                    .get("primaryDomain")
                    .and_then(Value::as_str)
                    .unwrap_or(&application_id);
                tracing::warn!(%error, %domain, "ACME certificate library reconcile failed");
            }
        }
    }

    let certificates = config
        .pointer("/ssl/certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let active_cert_id = config
        .pointer("/ssl/active_cert_id")
        .and_then(Value::as_str);
    let active_certificate = certificates
        .iter()
        .find(|certificate| certificate.get("id").and_then(Value::as_str) == active_cert_id);
    let has_acme_certificate = certificates
        .iter()
        .any(|certificate| certificate.get("source").and_then(Value::as_str) == Some("acme"));
    let deployment_mode = config
        .pointer("/ssl/deployment_mode")
        .and_then(Value::as_str);
    let should_sync = deployment_changed
        || (has_acme_certificate
            && (deployment_mode == Some("multi_sni")
                || active_certificate
                    .and_then(|certificate| certificate.get("source").and_then(Value::as_str))
                    == Some("acme")));
    if should_sync {
        ssl::sync_ssl_deployment_to_gateway(state, Some(&config)).await?;
    }
    Ok(())
}

pub(super) fn same_pem(left: Option<&str>, right: Option<&str>) -> bool {
    left.unwrap_or("").trim() == right.unwrap_or("").trim()
}

pub(super) async fn wait_for_acme_job_completion(
    state: &AppState,
    job: &Value,
) -> anyhow::Result<Option<String>> {
    let Some(job_id) = job.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    for _ in 0..acme_renew_wait_iterations() {
        if let Some(latest) = get_acme_job(state, job_id).await?
            && let Some(status) = latest.get("status").and_then(Value::as_str)
            && matches!(status, "succeeded" | "failed" | "stopped")
        {
            return Ok(Some(status.to_string()));
        }
        tokio_time::sleep(std::time::Duration::from_secs(5)).await;
    }
    Ok(None)
}

pub(super) fn acme_renew_interval() -> std::time::Duration {
    std::time::Duration::from_secs(
        env::var("ACME_RENEW_INTERVAL_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(6 * 60 * 60)
            .clamp(60, 7 * 24 * 60 * 60),
    )
}

pub(super) fn acme_renew_days() -> i64 {
    env::var("ACME_RENEW_DAYS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 90)
}

pub(super) fn acme_renew_lock_ttl_seconds() -> usize {
    env::var("ACME_RENEW_LOCK_TTL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(3600)
        .clamp(60, 6 * 60 * 60)
}

pub(super) fn acme_renew_wait_iterations() -> usize {
    env::var("ACME_RENEW_WAIT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(2 * 60 * 60)
        .clamp(60, 24 * 60 * 60)
        / 5
}

pub(super) fn parse_rfc3339_unix_timestamp(value: &str) -> Option<i64> {
    OffsetDateTime::parse(value.trim(), &Rfc3339)
        .ok()
        .map(|value| value.unix_timestamp())
}
