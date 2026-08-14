use super::*;

const ACME_STOP_WAIT_SECONDS: u64 = 15;

pub(super) async fn recover_orphaned_acme_runtime_job(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<bool> {
    let recovered = recover_orphaned_acme_jobs(state, t).await?;
    let Some(raw_lock) = state
        .storage
        .store
        .get_json_value(ACME_RUNTIME_LOCK_KEY)
        .await?
    else {
        return Ok(recovered);
    };
    let lock = normalize_runtime_lock(&raw_lock);
    if lock.get("locked").and_then(Value::as_bool) != Some(true) {
        state
            .storage
            .store
            .delete_key(ACME_RUNTIME_LOCK_KEY)
            .await?;
        return Ok(true);
    }

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
    }

    if let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) {
        state
            .storage
            .store
            .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
            .await?;
    } else {
        // This process has not started any ACME executor yet, so a malformed
        // persisted lease can only be leftover state from an interrupted run.
        state
            .storage
            .store
            .delete_key(ACME_RUNTIME_LOCK_KEY)
            .await?;
    }
    Ok(true)
}

async fn recover_orphaned_acme_jobs(state: &AppState, t: &Translator) -> anyhow::Result<bool> {
    let message = t.t("server.acmeJobRunner.manualStop");
    let mut recovered = false;
    for application in read_acme_applications(state).await? {
        if !matches!(
            application.get("latestJobStatus").and_then(Value::as_str),
            Some("queued" | "running")
        ) {
            continue;
        }
        let Some(job_id) = application.get("latestJobId").and_then(Value::as_str) else {
            continue;
        };
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

    if let Some(control) = state.acme_job_control(&job_id).await {
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
