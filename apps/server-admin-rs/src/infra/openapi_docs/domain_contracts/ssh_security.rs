use serde::Serialize;
use utoipa::ToSchema;

use super::{GatewayVisibilitySelectionData, GatewayVisibilitySelectionInputData};

#[derive(Serialize, ToSchema)]
pub(super) struct SshSecurityConfigData {
    enabled: bool,
    window_minutes: i64,
    failed_login_threshold: i64,
    block_duration_value: i64,
    block_duration_unit: String,
    allowed_regions: Vec<GatewayVisibilitySelectionData>,
    custom_cidrs: Vec<String>,
    #[schema(required = true)]
    configured_at: Option<String>,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshSecurityConfigUpdateData {
    enabled: Option<bool>,
    #[schema(minimum = 1, maximum = 1440)]
    window_minutes: Option<i64>,
    #[schema(minimum = 1, maximum = 1000)]
    failed_login_threshold: Option<i64>,
    #[schema(minimum = 1, maximum = 365)]
    block_duration_value: Option<i64>,
    block_duration_unit: Option<String>,
    allowed_regions: Option<Vec<GatewayVisibilitySelectionInputData>>,
    custom_cidrs: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshSecuritySummaryData {
    configured: bool,
    enabled: bool,
    allowed_cidr_count: u64,
    allowed_range_count: u64,
    active_block_count: usize,
    ssh_ports: Vec<i64>,
    log_source: String,
    available: bool,
    unavailable_reason: String,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshSecurityDetailsData {
    config: SshSecurityConfigData,
    summary: SshSecuritySummaryData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SshLoginLogEntryData {
    id: String,
    #[serde(rename = "happened_at")]
    happened_at: String,
    outcome: String,
    username: String,
    #[serde(rename = "invalid_user")]
    invalid_user: bool,
    ip: String,
    ip_location: Option<String>,
    port: Option<i64>,
    #[serde(rename = "related_ports")]
    related_ports: Option<Vec<i64>>,
    #[serde(rename = "repeat_count")]
    repeat_count: Option<i64>,
    #[serde(rename = "auth_method")]
    auth_method: Option<String>,
    service: String,
    source: String,
    raw: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshLoginLogListData {
    items: Vec<SshLoginLogEntryData>,
    total: usize,
    page: i64,
    limit: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SshSecurityBlockData {
    ip: String,
    ip_location: Option<String>,
    ports: Option<Vec<i64>>,
    #[serde(rename = "blocked_at")]
    blocked_at: String,
    #[serde(rename = "expires_at")]
    expires_at: String,
    reason: String,
    #[serde(rename = "failed_count")]
    failed_count: i64,
    #[serde(rename = "window_minutes")]
    window_minutes: i64,
    threshold: i64,
    #[serde(rename = "sample_user")]
    sample_user: Option<String>,
    #[serde(rename = "sample_auth_method")]
    sample_auth_method: Option<String>,
    #[serde(rename = "sample_log_time")]
    sample_log_time: Option<String>,
    applied: bool,
    #[schema(required = true)]
    #[serde(rename = "removed_at")]
    removed_at: Option<String>,
    #[schema(required = true)]
    #[serde(rename = "remove_reason")]
    remove_reason: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshSecurityBlockListData {
    items: Vec<SshSecurityBlockData>,
    total: usize,
    page: i64,
    limit: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshFirewallSyncData {
    cleared: usize,
    synced: usize,
    active_blocks: usize,
    allowed_cidrs: usize,
    ports: Vec<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshFirewallClearData {
    cleared_blocks: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshBlocksDeleteBodyData {
    ips: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SshBlocksDeleteData {
    removed: usize,
}
