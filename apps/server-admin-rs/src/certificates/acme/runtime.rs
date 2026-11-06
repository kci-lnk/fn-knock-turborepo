use super::*;

const ACME_STOP_WAIT_SECONDS: u64 = 15;

pub(super) async fn recover_orphaned_acme_runtime_job(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<bool> {
    // Only recover while owning the same lease used by issuance and deletion.
    // Finite TTLs cover live workers in other processes as well as restarts.
    let store = &state.storage.store;
    let previous = store.get_json_value(ACME_RUNTIME_LOCK_KEY).await?;
    if let Some(raw) = &previous {
        if store.ttl_seconds(ACME_RUNTIME_LOCK_KEY).await? >= 0 {
            return Ok(false);
        }
        // Non-expiring legacy leases can only be removed by exact value.
        store
            .delete_key_if_value(ACME_RUNTIME_LOCK_KEY, &raw.to_string())
            .await?;
    }
    let recovery = with_runtime_lock_lease(
        json!({"locked":true,"lockId":uuid::Uuid::new_v4().to_string(),"startedAt":now_node_iso()}),
    );
    if !store
        .set_json_value_nx_ex(
            ACME_RUNTIME_LOCK_KEY,
            &recovery,
            acme_runtime_lock_ttl_seconds(),
        )
        .await?
    {
        return Ok(false);
    }
    let result = tokio_time::timeout(std::time::Duration::from_secs(120), async {
        let recovered = recover_orphaned_acme_jobs(state, t).await?;
        let Some(raw) = previous else {
            return Ok(recovered);
        };
        let lock = normalize_runtime_lock(&raw);
        let message = t.t("server.acmeJobRunner.manualStop");
        if let Some(job_id) = lock.get("jobId").and_then(Value::as_str) {
            let status = get_acme_job(state, job_id).await?.and_then(|job| {
                job.get("status")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            if matches!(status.as_deref(), Some("queued" | "running") | None) {
                mark_acme_job_stopped(state, job_id, &message).await?;
                tracing::warn!(%job_id, "recovered an orphaned ACME job after restart");
            }
            if let Some(id) = lock["applicationId"].as_str()
                && let Some(application) = find_acme_application(state, id).await?
                && let Err(error) = preflight::finish(state, &application, job_id, t).await
            {
                tracing::warn!(%error,%job_id,"could not recheck DNS after restart");
            }
        }

        Ok(true)
    })
    .await;
    release_acme_runtime_lock(state, &recovery).await?;
    result?
}

pub(super) async fn recover_orphaned_acme_jobs(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<bool> {
    flush_interrupted_rates(state).await?;
    let message = t.t("server.acmeJobRunner.manualStop");
    let mut recovered = false;
    for application in read_acme_applications(state).await? {
        let Some(job_id) = application.get("latestJobId").and_then(Value::as_str) else {
            continue;
        };
        let baseline = state
            .storage
            .store
            .get_json_value(&format!("fn_knock:acme:dns-baseline:{job_id}"))
            .await?;
        if let Some(baseline) = &baseline {
            restore_interrupted_limit(state, &application, job_id, baseline).await?;
        }
        if !matches!(
            application["latestJobStatus"].as_str(),
            Some("queued" | "running")
        ) {
            if baseline.is_some() {
                preflight::finish(state, &application, job_id, t).await?;
            }
            continue;
        }
        let job = if let Some(job) = get_acme_job(state, job_id).await? {
            job
        } else {
            let recovered_job = json!({
                "id": job_id,
                "applicationId": application.get("id").cloned().unwrap_or(Value::Null),
                "domains": application.get("domains").cloned().unwrap_or_else(|| json!([])),
                "method": "dns",
                "provider": application.get("dnsType").cloned().unwrap_or(Value::Null),
                "trigger": application.get("latestJobTrigger").cloned().unwrap_or_else(|| json!("manual_request")),
                "createdAt": application.get("latestJobAt").or_else(|| application.get("updatedAt")).cloned().unwrap_or_else(|| json!(now_node_iso())),
                "finishedAt": now_node_iso(),
                "status": "stopped",
                "progress": 100,
                "message": message.clone(),
            });
            create_acme_job(state, &recovered_job, t).await?;
            let Some(job) = get_acme_job(state, job_id).await? else {
                continue;
            };
            recovered = true;
            job
        };
        if matches!(
            job.get("status").and_then(Value::as_str),
            Some("queued" | "running")
        ) {
            if let Err(error) = preflight::finish(state, &application, job_id, t).await {
                tracing::warn!(%error,%job_id,"interrupted DNS cleanup check failed");
            }
            recovered |= mark_acme_job_stopped(state, job_id, &message)
                .await?
                .is_some();
        } else {
            update_acme_application_job_state(state, &application, &job).await?;
        }
    }
    Ok(recovered)
}

pub(super) async fn stop_active_acme_job(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<Value> {
    let lock = get_active_acme_runtime_lock(state).await?;
    if lock.get("locked").and_then(Value::as_bool) != Some(true) {
        return Ok(json!({
            "stopped": false,
            "job": Value::Null,
            "lock": lock,
            "processResult": empty_acme_process_result(),
        }));
    }

    let message = t.t("server.acmeJobRunner.manualStop");
    let job_id = lock
        .get("jobId")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    anyhow::ensure!(!job_id.is_empty(), "DNS cleanup is in progress");
    // Cancellation must reach the executor before status updates wait for the
    // commit/output lock. Retain the control even if it finishes meanwhile.
    let control = state.acme_job_control(&job_id).await;
    if control.is_none()
        && state
            .storage
            .store
            .ttl_seconds(ACME_RUNTIME_LOCK_KEY)
            .await?
            >= 0
    {
        anyhow::bail!(t.t("server.acmeJobRunner.activeTaskRunning"));
    }
    if let Some(control) = &control {
        control.cancellation.cancel();
    }
    let mut job = if job_id.is_empty() {
        Value::Null
    } else {
        mark_acme_job_stopped(state, &job_id, &message)
            .await?
            .unwrap_or(Value::Null)
    };

    let mut matched_pids = Vec::new();
    let mut remaining_pids = Vec::new();
    let mut errors = Vec::new();
    let mut executor_finished = true;

    if let Some(control) = control {
        let pid = control.pid();
        if pid > 0 {
            matched_pids.push(pid);
        }
        control.cancellation.cancel();
        executor_finished = tokio_time::timeout(
            std::time::Duration::from_secs(ACME_STOP_WAIT_SECONDS),
            control.finished.cancelled(),
        )
        .await
        .is_ok();
        if !executor_finished {
            errors.push("Timed out while waiting for the ACME executor to stop".to_string());
        }
        if pid > 0
            && i32::try_from(pid)
                .ok()
                .is_some_and(crate::unix::process_exists)
        {
            remaining_pids.push(pid);
            errors.push("The ACME process is still running after cancellation".to_string());
        }
    } else {
        // No executor in this process owns the lease. This is the recovery
        // path for a container restart or an older persisted stopped job.
        release_orphaned_acme_runtime_lock(state, &lock).await?;
    }

    if job.is_null() && !job_id.is_empty() {
        job = mark_acme_job_stopped(state, &job_id, &message)
            .await?
            .unwrap_or(Value::Null);
    }

    let current_lock = get_active_acme_runtime_lock(state).await?;
    let lock_released = current_lock.get("locked").and_then(Value::as_bool) != Some(true);
    if executor_finished && !lock_released {
        errors.push("The ACME executor stopped but its runtime lock is still active".to_string());
    }
    let stopped = !job.is_null() && executor_finished && lock_released && remaining_pids.is_empty();

    Ok(json!({
        "stopped": stopped,
        "job": job,
        "lock": current_lock,
        "processResult": {
            "matchedPids": matched_pids,
            "remainingPids": remaining_pids,
            "errors": errors,
        },
    }))
}

pub(super) async fn mark_acme_job_stopped(
    state: &AppState,
    job_id: &str,
    message: &str,
) -> anyhow::Result<Option<Value>> {
    let Some(updated) = update_acme_job(
        state,
        job_id,
        json!({
            "status": "stopped",
            "progress": 100,
            "finishedAt": now_node_iso(),
            "message": message,
        }),
    )
    .await?
    else {
        return Ok(None);
    };

    append_acme_log(state, job_id, message).await.ok();
    if let Some(application_id) = updated.get("applicationId").and_then(Value::as_str)
        && let Some(application) = find_acme_application(state, application_id).await?
    {
        update_acme_application_job_state(state, &application, &updated).await?;
    }
    Ok(Some(updated))
}

async fn release_orphaned_acme_runtime_lock(
    state: &AppState,
    lock: &Value,
) -> crate::storage::StorageResult<()> {
    if let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) {
        state
            .storage
            .store
            .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
            .await?;
    } else {
        state
            .storage
            .store
            .delete_key(ACME_RUNTIME_LOCK_KEY)
            .await?;
    }
    Ok(())
}

fn empty_acme_process_result() -> Value {
    json!({ "matchedPids": [], "remainingPids": [], "errors": [] })
}
