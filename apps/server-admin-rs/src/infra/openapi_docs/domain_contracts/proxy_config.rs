use serde::Serialize;
use utoipa::ToSchema;

use super::{GatewayVisibilitySelectionData, GatewayVisibilitySelectionInputData};
use std::collections::HashMap;

#[derive(Serialize, ToSchema)]
pub(super) struct ProxyMappingData {
    path: Option<String>,
    target: String,
    rewrite_html: Option<bool>,
    use_auth: Option<bool>,
    use_root_mode: Option<bool>,
    strip_path: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ProxyMappingsUpdateData {
    mappings: Vec<ProxyMappingData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamServiceProfileData {
    service_id: String,
    service_family: String,
    device_role: String,
    service_confidence: String,
    role_confidence: String,
    source: String,
    observed_at: String,
    classifier_version: String,
    target_fingerprint: String,
    evidence_codes: Vec<String>,
    strict_capable: bool,
    metadata: HashMap<String, String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamBypassRegionSelectionData {
    province: String,
    #[schema(required = true)]
    city: Option<String>,
    #[schema(required = true)]
    query_city: Option<String>,
    #[schema(required = true)]
    operator: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamBypassConditionData {
    id: String,
    target: String,
    operator: String,
    policy_id: String,
    values: Vec<String>,
    selections: Vec<StreamBypassRegionSelectionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamBypassGroupData {
    id: String,
    conditions: Vec<StreamBypassConditionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamBypassPolicyData {
    enabled: bool,
    policy_version: String,
    groups: Vec<StreamBypassGroupData>,
    broad_rule_confirmed: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamMappingData {
    protocol: String,
    listen_port: i64,
    target: String,
    use_auth: bool,
    #[schema(nullable = false)]
    comment: Option<String>,
    #[schema(nullable = false)]
    disabled: Option<bool>,
    #[schema(nullable = false)]
    validation_mode: Option<String>,
    #[schema(nullable = false)]
    service_profile: Option<StreamServiceProfileData>,
    #[schema(nullable = false)]
    bypass_policy: Option<StreamBypassPolicyData>,
    #[schema(nullable = false)]
    probe_status: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamMappingInputData {
    #[schema(nullable = false)]
    protocol: Option<String>,
    listen_port: i64,
    target: String,
    #[schema(nullable = false)]
    use_auth: Option<bool>,
    #[schema(nullable = false)]
    comment: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StreamMappingsUpdateData {
    mappings: Vec<StreamMappingInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SubdomainModeData {
    root_domain: String,
    auth_host: String,
    auth_target: String,
    cookie_domain: String,
    edge_client_ip_enabled: bool,
    aliyun_esa_enabled: bool,
    tencent_edgeone_enabled: bool,
    public_auth_base_url: String,
    public_http_port: i64,
    public_https_port: i64,
    auth_cache_ttl_seconds: i64,
    auth_cache_unauthorized_ttl_seconds: i64,
    default_access_mode: String,
    auto_add_whitelist_on_login: bool,
    passkey_rp_mode: String,
    passkey_rp_id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SubdomainModeUpdateData {
    root_domain: Option<String>,
    auth_host: Option<String>,
    auth_target: Option<String>,
    cookie_domain: Option<String>,
    edge_client_ip_enabled: Option<bool>,
    aliyun_esa_enabled: Option<bool>,
    tencent_edgeone_enabled: Option<bool>,
    public_auth_base_url: Option<String>,
    public_http_port: Option<i64>,
    public_https_port: Option<i64>,
    auth_cache_ttl_seconds: Option<i64>,
    auth_cache_unauthorized_ttl_seconds: Option<i64>,
    default_access_mode: Option<String>,
    auto_add_whitelist_on_login: Option<bool>,
    passkey_rp_mode: Option<String>,
    passkey_rp_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SubdomainSslAutoSelectionData {
    applied: bool,
    certificate_id: String,
    label: String,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SubdomainModeResponseData {
    root_domain: String,
    auth_host: String,
    auth_target: String,
    cookie_domain: String,
    edge_client_ip_enabled: bool,
    aliyun_esa_enabled: bool,
    tencent_edgeone_enabled: bool,
    public_auth_base_url: String,
    public_http_port: i64,
    public_https_port: i64,
    auth_cache_ttl_seconds: i64,
    auth_cache_unauthorized_ttl_seconds: i64,
    default_access_mode: String,
    auto_add_whitelist_on_login: bool,
    passkey_rp_mode: String,
    passkey_rp_id: String,
    #[schema(required = true)]
    ssl_auto_selection: Option<SubdomainSslAutoSelectionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingBasicAuthInputData {
    enabled: bool,
    username: String,
    password: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingBasicAuthProbeBodyData {
    target: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostMappingBasicAuthProbeData {
    requires_basic_auth: bool,
    #[schema(required = true)]
    http_status: Option<u16>,
    #[schema(nullable = false)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum StaticPathTargetTypeData {
    File,
    Directory,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum StaticPathActualTypeData {
    File,
    Directory,
    Other,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StaticPathProbeBodyData {
    target_type: StaticPathTargetTypeData,
    path: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StaticPathProbeResultData {
    #[schema(required = true)]
    target_type: Option<StaticPathTargetTypeData>,
    normalized_path: String,
    exists: bool,
    readable: bool,
    #[schema(required = true)]
    actual_type: Option<StaticPathActualTypeData>,
    #[schema(required = true)]
    error_code: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum StaticPathBrowsePlatformData {
    Posix,
    Windows,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub(super) enum StaticPathBrowseErrorCodeData {
    InvalidPath,
    InvalidCursor,
    ProtectedPath,
    NotFound,
    PermissionDenied,
    NotDirectory,
    DirectoryTooLarge,
    UnsupportedType,
    Unavailable,
}

#[derive(Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct StaticPathBrowseBodyData {
    target_type: StaticPathTargetTypeData,
    #[schema(max_length = 4096)]
    path: Option<String>,
    #[schema(max_length = 512)]
    cursor: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StaticPathBreadcrumbData {
    #[schema(max_length = 255)]
    name: String,
    #[schema(max_length = 4096)]
    path: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StaticPathBrowseEntryData {
    #[schema(max_length = 255)]
    name: String,
    #[schema(max_length = 4096)]
    path: String,
    entry_type: StaticPathTargetTypeData,
    navigable: bool,
    selectable: bool,
    #[schema(required = true)]
    size_bytes: Option<u64>,
    #[schema(required = true, format = DateTime)]
    modified_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct StaticPathBrowseResultData {
    target_type: StaticPathTargetTypeData,
    platform: StaticPathBrowsePlatformData,
    #[schema(required = true, max_length = 4096)]
    current_path: Option<String>,
    #[schema(required = true, max_length = 4096)]
    parent_path: Option<String>,
    current_selectable: bool,
    #[schema(required = true, max_length = 4096)]
    selected_path: Option<String>,
    #[schema(max_items = 256)]
    breadcrumbs: Vec<StaticPathBreadcrumbData>,
    #[schema(max_items = 100)]
    entries: Vec<StaticPathBrowseEntryData>,
    #[schema(required = true, max_length = 512)]
    previous_cursor: Option<String>,
    #[schema(required = true, max_length = 512)]
    next_cursor: Option<String>,
    #[schema(required = true)]
    error_code: Option<StaticPathBrowseErrorCodeData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingMetadataBodyData {
    target: String,
    basic_auth: Option<HostMappingBasicAuthInputData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct HostMappingMetadataData {
    title: String,
    favicon: String,
    final_url: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct HostMappingRefreshSummaryData {
    updated: i64,
    failed: i64,
    skipped: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthConditionData {
    id: String,
    target: String,
    operator: String,
    name: String,
    values: Vec<String>,
    #[schema(nullable = false)]
    selections: Option<Vec<GatewayVisibilitySelectionData>>,
    #[schema(nullable = false)]
    cidrs: Option<Vec<String>>,
    #[schema(nullable = false)]
    policy_id: Option<String>,
    #[schema(nullable = false)]
    source_cidr_count: Option<usize>,
    #[schema(nullable = false)]
    range_count: Option<usize>,
    #[schema(nullable = false)]
    resolved_at: Option<String>,
    #[schema(nullable = false)]
    cidr_source: Option<String>,
    #[schema(nullable = false)]
    cidr_source_fingerprint: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthConditionInputData {
    id: String,
    target: String,
    operator: String,
    #[schema(nullable = false)]
    name: Option<String>,
    #[schema(nullable = false)]
    values: Option<Vec<String>>,
    #[schema(nullable = false)]
    selections: Option<Vec<GatewayVisibilitySelectionInputData>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthRuleGroupData {
    id: String,
    conditions: Vec<AdvancedAuthConditionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthRuleGroupInputData {
    id: String,
    conditions: Vec<AdvancedAuthConditionInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthConfigData {
    enabled: bool,
    idle_ttl_seconds: i64,
    max_lifetime_seconds: i64,
    #[schema(nullable = false)]
    policy_version: Option<String>,
    groups: Vec<AdvancedAuthRuleGroupData>,
    #[schema(nullable = false)]
    compiled_at: Option<String>,
    #[schema(nullable = false)]
    cidr_source: Option<String>,
    #[schema(nullable = false)]
    cidr_source_fingerprint: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthConfigInputData {
    enabled: Option<bool>,
    idle_ttl_seconds: Option<i64>,
    max_lifetime_seconds: Option<i64>,
    groups: Option<Vec<AdvancedAuthRuleGroupInputData>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthUpdateBodyData {
    revision: Option<String>,
    #[schema(nullable = false)]
    advanced_auth: Option<AdvancedAuthConfigInputData>,
    acknowledge_broad_rules: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AdvancedAuthDetailsData {
    host: String,
    revision: String,
    advanced_auth: AdvancedAuthConfigData,
}
