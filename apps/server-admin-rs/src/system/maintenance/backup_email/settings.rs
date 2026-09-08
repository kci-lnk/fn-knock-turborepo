use super::*;

pub(in crate::system::maintenance) fn validate(
    config: &BackupEmailConfig,
) -> Result<(), BackupImportError> {
    if !(1..=100).contains(&config.attachment_limit_mib)
        || !(1..=120).contains(&config.smtp.timeout_seconds)
        || config.smtp.port == 0
        || !["ssl_tls", "starttls", "none"].contains(&config.smtp.security.as_str())
        || !["auto", "plain", "login", "none"].contains(&config.smtp.auth_mode.as_str())
    {
        return Err(BackupImportError::bad_request(
            "Invalid backup email configuration",
        ));
    }
    if config.enabled {
        if config.smtp.host.trim().is_empty()
            || config.smtp.host.contains(['\r', '\n'])
            || config.to_addresses.is_empty()
            || config.to_addresses.len() > 100
        {
            return Err(BackupImportError::bad_request(
                "SMTP host and recipients are required",
            ));
        }
        addresses(config).map_err(|_| BackupImportError::bad_request("Invalid email address"))?;
        if config.smtp.auth_mode != "none" && config.smtp.username.trim().is_empty() {
            return Err(BackupImportError::bad_request("SMTP username is required"));
        }
    }
    Ok(())
}
fn addresses(config: &BackupEmailConfig) -> Result<(Mailbox, Vec<Mailbox>), MailError> {
    let from = config
        .from_address
        .parse()
        .map_err(|_| MailError::permanent("invalid_sender"))?;
    let to = config
        .to_addresses
        .iter()
        .map(|address| {
            address
                .parse::<Mailbox>()
                .map_err(|_| MailError::permanent("invalid_recipient"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((Mailbox::new(Some(config.from_name.clone()), from), to))
}
pub(in crate::system::maintenance) fn prepare_update(
    state: &AppState,
    email: &mut EmailState,
    update: BackupEmailUpdate,
    backup_enabled: bool,
) -> Result<Option<String>, BackupImportError> {
    validate(&update.config)?;
    if update.clear_password
        && update
            .password
            .as_ref()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(BackupImportError::bad_request(
            "Conflicting password update",
        ));
    }
    let changed = email.config != update.config
        || update
            .password
            .as_ref()
            .is_some_and(|value| !value.is_empty())
        || (update.clear_password && email.secret_id.is_some());
    let mut staged = None;
    if let Some(password) = update.password.filter(|value| !value.is_empty()) {
        let id = Uuid::new_v4().to_string();
        secrets(state)
            .write(&id, &password)
            .map_err(BackupImportError::internal)?;
        email.secret_id = Some(id.clone());
        staged = Some(id);
    } else if update.clear_password {
        email.secret_id = None;
    }
    if changed {
        email.revision = Uuid::new_v4().to_string();
    }
    email.config = update.config;
    cancel_pending(email, backup_enabled, changed);
    Ok(staged)
}
pub(in crate::system::maintenance) fn cancel_pending(
    email: &mut EmailState,
    backup_enabled: bool,
    changed: bool,
) {
    for job in &mut email.jobs {
        if job.status == JobStatus::Pending && (changed || !backup_enabled || !email.config.enabled)
        {
            job.status = JobStatus::Cancelled;
        }
    }
}
pub(in crate::system::maintenance) fn discard_secret(state: &AppState, id: Option<&str>) {
    if let Some(id) = id {
        let _ = secrets(state).delete(id);
    }
}
pub(super) fn password(state: &AppState, email: &EmailState) -> Result<String, MailError> {
    match email.secret_id.as_deref() {
        None => Ok(String::new()),
        Some(id) => secrets(state)
            .read(id)
            .map_err(|_| MailError::permanent("credential_unavailable"))?
            .ok_or_else(|| MailError::permanent("credential_unavailable")),
    }
}
pub(super) fn message(
    config: &BackupEmailConfig,
    subject: String,
    body: String,
    id: &str,
    attachment: Option<MailAttachment>,
) -> Result<lettre::Message, MailError> {
    let (from, to) = addresses(config)?;
    MailMessage {
        from,
        to,
        subject,
        body,
        message_id: format!("{id}@backup.fn-knock.local"),
        attachment,
    }
    .build()
}
pub(in crate::system::maintenance) async fn test_email(
    state: &AppState,
    mut update: BackupEmailUpdate,
) -> Result<Value, BackupImportError> {
    if update.clear_password
        && update
            .password
            .as_ref()
            .is_some_and(|value| !value.is_empty())
    {
        return Err(BackupImportError::bad_request(
            "Conflicting password update",
        ));
    }
    update.config.enabled = true;
    validate(&update.config)?;
    let guard = state.maintenance.automatic_backup_lock.lock().await;
    let email = load(state)
        .await
        .map_err(|_| BackupImportError::internal("Cannot load email configuration"))?;
    let password = if update.clear_password {
        String::new()
    } else if let Some(value) = update.password.filter(|value| !value.is_empty()) {
        value
    } else {
        password(state, &email).map_err(|error| BackupImportError::internal(error.code))?
    };
    drop(guard);
    let translator = Translator::from_state(state).await;
    let message = message(
        &update.config,
        maintenance_backup_text(&translator, "emailTestSubject"),
        maintenance_backup_text(&translator, "emailTestBody"),
        &Uuid::new_v4().to_string(),
        Some(MailAttachment {
            filename: "backup-email-test.txt".into(),
            bytes: b"fn-knock backup email test\n".to_vec(),
        }),
    )
    .map_err(|error| BackupImportError::bad_request(error.code))?;
    mail::send(&update.config.smtp, &password, message)
        .await
        .map_err(|error| BackupImportError::bad_request(error.code))?;
    Ok(json!({"success":true}))
}

pub(in crate::system::maintenance) fn clear_credentials(state: &AppState) -> Result<(), String> {
    secrets(state).clear_all()
}
