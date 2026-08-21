use super::*;
use axum::{
    body::to_bytes,
    extract::{DefaultBodyLimit, Request},
    http::{
        HeaderMap,
        header::{AUTHORIZATION, HOST},
    },
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use utoipa::ToSchema;
use utoipa_axum::router::UtoipaMethodRouterExt;

const EXTERNAL_CERTIFICATE_BINDINGS_KEY: &str = "fn_knock:ssl:external_certificate_bindings";
const EXTERNAL_CERTIFICATE_BODY_LIMIT: usize = 1024 * 1024;
const MAX_BINDING_NAME_LENGTH: usize = 80;
const MAX_BINDING_ERROR_LENGTH: usize = 500;
const TOKEN_PREFIX: &str = "fnk_cert_";
const PUBLIC_CERTIFICATE_DEPLOY_PATH_PREFIX: &str = "/__certificates__";

#[derive(Clone, Debug, Deserialize, Serialize)]
struct ExternalCertificateBinding {
    id: String,
    name: String,
    provider: String,
    certificate_id: String,
    token_hash: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    last_deployed_at: Option<String>,
    #[serde(default)]
    last_result: Option<String>,
    #[serde(default)]
    last_error: Option<String>,
    #[serde(default)]
    last_fingerprint_sha256: Option<String>,
    #[serde(default)]
    last_valid_to: Option<String>,
    #[serde(default)]
    last_dns_names: Vec<String>,
    #[serde(default)]
    last_replaced_certificate_count: usize,
    #[serde(default)]
    last_replaced_sources: Vec<String>,
    #[serde(default)]
    last_disabled_external_binding_count: usize,
    #[serde(default)]
    last_disabled_acme_renewal_count: usize,
    #[serde(default)]
    last_takeover_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Debug, Serialize, ToSchema)]
pub(super) struct ExternalCertificateBindingCredentialData {
    binding: ExternalCertificateBindingData,
    token: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct ExternalCertificateBindingCreateBody {
    name: String,
    provider: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct ExternalCertificateBindingUpdateBody {
    name: Option<String>,
    enabled: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub(super) struct ExternalCertificateDeployBody {
    cert: String,
    key: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Debug)]
struct CertificateMetadata {
    fingerprint_sha256: String,
    valid_to: String,
    not_after_ms: i64,
    dns_names: Vec<String>,
}

#[derive(Clone, Copy)]
struct ExternalCertificateProviderAdapter {
    id: &'static str,
    setup: ExternalCertificateProviderSetup,
}

#[derive(Clone, Copy)]
enum ExternalCertificateProviderSetup {
    Webhook {
        request_method: &'static str,
        request_body_template: &'static str,
        success_marker: &'static str,
    },
    DeployHook {
        script: ExternalCertificateDeployHook,
        usage_instructions: &'static str,
    },
}

#[derive(Clone, Copy)]
enum ExternalCertificateDeployHook {
    AcmeSh,
    Lego,
    Certbot,
}

const CERTIFICATE_UPLOAD_SHELL_FUNCTION: &str = r##"_fn_knock_upload_certificate() {
  _fnk_cert_path="$1"
  _fnk_key_path="$2"
  if ! command -v jq >/dev/null 2>&1 || ! command -v curl >/dev/null 2>&1; then
    echo "fn-knock deploy hook requires jq and curl" >&2
    return 1
  fi
  _fnk_attempt=1
  while [ "$_fnk_attempt" -le 4 ]; do
    if jq -n --rawfile cert "$_fnk_cert_path" --rawfile key "$_fnk_key_path" \
      '{cert: $cert, key: $key}' |
      curl --silent --show-error --fail-with-body \
        --connect-timeout 10 \
        --max-time 60 \
        --request PUT \
        --header "Authorization: Bearer __FN_KNOCK_DEPLOY_TOKEN__" \
        --header "Content-Type: application/json" \
        --data-binary @- \
        "__FN_KNOCK_DEPLOY_URL__"; then
      return 0
    fi
    if [ "$_fnk_attempt" -ge 4 ]; then
      return 1
    fi
    sleep 2
    _fnk_attempt=$((_fnk_attempt + 1))
  done
}"##;

const CERTD_PROVIDER_ADAPTER: ExternalCertificateProviderAdapter =
    ExternalCertificateProviderAdapter {
        id: "certd",
        setup: ExternalCertificateProviderSetup::Webhook {
            request_method: "PUT",
            request_body_template: r#"{"cert":"${crt}","key":"${key}"}"#,
            success_marker: r#""success":true"#,
        },
    };

const ACME_SH_PROVIDER_ADAPTER: ExternalCertificateProviderAdapter =
    ExternalCertificateProviderAdapter {
        id: "acme_sh",
        setup: ExternalCertificateProviderSetup::DeployHook {
            script: ExternalCertificateDeployHook::AcmeSh,
            usage_instructions: "Save as ~/.acme.sh/deploy/fnknock.sh with mode 700, then run: ~/.acme.sh/acme.sh --deploy -d example.com --deploy-hook fnknock",
        },
    };

const LEGO_PROVIDER_ADAPTER: ExternalCertificateProviderAdapter =
    ExternalCertificateProviderAdapter {
        id: "lego",
        setup: ExternalCertificateProviderSetup::DeployHook {
            script: ExternalCertificateDeployHook::Lego,
            usage_instructions: "Save the script and run chmod 700 on it. With lego v5 add --deploy-hook=/path/to/fn-knock-lego-hook.sh (or set hooks.deploy.command in .lego.yaml); with lego v4 use --renew-hook=/path/to/fn-knock-lego-hook.sh.",
        },
    };

const CERTBOT_PROVIDER_ADAPTER: ExternalCertificateProviderAdapter =
    ExternalCertificateProviderAdapter {
        id: "certbot",
        setup: ExternalCertificateProviderSetup::DeployHook {
            script: ExternalCertificateDeployHook::Certbot,
            usage_instructions: "Save with mode 700 under /etc/letsencrypt/renewal-hooks/deploy/fn-knock, or run certbot renew --deploy-hook /path/to/fn-knock-certbot-hook.sh.",
        },
    };

const EXTERNAL_CERTIFICATE_PROVIDER_ADAPTERS: &[ExternalCertificateProviderAdapter] = &[
    CERTD_PROVIDER_ADAPTER,
    ACME_SH_PROVIDER_ADAPTER,
    LEGO_PROVIDER_ADAPTER,
    CERTBOT_PROVIDER_ADAPTER,
];

fn render_deploy_hook(script: ExternalCertificateDeployHook) -> String {
    let provider_specific = match script {
        ExternalCertificateDeployHook::AcmeSh => {
            r##"# acme.sh deploy-hook contract: domain, key, cert, CA, fullchain, PFX.
fnknock_deploy() {
  _fnk_key="$2"
  _fnk_fullchain="$5"
  _fn_knock_upload_certificate "$_fnk_fullchain" "$_fnk_key"
}"##
        }
        ExternalCertificateDeployHook::Lego => {
            r##"# lego v5 uses LEGO_HOOK_*; the fallbacks keep the hook compatible with v4.
cert_path="${LEGO_HOOK_CERT_PATH:-${LEGO_CERT_PATH:-}}"
key_path="${LEGO_HOOK_CERT_KEY_PATH:-${LEGO_CERT_KEY_PATH:-}}"
if [ -z "$cert_path" ] || [ -z "$key_path" ]; then
  echo "lego did not provide certificate paths to the fn-knock deploy hook" >&2
  exit 1
fi
_fn_knock_upload_certificate "$cert_path" "$key_path""##
        }
        ExternalCertificateDeployHook::Certbot => {
            r##"lineage="${RENEWED_LINEAGE:?Certbot did not set RENEWED_LINEAGE}"
_fn_knock_upload_certificate "$lineage/fullchain.pem" "$lineage/privkey.pem""##
        }
    };
    let shell_options = if matches!(script, ExternalCertificateDeployHook::AcmeSh) {
        ""
    } else {
        "set -eu\n\n"
    };
    format!(
        "#!/usr/bin/env sh\n{shell_options}{CERTIFICATE_UPLOAD_SHELL_FUNCTION}\n\n{provider_specific}\n"
    )
}

#[derive(Debug)]
struct PreparedExternalCertificateUpdate {
    previous_ssl: Option<Value>,
    next_config: Value,
    changed: bool,
    should_sync_gateway: bool,
    is_active: bool,
    takeover: ExternalCertificateTakeover,
}

#[derive(Clone, Debug, Default)]
struct ExternalCertificateTakeover {
    replaced_certificate_ids: BTreeSet<String>,
    replaced_sources: BTreeSet<String>,
    acme_application_ids: BTreeSet<String>,
}

#[derive(Clone, Debug)]
struct ExternalCertificateDeploymentEndpoints {
    public_deploy_base_url: Option<String>,
    public_deploy_status: String,
    lan_deploy_base_urls: Vec<String>,
    lan_deploy_status: String,
}

#[derive(Debug)]
enum ExternalDeployError {
    BadRequest(String),
    PayloadTooLarge,
    Unauthorized,
    Unavailable,
    Conflict(String),
    Storage(String),
    Gateway(String),
    GatewayRestored(String),
    GatewayRollbackFailed(String),
}

impl ExternalDeployError {
    fn into_response(self) -> Response {
        match self {
            Self::BadRequest(message) => response::error(StatusCode::BAD_REQUEST, message),
            Self::PayloadTooLarge => response::error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "Certificate deployment payload is too large",
            ),
            Self::Unauthorized => response::error(
                StatusCode::UNAUTHORIZED,
                "Invalid certificate deployment token",
            ),
            Self::Unavailable => response::error(
                StatusCode::NOT_FOUND,
                "Certificate deployment binding is unavailable",
            ),
            Self::Conflict(message) => response::error(StatusCode::CONFLICT, message),
            Self::Storage(message) => {
                tracing::warn!(%message, "external certificate deployment storage failure");
                response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Certificate deployment storage operation failed",
                )
            }
            Self::Gateway(message) => {
                tracing::warn!(%message, "external certificate deployment gateway failure");
                response::error(
                    StatusCode::BAD_GATEWAY,
                    "Certificate deployment failed while synchronizing the gateway",
                )
            }
            Self::GatewayRestored(message) => {
                tracing::warn!(%message, "external certificate deployment failed and was rolled back");
                response::error(
                    StatusCode::BAD_GATEWAY,
                    "Certificate deployment failed and the previous configuration was restored",
                )
            }
            Self::GatewayRollbackFailed(message) => {
                tracing::error!(%message, "external certificate deployment rollback failed");
                response::error(
                    StatusCode::BAD_GATEWAY,
                    "Certificate deployment failed and fn-knock could not confirm restoration of the previous configuration",
                )
            }
        }
    }
}

fn credential_response(data: ExternalCertificateBindingCredentialData) -> Response {
    let mut response = response::ok(data).into_response();
    crate::http_utils::apply_no_store_headers(response.headers_mut());
    response
}

pub(crate) fn external_certificate_routes() -> Router<AppState> {
    external_certificate_openapi_routes().into()
}

pub(crate) fn public_external_certificate_routes() -> Router<AppState> {
    public_external_certificate_openapi_routes().into()
}

pub(crate) fn external_certificate_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(
        routes!(deploy_external_certificate)
            .layer(DefaultBodyLimit::max(EXTERNAL_CERTIFICATE_BODY_LIMIT)),
    )
}

pub(crate) fn public_external_certificate_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(
        routes!(deploy_public_external_certificate)
            .layer(DefaultBodyLimit::max(EXTERNAL_CERTIFICATE_BODY_LIMIT)),
    )
}

async fn external_certificate_deployment_endpoints(
    state: &AppState,
) -> ExternalCertificateDeploymentEndpoints {
    let Ok(config) = state.storage.store.get_config().await else {
        return ExternalCertificateDeploymentEndpoints {
            public_deploy_base_url: None,
            public_deploy_status: "auth_host_unconfigured".to_string(),
            lan_deploy_base_urls: Vec::new(),
            lan_deploy_status: "gateway_unavailable".to_string(),
        };
    };
    let lan = normalize_lan_deployment(config.get(SSL_LAN_DEPLOYMENT_KEY));
    let lan_enabled = lan.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let mut lan_status = if lan_enabled {
        "ready".to_string()
    } else {
        "disabled".to_string()
    };
    if lan_enabled && !default_ssl_available(&config) {
        lan_status = "ssl_unavailable".to_string();
    } else if lan_enabled {
        match state.gateway.client.get_gateway_listener_scope().await {
            Ok(scope) if scope == "loopback" => lan_status = "listener_loopback".to_string(),
            Ok(_) => {}
            Err(_) => lan_status = "gateway_unavailable".to_string(),
        }
    }
    let lan_deploy_base_urls = if lan_status == "ready" {
        lan.get("addresses")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(Value::as_str)
            .map(|address| format!("https://{address}:{}", super::lan::gateway_port()))
            .collect()
    } else {
        Vec::new()
    };
    let public_unavailable = |status: &str| ExternalCertificateDeploymentEndpoints {
        public_deploy_base_url: None,
        public_deploy_status: status.to_string(),
        lan_deploy_base_urls: lan_deploy_base_urls.clone(),
        lan_deploy_status: lan_status.clone(),
    };
    let auth = crate::proxy_config::build_gateway_auth_config(&config);
    let auth_host = auth
        .get("auth_host")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(auth_host) = auth_host else {
        return public_unavailable("auth_host_unconfigured");
    };
    let public_base_url = auth
        .get("public_auth_base_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let Some(public_base_url) = public_base_url else {
        return public_unavailable("auth_host_unconfigured");
    };
    let Ok(url) = url::Url::parse(public_base_url) else {
        return public_unavailable("https_required");
    };
    if url.scheme() != "https" {
        return public_unavailable("https_required");
    }
    let Ok(auth_authority) = auth_host.parse::<http::uri::Authority>() else {
        return public_unavailable("auth_host_unconfigured");
    };
    let mut public_url = url::Url::parse("https://certificate-deploy.invalid")
        .expect("static certificate deployment base URL must parse");
    if public_url.set_host(Some(auth_authority.host())).is_err()
        || public_url
            .set_port(url.port().or_else(|| auth_authority.port_u16()))
            .is_err()
    {
        return public_unavailable("auth_host_unconfigured");
    }
    ExternalCertificateDeploymentEndpoints {
        public_deploy_base_url: Some(public_url.as_str().trim_end_matches('/').to_string()),
        public_deploy_status: "ready".to_string(),
        lan_deploy_base_urls,
        lan_deploy_status: lan_status,
    }
}

fn binding_view(
    binding: &ExternalCertificateBinding,
    deploy_port: u16,
    endpoints: &ExternalCertificateDeploymentEndpoints,
) -> ExternalCertificateBindingData {
    let adapter = provider_adapter(&binding.provider).unwrap_or(&CERTD_PROVIDER_ADAPTER);
    let (
        setup_kind,
        request_method,
        request_body_template,
        success_marker,
        script_template,
        usage_instructions,
    ) = match adapter.setup {
        ExternalCertificateProviderSetup::Webhook {
            request_method,
            request_body_template,
            success_marker,
        } => (
            "webhook",
            Some(request_method.to_string()),
            Some(request_body_template.to_string()),
            Some(success_marker.to_string()),
            None,
            None,
        ),
        ExternalCertificateProviderSetup::DeployHook {
            script,
            usage_instructions,
        } => (
            "deploy_hook",
            None,
            None,
            None,
            Some(render_deploy_hook(script)),
            Some(usage_instructions.to_string()),
        ),
    };
    let public_deploy_url = endpoints.public_deploy_base_url.as_ref().map(|base| {
        format!(
            "{base}{PUBLIC_CERTIFICATE_DEPLOY_PATH_PREFIX}/{}",
            binding.id
        )
    });
    let lan_deploy_urls = endpoints
        .lan_deploy_base_urls
        .iter()
        .map(|base| {
            format!(
                "{base}{PUBLIC_CERTIFICATE_DEPLOY_PATH_PREFIX}/{}",
                binding.id
            )
        })
        .collect();
    ExternalCertificateBindingData {
        id: binding.id.clone(),
        name: binding.name.clone(),
        provider: adapter.id.to_string(),
        certificate_id: binding.certificate_id.clone(),
        enabled: binding.enabled,
        created_at: binding.created_at.clone(),
        updated_at: binding.updated_at.clone(),
        last_deployed_at: binding.last_deployed_at.clone(),
        last_result: binding.last_result.clone(),
        last_error: binding.last_error.clone(),
        last_fingerprint_sha256: binding.last_fingerprint_sha256.clone(),
        last_valid_to: binding.last_valid_to.clone(),
        last_dns_names: binding.last_dns_names.clone(),
        deploy_path: format!("/api/integrations/certificates/{}", binding.id),
        deploy_port,
        public_deploy_url,
        public_deploy_status: endpoints.public_deploy_status.clone(),
        lan_deploy_urls,
        lan_deploy_status: endpoints.lan_deploy_status.clone(),
        setup_kind: setup_kind.to_string(),
        request_method,
        request_body_template,
        success_marker,
        script_template,
        usage_instructions,
        last_replaced_certificate_count: binding.last_replaced_certificate_count,
        last_replaced_sources: binding.last_replaced_sources.clone(),
        last_disabled_external_binding_count: binding.last_disabled_external_binding_count,
        last_disabled_acme_renewal_count: binding.last_disabled_acme_renewal_count,
        last_takeover_at: binding.last_takeover_at.clone(),
    }
}

fn provider_adapter(provider: &str) -> Option<&'static ExternalCertificateProviderAdapter> {
    // Provider-specific presentation belongs in this registry. Validation,
    // storage, authorization and deployment transactions remain generic.
    EXTERNAL_CERTIFICATE_PROVIDER_ADAPTERS
        .iter()
        .find(|adapter| adapter.id == provider)
}

async fn load_bindings(state: &AppState) -> anyhow::Result<Vec<ExternalCertificateBinding>> {
    let value = state
        .storage
        .store
        .get_json_value(EXTERNAL_CERTIFICATE_BINDINGS_KEY)
        .await?;
    let bindings = match value {
        None => Vec::new(),
        Some(value) => serde_json::from_value(value)?,
    };
    validate_stored_bindings(&bindings)?;
    Ok(bindings)
}

fn validate_stored_bindings(bindings: &[ExternalCertificateBinding]) -> anyhow::Result<()> {
    let mut ids = BTreeSet::new();
    let mut certificate_ids = BTreeSet::new();
    for binding in bindings {
        if binding.id.trim().is_empty()
            || !ids.insert(binding.id.as_str())
            || binding.certificate_id.trim().is_empty()
            || !certificate_ids.insert(binding.certificate_id.as_str())
        {
            return Err(anyhow!(
                "external certificate binding identifiers are invalid or duplicated"
            ));
        }
        if provider_adapter(&binding.provider).is_none() {
            return Err(anyhow!(
                "external certificate binding provider is unsupported"
            ));
        }
        if binding.token_hash.len() != 64
            || !binding
                .token_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(anyhow!(
                "external certificate binding token hash is invalid"
            ));
        }
    }
    Ok(())
}

async fn save_bindings(
    state: &AppState,
    bindings: &[ExternalCertificateBinding],
) -> anyhow::Result<()> {
    state
        .storage
        .store
        .set_json_value(
            EXTERNAL_CERTIFICATE_BINDINGS_KEY,
            &serde_json::to_value(bindings)?,
        )
        .await?;
    Ok(())
}

fn normalize_binding_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err("Binding name is required".to_string());
    }
    if value.chars().count() > MAX_BINDING_NAME_LENGTH {
        return Err(format!(
            "Binding name must not exceed {MAX_BINDING_NAME_LENGTH} characters"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err("Binding name must not contain control characters".to_string());
    }
    Ok(value.to_string())
}

fn normalize_provider(value: Option<&str>) -> Result<String, String> {
    let value = value.unwrap_or("certd").trim().to_ascii_lowercase();
    provider_adapter(&value)
        .map(|adapter| adapter.id.to_string())
        .ok_or_else(|| "Unsupported external certificate provider".to_string())
}

fn new_deployment_token() -> String {
    format!("{TOKEN_PREFIX}{}", hex::encode(rand::random::<[u8; 32]>()))
}

fn deployment_token_hash(token: &str) -> String {
    crate::crypto_utils::sha256_hex_str(token)
}

fn deployment_token_matches(expected_hash: &str, supplied_token: &str) -> bool {
    let supplied_hash = deployment_token_hash(supplied_token);
    expected_hash.len() == supplied_hash.len()
        && bool::from(expected_hash.as_bytes().ct_eq(supplied_hash.as_bytes()))
}

fn authorized_binding_index(
    bindings: &[ExternalCertificateBinding],
    binding_id: &str,
    supplied_token: &str,
) -> Result<usize, ExternalDeployError> {
    let Some(index) = bindings.iter().position(|binding| binding.id == binding_id) else {
        return Err(ExternalDeployError::Unavailable);
    };
    if !bindings[index].enabled {
        return Err(ExternalDeployError::Unavailable);
    }
    if !deployment_token_matches(&bindings[index].token_hash, supplied_token) {
        return Err(ExternalDeployError::Unauthorized);
    }
    Ok(index)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().split_once(' '))
        .filter(|(scheme, _)| scheme.eq_ignore_ascii_case("bearer"))
        .map(|(_, token)| token)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

pub(super) fn normalized_request_host(headers: &HeaderMap) -> Option<String> {
    let authority = headers.get(HOST)?.to_str().ok()?.trim();
    let authority = authority.parse::<http::uri::Authority>().ok()?;
    let host = authority
        .host()
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

fn certificate_metadata(cert: &str) -> Result<CertificateMetadata, String> {
    let mut remaining = cert.as_bytes();
    let mut leaf = None;
    let mut chain = Vec::new();
    while !remaining.is_empty() {
        let (rest, pem) = parse_x509_pem(remaining).map_err(|_| {
            if chain.is_empty() {
                "Certificate format is invalid".to_string()
            } else {
                "Certificate chain contains invalid trailing content".to_string()
            }
        })?;
        let parsed = pem
            .parse_x509()
            .map_err(|_| "Certificate chain contains an invalid X.509 certificate".to_string())?;
        let now = time_utils::now_ms();
        let not_before_ms = parsed
            .validity()
            .not_before
            .timestamp()
            .saturating_mul(1000);
        let not_after_ms = parsed.validity().not_after.timestamp().saturating_mul(1000);
        if not_before_ms > now.saturating_add(5 * 60 * 1000) {
            return Err(
                "Certificate chain contains a certificate that is not valid yet".to_string(),
            );
        }
        if not_after_ms <= now {
            return Err("Certificate chain contains an expired certificate".to_string());
        }
        if leaf.is_none() {
            leaf = Some((pem.contents.clone(), not_after_ms));
        }
        chain.push(pem.contents);
        remaining = rest;
        if remaining.iter().all(u8::is_ascii_whitespace) {
            break;
        }
    }
    let Some((leaf_der, not_after_ms)) = leaf else {
        return Err("Certificate chain is empty".to_string());
    };
    validate_certificate_chain(&chain)?;
    let info = parse_cert_info(cert)
        .ok_or_else(|| "Certificate metadata could not be parsed".to_string())?;
    let dns_names = certificate_info_dns_names(&info);
    if dns_names.is_empty() {
        return Err(
            "Certificate leaf must contain at least one DNS/IP SAN or a common name".to_string(),
        );
    }
    Ok(CertificateMetadata {
        fingerprint_sha256: crate::crypto_utils::sha256_hex_bytes(&leaf_der),
        valid_to: time_utils::iso_from_ms(not_after_ms),
        not_after_ms,
        dns_names,
    })
}

fn validate_certificate_chain(chain: &[Vec<u8>]) -> Result<(), String> {
    if chain.len() == 1 {
        let (_, certificate) = parse_x509_certificate(&chain[0])
            .map_err(|_| "Certificate chain contains an invalid X.509 certificate".to_string())?;
        if certificate.subject() != certificate.issuer() {
            return Err("Certificate chain is incomplete".to_string());
        }
        certificate
            .verify_signature(None)
            .map_err(|_| "Self-signed certificate signature is invalid".to_string())?;
        return Ok(());
    }

    for pair in chain.windows(2) {
        let (_, certificate) = parse_x509_certificate(&pair[0])
            .map_err(|_| "Certificate chain contains an invalid X.509 certificate".to_string())?;
        let (_, issuer) = parse_x509_certificate(&pair[1])
            .map_err(|_| "Certificate chain contains an invalid X.509 certificate".to_string())?;
        if certificate.issuer() != issuer.subject() {
            return Err("Certificate chain is incomplete or out of order".to_string());
        }
        let is_ca = issuer
            .basic_constraints()
            .map_err(|_| "Certificate chain contains invalid basic constraints".to_string())?
            .is_some_and(|constraints| constraints.value.ca);
        if !is_ca {
            return Err("Certificate chain issuer is not a certificate authority".to_string());
        }
        if issuer
            .key_usage()
            .map_err(|_| "Certificate chain contains invalid key usage".to_string())?
            .is_some_and(|usage| !usage.value.key_cert_sign())
        {
            return Err("Certificate chain issuer cannot sign certificates".to_string());
        }
        certificate
            .verify_signature(Some(issuer.public_key()))
            .map_err(|_| "Certificate chain signature verification failed".to_string())?;
    }
    Ok(())
}

fn validate_external_certificate(
    body: &ExternalCertificateDeployBody,
) -> Result<(String, String, CertificateMetadata), ExternalDeployError> {
    let cert = body.cert.trim().to_string();
    let key = body.key.trim().to_string();
    if cert.is_empty() || key.is_empty() {
        return Err(ExternalDeployError::BadRequest(
            "Certificate and private key are required".to_string(),
        ));
    }
    if cert.len().saturating_add(key.len()) > EXTERNAL_CERTIFICATE_BODY_LIMIT {
        return Err(ExternalDeployError::PayloadTooLarge);
    }
    validate_ssl_cert(&cert, &key)
        .map_err(|error| ExternalDeployError::BadRequest(error.to_string()))?;
    let metadata = certificate_metadata(&cert).map_err(ExternalDeployError::BadRequest)?;
    Ok((cert, key, metadata))
}

fn normalized_domain_set(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| crate::certificates::domain_utils::normalize_domain_name(value))
        .filter(|value| !value.is_empty())
        .collect()
}

fn stored_certificate_metadata(certificate: &Value) -> Option<CertificateMetadata> {
    certificate
        .get("cert")
        .and_then(Value::as_str)
        .and_then(|cert| certificate_metadata(cert).ok())
}

fn prepare_external_certificate_update(
    config: &Value,
    binding: &ExternalCertificateBinding,
    cert: &str,
    key: &str,
    metadata: &CertificateMetadata,
) -> Result<(Value, bool, bool, bool, ExternalCertificateTakeover), ExternalDeployError> {
    let ssl = normalize_ssl_config(config.get("ssl"));
    let certificates = ssl
        .get("certificates")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let existing = certificates
        .iter()
        .find(|item| {
            item.get("id").and_then(Value::as_str) == Some(binding.certificate_id.as_str())
        })
        .cloned();
    let incoming_domains = normalized_domain_set(&metadata.dns_names);
    let mut takeover = ExternalCertificateTakeover::default();
    for candidate in &certificates {
        let candidate_id = candidate.get("id").and_then(Value::as_str).unwrap_or("");
        if candidate_id.is_empty() || candidate_id == binding.certificate_id {
            continue;
        }
        let Some(candidate_metadata) = stored_certificate_metadata(candidate) else {
            continue;
        };
        let candidate_domains = normalized_domain_set(&candidate_metadata.dns_names);
        if candidate_domains.is_disjoint(&incoming_domains) {
            continue;
        }
        if candidate_domains != incoming_domains {
            return Err(ExternalDeployError::Conflict(
                "Incoming certificate partially overlaps an existing certificate; resolve the overlapping SANs in the certificate library before retrying"
                    .to_string(),
            ));
        }
        if metadata.not_after_ms < candidate_metadata.not_after_ms {
            return Err(ExternalDeployError::Conflict(format!(
                "Incoming certificate expires before an existing same-SAN certificate ({})",
                candidate_metadata.valid_to
            )));
        }
        takeover
            .replaced_certificate_ids
            .insert(candidate_id.to_string());
        let source = normalize_certificate_source(candidate.get("source").and_then(Value::as_str));
        takeover.replaced_sources.insert(source.to_string());
        if source == "acme"
            && let Some(application_id) = candidate
                .get("source_ref_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
        {
            takeover
                .acme_application_ids
                .insert(application_id.to_string());
        }
    }
    let active_id = ssl
        .get("active_cert_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let is_active = active_id == binding.certificate_id
        || active_id.is_empty()
        || takeover.replaced_certificate_ids.contains(&active_id);

    if let Some(existing) = existing.as_ref() {
        let existing_cert = existing.get("cert").and_then(Value::as_str).unwrap_or("");
        let existing_key = existing.get("key").and_then(Value::as_str).unwrap_or("");
        if existing_cert.trim() == cert
            && existing_key.trim() == key
            && takeover.replaced_certificate_ids.is_empty()
        {
            return Ok((
                ssl,
                false,
                false,
                active_id == binding.certificate_id,
                ExternalCertificateTakeover::default(),
            ));
        }
        if let Ok(existing_metadata) = certificate_metadata(existing_cert)
            && metadata.not_after_ms < existing_metadata.not_after_ms
        {
            return Err(ExternalDeployError::Conflict(format!(
                "Incoming certificate expires before the currently stored certificate ({})",
                existing_metadata.valid_to
            )));
        }
    }

    let now = now_node_iso();
    let created_at = existing
        .as_ref()
        .and_then(|item| item.get("created_at"))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .unwrap_or(&now)
        .to_string();
    let mut certificate = Map::new();
    certificate.insert("id".to_string(), json!(binding.certificate_id));
    certificate.insert("label".to_string(), json!(binding.name));
    certificate.insert("source".to_string(), json!("external"));
    certificate.insert("source_provider".to_string(), json!(binding.provider));
    certificate.insert("source_ref_id".to_string(), json!(binding.id));
    if let Some(primary_domain) = metadata.dns_names.first() {
        certificate.insert("primary_domain".to_string(), json!(primary_domain));
    }
    certificate.insert("cert".to_string(), json!(cert));
    certificate.insert("key".to_string(), json!(key));
    certificate.insert("created_at".to_string(), json!(created_at));
    certificate.insert("updated_at".to_string(), json!(now));

    let mut next_certificates = certificates
        .into_iter()
        .filter(|item| {
            let id = item.get("id").and_then(Value::as_str).unwrap_or("");
            id != binding.certificate_id && !takeover.replaced_certificate_ids.contains(id)
        })
        .collect::<Vec<_>>();
    next_certificates.insert(0, Value::Object(certificate));
    let mut next_ssl = ssl;
    next_ssl["certificates"] = Value::Array(next_certificates);
    if is_active {
        next_ssl["cert"] = json!(cert);
        next_ssl["key"] = json!(key);
        next_ssl = mirror_active_ssl_certificate(&next_ssl, Some(&binding.certificate_id));
    }
    let should_sync_gateway =
        is_active || next_ssl.get("deployment_mode").and_then(Value::as_str) == Some("multi_sni");
    Ok((next_ssl, true, should_sync_gateway, is_active, takeover))
}

async fn prepare_and_store_external_certificate(
    state: &AppState,
    binding: &ExternalCertificateBinding,
    cert: &str,
    key: &str,
    metadata: &CertificateMetadata,
) -> Result<PreparedExternalCertificateUpdate, ExternalDeployError> {
    for _ in 0..8 {
        let config = state
            .storage
            .store
            .get_config()
            .await
            .map_err(|error| ExternalDeployError::Storage(error.to_string()))?;
        let previous_ssl = config.get("ssl").cloned();
        let (next_ssl, changed, should_sync_gateway, is_active, takeover) =
            prepare_external_certificate_update(&config, binding, cert, key, metadata)?;
        if !changed {
            return Ok(PreparedExternalCertificateUpdate {
                previous_ssl,
                next_config: config,
                changed,
                should_sync_gateway,
                is_active,
                takeover,
            });
        }
        let stored = state
            .storage
            .store
            .compare_and_set_ssl_config(previous_ssl.as_ref(), Some(&next_ssl))
            .await
            .map_err(|error| ExternalDeployError::Storage(error.to_string()))?;
        if let Some(next_config) = stored {
            crate::fnos_certificate_sync::notify_certificate_library_changed(state);
            return Ok(PreparedExternalCertificateUpdate {
                previous_ssl,
                next_config,
                changed,
                should_sync_gateway,
                is_active,
                takeover,
            });
        }
    }
    Err(ExternalDeployError::Conflict(
        "SSL configuration changed too frequently; retry deployment".to_string(),
    ))
}

async fn rollback_external_certificate(
    state: &AppState,
    prepared: &PreparedExternalCertificateUpdate,
    reapply_gateway: bool,
) -> Result<(), ExternalDeployError> {
    let restored = state
        .storage
        .store
        .compare_and_set_ssl_config(
            prepared.next_config.get("ssl"),
            prepared.previous_ssl.as_ref(),
        )
        .await
        .map_err(|error| ExternalDeployError::Storage(error.to_string()))?
        .ok_or_else(|| {
            ExternalDeployError::Storage(
                "SSL configuration changed before the previous certificate could be restored"
                    .to_string(),
            )
        })?;
    crate::fnos_certificate_sync::notify_certificate_library_changed(state);
    if reapply_gateway {
        sync_ssl_deployment_to_gateway(state, Some(&restored))
            .await
            .map_err(|error| ExternalDeployError::Gateway(error.to_string()))?;
    }
    Ok(())
}

fn binding_has_success_metadata(
    binding: &ExternalCertificateBinding,
    metadata: &CertificateMetadata,
) -> bool {
    binding.last_result.as_deref() == Some("success")
        && binding.last_fingerprint_sha256.as_deref() == Some(metadata.fingerprint_sha256.as_str())
        && binding.last_valid_to.as_deref() == Some(metadata.valid_to.as_str())
        && binding.last_dns_names == metadata.dns_names
}

fn truncate_binding_error(error: &str) -> String {
    error.chars().take(MAX_BINDING_ERROR_LENGTH).collect()
}

async fn save_binding_deployment_result(
    state: &AppState,
    bindings: &mut [ExternalCertificateBinding],
    binding_index: usize,
    metadata: Option<&CertificateMetadata>,
    error: Option<&str>,
    takeover: Option<(&ExternalCertificateTakeover, usize, usize)>,
) -> Result<(), ExternalDeployError> {
    let now = time_utils::now_iso();
    let binding = &mut bindings[binding_index];
    binding.updated_at = now.clone();
    binding.last_deployed_at = Some(now.clone());
    if let Some(error) = error {
        binding.last_result = Some("failed".to_string());
        binding.last_error = Some(truncate_binding_error(error));
    } else if let Some(metadata) = metadata {
        binding.last_result = Some("success".to_string());
        binding.last_error = None;
        binding.last_fingerprint_sha256 = Some(metadata.fingerprint_sha256.clone());
        binding.last_valid_to = Some(metadata.valid_to.clone());
        binding.last_dns_names = metadata.dns_names.clone();
        if let Some((takeover, disabled_external_bindings, disabled_acme_renewals)) = takeover {
            binding.last_replaced_certificate_count = takeover.replaced_certificate_ids.len();
            binding.last_replaced_sources = takeover.replaced_sources.iter().cloned().collect();
            binding.last_disabled_external_binding_count = disabled_external_bindings;
            binding.last_disabled_acme_renewal_count = disabled_acme_renewals;
            binding.last_takeover_at = if takeover.replaced_certificate_ids.is_empty() {
                None
            } else {
                Some(now.clone())
            };
        }
    }
    save_bindings(state, bindings)
        .await
        .map_err(|error| ExternalDeployError::Storage(error.to_string()))
}

async fn record_binding_deployment_failure(
    state: &AppState,
    bindings: &mut [ExternalCertificateBinding],
    binding_index: usize,
    stage: &'static str,
    detail: &str,
) {
    let binding_id = bindings[binding_index].id.clone();
    let certificate_id = bindings[binding_index].certificate_id.clone();
    let provider = bindings[binding_index].provider.clone();
    let detail = truncate_binding_error(detail);
    tracing::warn!(
        %binding_id,
        %certificate_id,
        %provider,
        %stage,
        error = %detail,
        "external certificate deployment failed"
    );
    if let Err(error) =
        save_binding_deployment_result(state, bindings, binding_index, None, Some(&detail), None)
            .await
    {
        tracing::warn!(
            %binding_id,
            %certificate_id,
            %provider,
            %stage,
            persistence_error = ?error,
            "failed to persist external certificate deployment audit status"
        );
    }
}

fn disable_superseded_external_bindings(
    bindings: &mut [ExternalCertificateBinding],
    current_binding_index: usize,
    takeover: &ExternalCertificateTakeover,
    current_binding_id: &str,
) -> usize {
    let now = time_utils::now_iso();
    let mut disabled = 0;
    for (index, binding) in bindings.iter_mut().enumerate() {
        if index == current_binding_index
            || !binding.enabled
            || !takeover
                .replaced_certificate_ids
                .contains(&binding.certificate_id)
        {
            continue;
        }
        binding.enabled = false;
        binding.updated_at = now.clone();
        binding.last_result = Some("superseded".to_string());
        binding.last_error = Some(truncate_binding_error(&format!(
            "Certificate ownership was transferred to external binding {current_binding_id}"
        )));
        disabled += 1;
    }
    disabled
}

async fn rollback_external_takeover(
    state: &AppState,
    prepared: &PreparedExternalCertificateUpdate,
    previous_bindings: &[ExternalCertificateBinding],
    acme_snapshot: Option<&crate::acme::ExternalCertificateTakeoverSnapshot>,
    reapply_gateway: bool,
) -> Result<(), String> {
    let mut failures = Vec::new();
    if let Some(snapshot) = acme_snapshot
        && let Err(error) =
            crate::acme::restore_external_certificate_takeover(state, snapshot).await
    {
        failures.push(format!("ACME state: {error}"));
    }
    if let Err(error) = save_bindings(state, previous_bindings).await {
        failures.push(format!("external bindings: {error}"));
    }
    if prepared.changed
        && let Err(error) = rollback_external_certificate(state, prepared, reapply_gateway).await
    {
        failures.push(format!("SSL/gateway: {error:?}"));
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures.join("; "))
    }
}

#[utoipa::path(
    get,
    path = "/api/admin/ssl/external-bindings",
    tag = "ssl",
    operation_id = "get_api_admin_ssl_external_bindings",
    responses((status = 200, description = "External certificate deployment bindings"))
)]
pub(super) async fn list_external_certificate_bindings(State(state): State<AppState>) -> Response {
    let endpoints = external_certificate_deployment_endpoints(&state).await;
    match load_bindings(&state).await {
        Ok(bindings) => response::ok(
            bindings
                .iter()
                .map(|binding| binding_view(binding, state.settings.backend_port, &endpoints))
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()),
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/ssl/external-bindings",
    tag = "ssl",
    operation_id = "post_api_admin_ssl_external_bindings",
    request_body = ExternalCertificateBindingCreateBody,
    responses((status = 200, description = "Created external certificate deployment binding"))
)]
pub(super) async fn create_external_certificate_binding(
    State(state): State<AppState>,
    Json(body): Json<ExternalCertificateBindingCreateBody>,
) -> Response {
    let name = match normalize_binding_name(&body.name) {
        Ok(name) => name,
        Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
    };
    let provider = match normalize_provider(body.provider.as_deref()) {
        Ok(provider) => provider,
        Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
    };
    let _guard = state.gateway.ssl_update_lock.lock().await;
    let mut bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => {
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let id = Uuid::new_v4().simple().to_string();
    let token = new_deployment_token();
    let now = time_utils::now_iso();
    let binding = ExternalCertificateBinding {
        id: id.clone(),
        name,
        provider,
        certificate_id: format!("external_{id}"),
        token_hash: deployment_token_hash(&token),
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
        last_deployed_at: None,
        last_result: None,
        last_error: None,
        last_fingerprint_sha256: None,
        last_valid_to: None,
        last_dns_names: Vec::new(),
        last_replaced_certificate_count: 0,
        last_replaced_sources: Vec::new(),
        last_disabled_external_binding_count: 0,
        last_disabled_acme_renewal_count: 0,
        last_takeover_at: None,
    };
    bindings.push(binding.clone());
    if let Err(error) = save_bindings(&state, &bindings).await {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let endpoints = external_certificate_deployment_endpoints(&state).await;
    credential_response(ExternalCertificateBindingCredentialData {
        binding: binding_view(&binding, state.settings.backend_port, &endpoints),
        token,
    })
}

#[utoipa::path(
    patch,
    path = "/api/admin/ssl/external-bindings/{id}",
    tag = "ssl",
    operation_id = "patch_api_admin_ssl_external_bindings_by_id",
    request_body = ExternalCertificateBindingUpdateBody,
    params(("id" = String, Path, description = "External certificate binding identifier")),
    responses((status = 200, description = "Updated external certificate deployment binding"))
)]
pub(super) async fn update_external_certificate_binding(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<ExternalCertificateBindingUpdateBody>,
) -> Response {
    let normalized_name = match body.name.as_deref().map(normalize_binding_name).transpose() {
        Ok(name) => name,
        Err(error) => return response::error(StatusCode::BAD_REQUEST, error),
    };
    let _guard = state.gateway.ssl_update_lock.lock().await;
    let mut bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => {
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let Some(binding) = bindings.iter_mut().find(|binding| binding.id == id) else {
        return response::error(StatusCode::NOT_FOUND, "Binding not found");
    };
    if let Some(name) = normalized_name {
        binding.name = name;
    }
    if let Some(enabled) = body.enabled {
        binding.enabled = enabled;
    }
    binding.updated_at = time_utils::now_iso();
    let binding = binding.clone();
    if let Err(error) = save_bindings(&state, &bindings).await {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let endpoints = external_certificate_deployment_endpoints(&state).await;
    let view = binding_view(&binding, state.settings.backend_port, &endpoints);
    response::ok(view).into_response()
}

#[utoipa::path(
    post,
    path = "/api/admin/ssl/external-bindings/{id}/rotate-token",
    tag = "ssl",
    operation_id = "post_api_admin_ssl_external_bindings_by_id_rotate_token",
    params(("id" = String, Path, description = "External certificate binding identifier")),
    responses((status = 200, description = "Rotated external certificate deployment token"))
)]
pub(super) async fn rotate_external_certificate_binding_token(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let _guard = state.gateway.ssl_update_lock.lock().await;
    let mut bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => {
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let Some(binding) = bindings.iter_mut().find(|binding| binding.id == id) else {
        return response::error(StatusCode::NOT_FOUND, "Binding not found");
    };
    let token = new_deployment_token();
    binding.token_hash = deployment_token_hash(&token);
    binding.updated_at = time_utils::now_iso();
    let binding = binding.clone();
    if let Err(error) = save_bindings(&state, &bindings).await {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    let endpoints = external_certificate_deployment_endpoints(&state).await;
    let view = binding_view(&binding, state.settings.backend_port, &endpoints);
    credential_response(ExternalCertificateBindingCredentialData {
        binding: view,
        token,
    })
}

#[utoipa::path(
    delete,
    path = "/api/admin/ssl/external-bindings/{id}",
    tag = "ssl",
    operation_id = "delete_api_admin_ssl_external_bindings_by_id",
    params(("id" = String, Path, description = "External certificate binding identifier")),
    responses((status = 200, description = "Deleted external certificate deployment binding"))
)]
pub(super) async fn delete_external_certificate_binding(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let _guard = state.gateway.ssl_update_lock.lock().await;
    let mut bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => {
            return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
        }
    };
    let previous_len = bindings.len();
    bindings.retain(|binding| binding.id != id);
    if bindings.len() == previous_len {
        return response::error(StatusCode::NOT_FOUND, "Binding not found");
    }
    if let Err(error) = save_bindings(&state, &bindings).await {
        return response::error(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    }
    response::success_empty().into_response()
}

#[utoipa::path(
    put,
    path = "/api/integrations/certificates/{binding_id}",
    tag = "ssl",
    operation_id = "put_api_integrations_certificates_by_binding_id",
    request_body = ExternalCertificateDeployBody,
    params(("binding_id" = String, Path, description = "External certificate binding identifier")),
    responses((status = 200, description = "Deployed external certificate"))
)]
pub(super) async fn deploy_external_certificate(
    State(state): State<AppState>,
    AxumPath(binding_id): AxumPath<String>,
    request: Request,
) -> Response {
    deploy_external_certificate_inner(state, binding_id, request).await
}

#[utoipa::path(
    put,
    path = "/__certificates__/{binding_id}",
    tag = "ssl",
    operation_id = "put_public_certificates_by_binding_id",
    request_body = ExternalCertificateDeployBody,
    params(("binding_id" = String, Path, description = "External certificate binding identifier")),
    responses((status = 200, description = "Deployed external certificate through the authentication host"))
)]
async fn deploy_public_external_certificate(
    State(state): State<AppState>,
    AxumPath(binding_id): AxumPath<String>,
    request: Request,
) -> Response {
    let mut response = if !gateway_deploy_request_matches(&state, request.headers()).await {
        ExternalDeployError::Unavailable.into_response()
    } else {
        deploy_external_certificate_inner(state, binding_id, request).await
    };
    crate::http_utils::apply_no_store_headers(response.headers_mut());
    response
}

async fn gateway_deploy_request_matches(state: &AppState, headers: &HeaderMap) -> bool {
    let Ok(config) = state.storage.store.get_config().await else {
        return false;
    };
    let public_matches = normalized_request_host(headers).is_some_and(|request_host| {
        if super::lan::configured_lan_host_matches(&config, &request_host) {
            return false;
        }
        let auth = crate::proxy_config::build_gateway_auth_config(&config);
        auth.get("auth_host")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(|host| host.trim_end_matches('.').to_ascii_lowercase())
            .filter(|host| !host.is_empty())
            .is_some_and(|host| host == request_host)
    });
    public_matches || lan_deploy_request_matches(&config, headers)
}

async fn deploy_external_certificate_inner(
    state: AppState,
    binding_id: String,
    request: Request,
) -> Response {
    let supplied_token = bearer_token(request.headers())
        .unwrap_or_default()
        .to_string();
    let initial_bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => return ExternalDeployError::Storage(error.to_string()).into_response(),
    };
    if let Err(error) = authorized_binding_index(&initial_bindings, &binding_id, &supplied_token) {
        return error.into_response();
    }
    let body = match to_bytes(request.into_body(), EXTERNAL_CERTIFICATE_BODY_LIMIT).await {
        Ok(body) => body,
        Err(_) => return ExternalDeployError::PayloadTooLarge.into_response(),
    };
    let body: ExternalCertificateDeployBody = match serde_json::from_slice(&body) {
        Ok(body) => body,
        Err(_) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                "Invalid certificate deployment request body",
            );
        }
    };
    let validated = validate_external_certificate(&body);

    let _guard = state.gateway.ssl_update_lock.lock().await;
    let mut bindings = match load_bindings(&state).await {
        Ok(bindings) => bindings,
        Err(error) => return ExternalDeployError::Storage(error.to_string()).into_response(),
    };
    let binding_index = match authorized_binding_index(&bindings, &binding_id, &supplied_token) {
        Ok(index) => index,
        Err(error) => return error.into_response(),
    };
    let binding = bindings[binding_index].clone();
    let (cert, key, metadata) = match validated {
        Ok(validated) => validated,
        Err(error) => {
            let detail = match &error {
                ExternalDeployError::BadRequest(message)
                | ExternalDeployError::Conflict(message) => message.as_str(),
                _ => "Certificate validation failed",
            };
            record_binding_deployment_failure(
                &state,
                &mut bindings,
                binding_index,
                "validation",
                detail,
            )
            .await;
            return error.into_response();
        }
    };
    let prepared = match prepare_and_store_external_certificate(
        &state, &binding, &cert, &key, &metadata,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            let detail = match &error {
                ExternalDeployError::BadRequest(message)
                | ExternalDeployError::Conflict(message)
                | ExternalDeployError::Storage(message) => message.as_str(),
                _ => "Certificate storage failed",
            };
            record_binding_deployment_failure(
                &state,
                &mut bindings,
                binding_index,
                "storage",
                detail,
            )
            .await;
            return error.into_response();
        }
    };
    let previous_bindings = bindings.clone();
    let disabled_external_binding_count = disable_superseded_external_bindings(
        &mut bindings,
        binding_index,
        &prepared.takeover,
        &binding.id,
    );
    let acme_snapshot = match crate::acme::apply_external_certificate_takeover(
        &state,
        &prepared.takeover.replaced_certificate_ids,
        &prepared.takeover.acme_application_ids,
    )
    .await
    {
        Ok(snapshot) => snapshot,
        Err(error) => {
            let detail = format!("failed to disable superseded ACME automation: {error}");
            let rollback =
                rollback_external_takeover(&state, &prepared, &previous_bindings, None, false)
                    .await;
            bindings = previous_bindings;
            record_binding_deployment_failure(
                &state,
                &mut bindings,
                binding_index,
                "storage",
                &detail,
            )
            .await;
            return match rollback {
                Ok(()) => ExternalDeployError::Storage(detail).into_response(),
                Err(rollback_error) => ExternalDeployError::GatewayRollbackFailed(format!(
                    "{detail}; rollback failed: {rollback_error}"
                ))
                .into_response(),
            };
        }
    };
    let disabled_acme_renewal_count = acme_snapshot
        .as_ref()
        .map(|snapshot| snapshot.disabled_renewal_count)
        .unwrap_or(0);
    // Revoke superseded binding credentials before applying the new gateway
    // configuration. This closes the window in which an old external binding
    // could still authenticate after its certificate has been removed.
    if disabled_external_binding_count > 0
        && let Err(error) = save_bindings(&state, &bindings).await
    {
        let detail = format!("failed to disable superseded external bindings: {error}");
        let rollback = rollback_external_takeover(
            &state,
            &prepared,
            &previous_bindings,
            acme_snapshot.as_ref(),
            false,
        )
        .await;
        bindings = previous_bindings;
        record_binding_deployment_failure(&state, &mut bindings, binding_index, "storage", &detail)
            .await;
        return match rollback {
            Ok(()) => ExternalDeployError::Storage(detail).into_response(),
            Err(rollback_error) => ExternalDeployError::GatewayRollbackFailed(format!(
                "{detail}; rollback failed: {rollback_error}"
            ))
            .into_response(),
        };
    }
    let mut gateway_applied = false;
    if prepared.changed && prepared.should_sync_gateway {
        if let Err(error) =
            sync_ssl_deployment_to_gateway(&state, Some(&prepared.next_config)).await
        {
            let deployment_error = error.to_string();
            let rollback = rollback_external_takeover(
                &state,
                &prepared,
                &previous_bindings,
                acme_snapshot.as_ref(),
                true,
            )
            .await;
            let (detail, response_error) = match rollback {
                Ok(()) => {
                    let detail = deployment_error;
                    let response_error = ExternalDeployError::GatewayRestored(detail.clone());
                    (detail, response_error)
                }
                Err(rollback_error) => {
                    let detail = format!("{deployment_error}; rollback failed: {rollback_error}");
                    let response_error = ExternalDeployError::GatewayRollbackFailed(detail.clone());
                    (detail, response_error)
                }
            };
            bindings = previous_bindings;
            record_binding_deployment_failure(
                &state,
                &mut bindings,
                binding_index,
                "gateway",
                &detail,
            )
            .await;
            return response_error.into_response();
        }
        gateway_applied = true;
    }
    // A normal Certd retry with byte-identical certificate material must not
    // rewrite either the SSL configuration or the binding status. If the
    // previous status write failed, the metadata mismatch deliberately heals
    // it on this retry.
    if (prepared.changed || !binding_has_success_metadata(&bindings[binding_index], &metadata))
        && let Err(error) = save_binding_deployment_result(
            &state,
            &mut bindings,
            binding_index,
            Some(&metadata),
            None,
            Some((
                &prepared.takeover,
                disabled_external_binding_count,
                disabled_acme_renewal_count,
            )),
        )
        .await
    {
        let detail = format!("certificate deployment audit state failed: {error:?}");
        let rollback = rollback_external_takeover(
            &state,
            &prepared,
            &previous_bindings,
            acme_snapshot.as_ref(),
            gateway_applied,
        )
        .await;
        return match rollback {
            Ok(()) => error.into_response(),
            Err(rollback_error) => ExternalDeployError::GatewayRollbackFailed(format!(
                "{detail}; rollback failed: {rollback_error}"
            ))
            .into_response(),
        };
    }
    if prepared.changed {
        crate::panel_sync::notify_source_changed(&state);
    }
    tracing::info!(
        binding_id = %binding.id,
        certificate_id = %binding.certificate_id,
        provider = %binding.provider,
        changed = prepared.changed,
        gateway_applied,
        fingerprint_sha256 = %metadata.fingerprint_sha256,
        valid_to = %metadata.valid_to,
        replaced_certificate_count = prepared.takeover.replaced_certificate_ids.len(),
        disabled_external_binding_count,
        disabled_acme_renewal_count,
        "external certificate deployment completed"
    );
    response::ok(ExternalCertificateDeployData {
        binding_id: binding.id,
        certificate_id: binding.certificate_id,
        changed: prepared.changed,
        gateway_applied,
        is_active: prepared.is_active,
        fingerprint_sha256: metadata.fingerprint_sha256,
        valid_to: metadata.valid_to,
        dns_names: metadata.dns_names,
        replaced_certificate_count: prepared.takeover.replaced_certificate_ids.len(),
        replaced_sources: prepared.takeover.replaced_sources.into_iter().collect(),
        disabled_external_binding_count,
        disabled_acme_renewal_count,
    })
    .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use http_body_util::BodyExt;
    use rcgen::{
        BasicConstraints, CertificateParams, CertifiedIssuer, DistinguishedName, DnType, IsCa,
        KeyPair, KeyUsagePurpose, generate_simple_self_signed,
    };

    fn test_binding() -> ExternalCertificateBinding {
        ExternalCertificateBinding {
            id: "binding-1".to_string(),
            name: "Certd example.com".to_string(),
            provider: "certd".to_string(),
            certificate_id: "external_binding-1".to_string(),
            token_hash: "hash".to_string(),
            enabled: true,
            created_at: "2026-08-16T00:00:00Z".to_string(),
            updated_at: "2026-08-16T00:00:00Z".to_string(),
            last_deployed_at: None,
            last_result: None,
            last_error: None,
            last_fingerprint_sha256: None,
            last_valid_to: None,
            last_dns_names: Vec::new(),
            last_replaced_certificate_count: 0,
            last_replaced_sources: Vec::new(),
            last_disabled_external_binding_count: 0,
            last_disabled_acme_renewal_count: 0,
            last_takeover_at: None,
        }
    }

    fn test_endpoints() -> ExternalCertificateDeploymentEndpoints {
        ExternalCertificateDeploymentEndpoints {
            public_deploy_base_url: Some("https://auth.example.com".to_string()),
            public_deploy_status: "ready".to_string(),
            lan_deploy_base_urls: vec!["https://192.168.31.98:7999".to_string()],
            lan_deploy_status: "ready".to_string(),
        }
    }

    fn generated_certificate(domains: &[&str]) -> (String, String) {
        let generated = generate_simple_self_signed(
            domains
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap();
        (
            generated.cert.pem().trim().to_string(),
            generated.signing_key.serialize_pem().trim().to_string(),
        )
    }

    fn generated_unnamed_certificate() -> (String, String) {
        let key = KeyPair::generate().unwrap();
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name = DistinguishedName::new();
        let certificate = params.self_signed(&key).unwrap();
        (certificate.pem().trim().to_string(), key.serialize_pem())
    }

    fn test_ca() -> CertifiedIssuer<'static, KeyPair> {
        let mut distinguished_name = DistinguishedName::new();
        distinguished_name.push(DnType::CommonName, "Shared Test CA");
        let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
        params.distinguished_name = distinguished_name;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        CertifiedIssuer::self_signed(params, KeyPair::generate().unwrap()).unwrap()
    }

    fn generated_signed_certificate(issuer: &CertifiedIssuer<'_, KeyPair>) -> (String, String) {
        let key = KeyPair::generate().unwrap();
        let params = CertificateParams::new(vec!["example.com".to_string()]).unwrap();
        let cert = params.signed_by(&key, issuer).unwrap();
        (cert.pem().trim().to_string(), key.serialize_pem())
    }

    async fn test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.backend_port = 18_080;
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
        settings.internal_rpc_token = "external-certificate-test".to_string();
        settings.request_timeout = std::time::Duration::from_millis(100);
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    async fn response_json(response: Response) -> Value {
        let body = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    fn deployment_request(token: &str, cert: &str, key: &str) -> Request {
        Request::builder()
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                serde_json::to_vec(&json!({ "cert": cert, "key": key })).unwrap(),
            ))
            .unwrap()
    }

    fn pending_deployment_request(
        token: &str,
    ) -> (
        Request,
        tokio::sync::mpsc::Sender<Result<Bytes, std::io::Error>>,
    ) {
        let (sender, receiver) = tokio::sync::mpsc::channel(1);
        let request = Request::builder()
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::from_stream(
                tokio_stream::wrappers::ReceiverStream::new(receiver),
            ))
            .unwrap();
        (request, sender)
    }

    #[test]
    fn deployment_tokens_are_scoped_and_constant_time_comparable() {
        let token = new_deployment_token();
        let hash = deployment_token_hash(&token);
        assert!(token.starts_with(TOKEN_PREFIX));
        assert!(deployment_token_matches(&hash, &token));
        assert!(!deployment_token_matches(&hash, "fnk_cert_wrong"));
    }

    #[test]
    fn stored_bindings_fail_closed_on_invalid_security_metadata() {
        let mut valid = test_binding();
        valid.token_hash = deployment_token_hash("token");
        assert!(validate_stored_bindings(&[valid.clone()]).is_ok());

        let mut unsupported = valid.clone();
        unsupported.provider = "unknown".to_string();
        assert!(validate_stored_bindings(&[unsupported]).is_err());

        let mut invalid_hash = valid.clone();
        invalid_hash.token_hash = "not-a-sha256".to_string();
        assert!(validate_stored_bindings(&[invalid_hash]).is_err());

        assert!(validate_stored_bindings(&[valid.clone(), valid]).is_err());
    }

    #[test]
    fn provider_adapters_emit_their_native_setup_contracts() {
        let binding = test_binding();
        let endpoints = test_endpoints();
        let view = binding_view(&binding, 7998, &endpoints);
        assert_eq!(view.setup_kind, "webhook");
        assert_eq!(view.request_method.as_deref(), Some("PUT"));
        assert_eq!(
            view.request_body_template.as_deref(),
            Some(r#"{"cert":"${crt}","key":"${key}"}"#)
        );
        assert!(view.script_template.is_none());
        assert_eq!(view.deploy_path, "/api/integrations/certificates/binding-1");
        assert_eq!(view.deploy_port, 7998);
        assert_eq!(
            view.public_deploy_url.as_deref(),
            Some("https://auth.example.com/__certificates__/binding-1")
        );
        assert_eq!(normalize_provider(None).as_deref(), Ok("certd"));

        for provider in ["acme_sh", "lego", "certbot"] {
            let mut binding = test_binding();
            binding.provider = provider.to_string();
            let view = binding_view(&binding, 7998, &endpoints);
            let script = view.script_template.as_deref().unwrap();
            assert_eq!(view.setup_kind, "deploy_hook");
            assert!(view.request_method.is_none());
            assert!(view.usage_instructions.is_some());
            assert!(script.contains("__FN_KNOCK_DEPLOY_URL__"));
            assert!(script.contains("__FN_KNOCK_DEPLOY_TOKEN__"));
            assert!(script.contains("--fail-with-body"));
            assert!(script.contains("while [ \"$_fnk_attempt\" -le 4 ]"));
            assert!(script.contains("--connect-timeout 10"));
            assert!(script.contains("--max-time 60"));
            assert!(script.contains("--rawfile cert"));
            #[cfg(unix)]
            assert!(
                std::process::Command::new("sh")
                    .args(["-n", "-c", script])
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let acme_sh = binding_view(
            &ExternalCertificateBinding {
                provider: "acme_sh".to_string(),
                ..test_binding()
            },
            7998,
            &endpoints,
        );
        assert!(acme_sh.script_template.unwrap().contains("$5"));
        let lego = binding_view(
            &ExternalCertificateBinding {
                provider: "lego".to_string(),
                ..test_binding()
            },
            7998,
            &endpoints,
        );
        let lego_script = lego.script_template.unwrap();
        assert!(lego_script.contains("LEGO_HOOK_CERT_PATH"));
        assert!(lego_script.contains("LEGO_CERT_PATH"));
        assert!(lego.usage_instructions.unwrap().contains("--renew-hook"));
        let certbot = binding_view(
            &ExternalCertificateBinding {
                provider: "certbot".to_string(),
                ..test_binding()
            },
            7998,
            &endpoints,
        );
        assert!(certbot.script_template.unwrap().contains("RENEWED_LINEAGE"));

        for provider in ["certd", "acme_sh", "lego", "certbot"] {
            assert_eq!(normalize_provider(Some(provider)).as_deref(), Ok(provider));
        }
        assert!(normalize_provider(Some("unknown")).is_err());
    }

    #[test]
    fn bearer_scheme_is_case_insensitive() {
        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_static("bearer fnk_cert_value"),
        );
        assert_eq!(bearer_token(&headers), Some("fnk_cert_value"));
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Basic value"));
        assert_eq!(bearer_token(&headers), None);
    }

    #[test]
    fn binding_names_reject_control_characters() {
        assert_eq!(normalize_binding_name("  Certd  ").as_deref(), Ok("Certd"));
        assert!(normalize_binding_name("Certd\nproduction").is_err());
        assert!(normalize_binding_name("Certd\u{0000}production").is_err());
    }

    #[test]
    fn external_validation_checks_pair_domains_and_entire_pem_chain() {
        let (cert, key) = generated_certificate(&["example.com", "*.example.com"]);
        let (_, other_key) = generated_certificate(&["other.example.com"]);
        let body = ExternalCertificateDeployBody {
            cert: cert.clone(),
            key,
        };
        let (_, _, metadata) = validate_external_certificate(&body).unwrap();
        assert!(metadata.dns_names.contains(&"example.com".to_string()));
        assert!(metadata.dns_names.contains(&"*.example.com".to_string()));
        assert!(
            validate_external_certificate(&ExternalCertificateDeployBody {
                cert: cert.clone(),
                key: other_key,
            })
            .is_err()
        );
        assert!(certificate_metadata(&format!("{cert}\nnot-a-certificate")).is_err());

        let (unnamed_cert, unnamed_key) = generated_unnamed_certificate();
        let unnamed_error = validate_external_certificate(&ExternalCertificateDeployBody {
            cert: unnamed_cert,
            key: unnamed_key,
        })
        .unwrap_err();
        assert!(matches!(
            unnamed_error,
            ExternalDeployError::BadRequest(message) if message.contains("SAN or a common name")
        ));

        let issuer = test_ca();
        let (leaf, leaf_key) = generated_signed_certificate(&issuer);
        let valid_chain = format!("{leaf}\n{}", issuer.pem().trim());
        validate_external_certificate(&ExternalCertificateDeployBody {
            cert: valid_chain,
            key: leaf_key.clone(),
        })
        .unwrap();

        let unrelated_issuer_with_same_name = test_ca();
        let forged_chain = format!("{leaf}\n{}", unrelated_issuer_with_same_name.pem().trim());
        let error = validate_external_certificate(&ExternalCertificateDeployBody {
            cert: forged_chain,
            key: leaf_key,
        })
        .unwrap_err();
        assert!(
            matches!(error, ExternalDeployError::BadRequest(message) if message.contains("signature"))
        );
    }

    #[test]
    fn stable_slot_is_idempotent_and_preserves_certificate_role() {
        let binding = test_binding();
        let (cert, key) = generated_certificate(&["example.com"]);
        let metadata = certificate_metadata(&cert).unwrap();
        let (first_ssl, changed, should_sync, active, takeover) =
            prepare_external_certificate_update(&json!({}), &binding, &cert, &key, &metadata)
                .unwrap();
        assert!((changed, should_sync, active) == (true, true, true));
        assert!(takeover.replaced_certificate_ids.is_empty());
        assert_eq!(first_ssl["active_cert_id"], json!(binding.certificate_id));
        assert_eq!(first_ssl["certificates"].as_array().unwrap().len(), 1);

        let config = json!({ "ssl": first_ssl });
        let (_, changed, should_sync, active, _) =
            prepare_external_certificate_update(&config, &binding, &cert, &key, &metadata).unwrap();
        assert!((changed, should_sync, active) == (false, false, true));

        let (manual_cert, manual_key) = generated_certificate(&["manual.example.com"]);
        let (incoming_cert, incoming_key) = generated_certificate(&["new.example.com"]);
        let incoming_metadata = certificate_metadata(&incoming_cert).unwrap();
        let manual_metadata = certificate_metadata(&manual_cert).unwrap();
        assert!(
            normalized_domain_set(&manual_metadata.dns_names)
                .is_disjoint(&normalized_domain_set(&incoming_metadata.dns_names)),
            "manual={:?} incoming={:?}",
            manual_metadata.dns_names,
            incoming_metadata.dns_names
        );
        let inactive_config = json!({
            "ssl": {
                "deployment_mode": "single_active",
                "active_cert_id": "manual-1",
                "certificates": [{
                    "id": "manual-1",
                    "label": "Manual",
                    "source": "manual",
                    "cert": manual_cert,
                    "key": manual_key,
                    "created_at": "2026-08-16T00:00:00Z",
                    "updated_at": "2026-08-16T00:00:00Z"
                }]
            }
        });
        let (_, changed, should_sync, active, _) = prepare_external_certificate_update(
            &inactive_config,
            &binding,
            &incoming_cert,
            &incoming_key,
            &incoming_metadata,
        )
        .unwrap();
        assert!((changed, should_sync, active) == (true, false, false));

        let mut multi_sni_config = inactive_config;
        multi_sni_config["ssl"]["deployment_mode"] = json!("multi_sni");
        let (_, changed, should_sync, active, _) = prepare_external_certificate_update(
            &multi_sni_config,
            &binding,
            &incoming_cert,
            &incoming_key,
            &incoming_metadata,
        )
        .unwrap();
        assert!((changed, should_sync, active) == (true, true, false));

        let (renewed_cert, renewed_key) = generated_certificate(&["example.com"]);
        let renewed_metadata = certificate_metadata(&renewed_cert).unwrap();
        let mut active_multi_sni_config = config;
        active_multi_sni_config["ssl"]["deployment_mode"] = json!("multi_sni");
        let (_, changed, should_sync, active, _) = prepare_external_certificate_update(
            &active_multi_sni_config,
            &binding,
            &renewed_cert,
            &renewed_key,
            &renewed_metadata,
        )
        .unwrap();
        assert!((changed, should_sync, active) == (true, true, true));
    }

    #[test]
    fn exact_normalized_san_set_is_taken_over_but_partial_overlap_is_rejected() {
        let binding = test_binding();
        let (existing_cert, existing_key) =
            generated_certificate(&["EXAMPLE.COM.", "www.example.com"]);
        let (incoming_cert, incoming_key) =
            generated_certificate(&["www.example.com", "example.com"]);
        let incoming_metadata = certificate_metadata(&incoming_cert).unwrap();
        let config = json!({
            "ssl": {
                "deployment_mode": "multi_sni",
                "active_cert_id": "manual-1",
                "certificates": [{
                    "id": "manual-1",
                    "source": "manual",
                    "cert": existing_cert,
                    "key": existing_key
                }]
            }
        });
        let (next, changed, should_sync, active, takeover) = prepare_external_certificate_update(
            &config,
            &binding,
            &incoming_cert,
            &incoming_key,
            &incoming_metadata,
        )
        .unwrap();
        assert!((changed, should_sync, active) == (true, true, true));
        assert_eq!(
            takeover.replaced_certificate_ids,
            BTreeSet::from(["manual-1".to_string()])
        );
        assert_eq!(
            takeover.replaced_sources,
            BTreeSet::from(["manual".to_string()])
        );
        assert_eq!(next["active_cert_id"], json!(binding.certificate_id));
        assert_eq!(next["certificates"].as_array().unwrap().len(), 1);

        let (partial_cert, partial_key) =
            generated_certificate(&["example.com", "api.example.com"]);
        let partial_metadata = certificate_metadata(&partial_cert).unwrap();
        let error = prepare_external_certificate_update(
            &config,
            &binding,
            &partial_cert,
            &partial_key,
            &partial_metadata,
        )
        .unwrap_err();
        assert!(
            matches!(error, ExternalDeployError::Conflict(message) if message.contains("partially overlaps"))
        );
    }

    #[test]
    fn takeover_disables_only_superseded_external_bindings() {
        let mut current = test_binding();
        current.certificate_id = "external-current".to_string();
        let mut superseded = test_binding();
        superseded.id = "binding-old".to_string();
        superseded.certificate_id = "external-old".to_string();
        let mut unrelated = test_binding();
        unrelated.id = "binding-unrelated".to_string();
        unrelated.certificate_id = "external-unrelated".to_string();
        let takeover = ExternalCertificateTakeover {
            replaced_certificate_ids: BTreeSet::from(["external-old".to_string()]),
            ..Default::default()
        };
        let mut bindings = vec![current, superseded, unrelated];
        assert_eq!(
            disable_superseded_external_bindings(&mut bindings, 0, &takeover, "binding-1"),
            1
        );
        assert!(!bindings[1].enabled);
        assert_eq!(bindings[1].last_result.as_deref(), Some("superseded"));
        assert!(bindings[2].enabled);
    }

    #[test]
    fn repeated_success_metadata_is_idempotent_but_incomplete_status_can_heal() {
        let (cert, _) = generated_certificate(&["example.com"]);
        let metadata = certificate_metadata(&cert).unwrap();
        let mut binding = test_binding();
        assert!(!binding_has_success_metadata(&binding, &metadata));

        binding.last_result = Some("success".to_string());
        binding.last_fingerprint_sha256 = Some(metadata.fingerprint_sha256.clone());
        binding.last_valid_to = Some(metadata.valid_to.clone());
        binding.last_dns_names = metadata.dns_names.clone();
        assert!(binding_has_success_metadata(&binding, &metadata));

        binding.last_result = Some("failed".to_string());
        assert!(!binding_has_success_metadata(&binding, &metadata));
    }

    #[test]
    fn older_certificate_cannot_replace_a_newer_stable_slot() {
        let binding = test_binding();
        let (cert, key) = generated_certificate(&["example.com"]);
        let current_metadata = certificate_metadata(&cert).unwrap();
        let current = json!({
            "ssl": {
                "active_cert_id": binding.certificate_id,
                "certificates": [{
                    "id": binding.certificate_id,
                    "cert": cert,
                    "key": key
                }]
            }
        });
        let older = CertificateMetadata {
            not_after_ms: current_metadata.not_after_ms - 1,
            valid_to: time_utils::iso_from_ms(current_metadata.not_after_ms - 1),
            fingerprint_sha256: "older".to_string(),
            dns_names: vec!["example.com".to_string()],
        };
        let result = prepare_external_certificate_update(
            &current,
            &binding,
            "different-cert",
            "different-key",
            &older,
        );
        assert!(matches!(result, Err(ExternalDeployError::Conflict(_))));
    }

    #[tokio::test]
    async fn binding_token_lifecycle_is_scoped_and_certificate_is_preserved() {
        let (_directory, state) = test_state().await;
        let created = create_external_certificate_binding(
            State(state.clone()),
            Json(ExternalCertificateBindingCreateBody {
                name: "Certd production".to_string(),
                provider: Some("certd".to_string()),
            }),
        )
        .await;
        assert_eq!(created.status(), StatusCode::OK);
        assert!(
            created
                .headers()
                .get(header::CACHE_CONTROL)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("no-store")
        );
        assert!(created.headers().contains_key("CDN-Cache-Control"));
        let created = response_json(created).await;
        let binding_id = created["data"]["binding"]["id"]
            .as_str()
            .unwrap()
            .to_string();
        let token = created["data"]["token"].as_str().unwrap().to_string();
        assert_eq!(created["data"]["binding"]["deploy_port"], json!(18_080));
        let stored = load_bindings(&state).await.unwrap();
        assert_eq!(stored.len(), 1);
        assert_ne!(stored[0].token_hash, token);
        assert!(!serde_json::to_string(&stored).unwrap().contains(&token));

        let (cert, key) = generated_certificate(&["certd.example.com"]);
        let update_guard = state.gateway.ssl_update_lock.lock().await;
        let (wrong_request, _body_sender) = pending_deployment_request("fnk_cert_wrong");
        let wrong = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            deploy_external_certificate(
                State(state.clone()),
                AxumPath(binding_id.clone()),
                wrong_request,
            ),
        )
        .await
        .expect("invalid tokens must be rejected before waiting for the SSL update lock");
        drop(update_guard);
        assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

        let (manual_cert, manual_key) = generated_certificate(&["manual.example.com"]);
        state
            .storage
            .store
            .save_config(&json!({
                "ssl": {
                    "deployment_mode": "single_active",
                    "active_cert_id": "manual-1",
                    "certificates": [{
                        "id": "manual-1",
                        "label": "Manual",
                        "source": "manual",
                        "cert": manual_cert,
                        "key": manual_key,
                        "created_at": "2026-08-16T00:00:00Z",
                        "updated_at": "2026-08-16T00:00:00Z"
                    }]
                }
            }))
            .await
            .unwrap();
        let deployed = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding_id.clone()),
            deployment_request(&token, &cert, &key),
        )
        .await;
        assert_eq!(deployed.status(), StatusCode::OK);
        let deployed = response_json(deployed).await;
        assert_eq!(deployed["data"]["changed"], json!(true));
        assert_eq!(deployed["data"]["is_active"], json!(false));
        assert_eq!(deployed["data"]["gateway_applied"], json!(false));

        let repeated = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding_id.clone()),
            deployment_request(&token, &cert, &key),
        )
        .await;
        assert_eq!(
            response_json(repeated).await["data"]["changed"],
            json!(false)
        );

        let rotated = rotate_external_certificate_binding_token(
            State(state.clone()),
            AxumPath(binding_id.clone()),
        )
        .await;
        assert!(
            rotated
                .headers()
                .get(header::CACHE_CONTROL)
                .unwrap()
                .to_str()
                .unwrap()
                .contains("no-store")
        );
        let rotated = response_json(rotated).await;
        let next_token = rotated["data"]["token"].as_str().unwrap().to_string();
        assert_ne!(token, next_token);
        let old_token = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding_id.clone()),
            deployment_request(&token, &cert, &key),
        )
        .await;
        assert_eq!(old_token.status(), StatusCode::UNAUTHORIZED);

        assert_eq!(
            delete_external_certificate_binding(
                State(state.clone()),
                AxumPath(binding_id.clone()),
            )
            .await
            .status(),
            StatusCode::OK
        );
        let revoked = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding_id),
            deployment_request(&next_token, &cert, &key),
        )
        .await;
        assert_eq!(revoked.status(), StatusCode::NOT_FOUND);
        let config = state.storage.store.get_config().await.unwrap();
        assert_eq!(config["ssl"]["certificates"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn exact_san_takeover_disables_the_previous_external_binding_and_token() {
        let (_directory, state) = test_state().await;
        let current_token = new_deployment_token();
        let old_token = new_deployment_token();
        let mut current = test_binding();
        current.token_hash = deployment_token_hash(&current_token);
        let mut old = test_binding();
        old.id = "binding-old".to_string();
        old.certificate_id = "external-old".to_string();
        old.token_hash = deployment_token_hash(&old_token);
        save_bindings(&state, &[current.clone(), old.clone()])
            .await
            .unwrap();

        let (active_cert, active_key) = generated_certificate(&["active.example.com"]);
        let (old_cert, old_key) = generated_certificate(&["takeover.example.com"]);
        state
            .storage
            .store
            .save_config(&json!({
                "ssl": {
                    "deployment_mode": "single_active",
                    "active_cert_id": "manual-active",
                    "certificates": [
                        {
                            "id": "manual-active",
                            "source": "manual",
                            "cert": active_cert,
                            "key": active_key
                        },
                        {
                            "id": old.certificate_id,
                            "source": "external",
                            "source_ref_id": old.id,
                            "cert": old_cert,
                            "key": old_key
                        }
                    ]
                }
            }))
            .await
            .unwrap();
        let (incoming_cert, incoming_key) = generated_certificate(&["takeover.example.com"]);
        let deployed = deploy_external_certificate(
            State(state.clone()),
            AxumPath(current.id.clone()),
            deployment_request(&current_token, &incoming_cert, &incoming_key),
        )
        .await;
        assert_eq!(deployed.status(), StatusCode::OK);
        let deployed = response_json(deployed).await;
        assert_eq!(deployed["data"]["replaced_certificate_count"], json!(1));
        assert_eq!(deployed["data"]["replaced_sources"], json!(["external"]));
        assert_eq!(
            deployed["data"]["disabled_external_binding_count"],
            json!(1)
        );
        assert_eq!(deployed["data"]["gateway_applied"], json!(false));

        let bindings = load_bindings(&state).await.unwrap();
        assert!(bindings[0].enabled);
        assert!(!bindings[1].enabled);
        assert_eq!(bindings[1].last_result.as_deref(), Some("superseded"));
        let takeover_at = bindings[0]
            .last_takeover_at
            .clone()
            .expect("takeover time should be recorded");
        let config = state.storage.store.get_config().await.unwrap();
        let certificate_ids = config["ssl"]["certificates"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|certificate| {
                certificate
                    .get("id")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<BTreeSet<_>>();
        assert!(certificate_ids.contains(&current.certificate_id));
        assert!(!certificate_ids.contains(&old.certificate_id));

        let failed_retry = deploy_external_certificate(
            State(state.clone()),
            AxumPath(current.id.clone()),
            deployment_request(&current_token, "not-a-certificate", "not-a-key"),
        )
        .await;
        assert_eq!(failed_retry.status(), StatusCode::BAD_REQUEST);
        let after_failure = load_bindings(&state).await.unwrap();
        assert_eq!(after_failure[0].last_result.as_deref(), Some("failed"));
        assert_eq!(
            after_failure[0].last_takeover_at.as_deref(),
            Some(takeover_at.as_str())
        );
        assert_eq!(after_failure[0].last_replaced_certificate_count, 1);

        let revoked = deploy_external_certificate(
            State(state),
            AxumPath(old.id),
            deployment_request(&old_token, &incoming_cert, &incoming_key),
        )
        .await;
        assert_eq!(revoked.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn public_alias_enforces_auth_host_and_reuses_local_validation_limits() {
        let (_directory, state) = test_state().await;
        let token = new_deployment_token();
        let mut binding = test_binding();
        binding.token_hash = deployment_token_hash(&token);
        save_bindings(&state, &[binding.clone()]).await.unwrap();
        state
            .storage
            .store
            .save_config(&json!({
                "run_type": 3,
                "host_mappings": [{
                    "host": "auth.example.com",
                    "target": "http://127.0.0.1:7997"
                }],
                "subdomain_mode": {
                    "public_auth_base_url": "https://auth.example.com"
                }
            }))
            .await
            .unwrap();

        let invalid_json = |host: &str, body: Body| {
            Request::builder()
                .header(HOST, host)
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .header(header::CONTENT_TYPE, "application/json")
                .body(body)
                .unwrap()
        };
        let wrong_host = deploy_public_external_certificate(
            State(state.clone()),
            AxumPath(binding.id.clone()),
            invalid_json("other.example.com", Body::from("{")),
        )
        .await;
        assert_eq!(wrong_host.status(), StatusCode::NOT_FOUND);

        let public_invalid = deploy_public_external_certificate(
            State(state.clone()),
            AxumPath(binding.id.clone()),
            invalid_json("AUTH.EXAMPLE.COM.:443", Body::from("{")),
        )
        .await;
        assert_eq!(public_invalid.status(), StatusCode::BAD_REQUEST);
        let local_invalid = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding.id.clone()),
            invalid_json("127.0.0.1:18080", Body::from("{")),
        )
        .await;
        assert_eq!(local_invalid.status(), StatusCode::BAD_REQUEST);

        let oversized = deploy_public_external_certificate(
            State(state.clone()),
            AxumPath(binding.id.clone()),
            invalid_json(
                "auth.example.com",
                Body::from(vec![b'x'; EXTERNAL_CERTIFICATE_BODY_LIMIT + 1]),
            ),
        )
        .await;
        assert_eq!(oversized.status(), StatusCode::PAYLOAD_TOO_LARGE);

        let mut disabled = load_bindings(&state).await.unwrap();
        disabled[0].enabled = false;
        save_bindings(&state, &disabled).await.unwrap();
        let disabled_response = deploy_public_external_certificate(
            State(state),
            AxumPath(binding.id),
            invalid_json("auth.example.com", Body::from("{")),
        )
        .await;
        assert_eq!(disabled_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn public_deploy_url_is_rooted_on_the_configured_auth_host() {
        let (_directory, state) = test_state().await;
        state
            .storage
            .store
            .save_config(&json!({
                "run_type": 3,
                "host_mappings": [{
                    "host": "auth.example.com",
                    "target": "http://127.0.0.1:7997"
                }],
                "subdomain_mode": {
                    "public_auth_base_url": "https://unrelated.example.net:8443/nested?token=wrong"
                }
            }))
            .await
            .unwrap();

        let endpoints = external_certificate_deployment_endpoints(&state).await;
        assert_eq!(endpoints.public_deploy_status, "ready");
        assert_eq!(
            endpoints.public_deploy_base_url.as_deref(),
            Some("https://auth.example.com:8443")
        );
    }

    #[tokio::test]
    async fn failed_gateway_deployment_restores_the_previous_ssl_config() {
        let (_directory, state) = test_state().await;
        let token = new_deployment_token();
        let mut binding = test_binding();
        binding.token_hash = deployment_token_hash(&token);
        save_bindings(&state, &[binding.clone()]).await.unwrap();
        let (cert, key) = generated_certificate(&["rollback.example.com"]);

        let response = deploy_external_certificate(
            State(state.clone()),
            AxumPath(binding.id),
            deployment_request(&token, &cert, &key),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
        assert!(
            response_json(response).await["message"]
                .as_str()
                .unwrap()
                .contains("could not confirm restoration")
        );
        let config = state.storage.store.get_config().await.unwrap();
        assert!(
            config
                .pointer("/ssl/certificates")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        );
        let stored = load_bindings(&state).await.unwrap();
        assert_eq!(stored[0].last_result.as_deref(), Some("failed"));
    }
}
