use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrOperatorCapabilityData {
    supported: bool,
    operators: Vec<String>,
    minimum_container_version: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrCapabilitiesData {
    source: String,
    operator_filtering: CidrOperatorCapabilityData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrProvinceItemData {
    name: String,
    city_count: i64,
    is_municipality: bool,
    has_children: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrProvinceOptionData {
    label: String,
    value: String,
    city_count: i64,
    is_municipality: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CidrProvincesData {
    items: Vec<CidrProvinceItemData>,
    options: Vec<CidrProvinceOptionData>,
    total: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrCityItemData {
    name: String,
    ipv4_count: i64,
    ipv6_count: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrCityOptionData {
    label: String,
    value: String,
    #[schema(required = true)]
    query_city: Option<String>,
    is_province_wide: bool,
    is_municipality: bool,
    ipv4_count: i64,
    ipv6_count: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrCitiesData {
    province: String,
    items: Vec<CidrCityItemData>,
    options: Vec<CidrCityOptionData>,
    total: i64,
    is_municipality: bool,
    supports_province_wide: bool,
    default_value: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CidrSelectorData {
    provinces: CidrProvincesData,
    #[schema(required = true)]
    cities: Option<CidrCitiesData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrSelectionData {
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
pub(super) struct CidrGroupsData {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CidrCountsData {
    ipv4: i64,
    ipv6: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct CidrLookupData {
    province: String,
    #[schema(required = true)]
    city: Option<String>,
    selection: CidrSelectionData,
    cidr_groups: CidrGroupsData,
    counts: CidrCountsData,
    total_count: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpLocationBatchBodyData {
    #[schema(max_items = 20)]
    ips: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct IpLocationResultData {
    ip: String,
    normalized_ip: String,
    version: String,
    continent: String,
    country: String,
    province: String,
    city: String,
    district: String,
    isp: String,
    country_code: String,
    raw: String,
    source_raw: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct IpLocationSnapshotData {
    ip: String,
    normalized_ip: String,
    status: String,
    attempts: i64,
    max_attempts: i64,
    location: String,
    result: Option<IpLocationResultData>,
    error: Option<String>,
    updated_at: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpLocationBatchData {
    items: Vec<IpLocationSnapshotData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpLocationApiConfigData {
    ip_lookup_mode: String,
    ip_lookup_url: String,
    cidr_mode: String,
    cidr_url: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpLocationTestUrlBodyData {
    url: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct IpLocationConnectionTestData {
    success: bool,
    message: Option<String>,
    msg: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct CidrConnectionTestData {
    success: bool,
    message: Option<String>,
    capabilities: Option<CidrCapabilitiesData>,
}
