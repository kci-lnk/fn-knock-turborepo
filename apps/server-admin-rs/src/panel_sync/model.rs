use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PanelProvider {
    SunPanel,
    OneNav,
    VanNav,
}

impl PanelProvider {
    pub fn default_api_path(self) -> &'static str {
        match self {
            Self::SunPanel => "/openapi/v1",
            Self::OneNav => "/index.php?c=api",
            Self::VanNav => "/api",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::SunPanel => "Sun-Panel",
            Self::OneNav => "OneNav",
            Self::VanNav => "Van Nav",
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProviderDescriptor {
    pub provider: PanelProvider,
    pub name: String,
    pub default_api_path: String,
    pub supports_delete: bool,
    pub supports_icon: bool,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GroupMode {
    Mirror,
    Single,
}

fn default_namespace() -> String {
    "fn-knock".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct GroupingConfig {
    pub mode: GroupMode,
    #[serde(default = "default_namespace")]
    pub namespace: String,
    #[serde(default)]
    pub single_group_name: String,
}

impl Default for GroupingConfig {
    fn default() -> Self {
        Self {
            mode: GroupMode::Mirror,
            namespace: default_namespace(),
            single_group_name: String::new(),
        }
    }
}

fn default_interval() -> u32 {
    60
}

fn default_auto_sync_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct AutoSyncConfig {
    #[serde(default = "default_auto_sync_enabled")]
    pub enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u32,
}

impl Default for AutoSyncConfig {
    fn default() -> Self {
        Self {
            enabled: default_auto_sync_enabled(),
            interval_minutes: default_interval(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct PanelConnection {
    pub id: String,
    pub name: String,
    pub provider: PanelProvider,
    pub base_url: String,
    pub api_path: String,
    #[serde(default)]
    pub allow_invalid_tls: bool,
    #[serde(default)]
    pub grouping: GroupingConfig,
    #[serde(default)]
    pub auto_sync: AutoSyncConfig,
    pub credential_configured: bool,
    pub verified_at: Option<String>,
    pub verified_version: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_run: Option<RunSummary>,
    pub next_sync_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct ConnectionInput {
    pub name: String,
    pub provider: PanelProvider,
    pub base_url: String,
    pub api_path: Option<String>,
    #[serde(default)]
    pub allow_invalid_tls: bool,
    #[serde(default)]
    pub grouping: GroupingConfig,
    #[serde(default)]
    pub auto_sync: AutoSyncConfig,
    pub credential: Option<String>,
    #[serde(default)]
    pub clear_credential: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct ConnectionUpdateInput {
    pub name: String,
    pub base_url: String,
    pub api_path: Option<String>,
    #[serde(default)]
    pub allow_invalid_tls: bool,
    #[serde(default)]
    pub grouping: GroupingConfig,
    #[serde(default)]
    pub auto_sync: AutoSyncConfig,
    pub credential: Option<String>,
    #[serde(default)]
    pub clear_credential: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct TestConnectionInput {
    pub connection_id: Option<String>,
    pub draft: Option<ConnectionInput>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProbeResult {
    pub success: bool,
    pub provider: PanelProvider,
    pub version: Option<String>,
    pub message: String,
    pub capabilities: AdapterCapabilities,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AdapterCapabilities {
    pub can_create: bool,
    pub can_update: bool,
    pub can_update_groups: bool,
    pub can_delete: bool,
    pub supports_icon: bool,
    pub residual_on_delete: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlanActionKind {
    Create,
    Update,
    Delete,
    Unchanged,
    Residual,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct PlanAction {
    pub kind: PlanActionKind,
    pub object_type: String,
    pub source_id: Option<String>,
    pub remote_id: Option<String>,
    pub title: String,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema, Default)]
pub struct PlanCounts {
    pub create: usize,
    pub update: usize,
    pub delete: usize,
    pub unchanged: usize,
    pub residual: usize,
    pub conflict: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SyncPreview {
    pub connection_id: String,
    pub source_revision: String,
    pub plan_hash: String,
    pub counts: PlanCounts,
    pub actions: Vec<PlanAction>,
    pub warnings: Vec<String>,
    pub can_apply: bool,
    pub expires_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct PreviewRequest {
    #[serde(default)]
    pub cleanup_remote: bool,
    pub refresh_remote: Option<bool>,
}

#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
pub struct DeleteConnectionRequest {
    #[serde(default)]
    pub cleanup_remote: bool,
    pub source_revision: Option<String>,
    pub plan_hash: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SyncRequest {
    pub source_revision: String,
    pub plan_hash: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SyncAccepted {
    pub run_id: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Success,
    Failed,
    Skipped,
    Conflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    Manual,
    ConfigChange,
    Periodic,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct SyncRun {
    pub id: String,
    pub connection_id: String,
    pub trigger: RunTrigger,
    pub status: RunStatus,
    pub source_revision: String,
    pub plan_hash: String,
    pub counts: PlanCounts,
    pub warnings: Vec<String>,
    pub message: Option<String>,
    pub started_at: String,
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct RunSummary {
    pub id: String,
    pub status: RunStatus,
    pub trigger: RunTrigger,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub message: Option<String>,
}

impl From<&SyncRun> for RunSummary {
    fn from(value: &SyncRun) -> Self {
        Self {
            id: value.id.clone(),
            status: value.status,
            trigger: value.trigger,
            started_at: value.started_at.clone(),
            finished_at: value.finished_at.clone(),
            message: value.message.clone(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct ManagedObject {
    pub remote_id: String,
    pub remote_group_id: Option<String>,
    pub fingerprint: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct ManagedState {
    pub groups: BTreeMap<String, ManagedObject>,
    pub links: BTreeMap<String, ManagedObject>,
}

#[derive(Clone, Debug)]
pub struct AdapterContext {
    pub connection: PanelConnection,
    pub credential: String,
}

#[derive(Clone, Debug, Default)]
pub struct RemoteSnapshot {
    pub groups: BTreeMap<String, RemoteObject>,
    pub links: BTreeMap<String, RemoteObject>,
    /// Objects carrying a provider-specific deterministic ownership marker.
    /// These can be re-registered after a process crash between remote apply
    /// and the local ownership checkpoint.
    pub recovered: ManagedState,
    pub warnings: Vec<String>,
    pub conflicts: Vec<PlanAction>,
}

#[derive(Clone, Debug)]
pub struct RemoteObject {
    pub remote_id: String,
    pub fingerprint: String,
    pub exists: bool,
}

#[derive(Clone, Debug)]
pub struct AdapterPlan {
    pub preview: SyncPreview,
    pub projection: PanelLinkProjection,
    pub managed: ManagedState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectedGroup {
    pub source_id: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProjectedLink {
    pub sync_id: String,
    pub group_source_id: String,
    pub title: String,
    pub url: String,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PanelLinkProjection {
    pub revision: String,
    pub groups: Vec<ProjectedGroup>,
    pub links: Vec<ProjectedLink>,
    pub warnings: Vec<String>,
}
