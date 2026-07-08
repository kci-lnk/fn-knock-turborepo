use super::*;

pub(super) async fn stop_active_acme_job(
    state: &AppState,
    t: &Translator,
) -> anyhow::Result<Value> {
    let lock = get_active_acme_runtime_lock(state).await?;
    let message = t.t("server.acmeJobRunner.manualStop");
    let mut job = Value::Null;
    if lock.get("locked").and_then(Value::as_bool) == Some(true)
        && let Some(job_id) = lock.get("jobId").and_then(Value::as_str)
        && let Some(updated) = update_acme_job(
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
    {
        append_acme_log(state, job_id, &message).await.ok();
        if let Some(application_id) = updated.get("applicationId").and_then(Value::as_str)
            && let Some(application) = find_acme_application(state, application_id).await?
        {
            update_acme_application_job_state(state, &application, &updated).await?;
        }
        job = updated;
    }
    let process_result = stop_all_acme_processes(t).await;
    if let Some(lock_id) = lock.get("lockId").and_then(Value::as_str) {
        state
            .redis
            .delete_lock_if_owned(ACME_RUNTIME_LOCK_KEY, lock_id)
            .await
            .ok();
    }
    Ok(json!({
        "stopped": !job.is_null(),
        "job": job,
        "lock": lock,
        "processResult": process_result,
    }))
}

pub(super) async fn stop_all_acme_processes(t: &Translator) -> Value {
    let matched_pids = find_acme_process_ids().await.unwrap_or_default();
    let mut errors = Vec::new();
    for pid in &matched_pids {
        if let Err(error) = crate::unix::send_signal(*pid, libc::SIGTERM) {
            errors.push(t.t_params(
                "server.acmeService.sendSignalFailed",
                &[
                    ("signal", "SIGTERM".to_string()),
                    ("target", pid.to_string()),
                    ("detail", error.to_string()),
                ],
            ));
        }
    }
    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
    let remaining_pids = find_acme_process_ids().await.unwrap_or_default();
    json!({
        "matchedPids": matched_pids,
        "remainingPids": remaining_pids,
        "errors": errors,
    })
}

pub(super) async fn find_acme_process_ids() -> anyhow::Result<Vec<i32>> {
    let output = Command::new("ps")
        .args(["-eo", "pid=,command="])
        .output()
        .await?;
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let current_pid = std::process::id() as i32;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut ids = BTreeSet::new();
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        let Some((pid_part, command)) = trimmed.split_once(char::is_whitespace) else {
            continue;
        };
        let Ok(pid) = pid_part.trim().parse::<i32>() else {
            continue;
        };
        if pid <= 0 || pid == current_pid || !command.contains("acme.sh") {
            continue;
        }
        ids.insert(pid);
    }
    Ok(ids.into_iter().collect())
}
