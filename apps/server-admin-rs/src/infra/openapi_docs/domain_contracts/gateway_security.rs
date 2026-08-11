use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayReverseProxyThrottleData {
    enabled: bool,
    requests_per_second: i64,
    burst: i64,
    block_seconds: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayReverseProxyThrottleUpdateData {
    enabled: Option<bool>,
    #[schema(minimum = 1)]
    requests_per_second: Option<i64>,
    #[schema(minimum = 1)]
    burst: Option<i64>,
    #[schema(minimum = 1)]
    block_seconds: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayCrawlerBlockerData {
    enabled: bool,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayCrawlerBlockerUpdateData {
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayPortalData {
    enabled: bool,
    display_style: String,
    show_app_icon: bool,
    show_wol: bool,
    icon_drag_mode: String,
    version: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayPortalUpdateData {
    enabled: Option<bool>,
    display_style: Option<String>,
    show_app_icon: Option<bool>,
    show_wol: Option<bool>,
    icon_drag_mode: Option<String>,
    version: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayUnmatchedRouteData {
    behavior: String,
    upstream_error_detail: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayUnmatchedRouteUpdateData {
    behavior: Option<String>,
    upstream_error_detail: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewaySettingsData {
    auth_cache_ttl_seconds: i64,
    auth_cache_unauthorized_ttl_seconds: i64,
    reverse_proxy_throttle: GatewayReverseProxyThrottleData,
    visibility: GatewayVisibilitySummaryData,
    proxy_headers: GatewayProxyHeadersSummaryData,
    host_response: GatewayHostResponseSummaryData,
    crawler_blocker: GatewayCrawlerBlockerData,
    portal: GatewayPortalData,
    unmatched_route: GatewayUnmatchedRouteData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewaySettingsUpdateData {
    #[schema(minimum = 0)]
    auth_cache_ttl_seconds: Option<i64>,
    #[schema(minimum = 0)]
    auth_cache_unauthorized_ttl_seconds: Option<i64>,
    reverse_proxy_throttle: Option<GatewayReverseProxyThrottleUpdateData>,
    crawler_blocker: Option<GatewayCrawlerBlockerUpdateData>,
    portal: Option<GatewayPortalUpdateData>,
    unmatched_route: Option<GatewayUnmatchedRouteUpdateData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilitySelectionData {
    province: String,
    #[schema(required = true)]
    city: Option<String>,
    label: String,
    value: String,
    #[schema(required = true)]
    query_city: Option<String>,
    #[schema(required = true)]
    operator: Option<String>,
    is_province_wide: bool,
    is_municipality: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilitySelectionInputData {
    province: Option<String>,
    query_city: Option<String>,
    operator: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilityConfigData {
    enabled: bool,
    selections: Vec<GatewayVisibilitySelectionData>,
    custom_cidrs: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilityUpdateData {
    enabled: Option<bool>,
    selections: Option<Vec<GatewayVisibilitySelectionInputData>>,
    custom_cidrs: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilitySummaryData {
    enabled: bool,
    selection_count: usize,
    custom_cidr_count: usize,
    cidr_count: u64,
    range_count: u64,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayVisibilityDetailsData {
    config: GatewayVisibilityConfigData,
    summary: GatewayVisibilitySummaryData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersConfigData {
    disabled_hosts: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersUpdateData {
    disabled_hosts: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersItemData {
    host: String,
    target: String,
    title: String,
    send_proxy_headers: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersAvailabilityData {
    available: bool,
    reason: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersSummaryData {
    total_count: usize,
    disabled_count: usize,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayProxyHeadersDetailsData {
    config: GatewayProxyHeadersConfigData,
    availability: GatewayProxyHeadersAvailabilityData,
    items: Vec<GatewayProxyHeadersItemData>,
    summary: GatewayProxyHeadersSummaryData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseConfigData {
    disabled_hosts: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseUpdateData {
    disabled_hosts: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseItemData {
    host: String,
    target: String,
    title: String,
    preserve_host: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseAvailabilityData {
    available: bool,
    reason: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseSummaryData {
    total_count: usize,
    disabled_count: usize,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GatewayHostResponseDetailsData {
    config: GatewayHostResponseConfigData,
    availability: GatewayHostResponseAvailabilityData,
    items: Vec<GatewayHostResponseItemData>,
    summary: GatewayHostResponseSummaryData,
}
