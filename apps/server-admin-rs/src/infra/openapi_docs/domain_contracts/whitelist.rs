use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WhitelistRecordData {
    id: String,
    ip: String,
    target_type: String,
    #[schema(required = true)]
    expire_at: Option<i64>,
    source: String,
    created_at: i64,
    status: String,
    comment: Option<String>,
    ip_location: Option<String>,
    resolved_targets: Option<Vec<String>>,
    check_interval_minutes: Option<i64>,
    last_checked_at: Option<i64>,
    last_resolved_at: Option<i64>,
    resolve_status: Option<String>,
    resolve_message: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistRegionInputData {
    province: String,
    query_city: Option<String>,
    operator: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WhitelistRegionGroupData {
    id: String,
    regions: Vec<WhitelistRegionInputData>,
    cidr_count: usize,
    #[schema(required = true)]
    expire_at: Option<i64>,
    source: String,
    created_at: i64,
    updated_at: i64,
    status: String,
    comment: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WhitelistAddBodyData {
    ip: String,
    target_type: Option<String>,
    expire_at: Option<i64>,
    source: Option<String>,
    comment: Option<String>,
    check_interval_minutes: Option<i64>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct WhitelistRegionAddBodyData {
    regions: Vec<WhitelistRegionInputData>,
    expire_at: Option<i64>,
    comment: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistCommentBodyData {
    comment: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistAddResultData {
    id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistRegionAddResultData {
    group: WhitelistRegionGroupData,
    total: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistRefreshData {
    changed: bool,
    skipped: bool,
    record: WhitelistRecordData,
}

/// CNAME resolution failures deliberately use HTTP 200 so the refreshed
/// record and its localized resolver error can still replace stale UI state.
#[derive(Serialize, ToSchema)]
pub(super) struct WhitelistRefreshEnvelopeData {
    success: bool,
    message: Option<String>,
    data: WhitelistRefreshData,
}
