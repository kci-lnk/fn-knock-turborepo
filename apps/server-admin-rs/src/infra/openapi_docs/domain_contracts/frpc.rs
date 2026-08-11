use super::CloudflaredSupervisorData;
use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcTcpItemData {
    name: String,
    #[serde(rename = "type")]
    item_type: String,
    status: String,
    err: String,
    local_addr: String,
    plugin: String,
    remote_addr: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcInstanceSummaryData {
    server_addr: String,
    server_port: String,
    local_port: String,
    remote_port: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcInstanceStatusData {
    id: String,
    name: String,
    is_primary: bool,
    config_path: String,
    work_dir: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
    desired_running: bool,
    running: bool,
    attached: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    stopped_at: Option<String>,
    #[schema(required = true)]
    last_exit_code: Option<i32>,
    #[schema(required = true)]
    last_message: Option<String>,
    supervisor: CloudflaredSupervisorData,
    summary: FrpcInstanceSummaryData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcDefaultsData {
    local_port: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcInstancesOverviewData {
    initialized: bool,
    platform: String,
    primary_instance_id: String,
    total: usize,
    extra_count: usize,
    running_count: usize,
    defaults: FrpcDefaultsData,
    items: Vec<FrpcInstanceStatusData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcStatusData {
    initialized: bool,
    platform: String,
    running: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    desired_running: bool,
    supervisor: CloudflaredSupervisorData,
    #[serde(rename = "config_path")]
    config_path: String,
    defaults: FrpcDefaultsData,
    total: usize,
    #[serde(rename = "running_count")]
    running_count: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcLegacyOverviewData {
    tcp: Vec<FrpcTcpItemData>,
    logs: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcWebStatusData {
    tcp: Vec<FrpcTcpItemData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcConfigData {
    content: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcConfigUpdateData {
    content: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcStartData {
    pid: u32,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcInstanceBodyData {
    name: Option<String>,
    content: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct FrpcInstanceDetailData {
    item: FrpcInstanceStatusData,
    content: String,
    logs: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcPrimaryStatusData {
    id: String,
    name: String,
    is_primary: bool,
    config_path: String,
    work_dir: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
    desired_running: bool,
    running: bool,
    attached: bool,
    #[schema(required = true)]
    pid: Option<u32>,
    #[schema(required = true)]
    started_at: Option<String>,
    #[schema(required = true)]
    stopped_at: Option<String>,
    #[schema(required = true)]
    last_exit_code: Option<i32>,
    #[schema(required = true)]
    last_message: Option<String>,
    supervisor: CloudflaredSupervisorData,
    summary: FrpcInstanceSummaryData,
    tcp: Vec<FrpcTcpItemData>,
    instances: FrpcInstancesOverviewData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcPollData {
    cursor: i64,
    reset: bool,
    logs: Vec<String>,
    status: FrpcPrimaryStatusData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct FrpcInstancePollData {
    cursor: i64,
    reset: bool,
    logs: Vec<String>,
    status: FrpcInstanceStatusData,
}
