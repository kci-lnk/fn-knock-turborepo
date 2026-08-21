use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct SslCertificateSaveBodyData {
    id: Option<String>,
    label: Option<String>,
    source: Option<String>,
    source_provider: Option<String>,
    primary_domain: Option<String>,
    source_ref_id: Option<String>,
    cert: String,
    key: String,
    activate: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCertificateActivateBodyData {
    id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslDeploymentModeBodyData {
    deployment_mode: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCaHostBodyData {
    value: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCaHostsDeleteBodyData {
    value: Option<String>,
    all: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SslCertificateInfoData {
    issuer: String,
    subject: String,
    valid_from: String,
    valid_to: String,
    dns_names: Vec<String>,
    serial_number: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslSubdomainCoverageData {
    status: String,
    #[schema(required = true)]
    auth_host: Option<String>,
    certificate_domains: Vec<String>,
    recommended_domains: Vec<String>,
    covered_recommended_domains: Vec<String>,
    uncovered_recommended_domains: Vec<String>,
    covered_hosts: Vec<String>,
    uncovered_hosts: Vec<String>,
    covers_auth_host: bool,
    warnings: Vec<String>,
    summary: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCertificateLibraryCoverageData {
    status: String,
    deployment_mode: String,
    #[schema(required = true)]
    active_certificate_id: Option<String>,
    fully_covering_certificate_ids: Vec<String>,
    partially_covering_certificate_ids: Vec<String>,
    combined_covering_certificate_ids: Vec<String>,
    #[schema(required = true)]
    suggested_certificate_id: Option<String>,
    can_auto_activate: bool,
    warnings: Vec<String>,
    summary: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCertificateSummaryData {
    id: String,
    label: String,
    source: String,
    source_provider: Option<String>,
    primary_domain: Option<String>,
    source_ref_id: Option<String>,
    created_at: String,
    updated_at: String,
    #[serde(rename = "certInfo")]
    cert_info: Option<SslCertificateInfoData>,
    is_active: bool,
    coverage: SslSubdomainCoverageData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslGatewayCertificateData {
    id: Option<String>,
    label: Option<String>,
    domains: Option<Vec<String>>,
    is_default: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslGatewayStatusData {
    enabled: bool,
    deployment_mode: String,
    certificates: Vec<SslGatewayCertificateData>,
    sync_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SslStatusData {
    enabled: bool,
    active_cert_id: Option<String>,
    deployment_mode: String,
    configured_deployment_mode: String,
    cert_info: Option<SslCertificateInfoData>,
    certificates: Vec<SslCertificateSummaryData>,
    #[serde(rename = "subdomain_coverage")]
    subdomain_coverage: SslSubdomainCoverageData,
    #[serde(rename = "library_coverage")]
    library_coverage: SslCertificateLibraryCoverageData,
    #[serde(rename = "gateway_status")]
    gateway_status: SslGatewayStatusData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SslSharedFileData {
    name: String,
    relative_path: String,
    extension: String,
    size: u64,
    modified_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct SslSharedFilesData {
    share_name: String,
    available: bool,
    files: Vec<SslSharedFileData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslSharedFileContentData {
    file: SslSharedFileData,
    content: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCaStatusData {
    initialized: bool,
    info: Option<SslCertificateInfoData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct SslCertificateSaveData {
    id: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateBindingCreateBodyData {
    name: String,
    provider: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateBindingUpdateBodyData {
    name: Option<String>,
    enabled: Option<bool>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LanCertificateDeploymentUpdateBodyData {
    enabled: bool,
    addresses: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LanCertificateDeploymentData {
    enabled: bool,
    configured_addresses: Vec<String>,
    detected_addresses: Vec<String>,
    gateway_port: u16,
    listener_scope: String,
    status: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateBindingData {
    id: String,
    name: String,
    provider: String,
    certificate_id: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
    last_deployed_at: Option<String>,
    last_result: Option<String>,
    last_error: Option<String>,
    last_fingerprint_sha256: Option<String>,
    last_valid_to: Option<String>,
    last_dns_names: Vec<String>,
    deploy_path: String,
    deploy_port: u16,
    #[schema(required = true)]
    public_deploy_url: Option<String>,
    public_deploy_status: String,
    lan_deploy_urls: Vec<String>,
    lan_deploy_status: String,
    setup_kind: String,
    request_method: Option<String>,
    request_body_template: Option<String>,
    success_marker: Option<String>,
    script_template: Option<String>,
    usage_instructions: Option<String>,
    last_replaced_certificate_count: usize,
    last_replaced_sources: Vec<String>,
    last_disabled_external_binding_count: usize,
    last_disabled_acme_renewal_count: usize,
    #[schema(required = true)]
    last_takeover_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateBindingCredentialData {
    binding: ExternalCertificateBindingData,
    token: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateDeployBodyData {
    cert: String,
    key: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalCertificateDeployData {
    binding_id: String,
    certificate_id: String,
    changed: bool,
    gateway_applied: bool,
    is_active: bool,
    fingerprint_sha256: String,
    valid_to: String,
    dns_names: Vec<String>,
    replaced_certificate_count: usize,
    replaced_sources: Vec<String>,
    disabled_external_binding_count: usize,
    disabled_acme_renewal_count: usize,
}
