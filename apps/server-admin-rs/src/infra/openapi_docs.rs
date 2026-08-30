use std::collections::BTreeSet;

use axum::{
    Extension, Router,
    body::Bytes,
    extract::Path,
    http::{StatusCode, header},
    response::{Html, IntoResponse},
    routing::get,
};
use serde_json::{Map, Value, json};
use utoipa::{OpenApi, ToSchema};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{app_version::APP_LOCAL_VERSION, state::AppState};

mod baseline_docs;
mod domain_contracts;
mod ssl_docs;

#[derive(ToSchema)]
#[allow(dead_code)]
struct ApiErrorEnvelope {
    /// Always false for an error response.
    success: bool,
    /// Stable numeric error code when the endpoint defines one.
    code: Option<u16>,
    message: String,
}

#[derive(OpenApi)]
#[openapi(components(schemas(ApiErrorEnvelope)))]
struct ContractSchemas;

const OPENAPI_DOCS_INDEX: &str = include_str!("openapi_docs/index.html");
const OPENAPI_DOCUMENT: &[u8] = include_bytes!("../../../../packages/api-contract/openapi.json");
const SWAGGER_UI_STYLESHEET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/swagger-ui.css"));
const SWAGGER_UI_INDEX_STYLESHEET: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index.css"));
const SWAGGER_UI_BUNDLE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/swagger-ui-bundle.js"));
const SWAGGER_UI_STANDALONE_PRESET: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/swagger-ui-standalone-preset.js"));
const SWAGGER_UI_FAVICON_16: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/favicon-16x16.png"));
const SWAGGER_UI_FAVICON_32: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/favicon-32x32.png"));

pub fn openapi_docs_routes() -> Router<AppState> {
    let document = Bytes::from_static(OPENAPI_DOCUMENT);

    Router::new()
        .route("/docs", get(openapi_docs_index))
        .route("/docs/", get(openapi_docs_index))
        .route("/docs/json", get(openapi_docs_json))
        .route("/docs/assets/{asset}", get(swagger_ui_asset))
        .layer(Extension(document))
}

async fn openapi_docs_index() -> Html<&'static str> {
    Html(OPENAPI_DOCS_INDEX)
}

async fn openapi_docs_json(Extension(document): Extension<Bytes>) -> impl IntoResponse {
    (
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        document,
    )
}

async fn swagger_ui_asset(Path(asset): Path<String>) -> impl IntoResponse {
    let (content_type, bytes) = match asset.as_str() {
        "swagger-ui.css" => ("text/css; charset=utf-8", SWAGGER_UI_STYLESHEET),
        "index.css" => ("text/css; charset=utf-8", SWAGGER_UI_INDEX_STYLESHEET),
        "swagger-ui-bundle.js" => ("application/javascript; charset=utf-8", SWAGGER_UI_BUNDLE),
        "swagger-ui-standalone-preset.js" => (
            "application/javascript; charset=utf-8",
            SWAGGER_UI_STANDALONE_PRESET,
        ),
        "favicon-16x16.png" => ("image/png", SWAGGER_UI_FAVICON_16),
        "favicon-32x32.png" => ("image/png", SWAGGER_UI_FAVICON_32),
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    ([(header::CONTENT_TYPE, content_type)], bytes).into_response()
}

pub(crate) fn build_openapi_document() -> Value {
    let typed_health = typed_health_contract();
    let typed_system_info = crate::system_info::system_info_routes().into_openapi();
    let typed_security_overview =
        crate::security_overview::security_overview_routes().into_openapi();
    let typed_dashboard = crate::dashboard::dashboard_routes().into_openapi();
    let typed_update = crate::update::update_routes().into_openapi();
    let typed_cidr = crate::scanner::cidr_routes().into_openapi();
    let typed_ip_location = crate::ip_location::ip_location_routes().into_openapi();
    let typed_ip_location_config =
        crate::ip_location_config::ip_location_config_routes().into_openapi();
    let typed_backoff = crate::backoff::backoff_routes().into_openapi();
    let typed_internal_events = crate::events::internal_system_event_routes().into_openapi();
    let typed_admin_events = crate::events::admin_event_routes().into_openapi();
    let typed_traces = crate::traces::trace_routes().into_openapi();
    let typed_runtime_health =
        crate::runtime_health::routes::runtime_health_routes().into_openapi();
    let typed_general_blacklist =
        crate::general_blacklist::general_blacklist_routes().into_openapi();
    let typed_scanner = crate::scanner::scanner_routes().into_openapi();
    let typed_gateway_settings = crate::gateway_settings::gateway_settings_routes().into_openapi();
    let typed_gateway_logs = crate::gateway_logs::gateway_logs_openapi_routes().into_openapi();
    let typed_scan_assets = crate::scan_assets::scan_asset_openapi_routes().into_openapi();
    let typed_fnos_certificate_sync =
        crate::fnos_certificate_sync::fnos_certificate_sync_routes().into_openapi();
    let typed_fnos_port_icon_hijack =
        crate::runtime_config::fnos_port_icon_hijack_routes().into_openapi();
    let typed_fnos_network_tuning =
        crate::runtime_config::fnos_network_tuning_routes().into_openapi();
    let typed_fnos_connect_waf = crate::runtime_config::fnos_connect_waf_routes().into_openapi();
    let typed_fnos_share_bypass = crate::runtime_config::fnos_share_bypass_routes().into_openapi();
    let typed_smart_connect = crate::runtime_config::smart_connect_config_routes().into_openapi();
    let typed_proxy_protocol_force =
        crate::runtime_config::proxy_protocol_force_routes().into_openapi();
    let typed_run_mode_prompt_preferences =
        crate::runtime_config::run_mode_prompt_preferences_routes().into_openapi();
    let typed_protocol_mapping_feature =
        crate::runtime_config::protocol_mapping_feature_routes().into_openapi();
    let typed_auto_https = crate::runtime_config::auto_https_config_routes().into_openapi();
    let typed_default_route = crate::runtime_config::default_route_config_routes().into_openapi();
    let typed_captcha = crate::runtime_config::captcha_config_routes().into_openapi();
    let typed_run_type = crate::runtime_config::run_type_config_routes().into_openapi();
    let typed_wol_feature = crate::runtime_config::wol_feature_config_routes().into_openapi();
    let typed_firewall_runtime = crate::runtime_config::firewall_runtime_routes().into_openapi();
    let typed_sync_routes = crate::runtime_config::sync_routes_config_routes().into_openapi();
    let typed_panel_config = crate::admin::panel::panel_config_routes().into_openapi();
    let typed_auth_mode = crate::admin_control::auth_mode_routes().into_openapi();
    let typed_auth_accounts = crate::admin_control::auth_account_routes().into_openapi();
    let typed_auth_credential_settings =
        crate::admin_control::auth_credential_settings_routes().into_openapi();
    let typed_sessions = crate::admin_control::session_routes().into_openapi();
    let typed_backup = crate::maintenance::backup_openapi_routes().into_openapi();
    let typed_maintenance_data =
        crate::maintenance::maintenance_data_openapi_routes().into_openapi();
    let typed_host_mappings = crate::proxy_config::host_mapping_routes().into_openapi();
    let typed_proxy_routing = crate::proxy_config::proxy_routing_routes().into_openapi();
    let typed_system_clock = crate::system_assets::system_clock_routes().into_openapi();
    let typed_system_binary_assets =
        crate::system_assets::system_binary_asset_routes().into_openapi();
    let typed_dnsmasq_assets = crate::system_assets::dnsmasq_asset_routes().into_openapi();
    let typed_terminal_runtime = crate::terminal::terminal_runtime_routes().into_openapi();
    let typed_whitelist = crate::whitelist::whitelist_openapi_routes().into_openapi();
    let typed_ssh_security = crate::ssh_security::ssh_security_openapi_routes().into_openapi();
    let typed_oidc_admin = crate::oidc_admin::oidc_admin_openapi_routes().into_openapi();
    let typed_ldap_admin = crate::ldap_auth::ldap_admin_openapi_routes().into_openapi();
    let typed_wol_local_relay = crate::wol::wol_local_relay_openapi_routes().into_openapi();
    let typed_wol_discovery = crate::wol::wol_discovery_openapi_routes().into_openapi();
    let typed_wol_relays = crate::wol::wol_relay_openapi_routes().into_openapi();
    let typed_wol_targets = crate::wol::wol_target_openapi_routes().into_openapi();
    let typed_notifications = crate::notifications::notification_openapi_routes().into_openapi();
    let typed_deep_monitor = crate::deep_monitor::deep_monitor_openapi_routes().into_openapi();
    let typed_waf = crate::waf::waf_openapi_routes().into_openapi();
    let typed_ssl = crate::ssl::ssl_openapi_routes().into_openapi();
    let typed_external_certificates =
        crate::ssl::external_certificate_openapi_routes().into_openapi();
    let typed_public_external_certificates =
        crate::ssl::public_external_certificate_openapi_routes().into_openapi();
    let typed_ddns = crate::ddns::ddns_openapi_routes().into_openapi();
    let typed_cloudflared = crate::cloudflared::cloudflared_openapi_routes().into_openapi();
    let typed_frpc = crate::frpc::frpc_openapi_routes().into_openapi();
    let typed_acme = crate::acme::acme_openapi_routes().into_openapi();
    let typed_panel_session = crate::admin::panel::panel_session_routes().into_openapi();
    let typed_totp_bootstrap = crate::admin_control::totp_bootstrap_routes().into_openapi();
    let typed_totp_management = crate::admin_control::totp_management_routes().into_openapi();
    let typed_panel_sync = crate::panel_sync::panel_sync_routes().into_openapi();
    let typed_health_operation = serde_json::to_value(typed_health)
        .ok()
        .and_then(|value| value.pointer("/paths/~1api~1admin~1healthz/get").cloned());
    let schema_document = serde_json::to_value(ContractSchemas::openapi()).unwrap_or_default();
    let mut components = schema_document
        .get("components")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let schemas = components
        .entry("schemas")
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(schemas) = schemas.as_object_mut() else {
        return Value::Null;
    };
    schemas.insert(
        "ApiSuccessEnvelope".to_string(),
        json!({
            "type": "object",
            "required": ["success"],
            "properties": {
                "success": { "type": "boolean", "const": true },
                "message": { "type": ["string", "null"] },
                "data": {}
            },
            "additionalProperties": true
        }),
    );
    schemas.extend(domain_contracts::components());
    if let Some(index_file_schema) = schemas
        .get_mut("StaticServeConfigData")
        .and_then(|schema| schema.pointer_mut("/properties/index_files/items"))
        .and_then(Value::as_object_mut)
    {
        index_file_schema.insert("minLength".to_string(), json!(1));
        index_file_schema.insert("maxLength".to_string(), json!(255));
        index_file_schema.insert("pattern".to_string(), json!(r"^(?!\.{1,2}$)[^/\\\u0000]+$"));
    }
    if let Ok(panel_sync_document) = serde_json::to_value(&typed_panel_sync)
        && let Some(panel_sync_schemas) = panel_sync_document
            .pointer("/components/schemas")
            .and_then(Value::as_object)
    {
        schemas.extend(panel_sync_schemas.clone());
    }
    if let Ok(terminal_document) = serde_json::to_value(&typed_terminal_runtime)
        && let Some(terminal_schemas) = terminal_document
            .pointer("/components/schemas")
            .and_then(Value::as_object)
    {
        // The terminal contract is generated from the same domain types used
        // by the handlers. This prevents secret-bearing request fields and
        // runtime response fields from drifting apart in duplicate schemas.
        schemas.extend(terminal_schemas.clone());
    }
    components.insert(
        "securitySchemes".to_string(),
        json!({
            "certificateDeploymentToken": {
                "type": "http",
                "scheme": "bearer",
                "bearerFormat": "fnk_cert_*",
                "description": "Binding-scoped token shown only when an external certificate deployment binding is created or rotated."
            }
        }),
    );
    let mut paths = Map::new();
    if let Some(mut health_operation) = typed_health_operation {
        health_operation["responses"]["default"] = json!({
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        });
        health_operation["x-fn-knock-contract-source"] = json!("utoipa");
        paths.insert(
            "/api/admin/healthz".to_string(),
            json!({ "get": health_operation }),
        );
    }

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_system_info,
        "/api/admin/system/access-entry",
        "get",
        "AccessEntryData",
        None,
        None,
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/providers",
        "get",
        "ProviderDescriptor",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings/lan",
        "get",
        "LanCertificateDeploymentData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings/lan",
        "put",
        "LanCertificateDeploymentData",
        None,
        Some("LanCertificateDeploymentUpdateBodyData"),
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections",
        "get",
        "PanelConnection",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections",
        "post",
        "PanelConnection",
        None,
        Some("ConnectionInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections/{id}",
        "put",
        "PanelConnection",
        None,
        Some("ConnectionUpdateInput"),
    );
    insert_typed_message_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections/{id}",
        "delete",
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/test",
        "post",
        "ProbeResult",
        None,
        Some("TestConnectionInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections/{id}/preview",
        "post",
        "SyncPreview",
        None,
        Some("PreviewRequest"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections/{id}/sync",
        "post",
        "SyncAccepted",
        None,
        Some("SyncRequest"),
    );
    if let Some(responses) = paths
        .get_mut("/api/admin/panel-sync/connections/{id}/sync")
        .and_then(Value::as_object_mut)
        .and_then(|path| path.get_mut("post"))
        .and_then(|operation| operation.get_mut("responses"))
        .and_then(Value::as_object_mut)
        && let Some(success) = responses.get("200").cloned()
    {
        responses.insert("202".to_string(), success);
    }
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/connections/{id}/runs",
        "get",
        "SyncRun",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_sync,
        "/api/admin/panel-sync/runs/{run_id}",
        "get",
        "SyncRun",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_system_clock,
        "/api/admin/system/clock/status",
        "get",
        "SystemClockStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_system_clock,
        "/api/admin/system/clock/check",
        "post",
        "SystemClockStatusData",
        None,
        None,
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_system_clock,
        "/api/admin/system/clock/sync",
        "post",
        "SystemClockSyncResponseData",
        None,
    );
    for (path, method, schema, enveloped) in [
        (
            "/api/admin/system/cloudflared/status",
            "get",
            "CloudflaredAssetStatusData",
            true,
        ),
        (
            "/api/admin/system/cloudflared/download",
            "post",
            "SystemAssetMutationResponseData",
            false,
        ),
        (
            "/api/admin/system/cloudflared/cancel",
            "post",
            "SystemAssetMutationResponseData",
            false,
        ),
        (
            "/api/admin/system/cloudflared",
            "delete",
            "SystemAssetMutationResponseData",
            false,
        ),
        (
            "/api/admin/system/frp/status",
            "get",
            "FrpAssetStatusData",
            true,
        ),
        (
            "/api/admin/system/frp/download",
            "post",
            "SystemAssetMutationResponseData",
            false,
        ),
        (
            "/api/admin/system/frp/cancel",
            "post",
            "SystemAssetMutationResponseData",
            false,
        ),
        (
            "/api/admin/system/frp",
            "delete",
            "SystemAssetMutationResponseData",
            false,
        ),
    ] {
        if enveloped {
            insert_typed_enveloped_operation(
                &mut paths,
                &typed_system_binary_assets,
                path,
                method,
                schema,
                None,
                None,
            );
        } else {
            insert_typed_direct_operation(
                &mut paths,
                &typed_system_binary_assets,
                path,
                method,
                schema,
                None,
            );
        }
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dnsmasq_assets,
        "/api/admin/system/dnsmasq/status",
        "get",
        "DnsmasqStatusData",
        None,
        None,
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist",
        "get",
        "WhitelistRecordData",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist",
        "post",
        "WhitelistAddResultData",
        None,
        Some("WhitelistAddBodyData"),
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/regions",
        "get",
        "WhitelistRegionGroupData",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/regions",
        "post",
        "WhitelistRegionAddResultData",
        None,
        Some("WhitelistRegionAddBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/regions/{id}",
        "delete",
        None,
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/{id}",
        "delete",
        None,
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/{id}/comment",
        "patch",
        Some("WhitelistCommentBodyData"),
        None,
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_whitelist,
        "/api/admin/whitelist/{id}/refresh",
        "post",
        "WhitelistRefreshEnvelopeData",
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/config",
        "get",
        "GatewayLoggingConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/config",
        "post",
        "GatewayLoggingConfigData",
        None,
        Some("GatewayLoggingConfigUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/directory",
        "get",
        "GatewayLogDirectoryData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/dates",
        "get",
        "GatewayLogDatesData",
        None,
        None,
    );
    let gateway_log_entries_parameters = json!([
        {"name":"date","in":"query","required":false,"schema":{"type":"string","format":"date"}},
        {"name":"pagination","in":"query","required":false,"schema":{"type":"string","enum":["page","cursor"]}},
        {"name":"page","in":"query","required":false,"schema":{"type":"integer","minimum":1}},
        {"name":"limit","in":"query","required":false,"schema":{"type":"string","pattern":"^[1-9][0-9]*$"}},
        {"name":"cursor","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"search","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"status","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"logged_in","in":"query","required":false,"schema":{"type":"string","enum":["true","false"]}},
        {"name":"credential","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"waf_status","in":"query","required":false,"schema":{"type":"string","enum":["has_waf","none"]}},
        {"name":"trace_id","in":"query","required":false,"schema":{"type":"string","pattern":crate::trace_id::TRACE_ID_PATTERN}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/entries",
        "get",
        "GatewayLogEntriesData",
        Some(gateway_log_entries_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/entries",
        "delete",
        "GatewayLogDeleteData",
        None,
        Some("GatewayLogDeleteBodyData"),
    );
    let gateway_log_analytics_parameters = json!([
        {"name":"from","in":"query","required":false,"schema":{"type":"string","format":"date"}},
        {"name":"to","in":"query","required":false,"schema":{"type":"string","format":"date"}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/analytics",
        "get",
        "GatewayLogAnalyticsData",
        Some(gateway_log_analytics_parameters.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_gateway_logs,
        "/api/admin/gateway-logs/analytics",
        "post",
        "GatewayLogAnalyticsRefreshData",
        Some(gateway_log_analytics_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover-settings",
        "get",
        "ScanDiscoverySettingsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover-settings",
        "post",
        "ScanDiscoverySettingsData",
        None,
        Some("ScanDiscoverySettingsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover-targets",
        "get",
        "ScanDiscoveryTargetsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover-targets",
        "post",
        "ScanDiscoveryTargetsData",
        None,
        Some("ScanDiscoveryTargetsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover/jobs",
        "post",
        "ScanDiscoverJobData",
        None,
        Some("ScanDiscoverJobBodyData"),
    );
    let scan_job_parameters = json!([
        {"name":"job_id","in":"path","required":true,"schema":{"type":"string"}},
        {"name":"cursor","in":"query","required":false,"schema":{"type":"integer","minimum":0}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover/jobs/{job_id}",
        "get",
        "ScanDiscoverJobData",
        Some(scan_job_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/discover/jobs/{job_id}",
        "delete",
        "ScanDiscoverJobData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scan_assets,
        "/api/admin/scan/host-mappings/probe",
        "post",
        "HostMappingsProbeData",
        None,
        Some("HostMappingsProbeBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/config",
        "get",
        "SshSecurityDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/config",
        "post",
        "SshSecurityDetailsData",
        None,
        Some("SshSecurityConfigUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/firewall/sync",
        "post",
        "SshFirewallSyncData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/firewall/clear",
        "post",
        "SshFirewallClearData",
        None,
        None,
    );
    let ssh_login_log_parameters = json!([
        {"name":"page","in":"query","required":false,"schema":{"type":"integer","minimum":1}},
        {"name":"limit","in":"query","required":false,"schema":{"type":"string","pattern":"^[1-9][0-9]*$"}},
        {"name":"search","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"outcome","in":"query","required":false,"schema":{"type":"string","enum":["success","failure"]}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/login-logs",
        "get",
        "SshLoginLogListData",
        Some(ssh_login_log_parameters),
        None,
    );
    let ssh_blocks_parameters = json!([
        {"name":"page","in":"query","required":false,"schema":{"type":"integer","minimum":1}},
        {"name":"limit","in":"query","required":false,"schema":{"type":"string","pattern":"^[1-9][0-9]*$"}},
        {"name":"search","in":"query","required":false,"schema":{"type":"string"}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/blocks",
        "get",
        "SshSecurityBlockListData",
        Some(ssh_blocks_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/blocks",
        "delete",
        "SshBlocksDeleteData",
        None,
        Some("SshBlocksDeleteBodyData"),
    );
    let ssh_block_ip_parameter = json!([{
        "name": "ip",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/blocks/{ip}",
        "get",
        "SshSecurityBlockData",
        Some(ssh_block_ip_parameter.clone()),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssh_security,
        "/api/admin/ssh-security/blocks/{ip}",
        "delete",
        None,
        Some(ssh_block_ip_parameter),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/catalog",
        "get",
        "OidcProviderCatalogData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/providers",
        "get",
        "OidcProvidersData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/providers",
        "post",
        "OidcProviderData",
        None,
        Some("OidcProviderCreateData"),
    );
    let oidc_provider_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/providers/{id}",
        "patch",
        "OidcProviderData",
        Some(oidc_provider_id_parameter.clone()),
        Some("OidcProviderUpdateData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/providers/{id}",
        "delete",
        None,
        Some(oidc_provider_id_parameter.clone()),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/providers/{id}/test",
        "post",
        "ExternalAuthConnectionTestData",
        None,
    );
    let oidc_totp_id_parameter = json!([{
        "name": "totp_id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/totp/{totp_id}/bindings",
        "get",
        "OidcBindingsData",
        Some(oidc_totp_id_parameter),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/bindings/{id}",
        "delete",
        None,
        Some(oidc_provider_id_parameter),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_oidc_admin,
        "/api/admin/auth/oidc/invitations",
        "post",
        "ExternalAuthInvitationData",
        None,
        Some("ExternalAuthInvitationBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/catalog",
        "get",
        "LdapProviderCatalogData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/providers",
        "get",
        "LdapProvidersData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/providers",
        "post",
        "LdapProviderData",
        None,
        Some("LdapProviderCreateData"),
    );
    let ldap_provider_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/providers/{id}",
        "patch",
        "LdapProviderData",
        Some(ldap_provider_id_parameter.clone()),
        Some("LdapProviderUpdateData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/providers/{id}",
        "delete",
        None,
        Some(ldap_provider_id_parameter.clone()),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/providers/{id}/test",
        "post",
        "ExternalAuthConnectionTestData",
        None,
    );
    if let Some(operation) = paths
        .get_mut("/api/admin/auth/ldap/providers/{id}/test")
        .and_then(Value::as_object_mut)
        .and_then(|methods| methods.get_mut("post"))
        .and_then(Value::as_object_mut)
    {
        operation["requestBody"] = json!({
            "required": false,
            "content": { "application/json": { "schema": { "$ref": "#/components/schemas/LdapProviderTestBodyData" } } }
        });
    }
    let ldap_totp_id_parameter = json!([{
        "name": "totp_id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/totp/{totp_id}/bindings",
        "get",
        "LdapBindingsData",
        Some(ldap_totp_id_parameter),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/bindings/{id}",
        "delete",
        None,
        Some(ldap_provider_id_parameter),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ldap_admin,
        "/api/admin/auth/ldap/invitations",
        "post",
        "ExternalAuthInvitationData",
        None,
        Some("ExternalAuthInvitationBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_local_relay,
        "/api/admin/wol/local-relay",
        "get",
        "WolLocalRelayData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_local_relay,
        "/api/admin/wol/local-relay",
        "put",
        "WolLocalRelayData",
        None,
        Some("WolLocalRelayInputData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_local_relay,
        "/api/admin/wol/local-relay/pair",
        "post",
        "WolLocalRelayData",
        None,
        Some("WolLocalRelayPairBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_discovery,
        "/api/admin/wol/discover/jobs",
        "post",
        "WolDiscoveryJobData",
        None,
        Some("WolDiscoveryBodyData"),
    );
    let wol_discovery_job_parameters = json!([
        {"name":"id","in":"path","required":true,"schema":{"type":"string"}},
        {"name":"cursor","in":"query","required":false,"schema":{"type":"integer","minimum":0}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_discovery,
        "/api/admin/wol/discover/jobs/{id}",
        "get",
        "WolDiscoveryJobData",
        Some(wol_discovery_job_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays",
        "get",
        "WolRelayListData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays",
        "post",
        "WolRelayCredentialData",
        None,
        Some("WolRelayInputData"),
    );
    let wol_relay_id_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays/{id}",
        "get",
        "WolRelayData",
        Some(wol_relay_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays/{id}",
        "put",
        "WolRelayData",
        Some(wol_relay_id_parameter.clone()),
        Some("WolRelayInputData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays/{id}",
        "delete",
        None,
        Some(wol_relay_id_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays/{id}/rotate-psk",
        "post",
        "WolRelayCredentialData",
        Some(wol_relay_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_relays,
        "/api/admin/wol/relays/{id}/probe",
        "post",
        "WolDispatchData",
        Some(wol_relay_id_parameter),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets",
        "get",
        "WolTargetListData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets",
        "post",
        "WolTargetData",
        None,
        Some("WolTargetInputData"),
    );
    let wol_target_id_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}",
        "get",
        "WolTargetData",
        Some(wol_target_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}",
        "put",
        "WolTargetData",
        Some(wol_target_id_parameter.clone()),
        Some("WolTargetInputData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}",
        "delete",
        None,
        Some(wol_target_id_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}/wake",
        "post",
        "WolDispatchData",
        Some(wol_target_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}/ssh/test",
        "post",
        "WolSshConnectionTestData",
        Some(wol_target_id_parameter.clone()),
        Some("WolTargetSshInputData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_targets,
        "/api/admin/wol/targets/{id}/shutdown",
        "post",
        "WolShutdownData",
        Some(wol_target_id_parameter),
        None,
    );
    let notification_provider_id_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/catalog",
        "get",
        "NotificationProviderCatalogData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers",
        "get",
        "NotificationProviderListData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers",
        "post",
        "NotificationProviderData",
        None,
        Some("NotificationProviderCreateBodyData"),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/test",
        "post",
        "NotificationProviderTestResponseData",
        Some("NotificationProviderTestBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/{id}",
        "get",
        "NotificationProviderDetailData",
        Some(notification_provider_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/{id}",
        "patch",
        "NotificationProviderData",
        Some(notification_provider_id_parameter.clone()),
        Some("NotificationProviderUpdateBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/{id}",
        "delete",
        None,
        Some(notification_provider_id_parameter.clone()),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/providers/{id}/test",
        "post",
        "NotificationProviderTestResponseData",
        None,
    );
    let notification_rule_id_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/rules",
        "get",
        "NotificationRuleListData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/rules",
        "post",
        "NotificationRuleData",
        None,
        Some("NotificationRuleCreateBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/rules/{id}",
        "patch",
        "NotificationRuleData",
        Some(notification_rule_id_parameter.clone()),
        Some("NotificationRuleUpdateBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/rules/{id}",
        "delete",
        None,
        Some(notification_rule_id_parameter),
    );
    let notification_delivery_parameters = json!([
        {"name":"page","in":"query","required":false,"schema":{"oneOf":[{"type":"integer","minimum":1},{"type":"string","pattern":"^\\s*[+]?[1-9]\\d*"}],"default":1}},
        {"name":"limit","in":"query","required":false,"schema":{"oneOf":[{"type":"integer","minimum":1},{"type":"string","pattern":"^\\s*[+]?[1-9]\\d*"}],"default":20}},
        {"name":"rule_id","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"provider_id","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"trigger_id","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"status","in":"query","required":false,"schema":{"type":"string","enum":["queued","sending","success","failed","gave_up","skipped"]}},
        {"name":"trace_id","in":"query","required":false,"schema":{"type":"string","pattern":crate::trace_id::TRACE_ID_PATTERN}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/deliveries",
        "get",
        "NotificationDeliveryListData",
        Some(notification_delivery_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/deliveries",
        "delete",
        "NotificationDeliveryClearData",
        None,
        Some("NotificationDeliveryClearBodyData"),
    );
    let notification_trigger_parameters = json!([
        {"name":"page","in":"query","required":false,"schema":{"oneOf":[{"type":"integer","minimum":1},{"type":"string","pattern":"^\\s*[+]?[1-9]\\d*"}],"default":1}},
        {"name":"limit","in":"query","required":false,"schema":{"oneOf":[{"type":"integer","minimum":1},{"type":"string","pattern":"^\\s*[+]?[1-9]\\d*"}],"default":20,"description":"Page size; positive values are capped at 100."}},
        {"name":"rule_id","in":"query","required":false,"schema":{"type":"string","minLength":1}},
        {"name":"status","in":"query","required":false,"schema":{"type":"string","enum":["created","fanout_done","partially_failed","completed"]}},
        {"name":"trace_id","in":"query","required":false,"schema":{"type":"string","pattern":crate::trace_id::TRACE_ID_PATTERN}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_notifications,
        "/api/admin/notifications/triggers",
        "get",
        "NotificationTriggerListData",
        Some(notification_trigger_parameters),
        None,
    );
    let deep_monitor_session_parameter =
        json!([{"name":"session_id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions",
        "get",
        "DeepMonitorSessionListData",
        Some(
            json!([{"name":"include_expired","in":"query","required":false,"schema":{"type":"boolean","default":false}}]),
        ),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions",
        "post",
        "DeepMonitorSessionData",
        None,
        Some("DeepMonitorStartBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}",
        "get",
        "DeepMonitorSessionData",
        Some(deep_monitor_session_parameter.clone()),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}",
        "delete",
        None,
        Some(deep_monitor_session_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/extend",
        "post",
        "DeepMonitorSessionData",
        Some(deep_monitor_session_parameter.clone()),
        Some("DeepMonitorExtendBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/stop",
        "post",
        "DeepMonitorSessionData",
        Some(deep_monitor_session_parameter.clone()),
        None,
    );
    let deep_monitor_event_parameters = json!([
        {"name":"session_id","in":"path","required":true,"schema":{"type":"string"}},
        {"name":"cursor","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"limit","in":"query","required":false,"schema":{"type":"integer","minimum":1,"maximum":200,"default":100}},
        {"name":"type","in":"query","required":false,"schema":{"type":"string","enum":["http_exchange","ws_open","ws_frame","monitor_notice"]}},
        {"name":"search","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"direction","in":"query","required":false,"schema":{"type":"string","enum":["client_to_upstream","upstream_to_client"]}},
        {"name":"method","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"status","in":"query","required":false,"schema":{"type":"integer"}},
        {"name":"client_ip","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"identity","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"path","in":"query","required":false,"schema":{"type":"string"}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/events",
        "get",
        "DeepMonitorEventListData",
        Some(deep_monitor_event_parameters),
        None,
    );
    let deep_monitor_event_parameter = json!([
        {"name":"session_id","in":"path","required":true,"schema":{"type":"string"}},
        {"name":"event_id","in":"path","required":true,"schema":{"type":"string"}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}",
        "get",
        "DeepMonitorEventData",
        Some(deep_monitor_event_parameter.clone()),
        None,
    );
    insert_typed_media_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload",
        "get",
        json!({
            "description": "Captured event payload",
            "headers": {"Content-Disposition":{"description":"Attachment disposition for streamed payloads","schema":{"type":"string"}}},
            "content": {"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}
        }),
        Some(json!([
            {"name":"session_id","in":"path","required":true,"schema":{"type":"string"}},
            {"name":"event_id","in":"path","required":true,"schema":{"type":"string"}},
            {"name":"part","in":"query","required":true,"schema":{"type":"string","minLength":1}},
            {"name":"offset","in":"query","required":false,"schema":{"type":"integer","minimum":0,"default":0}},
            {"name":"limit","in":"query","required":false,"schema":{"type":"integer","minimum":1,"maximum":262144}}
        ])),
        true,
    );
    insert_typed_media_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/live",
        "get",
        json!({"description":"Live traffic event stream","content":{"text/event-stream":{"schema":{"type":"string"}}}}),
        Some(json!([
            {"name":"session_id","in":"path","required":true,"schema":{"type":"string"}},
            {"name":"after_sequence","in":"query","required":false,"schema":{"type":"integer","minimum":0,"default":0}},
            {"name":"Last-Event-ID","in":"header","required":false,"schema":{"type":"integer","minimum":0}}
        ])),
        false,
    );
    insert_typed_media_operation(
        &mut paths,
        &typed_deep_monitor,
        "/api/admin/deep-monitor/sessions/{session_id}/download",
        "get",
        json!({
            "description":"ZIP archive attachment",
            "headers":{"Content-Disposition":{"description":"Attachment filename","schema":{"type":"string"}}},
            "content":{"application/zip":{"schema":{"type":"string","format":"binary"}}}
        }),
        Some(deep_monitor_session_parameter),
        true,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/details",
        "get",
        "WafDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/status",
        "get",
        "WafStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/config",
        "post",
        "WafDetailsData",
        None,
        Some("WafConfigUpdateData"),
    );
    for path in [
        "/api/admin/waf/manifest/refresh",
        "/api/admin/waf/system/sync",
        "/api/admin/waf/rules/recommended",
    ] {
        insert_typed_enveloped_operation(
            &mut paths,
            &typed_waf,
            path,
            "post",
            "WafDetailsData",
            None,
            None,
        );
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/rules/enabled",
        "post",
        "WafDetailsData",
        None,
        Some("WafRuleToggleBodyData"),
    );
    let waf_rule_parameters = json!([
        {"name":"source","in":"path","required":true,"schema":{"type":"string","enum":["system","custom"]}},
        {"name":"filename","in":"path","required":true,"schema":{"type":"string","pattern":"(?i)\\.conf$"}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/rules/{source}/{filename}",
        "get",
        "WafRuleFileContentData",
        Some(waf_rule_parameters),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/custom/upload",
        "post",
        "WafDetailsData",
        None,
        Some("WafUploadBodyData"),
    );
    let waf_filename_parameter = json!([{"name":"filename","in":"path","required":true,"schema":{"type":"string","pattern":"(?i)\\.conf$"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/custom/{filename}",
        "delete",
        "WafDetailsData",
        Some(waf_filename_parameter),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/events/drain",
        "post",
        "WafDrainResultData",
        None,
        None,
    );
    let waf_log_parameters = json!([
        {"name":"date","in":"query","required":false,"schema":{"type":"string","format":"date"}},
        {"name":"trace_id","in":"query","required":false,"schema":{"type":"string","pattern":crate::trace_id::TRACE_ID_PATTERN}},
        {"name":"search","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"host","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"client_ip","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"rule_id","in":"query","required":false,"schema":{"type":"string","pattern":"^\\s*[+-]?\\d+","description":"Rule ID. Legacy integer-prefix strings remain accepted."}},
        {"name":"route_type","in":"query","required":false,"schema":{"type":"string"}},
        {"name":"mode","in":"query","required":false,"schema":{"type":"string","enum":["off","detection","blocking"]}},
        {"name":"cursor","in":"query","required":false,"schema":{"type":"string","pattern":"^\\s*[+-]?\\d+","default":"0","description":"Result offset. Invalid or negative values select zero."}},
        {"name":"limit","in":"query","required":false,"schema":{"type":"string","pattern":"^\\s*[+-]?\\d+","default":"50","description":"Page size. Positive values are capped at 200; other values select 50."}}
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/logs",
        "get",
        "WafLogEntriesData",
        Some(waf_log_parameters),
        None,
    );
    let waf_trace_id_parameter =
        json!([{"name":"trace_id","in":"path","required":true,"schema":{"type":"string"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/logs/{trace_id}",
        "get",
        "WafEventData",
        Some(waf_trace_id_parameter),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_waf,
        "/api/admin/waf/logs",
        "delete",
        "WafLogDeleteData",
        None,
        Some("WafLogDeleteBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/status",
        "get",
        "SslStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/shared-files",
        "get",
        "SslSharedFilesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/shared-files/content",
        "get",
        "SslSharedFileContentData",
        Some(
            json!([{"name":"path","in":"query","required":true,"schema":{"type":"string","minLength":1}}]),
        ),
        None,
    );
    let pem_attachment = json!({
        "description":"PEM certificate attachment",
        "headers":{"Content-Disposition":{"description":"Attachment filename","schema":{"type":"string"}}},
        "content":{"application/x-pem-file":{"schema":{"type":"string","format":"binary"}}}
    });
    for path in ["/api/admin/ssl/cert.pem", "/api/admin/ssl/ca/cert.pem"] {
        insert_typed_media_operation(
            &mut paths,
            &typed_ssl,
            path,
            "get",
            pem_attachment.clone(),
            None,
            false,
        );
    }
    let zip_attachment = json!({
        "description":"ZIP archive attachment",
        "headers":{"Content-Disposition":{"description":"Attachment filename","schema":{"type":"string"}}},
        "content":{"application/zip":{"schema":{"type":"string","format":"binary"}}}
    });
    for path in [
        "/api/admin/ssl/cert.zip",
        "/api/admin/ssl/ca/server-cert.zip",
    ] {
        insert_typed_media_operation(
            &mut paths,
            &typed_ssl,
            path,
            "get",
            zip_attachment.clone(),
            None,
            false,
        );
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/status",
        "get",
        "SslCaStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/init",
        "post",
        "SslCertificateInfoData",
        None,
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca",
        "delete",
        None,
        None,
    );
    let string_array_schema = json!({"type":"array","items":{"type":"string"}});
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/hosts",
        "get",
        string_array_schema.clone(),
        None,
        None,
    );
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/hosts",
        "post",
        string_array_schema.clone(),
        None,
        Some(("SslCaHostBodyData", true)),
    );
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/hosts",
        "delete",
        string_array_schema,
        None,
        Some(("SslCaHostsDeleteBodyData", false)),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/ca/issue",
        "post",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/certificates",
        "post",
        "SslCertificateSaveData",
        None,
        Some("SslCertificateSaveBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/certificates",
        "delete",
        None,
        None,
    );
    let ssl_certificate_id_parameter =
        json!([{"name":"id","in":"path","required":true,"schema":{"type":"string","minLength":1}}]);
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/certificates/{id}",
        "delete",
        None,
        Some(ssl_certificate_id_parameter),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/activate",
        "post",
        Some("SslCertificateActivateBodyData"),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/deployment-mode",
        "post",
        "SslStatusData",
        None,
        Some("SslDeploymentModeBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl",
        "delete",
        None,
        None,
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings",
        "get",
        "ExternalCertificateBindingData",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings",
        "post",
        "ExternalCertificateBindingCredentialData",
        None,
        Some("ExternalCertificateBindingCreateBodyData"),
    );
    let external_binding_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string", "minLength": 1 }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings/{id}",
        "patch",
        "ExternalCertificateBindingData",
        Some(external_binding_id_parameter.clone()),
        Some("ExternalCertificateBindingUpdateBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings/{id}/rotate-token",
        "post",
        "ExternalCertificateBindingCredentialData",
        Some(external_binding_id_parameter.clone()),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ssl,
        "/api/admin/ssl/external-bindings/{id}",
        "delete",
        None,
        Some(external_binding_id_parameter),
    );
    let external_deploy_path = "/api/integrations/certificates/{binding_id}";
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_external_certificates,
        external_deploy_path,
        "put",
        "ExternalCertificateDeployData",
        Some(json!([{
            "name": "binding_id",
            "in": "path",
            "required": true,
            "schema": { "type": "string", "minLength": 1 }
        }])),
        Some("ExternalCertificateDeployBodyData"),
    );
    if let Some(operation) = paths
        .get_mut(external_deploy_path)
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut("put"))
    {
        operation["security"] = json!([{ "certificateDeploymentToken": [] }]);
    }
    let public_external_deploy_path = "/__certificates__/{binding_id}";
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_public_external_certificates,
        public_external_deploy_path,
        "put",
        "ExternalCertificateDeployData",
        Some(json!([{
            "name": "binding_id",
            "in": "path",
            "required": true,
            "schema": { "type": "string", "minLength": 1 }
        }])),
        Some("ExternalCertificateDeployBodyData"),
    );
    if let Some(operation) = paths
        .get_mut(public_external_deploy_path)
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut("put"))
    {
        operation["security"] = json!([{ "certificateDeploymentToken": [] }]);
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/status",
        "get",
        "DdnsStatusData",
        None,
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/toggle",
        "post",
        Some("DdnsToggleBodyData"),
        None,
    );
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/providers",
        "get",
        json!({"type":"array","items":{"$ref":"#/components/schemas/DdnsProviderData"}}),
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/settings",
        "get",
        "DdnsSettingsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/settings",
        "post",
        "DdnsSettingsData",
        None,
        Some("DdnsSettingsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/public-check/test",
        "post",
        "DdnsPublicCheckTestResultsData",
        None,
        Some("DdnsPublicCheckTestBodyData"),
    );
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/interfaces",
        "get",
        json!({"type":"array","items":{"$ref":"#/components/schemas/DdnsNetworkInterfaceData"}}),
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/interfaces/resolve",
        "post",
        "DdnsInterfaceSelectorPreviewData",
        None,
        Some("DdnsInterfaceSelectorPreviewBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/provider",
        "post",
        Some("DdnsProviderBodyData"),
        None,
    );
    let ddns_config_read_parameter = json!([{"name":"provider","in":"path","required":true,"schema":{"type":"string","minLength":1,"description":"Configuration lookup key. Unknown providers return an empty authenticated configuration object."}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/config/{provider}",
        "get",
        "DdnsConfigData",
        Some(ddns_config_read_parameter),
        None,
    );
    let ddns_config_write_parameter = json!([{"name":"provider","in":"path","required":true,"schema":{"oneOf":[{"type":"string","enum":["alidns","baiducloud","cloudflare","dnshe","dnspod","duckdns","dynu","dynv6","edgeone_cname","edgeone","esa","godaddy","huaweicloud","noip","porkbun","tencentcloud"]},{"type":"string","pattern":"^\\s*(?:alidns|baiducloud|cloudflare|dnshe|dnspod|duckdns|dynu|dynv6|edgeone_cname|edgeone|esa|godaddy|huaweicloud|noip|porkbun|tencentcloud)\\s*$"}]}}]);
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/config/{provider}",
        "post",
        Some("DdnsConfigBodyData"),
        Some(ddns_config_write_parameter),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets",
        "get",
        "DdnsTargetListData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets",
        "post",
        "DdnsTargetDetailData",
        None,
        Some("DdnsTargetBodyData"),
    );
    let ddns_target_id_parameter = json!([{"name":"id","in":"path","required":true,"schema":{"type":"string","pattern":"^[A-Za-z0-9-]{1,80}$"}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets/{id}",
        "get",
        "DdnsTargetDetailData",
        Some(ddns_target_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets/{id}",
        "put",
        "DdnsTargetDetailData",
        Some(ddns_target_id_parameter.clone()),
        Some("DdnsTargetBodyData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets/{id}",
        "delete",
        None,
        Some(ddns_target_id_parameter.clone()),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets/{id}/enabled",
        "post",
        Some("DdnsTargetEnabledBodyData"),
        Some(ddns_target_id_parameter.clone()),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/test",
        "post",
        "DdnsTestResponseData",
        None,
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/targets/{id}/test",
        "post",
        "DdnsTestResponseData",
        None,
    );
    // `Response` handlers erase extractor metadata, so restore the stable target-ID bound.
    if let Some(operation) = paths
        .get_mut("/api/admin/ddns/targets/{id}/test")
        .and_then(Value::as_object_mut)
        .and_then(|path| path.get_mut("post"))
        .and_then(Value::as_object_mut)
    {
        operation.insert("parameters".to_string(), ddns_target_id_parameter.clone());
    }
    let ddns_logs_parameter = json!([{"name":"limit","in":"query","required":false,"schema":{"oneOf":[{"type":"integer"},{"type":"string","pattern":"^\\s*[+-]?\\d+"}],"default":200,"description":"Number of retained entries. Legacy integer-prefix strings remain accepted; the result is clamped to 1–1000."}}]);
    insert_typed_json_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/logs",
        "get",
        json!({"type":"array","items":{"$ref":"#/components/schemas/DdnsLogEntryData"}}),
        Some(ddns_logs_parameter),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/logs",
        "delete",
        None,
        None,
    );
    let ddns_poll_parameter = json!([{"name":"cursor","in":"query","required":false,"schema":{"oneOf":[{"type":"integer","minimum":0},{"type":"string","pattern":"^[0-9]+$"}],"description":"Last observed log sequence. Invalid values request the retained buffer; stale or future cursors set reset=true."}}]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ddns,
        "/api/admin/ddns/poll",
        "get",
        "DdnsPollData",
        Some(ddns_poll_parameter),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_discovery,
        "/api/admin/wol/discover/jobs/{id}",
        "delete",
        "WolDiscoveryJobData",
        None,
        None,
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets",
        "get",
        "TerminalTarget",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets",
        "post",
        "TerminalTarget",
        None,
        Some("TargetCreateInput"),
    );
    for (method, response_schema, request_schema) in [
        ("get", "TerminalTarget", None),
        ("patch", "TerminalTarget", Some("TargetUpdateInput")),
    ] {
        insert_typed_enveloped_operation(
            &mut paths,
            &typed_terminal_runtime,
            "/api/admin/terminal/targets/{id}",
            method,
            response_schema,
            None,
            request_schema,
        );
    }
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets/{id}",
        "delete",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets/probe-host-key",
        "post",
        "HostKeyProbeResult",
        None,
        Some("ProbeHostKeyInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets/test-connection",
        "post",
        "ConnectionTestResult",
        None,
        Some("TerminalTestConnectionInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/sessions",
        "get",
        "SessionListResult",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/targets/{id}/sessions",
        "post",
        "TerminalSession",
        None,
        Some("CreateSessionInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/sessions/{id}",
        "patch",
        "TerminalSession",
        None,
        Some("RenameSessionInput"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/sessions/{id}",
        "delete",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/sessions/{id}/attachments",
        "post",
        "TerminalAttachment",
        None,
        Some("CreateAttachmentInput"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/attachments/{id}/events",
        "get",
        "EventsResult",
        Some(json!([
            {
                "name": "id",
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            },
            {
                "name": "after",
                "in": "query",
                "required": false,
                "schema": { "type": "integer", "minimum": 0, "default": 0 }
            },
            {
                "name": "timeoutMs",
                "in": "query",
                "required": false,
                "schema": {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 5_000,
                    "default": 4_500
                }
            }
        ])),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/attachments/{id}/input",
        "post",
        Some("InputRequest"),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/attachments/{id}/resize",
        "post",
        "TerminalSession",
        None,
        Some("ResizeRequest"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/attachments/{id}/control",
        "post",
        "TerminalAttachment",
        None,
        Some("ClaimControlRequest"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_terminal_runtime,
        "/api/admin/terminal/attachments/{id}",
        "delete",
        None,
        None,
    );
    for (method, path) in [
        ("get", "/api/admin/terminal/targets"),
        ("post", "/api/admin/terminal/targets"),
        ("get", "/api/admin/terminal/targets/{id}"),
        ("patch", "/api/admin/terminal/targets/{id}"),
        ("delete", "/api/admin/terminal/targets/{id}"),
        ("post", "/api/admin/terminal/targets/probe-host-key"),
        ("post", "/api/admin/terminal/targets/test-connection"),
        ("get", "/api/admin/terminal/sessions"),
        ("post", "/api/admin/terminal/targets/{id}/sessions"),
        ("patch", "/api/admin/terminal/sessions/{id}"),
        ("delete", "/api/admin/terminal/sessions/{id}"),
        ("post", "/api/admin/terminal/sessions/{id}/attachments"),
        ("get", "/api/admin/terminal/attachments/{id}/events"),
        ("post", "/api/admin/terminal/attachments/{id}/input"),
        ("post", "/api/admin/terminal/attachments/{id}/resize"),
        ("post", "/api/admin/terminal/attachments/{id}/control"),
        ("delete", "/api/admin/terminal/attachments/{id}"),
    ] {
        set_operation_error_schema(&mut paths, path, method, "TerminalErrorEnvelope");
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dnsmasq_assets,
        "/api/admin/system/dnsmasq/install",
        "post",
        "DnsmasqInstallStateData",
        None,
        None,
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_security_overview,
        "/api/admin/security/overview",
        "get",
        "SecurityOverviewData",
        Some(json!([{
            "name": "rangeSec",
            "in": "query",
            "required": false,
            "schema": { "type": "integer", "minimum": 60, "maximum": 2_592_000 }
        }])),
        None,
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/config/dashboard_display",
        "get",
        "DashboardDisplayData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/config/dashboard_display",
        "post",
        "DashboardDisplayData",
        None,
        Some("DashboardDisplayUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/dashboard/stats",
        "get",
        "DashboardStatsData",
        Some(json!([
            {
                "name": "rangeSec",
                "in": "query",
                "required": false,
                "schema": { "type": "integer", "minimum": 60, "maximum": 2_592_000 }
            },
            {
                "name": "userId",
                "in": "query",
                "required": false,
                "schema": { "type": "string" }
            },
            {
                "name": "host",
                "in": "query",
                "required": false,
                "schema": { "type": "string" }
            },
            {
                "name": "stream",
                "in": "query",
                "required": false,
                "schema": { "type": "string" }
            }
        ])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/dashboard/realtime",
        "get",
        "DashboardRealtimeData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/dashboard/active-ips",
        "get",
        "DashboardActiveIpsData",
        Some(json!([{
            "name": "host",
            "in": "query",
            "required": true,
            "schema": { "type": "string" }
        }])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_dashboard,
        "/api/admin/dashboard/stream-active-ips",
        "get",
        "DashboardStreamActiveIpsData",
        Some(json!([{
            "name": "stream",
            "in": "query",
            "required": true,
            "schema": { "type": "string" }
        }])),
        None,
    );

    for (path, method) in [
        ("/api/admin/update/status", "get"),
        ("/api/admin/update/check", "post"),
        ("/api/admin/update/check-and-download", "post"),
        ("/api/admin/update/download", "post"),
    ] {
        insert_typed_enveloped_operation(
            &mut paths,
            &typed_update,
            path,
            method,
            "UpdateStatusData",
            None,
            None,
        );
    }
    insert_typed_message_operation(
        &mut paths,
        &typed_update,
        "/api/admin/update/install",
        "post",
        None,
    );
    insert_typed_nullable_enveloped_operation(
        &mut paths,
        &typed_update,
        "/api/admin/update/confirm",
        "get",
        "UpdateConfirmData",
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_cidr,
        "/api/admin/cidr/capabilities",
        "get",
        "CidrCapabilitiesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_cidr,
        "/api/admin/cidr/provinces",
        "get",
        "CidrProvincesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_cidr,
        "/api/admin/cidr/cities",
        "get",
        "CidrCitiesData",
        Some(json!([{
            "name": "province",
            "in": "query",
            "required": true,
            "schema": { "type": "string" }
        }])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_cidr,
        "/api/admin/cidr/selector",
        "get",
        "CidrSelectorData",
        Some(json!([{
            "name": "province",
            "in": "query",
            "required": false,
            "schema": { "type": "string" }
        }])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_cidr,
        "/api/admin/cidr/cidrs",
        "get",
        "CidrLookupData",
        Some(json!([
            {
                "name": "province",
                "in": "query",
                "required": true,
                "schema": { "type": "string" }
            },
            {
                "name": "city",
                "in": "query",
                "required": false,
                "schema": { "type": "string" }
            },
            {
                "name": "operator",
                "in": "query",
                "required": false,
                "schema": { "type": "string", "enum": ["电信", "联通", "移动"] }
            }
        ])),
        None,
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ip_location,
        "/api/admin/ip-location/batch",
        "post",
        "IpLocationBatchData",
        None,
        Some("IpLocationBatchBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ip_location_config,
        "/api/admin/config/ip_location_api",
        "get",
        "IpLocationApiConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_ip_location_config,
        "/api/admin/config/ip_location_api",
        "post",
        "IpLocationApiConfigData",
        None,
        Some("IpLocationApiConfigData"),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_ip_location_config,
        "/api/admin/config/ip_location_api/test-ip-lookup",
        "post",
        "IpLocationConnectionTestData",
        Some("IpLocationTestUrlBodyData"),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_ip_location_config,
        "/api/admin/config/ip_location_api/test-cidr",
        "post",
        "CidrConnectionTestData",
        Some("IpLocationTestUrlBodyData"),
    );

    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_backoff,
        "/api/admin/backoff/list",
        "get",
        "LoginBackoffData",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backoff,
        "/api/admin/backoff/status",
        "get",
        "LoginBackoffData",
        Some(json!([{
            "name": "ip",
            "in": "query",
            "required": true,
            "schema": { "type": "string", "minLength": 1 }
        }])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backoff,
        "/api/admin/backoff/reset",
        "post",
        "LoginBackoffResetData",
        None,
        Some("LoginBackoffResetBodyData"),
    );

    insert_typed_direct_operation(
        &mut paths,
        &typed_internal_events,
        "/api/internal/system-events",
        "post",
        "SystemEventPublishResultData",
        Some("SystemEventPublishBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_admin_events,
        "/api/admin/events",
        "get",
        "SystemEventListData",
        Some(json!([
            { "name": "page", "in": "query", "required": false, "schema": { "type": "integer", "minimum": 1 } },
            { "name": "limit", "in": "query", "required": false, "schema": { "type": "string", "pattern": "^[1-9][0-9]*$" } },
            { "name": "search", "in": "query", "required": false, "schema": { "type": "string" } },
            { "name": "trace_id", "in": "query", "required": false, "schema": { "type": "string", "pattern": crate::trace_id::TRACE_ID_PATTERN } },
            { "name": "type", "in": "query", "required": false, "schema": { "type": "string", "enum": crate::events::SYSTEM_EVENT_TYPES } },
            { "name": "level", "in": "query", "required": false, "schema": { "type": "string", "enum": crate::events::SYSTEM_EVENT_LEVELS } },
            { "name": "source", "in": "query", "required": false, "schema": { "type": "string", "enum": crate::events::SYSTEM_EVENT_SOURCES } }
        ])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_traces,
        "/api/admin/traces/{trace_id}",
        "get",
        "TraceLookupData",
        Some(json!([{
            "name": "trace_id",
            "in": "path",
            "required": true,
            "schema": {
                "type": "string",
                "pattern": crate::trace_id::TRACE_ID_PATTERN
            }
        }])),
        None,
    );
    add_standard_error_response(
        &mut paths,
        "/api/admin/traces/{trace_id}",
        "get",
        "400",
        "Invalid Trace ID",
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_admin_events,
        "/api/admin/events",
        "delete",
        Some("SystemEventDeleteBodyData"),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_admin_events,
        "/api/admin/events/clear",
        "delete",
        "SystemEventClearData",
        None,
        None,
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health",
        "get",
        "RuntimeHealthSnapshotData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/gateway-memory",
        "get",
        "GatewayMemoryConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/gateway-memory",
        "put",
        "GatewayMemoryConfigData",
        None,
        Some("GatewayMemoryConfigUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/gateway-memory/reclaim",
        "post",
        "GatewayMemoryReclaimData",
        None,
        Some("GatewayMemoryReclaimBodyData"),
    );
    let runtime_log_parameters = json!([
        { "name": "component", "in": "path", "required": true, "schema": { "type": "string", "enum": ["management", "gateway_process"] } },
        { "name": "limit", "in": "query", "required": false, "schema": { "type": "integer", "minimum": 1, "maximum": 500 } }
    ]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/logs/{component}",
        "get",
        "RuntimeComponentLogsData",
        Some(runtime_log_parameters.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/logs/{component}",
        "delete",
        "RuntimeLogClearData",
        Some(json!([{
            "name": "component",
            "in": "path",
            "required": true,
            "schema": { "type": "string", "enum": ["management", "gateway_process"] }
        }])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/diagnostics",
        "get",
        "RuntimeDiagnosticsData",
        None,
        None,
    );
    insert_typed_binary_operation(
        &mut paths,
        &typed_runtime_health,
        "/api/admin/runtime-health/diagnostics/archive",
        "get",
        "application/zip",
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_general_blacklist,
        "/api/admin/general-blacklist",
        "get",
        "GeneralBlacklistListData",
        Some(json!([
            { "name": "page", "in": "query", "required": false, "schema": { "type": "integer", "minimum": 1 } },
            { "name": "limit", "in": "query", "required": false, "schema": { "type": "string", "pattern": "^[1-9][0-9]*$" } },
            { "name": "search", "in": "query", "required": false, "schema": { "type": "string" } }
        ])),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_general_blacklist,
        "/api/admin/general-blacklist",
        "post",
        "GeneralBlacklistMutationData",
        None,
        Some("GeneralBlacklistAddBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_general_blacklist,
        "/api/admin/general-blacklist",
        "delete",
        "GeneralBlacklistMutationData",
        None,
        Some("IpListBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_general_blacklist,
        "/api/admin/general-blacklist/status",
        "post",
        "GeneralBlacklistStatusData",
        None,
        Some("IpListBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_general_blacklist,
        "/api/admin/general-blacklist/{ip}",
        "delete",
        "GeneralBlacklistMutationData",
        Some(json!([{
            "name": "ip",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
        }])),
        None,
    );

    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/settings",
        "get",
        "ScannerSettingsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/settings",
        "post",
        "ScannerSettingsData",
        None,
        Some("ScannerSettingsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/path-whitelist",
        "get",
        "ScannerPathWhitelistData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/path-whitelist",
        "put",
        "ScannerPathWhitelistData",
        None,
        Some("ScannerPathWhitelistUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/path-whitelist/false-positive",
        "post",
        "ScannerFalsePositiveResultData",
        None,
        Some("ScannerFalsePositiveBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/blacklist",
        "get",
        "ScannerBlacklistListData",
        Some(json!([
            { "name": "page", "in": "query", "required": false, "schema": { "type": "integer", "minimum": 1 } },
            { "name": "limit", "in": "query", "required": false, "schema": { "type": "string", "pattern": "^[1-9][0-9]*$" } },
            { "name": "search", "in": "query", "required": false, "schema": { "type": "string" } }
        ])),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/blacklist",
        "delete",
        Some("IpListBodyData"),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/blacklist/{ip}",
        "get",
        "ScannerBlacklistRecordData",
        Some(json!([{
            "name": "ip",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
        }])),
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_scanner,
        "/api/admin/scanner/blacklist/{ip}",
        "delete",
        None,
        Some(json!([{
            "name": "ip",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
        }])),
    );

    for (path, response_schema, request_schema) in [
        (
            "/api/admin/config/gateway",
            "GatewaySettingsData",
            "GatewaySettingsUpdateData",
        ),
        (
            "/api/admin/config/gateway/visibility",
            "GatewayVisibilityDetailsData",
            "GatewayVisibilityUpdateData",
        ),
        (
            "/api/admin/config/gateway/proxy-headers",
            "GatewayProxyHeadersDetailsData",
            "GatewayProxyHeadersUpdateData",
        ),
        (
            "/api/admin/config/gateway/host-response",
            "GatewayHostResponseDetailsData",
            "GatewayHostResponseUpdateData",
        ),
        (
            "/api/admin/config/gateway/proxy-protocol",
            "GatewayProxyProtocolData",
            "GatewayProxyProtocolUpdateData",
        ),
    ] {
        insert_typed_enveloped_operation(
            &mut paths,
            &typed_gateway_settings,
            path,
            "get",
            response_schema,
            None,
            None,
        );
        insert_typed_enveloped_operation(
            &mut paths,
            &typed_gateway_settings,
            path,
            "post",
            response_schema,
            None,
            Some(request_schema),
        );
    }
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_certificate_sync,
        "/api/admin/config/fnos_certificate_sync/details",
        "get",
        "FnosCertificateSyncDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_certificate_sync,
        "/api/admin/config/fnos_certificate_sync",
        "post",
        "FnosCertificateSyncDetailsData",
        None,
        Some("FnosCertificateSyncUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_certificate_sync,
        "/api/admin/config/fnos_certificate_sync/sync",
        "post",
        "FnosCertificateSyncResponseData",
        None,
        Some("FnosCertificateSyncBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_port_icon_hijack,
        "/api/admin/config/fnos_port_icon_hijack",
        "get",
        "FnosPortIconHijackData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_port_icon_hijack,
        "/api/admin/config/fnos_port_icon_hijack",
        "post",
        "FnosPortIconHijackData",
        None,
        Some("FnosPortIconHijackUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_network_tuning,
        "/api/admin/config/fnos_network_tuning",
        "get",
        "FnosNetworkTuningData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_network_tuning,
        "/api/admin/config/fnos_network_tuning",
        "post",
        "FnosNetworkTuningData",
        None,
        Some("FnosNetworkTuningUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_connect_waf,
        "/api/admin/config/fnos_connect_waf",
        "get",
        "FnosConnectWafData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_connect_waf,
        "/api/admin/config/fnos_connect_waf",
        "post",
        "FnosConnectWafData",
        None,
        Some("FnosConnectWafUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_share_bypass,
        "/api/admin/config/fnos_share_bypass",
        "get",
        "FnosShareBypassData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_fnos_share_bypass,
        "/api/admin/config/fnos_share_bypass",
        "post",
        "FnosShareBypassData",
        None,
        Some("FnosShareBypassUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_smart_connect,
        "/api/admin/config/smart_connect/details",
        "get",
        "SmartConnectDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_smart_connect,
        "/api/admin/config/smart_connect",
        "post",
        "SmartConnectDetailsData",
        None,
        Some("SmartConnectUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_proxy_protocol_force,
        "/api/admin/config/proxy_protocol_force",
        "get",
        "ProxyProtocolForceData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_proxy_protocol_force,
        "/api/admin/config/proxy_protocol_force",
        "post",
        "ProxyProtocolForceData",
        None,
        Some("ProxyProtocolForceData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_run_mode_prompt_preferences,
        "/api/admin/config/run_mode_prompt_preferences",
        "get",
        "RunModePromptPreferencesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_run_mode_prompt_preferences,
        "/api/admin/config/run_mode_prompt_preferences",
        "post",
        "RunModePromptPreferencesData",
        None,
        Some("RunModePromptPreferencesUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_protocol_mapping_feature,
        "/api/admin/config/protocol_mapping_feature",
        "get",
        "ProtocolMappingFeatureData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_protocol_mapping_feature,
        "/api/admin/config/protocol_mapping_feature",
        "post",
        "ProtocolMappingFeatureData",
        None,
        Some("ProtocolMappingFeatureUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auto_https,
        "/api/admin/config/auto_https",
        "get",
        "AutoHttpsDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auto_https,
        "/api/admin/config/auto_https",
        "post",
        "AutoHttpsDetailsData",
        None,
        Some("AutoHttpsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_default_route,
        "/api/admin/config/default_route",
        "get",
        "DefaultRouteData",
        None,
        None,
    );
    insert_typed_message_operation(
        &mut paths,
        &typed_default_route,
        "/api/admin/config/default_route",
        "post",
        Some("DefaultRouteUpdateData"),
    );
    insert_typed_message_operation(
        &mut paths,
        &typed_default_route,
        "/api/admin/config/default_tunnel",
        "post",
        Some("DefaultTunnelUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_captcha,
        "/api/admin/config/captcha",
        "get",
        "CaptchaSettingsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_captcha,
        "/api/admin/config/captcha",
        "post",
        "CaptchaSettingsData",
        None,
        Some("CaptchaSettingsUpdateData"),
    );
    insert_typed_message_operation(
        &mut paths,
        &typed_run_type,
        "/api/admin/config/run_type",
        "post",
        Some("RunTypeUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_feature,
        "/api/admin/config/wol_feature",
        "get",
        "WolFeatureConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_wol_feature,
        "/api/admin/config/wol_feature",
        "post",
        "WolFeatureConfigData",
        None,
        Some("WolFeatureConfigUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_firewall_runtime,
        "/api/admin/config/auto_manage_firewall",
        "post",
        "AutoManageFirewallData",
        None,
        Some("AutoManageFirewallUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_firewall_runtime,
        "/api/admin/config/firewall_additional_ports",
        "get",
        "FirewallAdditionalPortsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_config,
        "/api/admin/config",
        "get",
        "ApplicationConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_config,
        "/api/admin/config/locale",
        "get",
        "LocaleConfigData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_config,
        "/api/admin/config/locale",
        "post",
        "LocaleConfigData",
        None,
        Some("LocaleConfigData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_config,
        "/api/admin/config/appearance",
        "get",
        "PanelAppearanceData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_config,
        "/api/admin/config/appearance",
        "post",
        "PanelAppearanceData",
        None,
        Some("PanelAppearanceData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_mode,
        "/api/admin/auth/mode",
        "get",
        "AuthModeStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_mode,
        "/api/admin/auth/mode/preview",
        "post",
        "AuthModePreviewData",
        None,
        Some("AuthLoginModeBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts",
        "get",
        "AuthAccountsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts",
        "post",
        "AuthAccountData",
        None,
        Some("AuthAccountCreateBody"),
    );
    let auth_account_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}",
        "patch",
        "AuthAccountData",
        Some(auth_account_id_parameter.clone()),
        Some("AuthAccountPatchBody"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}",
        "delete",
        None,
        Some(auth_account_id_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/password",
        "post",
        "AuthAccountData",
        Some(auth_account_id_parameter.clone()),
        Some("AuthAccountPasswordBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/setup",
        "post",
        "AuthAccountData",
        Some(auth_account_id_parameter.clone()),
        Some("AuthAccountSetupBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/totp/setup",
        "post",
        "TotpSetupData",
        Some(auth_account_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/totp/bind",
        "post",
        "AuthAccountData",
        Some(auth_account_id_parameter.clone()),
        Some("TotpBindBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/access-scopes",
        "patch",
        "AuthAccountData",
        Some(auth_account_id_parameter.clone()),
        Some("AccessScopesUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_accounts,
        "/api/admin/auth/accounts/{id}/subdomain-access",
        "patch",
        "AuthAccountData",
        Some(auth_account_id_parameter),
        Some("SubdomainAccessUpdateData"),
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_sessions,
        "/api/admin/sessions",
        "get",
        "SessionRecordData",
    );
    let session_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_sessions,
        "/api/admin/sessions/{id}",
        "get",
        "SessionRecordData",
        Some(session_id_parameter.clone()),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/automatic",
        "get",
        "AutomaticBackupDetailsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/automatic",
        "put",
        "AutomaticBackupDetailsData",
        None,
        Some("UpdateAutomaticBackupBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/automatic/files",
        "get",
        "AutomaticBackupFilesData",
        None,
        None,
    );
    insert_typed_binary_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/export",
        "get",
        "application/octet-stream",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/files",
        "get",
        "BackupDirectoryFilesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/export/fnos",
        "post",
        "BackupDirectoryExportData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/import",
        "post",
        "BackupImportResultData",
        None,
        Some("ImportBackupBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/import/automatic",
        "post",
        "BackupImportResultData",
        None,
        Some("ImportBackupFromDirectoryBody"),
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings",
        "get",
        "HostMappingData",
    );
    insert_typed_array_enveloped_request_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings",
        "post",
        "HostMappingData",
        "MappingsBody",
    );
    add_success_response_headers(
        &mut paths,
        "/api/admin/config/host_mappings",
        "get",
        &[crate::proxy_config::HOST_MAPPINGS_REVISION_HEADER],
    );
    add_success_response_headers(
        &mut paths,
        "/api/admin/config/host_mappings",
        "post",
        &[crate::proxy_config::HOST_MAPPINGS_REVISION_HEADER],
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mapping_catalog",
        "get",
        "HostMappingCatalogData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mapping_catalog",
        "post",
        "HostMappingCatalogData",
        None,
        Some("HostMappingCatalogBody"),
    );
    for method in ["get", "post"] {
        add_success_response_headers(
            &mut paths,
            "/api/admin/config/host_mapping_catalog",
            method,
            &[
                crate::proxy_config::HOST_MAPPING_CATALOG_REVISION_HEADER,
                crate::proxy_config::HOST_MAPPINGS_REVISION_HEADER,
            ],
        );
    }
    insert_typed_direct_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/basic_auth_probe",
        "post",
        "HostMappingBasicAuthProbeData",
        Some("HostMappingBasicAuthProbeBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/static_path_probe",
        "post",
        "StaticPathProbeResultData",
        None,
        Some("StaticPathProbeBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/static_path_browse",
        "post",
        "StaticPathBrowseResultData",
        None,
        Some("StaticPathBrowseBodyData"),
    );
    insert_typed_html_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/bookmarks/export",
        "get",
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/metadata",
        "post",
        "HostMappingMetadataData",
        Some("HostMappingMetadataBodyData"),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/refresh_titles",
        "post",
        "HostMappingRefreshSummaryData",
        None,
    );
    let host_mapping_parameter = json!([{
        "name": "host",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_direct_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/{host}/advanced_auth",
        "get",
        "AdvancedAuthDetailsData",
        None,
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_host_mappings,
        "/api/admin/config/host_mappings/{host}/advanced_auth",
        "put",
        "AdvancedAuthDetailsData",
        Some("AdvancedAuthUpdateBodyData"),
    );
    for method in ["get", "put"] {
        if let Some(operation) = paths
            .get_mut("/api/admin/config/host_mappings/{host}/advanced_auth")
            .and_then(Value::as_object_mut)
            .and_then(|path| path.get_mut(method))
        {
            operation["parameters"] = host_mapping_parameter.clone();
        }
    }
    insert_typed_array_enveloped_request_operation(
        &mut paths,
        &typed_proxy_routing,
        "/api/admin/config/proxy_mappings",
        "post",
        "ProxyMappingData",
        "ProxyMappingsUpdateData",
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_proxy_routing,
        "/api/admin/config/stream_mappings",
        "get",
        "StreamMappingData",
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_proxy_routing,
        "/api/admin/config/stream_mappings",
        "post",
        Some("StreamMappingsUpdateData"),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_proxy_routing,
        "/api/admin/config/subdomain_mode",
        "get",
        "SubdomainModeData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_proxy_routing,
        "/api/admin/config/subdomain_mode",
        "post",
        "SubdomainModeResponseData",
        None,
        Some("SubdomainModeUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_maintenance_data,
        "/api/admin/maintenance/data/clear",
        "post",
        "MaintenanceClearData",
        None,
        Some("MaintenanceClearBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_backup,
        "/api/admin/maintenance/backup/import/fnos",
        "post",
        "BackupImportResultData",
        None,
        Some("ImportBackupFromDirectoryBody"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_sessions,
        "/api/admin/sessions/{id}",
        "delete",
        None,
        Some(session_id_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_sessions,
        "/api/admin/sessions/{id}/comment",
        "patch",
        "SessionRecordData",
        Some(session_id_parameter.clone()),
        Some("SessionCommentBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_sessions,
        "/api/admin/sessions/{id}/mobility",
        "get",
        "SessionMobilityDetailsData",
        Some(session_id_parameter),
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_credential_settings,
        "/api/admin/config/auth_credential_settings",
        "get",
        "AuthCredentialSettingsData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_credential_settings,
        "/api/admin/config/auth_credential_settings",
        "post",
        "AuthCredentialSettingsData",
        None,
        Some("AuthCredentialSettingsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_session,
        "/api/admin/panel/bootstrap",
        "get",
        "PanelBootstrapData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_session,
        "/api/admin/panel/password",
        "post",
        "PanelBootstrapData",
        None,
        Some("PanelPasswordBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_session,
        "/api/admin/panel/password/change",
        "post",
        "PanelBootstrapData",
        None,
        Some("PanelPasswordBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_session,
        "/api/admin/panel/login",
        "post",
        "PanelBootstrapData",
        None,
        Some("PanelLoginBodyData"),
    );
    add_panel_login_rate_limit_response(&mut paths);
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_totp_bootstrap,
        "/api/admin/totp/status",
        "get",
        "TotpStatusData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_totp_bootstrap,
        "/api/admin/totp/setup",
        "post",
        "TotpSetupData",
        None,
        None,
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_totp_bootstrap,
        "/api/admin/totp/bind",
        "post",
        Some("TotpBindBody"),
        None,
    );
    let totp_id_parameter = json!([{
        "name": "id",
        "in": "path",
        "required": true,
        "schema": { "type": "string" }
    }]);
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/{id}",
        "delete",
        None,
        Some(totp_id_parameter.clone()),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/{id}/access-scopes",
        "patch",
        "TotpCredentialData",
        Some(totp_id_parameter.clone()),
        Some("AccessScopesUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/{id}/subdomain-access",
        "patch",
        "TotpCredentialData",
        Some(totp_id_parameter.clone()),
        Some("SubdomainAccessUpdateData"),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/{id}/comment",
        "patch",
        Some("TotpCommentBody"),
        Some(totp_id_parameter),
    );
    insert_typed_empty_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/passkeys/{id}",
        "delete",
        None,
        Some(json!([{
            "name": "id",
            "in": "path",
            "required": true,
            "schema": { "type": "string" }
        }])),
    );
    insert_typed_direct_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/credentials/export",
        "get",
        "CredentialTransferData",
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/credentials/import",
        "post",
        "CredentialImportSummaryData",
        None,
        Some("CredentialImportBodyData"),
    );
    insert_typed_array_enveloped_operation(
        &mut paths,
        &typed_totp_management,
        "/api/admin/totp/{totp_id}/passkeys",
        "get",
        "PasskeyCredentialData",
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_panel_session,
        "/api/admin/panel/logout",
        "post",
        "PanelBootstrapData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_auth_mode,
        "/api/admin/auth/mode/switch",
        "post",
        "AuthModeStatusData",
        None,
        Some("AuthLoginModeBody"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_sync_routes,
        "/api/admin/sync-routes",
        "post",
        "SyncRoutesData",
        None,
        None,
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_firewall_runtime,
        "/api/admin/config/firewall_additional_ports",
        "post",
        "FirewallAdditionalPortsData",
        None,
        Some("FirewallAdditionalPortsUpdateData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_firewall_runtime,
        "/api/admin/firewall/reset",
        "post",
        "FirewallResetData",
        None,
        Some("FirewallResetBodyData"),
    );
    insert_typed_enveloped_operation(
        &mut paths,
        &typed_firewall_runtime,
        "/api/admin/firewall/clear",
        "post",
        "FirewallClearData",
        None,
        None,
    );
    domain_contracts::apply(&mut paths);
    promote_typed_operations(
        &mut paths,
        &typed_cloudflared,
        &[
            ("/api/admin/cloudflared/status", "get"),
            ("/api/admin/cloudflared/config", "get"),
            ("/api/admin/cloudflared/config", "post"),
            ("/api/admin/cloudflared/start", "post"),
            ("/api/admin/cloudflared/stop", "post"),
            ("/api/admin/cloudflared/logs", "get"),
            ("/api/admin/cloudflared/logs", "delete"),
            ("/api/admin/cloudflared/poll", "get"),
            ("/api/admin/cloudflared/cloudflare/credential", "put"),
            ("/api/admin/cloudflared/cloudflare/credential", "delete"),
            ("/api/admin/cloudflared/cloudflare/state", "get"),
            ("/api/admin/cloudflared/reconcile/preview", "post"),
            ("/api/admin/cloudflared/reconcile/apply", "post"),
            ("/api/admin/cloudflared/reconcile/jobs/active", "get"),
            ("/api/admin/cloudflared/reconcile/jobs/{id}", "get"),
            (
                "/api/admin/cloudflared/reconcile/jobs/by-plan/{plan_id}",
                "get",
            ),
            ("/api/admin/cloudflared/optimization/scans", "post"),
            ("/api/admin/cloudflared/optimization/scans/{id}", "get"),
            ("/api/admin/cloudflared/optimization/scans/{id}", "delete"),
            ("/api/admin/cloudflared/optimization/apply", "post"),
            ("/api/admin/cloudflared/optimization/fallback", "post"),
            ("/api/admin/cloudflared/optimization/settings", "put"),
            (
                "/api/admin/cloudflared/optimization/domains/{hostname}",
                "put",
            ),
        ],
    );
    promote_typed_operations(
        &mut paths,
        &typed_frpc,
        &[
            ("/api/admin/frpc/status", "get"),
            ("/api/admin/frpc/overview", "get"),
            ("/api/admin/frpc/web-status", "get"),
            ("/api/admin/frpc/config", "get"),
            ("/api/admin/frpc/config", "post"),
            ("/api/admin/frpc/start", "post"),
            ("/api/admin/frpc/stop", "post"),
            ("/api/admin/frpc/logs", "get"),
            ("/api/admin/frpc/logs", "delete"),
            ("/api/admin/frpc/poll", "get"),
            ("/api/admin/frpc/instances", "get"),
            ("/api/admin/frpc/instances", "post"),
            ("/api/admin/frpc/instances/draft", "post"),
            ("/api/admin/frpc/instances/{id}", "get"),
            ("/api/admin/frpc/instances/{id}", "put"),
            ("/api/admin/frpc/instances/{id}", "delete"),
            ("/api/admin/frpc/instances/{id}/start", "post"),
            ("/api/admin/frpc/instances/{id}/stop", "post"),
            ("/api/admin/frpc/instances/{id}/restart", "post"),
            ("/api/admin/frpc/instances/{id}/logs", "get"),
            ("/api/admin/frpc/instances/{id}/logs", "delete"),
            ("/api/admin/frpc/instances/{id}/poll", "get"),
        ],
    );
    promote_typed_operations(
        &mut paths,
        &typed_acme,
        &[
            ("/api/admin/acme", "delete"),
            ("/api/admin/acme/status", "get"),
            ("/api/admin/acme/resource/status", "get"),
            ("/api/admin/acme/resource/initialize", "post"),
            ("/api/admin/acme/resource/cancel", "post"),
            ("/api/admin/acme/resource", "delete"),
            ("/api/admin/acme/overview", "get"),
            ("/api/admin/acme/dns-providers", "get"),
            ("/api/admin/acme/subdomain-recommendation", "get"),
            ("/api/admin/acme/init", "post"),
            ("/api/admin/acme/client-settings", "post"),
            ("/api/admin/acme/config", "get"),
            ("/api/admin/acme/config", "post"),
            ("/api/admin/acme/applications", "get"),
            ("/api/admin/acme/applications", "post"),
            ("/api/admin/acme/applications/{id}", "get"),
            ("/api/admin/acme/applications/{id}", "patch"),
            ("/api/admin/acme/applications/{id}", "delete"),
            ("/api/admin/acme/applications/{id}/certificate", "delete"),
            ("/api/admin/acme/applications/{id}/library/sync", "post"),
            ("/api/admin/acme/applications/{id}/deploy", "post"),
            ("/api/admin/acme/applications/{id}/request", "post"),
            ("/api/admin/acme/request", "post"),
            ("/api/admin/acme/jobs/active/stop", "post"),
            ("/api/admin/acme/jobs/{id}", "get"),
            ("/api/admin/acme/jobs/{id}/logs", "get"),
            ("/api/admin/acme/jobs/{id}/poll", "get"),
            ("/api/admin/acme/certs/{domain}", "get"),
            ("/api/admin/acme/certs/{domain}", "delete"),
            ("/api/admin/acme/certs/{domain}/download", "get"),
            ("/api/admin/acme/certs/{domain}/deploy", "post"),
        ],
    );
    baseline_docs::apply(&mut paths);
    ssl_docs::apply(&mut paths, &mut components);
    let mut tags = baseline_docs::tags();
    tags.push(ssl_docs::tag());
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "fn-knock server-admin API",
            "version": APP_LOCAL_VERSION,
            "description": "server-admin 7998 端口提供的完整管理端接口契约。路径、请求与响应均由 utoipa schema 和领域 operation 清单生成。"
        },
        "servers": [
            {
                "url": "/",
                "description": "server-admin (port 7998)"
            }
        ],
        "tags": tags,
        "paths": Value::Object(paths),
        "components": Value::Object(components)
    })
}

fn typed_operation(document: &utoipa::openapi::OpenApi, path: &str, method: &str) -> Option<Value> {
    serde_json::to_value(document)
        .ok()
        .and_then(|value| value.get("paths")?.get(path)?.get(method).cloned())
}

/// Retain the compatibility schemas curated for dynamic JSON handlers while
/// proving that the operation itself is emitted by the production router.
fn promote_typed_operations(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    operations: &[(&str, &str)],
) {
    for (path, method) in operations {
        let Some(typed) = typed_operation(document, path, method) else {
            continue;
        };
        let Some(operation) = paths
            .get_mut(*path)
            .and_then(Value::as_object_mut)
            .and_then(|item| item.get_mut(*method))
            .and_then(Value::as_object_mut)
        else {
            continue;
        };
        for key in ["operationId", "summary", "tags"] {
            if let Some(value) = typed.get(key) {
                operation.insert(key.to_string(), value.clone());
            }
        }
        operation.insert("x-fn-knock-contract-source".to_string(), json!("utoipa"));
    }
}

fn insert_typed_operation(
    paths: &mut Map<String, Value>,
    path: &str,
    method: &str,
    operation: Value,
) {
    paths
        .entry(path.to_string())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("typed path is generated as an object")
        .insert(method.to_string(), operation);
}

fn insert_typed_enveloped_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    data_schema: &str,
    parameters: Option<Value>,
    request_schema: Option<&str>,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["success", "data"],
                        "properties": {
                            "success": { "type": "boolean", "const": true },
                            "message": { "type": ["string", "null"] },
                            "data": { "$ref": format!("#/components/schemas/{data_schema}") }
                        },
                        "additionalProperties": true
                    }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    if let Some(parameters) = parameters {
        operation["parameters"] = parameters;
    }
    if let Some(request_schema) = request_schema {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
                }
            }
        });
    }
    insert_typed_operation(paths, path, method, operation);
}

fn add_panel_login_rate_limit_response(paths: &mut Map<String, Value>) {
    let Some(responses) = paths
        .get_mut("/api/admin/panel/login")
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut("post"))
        .and_then(|operation| operation.get_mut("responses"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    responses.insert(
        "429".to_string(),
        json!({
            "description": "Login rejected with an exponential retry delay",
            "headers": {
                "Retry-After": {
                    "description": "Seconds until the next login attempt",
                    "schema": { "type": "integer", "minimum": 1 }
                }
            },
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/PanelLoginRateLimitErrorData" }
                }
            }
        }),
    );
}

fn add_standard_error_response(
    paths: &mut Map<String, Value>,
    path: &str,
    method: &str,
    status: &str,
    description: &str,
) {
    let Some(responses) = paths
        .get_mut(path)
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut(method))
        .and_then(|operation| operation.get_mut("responses"))
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    responses.insert(
        status.to_string(),
        json!({
            "description": description,
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }),
    );
}

fn set_operation_error_schema(
    paths: &mut Map<String, Value>,
    path: &str,
    method: &str,
    schema: &str,
) {
    let Some(default_response) = paths
        .get_mut(path)
        .and_then(Value::as_object_mut)
        .and_then(|item| item.get_mut(method))
        .and_then(|operation| operation.get_mut("responses"))
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut("default"))
    else {
        return;
    };
    *default_response = json!({
        "description": "Stable Web Terminal domain error",
        "content": {
            "application/json": {
                "schema": { "$ref": format!("#/components/schemas/{schema}") }
            }
        }
    });
}

fn insert_typed_message_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    request_schema: Option<&str>,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiSuccessEnvelope" }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    if let Some(request_schema) = request_schema {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
                }
            }
        });
    }
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_direct_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    response_schema: &str,
    request_schema: Option<&str>,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{response_schema}") }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    if let Some(request_schema) = request_schema {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
                }
            }
        });
    }
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_array_enveloped_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    item_schema: &str,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["success", "data"],
                        "properties": {
                            "success": { "type": "boolean", "const": true },
                            "message": { "type": ["string", "null"] },
                            "data": {
                                "type": "array",
                                "items": { "$ref": format!("#/components/schemas/{item_schema}") }
                            }
                        },
                        "additionalProperties": true
                    }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_json_enveloped_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    data_schema: Value,
    parameters: Option<Value>,
    request_schema: Option<(&str, bool)>,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["success"],
                        "properties": {
                            "success": { "type": "boolean", "const": true },
                            "message": { "type": ["string", "null"] },
                            "data": data_schema
                        },
                        "additionalProperties": true
                    }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    if let Some(parameters) = parameters {
        operation["parameters"] = parameters;
    }
    if let Some((request_schema, required)) = request_schema {
        operation["requestBody"] = json!({
            "required": required,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
                }
            }
        });
    }
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_array_enveloped_request_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    item_schema: &str,
    request_schema: &str,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["success", "data"],
                        "properties": {
                            "success": { "type": "boolean", "const": true },
                            "message": { "type": ["string", "null"] },
                            "data": {
                                "type": "array",
                                "items": { "$ref": format!("#/components/schemas/{item_schema}") }
                            }
                        },
                        "additionalProperties": true
                    }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["requestBody"] = json!({
        "required": true,
        "content": {
            "application/json": {
                "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_empty_enveloped_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    request_schema: Option<&str>,
    parameters: Option<Value>,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiSuccessEnvelope" }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    if let Some(request_schema) = request_schema {
        operation["requestBody"] = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "$ref": format!("#/components/schemas/{request_schema}") }
                }
            }
        });
    }
    if let Some(parameters) = parameters {
        operation["parameters"] = parameters;
    }
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_binary_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    content_type: &str,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    let mut content = Map::new();
    content.insert(
        content_type.to_string(),
        json!({ "schema": { "type": "string", "format": "binary" } }),
    );
    operation["responses"] = json!({
        "200": {
            "description": "Binary attachment",
            "content": Value::Object(content)
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_media_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    response: Value,
    parameters: Option<Value>,
    empty_stream_response: bool,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    let mut responses = Map::new();
    responses.insert("200".to_string(), response);
    if empty_stream_response {
        responses.insert(
            "204".to_string(),
            json!({ "description": "The requested stream is empty" }),
        );
    }
    responses.insert(
        "default".to_string(),
        json!({
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }),
    );
    operation["responses"] = Value::Object(responses);
    if let Some(parameters) = parameters {
        operation["parameters"] = parameters;
    }
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn insert_typed_html_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "HTML attachment",
            "content": {
                "text/html": { "schema": { "type": "string" } }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn add_success_response_headers(
    paths: &mut Map<String, Value>,
    path: &str,
    method: &str,
    header_names: &[&str],
) {
    let Some(headers) = paths
        .get_mut(path)
        .and_then(Value::as_object_mut)
        .and_then(|path| path.get_mut(method))
        .and_then(|operation| operation.get_mut("responses"))
        .and_then(Value::as_object_mut)
        .and_then(|responses| responses.get_mut("200"))
        .and_then(Value::as_object_mut)
        .and_then(|response| {
            response
                .entry("headers")
                .or_insert_with(|| json!({}))
                .as_object_mut()
        })
    else {
        return;
    };
    for header_name in header_names {
        headers.insert(
            (*header_name).to_string(),
            json!({
                "description": "Opaque configuration revision for optimistic concurrency control",
                "schema": { "type": "string", "minLength": 1 }
            }),
        );
    }
}

fn insert_typed_nullable_enveloped_operation(
    paths: &mut Map<String, Value>,
    document: &utoipa::openapi::OpenApi,
    path: &str,
    method: &str,
    data_schema: &str,
) {
    let Some(mut operation) = typed_operation(document, path, method) else {
        return;
    };
    operation["responses"] = json!({
        "200": {
            "description": "Successful response",
            "content": {
                "application/json": {
                    "schema": {
                        "type": "object",
                        "required": ["success", "data"],
                        "properties": {
                            "success": { "type": "boolean", "const": true },
                            "message": { "type": ["string", "null"] },
                            "data": {
                                "anyOf": [
                                    { "$ref": format!("#/components/schemas/{data_schema}") },
                                    { "type": "null" }
                                ]
                            }
                        },
                        "additionalProperties": true
                    }
                }
            }
        },
        "default": {
            "description": "Standard fn-knock error response",
            "content": {
                "application/json": {
                    "schema": { "$ref": "#/components/schemas/ApiErrorEnvelope" }
                }
            }
        }
    });
    operation["x-fn-knock-contract-source"] = json!("utoipa");
    insert_typed_operation(paths, path, method, operation);
}

fn typed_health_contract() -> utoipa::openapi::OpenApi {
    let (_router, document): (Router<AppState>, utoipa::openapi::OpenApi) = OpenApiRouter::new()
        .routes(routes!(crate::response::healthz))
        .split_for_parts();
    document
}

pub(super) fn path_parameters(path: &str) -> Vec<Value> {
    let mut names = BTreeSet::new();
    let mut cursor = path;
    while let Some(start) = cursor.find('{') {
        let remaining = &cursor[start + 1..];
        let Some(end) = remaining.find('}') else {
            break;
        };
        let name = &remaining[..end];
        if !name.is_empty() {
            names.insert(name.to_string());
        }
        cursor = &remaining[end + 1..];
    }
    names
        .into_iter()
        .map(|name| {
            json!({
                "name": name,
                "in": "path",
                "required": true,
                "schema": { "type": "string" }
            })
        })
        .collect()
}

pub(super) fn operation_id(method: &str, path: &str) -> String {
    let path = path
        .trim_matches('/')
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("{method}_{path}")
}

pub(super) fn route_tag(path: &str) -> String {
    path.strip_prefix("/api/admin/")
        .or_else(|| path.strip_prefix("/api/internal/"))
        .and_then(|rest| rest.split('/').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("admin")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document_value() -> Value {
        build_openapi_document()
    }

    #[test]
    fn embedded_openapi_matches_exported_contract_byte_for_byte() {
        let mut generated = serde_json::to_string_pretty(&document_value()).unwrap();
        generated.push('\n');
        assert_eq!(OPENAPI_DOCUMENT, generated.as_bytes());
    }

    #[test]
    fn docs_ui_preserves_swagger_defaults_with_local_assets() {
        assert!(OPENAPI_DOCS_INDEX.contains("<title>Swagger UI</title>"));
        assert!(OPENAPI_DOCS_INDEX.contains("/docs/assets/swagger-ui.css"));
        assert!(OPENAPI_DOCS_INDEX.contains("/docs/assets/index.css"));
        assert!(OPENAPI_DOCS_INDEX.contains("/docs/assets/swagger-ui-bundle.js"));
        assert!(OPENAPI_DOCS_INDEX.contains("/docs/assets/swagger-ui-standalone-preset.js"));
        assert!(OPENAPI_DOCS_INDEX.contains("layout: \"StandaloneLayout\""));
        assert!(OPENAPI_DOCS_INDEX.contains("validatorUrl: null"));
        assert!(OPENAPI_DOCS_INDEX.contains("SwaggerUIStandalonePreset"));
        assert!(!OPENAPI_DOCS_INDEX.contains("fn-knock API Explorer"));
        assert!(!OPENAPI_DOCS_INDEX.contains("docs-hero"));
        assert!(!OPENAPI_DOCS_INDEX.contains("filter: true"));
        assert!(!OPENAPI_DOCS_INDEX.contains("docExpansion"));
        assert!(!OPENAPI_DOCS_INDEX.contains("cdn.jsdelivr.net"));
        assert!(!OPENAPI_DOCS_INDEX.contains("unpkg.com"));
        assert!(SWAGGER_UI_STYLESHEET.len() > 100_000);
        assert!(SWAGGER_UI_BUNDLE.len() > 1_000_000);
        assert!(SWAGGER_UI_STANDALONE_PRESET.len() > 200_000);
    }

    #[test]
    fn ssl_operations_have_curated_swagger_documentation() {
        let document = document_value();
        let ssl_tag = document
            .get("tags")
            .and_then(Value::as_array)
            .and_then(|tags| tags.iter().find(|tag| tag["name"] == "ssl"))
            .expect("SSL tag documentation");
        assert!(ssl_tag["description"].as_str().is_some_and(|description| {
            description.contains("证书库") && description.contains("同源管理面板")
        }));
        assert!(
            !ssl_tag["description"]
                .as_str()
                .is_some_and(|description| description.contains("\\n")),
            "tag descriptions must contain real line breaks instead of literal escape sequences"
        );

        let mut ssl_operations = 0;
        let paths = document["paths"].as_object().expect("OpenAPI paths");
        for path_item in paths.values().filter_map(Value::as_object) {
            for operation in path_item.values().filter_map(Value::as_object) {
                if operation["tags"] != json!(["ssl"]) {
                    continue;
                }
                ssl_operations += 1;
                assert!(
                    operation["summary"]
                        .as_str()
                        .is_some_and(|summary| !summary.is_ascii())
                );
                assert!(
                    operation["description"]
                        .as_str()
                        .is_some_and(|description| { !description.is_ascii() })
                );
                assert!(
                    operation["responses"]["200"]["description"]
                        .as_str()
                        .is_some_and(|description| { !description.is_ascii() })
                );
            }
        }
        assert_eq!(ssl_operations, 29);

        assert!(
            document
                .pointer("/paths/~1api~1admin~1ssl~1certificates/post/responses/400")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1ssl~1activate/post/responses/404")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1ssl~1shared-files~1content/get/responses/403")
                .is_some()
        );
        assert!(
            document
                .pointer("/components/schemas/SslCertificateSaveBodyData/properties/key/writeOnly")
                == Some(&json!(true))
        );
        assert!(
            document
                .pointer(
                    "/components/schemas/SslCertificateSaveBodyData/properties/key/description"
                )
                .and_then(Value::as_str)
                .is_some_and(|description| description.contains("仅写入"))
        );
        assert!(
            !serde_json::to_string(&document)
                .expect("serialize OpenAPI document")
                .contains("-----BEGIN PRIVATE KEY-----")
        );
    }

    #[test]
    fn every_operation_has_a_chinese_swagger_baseline() {
        let document = document_value();
        let paths = document["paths"].as_object().expect("OpenAPI paths");
        let tags = document["tags"].as_array().expect("top-level OpenAPI tags");
        let documented_tags = tags
            .iter()
            .filter_map(|tag| tag["name"].as_str())
            .collect::<BTreeSet<_>>();
        let mut operation_tags = BTreeSet::new();
        let mut operations = 0;
        for path_item in paths.values().filter_map(Value::as_object) {
            for operation in path_item.values().filter_map(Value::as_object) {
                operations += 1;
                for tag in operation["tags"].as_array().into_iter().flatten() {
                    if let Some(tag) = tag.as_str() {
                        operation_tags.insert(tag);
                    }
                }
                for field in ["summary", "description"] {
                    assert!(
                        operation[field]
                            .as_str()
                            .is_some_and(|value| { !value.is_ascii() })
                    );
                }
                let summary = operation["summary"].as_str().expect("operation summary");
                let untranslated = summary.replace("Cloudflare", "").replace("Web", "");
                assert!(
                    !untranslated
                        .chars()
                        .any(|character| character.is_ascii_lowercase()),
                    "summary must not expose untranslated path segments: {summary}"
                );
                for response in operation["responses"]
                    .as_object()
                    .into_iter()
                    .flat_map(|responses| responses.values())
                {
                    assert!(
                        response["description"]
                            .as_str()
                            .is_some_and(|description| { !description.is_ascii() }),
                        "response descriptions must be localized"
                    );
                }
            }
        }
        assert_eq!(operations, 447);
        assert_eq!(documented_tags, operation_tags);
        assert!(documented_tags.iter().all(|tag| {
            tags.iter().any(|item| {
                item["name"] == *tag
                    && item["description"]
                        .as_str()
                        .is_some_and(|description| !description.is_ascii())
            })
        }));
    }

    #[test]
    fn generated_openapi_document_contains_admin_routes_and_schemas() {
        let document = document_value();
        assert_eq!(document["openapi"], json!("3.1.0"));
        assert_eq!(
            document.pointer("/paths/~1api~1admin~1healthz/get/x-fn-knock-contract-source"),
            Some(&json!("utoipa"))
        );
        let config = document
            .pointer("/paths/~1api~1admin~1config/get")
            .expect("config route");
        assert_eq!(config["x-fn-knock-contract-source"], json!("utoipa"));
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1system~1access-entry/get/x-fn-knock-contract-source"
            ),
            Some(&json!("utoipa")),
            "migrated routes must retain their OpenApiRouter provenance"
        );
        assert_eq!(
            document
                .pointer("/paths/~1api~1admin~1security~1overview/get/x-fn-knock-contract-source"),
            Some(&json!("utoipa")),
            "migrated routes must retain their OpenApiRouter provenance"
        );
        for (path, method) in [
            ("/api/admin/config/dashboard_display", "get"),
            ("/api/admin/config/dashboard_display", "post"),
            ("/api/admin/dashboard/stats", "get"),
            ("/api/admin/dashboard/realtime", "get"),
            ("/api/admin/dashboard/active-ips", "get"),
            ("/api/admin/dashboard/stream-active-ips", "get"),
            ("/api/admin/update/status", "get"),
            ("/api/admin/update/check", "post"),
            ("/api/admin/update/check-and-download", "post"),
            ("/api/admin/update/download", "post"),
            ("/api/admin/update/install", "post"),
            ("/api/admin/update/confirm", "get"),
            ("/api/admin/cidr/capabilities", "get"),
            ("/api/admin/cidr/provinces", "get"),
            ("/api/admin/cidr/cities", "get"),
            ("/api/admin/cidr/selector", "get"),
            ("/api/admin/cidr/cidrs", "get"),
            ("/api/admin/ip-location/batch", "post"),
            ("/api/admin/config/ip_location_api", "get"),
            ("/api/admin/config/ip_location_api", "post"),
            ("/api/admin/config/ip_location_api/test-ip-lookup", "post"),
            ("/api/admin/config/ip_location_api/test-cidr", "post"),
            ("/api/admin/backoff/list", "get"),
            ("/api/admin/backoff/status", "get"),
            ("/api/admin/backoff/reset", "post"),
            ("/api/internal/system-events", "post"),
            ("/api/admin/events", "get"),
            ("/api/admin/events", "delete"),
            ("/api/admin/events/clear", "delete"),
            ("/api/admin/runtime-health", "get"),
            ("/api/admin/runtime-health/gateway-memory", "get"),
            ("/api/admin/runtime-health/gateway-memory", "put"),
            ("/api/admin/runtime-health/gateway-memory/reclaim", "post"),
            ("/api/admin/runtime-health/logs/{component}", "get"),
            ("/api/admin/runtime-health/logs/{component}", "delete"),
            ("/api/admin/runtime-health/diagnostics", "get"),
            ("/api/admin/runtime-health/diagnostics/archive", "get"),
            ("/api/admin/general-blacklist", "get"),
            ("/api/admin/general-blacklist", "post"),
            ("/api/admin/general-blacklist", "delete"),
            ("/api/admin/general-blacklist/status", "post"),
            ("/api/admin/general-blacklist/{ip}", "delete"),
            ("/api/admin/scanner/settings", "get"),
            ("/api/admin/scanner/settings", "post"),
            ("/api/admin/scanner/blacklist", "get"),
            ("/api/admin/scanner/blacklist", "delete"),
            ("/api/admin/scanner/blacklist/{ip}", "get"),
            ("/api/admin/scanner/blacklist/{ip}", "delete"),
            ("/api/admin/config/gateway", "get"),
            ("/api/admin/config/gateway", "post"),
            ("/api/admin/config/gateway/visibility", "get"),
            ("/api/admin/config/gateway/visibility", "post"),
            ("/api/admin/config/gateway/proxy-headers", "get"),
            ("/api/admin/config/gateway/proxy-headers", "post"),
            ("/api/admin/config/gateway/host-response", "get"),
            ("/api/admin/config/gateway/host-response", "post"),
            ("/api/admin/config/gateway/proxy-protocol", "get"),
            ("/api/admin/config/gateway/proxy-protocol", "post"),
            ("/api/admin/config/fnos_certificate_sync/details", "get"),
            ("/api/admin/config/fnos_certificate_sync", "post"),
            ("/api/admin/config/fnos_certificate_sync/sync", "post"),
            ("/api/admin/config/fnos_port_icon_hijack", "get"),
            ("/api/admin/config/fnos_port_icon_hijack", "post"),
            ("/api/admin/config/fnos_network_tuning", "get"),
            ("/api/admin/config/fnos_network_tuning", "post"),
            ("/api/admin/config/fnos_connect_waf", "get"),
            ("/api/admin/config/fnos_connect_waf", "post"),
            ("/api/admin/config/fnos_share_bypass", "get"),
            ("/api/admin/config/fnos_share_bypass", "post"),
            ("/api/admin/config/smart_connect/details", "get"),
            ("/api/admin/config/smart_connect", "post"),
            ("/api/admin/config/proxy_protocol_force", "get"),
            ("/api/admin/config/proxy_protocol_force", "post"),
            ("/api/admin/config/run_mode_prompt_preferences", "get"),
            ("/api/admin/config/run_mode_prompt_preferences", "post"),
            ("/api/admin/config/protocol_mapping_feature", "get"),
            ("/api/admin/config/protocol_mapping_feature", "post"),
            ("/api/admin/config/auto_https", "get"),
            ("/api/admin/config/auto_https", "post"),
            ("/api/admin/config/default_route", "get"),
            ("/api/admin/config/default_route", "post"),
            ("/api/admin/config/default_tunnel", "post"),
            ("/api/admin/config/captcha", "get"),
            ("/api/admin/config/captcha", "post"),
            ("/api/admin/config/run_type", "post"),
            ("/api/admin/config/wol_feature", "get"),
            ("/api/admin/config/wol_feature", "post"),
            ("/api/admin/config/locale", "get"),
            ("/api/admin/config/locale", "post"),
            ("/api/admin/config/appearance", "get"),
            ("/api/admin/config/appearance", "post"),
            ("/api/admin/auth/mode", "get"),
            ("/api/admin/auth/mode/preview", "post"),
            ("/api/admin/auth/mode/switch", "post"),
            ("/api/admin/auth/accounts", "get"),
            ("/api/admin/auth/accounts", "post"),
            ("/api/admin/auth/accounts/{id}", "patch"),
            ("/api/admin/auth/accounts/{id}", "delete"),
            ("/api/admin/auth/accounts/{id}/password", "post"),
            ("/api/admin/auth/accounts/{id}/setup", "post"),
            ("/api/admin/auth/accounts/{id}/totp/setup", "post"),
            ("/api/admin/auth/accounts/{id}/totp/bind", "post"),
            ("/api/admin/auth/accounts/{id}/access-scopes", "patch"),
            ("/api/admin/auth/accounts/{id}/subdomain-access", "patch"),
            ("/api/admin/sessions", "get"),
            ("/api/admin/sessions/{id}", "get"),
            ("/api/admin/sessions/{id}", "delete"),
            ("/api/admin/sessions/{id}/comment", "patch"),
            ("/api/admin/sessions/{id}/mobility", "get"),
            ("/api/admin/maintenance/backup/automatic", "get"),
            ("/api/admin/maintenance/backup/automatic", "put"),
            ("/api/admin/maintenance/backup/automatic/files", "get"),
            ("/api/admin/maintenance/backup/export", "get"),
            ("/api/admin/maintenance/backup/files", "get"),
            ("/api/admin/maintenance/backup/export/fnos", "post"),
            ("/api/admin/maintenance/backup/import", "post"),
            ("/api/admin/maintenance/backup/import/automatic", "post"),
            ("/api/admin/maintenance/backup/import/fnos", "post"),
            ("/api/admin/maintenance/data/clear", "post"),
            ("/api/admin/config/auth_credential_settings", "get"),
            ("/api/admin/config/auth_credential_settings", "post"),
            ("/api/admin/panel/bootstrap", "get"),
            ("/api/admin/panel/password", "post"),
            ("/api/admin/panel/password/change", "post"),
            ("/api/admin/panel/login", "post"),
            ("/api/admin/panel/logout", "post"),
            ("/api/admin/totp/status", "get"),
            ("/api/admin/totp/setup", "post"),
            ("/api/admin/totp/bind", "post"),
            ("/api/admin/totp/{id}", "delete"),
            ("/api/admin/totp/{id}/access-scopes", "patch"),
            ("/api/admin/totp/{id}/subdomain-access", "patch"),
            ("/api/admin/totp/{id}/comment", "patch"),
            ("/api/admin/passkeys/{id}", "delete"),
            ("/api/admin/totp/credentials/export", "get"),
            ("/api/admin/totp/credentials/import", "post"),
            ("/api/admin/totp/{totp_id}/passkeys", "get"),
            ("/api/admin/config/auto_manage_firewall", "post"),
            ("/api/admin/config/firewall_additional_ports", "get"),
            ("/api/admin/config/firewall_additional_ports", "post"),
            ("/api/admin/firewall/reset", "post"),
            ("/api/admin/firewall/clear", "post"),
            ("/api/admin/sync-routes", "post"),
        ] {
            assert_eq!(
                document
                    .get("paths")
                    .and_then(|paths| paths.get(path))
                    .and_then(|item| item.get(method))
                    .and_then(|operation| operation.get("x-fn-knock-contract-source")),
                Some(&json!("utoipa")),
                "{method} {path} must retain OpenApiRouter provenance"
            );
        }
        for (path, method, operation_id) in [
            (
                "/api/admin/runtime-health/logs/{component}",
                "get",
                "get_api_admin_runtime_health_logs__component_",
            ),
            (
                "/api/admin/runtime-health/logs/{component}",
                "delete",
                "delete_api_admin_runtime_health_logs__component_",
            ),
            (
                "/api/admin/general-blacklist/{ip}",
                "delete",
                "delete_api_admin_general_blacklist__ip_",
            ),
            (
                "/api/admin/scanner/blacklist/{ip}",
                "get",
                "get_api_admin_scanner_blacklist__ip_",
            ),
            (
                "/api/admin/scanner/blacklist/{ip}",
                "delete",
                "delete_api_admin_scanner_blacklist__ip_",
            ),
            (
                "/api/admin/config/fnos_certificate_sync/details",
                "get",
                "get_api_admin_config_fnos_certificate_sync_details",
            ),
            (
                "/api/admin/config/fnos_certificate_sync",
                "post",
                "post_api_admin_config_fnos_certificate_sync",
            ),
            (
                "/api/admin/config/fnos_certificate_sync/sync",
                "post",
                "post_api_admin_config_fnos_certificate_sync_sync",
            ),
            (
                "/api/admin/config/fnos_port_icon_hijack",
                "get",
                "get_api_admin_config_fnos_port_icon_hijack",
            ),
            (
                "/api/admin/config/fnos_port_icon_hijack",
                "post",
                "post_api_admin_config_fnos_port_icon_hijack",
            ),
            (
                "/api/admin/config/fnos_network_tuning",
                "get",
                "get_api_admin_config_fnos_network_tuning",
            ),
            (
                "/api/admin/config/fnos_network_tuning",
                "post",
                "post_api_admin_config_fnos_network_tuning",
            ),
            (
                "/api/admin/config/fnos_connect_waf",
                "get",
                "get_api_admin_config_fnos_connect_waf",
            ),
            (
                "/api/admin/config/fnos_connect_waf",
                "post",
                "post_api_admin_config_fnos_connect_waf",
            ),
            (
                "/api/admin/config/fnos_share_bypass",
                "get",
                "get_api_admin_config_fnos_share_bypass",
            ),
            (
                "/api/admin/config/fnos_share_bypass",
                "post",
                "post_api_admin_config_fnos_share_bypass",
            ),
            (
                "/api/admin/config/smart_connect/details",
                "get",
                "get_api_admin_config_smart_connect_details",
            ),
            (
                "/api/admin/config/smart_connect",
                "post",
                "post_api_admin_config_smart_connect",
            ),
            (
                "/api/admin/config/proxy_protocol_force",
                "get",
                "get_api_admin_config_proxy_protocol_force",
            ),
            (
                "/api/admin/config/proxy_protocol_force",
                "post",
                "post_api_admin_config_proxy_protocol_force",
            ),
            (
                "/api/admin/config/run_mode_prompt_preferences",
                "get",
                "get_api_admin_config_run_mode_prompt_preferences",
            ),
            (
                "/api/admin/config/run_mode_prompt_preferences",
                "post",
                "post_api_admin_config_run_mode_prompt_preferences",
            ),
            (
                "/api/admin/config/protocol_mapping_feature",
                "get",
                "get_api_admin_config_protocol_mapping_feature",
            ),
            (
                "/api/admin/config/protocol_mapping_feature",
                "post",
                "post_api_admin_config_protocol_mapping_feature",
            ),
            (
                "/api/admin/config/auto_https",
                "get",
                "get_api_admin_config_auto_https",
            ),
            (
                "/api/admin/config/auto_https",
                "post",
                "post_api_admin_config_auto_https",
            ),
            (
                "/api/admin/config/default_route",
                "get",
                "get_api_admin_config_default_route",
            ),
            (
                "/api/admin/config/default_route",
                "post",
                "post_api_admin_config_default_route",
            ),
            (
                "/api/admin/config/default_tunnel",
                "post",
                "post_api_admin_config_default_tunnel",
            ),
            (
                "/api/admin/config/captcha",
                "get",
                "get_api_admin_config_captcha",
            ),
            (
                "/api/admin/config/captcha",
                "post",
                "post_api_admin_config_captcha",
            ),
            (
                "/api/admin/config/run_type",
                "post",
                "post_api_admin_config_run_type",
            ),
            (
                "/api/admin/config/wol_feature",
                "get",
                "get_api_admin_config_wol_feature",
            ),
            (
                "/api/admin/config/wol_feature",
                "post",
                "post_api_admin_config_wol_feature",
            ),
            (
                "/api/admin/config/locale",
                "get",
                "get_api_admin_config_locale",
            ),
            (
                "/api/admin/config/locale",
                "post",
                "post_api_admin_config_locale",
            ),
            (
                "/api/admin/config/appearance",
                "get",
                "get_api_admin_config_appearance",
            ),
            (
                "/api/admin/config/appearance",
                "post",
                "post_api_admin_config_appearance",
            ),
            ("/api/admin/auth/mode", "get", "get_api_admin_auth_mode"),
            (
                "/api/admin/auth/mode/preview",
                "post",
                "post_api_admin_auth_mode_preview",
            ),
            (
                "/api/admin/auth/mode/switch",
                "post",
                "post_api_admin_auth_mode_switch",
            ),
            (
                "/api/admin/auth/accounts",
                "get",
                "get_api_admin_auth_accounts",
            ),
            (
                "/api/admin/auth/accounts",
                "post",
                "post_api_admin_auth_accounts",
            ),
            (
                "/api/admin/auth/accounts/{id}",
                "patch",
                "patch_api_admin_auth_accounts_by_id",
            ),
            (
                "/api/admin/auth/accounts/{id}",
                "delete",
                "delete_api_admin_auth_accounts_by_id",
            ),
            (
                "/api/admin/auth/accounts/{id}/password",
                "post",
                "post_api_admin_auth_accounts_by_id_password",
            ),
            (
                "/api/admin/auth/accounts/{id}/setup",
                "post",
                "post_api_admin_auth_accounts_by_id_setup",
            ),
            (
                "/api/admin/auth/accounts/{id}/totp/setup",
                "post",
                "post_api_admin_auth_accounts_by_id_totp_setup",
            ),
            (
                "/api/admin/auth/accounts/{id}/totp/bind",
                "post",
                "post_api_admin_auth_accounts_by_id_totp_bind",
            ),
            (
                "/api/admin/auth/accounts/{id}/access-scopes",
                "patch",
                "patch_api_admin_auth_accounts_by_id_access_scopes",
            ),
            (
                "/api/admin/auth/accounts/{id}/subdomain-access",
                "patch",
                "patch_api_admin_auth_accounts_by_id_subdomain_access",
            ),
            ("/api/admin/sessions", "get", "get_api_admin_sessions"),
            (
                "/api/admin/sessions/{id}",
                "get",
                "get_api_admin_sessions_by_id",
            ),
            (
                "/api/admin/sessions/{id}",
                "delete",
                "delete_api_admin_sessions_by_id",
            ),
            (
                "/api/admin/sessions/{id}/comment",
                "patch",
                "patch_api_admin_sessions_by_id_comment",
            ),
            (
                "/api/admin/sessions/{id}/mobility",
                "get",
                "get_api_admin_sessions_by_id_mobility",
            ),
            (
                "/api/admin/maintenance/backup/automatic",
                "get",
                "get_api_admin_maintenance_backup_automatic",
            ),
            (
                "/api/admin/maintenance/backup/automatic",
                "put",
                "put_api_admin_maintenance_backup_automatic",
            ),
            (
                "/api/admin/maintenance/backup/automatic/files",
                "get",
                "get_api_admin_maintenance_backup_automatic_files",
            ),
            (
                "/api/admin/maintenance/backup/export",
                "get",
                "get_api_admin_maintenance_backup_export",
            ),
            (
                "/api/admin/maintenance/backup/files",
                "get",
                "get_api_admin_maintenance_backup_files",
            ),
            (
                "/api/admin/maintenance/backup/export/fnos",
                "post",
                "post_api_admin_maintenance_backup_export_fnos",
            ),
            (
                "/api/admin/maintenance/backup/import",
                "post",
                "post_api_admin_maintenance_backup_import",
            ),
            (
                "/api/admin/maintenance/backup/import/automatic",
                "post",
                "post_api_admin_maintenance_backup_import_automatic",
            ),
            (
                "/api/admin/maintenance/backup/import/fnos",
                "post",
                "post_api_admin_maintenance_backup_import_fnos",
            ),
            (
                "/api/admin/maintenance/data/clear",
                "post",
                "post_api_admin_maintenance_data_clear",
            ),
            (
                "/api/admin/config/auth_credential_settings",
                "get",
                "get_api_admin_config_auth_credential_settings",
            ),
            (
                "/api/admin/config/auth_credential_settings",
                "post",
                "post_api_admin_config_auth_credential_settings",
            ),
            (
                "/api/admin/panel/bootstrap",
                "get",
                "get_api_admin_panel_bootstrap",
            ),
            (
                "/api/admin/panel/password",
                "post",
                "post_api_admin_panel_password",
            ),
            (
                "/api/admin/panel/password/change",
                "post",
                "post_api_admin_panel_password_change",
            ),
            (
                "/api/admin/panel/login",
                "post",
                "post_api_admin_panel_login",
            ),
            (
                "/api/admin/panel/logout",
                "post",
                "post_api_admin_panel_logout",
            ),
            ("/api/admin/totp/status", "get", "get_api_admin_totp_status"),
            ("/api/admin/totp/setup", "post", "post_api_admin_totp_setup"),
            ("/api/admin/totp/bind", "post", "post_api_admin_totp_bind"),
            (
                "/api/admin/totp/{id}",
                "delete",
                "delete_api_admin_totp_by_id",
            ),
            (
                "/api/admin/totp/{id}/access-scopes",
                "patch",
                "patch_api_admin_totp_by_id_access_scopes",
            ),
            (
                "/api/admin/totp/{id}/subdomain-access",
                "patch",
                "patch_api_admin_totp_by_id_subdomain_access",
            ),
            (
                "/api/admin/totp/{id}/comment",
                "patch",
                "patch_api_admin_totp_by_id_comment",
            ),
            (
                "/api/admin/passkeys/{id}",
                "delete",
                "delete_api_admin_passkeys_by_id",
            ),
            (
                "/api/admin/totp/credentials/export",
                "get",
                "get_api_admin_totp_credentials_export",
            ),
            (
                "/api/admin/totp/credentials/import",
                "post",
                "post_api_admin_totp_credentials_import",
            ),
            (
                "/api/admin/totp/{totp_id}/passkeys",
                "get",
                "get_api_admin_totp_by_totp_id_passkeys",
            ),
            (
                "/api/admin/config/auto_manage_firewall",
                "post",
                "post_api_admin_config_auto_manage_firewall",
            ),
            (
                "/api/admin/config/firewall_additional_ports",
                "get",
                "get_api_admin_config_firewall_additional_ports",
            ),
            (
                "/api/admin/config/firewall_additional_ports",
                "post",
                "post_api_admin_config_firewall_additional_ports",
            ),
            (
                "/api/admin/firewall/reset",
                "post",
                "post_api_admin_firewall_reset",
            ),
            (
                "/api/admin/firewall/clear",
                "post",
                "post_api_admin_firewall_clear",
            ),
            (
                "/api/admin/sync-routes",
                "post",
                "post_api_admin_sync_routes",
            ),
        ] {
            assert_eq!(
                document
                    .get("paths")
                    .and_then(|paths| paths.get(path))
                    .and_then(|item| item.get(method))
                    .and_then(|operation| operation.get("operationId")),
                Some(&json!(operation_id)),
                "{method} {path} must preserve its legacy operationId"
            );
        }
        assert!(
            config
                .pointer("/responses/200/content/application~1json/schema")
                .is_some()
        );
        assert!(
            config
                .pointer("/responses/default/content/application~1json/schema")
                .is_some()
        );
        assert!(
            document
                .pointer("/components/schemas/ApiErrorEnvelope")
                .is_some()
        );
        assert!(
            document
                .pointer("/components/schemas/ApiSuccessEnvelope")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1maintenance~1backup~1export~1fnos/post")
                .is_some()
        );
        assert!(document.pointer("/paths/~1api~1auth~1login").is_none());
    }

    #[test]
    fn core_domains_use_typed_contracts_instead_of_scanner_fallbacks() {
        let document = document_value();
        let paths = document["paths"].as_object().expect("OpenAPI paths");
        let operations = paths
            .values()
            .filter_map(Value::as_object)
            .flat_map(|path| path.values())
            .collect::<Vec<_>>();
        assert_eq!(operations.len(), 447);
        assert!(
            operations
                .iter()
                .all(|operation| { operation["x-fn-knock-contract-source"] == json!("utoipa") })
        );
        assert_eq!(domain_contracts::expected_operation_count(), 76);

        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1events/delete/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/SystemEventDeleteBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1internal~1system-events/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/SystemEventPublishResultData"
            )),
            "internal event publication returns direct JSON"
        );
        let event_parameters = document
            .pointer("/paths/~1api~1admin~1events/get/parameters")
            .and_then(Value::as_array)
            .expect("system event query parameters");
        assert!(event_parameters.iter().any(|parameter| {
            parameter["name"] == "level"
                && parameter["schema"]["enum"] == json!(["INFO", "WARN", "ERROR", "CRITICAL"])
        }));
        assert!(event_parameters.iter().any(|parameter| {
            parameter["name"] == "source"
                && parameter["schema"]["enum"]
                    == json!([
                        "SERVER_ADMIN",
                        "GO_REAUTH_PROXY",
                        "SYSTEM_MONITOR",
                        "RUNTIME_MONITOR"
                    ])
        }));
        let publish_required = document
            .pointer("/components/schemas/SystemEventPublishResultData/required")
            .and_then(Value::as_array)
            .expect("system event publication required fields");
        assert!(publish_required.iter().any(|field| field == "data"));
        let backoff_parameters = document
            .pointer("/paths/~1api~1admin~1backoff~1status/get/parameters")
            .and_then(Value::as_array)
            .expect("login backoff query parameters");
        assert!(backoff_parameters.iter().any(|parameter| {
            parameter["name"] == "ip" && parameter["required"] == json!(true)
        }));
        for field in ["attempts", "blocked", "retryAfter", "blockedUntil"] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/LoginBackoffData/properties/{field}"
                    ))
                    .is_some(),
                "login backoff responses must preserve {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1captcha/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/CaptchaSettingsUpdateData"))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/CaptchaPowData/properties/base_max_number/multipleOf"
            ),
            Some(&json!(10_000))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/CaptchaTurnstileData/properties/secret_key/format"),
            Some(&json!("password"))
        );
        assert_eq!(
            document.pointer("/components/schemas/RunTypeUpdateData/properties/run_type/enum"),
            Some(&json!([0, 1, 3]))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1appearance/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/PanelAppearanceData"))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/AutoHttpsRuntimeData/properties/listen_port/const"),
            Some(&json!(80))
        );
        assert_eq!(
            document.pointer("/components/schemas/DefaultTunnelUpdateData/properties/tunnel/enum"),
            Some(&json!(["frp", "cloudflared"]))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/FirewallAdditionalPortsUpdateData/properties/ports/maxItems"
            ),
            Some(&json!(128))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/FirewallAdditionalPortsUpdateData/properties/ports/items/maximum"
            ),
            Some(&json!(65535))
        );
        assert_eq!(
            document.pointer("/components/schemas/FirewallResetBodyData/properties/run_type/enum"),
            Some(&json!([0, 1, 3]))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/FirewallResetData/properties/gatewayPort/maximum"),
            Some(&json!(65_535))
        );
        assert!(
            document
                .pointer("/components/schemas/SyncRoutesData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "waf_bundle_id"))
        );
        assert!(
            document
                .pointer("/components/schemas/MaintenanceClearData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "gateway_reset"))
        );
        assert_eq!(
            document.pointer("/components/schemas/AccessEntryData/properties/env/enum"),
            Some(&json!(["GO_REPROXY_PORT", "FRP_REMOTE_PORT"]))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/SystemClockStatusData/properties/expectedTimeZone/const"
            ),
            Some(&json!("Asia/Shanghai"))
        );
        assert!(
            document
                .pointer("/components/schemas/SystemClockStatusData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "syncSummary"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1system~1clock~1sync/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/SystemClockSyncResponseData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/SystemAssetDownloadProgressData/properties/status/enum"
            ),
            Some(&json!(["idle", "downloading", "completed", "error"]))
        );
        assert!(
            document
                .pointer("/components/schemas/SystemAssetDownloadProgressData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "error"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1system~1frp~1download/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/SystemAssetMutationResponseData"
            ))
        );
        assert_eq!(
            document.pointer("/components/schemas/DnsmasqInstallStateData/properties/status/enum"),
            Some(&json!(["uninstalled", "installing", "installed", "error"]))
        );
        assert_eq!(
            document.pointer("/components/schemas/TerminalAttachment/properties/transport/$ref"),
            Some(&json!("#/components/schemas/TerminalTransport"))
        );
        assert!(
            document
                .pointer("/components/schemas/SessionListResult/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "runtimeId"))
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1terminal~1attachments~1{id}~1events/get")
                .is_some()
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1cloudflared~1config/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/CloudflaredConfigUpdateData"
            ))
        );
        assert!(
            document
                .pointer("/components/schemas/CloudflaredConfigData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "rootDomain")),
            "cloudflared config always reports the managed root domain, including null"
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/CloudflaredConfigUpdateData/properties/token/writeOnly"
            ),
            Some(&json!(true))
        );
        assert!(
            document
                .pointer("/components/schemas/CloudflareReconcileRequestData/required")
                .is_none(),
            "reconcile preview fields have serde defaults"
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/CloudflareOptimizationSourceSettingsBodyData/properties/customHostnames/maxItems"
            ),
            Some(&json!(16))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/CloudflareOptimizationScanData/properties/status/enum"
            ),
            Some(&json!([
                "queued",
                "running",
                "completed",
                "failed",
                "cancelled"
            ]))
        );
        assert!(
            document
                .pointer("/components/schemas/CloudflaredSupervisorFailureData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "resources")),
            "supervisor failures always report nullable resource samples"
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1frpc~1config/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/FrpcConfigUpdateData"))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/FrpcConfigUpdateData/properties/content/writeOnly"),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/FrpcInstancesOverviewData/properties/primaryInstanceId/const"
            ),
            Some(&json!("primary"))
        );
        let frpc_instance_required = document
            .pointer("/components/schemas/FrpcInstanceStatusData/required")
            .and_then(Value::as_array)
            .expect("FRPC instance required fields");
        for field in [
            "pid",
            "startedAt",
            "stoppedAt",
            "lastExitCode",
            "lastMessage",
        ] {
            assert!(
                frpc_instance_required
                    .iter()
                    .any(|required| required == field),
                "FRPC instance status must always include nullable {field}"
            );
        }
        let frpc_poll_parameters = document
            .pointer("/paths/~1api~1admin~1frpc~1instances~1{id}~1poll/get/parameters")
            .and_then(Value::as_array)
            .expect("FRPC poll parameters");
        assert!(frpc_poll_parameters.iter().any(|parameter| {
            parameter["name"] == "cursor"
                && parameter["schema"]["oneOf"][0]["minimum"] == json!(0)
                && parameter["schema"]["oneOf"][1]["pattern"] == json!("^[0-9]+$")
        }));
        assert!(frpc_poll_parameters.iter().any(|parameter| {
            parameter["name"] == "id"
                && parameter["schema"]["pattern"] == json!("^[A-Za-z0-9-]{1,80}$")
        }));
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ddns~1settings/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/DdnsSettingsUpdateData"))
        );
        assert_eq!(
            document.pointer("/components/schemas/DdnsConfigBodyData/properties/config/writeOnly"),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer("/components/schemas/DdnsTargetBodyData/properties/config/writeOnly"),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/DdnsSettingsData/properties/updateIntervalMinutes/minimum"
            ),
            Some(&json!(crate::ddns::MIN_DDNS_UPDATE_INTERVAL_MINUTES))
        );
        let ddns_status_required = document
            .pointer("/components/schemas/DdnsStatusData/required")
            .and_then(Value::as_array)
            .expect("DDNS status required fields");
        for field in ["provider", "primaryTargetId"] {
            assert!(
                ddns_status_required
                    .iter()
                    .any(|required| required == field),
                "DDNS status must always include nullable {field}"
            );
        }
        let ddns_poll_parameters = document
            .pointer("/paths/~1api~1admin~1ddns~1poll/get/parameters")
            .and_then(Value::as_array)
            .expect("DDNS poll parameters");
        assert!(ddns_poll_parameters.iter().any(|parameter| {
            parameter["name"] == "cursor"
                && parameter["schema"]["oneOf"][0]["minimum"] == json!(0)
                && parameter["schema"]["oneOf"][1]["pattern"] == json!("^[0-9]+$")
        }));
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ddns~1test/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/DdnsTestResponseData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ssl~1certificates/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/SslCertificateSaveBodyData"))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/SslCertificateSaveBodyData/properties/key/writeOnly"),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer("/components/schemas/SslStatusData/properties/deploymentMode/enum"),
            Some(&json!(["single_active", "multi_sni"]))
        );
        let ssl_status_required = document
            .pointer("/components/schemas/SslStatusData/required")
            .and_then(Value::as_array)
            .expect("SSL status required fields");
        for field in ["subdomain_coverage", "library_coverage", "gateway_status"] {
            assert!(
                ssl_status_required.iter().any(|required| required == field),
                "SSL status must always include {field}"
            );
        }
        let shared_file_parameters = document
            .pointer("/paths/~1api~1admin~1ssl~1shared-files~1content/get/parameters")
            .and_then(Value::as_array)
            .expect("SSL shared file parameters");
        assert!(shared_file_parameters.iter().any(|parameter| {
            parameter["name"] == "path"
                && parameter["required"] == json!(true)
                && parameter["schema"]["minLength"] == json!(1)
        }));
        assert_eq!(
            document.pointer("/paths/~1api~1admin~1ssl~1ca~1hosts/delete/requestBody/required"),
            Some(&json!(false))
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1ssl~1cert.pem/get/responses/200/content/application~1x-pem-file"
                )
                .is_some()
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1ssl~1ca~1server-cert.zip/get/responses/200/content/application~1zip"
                )
                .is_some()
        );
        assert_eq!(
            document.pointer("/components/schemas/WafConfigData/properties/mode/const"),
            Some(&json!("blocking"))
        );
        assert_eq!(
            document.pointer("/components/schemas/WafConfigData/properties/block_behavior/enum"),
            Some(&json!(["error_page", "reset_connection"]))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1waf~1config/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/WafConfigUpdateData"))
        );
        let waf_rule_parameters = document
            .pointer("/paths/~1api~1admin~1waf~1rules~1{source}~1{filename}/get/parameters")
            .and_then(Value::as_array)
            .expect("WAF rule path parameters");
        assert!(waf_rule_parameters.iter().any(|parameter| {
            parameter["name"] == "source"
                && parameter["schema"]["enum"] == json!(["system", "custom"])
        }));
        assert!(
            document
                .pointer("/components/schemas/WafDetailsData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "status"))
        );
        assert!(
            document
                .pointer("/components/schemas/WafDrainResultData/properties/events")
                .is_none(),
            "the management drain endpoint does not return raw events"
        );
        assert_eq!(
            document.pointer("/components/schemas/WafUploadBodyData/properties/files/minItems"),
            Some(&json!(1))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/NotificationProviderCreateBodyData/properties/connection_config/writeOnly"
            ),
            Some(&json!(true))
        );
        assert!(
            document
                .pointer(
                    "/components/schemas/NotificationProviderDetailData/properties/connection_config/writeOnly"
                )
                .is_none(),
            "the authenticated provider detail explicitly returns the unmasked configuration"
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1notifications~1providers~1test/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/NotificationProviderTestResponseData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1notifications~1providers~1{id}~1test/post/parameters/0"
            ),
            Some(&json!({
                "description": "Notification provider identifier",
                "in": "path",
                "name": "id",
                "required": true,
                "schema": { "type": "string" }
            }))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/NotificationRuleCreateBodyData/properties/targets/minItems"
            ),
            Some(&json!(1))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/NotificationDeliveryPolicyData/properties/max_attempts/maximum"
            ),
            Some(&json!(10))
        );
        let protocol_mapping_required = document
            .pointer("/components/schemas/ProtocolMappingFeatureData/required")
            .and_then(Value::as_array)
            .expect("protocol mapping required fields");
        assert!(
            protocol_mapping_required
                .iter()
                .any(|field| field == "availability")
        );
        let smart_ip_required = document
            .pointer("/components/schemas/SmartConnectLocalIpData/required")
            .and_then(Value::as_array)
            .expect("smart connect local IP required fields");
        for field in ["netmask", "prefix"] {
            assert!(
                smart_ip_required.iter().any(|required| required == field),
                "smart connect local IP must include {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/components/schemas/FnosShareBypassData/properties/upstream_timeout_ms/maximum"
            ),
            Some(&json!(15_000))
        );
        assert!(
            document
                .pointer("/components/schemas/FnosPortIconHijackUpdateData/properties/updated_at")
                .is_none()
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/FnosNetworkTuningData/properties/blocked_reason_code/enum"
            ),
            Some(&json!(["lite", "deployment", "platform", "permission"]))
        );
        let connect_waf_required = document
            .pointer("/components/schemas/FnosConnectWafRuntimeData/required")
            .and_then(Value::as_array)
            .expect("FN Connect WAF required fields");
        for field in ["listener_port", "local_networks", "last_error"] {
            assert!(
                connect_waf_required
                    .iter()
                    .any(|required| required == field),
                "FN Connect WAF runtime must include {field}"
            );
        }
        let certificate_runtime_required = document
            .pointer("/components/schemas/FnosCertificateSyncRuntimeData/required")
            .and_then(Value::as_array)
            .expect("fnOS certificate sync runtime required fields");
        for field in [
            "last_sync_at",
            "last_result",
            "last_error",
            "failed_target_ids",
        ] {
            assert!(
                certificate_runtime_required
                    .iter()
                    .any(|required| required == field),
                "fnOS certificate sync runtime must include {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1fnos_certificate_sync~1sync/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/FnosCertificateSyncBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/StreamMappingInputData/properties/listen_port/maximum"
            ),
            Some(&json!(65535))
        );
        let subdomain_mode_required = document
            .pointer("/components/schemas/SubdomainModeData/required")
            .and_then(Value::as_array)
            .expect("subdomain mode required fields");
        for field in ["public_http_port", "public_https_port", "passkey_rp_id"] {
            assert!(
                subdomain_mode_required
                    .iter()
                    .any(|required| required == field),
                "subdomain mode must include {field}"
            );
        }
        assert!(
            document
                .pointer("/components/schemas/SubdomainModeResponseData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "ssl_auto_selection"))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/HostMappingBasicAuthInputData/properties/password/writeOnly"
            ),
            Some(&json!(true))
        );
        assert!(
            document
                .pointer("/components/schemas/HostMappingBasicAuthProbeData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "httpStatus"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1host_mappings~1static_path_probe/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/StaticPathProbeBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1host_mappings~1static_path_probe/post/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/StaticPathProbeResultData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1host_mappings~1static_path_browse/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/StaticPathBrowseBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1host_mappings~1static_path_browse/post/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/StaticPathBrowseResultData"))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/StaticPathBrowseResultData/properties/entries/maxItems"
            ),
            Some(&json!(100))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/StaticPathBrowseBodyData/properties/cursor/maxLength"
            ),
            Some(&json!(512))
        );
        assert_eq!(
            document.pointer("/components/schemas/StaticPathBrowseBodyData/additionalProperties"),
            Some(&json!(false))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/StaticPathBrowseEntryData/properties/modified_at/format"
            ),
            Some(&json!("date-time"))
        );
        let browse_required = document
            .pointer("/components/schemas/StaticPathBrowseResultData/required")
            .and_then(Value::as_array)
            .expect("static path browse required fields");
        for field in [
            "current_path",
            "parent_path",
            "selected_path",
            "previous_cursor",
            "next_cursor",
            "error_code",
        ] {
            assert!(
                browse_required.iter().any(|required| required == field),
                "static path browse result must always include {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/components/schemas/StaticServeConfigData/properties/index_files/maxItems"
            ),
            Some(&json!(16))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/StaticServeConfigData/properties/index_files/items/maxLength"
            ),
            Some(&json!(255))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/AdvancedAuthConfigInputData/properties/groups/maxItems"
            ),
            Some(&json!(16))
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1config~1host_mappings~1bookmarks~1export/get/responses/200/content/text~1html"
                )
                .is_some()
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/ScanDiscoverJobBodyData/properties/target_cidrs/minItems"
            ),
            Some(&json!(1))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/ScanDiscoverJobBodyData/properties/target_cidrs/maxItems"
            ),
            Some(&json!(16))
        );
        let scan_meta_required = document
            .pointer("/components/schemas/ScanDiscoverMetaData/required")
            .and_then(Value::as_array)
            .expect("scan discovery metadata required fields");
        assert!(scan_meta_required.iter().any(|field| field == "portRange"));
        assert!(!scan_meta_required.iter().any(|field| field == "services"));
        assert!(
            document
                .pointer("/components/schemas/ScanDiscoverResultData/required")
                .and_then(Value::as_array)
                .is_some_and(|required| required.iter().any(|field| field == "services"))
        );
        let scan_job_required = document
            .pointer("/components/schemas/ScanDiscoverJobData/required")
            .and_then(Value::as_array)
            .expect("scan discovery job required fields");
        for field in ["meta", "progress", "result", "error"] {
            assert!(
                scan_job_required.iter().any(|required| required == field),
                "scan discovery job must include nullable {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/components/schemas/DeepMonitorStartBodyData/properties/duration_seconds/oneOf/0/const"
            ),
            Some(&json!(0))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/DeepMonitorStartBodyData/properties/duration_seconds/oneOf/1/minimum"
            ),
            Some(&json!(300))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/DeepMonitorExtendBodyData/properties/duration_seconds/maximum"
            ),
            Some(&json!(7_200))
        );
        let deep_monitor_event_required = document
            .pointer("/components/schemas/DeepMonitorEventData/required")
            .and_then(Value::as_array)
            .expect("deep monitor event required fields");
        for field in ["summary", "timing", "websocket_frame"] {
            assert!(
                deep_monitor_event_required
                    .iter()
                    .any(|required| required == field),
                "deep monitor events must include nullable {field}"
            );
        }
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1deep-monitor~1sessions~1{session_id}~1live/get/responses/200/content/text~1event-stream"
                )
                .is_some()
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1deep-monitor~1sessions~1{session_id}~1download/get/responses/200/content/application~1zip"
                )
                .is_some()
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1deep-monitor~1sessions~1{session_id}~1events~1{event_id}~1payload/get/responses/200/content/application~1octet-stream"
                )
                .is_some()
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1dashboard_display/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/DashboardDisplayUpdateData"
            ))
        );
        let dashboard_stats_parameters = document
            .pointer("/paths/~1api~1admin~1dashboard~1stats/get/parameters")
            .and_then(Value::as_array)
            .expect("dashboard stats query parameters");
        assert!(dashboard_stats_parameters.iter().any(|parameter| {
            parameter["name"] == "rangeSec"
                && parameter["schema"]["minimum"] == 60
                && parameter["schema"]["maximum"] == 2_592_000
        }));
        let active_ip_parameters = document
            .pointer("/paths/~1api~1admin~1dashboard~1active-ips/get/parameters")
            .and_then(Value::as_array)
            .expect("dashboard active IP query parameters");
        assert!(active_ip_parameters.iter().any(|parameter| {
            parameter["name"] == "host" && parameter["required"] == json!(true)
        }));
        let stream_active_ip_parameters = document
            .pointer("/paths/~1api~1admin~1dashboard~1stream-active-ips/get/parameters")
            .and_then(Value::as_array)
            .expect("dashboard stream active IP query parameters");
        assert!(stream_active_ip_parameters.iter().any(|parameter| {
            parameter["name"] == "stream" && parameter["required"] == json!(true)
        }));
        for (schema, fields) in [
            (
                "DashboardRealtimeData",
                &["by_host", "by_stream", "timestamp"][..],
            ),
            ("DashboardHostTrafficData", &["active_ip_count"][..]),
            (
                "DashboardStreamTrafficData",
                &["key", "active_conns", "active_ip_count"][..],
            ),
            ("DashboardActiveIpsData", &["timestamp"][..]),
            ("DashboardStreamActiveIpsData", &["timestamp"][..]),
        ] {
            let required = document
                .pointer(&format!("/components/schemas/{schema}/required"))
                .and_then(Value::as_array)
                .expect("dashboard required fields");
            for field in fields {
                assert!(
                    required.iter().any(|required| required == field),
                    "{schema} must always emit {field}"
                );
            }
        }
        assert_eq!(
            document.pointer("/components/schemas/UpdateDownloadData/properties/status/enum"),
            Some(&json!([
                "idle",
                "downloading",
                "verifying",
                "downloaded",
                "installing",
                "error"
            ]))
        );
        let update_status_required = document
            .pointer("/components/schemas/UpdateStatusData/required")
            .and_then(Value::as_array)
            .expect("update status required fields");
        assert!(
            update_status_required
                .iter()
                .any(|required| required == "latest")
        );
        let update_confirm_data = document
            .pointer(
                "/paths/~1api~1admin~1update~1confirm/get/responses/200/content/application~1json/schema/properties/data/anyOf"
            )
            .and_then(Value::as_array)
            .expect("nullable update confirmation data");
        assert!(
            update_confirm_data
                .iter()
                .any(|schema| { schema["$ref"] == "#/components/schemas/UpdateConfirmData" })
        );
        assert!(
            update_confirm_data
                .iter()
                .any(|schema| schema["type"] == "null")
        );

        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1auth~1mode~1preview/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/AuthLoginModeBody"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1host_mappings/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/MappingsBody"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1maintenance~1backup~1import/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/ImportBackupBody"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1maintenance~1backup~1export/get/responses/200/content/application~1octet-stream/schema/format"
            ),
            Some(&json!("binary"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1totp~1credentials~1import/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/CredentialImportBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1totp~1credentials~1export/get/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/CredentialTransferData"))
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1totp~1credentials~1export/get/responses/200/content/application~1json/schema/properties/success"
                )
                .is_none(),
            "credential export is a raw JSON attachment, not an API envelope"
        );
        assert!(
            document
                .pointer("/components/schemas/AuthAccountData/properties/access_scopes")
                .is_some()
        );
        assert!(
            document
                .pointer("/components/schemas/AuthAccountData/properties/accessScopes")
                .is_none(),
            "permission payloads retain their existing snake_case wire keys"
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/AccessScopesUpdateData/properties/access_scopes/items/enum/0"
            ),
            Some(&json!("docker_admin_panel"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1auth~1ldap~1providers~1{id}~1test/post/requestBody/required"
            ),
            Some(&json!(false))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1auth~1oidc~1providers~1{id}~1test/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/ExternalAuthConnectionTestData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/OidcConnectionConfigInputData/properties/client_secret/writeOnly"
            ),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/LdapConnectionConfigInputData/properties/service_bind_password/writeOnly"
            ),
            Some(&json!(true))
        );
        assert!(
            document
                .pointer("/components/schemas/OidcProviderData/properties/connection_config")
                .is_none(),
            "OIDC read models must expose only connection_config_masked"
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/LdapConnectionConfigMaskedData/properties/service_bind_password/enum"
            ),
            Some(&json!(["", "********"]))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/OidcConnectionConfigMaskedData/properties/client_secret/enum"
            ),
            Some(&json!(["", "********", "[configured]"]))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1panel~1login/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/PanelLoginBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1panel~1login/post/responses/429/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/PanelLoginRateLimitErrorData"
            ))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/PanelLoginBodyData/properties/password/writeOnly"),
            Some(&json!(true))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/PanelBootstrapData/properties/deployment_target/enum"
            ),
            Some(&json!([
                "fpk", "fpk-lite", "docker", "openwrt", "linux", "macos", "synology", "windows",
                "dev"
            ]))
        );
        let panel_required = document
            .pointer("/components/schemas/PanelBootstrapData/required")
            .and_then(Value::as_array)
            .expect("panel bootstrap required fields");
        for field in ["auth_source", "session_expires_at"] {
            assert!(
                panel_required.iter().any(|required| required == field),
                "panel bootstrap always emits {field}, including null"
            );
        }
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1wol~1local-relay/put/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/WolLocalRelayInputData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1wol~1discover~1jobs~1{id}/delete/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/WolDiscoveryJobData"))
        );
        let discovery_parameters = document
            .pointer("/paths/~1api~1admin~1wol~1discover~1jobs~1{id}/get/parameters")
            .and_then(Value::as_array)
            .expect("WOL discovery parameters");
        assert!(
            discovery_parameters
                .iter()
                .any(|parameter| { parameter["name"] == "cursor" && parameter["in"] == "query" })
        );
        for (schema, property) in [
            ("WolLocalRelayInputData", "psk"),
            ("WolLocalRelayPairBodyData", "pairingCode"),
            ("WolBlinkerIntegrationInputData", "deviceKey"),
            ("WolBemfaIntegrationInputData", "privateKey"),
            ("WolTargetSshInputData", "password"),
            ("WolTargetSshInputData", "privateKey"),
            ("WolTargetSshInputData", "privateKeyPassphrase"),
        ] {
            assert_eq!(
                document.pointer(&format!(
                    "/components/schemas/{schema}/properties/{property}/writeOnly"
                )),
                Some(&json!(true)),
                "{schema}.{property} must remain write-only"
            );
        }
        assert!(
            document
                .pointer("/components/schemas/WolTargetData/properties/deviceKey")
                .is_none()
        );
        assert!(
            document
                .pointer("/components/schemas/WolRelayData/properties/psk")
                .is_none()
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1gateway/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/GatewaySettingsUpdateData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1gateway/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/GatewaySettingsData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1gateway~1visibility/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/GatewayVisibilityUpdateData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1gateway~1proxy-headers/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!(
                "#/components/schemas/GatewayProxyHeadersDetailsData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1gateway~1host-response/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/GatewayHostResponseUpdateData"
            ))
        );
        assert!(
            document
                .pointer("/components/schemas/GatewayVisibilitySummaryData/properties/range_count")
                .is_some(),
            "visibility summary must retain the runtime range count"
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/GatewayVisibilitySelectionInputData/properties/operator/enum"
            ),
            Some(&json!(["电信", "联通", "移动"]))
        );
        assert!(
            document
                .pointer("/components/schemas/GatewayProxyHeadersUpdateData/properties/items")
                .is_none(),
            "derived gateway item lists must not be accepted as update input"
        );
        for derived_property in ["visibility", "proxy_headers", "host_response"] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/GatewaySettingsUpdateData/properties/{derived_property}"
                    ))
                    .is_none(),
                "derived {derived_property} must not be accepted by gateway settings updates"
            );
        }
        assert_eq!(
            document.pointer("/components/schemas/GatewayPortalUpdateData/properties/version/enum"),
            Some(&json!(["v1", "v2"]))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/GatewayUnmatchedRouteUpdateData/properties/behavior/enum"
            ),
            Some(&json!(["error_page", "reset_connection"]))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1gateway-logs~1config/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/GatewayLoggingConfigUpdateData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1gateway-logs~1entries/delete/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/GatewayLogDeleteBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1gateway-logs~1analytics/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/GatewayLogAnalyticsData"))
        );
        let gateway_log_parameters = document
            .pointer("/paths/~1api~1admin~1gateway-logs~1entries/get/parameters")
            .and_then(Value::as_array)
            .expect("gateway log query parameters");
        for parameter_name in ["pagination", "cursor", "waf_status"] {
            assert!(
                gateway_log_parameters
                    .iter()
                    .any(|parameter| parameter["name"] == parameter_name),
                "missing gateway log {parameter_name} query parameter"
            );
        }
        for property in [
            "auth_rule_group_id",
            "auth_grant_state",
            "upstream_error_class",
        ] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/GatewayLogEntryData/properties/{property}"
                    ))
                    .is_some(),
                "gateway log entries must retain {property}"
            );
        }
        assert!(
            document
                .pointer("/components/schemas/GatewayLogAnalyticsData/properties/clients")
                .is_none(),
            "raw analytics client IPs are internal hydration inputs only"
        );
        for runtime_property in ["logs_dir", "dropped_entries", "queue_size", "queue_depth"] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/GatewayLoggingConfigUpdateData/properties/{runtime_property}"
                    ))
                    .is_none(),
                "runtime logging field {runtime_property} must be read-only"
            );
        }
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1runtime-health/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/RuntimeHealthSnapshotData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1runtime-health~1diagnostics/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/RuntimeDiagnosticsData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1runtime-health~1diagnostics~1archive/get/responses/200/content/application~1zip/schema/format"
            ),
            Some(&json!("binary"))
        );
        assert!(
            document
                .pointer(
                    "/paths/~1api~1admin~1runtime-health~1diagnostics~1archive/get/responses/200/content/application~1json"
                )
                .is_none(),
            "diagnostics archive is not a JSON API envelope"
        );
        let runtime_log_parameters = document
            .pointer("/paths/~1api~1admin~1runtime-health~1logs~1{component}/get/parameters")
            .and_then(Value::as_array)
            .expect("runtime log parameters");
        assert!(runtime_log_parameters.iter().any(|parameter| {
            parameter["name"] == "component"
                && parameter["schema"]["enum"] == json!(["management", "gateway_process"])
        }));
        assert!(runtime_log_parameters.iter().any(|parameter| {
            parameter["name"] == "limit"
                && parameter["schema"]["minimum"] == 1
                && parameter["schema"]["maximum"] == 500
        }));
        let runtime_component_required = document
            .pointer("/components/schemas/RuntimeComponentHealthData/required")
            .and_then(Value::as_array)
            .expect("runtime component required fields");
        for field in [
            "version",
            "commit",
            "pid",
            "instance_id",
            "started_at",
            "last_checked_at",
            "last_success_at",
            "reason_code",
        ] {
            assert!(
                runtime_component_required
                    .iter()
                    .any(|required| required == field),
                "runtime component always emits nullable {field}"
            );
        }
        assert!(
            document
                .pointer("/components/schemas/RuntimeDiagnosticsData/properties/collection")
                .is_some(),
            "diagnostics must declare its collection privacy boundary"
        );
        assert_eq!(
            document
                .pointer("/components/schemas/TypedConfigShadowStatusData/properties/phase/enum"),
            Some(&json!(["typed_primary"]))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/RuntimeDiagnosticsData/properties/storage_migration/$ref"
            ),
            Some(&json!("#/components/schemas/RuntimeStorageMigrationData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ip-location~1batch/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/IpLocationBatchBodyData"))
        );
        assert_eq!(
            document.pointer("/components/schemas/IpLocationBatchBodyData/properties/ips/maxItems"),
            Some(&json!(20))
        );
        assert!(
            document
                .pointer("/components/schemas/IpLocationSnapshotData/properties/result")
                .is_some(),
            "IP location snapshots retain their structured result"
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1config~1ip_location_api~1test-ip-lookup/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/IpLocationConnectionTestData"
            )),
            "connection tests return direct JSON rather than an API data envelope"
        );
        let cidr_city_parameters = document
            .pointer("/paths/~1api~1admin~1cidr~1cities/get/parameters")
            .and_then(Value::as_array)
            .expect("CIDR city parameters");
        assert!(cidr_city_parameters.iter().any(|parameter| {
            parameter["name"] == "province" && parameter["required"] == json!(true)
        }));
        let cidr_lookup_parameters = document
            .pointer("/paths/~1api~1admin~1cidr~1cidrs/get/parameters")
            .and_then(Value::as_array)
            .expect("CIDR lookup parameters");
        assert!(cidr_lookup_parameters.iter().any(|parameter| {
            parameter["name"] == "operator"
                && parameter["schema"]["enum"] == json!(["电信", "联通", "移动"])
        }));
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1security~1overview/get/responses/200/content/application~1json/schema/properties/data/$ref"
            ),
            Some(&json!("#/components/schemas/SecurityOverviewData"))
        );
        assert_eq!(
            document.pointer(
                "/components/schemas/SecurityOverviewSeriesData/properties/failedLogins/items/items"
            ),
            Some(&json!(false)),
            "security overview points must remain exact timestamp/count pairs"
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1scanner~1settings/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/ScannerSettingsUpdateData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1scanner~1blacklist/delete/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/IpListBodyData"))
        );
        for read_only_property in [
            "windowSeconds",
            "cidrExemptionPolicyId",
            "cidrExemptionSourceCidrCount",
            "cidrExemptionRangeCount",
        ] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/ScannerSettingsUpdateData/properties/{read_only_property}"
                    ))
                    .is_none(),
                "scanner updates must not accept read-only {read_only_property}"
            );
        }
        assert_eq!(
            document
                .pointer("/components/schemas/GeneralBlacklistRecordData/properties/source/enum"),
            Some(&json!(["manual", "request_log", "active_ip", "waf_log"]))
        );
        let general_record_required = document
            .pointer("/components/schemas/GeneralBlacklistRecordData/required")
            .and_then(Value::as_array)
            .expect("general blacklist required fields");
        for field in ["source", "comment", "created_at", "updated_at"] {
            assert!(
                general_record_required
                    .iter()
                    .any(|required| required == field),
                "gRPC conversion always emits general blacklist {field}"
            );
        }
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ssh-security~1config/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/SshSecurityConfigUpdateData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1ssh-security~1blocks/delete/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/SshBlocksDeleteBodyData"))
        );
        assert!(
            document
                .pointer(
                    "/components/schemas/SshSecuritySummaryData/properties/allowed_range_count"
                )
                .is_some(),
            "SSH summary must retain the runtime allowed range count"
        );
        for read_only_property in ["configured_at", "updated_at"] {
            assert!(
                document
                    .pointer(&format!(
                        "/components/schemas/SshSecurityConfigUpdateData/properties/{read_only_property}"
                    ))
                    .is_none(),
                "SSH config updates must not accept read-only {read_only_property}"
            );
        }
        for schema in ["SshSecurityConfigData", "SshSecurityConfigUpdateData"] {
            assert_eq!(
                document.pointer(&format!(
                    "/components/schemas/{schema}/properties/block_duration_unit/enum"
                )),
                Some(&json!(["minute", "hour", "day", "month"]))
            );
        }
        assert_eq!(
            document.pointer("/components/schemas/SshSecurityBlockData/properties/reason/enum"),
            Some(&json!(["failed_login_threshold", "cidr_not_allowed"]))
        );
        let ssh_login_parameters = document
            .pointer("/paths/~1api~1admin~1ssh-security~1login-logs/get/parameters")
            .and_then(Value::as_array)
            .expect("SSH login log query parameters");
        assert!(ssh_login_parameters.iter().any(|parameter| {
            parameter["name"] == "outcome"
                && parameter["schema"]["enum"] == json!(["success", "failure"])
        }));
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1whitelist/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!("#/components/schemas/WhitelistAddBodyData"))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1whitelist~1regions/post/requestBody/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/WhitelistRegionAddBodyData"
            ))
        );
        assert_eq!(
            document.pointer(
                "/paths/~1api~1admin~1whitelist~1{id}~1refresh/post/responses/200/content/application~1json/schema/$ref"
            ),
            Some(&json!(
                "#/components/schemas/WhitelistRefreshEnvelopeData"
            )),
            "refresh must allow an HTTP 200 resolver failure with replacement data"
        );
        assert_eq!(
            document.pointer("/components/schemas/WhitelistRecordData/properties/status/enum"),
            Some(&json!(["active", "pending", "expired", "deleted"]))
        );
        assert_eq!(
            document
                .pointer("/components/schemas/WhitelistRegionInputData/properties/operator/enum"),
            Some(&json!(["电信", "联通", "移动"]))
        );
        let whitelist_record_required = document
            .pointer("/components/schemas/WhitelistRecordData/required")
            .and_then(Value::as_array)
            .expect("whitelist record required fields");
        assert!(
            whitelist_record_required
                .iter()
                .any(|required| required == "expireAt"),
            "whitelist records always emit expireAt, including null"
        );
        assert_eq!(
            document.pointer("/paths/~1api~1admin~1ddns~1status/get/x-fn-knock-contract-source"),
            Some(&json!("utoipa"))
        );
        assert_eq!(
            document.pointer("/paths/~1api~1admin~1acme~1status/get/x-fn-knock-contract-source"),
            Some(&json!("utoipa"))
        );
        assert!(
            document["paths"]
                .as_object()
                .into_iter()
                .flat_map(Map::values)
                .filter_map(Value::as_object)
                .flat_map(Map::values)
                .all(|operation| operation["x-fn-knock-contract-source"] != "scanner-fallback"),
            "fully typed OpenAPI must not retain scanner fallbacks"
        );
        for schema in [
            "ApplicationConfigData",
            "AuthCredentialSettingsData",
            "AuthAccountData",
            "AuthModeStatusData",
            "TotpCredentialData",
            "PasskeyCredentialData",
            "CredentialImportSummaryData",
            "CredentialTransferData",
            "OidcProviderCatalogData",
            "OidcProviderData",
            "OidcBindingData",
            "LdapProviderCatalogData",
            "LdapProviderData",
            "LdapBindingData",
            "ExternalAuthInvitationData",
            "PanelBootstrapData",
            "PanelLoginBodyData",
            "PanelLoginRateLimitErrorData",
            "HostMappingData",
            "SessionRecordData",
            "AutomaticBackupDetailsData",
            "BackupImportResultData",
            "WolFeatureConfigData",
            "WolLocalRelayData",
            "WolRelayData",
            "WolTargetData",
            "WolDiscoveryJobData",
            "GatewayVisibilityDetailsData",
            "GatewayProxyHeadersDetailsData",
            "GatewayHostResponseDetailsData",
            "GatewaySettingsData",
            "GatewaySettingsUpdateData",
            "GatewayLoggingConfigData",
            "GatewayLogEntryData",
            "GatewayLogEntriesData",
            "GatewayLogAnalyticsData",
            "RuntimeComponentHealthData",
            "RuntimeHealthSnapshotData",
            "RuntimeComponentLogsData",
            "RuntimeLogClearData",
            "GatewayMemoryConfigData",
            "GatewayMemoryConfigUpdateData",
            "GatewayMemoryReclaimBodyData",
            "GatewayMemoryReclaimData",
            "RuntimeDiagnosticsData",
            "RuntimeDiagnosticsCollectionData",
            "CidrCapabilitiesData",
            "CidrProvincesData",
            "CidrCitiesData",
            "CidrSelectorData",
            "CidrLookupData",
            "IpLocationBatchBodyData",
            "IpLocationSnapshotData",
            "IpLocationBatchData",
            "IpLocationApiConfigData",
            "IpLocationConnectionTestData",
            "CidrConnectionTestData",
            "SecurityOverviewData",
            "ScannerSettingsData",
            "ScannerPathWhitelistData",
            "ScannerFalsePositiveResultData",
            "ScannerBlacklistRecordData",
            "GeneralBlacklistRecordData",
            "GeneralBlacklistMutationData",
            "SshSecurityDetailsData",
            "SshLoginLogListData",
            "SshSecurityBlockData",
            "SshSecurityBlockListData",
            "SshBlocksDeleteBodyData",
            "WhitelistRecordData",
            "WhitelistRegionGroupData",
            "WhitelistRefreshEnvelopeData",
        ] {
            assert!(
                document
                    .pointer(&format!("/components/schemas/{schema}"))
                    .is_some(),
                "missing {schema} schema"
            );
        }
    }

    #[test]
    fn path_parameters_are_declared() {
        let document = document_value();
        let paths = document["paths"].as_object().expect("OpenAPI paths");
        for (path, path_item) in paths {
            let expected = path_parameters(path);
            if expected.is_empty() {
                continue;
            }
            let path_item = path_item.as_object().expect("OpenAPI path item");
            let path_level = path_item
                .get("parameters")
                .and_then(Value::as_array)
                .into_iter()
                .flatten();
            for method in [
                "get", "put", "post", "delete", "options", "head", "patch", "trace",
            ] {
                let Some(operation) = path_item.get(method).and_then(Value::as_object) else {
                    continue;
                };
                let operation_level = operation
                    .get("parameters")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten();
                let declared = path_level
                    .clone()
                    .chain(operation_level)
                    .filter(|parameter| parameter["in"] == "path")
                    .filter_map(|parameter| parameter["name"].as_str())
                    .collect::<BTreeSet<_>>();
                for parameter in &expected {
                    let name = parameter["name"].as_str().expect("path parameter name");
                    assert!(
                        declared.contains(name),
                        "{method} {path} must declare path parameter {name}"
                    );
                }
            }
        }
    }

    #[test]
    fn route_tags_follow_first_admin_segment() {
        assert_eq!(route_tag("/api/admin/ddns/status"), "ddns");
        assert_eq!(route_tag("/api/internal/system-events"), "system-events");
    }
}
