use super::*;

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
    let lock = build_acme_runtime_lock(application, &job, trigger);
    let leased_lock = with_runtime_lock_lease(lock);
    let acquired = state
        .store
        .set_json_value_nx_ex(
            ACME_RUNTIME_LOCK_KEY,
            &leased_lock,
            acme_runtime_lock_ttl_seconds(),
        )
        .await?;
    if !acquired {
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }

    let job_id = job
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if let Err(error) = async {
        create_acme_job(state, &job, t).await?;
        clear_acme_logs(state, &job_id).await?;
        update_acme_application_job_state(state, application, &job).await
    }
    .await
    {
        release_acme_runtime_lock(state, &leased_lock).await.ok();
        return Err(error);
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
    tokio::spawn(async move {
        if let Err(error) =
            execute_acme_application_job(run_state, run_application, run_job_id, run_lock, run_t)
                .await
        {
            tracing::warn!(%error, "ACME job runner failed");
        }
    });

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
    if !job_id.is_empty() {
        append_acme_log(
            state,
            job_id,
            &t.t_params(
                "server.acmeJobRunner.flowFailed",
                &[("message", message.to_string())],
            ),
        )
        .await
        .ok();
        let finished_at = now_node_iso();
        if let Some(updated) = update_acme_job(
            state,
            job_id,
            json!({
                "applicationId": application.get("id").and_then(Value::as_str).unwrap_or(""),
                "status": "failed",
                "progress": 100,
                "finishedAt": finished_at,
                "message": message,
            }),
        )
        .await?
        {
            update_acme_application_job_state(state, application, &updated).await?;
        }
    }
    release_acme_runtime_lock(state, lock).await.ok();
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
        .store
        .set_json_value_ex(
            &format!("{ACME_JOB_PREFIX}{id}"),
            &job,
            ACME_JOB_TTL_SECONDS,
        )
        .await?;
    Ok(Some(job))
}

async fn acme_job_is_stopped(state: &AppState, id: &str) -> anyhow::Result<bool> {
    Ok(get_acme_job(state, id)
        .await?
        .is_some_and(|job| job.get("status").and_then(Value::as_str) == Some("stopped")))
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
        .store
        .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
        .await
}

pub(super) async fn execute_acme_application_job(
    state: AppState,
    application: Value,
    job_id: String,
    lock: Value,
    t: Translator,
) -> anyhow::Result<()> {
    let heartbeat_stop = Arc::new(AtomicBool::new(false));
    let heartbeat_task =
        start_acme_lock_heartbeat(state.clone(), lock.clone(), heartbeat_stop.clone());
    let started_at = now_node_iso();
    let running_message = t.t("server.acmeJobRunner.lockMessages.manualRequest");
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

    let result = async {
        let client_settings = ensure_client_settings(&state).await?;
        let certificate_authority = client_settings
            .get("certificateAuthority")
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_ACME_CERTIFICATE_AUTHORITY)
            .to_string();
        issue_acme_certificate(&state, &application, &job_id, &certificate_authority, &t).await?;
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
        let application_id = application
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!(t.t("server.store.acme.jobDataInvalid")))?;
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
        save_acme_issued_cert_from_fs(&state, &latest_application, &job_id, &t).await?;
        if let Some(primary_domain) = latest_application
            .get("primaryDomain")
            .and_then(Value::as_str)
        {
            match clear_acme_domain_working_state(&state, primary_domain).await {
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
        sync_acme_library_after_issue(&state, &latest_application, &job_id, &t).await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

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

    heartbeat_stop.store(true, Ordering::Relaxed);
    heartbeat_task.await.ok();
    release_acme_runtime_lock(&state, &lock).await.ok();
    Ok(())
}

pub(super) fn start_acme_lock_heartbeat(
    state: AppState,
    lock: Value,
    stop: Arc<AtomicBool>,
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
        while !stop.load(Ordering::Relaxed) {
            tokio::time::sleep(std::time::Duration::from_secs(interval_seconds as u64)).await;
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let next = with_runtime_lock_lease(lock.clone());
            match state
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
    let acme_home = acme_home_dir(state);
    apply_acme_dns_provider_patches(state, &dns_type, job_id, t).await?;
    register_acme_account(state, None, Some(certificate_authority), t).await?;
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
        &format!("$ {} {}", executable.display(), args.join(" ")),
    )
    .await
    .ok();

    let mut command = Command::new(executable);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let env_vars = normalize_acme_env_vars(&dns_type, application.get("credentials"));
    for (key, value) in env_vars {
        if let Some(value) = value.as_str() {
            command.env(key, value);
        }
    }
    let mut child = command.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let out_state = state.clone();
    let out_job = job_id.to_string();
    let err_state = state.clone();
    let err_job = job_id.to_string();
    let stdout_task = tokio::spawn(async move {
        if let Some(stream) = stdout {
            append_acme_stream_lines(out_state, out_job, stream).await;
        }
    });
    let stderr_task = tokio::spawn(async move {
        if let Some(stream) = stderr {
            append_acme_stream_lines(err_state, err_job, stream).await;
        }
    });
    let status = child.wait().await?;
    let _ = stdout_task.await;
    let _ = stderr_task.await;
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

pub(super) async fn append_acme_stream_lines<R>(state: AppState, job_id: String, stream: R)
where
    R: AsyncRead + Unpin,
{
    let mut lines = BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        append_acme_log(&state, &job_id, &line).await.ok();
    }
}
