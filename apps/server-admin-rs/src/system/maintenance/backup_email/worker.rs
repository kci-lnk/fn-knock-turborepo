use super::*;

pub(in crate::system::maintenance) fn start(state: AppState) {
    state.spawn_background("backup-email", run(state.clone()));
}
async fn run(state: AppState) {
    // Only this worker owns delivery. A persisted Sending job means the prior
    // process stopped before recording its SMTP result (at-least-once delivery).
    let mut worker = DeliveryWorker::new(true);
    loop {
        if state.shutdown.is_cancelled() {
            return;
        }
        if let Err(error) = worker.tick(&state).await {
            tracing::warn!(%error, "backup email worker failed");
        }
        tokio::select! {
            _ = state.shutdown.cancelled() => return,
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
        }
    }
}
// SMTP and storage cannot share a transaction. Keep the known SMTP result in
// memory until bookkeeping commits, instead of sending again on a storage error.
pub(super) struct DeliveryWorker {
    recover: bool,
    pub(super) completion: Option<Completion>,
}
pub(super) struct Completion {
    pub(super) job: DeliveryJob,
    pub(super) result: Result<(), MailError>,
}
impl DeliveryWorker {
    pub(super) fn new(recover: bool) -> Self {
        Self {
            recover,
            completion: None,
        }
    }
    pub(super) async fn tick(&mut self, state: &AppState) -> anyhow::Result<()> {
        if self.completion.is_some() {
            return self.complete(state).await;
        }
        let work = {
            let _guard = state.maintenance.automatic_backup_lock.lock().await;
            let mut email = load(state).await?;
            let previous = serde_json::to_value(&email)?;
            let config = load_automatic_backup_config(state).await?;
            let enabled = config["enabled"].as_bool() == Some(true) && email.config.enabled;
            let now = time_utils::now_ms();
            for job in &mut email.jobs {
                if self.recover && job.status == JobStatus::Sending {
                    job.status = JobStatus::Pending;
                }
                if job.active() && (!enabled || job.revision != email.revision) {
                    job.status = JobStatus::Cancelled;
                }
                if job.active() && (now - job.created_ms >= 86_400_000 || job.attempts >= 4) {
                    job.status = JobStatus::Failed;
                    email.last_error = Some("delivery_expired".into());
                }
            }
            let next = email
                .jobs
                .iter()
                .position(|job| job.status == JobStatus::Pending && job.next_ms <= now);
            let work = if let Some(index) = next {
                email.jobs[index].status = JobStatus::Sending;
                email.jobs[index].attempts += 1;
                let job = email.jobs[index].clone();
                email.last_attempt_at = Some(time_utils::now_iso());
                email.last_filename = Some(job.filename.clone());
                Some((job, email.config.clone(), password(state, &email)))
            } else {
                None
            };
            if previous != serde_json::to_value(&email)? {
                persist(state, &email).await?;
            }
            work
        };
        self.recover = false;
        if let Some((job, mut config, password)) = work {
            config.to_addresses = job.recipients.clone();
            let result = match password {
                Ok(password) => deliver(state, &job, &config, &password).await,
                Err(error) => Err(error),
            };
            self.completion = Some(Completion { job, result });
            self.complete(state).await?;
        }
        Ok(())
    }
    async fn complete(&mut self, state: &AppState) -> anyhow::Result<()> {
        let Some(Completion { job, result }) = self.completion.as_ref() else {
            return Ok(());
        };
        let _guard = state.maintenance.automatic_backup_lock.lock().await;
        let mut email = load(state).await?;
        let backup = load_automatic_backup_config(state).await?;
        let can_retry = email.config.enabled
            && backup["enabled"].as_bool() == Some(true)
            && job.revision == email.revision;
        if let Some(current) = email
            .jobs
            .iter_mut()
            .find(|candidate| candidate.id == job.id)
        {
            match result {
                Ok(()) => {
                    current.status = JobStatus::Sent;
                    email.last_success_at = Some(time_utils::now_iso());
                    email.last_error = None;
                }
                Err(error) => {
                    email.last_error = Some(error.code.into());
                    if error.retryable && current.attempts < 4 && can_retry {
                        current.status = JobStatus::Pending;
                        current.next_ms = time_utils::now_ms()
                            + [60_000, 300_000, 1_800_000][usize::from(current.attempts - 1)];
                    } else {
                        current.status = JobStatus::Failed;
                    }
                }
            }
            persist(state, &email).await?;
        }
        self.completion = None;
        Ok(())
    }
}

#[cfg(test)]
pub(super) async fn tick(state: &AppState, recover: bool) -> anyhow::Result<()> {
    DeliveryWorker::new(recover).tick(state).await
}
async fn deliver(
    state: &AppState,
    job: &DeliveryJob,
    config: &BackupEmailConfig,
    password: &str,
) -> Result<(), MailError> {
    let path = resolve_automatic_backup_archive_path(state, &job.filename)
        .await
        .map_err(|_| MailError::permanent("invalid_backup_path"))?;
    let metadata = fs::symlink_metadata(&path)
        .await
        .map_err(|_| MailError::permanent("backup_missing"))?;
    if !metadata.file_type().is_file() {
        return Err(MailError::permanent("invalid_backup_path"));
    }
    let oversized = metadata.len() > config.attachment_limit_mib * 1024 * 1024;
    let translator = Translator::from_state(state).await;
    let mut body = maintenance_backup_text_params(
        &translator,
        "emailBody",
        &[
            ("name", config.from_name.clone()),
            ("time", job.exported_at.clone()),
            ("filename", job.filename.clone()),
            ("size", metadata.len().to_string()),
        ],
    );
    let attachment = if oversized {
        body.push('\n');
        body.push_str(&maintenance_backup_text(&translator, "emailOversized"));
        None
    } else {
        let bytes = read_backup_archive_file(&path)
            .await
            .map_err(|_| MailError::permanent("backup_unreadable"))?;
        if bytes.len() as u64 > config.attachment_limit_mib * 1024 * 1024 {
            return Err(MailError::permanent("attachment_too_large"));
        }
        Some(MailAttachment {
            filename: job.filename.clone(),
            bytes,
        })
    };
    let message = message(
        config,
        maintenance_backup_text_params(
            &translator,
            "emailSubject",
            &[("time", job.exported_at.clone())],
        ),
        body,
        &job.id,
        attachment,
    )?;
    let result = mail::send(&config.smtp, password, message).await;
    if oversized {
        return Err(MailError::permanent("attachment_too_large"));
    }
    result.map(|_| ())
}
