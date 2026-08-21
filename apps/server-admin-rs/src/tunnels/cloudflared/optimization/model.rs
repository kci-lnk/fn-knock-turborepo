use super::*;

pub(super) const OPTIMIZATION_RUNTIME_KEY: &str = "fn_knock:cloudflared:optimization:runtime:v1";
pub(super) const OPTIMIZATION_SETTINGS_KEY: &str = "fn_knock:cloudflared:optimization:settings:v1";
pub(super) const OPTIMIZATION_DOMAIN_SETTINGS_KEY: &str =
    "fn_knock:cloudflared:optimization:domain-settings:v1";
pub(super) const CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE: &str = "cloudflare-saas-required";
pub(super) const CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR: &str =
    "Cloudflare for SaaS is not enabled or available for the selected zone";
pub(super) const CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE: &str =
    "cloudflare-saas-validation-pending";
pub(super) const CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR: &str =
    "Cloudflare for SaaS is enabled, but hostname and certificate validation is still in progress";
pub(super) const CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE: &str = "cloudflare-resource-conflict";
pub(super) const CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR: &str =
    "Cloudflare Custom Hostname or DNS ownership conflicts must be reconciled";
pub(super) const OPTIMIZATION_NOT_READY_ERROR_CODE: &str = "cloudflare-optimization-not-ready";
pub(super) const OPTIMIZATION_NOT_READY_SCAN_ERROR: &str =
    "Cloudflare optimization is not ready for TLS and SNI validation";
pub(super) const CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE: &str =
    "cloudflare-candidate-resolution-unavailable";
pub(super) const CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR: &str =
    "No verified Cloudflare candidate address could be resolved";
pub(super) const SPEEDTEST_HOST: &str = "speed.cloudflare.com";
pub(super) const SPEEDTEST_PATH: &str = "/__down";
pub(super) const MAX_CANDIDATES: usize = 128;
pub(super) const CANDIDATES_PER_PREFIX: usize = 8;
pub(super) const PROBE_CONCURRENCY: usize = 32;
pub(super) const SNI_VALIDATION_CONCURRENCY: usize = 16;
pub(super) const LATENCY_PROBES: usize = 3;
pub(super) const DOWNLOAD_SHORTLIST: usize = 8;
pub(super) const DOWNLOAD_BYTES: usize = 1024 * 1024;
pub(super) const MAX_DOWNLOAD_BUDGET: usize = 20 * 1024 * 1024;
const _: () = {
    assert!(MAX_CANDIDATES <= 128);
    assert!(PROBE_CONCURRENCY <= 32);
    assert!(LATENCY_PROBES == 3);
    assert!(DOWNLOAD_SHORTLIST * 2 * DOWNLOAD_BYTES <= MAX_DOWNLOAD_BUDGET);
};
pub(super) const MAX_CUSTOM_HOSTNAMES: usize = 100;
pub(super) const MAX_CUSTOM_HOSTNAME_CREATES_PER_RECONCILE: usize = 10;
pub(super) const MAX_CUSTOM_SOURCE_HOSTNAMES: usize = 16;
pub(super) const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;
pub(super) const HEALTH_INTERVAL_MS: i64 = 15 * 60 * 1000;
pub(super) const CONFIRMATION_DELAY_MS: i64 = 10 * 60 * 1000;
pub(super) const SCAN_APPLY_TTL_MS: i64 = 10 * 60 * 1000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CapabilityProbeResult {
    Ready,
    Pending,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum FallbackOriginResult {
    Ready,
    Pending,
}

#[derive(Clone, Debug)]
pub(super) struct RecoverableCustomHostname {
    pub(super) legacy_instance_id: String,
    pub(super) origin_hostname: String,
    pub(super) origin_dns: Value,
    pub(super) exact_dns: Value,
}

pub(super) const CLOUDFLARE_IPV4_FALLBACK: &[&str] = &[
    "103.21.244.0/22",
    "103.22.200.0/22",
    "103.31.4.0/22",
    "104.16.0.0/13",
    "104.24.0.0/14",
    "108.162.192.0/18",
    "131.0.72.0/22",
    "141.101.64.0/18",
    "162.158.0.0/15",
    "172.64.0.0/13",
    "173.245.48.0/20",
    "188.114.96.0/20",
    "190.93.240.0/20",
    "197.234.240.0/22",
    "198.41.128.0/17",
];

#[derive(Clone, Copy, Debug)]
pub(super) struct BuiltinCandidateSource {
    pub(super) id: &'static str,
    pub(super) hostname: &'static str,
    pub(super) category: &'static str,
}

// These hostnames are only resolved into candidate Cloudflare IPv4 addresses.
// fn-knock never publishes a customer CNAME to, or sends HTTP traffic with the
// third-party hostname/SNI to, any source in this catalog.
pub(super) const BUILTIN_CANDIDATE_SOURCES: &[BuiltinCandidateSource] = &[
    BuiltinCandidateSource {
        id: "sweden-government",
        hostname: "www.gov.se",
        category: "government",
    },
    BuiltinCandidateSource {
        id: "us-library-of-congress",
        hostname: "www.loc.gov",
        category: "public-institution",
    },
    BuiltinCandidateSource {
        id: "icann",
        hostname: "www.icann.org",
        category: "internet-infrastructure",
    },
    BuiltinCandidateSource {
        id: "visa",
        hostname: "www.visa.com",
        category: "payment-infrastructure",
    },
];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizationSourceSettings {
    #[serde(default = "default_true")]
    pub(super) official_ranges: bool,
    #[serde(default = "default_builtin_source_ids")]
    pub(super) builtin_ids: Vec<String>,
    #[serde(default)]
    pub(super) custom_hostnames: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizationDomainSettings {
    #[serde(default)]
    pub(super) external_hostnames: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartOptimizationScanRequest {
    #[serde(default)]
    pub(super) preferred_ip: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UpdateOptimizationDomainRequest {
    pub(super) mode: String,
}

impl Default for OptimizationSourceSettings {
    fn default() -> Self {
        Self {
            official_ranges: true,
            builtin_ids: default_builtin_source_ids(),
            custom_hostnames: Vec::new(),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct CandidateSeed {
    pub(super) ip: Ipv4Addr,
    pub(super) source_types: Vec<String>,
    pub(super) source_hostnames: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct LatencyProbeMetrics {
    pub(super) median_latency_ms: f64,
    pub(super) jitter_ms: f64,
    pub(super) loss_ratio: f64,
    pub(super) colo: Option<String>,
    pub(super) cf_ray: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct BusinessProbeResult {
    pub(super) status: u16,
    pub(super) colo: Option<String>,
    pub(super) cf_ray: Option<String>,
}

#[derive(Debug)]
pub(super) struct OptimizationScanResult {
    pub(super) candidates: Vec<OptimizationCandidate>,
    pub(super) vantage: Value,
    pub(super) source_warnings: Vec<String>,
    pub(super) resolver_diagnostics: Vec<ResolverDiagnostic>,
    pub(super) resolution_path: String,
    pub(super) source_fingerprint: String,
}

#[derive(Clone, Debug)]
pub(super) struct ConfirmationSnapshot {
    pub(super) pending: OptimizationCandidate,
    pub(super) current: OptimizationCandidate,
    pub(super) hostname: String,
    pub(super) selected_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OptimizationCandidate {
    pub(super) ip: String,
    pub(super) median_latency_ms: f64,
    pub(super) jitter_ms: f64,
    pub(super) loss_ratio: f64,
    pub(super) download_mbps: f64,
    pub(super) score: f64,
    #[serde(default)]
    pub(super) verified_at: Option<String>,
    #[serde(default)]
    pub(super) source_types: Vec<String>,
    #[serde(default)]
    pub(super) source_hostnames: Vec<String>,
    #[serde(default)]
    pub(super) colo: Option<String>,
    #[serde(default)]
    pub(super) cf_ray: Option<String>,
    #[serde(default)]
    pub(super) business_hostname: Option<String>,
    #[serde(default)]
    pub(super) business_status: Option<u16>,
    #[serde(default)]
    pub(super) business_colo: Option<String>,
    #[serde(default)]
    pub(super) business_cf_ray: Option<String>,
    #[serde(default)]
    pub(super) business_validated: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ApplyOptimizationRequest {
    pub(super) scan_id: String,
    #[serde(default)]
    pub(super) candidate_ip: Option<String>,
}
