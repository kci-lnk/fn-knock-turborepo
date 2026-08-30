use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::state::AppState;

use super::{
    domain::{TargetRecord, TerminalError, TerminalResult},
    runtime::MAX_TARGETS,
};

const TARGETS_KEY: &str = "fn_knock:terminal:targets";
pub(super) const LOCAL_SETTINGS_KEY: &str = "fn_knock:terminal:local-settings";

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct LocalSettingsRecord {
    pub enabled: bool,
    pub revision: u64,
}

pub(super) struct LocalSettingsRepository<'a> {
    state: &'a AppState,
}

impl<'a> LocalSettingsRepository<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn get(&self) -> TerminalResult<LocalSettingsRecord> {
        let value = self
            .state
            .storage
            .store
            .get_json_value(LOCAL_SETTINGS_KEY)
            .await
            .map_err(|error| TerminalError::internal(error.to_string()))?;
        let Some(value) = value else {
            return Ok(LocalSettingsRecord::default());
        };
        let record = serde_json::from_value::<LocalSettingsRecord>(value).map_err(|error| {
            tracing::error!(%error, "local terminal settings are corrupted");
            TerminalError::internal("local terminal settings are corrupted")
        })?;
        if record.revision == 0 {
            tracing::error!("local terminal settings revision is invalid");
            return Err(TerminalError::internal(
                "local terminal settings are corrupted",
            ));
        }
        Ok(record)
    }

    pub async fn save(&self, record: LocalSettingsRecord) -> TerminalResult<()> {
        if record.revision == 0 {
            return Err(TerminalError::internal(
                "local terminal settings revision is invalid",
            ));
        }
        let value = serde_json::to_value(record)
            .map_err(|error| TerminalError::internal(error.to_string()))?;
        self.state
            .storage
            .store
            .set_json_value(LOCAL_SETTINGS_KEY, &value)
            .await
            .map_err(|error| TerminalError::internal(error.to_string()))
    }
}

pub struct TargetRepository<'a> {
    state: &'a AppState,
}

impl<'a> TargetRepository<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn list(&self) -> TerminalResult<Vec<TargetRecord>> {
        let value = self
            .state
            .storage
            .store
            .get_json_value(TARGETS_KEY)
            .await
            .map_err(|error| TerminalError::internal(error.to_string()))?;
        let mut targets = value
            .map(serde_json::from_value::<Vec<TargetRecord>>)
            .transpose()
            .map_err(|error| {
                tracing::error!(%error, "terminal target metadata is corrupted");
                TerminalError::internal("terminal target metadata is corrupted")
            })?
            .unwrap_or_default();
        validate_records(&targets)?;
        targets.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(targets)
    }

    pub async fn get(&self, id: &str) -> TerminalResult<Option<TargetRecord>> {
        Ok(self
            .list()
            .await?
            .into_iter()
            .find(|target| target.id == id))
    }

    pub async fn insert(&self, target: TargetRecord) -> TerminalResult<()> {
        let mut targets = self.list().await?;
        if targets.iter().any(|item| item.id == target.id) {
            return Err(TerminalError::internal(
                "terminal target identifier collision",
            ));
        }
        targets.push(target);
        self.save_all(&targets).await
    }

    pub async fn replace(&self, target: TargetRecord) -> TerminalResult<()> {
        let mut targets = self.list().await?;
        let Some(existing) = targets.iter_mut().find(|item| item.id == target.id) else {
            return Err(super::domain::TerminalError::new(
                super::domain::TerminalErrorCode::TargetNotFound,
                "terminal target not found",
            ));
        };
        *existing = target;
        self.save_all(&targets).await
    }

    pub async fn delete(&self, id: &str) -> TerminalResult<bool> {
        let mut targets = self.list().await?;
        let previous = targets.len();
        targets.retain(|target| target.id != id);
        if targets.len() == previous {
            return Ok(false);
        }
        self.save_all(&targets).await?;
        Ok(true)
    }

    async fn save_all(&self, targets: &[TargetRecord]) -> TerminalResult<()> {
        validate_records(targets)?;
        self.state
            .storage
            .store
            .set_json_value(
                TARGETS_KEY,
                &serde_json::to_value(targets).unwrap_or(Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| TerminalError::internal(error.to_string()))
    }
}

fn validate_records(targets: &[TargetRecord]) -> TerminalResult<()> {
    let mut ids = HashSet::with_capacity(targets.len());
    let valid = targets.len() <= MAX_TARGETS
        && targets.iter().all(|target| {
            Uuid::parse_str(&target.id).is_ok()
                && ids.insert(target.id.as_str())
                && target.revision > 0
                && valid_text(&target.name, 80)
                && valid_text(&target.host, 253)
                && valid_text(&target.username, 128)
                && target.port > 0
                && target.trusted_host_key.as_ref().is_none_or(|key| {
                    valid_text(&key.algorithm, 64)
                        && key.fingerprint.starts_with("SHA256:")
                        && key.fingerprint.len() <= 128
                        && !key.fingerprint.chars().any(char::is_control)
                })
        });
    if !valid {
        tracing::error!(
            count = targets.len(),
            "terminal target metadata failed validation"
        );
        return Err(TerminalError::internal(
            "terminal target metadata is corrupted",
        ));
    }
    Ok(())
}

fn valid_text(value: &str, max_chars: usize) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= max_chars
        && !trimmed.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::system::terminal::domain::AuthMethod;

    fn record(id: String) -> TargetRecord {
        TargetRecord {
            id,
            name: "target".to_string(),
            host: "localhost".to_string(),
            port: 22,
            username: "user".to_string(),
            auth_method: AuthMethod::Password,
            trusted_host_key: None,
            revision: 1,
            last_verified_at: None,
            created_at: "now".to_string(),
            updated_at: "now".to_string(),
        }
    }

    #[test]
    fn rejects_duplicate_invalid_or_oversized_metadata() {
        let id = Uuid::new_v4().to_string();
        assert!(validate_records(&[record(id.clone()), record(id)]).is_err());
        assert!(validate_records(&[record("not-a-uuid".to_string())]).is_err());
        let targets = (0..=MAX_TARGETS)
            .map(|_| record(Uuid::new_v4().to_string()))
            .collect::<Vec<_>>();
        assert!(validate_records(&targets).is_err());
    }
}
