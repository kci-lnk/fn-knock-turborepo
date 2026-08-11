use std::collections::HashMap;

use serde::Serialize;
use utoipa::ToSchema;

use super::GatewayVisibilitySelectionData;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SecurityOverviewTotalsData {
    failed_logins: usize,
    blocked_scanners: usize,
    waf_events: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SecurityOverviewSeriesData {
    failed_logins: Vec<[i64; 2]>,
    blocked_scanners: Vec<[i64; 2]>,
    waf_events: Vec<[i64; 2]>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SecurityOverviewData {
    range_sec: i64,
    totals: SecurityOverviewTotalsData,
    series: SecurityOverviewSeriesData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScannerSettingsData {
    enabled: bool,
    window_minutes: i64,
    threshold: i64,
    window_seconds: i64,
    blacklist_ttl_seconds: i64,
    common_location_exempt_enabled: bool,
    cidr_exemptions: Vec<String>,
    cidr_exemption_regions: Vec<GatewayVisibilitySelectionData>,
    cidr_exemption_region_cidrs: Vec<String>,
    cidr_exemption_cidrs: Vec<String>,
    #[schema(nullable = false)]
    cidr_exemption_policy_id: Option<String>,
    cidr_exemption_source_cidr_count: usize,
    cidr_exemption_range_count: usize,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScannerCidrExemptionRegionInputData {
    province: String,
    query_city: Option<String>,
    operator: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScannerSettingsUpdateData {
    enabled: bool,
    #[schema(minimum = 1)]
    window_minutes: f64,
    #[schema(minimum = 1)]
    threshold: f64,
    #[schema(minimum = 60)]
    blacklist_ttl_seconds: f64,
    common_location_exempt_enabled: Option<bool>,
    cidr_exemptions: Option<Vec<String>>,
    cidr_exemption_regions: Option<Vec<ScannerCidrExemptionRegionInputData>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScannerBlacklistHitData {
    path: String,
    created_at: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScannerBlacklistRecordData {
    ip: String,
    #[schema(nullable = false)]
    ip_location: Option<String>,
    blocked_at: i64,
    window_minutes: i64,
    threshold: i64,
    hits: Vec<ScannerBlacklistHitData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ScannerBlacklistListData {
    items: Vec<ScannerBlacklistRecordData>,
    total: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpListBodyData {
    ips: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GeneralBlacklistAddBodyData {
    ips: Vec<String>,
    source: Option<String>,
    comment: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GeneralBlacklistRecordData {
    ip: String,
    source: String,
    comment: String,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GeneralBlacklistListData {
    items: Vec<GeneralBlacklistRecordData>,
    total: i32,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GeneralBlacklistMutationData {
    added: i32,
    updated: i32,
    removed: i32,
    total: i32,
    items: Vec<GeneralBlacklistRecordData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct GeneralBlacklistStatusData {
    records: HashMap<String, GeneralBlacklistRecordData>,
}
