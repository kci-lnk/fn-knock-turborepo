use super::*;

struct AcmeExecutionDropGuard {
    heartbeat_stop: CancellationToken,
    control: AcmeJobControl,
}

impl Drop for AcmeExecutionDropGuard {
    fn drop(&mut self) {
        self.heartbeat_stop.cancel();
        #[cfg(unix)]
        let pid = self.control.pid();
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(pid) {
            let _ = send_acme_process_group_signal(pid, libc::SIGKILL);
        }
        self.control.set_pid(0);
        self.control.finished.cancel();
    }
}

pub(super) async fn start_acme_application_job(
    state: AppState,
    application: Value,
    trigger: &str,
    t: Translator,
) -> anyhow::Result<(Value, Value)> {
    let (job, lock) = reserve_acme_application_job(&state, &application, trigger, &t).await?;
    match run_reserved_acme_application_job(
        state.clone(),
        application.clone(),
        trigger,
        job.clone(),
        lock.clone(),
        t.clone(),
    )
    .await
    {
        Ok(result) => Ok(result),
        Err(error) => {
            let message = error.to_string();
            fail_reserved_acme_application_job(&state, &application, &job, &lock, &message, &t)
                .await
                .ok();
            Err(error)
        }
    }
}

pub(super) async fn ensure_acme_installed_for_request(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<()> {
    let install_state = current_acme_install_state(state, t).await;
    match install_state.get("status").and_then(Value::as_str) {
        Some("installed") => {
            return Ok(());
        }
        Some("installing") => {
            anyhow::bail!(acme_route_text(t, "installingRetryLater"));
        }
        _ => {}
    }
    if acme_executable_path(state).is_file() {
        return Ok(());
    }
    anyhow::bail!(acme_route_text(t, "installFirst"));
}

pub(super) async fn reserve_acme_application_job(
    state: &AppState,
    application: &Value,
    trigger: &str,
    t: &Translator,
) -> anyhow::Result<(Value, Value)> {
    ensure_acme_installed_for_request(state, t).await?;
    let active_lock = get_active_acme_runtime_lock(state).await?;
    if active_lock.get("locked").and_then(Value::as_bool) == Some(true) {
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }

    let job = build_queued_acme_job(application, trigger, t)?;
    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let control = state
        .register_acme_job_control(&job_id)
        .await
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.activeTaskRunning")))?;
    let lock = build_acme_runtime_lock(application, &job, trigger);
    let leased_lock = with_runtime_lock_lease(lock);
    let acquired = match state
        .storage
        .store
        .set_json_value_nx_ex(
            ACME_RUNTIME_LOCK_KEY,
            &leased_lock,
            acme_runtime_lock_ttl_seconds(),
        )
        .await
    {
        Ok(acquired) => acquired,
        Err(error) => {
            state.finish_acme_job_control(&job_id).await;
            return Err(error.into());
        }
    };
    if !acquired {
        state.finish_acme_job_control(&job_id).await;
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }

    if let Err(error) = async {
        create_acme_job(state, &job, t).await?;
        clear_acme_logs(state, &job_id).await?;
        update_acme_application_job_state(state, application, &job).await
    }
    .await
    {
        release_acme_runtime_lock(state, &leased_lock).await.ok();
        state.finish_acme_job_control(&job_id).await;
        return Err(error);
    }

    if control.cancellation.is_cancelled() {
        release_acme_runtime_lock(state, &leased_lock).await.ok();
        state.finish_acme_job_control(&job_id).await;
        anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
    }

    Ok((job, leased_lock))
}

pub(super) async fn run_reserved_acme_application_job(
    state: AppState,
    application: Value,
    trigger: &str,
    job: Value,
    lock: Value,
    t: Translator,
) -> anyhow::Result<(Value, Value)> {
    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if job_id.is_empty() {
        anyhow::bail!(t.t("server.store.acme.jobDataInvalid"));
    }
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    let provider = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .or_else(|| {
            application
                .get("dnsType")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    update_acme_job(
        &state,
        &job_id,
        json!({
            "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
            "domains": domains,
            "provider": provider,
            "trigger": normalize_trigger_string(trigger),
        }),
    )
    .await?;

    let run_state = state.clone();
    let run_application = application.clone();
    let run_lock = lock.clone();
    let run_t = t.clone();
    let run_job_id = job_id.clone();
    let control = state
        .acme_job_control(&job_id)
        .await
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeJobRunner.manualStop")))?;
    if control.cancellation.is_cancelled() {
        anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
    }
    let run_control = control.clone();
    let spawned = state.spawn_abortable_background("acme-application-job", async move {
        if let Err(error) = execute_acme_application_job(
            run_state,
            run_application,
            run_job_id,
            run_lock,
            run_control,
            run_t,
        )
        .await
        {
            tracing::warn!(%error, "ACME job runner failed");
        }
    });
    if spawned.is_none() {
        state.finish_acme_job_control(&job_id).await;
        anyhow::bail!("ACME runtime is shutting down");
    }

    Ok((job, lock))
}

pub(super) async fn fail_reserved_acme_application_job(
    state: &AppState,
    application: &Value,
    job: &Value,
    lock: &Value,
    message: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let job_id = job.get("id").and_then(Value::as_str).unwrap_or("");
    let already_stopped = !job_id.is_empty() && acme_job_is_stopped(state, job_id).await?;
    if !job_id.is_empty() && !already_stopped {
        let cancelled = state
            .acme_job_control(job_id)
            .await
            .is_some_and(|control| control.cancellation.is_cancelled());
        let (status, final_message, log_message) = if cancelled {
            let stopped_message = t.t("server.acmeJobRunner.manualStop");
            ("stopped", stopped_message.clone(), stopped_message)
        } else {
            (
                "failed",
                message.to_string(),
                t.t_params(
                    "server.acmeJobRunner.flowFailed",
                    &[("message", message.to_string())],
                ),
            )
        };
        append_acme_log(state, job_id, &log_message).await.ok();
        let finished_at = now_node_iso();
        if let Some(updated) = update_acme_job(
            state,
            job_id,
            json!({
                "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
                "status": status,
                "progress": 100,
                "finishedAt": finished_at,
                "message": final_message,
            }),
        )
        .await?
        {
            update_acme_application_job_state(state, application, &updated).await?;
        }
    }
    release_acme_runtime_lock(state, lock).await.ok();
    state.finish_acme_job_control(job_id).await;
    Ok(())
}

pub(super) fn build_queued_acme_job(
    application: &Value,
    trigger: &str,
    t: &Translator,
) -> anyhow::Result<Value> {
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        anyhow::bail!(t.t("server.acmeRoutes.domainsInvalid"));
    }
    let dns_type = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .or_else(|| {
            application
                .get("dnsType")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    Ok(json!({
        "id": uuid::Uuid::new_v4().to_string(),
        "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
        "domains": domains,
        "method": "dns",
        "provider": dns_type,
        "trigger": normalize_trigger_string(trigger),
        "createdAt": now_node_iso(),
        "status": "queued",
        "progress": 0,
        "message": if trigger == "auto_renew" { "queued for renew" } else { "queued" },
    }))
}

pub(super) fn build_acme_runtime_lock(application: &Value, job: &Value, trigger: &str) -> Value {
    json!({
        "locked": true,
        "lockId": uuid::Uuid::new_v4().to_string(),
        "jobId": job.get("id").and_then(Value::as_str).unwrap_or(""),
        "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
        "reason": normalize_trigger_string(trigger),
        "startedAt": job.get("createdAt").and_then(Value::as_str).unwrap_or(""),
    })
}

pub(super) fn with_runtime_lock_lease(mut lock: Value) -> Value {
    let ttl = acme_runtime_lock_ttl_seconds() as i64;
    lock["heartbeatAt"] = json!(now_node_iso());
    lock["expiresAt"] = json!(iso_after_seconds_node(ttl));
    lock
}

pub(super) fn normalize_trigger_string(value: &str) -> &'static str {
    match value {
        "auto_renew" => "auto_renew",
        _ => "manual_request",
    }
}

pub(super) fn acme_runtime_lock_ttl_seconds() -> usize {
    std::env::var("ACME_RUNTIME_LOCK_TTL")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(900)
        .clamp(
            ACME_RUNTIME_LOCK_MIN_TTL_SECONDS,
            ACME_RUNTIME_LOCK_MAX_TTL_SECONDS,
        )
}

pub(super) async fn create_acme_job(
    state: &AppState,
    job: &Value,
    t: &Translator,
) -> anyhow::Result<()> {
    let job = normalize_acme_job(job.clone())
        .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
    let id = job.get("id").and_then(Value::as_str).unwrap_or("");
    state
        .storage
        .store
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(())
}

pub(super) async fn update_acme_job(
    state: &AppState,
    id: &str,
    patch: Value,
) -> anyhow::Result<Option<Value>> {
    let Some(mut job) = get_acme_job(state, id).await? else {
        return Ok(None);
    };
    if let (Some(job_obj), Some(patch_obj)) = (job.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            job_obj.insert(key.clone(), value.clone());
        }
    }
    let Some(job) = normalize_acme_job(job) else {
        return Ok(None);
    };
    state
        .storage
        .store
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(Some(job))
}

async fn update_running_acme_job(
    state: &AppState,
    id: &str,
    patch: Value,
    t: &Translator,
) -> anyhow::Result<Option<Value>> {
    let Some(mut job) = get_acme_job(state, id).await? else {
        return Ok(None);
    };
    if job.get("status").and_then(Value::as_str) == Some("stopped")
        && patch.get("status").and_then(Value::as_str) != Some("stopped")
    {
        anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
    }
    if let (Some(job_obj), Some(patch_obj)) = (job.as_object_mut(), patch.as_object()) {
        for (key, value) in patch_obj {
            job_obj.insert(key.clone(), value.clone());
        }
    }
    let Some(job) = normalize_acme_job(job) else {
        return Ok(None);
    };
    state
        .storage
        .store
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(Some(job))
}

pub(super) async fn acme_job_is_stopped(state: &AppState, id: &str) -> anyhow::Result<bool> {
    Ok(get_acme_job(state, id)
        .await?
        .is_some_and(|job| job.get("status").and_then(Value::as_str) == Some("stopped")))
}

async fn ensure_acme_job_running(state: &AppState, id: &str, t: &Translator) -> anyhow::Result<()> {
    if acme_job_is_stopped(state, id).await? {
        anyhow::bail!(t.t("server.acmeJobRunner.manualStop"));
    }
    Ok(())
}

async fn append_stopped_ignored_log(state: &AppState, id: &str, t: &Translator) {
    append_acme_log(
        state,
        id,
        &t.t("server.acmeJobRunner.stoppedIgnoredProcessError"),
    )
    .await
    .ok();
}

pub(super) async fn append_acme_log(
    state: &AppState,
    job_id: &str,
    line: &str,
) -> crate::storage::StorageResult<()> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(());
    }
    state
        .storage
        .store
        .append_log_buffer(
            &format!("{ACME_LOGS_PREFIX}{job_id}"),
            &[line.to_string()],
            ACME_JOB_TTL_SECONDS,
            MAX_ACME_LOG_LIMIT,
        )
        .await
}

pub(super) async fn clear_acme_logs(
    state: &AppState,
    job_id: &str,
) -> crate::storage::StorageResult<()> {
    state
        .storage
        .store
        .clear_log_buffer(&format!("{ACME_LOGS_PREFIX}{job_id}"))
        .await
}

pub(super) async fn update_acme_application_job_state(
    state: &AppState,
    application: &Value,
    job: &Value,
) -> anyhow::Result<()> {
    let Some(application_id) = application.get("id").and_then(Value::as_str) else {
        return Ok(());
    };
    let mut applications = read_acme_applications_raw(state).await?;
    let Some(index) = applications
        .iter()
        .position(|item| item.get("id").and_then(Value::as_str) == Some(application_id))
    else {
        return Ok(());
    };
    if let Some(object) = applications[index].as_object_mut() {
        object.insert(
            "latestJobId".to_string(),
            job.get("id").cloned().unwrap_or(Value::Null),
        );
        object.insert(
            "latestJobStatus".to_string(),
            job.get("status").cloned().unwrap_or_else(|| json!("idle")),
        );
        object.insert(
            "latestJobTrigger".to_string(),
            job.get("trigger")
                .cloned()
                .unwrap_or_else(|| json!("manual_request")),
        );
        object.insert(
            "latestJobAt".to_string(),
            job.get("finishedAt")
                .or_else(|| job.get("startedAt"))
                .or_else(|| job.get("createdAt"))
                .cloned()
                .unwrap_or_else(|| json!(now_node_iso())),
        );
        if job.get("status").and_then(Value::as_str) == Some("failed") {
            if let Some(message) = job.get("message").and_then(Value::as_str) {
                object.insert("lastError".to_string(), json!(message));
            }
        } else {
            object.remove("lastError");
        }
    }
    write_acme_applications(state, &applications).await?;
    Ok(())
}

pub(super) async fn release_acme_runtime_lock(
    state: &AppState,
    lock: &Value,
) -> crate::storage::StorageResult<bool> {
    let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) else {
        return Ok(false);
    };
    state
        .storage
        .store
        .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
        .await
}

pub(super) async fn execute_acme_application_job(
    state: AppState,
    application: Value,
    job_id: String,
    lock: Value,
    control: AcmeJobControl,
    t: Translator,
) -> anyhow::Result<()> {
    let heartbeat_stop = CancellationToken::new();
    let _drop_guard = AcmeExecutionDropGuard {
        heartbeat_stop: heartbeat_stop.clone(),
        control: control.clone(),
    };
    let heartbeat_task =
        start_acme_lock_heartbeat(state.clone(), lock.clone(), heartbeat_stop.clone());
    let result = execute_acme_application_job_inner(
        state.clone(),
        application,
        job_id.clone(),
        lock.clone(),
        control.clone(),
        t,
    )
    .await;

    heartbeat_stop.cancel();
    let release_result = release_acme_runtime_lock(&state, &lock).await;
    heartbeat_task.await.ok();
    control.set_pid(0);
    state.finish_acme_job_control(&job_id).await;

    if let Err(error) = release_result {
        tracing::error!(%error, %job_id, "failed to release ACME runtime lock");
        if result.is_ok() {
            return Err(error.into());
        }
    }
    result
}

async fn execute_acme_application_job_inner(
    state: AppState,
    application: Value,
    job_id: String,
    lock: Value,
    control: AcmeJobControl,
    t: Translator,
) -> anyhow::Result<()> {
    let started_at = now_node_iso();
    let running_message = acme_job_running_message(&t, lock.get("reason").and_then(Value::as_str));
    if let Some(job) = update_running_acme_job(
        &state,
        &job_id,
        json!({
            "status": "running",
            "progress": 5,
            "startedAt": started_at,
            "message": running_message,
        }),
        &t,
    )
    .await?
    {
        update_acme_application_job_state(&state, &application, &job).await?;
    }

    let mut previous_issued_certificate = None;
    let mut issued_certificate_commit_started = false;
    let result = async {
        let client_settings = ensure_client_settings(&state).await?;
        let certificate_authority = client_settings
            .get("certificateAuthority")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
            .to_string();
        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
        previous_issued_certificate =
            read_issued_certificates(&state)
                .await?
                .into_iter()
                .find(|certificate| {
                    certificate.get("applicationId").and_then(Value::as_str) == Some(application_id)
                });
        issue_acme_certificate(
            &state,
            &application,
            &job_id,
            &certificate_authority,
            &control,
            &t,
        )
        .await?;
        ensure_acme_job_running(&state, &job_id, &t).await?;
        if let Some(job) = update_running_acme_job(
            &state,
            &job_id,
            json!({
                "progress": 80,
                "message": "saving",
            }),
            &t,
        )
        .await?
        {
            update_acme_application_job_state(&state, &application, &job).await?;
        }
        let latest_application = find_acme_application(&state, application_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(t.t("server.acmeJobRunner.issuedButApplicationChanged"))
            })?;
        if application.get("primaryDomain").and_then(Value::as_str)
            != latest_application
                .get("primaryDomain")
                .and_then(Value::as_str)
            || normalized_domain_signature(
                &application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            ) != normalized_domain_signature(
                &latest_application
                    .get("domains")
                    .and_then(Value::as_array)
                    .map(|values| normalize_domain_list(values.iter()))
                    .unwrap_or_default(),
            )
        {
            append_acme_log(
                &state,
                &job_id,
                &t.t("server.acmeJobRunner.applicationChangedSkipped"),
            )
            .await
            .ok();
            anyhow::bail!(t.t("server.acmeJobRunner.issuedButApplicationChanged"));
        }
        ensure_acme_job_running(&state, &job_id, &t).await?;
        issued_certificate_commit_started = true;
        save_acme_issued_cert_from_fs(&state, &latest_application, &job_id, &t).await?;
        ensure_acme_job_running(&state, &job_id, &t).await?;
        sync_acme_library_after_issue(&state, &latest_application, &job_id, &t).await?;
        if let Some(previous_primary_domain) = previous_issued_certificate
            .as_ref()
            .and_then(|certificate| certificate.get("primaryDomain"))
            .and_then(Value::as_str)
        {
            let current_primary_domain = latest_application
                .get("primaryDomain")
                .and_then(Value::as_str)
                .unwrap_or("");
            match cleanup_superseded_acme_domain_artifacts(
                &state,
                application_id,
                previous_primary_domain,
                current_primary_domain,
            )
            .await
            {
                Ok(true) => {
                    tracing::info!(
                        %application_id,
                        %previous_primary_domain,
                        %current_primary_domain,
                        "removed superseded ACME certificate artifacts"
                    );
                }
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(
                        %error,
                        %application_id,
                        %previous_primary_domain,
                        %current_primary_domain,
                        "failed to remove superseded ACME certificate artifacts"
                    );
                }
            }
        }
        let working_domain = acme_issued_storage_domain(&state, &latest_application);
        if !working_domain.is_empty() {
            match clear_acme_domain_working_state(&state, &working_domain).await {
                Ok(()) => {
                    append_acme_log(
                        &state,
                        &job_id,
                        &t.t("server.acmeJobRunner.clearedDomainWorkingState"),
                    )
                    .await
                    .ok();
                }
                Err(error) => {
                    append_acme_log(
                        &state,
                        &job_id,
                        &t.t_params(
                            "server.acmeJobRunner.clearDomainWorkingStateFailed",
                            &[("message", error.to_string())],
                        ),
                    )
                    .await
                    .ok();
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if result.is_err() && issued_certificate_commit_started {
        let current_primary_domain = application
            .get("primaryDomain")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Err(error) = restore_acme_issued_certificate_snapshot(
            &state,
            application.get("id").and_then(Value::as_str).unwrap_or(""),
            current_primary_domain,
            previous_issued_certificate.as_ref(),
        )
        .await
        {
            tracing::error!(
                %error,
                %job_id,
                "failed to restore ACME issued certificate after job failure"
            );
        }
    }

    async {
        match result {
            Ok(()) => {
                if acme_job_is_stopped(&state, &job_id).await? {
                    append_stopped_ignored_log(&state, &job_id, &t).await;
                } else if let Some(job) = update_running_acme_job(
                    &state,
                    &job_id,
                    json!({
                        "status": "succeeded",
                        "progress": 100,
                        "finishedAt": now_node_iso(),
                        "message": "succeeded",
                    }),
                    &t,
                )
                .await?
                {
                    update_acme_application_job_state(&state, &application, &job).await?;
                }
            }
            Err(error) => {
                let message = error.to_string();
                if acme_job_is_stopped(&state, &job_id).await? {
                    append_stopped_ignored_log(&state, &job_id, &t).await;
                } else if control.cancellation.is_cancelled() {
                    let stopped_message = t.t("server.acmeJobRunner.manualStop");
                    append_acme_log(&state, &job_id, &stopped_message)
                        .await
                        .ok();
                    if let Some(job) = update_running_acme_job(
                        &state,
                        &job_id,
                        json!({
                            "status": "stopped",
                            "progress": 100,
                            "finishedAt": now_node_iso(),
                            "message": stopped_message,
                        }),
                        &t,
                    )
                    .await?
                    {
                        update_acme_application_job_state(&state, &application, &job).await?;
                    }
                } else {
                    append_acme_log(
                        &state,
                        &job_id,
                        &t.t_params(
                            "server.acmeJobRunner.flowFailed",
                            &[("message", message.clone())],
                        ),
                    )
                    .await
                    .ok();
                    if let Some(job) = update_running_acme_job(
                        &state,
                        &job_id,
                        json!({
                            "status": "failed",
                            "progress": 100,
                            "finishedAt": now_node_iso(),
                            "message": message,
                        }),
                        &t,
                    )
                    .await?
                    {
                        update_acme_application_job_state(&state, &application, &job).await?;
                    }
                }
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await
}

pub(super) fn acme_job_running_message(t: &Translator, trigger: Option<&str>) -> String {
    let key = if trigger == Some("auto_renew") {
        "server.acmeJobRunner.lockMessages.autoRenew"
    } else {
        "server.acmeJobRunner.lockMessages.manualRequest"
    };
    t.t(key)
}

pub(super) fn start_acme_lock_heartbeat(
    state: AppState,
    lock: Value,
    stop: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let interval_seconds = (acme_runtime_lock_ttl_seconds() / 3).clamp(30, 60);
        let Some(lock_id) = lock
            .get("lockId")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            return;
        };
        loop {
            tokio::select! {
                _ = stop.cancelled() => break,
                _ = tokio::time::sleep(std::time::Duration::from_secs(interval_seconds as u64)) => {}
            }
            let next = with_runtime_lock_lease(lock.clone());
            match state
                .storage
                .store
                .set_json_lock_if_owned_ex(
                    ACME_RUNTIME_LOCK_KEY,
                    &lock_id,
                    &next,
                    acme_runtime_lock_ttl_seconds(),
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => break,
                Err(error) => {
                    tracing::warn!(%error, "failed to refresh ACME runtime lock");
                }
            }
        }
    })
}

pub(super) async fn issue_acme_certificate(
    state: &AppState,
    application: &Value,
    job_id: &str,
    certificate_authority: &str,
    control: &AcmeJobControl,
    t: &Translator,
) -> anyhow::Result<()> {
    let executable = acme_executable_path(state);
    if !executable.is_file() {
        anyhow::bail!(t.t("server.acmeService.installFirst"));
    }
    let domains = application
        .get("domains")
        .and_then(Value::as_array)
        .map(|values| normalize_domain_list(values.iter()))
        .unwrap_or_default();
    if domains.is_empty() {
        anyhow::bail!(t.t("server.store.acme.domainsRequired"));
    }
    let dns_type = application
        .get("dnsType")
        .and_then(Value::as_str)
        .and_then(normalize_acme_dns_type)
        .ok_or_else(|| anyhow::anyhow!(t.t("server.acmeRoutes.dnsTypeRequired")))?;
    if crate::runtime_profile::deployment_target(state) == "windows" {
        return issue_windows_acme_certificate(
            state,
            application,
            job_id,
            &domains,
            &dns_type,
            control,
            t,
        )
        .await;
    }
    let acme_home = acme_home_dir(state);
    apply_acme_dns_provider_patches(state, &dns_type, job_id, t).await?;
    register_acme_account_for_job(state, certificate_authority, control, t).await?;
    let mut args = vec![
        "--issue".to_string(),
        "--home".to_string(),
        acme_home.to_string_lossy().to_string(),
        "--config-home".to_string(),
        acme_home.to_string_lossy().to_string(),
        "--server".to_string(),
        certificate_authority.to_string(),
        "--force".to_string(),
        "--dns".to_string(),
        dns_type.clone(),
        "--debug".to_string(),
    ];
    for domain in domains {
        args.push("-d".to_string());
        args.push(domain);
    }
    append_acme_log(
        state,
        job_id,
        &format!("$ {}", format_acme_command_for_log(&executable, &args)),
    )
    .await
    .ok();

    let workspace = AcmeCommandWorkspace::prepare(state)?;
    let mut execution_args = args;
    if let Some(workspace) = &workspace {
        workspace.rewrite_home_args(&mut execution_args);
    }
    let mut command = Command::new(executable);
    command
        .args(execution_args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    let env_vars = normalize_acme_env_vars(&dns_type, application.get("credentials"));
    for (key, value) in env_vars {
        if let Some(value) = value.as_str() {
            command.env(key, value);
        }
    }
    if let Some(workspace) = &workspace {
        workspace.configure_command(&mut command);
    }
    let mut child = command.spawn()?;
    control.set_pid(child.id().unwrap_or(0));
    let stdout_task = spawn_acme_log_stream(state.clone(), job_id.to_string(), child.stdout.take());
    let stderr_task = spawn_acme_log_stream(state.clone(), job_id.to_string(), child.stderr.take());
    let status = wait_for_acme_child(&mut child, control, t).await;
    wait_for_acme_output_task(stdout_task).await;
    wait_for_acme_output_task(stderr_task).await;
    let status = status?;
    if status.success() {
        return Ok(());
    }
    anyhow::bail!(t.t_params(
        "server.acmeService.issueFailed",
        &[
            ("code", status.code().unwrap_or(-1).to_string()),
            ("brief", String::new())
        ],
    ))
}

async fn issue_windows_acme_certificate(
    state: &AppState,
    application: &Value,
    job_id: &str,
    domains: &[String],
    dns_type: &str,
    control: &AcmeJobControl,
    t: &Translator,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        windows_acme_provider_ids().contains(&dns_type),
        "The selected DNS provider is not supported by the Windows ACME client"
    );
    let executable = acme_executable_path(state);
    anyhow::ensure!(executable.is_file(), t.t("server.acmeService.installFirst"));
    let email = resolve_account_email(state, None).await;
    let mut args = vec![
        "--issue".to_string(),
        "--home".to_string(),
        acme_home_dir(state).to_string_lossy().to_string(),
        "--config-home".to_string(),
        acme_home_dir(state).to_string_lossy().to_string(),
        "--server".to_string(),
        "letsencrypt".to_string(),
        "--email".to_string(),
        email,
        "--force".to_string(),
        "--dns".to_string(),
        dns_type.to_string(),
        "--debug".to_string(),
        "--dnssleep".to_string(),
        "30".to_string(),
        "--no-save-credentials".to_string(),
    ];
    for domain in domains {
        args.push("-d".to_string());
        args.push(domain.clone());
    }
    append_acme_log(
        state,
        job_id,
        &format!("$ rust-acmesh --issue --dns {dns_type} [credentials redacted]"),
    )
    .await
    .ok();
    let credentials = normalize_acme_env_vars(dns_type, application.get("credentials"));
    let mut command = Command::new(&executable);
    command
        .args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.as_std_mut().creation_flags(0x0800_0000);
    }
    for (key, value) in credentials {
        if let Some(value) = value.as_str() {
            command.env(key, value);
        }
    }
    let mut child = command.spawn()?;
    control.set_pid(child.id().unwrap_or(0));
    let stdout_task = spawn_acme_log_stream(state.clone(), job_id.to_string(), child.stdout.take());
    let stderr_task = spawn_acme_log_stream(state.clone(), job_id.to_string(), child.stderr.take());
    let status = wait_for_acme_child(&mut child, control, t).await;
    wait_for_acme_output_task(stdout_task).await;
    wait_for_acme_output_task(stderr_task).await;
    let status = status?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!(t.t_params(
            "server.acmeService.issueFailed",
            &[
                ("code", status.code().unwrap_or(-1).to_string()),
                ("brief", String::new())
            ]
        ))
    }
}

pub(super) async fn wait_for_acme_child(
    child: &mut tokio::process::Child,
    control: &AcmeJobControl,
    t: &Translator,
) -> anyhow::Result<std::process::ExitStatus> {
    let timeout = tokio_time::sleep(acme_job_timeout());
    tokio::pin!(timeout);
    let result = tokio::select! {
        status = child.wait() => status.map_err(anyhow::Error::from),
        _ = control.cancellation.cancelled() => {
            let termination = terminate_acme_child(child, acme_stop_grace_period()).await;
            match termination {
                Ok(_) => Err(anyhow::anyhow!(t.t("server.acmeJobRunner.manualStop"))),
                Err(error) => Err(error),
            }
        }
        _ = &mut timeout => {
            let termination = terminate_acme_child(child, acme_stop_grace_period()).await;
            match termination {
                Ok(_) => Err(anyhow::anyhow!("ACME certificate request exceeded its execution timeout")),
                Err(error) => Err(error),
            }
        }
    };
    control.set_pid(0);
    result
}

pub(super) async fn terminate_acme_child(
    child: &mut tokio::process::Child,
    grace: std::time::Duration,
) -> anyhow::Result<std::process::ExitStatus> {
    let pid = child.id().unwrap_or(0);
    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(pid) {
        let _ = send_acme_process_group_signal(pid, libc::SIGTERM);
    }
    #[cfg(not(unix))]
    {
        let _ = child.start_kill();
    }

    if let Ok(status) = tokio_time::timeout(grace, child.wait()).await {
        let status = status.map_err(anyhow::Error::from)?;
        #[cfg(unix)]
        if let Ok(pid) = i32::try_from(pid)
            && acme_process_group_exists(pid)
        {
            let _ = send_acme_process_group_signal(pid, libc::SIGKILL);
        }
        return Ok(status);
    }

    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(pid) {
        let _ = send_acme_process_group_signal(pid, libc::SIGKILL);
    }
    let _ = child.start_kill();
    tokio_time::timeout(grace, child.wait())
        .await
        .map_err(|_| anyhow::anyhow!("ACME process did not exit after forced termination"))?
        .map_err(anyhow::Error::from)
}

#[cfg(unix)]
fn send_acme_process_group_signal(pid: i32, signal: libc::c_int) -> std::io::Result<()> {
    if pid <= 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "process group id must be positive",
        ));
    }
    // SAFETY: the child is created as a process-group leader, and a negative
    // pid asks kill(2) to signal only that group. No Rust memory is accessed.
    if unsafe { libc::kill(-pid, signal) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(unix)]
fn acme_process_group_exists(pid: i32) -> bool {
    if pid <= 0 {
        return false;
    }
    // SAFETY: signal 0 performs an existence/permission check only.
    if unsafe { libc::kill(-pid, 0) } == 0 {
        true
    } else {
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

async fn wait_for_acme_output_task(mut task: tokio::task::JoinHandle<()>) {
    if tokio_time::timeout(std::time::Duration::from_secs(2), &mut task)
        .await
        .is_err()
    {
        task.abort();
    }
}

fn spawn_acme_log_stream<R>(
    state: AppState,
    job_id: String,
    stream: Option<R>,
) -> tokio::task::JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        if let Some(stream) = stream {
            append_acme_stream_lines(state, job_id, stream).await;
        }
    })
}

pub(super) fn spawn_acme_output_collector<R>(stream: Option<R>) -> tokio::task::JoinHandle<Vec<u8>>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            stream.read_to_end(&mut bytes).await.ok();
        }
        bytes
    })
}

pub(super) async fn wait_for_acme_collected_output(
    mut task: tokio::task::JoinHandle<Vec<u8>>,
) -> String {
    match tokio_time::timeout(std::time::Duration::from_secs(2), &mut task).await {
        Ok(Ok(bytes)) => String::from_utf8_lossy(&bytes).to_string(),
        Ok(Err(_)) => String::new(),
        Err(_) => {
            task.abort();
            String::new()
        }
    }
}

pub(super) fn acme_job_timeout() -> std::time::Duration {
    let seconds = std::env::var("ACME_JOB_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30 * 60)
        .clamp(60, 6 * 60 * 60);
    std::time::Duration::from_secs(seconds)
}

fn acme_stop_grace_period() -> std::time::Duration {
    std::time::Duration::from_secs(3)
}

pub(super) async fn append_acme_stream_lines<R>(state: AppState, job_id: String, stream: R)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        append_acme_log(&state, &job_id, &line).await.ok();
    }
}
