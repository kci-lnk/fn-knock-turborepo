use serde_json::{Value, json};
use time::{Duration, OffsetDateTime, format_description::well_known::Rfc3339};

use crate::state::AppState;

use super::model::{ManagedState, PanelConnection, RunSummary, SyncRun};

const CONNECTIONS_KEY: &str = "fn_knock:panel_sync:connections";
const MANAGED_PREFIX: &str = "fn_knock:panel_sync:managed:";
pub const RUNTIME_PREFIX: &str = "fn_knock:panel_sync:runtime:";
const RUN_TTL_SECONDS: usize = 30 * 24 * 60 * 60;

pub struct Repository<'a> {
    state: &'a AppState,
}

impl<'a> Repository<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub async fn connections(&self) -> Result<Vec<PanelConnection>, String> {
        let value = self
            .state
            .storage
            .store
            .get_json_value(CONNECTIONS_KEY)
            .await
            .map_err(|error| error.to_string())?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map(|value| value.unwrap_or_default())
            .map_err(|error| format!("面板连接配置已损坏: {error}"))
    }

    pub async fn connection(&self, id: &str) -> Result<Option<PanelConnection>, String> {
        Ok(self
            .connections()
            .await?
            .into_iter()
            .find(|item| item.id == id))
    }

    pub async fn save_connections(&self, connections: &[PanelConnection]) -> Result<(), String> {
        self.state
            .storage
            .store
            .set_json_value(
                CONNECTIONS_KEY,
                &serde_json::to_value(connections).unwrap_or(Value::Array(Vec::new())),
            )
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn managed(&self, id: &str) -> Result<ManagedState, String> {
        self.state
            .storage
            .store
            .get_json_value(&format!("{MANAGED_PREFIX}{id}"))
            .await
            .map_err(|error| error.to_string())?
            .map(serde_json::from_value)
            .transpose()
            .map(|value| value.unwrap_or_default())
            .map_err(|error| format!("面板所有权状态已损坏: {error}"))
    }

    pub async fn save_managed(&self, id: &str, managed: &ManagedState) -> Result<(), String> {
        self.state
            .storage
            .store
            .set_json_value(
                &format!("{MANAGED_PREFIX}{id}"),
                &serde_json::to_value(managed).unwrap_or(json!({})),
            )
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn clear_managed(&self, id: &str) -> Result<(), String> {
        self.state
            .storage
            .store
            .delete_key(&format!("{MANAGED_PREFIX}{id}"))
            .await
            .map_err(|error| error.to_string())
    }

    pub async fn save_run(&self, run: &SyncRun) -> Result<(), String> {
        self.state
            .storage
            .store
            .set_json_value_ex(
                &run_key(&run.id),
                &serde_json::to_value(run).unwrap_or(json!({})),
                RUN_TTL_SECONDS,
            )
            .await
            .map_err(|error| error.to_string())?;
        let index_key = run_index_key(&run.connection_id);
        let mut ids = self
            .state
            .storage
            .store
            .get_json_value(&index_key)
            .await
            .map_err(|error| error.to_string())?
            .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
            .unwrap_or_default();
        ids.retain(|id| id != &run.id);
        ids.insert(0, run.id.clone());
        let evicted = if ids.len() > 20 {
            ids.split_off(20)
        } else {
            Vec::new()
        };
        self.state
            .storage
            .store
            .set_json_value_ex(&index_key, &json!(ids), RUN_TTL_SECONDS)
            .await
            .map_err(|error| error.to_string())?;
        for id in evicted {
            self.state
                .storage
                .store
                .delete_key(&run_key(&id))
                .await
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }

    pub async fn run(&self, id: &str) -> Result<Option<SyncRun>, String> {
        self.state
            .storage
            .store
            .get_json_value(&run_key(id))
            .await
            .map_err(|error| error.to_string())?
            .map(serde_json::from_value)
            .transpose()
            .map_err(|error| error.to_string())
    }

    pub async fn runs(&self, connection_id: &str) -> Result<Vec<SyncRun>, String> {
        let ids = self
            .state
            .storage
            .store
            .get_json_value(&run_index_key(connection_id))
            .await
            .map_err(|error| error.to_string())?
            .and_then(|value| serde_json::from_value::<Vec<String>>(value).ok())
            .unwrap_or_default();
        let mut runs = Vec::new();
        for id in ids.into_iter().take(20) {
            if let Some(run) = self.run(&id).await? {
                runs.push(run);
            }
        }
        Ok(runs)
    }

    pub async fn decorate(
        &self,
        mut connection: PanelConnection,
    ) -> Result<PanelConnection, String> {
        let runs = self.runs(&connection.id).await?;
        connection.last_run = runs.first().map(RunSummary::from);
        connection.next_sync_at =
            if connection.auto_sync.enabled && connection.verified_at.is_some() {
                let base = runs
                    .first()
                    .and_then(|run| OffsetDateTime::parse(&run.started_at, &Rfc3339).ok())
                    .unwrap_or_else(OffsetDateTime::now_utc);
                (base + Duration::minutes(connection.auto_sync.interval_minutes.into()))
                    .format(&Rfc3339)
                    .ok()
            } else {
                None
            };
        Ok(connection)
    }
}

fn run_key(id: &str) -> String {
    format!("{RUNTIME_PREFIX}run:{id}")
}
fn run_index_key(id: &str) -> String {
    format!("{RUNTIME_PREFIX}connection:{id}:runs")
}
