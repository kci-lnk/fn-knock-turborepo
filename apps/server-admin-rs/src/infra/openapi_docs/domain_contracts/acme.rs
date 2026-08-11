use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeCertificateInfoData {
    issuer: String,
    subject: String,
    valid_from: String,
    valid_to: String,
    dns_names: Vec<String>,
    serial_number: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeCertificateSummaryData {
    primary_domain: String,
    info: AcmeCertificateInfoData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeInstallStateData {
    status: String,
    progress: i64,
    message: String,
    message_key: String,
    message_params: Option<HashMap<String, String>>,
    executable_path: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeStatusData {
    status: String,
    progress: i64,
    message: String,
    message_key: String,
    message_params: Option<HashMap<String, String>>,
    executable_path: String,
    #[schema(required = true)]
    acme_cert: Option<AcmeCertificateSummaryData>,
    certificate_authority: String,
    certificate_authority_updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeResourceProgressData {
    status: String,
    percent: i64,
    #[schema(required = true)]
    error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeResourceStatusData {
    supported: bool,
    initialized: bool,
    platform: String,
    #[schema(required = true)]
    installed_version: Option<String>,
    #[schema(required = true)]
    available_version: Option<String>,
    progress: AcmeResourceProgressData,
    provider_ids: Vec<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeResourceInitializeData {
    started: bool,
    bundled: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeResourceCancelData {
    cancel_requested: bool,
    bundled: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeClientSettingsBodyData {
    certificate_authority: Option<String>,
    account_email: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeClientSettingsData {
    certificate_authority: String,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeClientSettingsUpdateData {
    certificate_authority: String,
    updated_at: String,
    synced: bool,
    #[schema(required = true)]
    account_email: Option<String>,
    state: Option<AcmeInstallStateData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeInitData {
    executable_path: String,
    certificate_authority: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeConfigBodyData {
    domains: Vec<String>,
    dns_type: Option<String>,
    provider: Option<String>,
    credentials: Option<HashMap<String, Value>>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeConfigData {
    domains: Vec<String>,
    dns_type: String,
    credentials: HashMap<String, String>,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeApplicationBodyData {
    name: Option<String>,
    domains: Vec<String>,
    dns_type: Option<String>,
    provider: Option<String>,
    credentials: Option<HashMap<String, Value>>,
    renew_enabled: Option<bool>,
    submit_now: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeApplicationData {
    id: String,
    name: Option<String>,
    domains: Vec<String>,
    primary_domain: String,
    dns_type: String,
    credentials: HashMap<String, String>,
    renew_enabled: bool,
    created_at: String,
    updated_at: String,
    latest_job_id: Option<String>,
    latest_job_status: Option<String>,
    latest_job_trigger: Option<String>,
    latest_job_at: Option<String>,
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeRuntimeLockData {
    locked: bool,
    lock_id: Option<String>,
    job_id: Option<String>,
    application_id: Option<String>,
    reason: Option<String>,
    started_at: Option<String>,
    heartbeat_at: Option<String>,
    expires_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeJobData {
    id: String,
    application_id: Option<String>,
    domains: Vec<String>,
    method: String,
    provider: Option<String>,
    trigger: Option<String>,
    created_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    status: String,
    progress: i64,
    message: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeApplicationMutationData {
    application: AcmeApplicationData,
    job: Option<AcmeJobData>,
    lock: Option<AcmeRuntimeLockData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeApplicationRequestData {
    job: AcmeJobData,
    lock: AcmeRuntimeLockData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeApplicationDeleteData {
    id: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeLibrarySyncData {
    certificate_id: String,
    linked: bool,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeLegacyRequestBodyData {
    domains: Vec<String>,
    dns_type: Option<String>,
    provider: Option<String>,
    credentials: Option<HashMap<String, Value>>,
    method: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeLegacyRequestData {
    job_id: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeProcessResultData {
    matched_pids: Vec<i64>,
    remaining_pids: Vec<i64>,
    errors: Vec<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeStopJobData {
    stopped: bool,
    #[schema(required = true)]
    job: Option<AcmeJobData>,
    lock: AcmeRuntimeLockData,
    process_result: AcmeProcessResultData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeCredentialFieldData {
    key: String,
    required: bool,
    label: Option<String>,
    description: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeCredentialSchemeData {
    id: String,
    label: String,
    description: Option<String>,
    fields: Vec<AcmeCredentialFieldData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeDnsProviderData {
    dns_type: String,
    label: String,
    group: String,
    credential_schemes: Vec<AcmeCredentialSchemeData>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeLatestJobData {
    id: String,
    status: String,
    trigger: String,
    created_at: String,
    message: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeOverviewCertificateData {
    exists: bool,
    valid_from: Option<String>,
    valid_to: Option<String>,
    dns_names: Option<Vec<String>>,
    issuer: Option<String>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeOverviewLibraryData {
    linked: bool,
    certificate_id: Option<String>,
    is_active: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeApplicationOverviewData {
    id: String,
    name: Option<String>,
    primary_domain: String,
    domains: Vec<String>,
    dns_type: String,
    provider_label: String,
    renew_enabled: bool,
    created_at: String,
    updated_at: String,
    #[schema(required = true)]
    latest_job: Option<AcmeLatestJobData>,
    certificate: AcmeOverviewCertificateData,
    library: AcmeOverviewLibraryData,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeRunningJobData {
    id: String,
    #[schema(required = true)]
    application_id: Option<String>,
    status: String,
    progress: i64,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct AcmeOverviewData {
    acme_state: AcmeInstallStateData,
    client_settings: AcmeClientSettingsData,
    lock: AcmeRuntimeLockData,
    applications: Vec<AcmeApplicationOverviewData>,
    #[schema(required = true)]
    running_job: Option<AcmeRunningJobData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeLogAnalysisData {
    reason: String,
    provider: Option<String>,
    message: String,
    evidence: Option<Vec<String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeJobPollData {
    job: AcmeJobData,
    logs: Vec<String>,
    #[schema(required = true)]
    analysis: Option<AcmeLogAnalysisData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeCertificateData {
    domain: String,
    info: AcmeCertificateInfoData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeSubdomainRecommendationData {
    mode: String,
    #[schema(required = true)]
    root_domain: Option<String>,
    #[schema(required = true)]
    auth_host: Option<String>,
    recommended_domains: Vec<String>,
    covered_hosts: Vec<String>,
    uncovered_hosts: Vec<String>,
    warnings: Vec<String>,
    can_autofill: bool,
    summary: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct AcmeActionMessageData {
    success: bool,
    message: String,
}
