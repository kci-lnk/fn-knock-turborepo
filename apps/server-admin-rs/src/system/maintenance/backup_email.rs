//! Backup delivery policy; transport and archive production live elsewhere.
use super::*;
use crate::infra::{
    credentials::CredentialStore,
    mail::{self, MailAttachment, MailError, MailMessage, SmtpConfig},
};
use lettre::message::Mailbox;
use serde::{Deserialize, Serialize};

mod settings;
mod worker;
pub(super) use settings::*;
pub(super) use worker::*;
pub(super) const EMAIL_KEY: &str = "fn_knock:config:backup:automatic:email";

#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema, PartialEq, Eq)]
#[serde(default)]
pub(crate) struct BackupEmailConfig {
    pub enabled: bool,
    pub smtp: SmtpConfig,
    pub from_address: String,
    pub from_name: String,
    pub to_addresses: Vec<String>,
    pub attachment_limit_mib: u64,
}
impl Default for BackupEmailConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            smtp: SmtpConfig::default(),
            from_address: String::new(),
            from_name: "fn-knock".into(),
            to_addresses: vec![],
            attachment_limit_mib: 20,
        }
    }
}
#[derive(Deserialize, utoipa::ToSchema)]
pub(crate) struct BackupEmailUpdate {
    #[serde(flatten)]
    pub config: BackupEmailConfig,
    #[schema(write_only)]
    pub password: Option<String>,
    #[serde(default)]
    pub clear_password: bool,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub(super) struct EmailState {
    config: BackupEmailConfig,
    revision: String,
    pub(super) secret_id: Option<String>,
    jobs: Vec<DeliveryJob>,
    last_attempt_at: Option<String>,
    last_success_at: Option<String>,
    last_filename: Option<String>,
    last_error: Option<String>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum JobStatus {
    Pending,
    Sending,
    Sent,
    Failed,
    Cancelled,
}
#[derive(Clone, Serialize, Deserialize)]
struct DeliveryJob {
    id: String,
    revision: String,
    filename: String,
    exported_at: String,
    recipients: Vec<String>,
    created_ms: i64,
    next_ms: i64,
    attempts: u8,
    status: JobStatus,
}
impl DeliveryJob {
    fn active(&self) -> bool {
        matches!(self.status, JobStatus::Pending | JobStatus::Sending)
    }
}
fn secrets(state: &AppState) -> CredentialStore {
    CredentialStore::with_directory(state.settings.data_dir.join("backup-email"), "backup-email")
}
pub(super) async fn load(state: &AppState) -> anyhow::Result<EmailState> {
    Ok(state
        .storage
        .store
        .get_json_value(EMAIL_KEY)
        .await?
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}
async fn persist(state: &AppState, email: &EmailState) -> anyhow::Result<()> {
    state
        .storage
        .store
        .set_json_value(EMAIL_KEY, &serde_json::to_value(email)?)
        .await?;
    Ok(())
}
pub(super) fn enqueue(email: &mut EmailState, filename: &str, exported_at: &str) {
    if !email.config.enabled {
        return;
    }
    email.jobs.retain(DeliveryJob::active);
    let now = time_utils::now_ms();
    email.jobs.push(DeliveryJob {
        id: Uuid::new_v4().to_string(),
        revision: email.revision.clone(),
        filename: filename.into(),
        exported_at: exported_at.into(),
        recipients: email.config.to_addresses.clone(),
        created_ms: now,
        next_ms: now,
        attempts: 0,
        status: JobStatus::Pending,
    });
}
pub(super) async fn decorate(state: &AppState, mut details: Value) -> anyhow::Result<Value> {
    let email = load(state).await?;
    let mut config = serde_json::to_value(&email.config)?;
    config["password_configured"] = json!(email.secret_id.is_some());
    details["config"]["email"] = config;
    details["status"]["email"] = json!({
        "last_attempt_at": email.last_attempt_at, "last_success_at": email.last_success_at,
        "last_filename": email.last_filename, "last_error": email.last_error,
        "pending_count": email.jobs.iter().filter(|job| job.active()).count(),
        "next_retry_at": email.jobs.iter().filter(|job| job.active()).map(|job| job.next_ms).min().map(time_utils::iso_from_ms),
    });
    Ok(details)
}
pub(super) async fn pinned_files(state: &AppState) -> anyhow::Result<Vec<String>> {
    let now = time_utils::now_ms();
    Ok(load(state)
        .await?
        .jobs
        .into_iter()
        .filter(|job| job.active() && now - job.created_ms < 86_400_000)
        .map(|job| job.filename)
        .collect())
}

#[cfg(test)]
mod tests;
