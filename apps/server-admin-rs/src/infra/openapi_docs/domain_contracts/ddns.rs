use serde::Serialize;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsPublicCheckSourcesData {
    ipv4: Vec<String>,
    ipv6: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsSettingsData {
    update_interval_minutes: i64,
    public_check_sources: DdnsPublicCheckSourcesData,
    default_public_check_sources: DdnsPublicCheckSourcesData,
    http_transport: String,
    public_dns_provider: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsSettingsUpdateData {
    update_interval_minutes: Option<i64>,
    public_check_sources: Option<DdnsPublicCheckSourcesData>,
    http_transport: Option<String>,
    public_dns_provider: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsToggleBodyData {
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsPublicCheckTestBodyData {
    public_check_sources: DdnsPublicCheckSourcesData,
    http_transport: Option<String>,
    public_dns_provider: Option<String>,
    network_interface: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsPublicCheckTestResultData {
    family: String,
    url: String,
    success: bool,
    #[schema(required = true)]
    status: Option<u16>,
    #[schema(required = true)]
    ip: Option<String>,
    response_preview: Option<String>,
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsPublicCheckTestResultsData {
    results: Vec<DdnsPublicCheckTestResultData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsProviderDomainTargetsData {
    mode: String,
    root_field: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsProviderCapabilitiesData {
    address_mode: Option<String>,
    ip_sources: Option<Vec<String>>,
    domain_targets: Option<DdnsProviderDomainTargetsData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsProviderFieldOptionData {
    label: String,
    value: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsProviderFieldData {
    key: String,
    label: String,
    #[serde(rename = "type")]
    field_type: String,
    placeholder: Option<String>,
    required: Option<bool>,
    options: Option<Vec<DdnsProviderFieldOptionData>>,
    description: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsProviderData {
    name: String,
    label: String,
    fields: Vec<DdnsProviderFieldData>,
    capabilities: Option<DdnsProviderCapabilitiesData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsNetworkInterfaceAddressData {
    family: String,
    address: String,
    #[schema(required = true)]
    cidr: Option<String>,
    prefix_length: Option<u8>,
    internal: bool,
    source: Option<String>,
    #[schema(required = true)]
    temporary: Option<bool>,
    #[schema(required = true)]
    deprecated: Option<bool>,
    #[schema(required = true)]
    tentative: Option<bool>,
    #[schema(required = true)]
    dad_failed: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsNetworkInterfaceData {
    name: String,
    label: String,
    summary: String,
    has_ipv4: bool,
    has_ipv6: bool,
    addresses: Vec<DdnsNetworkInterfaceAddressData>,
    selectable_addresses: Vec<DdnsNetworkInterfaceAddressData>,
    private_addresses: Vec<DdnsNetworkInterfaceAddressData>,
    source: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsInterfaceSelectorData {
    version: u8,
    mode: String,
    preferred_address: Option<String>,
    include_cidrs: Option<Vec<String>>,
    exclude_cidrs: Option<Vec<String>>,
    ipv6_interface_id: Option<String>,
    allow_temporary: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsInterfaceSelectorPreviewBodyData {
    network_interface: String,
    family: String,
    selector: DdnsInterfaceSelectorData,
    current_address: Option<String>,
    allow_private_addresses: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsRejectedAddressData {
    #[schema(required = true)]
    address: Option<String>,
    reasons: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsInterfaceSelectorPreviewData {
    #[schema(required = true)]
    selected_address: Option<String>,
    matched_addresses: Vec<DdnsNetworkInterfaceAddressData>,
    rejected_addresses: Vec<DdnsRejectedAddressData>,
    reason: String,
    warnings: Vec<String>,
    selector: DdnsInterfaceSelectorData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsProviderBodyData {
    provider: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsConfigData(pub(super) HashMap<String, String>);

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsConfigBodyData {
    config: HashMap<String, String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsTargetBodyData {
    name: Option<String>,
    provider: String,
    enabled: Option<bool>,
    config: Option<HashMap<String, String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsTargetEnabledBodyData {
    enabled: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsLastIpData {
    #[schema(required = true)]
    ipv4: Option<String>,
    #[schema(required = true)]
    ipv6: Option<String>,
    #[schema(required = true)]
    updated_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsLastCheckData {
    #[schema(required = true)]
    checked_at: Option<String>,
    #[schema(required = true)]
    outcome: Option<String>,
    #[schema(required = true)]
    message: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsTargetSummaryData {
    id: String,
    name: String,
    is_primary: bool,
    enabled: bool,
    #[schema(required = true)]
    provider: Option<String>,
    update_scope: String,
    provider_label: String,
    domain_summary: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
    #[serde(rename = "lastIP")]
    last_ip: DdnsLastIpData,
    selection_anchor: DdnsLastIpData,
    last_check: DdnsLastCheckData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsTargetDetailData {
    id: String,
    name: String,
    is_primary: bool,
    enabled: bool,
    #[schema(required = true)]
    provider: Option<String>,
    update_scope: String,
    provider_label: String,
    domain_summary: String,
    created_at: String,
    updated_at: String,
    sort_order: i64,
    #[serde(rename = "lastIP")]
    last_ip: DdnsLastIpData,
    selection_anchor: DdnsLastIpData,
    last_check: DdnsLastCheckData,
    raw_name: Option<String>,
    config: HashMap<String, String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsTargetListData {
    #[schema(required = true)]
    primary_target_id: Option<String>,
    total: usize,
    extra_count: usize,
    enabled_extra_count: usize,
    items: Vec<DdnsTargetSummaryData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct DdnsStatusData {
    enabled: bool,
    #[schema(required = true)]
    provider: Option<String>,
    update_interval_minutes: i64,
    public_check_sources: DdnsPublicCheckSourcesData,
    default_public_check_sources: DdnsPublicCheckSourcesData,
    http_transport: String,
    public_dns_provider: String,
    update_scope: String,
    ip_source: String,
    network_interface: String,
    #[serde(rename = "lastIP")]
    last_ip: DdnsLastIpData,
    selection_anchor: DdnsLastIpData,
    last_check: DdnsLastCheckData,
    #[schema(required = true)]
    primary_target_id: Option<String>,
    extra_target_count: usize,
    enabled_extra_target_count: usize,
    targets: Vec<DdnsTargetSummaryData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsLogEntryData {
    time: String,
    level: String,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsTestResultData {
    #[schema(required = true)]
    ipv4: Option<String>,
    #[schema(required = true)]
    ipv6: Option<String>,
    source: String,
    #[serde(rename = "sourceLabel")]
    source_label: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsTestResponseData {
    success: bool,
    message: String,
    data: DdnsTestResultData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct DdnsPollData {
    cursor: i64,
    reset: bool,
    logs: Vec<DdnsLogEntryData>,
    status: DdnsStatusData,
}
