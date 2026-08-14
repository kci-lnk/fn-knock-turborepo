use super::*;

const DEFAULT_ACME_RENEW_CRON: &str = "0 */6 * * *";
const DEFAULT_ACME_RENEW_INTERVAL_SECONDS: u64 = 6 * 60 * 60;
pub(super) const ACME_RENEW_LOCK_KEY: &str = "fn_knock:lock:acme-renew";

#[derive(Clone, Debug)]
struct AcmeRenewLease {
    lock_id: String,
    started_at: String,
}

pub(super) async fn run_acme_auto_renew_once(state: AppState) -> anyhow::Result<()> {
    let Some(lease) = try_acquire_acme_renew_lease(&state).await? else {
        tracing::debug!("skipping ACME auto-renew because another scan owns the lease");
        return Ok(());
    };

    let result =
        with_acme_renew_lease(&state, &lease, run_acme_auto_renew_locked(state.clone())).await;
    match state
        .storage
        .store
        .delete_lock_if_owned(ACME_RENEW_LOCK_KEY, &lease.lock_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(
                lock_id = %lease.lock_id,
                "ACME auto-renew scan lease was no longer owned during release"
            );
        }
        Err(error) => {
            tracing::warn!(%error, lock_id = %lease.lock_id, "failed to release ACME auto-renew scan lease");
        }
    }
    result
}

async fn run_acme_auto_renew_locked(state: AppState) -> anyhow::Result<()> {
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
    let now = time_utils::now_ms() / 1000;
    let mut renewable = Vec::new();
    for application in read_acme_applications(&state).await? {
        if application.get("renewEnabled").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        if !auto_renew_retry_allowed(&application, now) {
            let application_id = application
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            tracing::info!(
                %application_id,
                "skipping ACME auto-renew during failure backoff"
            );
            continue;
        }
        let Some(certificate) =
            get_usable_issued_certificate_for_application(&state, &application).await?
        else {
            continue;
        };
        let Some(valid_to) = parse_acme_certificate_expiration(&certificate) else {
            let application_id = application
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let primary_domain = application
                .get("primaryDomain")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let valid_to = certificate
                .pointer("/certInfo/validTo")
                .and_then(Value::as_str)
                .unwrap_or_default();
            tracing::warn!(
                %application_id,
                %primary_domain,
                %valid_to,
                "skipping ACME auto-renew because certificate expiration is invalid"
            );
            continue;
        };
        if !certificate_due_for_renewal(valid_to, now, threshold_seconds) {
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

fn auto_renew_retry_allowed(application: &Value, now: i64) -> bool {
    auto_renew_retry_allowed_with_backoff(application, now, acme_renew_failure_backoff_seconds())
}

pub(super) fn auto_renew_retry_allowed_with_backoff(
    application: &Value,
    now: i64,
    backoff_seconds: i64,
) -> bool {
    if !matches!(
        application.get("latestJobStatus").and_then(Value::as_str),
        Some("failed" | "stopped")
    ) {
        return true;
    }
    let Some(latest_job_at) = application
        .get("latestJobAt")
        .and_then(Value::as_str)
        .and_then(parse_certificate_unix_timestamp)
    else {
        return true;
    };
    let updated_at = application
        .get("updatedAt")
        .and_then(Value::as_str)
        .and_then(parse_certificate_unix_timestamp);
    if updated_at.is_some_and(|updated_at| updated_at > latest_job_at) {
        return true;
    }
    now.saturating_sub(latest_job_at) >= backoff_seconds
}

fn acme_renew_failure_backoff_seconds() -> i64 {
    env::var("ACME_RENEW_FAILURE_BACKOFF_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(6 * 60 * 60)
        .clamp(60, 7 * 24 * 60 * 60)
}

async fn try_acquire_acme_renew_lease(
    state: &AppState,
) -> crate::storage::StorageResult<Option<AcmeRenewLease>> {
    let lease = AcmeRenewLease {
        lock_id: uuid::Uuid::new_v4().to_string(),
        started_at: now_node_iso(),
    };
    let acquired = state
        .storage
        .store
        .set_json_value_nx_ex(
            ACME_RENEW_LOCK_KEY,
            &acme_renew_lease_value(&lease),
            acme_renew_lock_ttl_seconds(),
        )
        .await?;
    Ok(acquired.then_some(lease))
}

fn acme_renew_lease_value(lease: &AcmeRenewLease) -> Value {
    json!({
        "lockId": lease.lock_id,
        "startedAt": lease.started_at,
        "heartbeatAt": now_node_iso(),
    })
}

async fn with_acme_renew_lease<T>(
    state: &AppState,
    lease: &AcmeRenewLease,
    work: impl std::future::Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    tokio::pin!(work);
    let heartbeat_seconds = (acme_renew_lock_ttl_seconds() / 3).clamp(30, 300) as u64;
    let mut heartbeat = tokio_time::interval(std::time::Duration::from_secs(heartbeat_seconds));
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            result = &mut work => return result,
            _ = heartbeat.tick() => {
                let refreshed = state
                    .storage
                    .store
                    .set_json_lock_if_owned_ex(
                        ACME_RENEW_LOCK_KEY,
                        &lease.lock_id,
                        &acme_renew_lease_value(lease),
                        acme_renew_lock_ttl_seconds(),
                    )
                    .await?;
                if !refreshed {
                    anyhow::bail!("ACME auto-renew scan lease was lost");
                }
            }
        }
    }
}

pub(super) async fn reconcile_acme_ssl_deployment(state: &AppState) -> anyhow::Result<()> {
    let applications = read_acme_applications(state).await?;
    let t = Translator::from_state(state).await;
    let mut config = state.storage.store.get_config().await?;
    let previous_ssl = config.get("ssl").cloned();
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
                replacement_library_certificate(&config, &application_id, &issued_certificate);
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
            if let Some(linked_id) = linked_id.as_deref() {
                link_issued_certificate_to_library(state, &application_id, linked_id).await?;
            }
            let normalized_ssl = ssl::normalize_ssl_config(config.get("ssl"));
            let should_activate = linked_id.as_deref().is_some_and(|id| {
                normalized_ssl.get("active_cert_id").and_then(Value::as_str) == Some(id)
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
            config = state.storage.store.get_config().await?;
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
        sync_ssl_deployment_with_rollback(state, previous_ssl.as_ref(), &config).await?;
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
    let cron = env::var("ACME_RENEW_CRON").ok();
    let interval_seconds = env::var("ACME_RENEW_INTERVAL_SECONDS").ok();
    acme_renew_interval_from_values(cron.as_deref(), interval_seconds.as_deref())
}

pub(super) fn acme_renew_interval_from_values(
    cron: Option<&str>,
    interval_seconds: Option<&str>,
) -> std::time::Duration {
    let seconds = cron
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            crate::settings::parse_cron_interval_seconds(value, DEFAULT_ACME_RENEW_INTERVAL_SECONDS)
        })
        .or_else(|| {
            interval_seconds
                .and_then(crate::node_compat::parse_i64_prefix_trim_start)
                .and_then(|value| u64::try_from(value).ok())
                .filter(|value| *value > 0)
        })
        .unwrap_or_else(|| {
            crate::settings::parse_cron_interval_seconds(
                DEFAULT_ACME_RENEW_CRON,
                DEFAULT_ACME_RENEW_INTERVAL_SECONDS,
            )
        })
        .clamp(60, 7 * 24 * 60 * 60);
    std::time::Duration::from_secs(seconds)
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

pub(super) fn parse_certificate_unix_timestamp(value: &str) -> Option<i64> {
    let value = value.trim();
    OffsetDateTime::parse(value, &Rfc3339)
        .ok()
        .map(|value| value.unix_timestamp())
        .or_else(|| parse_openssl_utc_timestamp(value))
}

fn parse_openssl_utc_timestamp(value: &str) -> Option<i64> {
    let mut parts = value.split_ascii_whitespace();
    let month = match parts.next()? {
        "Jan" => Month::January,
        "Feb" => Month::February,
        "Mar" => Month::March,
        "Apr" => Month::April,
        "May" => Month::May,
        "Jun" => Month::June,
        "Jul" => Month::July,
        "Aug" => Month::August,
        "Sep" => Month::September,
        "Oct" => Month::October,
        "Nov" => Month::November,
        "Dec" => Month::December,
        _ => return None,
    };
    let day = parts.next()?.parse::<u8>().ok()?;
    let mut clock = parts.next()?.split(':');
    let hour = clock.next()?.parse::<u8>().ok()?;
    let minute = clock.next()?.parse::<u8>().ok()?;
    let second = clock.next()?.parse::<u8>().ok()?;
    if clock.next().is_some() {
        return None;
    }
    let year = parts.next()?.parse::<i32>().ok()?;
    if !matches!(parts.next(), Some("GMT" | "UTC")) || parts.next().is_some() {
        return None;
    }
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms(hour, minute, second).ok()?;
    Some(
        PrimitiveDateTime::new(date, time)
            .assume_utc()
            .unix_timestamp(),
    )
}

pub(super) fn parse_acme_certificate_expiration(certificate: &Value) -> Option<i64> {
    certificate
        .pointer("/certInfo/validTo")
        .and_then(Value::as_str)
        .and_then(parse_certificate_unix_timestamp)
}

pub(super) fn certificate_due_for_renewal(valid_to: i64, now: i64, threshold_seconds: i64) -> bool {
    valid_to.saturating_sub(now) <= threshold_seconds
}
