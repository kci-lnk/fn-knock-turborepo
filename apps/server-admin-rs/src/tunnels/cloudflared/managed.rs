use std::{collections::HashSet, time::Duration};

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{crypto_utils, response, state::AppState, time_utils};

use super::{
    cloudflare_api::{CloudflareApi, CloudflareApiError},
    optimization,
    secrets::{CloudflaredSecretStore, SecretKind},
};

const MANAGED_CONFIG_KEY: &str = "fn_knock:cloudflared:managed:config:v1";
const MANAGED_STATE_KEY: &str = "fn_knock:cloudflared:managed:state:v1";
const PLAN_TTL_MS: i64 = 10 * 60 * 1000;
const DNS_COMMENT_PREFIX: &str = "Managed by fn-knock";
// The Go gateway listens on this dedicated loopback destination so it can
// distinguish managed Cloudflare Tunnel traffic from FRP and other local
// ingress before trusting CF-Connecting-IP for security decisions.
const MANAGED_CLOUDFLARE_INGRESS_PORT: u16 = 17999;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileRequest {
    #[serde(default = "default_action")]
    action: String,
    #[serde(default = "default_tunnel_mode")]
    tunnel_mode: String,
    #[serde(default)]
    tunnel_id: Option<String>,
    #[serde(default)]
    optimization_enabled: bool,
    #[serde(default)]
    delete_dedicated_tunnel: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApplyRequest {
    plan_id: String,
    #[serde(default)]
    takeover_resource_ids: Vec<String>,
}

fn default_action() -> String {
    "apply".to_string()
}

fn default_tunnel_mode() -> String {
    "dedicated".to_string()
}

pub(super) fn openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(save_credential))
        .routes(routes!(delete_credential))
        .routes(routes!(cloudflare_state))
        .routes(routes!(preview_reconcile))
        .routes(routes!(apply_reconcile))
}

pub(super) fn secret_store(state: &AppState) -> CloudflaredSecretStore {
    CloudflaredSecretStore::new(state.settings.data_dir.join("cloudflared"))
}

pub(super) async fn public_config_state(state: &AppState, protocol: &str) -> Value {
    let managed = load_managed_config(state).await;
    json!({
        "mode": managed.get("mode").and_then(Value::as_str).unwrap_or("manual"),
        "protocol": protocol,
        "apiTokenConfigured": secret_store(state).configured(SecretKind::ApiToken),
        "tunnelTokenConfigured": secret_store(state).configured(SecretKind::TunnelToken),
        "accountId": managed.get("accountId").cloned().unwrap_or(Value::Null),
        "zoneId": managed.get("zoneId").cloned().unwrap_or(Value::Null),
        "zoneName": managed.get("zoneName").cloned().unwrap_or(Value::Null),
        "rootDomain": managed.get("rootDomain").cloned().or_else(|| managed.get("zoneName").cloned()).unwrap_or(Value::Null),
        "tunnel": managed.get("tunnel").cloned().unwrap_or(Value::Null),
        "optimizationEnabled": managed.get("optimizationEnabled").and_then(Value::as_bool).unwrap_or(false),
    })
}

pub(super) async fn mark_manual_mode(state: &AppState) -> Result<(), String> {
    let mut config = load_managed_config(state).await;
    ensure_object(&mut config).insert("mode".to_string(), json!("manual"));
    state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, &config)
        .await
        .map_err(|error| error.to_string())
}

#[utoipa::path(put, path = "/api/admin/cloudflared/cloudflare/credential", tag = "cloudflared", operation_id = "put_api_admin_cloudflared_cloudflare_credential", responses((status = 200, description = "Saved Cloudflare credential")))]
async fn save_credential(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let token = body
        .get("apiToken")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if token.is_empty() {
        return response::error(StatusCode::BAD_REQUEST, "Cloudflare API Token is required");
    }
    let local_config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load root domain for Cloudflare credential");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load the current root domain",
            );
        }
    };
    let local_root = match root_domain(&local_config) {
        Ok(root) => root,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let existing_managed = load_managed_config(&state).await;
    let ownership = load_managed_state(&state).await;
    let stored_root = managed_root_domain(&existing_managed);
    let root = if has_managed_resources(&ownership)
        && !stored_root.is_empty()
        && !stored_root.eq_ignore_ascii_case(&local_root)
    {
        stored_root.to_string()
    } else {
        local_root
    };
    let api = CloudflareApi::new(state.fallback_client.clone(), token);
    let zone = match api.find_zone(&root).await {
        Ok(zone) => zone,
        Err(error) => return cloudflare_error_response(error),
    };
    let zone_id = string_field(&zone, "id");
    let zone_name = string_field(&zone, "name");
    let account_id = zone
        .pointer("/account/id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if zone_id.is_empty() || zone_name.is_empty() || account_id.is_empty() {
        return response::error(
            StatusCode::BAD_GATEWAY,
            "Cloudflare did not return a Zone name, Zone ID, and Account ID",
        );
    }
    if let Err(error) = api.verify_token(&account_id).await {
        return cloudflare_error_response(error);
    }
    if let Err(error) = api.list_tunnels(&account_id).await {
        return missing_permission_response("Cloudflare Tunnel Edit", error);
    }
    if let Err(error) = api.list_dns_records(&zone_id, None).await {
        return missing_permission_response("Zone DNS Edit", error);
    }

    let secrets = secret_store(&state);
    let previous_token = match secrets.read(SecretKind::ApiToken) {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to read the previous Cloudflare API Token");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read the existing Cloudflare API credential",
            );
        }
    };
    if let Err(error) = secrets.write(SecretKind::ApiToken, token) {
        tracing::warn!(%error, "failed to persist Cloudflare API Token");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to securely save the Cloudflare API Token",
        );
    }
    let mut managed = existing_managed;
    let object = ensure_object(&mut managed);
    object.insert("mode".to_string(), json!("managed"));
    object.insert("accountId".to_string(), json!(account_id));
    object.insert("zoneId".to_string(), json!(zone_id));
    object.insert("zoneName".to_string(), json!(zone_name));
    object.insert("rootDomain".to_string(), json!(root));
    object
        .entry("instanceId".to_string())
        .or_insert_with(|| json!(uuid::Uuid::new_v4().simple().to_string()));
    object.insert(
        "credentialUpdatedAt".to_string(),
        json!(time_utils::now_iso()),
    );
    if let Err(error) = state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, &managed)
        .await
    {
        let rollback = match previous_token {
            Some(previous) => secrets.write(SecretKind::ApiToken, &previous),
            None => secrets.delete(SecretKind::ApiToken),
        };
        if let Err(rollback_error) = rollback {
            tracing::error!(%rollback_error, "failed to roll back Cloudflare API credential");
        }
        tracing::warn!(%error, "failed to persist Cloudflare managed config");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to save Cloudflare connection state",
        );
    }
    state.tunnel.cloudflared_schedule_notify.notify_one();
    match build_public_state(&state, true).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => cloudflare_error_response(error),
    }
}

#[utoipa::path(delete, path = "/api/admin/cloudflared/cloudflare/credential", tag = "cloudflared", operation_id = "delete_api_admin_cloudflared_cloudflare_credential", responses((status = 200, description = "Deleted Cloudflare credential")))]
async fn delete_credential(State(state): State<AppState>) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    if let Err(error) = secret_store(&state).delete(SecretKind::ApiToken) {
        tracing::warn!(%error, "failed to remove Cloudflare API Token");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to remove the Cloudflare API Token",
        );
    }
    response::success_empty().into_response()
}

#[utoipa::path(get, path = "/api/admin/cloudflared/cloudflare/state", tag = "cloudflared", operation_id = "get_api_admin_cloudflared_cloudflare_state", responses((status = 200, description = "Cloudflare managed state")))]
async fn cloudflare_state(State(state): State<AppState>) -> Response {
    match build_public_state(&state, true).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => cloudflare_error_response(error),
    }
}

pub(super) async fn build_public_state(
    state: &AppState,
    discover_remote: bool,
) -> Result<Value, CloudflareApiError> {
    let managed = load_managed_config(state).await;
    let ownership = load_managed_state(state).await;
    let secrets = secret_store(state);
    let api_configured = secrets.configured(SecretKind::ApiToken);
    let tunnel_configured = secrets.configured(SecretKind::TunnelToken);
    let mut tunnels = Vec::new();
    let mut remote_error = Value::Null;
    if discover_remote && api_configured {
        match cloudflare_api(state).await {
            Ok(api) => {
                let account_id = managed
                    .get("accountId")
                    .and_then(Value::as_str)
                    .unwrap_or("");
                if !account_id.is_empty() {
                    match api.list_tunnels(account_id).await {
                        Ok(items) => {
                            tunnels = items
                                .into_iter()
                                .filter(|item| {
                                    item.get("config_src").and_then(Value::as_str)
                                        == Some("cloudflare")
                                })
                                .map(|item| {
                                    json!({
                                        "id": item.get("id").cloned().unwrap_or(Value::Null),
                                        "name": item.get("name").cloned().unwrap_or(Value::Null),
                                        "status": item.get("status").cloned().unwrap_or(Value::Null),
                                        "connections": item.get("connections").and_then(Value::as_array).map(Vec::len).unwrap_or(0),
                                    })
                                })
                                .collect();
                        }
                        Err(error) => remote_error = json!(error.to_string()),
                    }
                }
            }
            Err(error) => remote_error = json!(error.to_string()),
        }
    }
    let local = state
        .storage
        .store
        .get_config()
        .await
        .unwrap_or_else(|_| json!({}));
    let configured_root = root_domain(&local).unwrap_or_default();
    let stored_root = managed_root_domain(&managed);
    let drift = !stored_root.is_empty() && !configured_root.eq_ignore_ascii_case(stored_root);
    Ok(json!({
        "mode": managed.get("mode").and_then(Value::as_str).unwrap_or("manual"),
        "apiTokenConfigured": api_configured,
        "tunnelTokenConfigured": tunnel_configured,
        "connection": {
            "accountId": managed.get("accountId").cloned().unwrap_or(Value::Null),
            "zoneId": managed.get("zoneId").cloned().unwrap_or(Value::Null),
            "zoneName": managed.get("zoneName").cloned().unwrap_or(Value::Null),
            "configuredRootDomain": configured_root,
            "rootDomainDrift": drift,
            "remoteError": remote_error,
        },
        "tunnels": tunnels,
        "managed": ownership,
        "optimization": optimization::public_state(state, &managed, &ownership).await,
        "permissions": [
            "Account / Cloudflare Tunnel / Edit",
            "Zone / Zone / Read",
            "Zone / DNS / Edit",
            "Zone / SSL and Certificates / Edit (optimization only)"
        ],
    }))
}

#[utoipa::path(post, path = "/api/admin/cloudflared/reconcile/preview", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_reconcile_preview", responses((status = 200, description = "Cloudflare reconcile preview")))]
async fn preview_reconcile(
    State(state): State<AppState>,
    Json(request): Json<ReconcileRequest>,
) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    if request.action != "apply" && request.action != "cleanup" {
        return response::error(StatusCode::BAD_REQUEST, "Unsupported reconcile action");
    }
    if request.tunnel_mode != "dedicated" && request.tunnel_mode != "existing" {
        return response::error(StatusCode::BAD_REQUEST, "Unsupported Tunnel selection mode");
    }
    let api = match cloudflare_api(&state).await {
        Ok(api) => api,
        Err(error) => return cloudflare_error_response(error),
    };
    let plan = match build_plan(&state, &api, &request).await {
        Ok(plan) => plan,
        Err(error) => return cloudflare_error_response(error),
    };
    let plan_id = uuid::Uuid::new_v4().to_string();
    let now = time_utils::now_ms();
    let expires_at = now + PLAN_TTL_MS;
    let mut cached = plan.clone();
    ensure_object(&mut cached).insert("request".to_string(), json!(request));
    ensure_object(&mut cached).insert("createdAtMs".to_string(), json!(now));
    ensure_object(&mut cached).insert("expiresAtMs".to_string(), json!(expires_at));
    let mut plans = state.tunnel.cloudflared_plans.lock().await;
    plans.retain(|_, value| {
        value
            .get("expiresAtMs")
            .and_then(Value::as_i64)
            .is_some_and(|value| value > now)
    });
    if plans.len() >= 20 {
        let oldest = plans
            .iter()
            .min_by_key(|(_, value)| {
                value
                    .get("createdAtMs")
                    .and_then(Value::as_i64)
                    .unwrap_or(i64::MIN)
            })
            .map(|(id, _)| id.clone());
        if let Some(oldest) = oldest {
            plans.remove(&oldest);
        }
    }
    plans.insert(plan_id.clone(), cached);
    drop(plans);
    let mut output = plan;
    let object = ensure_object(&mut output);
    object.insert("planId".to_string(), json!(plan_id));
    object.insert(
        "expiresAt".to_string(),
        json!(time_utils::iso_from_ms(expires_at)),
    );
    response::ok(output).into_response()
}

#[utoipa::path(post, path = "/api/admin/cloudflared/reconcile/apply", tag = "cloudflared", operation_id = "post_api_admin_cloudflared_reconcile_apply", responses((status = 200, description = "Applied Cloudflare reconcile plan")))]
async fn apply_reconcile(
    State(state): State<AppState>,
    Json(body): Json<ApplyRequest>,
) -> Response {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let cached = match state
        .tunnel
        .cloudflared_plans
        .lock()
        .await
        .remove(body.plan_id.trim())
    {
        Some(plan) => plan,
        None => {
            return response::error(
                StatusCode::CONFLICT,
                "Reconcile plan is missing or has already been applied",
            );
        }
    };
    if cached
        .get("expiresAtMs")
        .and_then(Value::as_i64)
        .is_none_or(|value| value <= time_utils::now_ms())
    {
        return response::error(StatusCode::CONFLICT, "Reconcile plan has expired");
    }
    let request = match serde_json::from_value::<ReconcileRequest>(
        cached.get("request").cloned().unwrap_or(Value::Null),
    ) {
        Ok(request) => request,
        Err(_) => return response::error(StatusCode::CONFLICT, "Reconcile plan is invalid"),
    };
    let api = match cloudflare_api(&state).await {
        Ok(api) => api,
        Err(error) => return cloudflare_error_response(error),
    };
    let latest = match build_plan(&state, &api, &request).await {
        Ok(plan) => plan,
        Err(error) => return cloudflare_error_response(error),
    };
    if latest.get("remoteFingerprint") != cached.get("remoteFingerprint") {
        return response::error(
            StatusCode::CONFLICT,
            "Cloudflare state changed after preview; create a new preview before applying",
        );
    }
    let takeover = body
        .takeover_resource_ids
        .iter()
        .map(|value| value.trim().to_string())
        .collect::<HashSet<_>>();
    let blocking_conflicts = latest
        .get("conflicts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|conflict| conflict.get("takeoverAllowed").and_then(Value::as_bool) != Some(true))
        .filter_map(|conflict| conflict.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if !blocking_conflicts.is_empty() {
        return response::error(
            StatusCode::CONFLICT,
            format!(
                "Reconcile plan contains non-takeover conflicts: {}",
                blocking_conflicts.join(", ")
            ),
        );
    }
    let missing_confirmations = latest
        .get("conflicts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|conflict| conflict.get("takeoverAllowed").and_then(Value::as_bool) == Some(true))
        .filter_map(|conflict| conflict.get("id").and_then(Value::as_str))
        .filter(|id| !takeover.contains(*id))
        .collect::<Vec<_>>();
    if !missing_confirmations.is_empty() {
        return response::error(
            StatusCode::CONFLICT,
            format!(
                "Explicit takeover confirmation is required for: {}",
                missing_confirmations.join(", ")
            ),
        );
    }

    let result = if request.action == "cleanup" {
        apply_cleanup(&state, &api, &request, &takeover).await
    } else {
        apply_setup(&state, &api, &request, &takeover).await
    };
    if let Err(error) = result {
        tracing::warn!(%error, "failed to apply Cloudflare reconciliation plan");
        return cloudflare_error_response(error);
    }
    state.tunnel.cloudflared_schedule_notify.notify_one();
    match build_public_state(&state, true).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => cloudflare_error_response(error),
    }
}

async fn build_plan(
    state: &AppState,
    api: &CloudflareApi,
    request: &ReconcileRequest,
) -> Result<Value, CloudflareApiError> {
    let local = state
        .storage
        .store
        .get_config()
        .await
        .map_err(local_error)?;
    let local_root = root_domain(&local).map_err(bad_request_error)?;
    let managed = load_managed_config(state).await;
    let ownership = load_managed_state(state).await;
    let stored_root = managed_root_domain(&managed);
    let root_drift = has_managed_resources(&ownership)
        && !stored_root.is_empty()
        && !stored_root.eq_ignore_ascii_case(&local_root);
    if request.action == "apply" && root_drift {
        return Err(CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: format!(
                "The root domain changed from {stored_root} to {local_root}; remove the previously managed Cloudflare resources before applying the new zone"
            ),
        });
    }
    let root = if request.action == "cleanup" && root_drift {
        stored_root.to_string()
    } else {
        local_root
    };
    let zone = api.find_zone(&root).await?;
    let zone_id = string_field(&zone, "id");
    let zone_name = string_field(&zone, "name");
    let account_id = zone
        .pointer("/account/id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if zone_id.is_empty() || zone_name.is_empty() || account_id.is_empty() {
        return Err(CloudflareApiError {
            status: None,
            message: "Cloudflare Zone response is missing a Zone name, Zone ID, or Account ID"
                .to_string(),
        });
    }
    let tunnels = api.list_tunnels(&account_id).await?;
    let selected_tunnel_id = if request.action == "cleanup" {
        ownership
            .pointer("/tunnel/id")
            .and_then(Value::as_str)
            .filter(|id| {
                tunnels
                    .iter()
                    .any(|item| item.get("id").and_then(Value::as_str) == Some(*id))
            })
            .map(str::to_string)
    } else {
        select_tunnel_id(request, &managed, &tunnels)?
    };
    let tunnel_config = if let Some(tunnel_id) = selected_tunnel_id.as_deref() {
        api.get_tunnel_config(&account_id, tunnel_id).await?
    } else {
        json!({})
    };
    let wildcard = format!("*.{root}");
    let dns_records = api.list_dns_records(&zone_id, Some(&wildcard)).await?;
    let service = local_gateway_service();
    let desired_hosts = if request.optimization_enabled {
        optimization::configured_optimization_hosts(state, &local).await?
    } else {
        configured_hosts(&local)
    };
    let mut operations = Vec::new();
    let mut conflicts = Vec::new();
    let mut optimization_remote = Vec::new();
    let ssl_permission_required = request.optimization_enabled
        || (request.action == "cleanup"
            && (ownership.pointer("/optimization/customHostnames").is_some()
                || ownership
                    .pointer("/optimization/capabilityProbe/id")
                    .is_some()));
    let mut ssl_read_access = false;
    if request.action == "cleanup" {
        append_cleanup_plan(&ownership, request, &mut operations);
        inspect_cleanup_dns(
            &dns_records,
            ownership.get("wildcardDns"),
            "dns:wildcard-dns",
            &managed_instance_id(&managed),
            &mut conflicts,
        );
        let deleting_dedicated_tunnel = request.delete_dedicated_tunnel
            && ownership
                .pointer("/tunnel/ownership")
                .and_then(Value::as_str)
                == Some("dedicated");
        if !deleting_dedicated_tunnel {
            inspect_cleanup_ingress(&tunnel_config, &ownership, &mut operations, &mut conflicts);
        }
        let cleanup_custom_hostnames = if ssl_permission_required {
            match api.list_custom_hostnames(&zone_id, None).await {
                Ok(items) => {
                    ssl_read_access = true;
                    optimization_remote.push(json!({ "customHostnames": items.clone() }));
                    items
                }
                Err(error) => {
                    conflicts.push(custom_hostname_access_conflict(error));
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        optimization::append_cleanup_remote_snapshot(
            api,
            &zone_id,
            &ownership,
            &managed_instance_id(&managed),
            &cleanup_custom_hostnames,
            &mut conflicts,
            &mut optimization_remote,
        )
        .await?;
    } else {
        if selected_tunnel_id.is_none() {
            operations.push(operation(
                "tunnel:create",
                "tunnel",
                "create",
                dedicated_tunnel_name(&root, &managed),
                false,
            ));
        }
        inspect_ingress(
            &tunnel_config,
            &ownership,
            request,
            &wildcard,
            &service,
            &mut operations,
            &mut conflicts,
        );
        inspect_dns(
            &dns_records,
            ownership.pointer("/wildcardDns/id").and_then(Value::as_str),
            "wildcard-dns",
            &wildcard,
            "CNAME",
            selected_tunnel_id
                .as_deref()
                .map(|id| format!("{id}.cfargotunnel.com"))
                .as_deref(),
            true,
            &managed_instance_id(&managed),
            &mut operations,
            &mut conflicts,
        );
        if request.optimization_enabled {
            match api.list_custom_hostnames(&zone_id, None).await {
                Ok(custom_hostnames) => {
                    ssl_read_access = true;
                    optimization_remote.push(json!({
                        "customHostnames": custom_hostnames.clone(),
                    }));
                    optimization::append_preview(
                        api,
                        &zone_id,
                        &root,
                        &managed_instance_id(&managed),
                        &desired_hosts,
                        &ownership,
                        &custom_hostnames,
                        &mut operations,
                        &mut conflicts,
                        &mut optimization_remote,
                    )
                    .await?;
                }
                Err(error) => conflicts.push(custom_hostname_access_conflict(error)),
            }
        }
    }
    let snapshot = json!({
        "accountId": account_id,
        "zoneId": zone_id,
        "zoneName": zone_name,
        "root": root,
        "selectedTunnelId": selected_tunnel_id,
        "desiredHosts": desired_hosts,
        "desiredService": service,
        "tunnelConfig": tunnel_config,
        "wildcardDns": dns_records,
        "optimizationRemote": optimization_remote,
        "managed": ownership,
    });
    let fingerprint = reconcile_plan_fingerprint(&snapshot, &operations, &conflicts);
    let can_apply = conflicts
        .iter()
        .all(|conflict| conflict.get("takeoverAllowed").and_then(Value::as_bool) == Some(true));
    Ok(json!({
        "action": request.action,
        "rootDomain": snapshot.get("root").cloned().unwrap_or(Value::Null),
        "accountId": snapshot.get("accountId").cloned().unwrap_or(Value::Null),
        "zoneId": snapshot.get("zoneId").cloned().unwrap_or(Value::Null),
        "selectedTunnelId": snapshot.get("selectedTunnelId").cloned().unwrap_or(Value::Null),
        "remoteFingerprint": fingerprint,
        "capabilities": {
            "zoneRead": { "required": true, "readable": true, "writeVerified": Value::Null },
            "tunnelEdit": { "required": true, "readable": true, "writeVerified": Value::Null },
            "dnsEdit": { "required": true, "readable": true, "writeVerified": Value::Null },
            "sslCertificatesEdit": {
                "required": ssl_permission_required,
                "readable": if ssl_permission_required {
                    Value::Bool(ssl_read_access)
                } else {
                    Value::Null
                },
                // Cloudflare's token verification response only reports token status. A
                // read-only preview cannot truthfully prove Edit access without mutation;
                // apply reports the authoritative API error if the write scope is absent.
                "writeVerified": Value::Null,
            },
        },
        "operations": operations,
        "conflicts": conflicts,
        "warnings": optimization::plan_warnings(request.optimization_enabled),
        "warningCodes": optimization::plan_warning_codes(request.optimization_enabled),
        "canApply": can_apply,
    }))
}

async fn apply_setup(
    state: &AppState,
    api: &CloudflareApi,
    request: &ReconcileRequest,
    takeover: &HashSet<String>,
) -> Result<(), CloudflareApiError> {
    let local = state
        .storage
        .store
        .get_config()
        .await
        .map_err(local_error)?;
    let root = root_domain(&local).map_err(bad_request_error)?;
    let zone = api.find_zone(&root).await?;
    let zone_id = string_field(&zone, "id");
    let zone_name = string_field(&zone, "name");
    let account_id = zone
        .pointer("/account/id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if zone_id.is_empty() || zone_name.is_empty() || account_id.is_empty() {
        return Err(CloudflareApiError {
            status: None,
            message: "Cloudflare Zone response is missing a Zone name, Zone ID, or Account ID"
                .to_string(),
        });
    }
    let mut managed = load_managed_config(state).await;
    let mut ownership = load_managed_state(state).await;
    let tunnels = api.list_tunnels(&account_id).await?;
    let mut tunnel_id = select_tunnel_id(request, &managed, &tunnels)?;
    let tunnel_ownership;
    let tunnel_name;
    if let Some(selected) = tunnel_id.as_deref() {
        let tunnel = tunnels
            .iter()
            .find(|item| item.get("id").and_then(Value::as_str) == Some(selected));
        tunnel_name = tunnel
            .and_then(|item| item.get("name"))
            .and_then(Value::as_str)
            .unwrap_or(selected)
            .to_string();
        tunnel_ownership = if request.tunnel_mode == "dedicated" {
            "dedicated"
        } else {
            "adopted"
        };
    } else {
        let name = dedicated_tunnel_name(&root, &managed);
        let created = api.create_tunnel(&account_id, &name).await?;
        let created_id = string_field(&created, "id");
        if created_id.is_empty() {
            return Err(CloudflareApiError {
                status: None,
                message: "Cloudflare created a Tunnel without returning its ID".to_string(),
            });
        }
        tunnel_id = Some(created_id);
        tunnel_name = name;
        tunnel_ownership = "dedicated";
    }
    let tunnel_id = tunnel_id.expect("Tunnel ID is present after creation");
    {
        let object = ensure_object(&mut managed);
        object.insert("mode".to_string(), json!("managed"));
        object.insert("accountId".to_string(), json!(account_id));
        object.insert("zoneId".to_string(), json!(zone_id));
        object.insert("zoneName".to_string(), json!(zone_name));
        object.insert("rootDomain".to_string(), json!(root));
        object.insert(
            "tunnel".to_string(),
            json!({ "id": tunnel_id, "name": tunnel_name, "ownership": tunnel_ownership }),
        );
    }
    state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, &managed)
        .await
        .map_err(local_error)?;
    checkpoint_state(
        state,
        &mut ownership,
        "tunnel",
        json!({ "id": tunnel_id, "name": tunnel_name, "ownership": tunnel_ownership }),
    )
    .await?;

    let current = api.get_tunnel_config(&account_id, &tunnel_id).await?;
    let wildcard = format!("*.{root}");
    let service = local_gateway_service();
    let desired = merge_tunnel_config(
        &current,
        &ownership,
        tunnel_ownership == "dedicated",
        &wildcard,
        &service,
        takeover.contains(&format!("ingress:{wildcard}")),
    )?;
    api.update_tunnel_config(&account_id, &tunnel_id, desired.clone())
        .await?;
    let ingress_rule = json!({ "hostname": wildcard, "service": service });
    checkpoint_state(
        state,
        &mut ownership,
        "ingress",
        json!({
            "hostname": format!("*.{root}"),
            "signature": value_fingerprint(&ingress_rule),
            "configFingerprint": value_fingerprint(&desired),
        }),
    )
    .await?;

    let wildcard = format!("*.{root}");
    let target = format!("{tunnel_id}.cfargotunnel.com");
    let dns = upsert_managed_dns(
        api,
        ManagedDnsRequest {
            zone_id: &zone_id,
            name: &wildcard,
            record_type: "CNAME",
            content: &target,
            proxied: true,
            owned_id: ownership.pointer("/wildcardDns/id").and_then(Value::as_str),
            takeover: takeover.contains("dns:wildcard-dns"),
            instance_id: &managed_instance_id(&managed),
        },
    )
    .await?;
    checkpoint_state(state, &mut ownership, "wildcardDns", dns).await?;

    let tunnel_token = api.get_tunnel_token(&account_id, &tunnel_id).await?;
    secret_store(state)
        .write(SecretKind::TunnelToken, &tunnel_token)
        .map_err(local_message_error)?;

    if request.optimization_enabled
        && ownership
            .pointer("/optimization/capabilityProbe/status")
            .and_then(Value::as_str)
            == Some("unsupported")
        && let Some(optimization) = ownership
            .pointer_mut("/optimization")
            .and_then(Value::as_object_mut)
    {
        optimization.remove("capabilityProbe");
    }
    let object = ensure_object(&mut managed);
    object.insert("mode".to_string(), json!("managed"));
    object.insert("accountId".to_string(), json!(account_id));
    object.insert("zoneId".to_string(), json!(zone_id));
    object.insert("zoneName".to_string(), json!(zone_name));
    object.insert("rootDomain".to_string(), json!(root));
    object.insert(
        "optimizationEnabled".to_string(),
        json!(request.optimization_enabled),
    );
    object.insert(
        "tunnel".to_string(),
        json!({ "id": tunnel_id, "name": tunnel_name, "ownership": tunnel_ownership }),
    );
    object.insert("lastAppliedAt".to_string(), json!(time_utils::now_iso()));
    state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, &managed)
        .await
        .map_err(local_error)?;

    // The standard wildcard Tunnel is the safety baseline. Bring it online
    // before any optional Cloudflare for SaaS work so optimization failures
    // can never prevent ordinary Tunnel access.
    let handle = super::ensure_cloudflared_supervisor(state)
        .await
        .map_err(local_message_error)?;
    if handle.snapshot().desired_running {
        handle.restart().await.map_err(local_message_error)?;
    } else {
        handle.start().await.map_err(local_message_error)?;
    }
    state
        .storage
        .store
        .set_config_top_level_value("default_tunnel", json!("cloudflared"))
        .await
        .map_err(local_error)?;

    if request.optimization_enabled {
        optimization::reconcile_resources(
            state,
            api,
            &managed,
            &mut ownership,
            false,
            Some(takeover),
        )
        .await?;
    } else {
        optimization::fallback_to_wildcard(state, api, &managed, &mut ownership).await?;
    }
    state
        .storage
        .store
        .set_json_value(MANAGED_STATE_KEY, &ownership)
        .await
        .map_err(local_error)?;
    Ok(())
}

async fn apply_cleanup(
    state: &AppState,
    api: &CloudflareApi,
    request: &ReconcileRequest,
    takeover: &HashSet<String>,
) -> Result<(), CloudflareApiError> {
    let managed = load_managed_config(state).await;
    let mut ownership = load_managed_state(state).await;
    let zone_id = managed.get("zoneId").and_then(Value::as_str).unwrap_or("");
    let account_id = managed
        .get("accountId")
        .and_then(Value::as_str)
        .unwrap_or("");
    optimization::cleanup_resources(state, api, &managed, &mut ownership).await?;
    if !zone_id.is_empty()
        && let Some(id) = ownership.pointer("/wildcardDns/id").and_then(Value::as_str)
    {
        ignore_not_found(api.delete_dns_record(zone_id, id).await)?;
    }
    let tunnel_id = ownership.pointer("/tunnel/id").and_then(Value::as_str);
    let dedicated = ownership
        .pointer("/tunnel/ownership")
        .and_then(Value::as_str)
        == Some("dedicated");
    let handle = super::ensure_cloudflared_supervisor(state)
        .await
        .map_err(local_message_error)?;
    if let Some(tunnel_id) = tunnel_id
        && !account_id.is_empty()
    {
        if dedicated && request.delete_dedicated_tunnel {
            handle.stop().await.map_err(local_message_error)?;
            ignore_not_found(api.delete_tunnel(account_id, tunnel_id).await)?;
        } else {
            match api.get_tunnel_config(account_id, tunnel_id).await {
                Ok(current) => {
                    let cleaned = remove_owned_ingress(
                        &current,
                        &ownership,
                        takeover.contains("ingress:cleanup"),
                    );
                    api.update_tunnel_config(account_id, tunnel_id, cleaned)
                        .await?;
                }
                Err(error) if error.status == Some(StatusCode::NOT_FOUND) => {}
                Err(error) => return Err(error),
            }
            handle.stop().await.map_err(local_message_error)?;
        }
    } else {
        handle.stop().await.map_err(local_message_error)?;
    }
    secret_store(state)
        .delete(SecretKind::TunnelToken)
        .map_err(local_message_error)?;
    state
        .storage
        .store
        .delete_key(MANAGED_STATE_KEY)
        .await
        .map_err(local_error)?;
    let mut next_managed = managed;
    let object = ensure_object(&mut next_managed);
    object.remove("tunnel");
    object.insert("optimizationEnabled".to_string(), json!(false));
    object.insert("lastCleanupAt".to_string(), json!(time_utils::now_iso()));
    state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, &next_managed)
        .await
        .map_err(local_error)?;
    Ok(())
}

pub(super) async fn cleanup_before_data_clear(state: &AppState) -> Result<(), CloudflareApiError> {
    let _guard = state.tunnel.cloudflared_manage_lock.lock().await;
    let managed = load_managed_config(state).await;
    let ownership = load_managed_state(state).await;
    let secrets = secret_store(state);

    if has_managed_resources(&ownership) {
        let api = cloudflare_api(state)
            .await
            .map_err(|error| CloudflareApiError {
                status: error.status,
                message: format!(
                    "Cloudflare managed resources must be cleaned before local data is cleared: {}",
                    error.message
                ),
            })?;
        let request = ReconcileRequest {
            action: "cleanup".to_string(),
            tunnel_mode: "dedicated".to_string(),
            tunnel_id: None,
            optimization_enabled: managed
                .get("optimizationEnabled")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            delete_dedicated_tunnel: true,
        };
        let plan = build_plan(state, &api, &request).await?;
        let conflicts = plan
            .get("conflicts")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|conflict| conflict.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        if !conflicts.is_empty() {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: format!(
                    "Cloudflare resources changed outside fn-knock; reconcile them before clearing local data: {}",
                    conflicts.join(", ")
                ),
            });
        }
        apply_cleanup(state, &api, &request, &HashSet::new()).await?;
    } else if secrets.configured(SecretKind::TunnelToken) {
        let handle = super::ensure_cloudflared_supervisor(state)
            .await
            .map_err(local_message_error)?;
        handle.stop().await.map_err(local_message_error)?;
    }

    secrets
        .delete(SecretKind::TunnelToken)
        .map_err(local_message_error)?;
    secrets
        .delete(SecretKind::ApiToken)
        .map_err(local_message_error)
}

fn append_cleanup_plan(ownership: &Value, request: &ReconcileRequest, output: &mut Vec<Value>) {
    if ownership.pointer("/optimization").is_some() {
        output.push(operation(
            "optimization:cleanup",
            "optimization",
            "delete",
            "fn-knock optimized hostnames",
            true,
        ));
    }
    if ownership.pointer("/wildcardDns/id").is_some() {
        output.push(operation(
            "dns:wildcard-dns",
            "dns",
            "delete",
            "managed wildcard CNAME",
            true,
        ));
    }
    if request.delete_dedicated_tunnel
        && ownership
            .pointer("/tunnel/ownership")
            .and_then(Value::as_str)
            == Some("dedicated")
    {
        output.push(operation(
            "tunnel:delete",
            "tunnel",
            "delete",
            ownership
                .pointer("/tunnel/name")
                .and_then(Value::as_str)
                .unwrap_or("fn-knock Tunnel"),
            true,
        ));
    }
}

fn inspect_cleanup_ingress(
    current: &Value,
    ownership: &Value,
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
) {
    let hostname = ownership
        .pointer("/ingress/hostname")
        .and_then(Value::as_str)
        .unwrap_or("");
    if hostname.is_empty() {
        return;
    }
    let signature = ownership
        .pointer("/ingress/signature")
        .and_then(Value::as_str)
        .unwrap_or("");
    let matching = current
        .pointer("/config/ingress")
        .or_else(|| current.pointer("/ingress"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .find(|rule| {
            rule.get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
        });
    match matching {
        Some(rule) if value_fingerprint(rule) != signature => conflicts.push(json!({
            "id": "ingress:cleanup",
            "kind": "ingress",
            "target": hostname,
            "messageCode": "managedIngressChanged",
            "message": "The managed Tunnel ingress changed after fn-knock last wrote it",
            "takeoverAllowed": true,
        })),
        Some(_) => operations.push(operation(
            "ingress:cleanup",
            "ingress",
            "delete",
            hostname,
            true,
        )),
        None => operations.push(operation(
            "ingress:cleanup",
            "ingress",
            "keep-deleted",
            hostname,
            true,
        )),
    }
}

fn inspect_cleanup_dns(
    records: &[Value],
    owned: Option<&Value>,
    logical_id: &str,
    instance_id: &str,
    conflicts: &mut Vec<Value>,
) {
    let Some(owned) = owned else {
        return;
    };
    let Some(id) = owned.get("id").and_then(Value::as_str) else {
        return;
    };
    let Some(record) = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
    else {
        return;
    };
    let record_type = owned.get("type").and_then(Value::as_str).unwrap_or("");
    let content = owned.get("content").and_then(Value::as_str);
    let proxied = owned
        .get("proxied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !record_type.is_empty()
        && dns_record_owned_for_update(record, Some(id), instance_id, record_type, content, proxied)
    {
        return;
    }
    conflicts.push(json!({
        "id": logical_id,
        "kind": "dns",
        "target": record.get("name").cloned().unwrap_or(Value::Null),
        "messageCode": "managedDnsChanged",
        "message": "The previously managed DNS record has been claimed or changed by another configuration",
        "takeoverAllowed": true,
    }));
}

fn inspect_ingress(
    current: &Value,
    ownership: &Value,
    request: &ReconcileRequest,
    hostname: &str,
    service: &str,
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
) {
    let ingress = current
        .pointer("/config/ingress")
        .or_else(|| current.pointer("/ingress"))
        .and_then(Value::as_array);
    let existing = ingress.into_iter().flatten().find(|rule| {
        rule.get("hostname")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
    });
    let desired = json!({ "hostname": hostname, "service": service });
    let owned_signature = ownership
        .pointer("/ingress/signature")
        .and_then(Value::as_str);
    match existing {
        None => operations.push(operation(
            &format!("ingress:{hostname}"),
            "ingress",
            "create",
            hostname,
            false,
        )),
        Some(existing) if ingress_rule_matches(existing, &desired) => {
            let owned = request.tunnel_mode == "dedicated"
                || owned_signature.is_some_and(|value| value == value_fingerprint(existing));
            if owned {
                operations.push(operation(
                    &format!("ingress:{hostname}"),
                    "ingress",
                    "keep",
                    hostname,
                    true,
                ));
            } else {
                conflicts.push(json!({
                    "id": format!("ingress:{hostname}"),
                    "kind": "ingress",
                    "target": hostname,
                    "messageCode": "unownedIngress",
                    "message": "An unowned Tunnel ingress rule already uses this hostname",
                    "takeoverAllowed": true,
                }));
            }
        }
        Some(existing) => {
            let owned = request.tunnel_mode == "dedicated"
                || owned_signature.is_some_and(|value| value == value_fingerprint(existing));
            if owned {
                operations.push(operation(
                    &format!("ingress:{hostname}"),
                    "ingress",
                    "update",
                    hostname,
                    true,
                ));
            } else {
                conflicts.push(json!({
                    "id": format!("ingress:{hostname}"),
                    "kind": "ingress",
                    "target": hostname,
                    "messageCode": "unownedIngress",
                    "message": "An unowned Tunnel ingress rule already uses this hostname",
                    "takeoverAllowed": true,
                }));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn inspect_dns(
    records: &[Value],
    owned_id: Option<&str>,
    logical_id: &str,
    name: &str,
    record_type: &str,
    content: Option<&str>,
    proxied: bool,
    instance_id: &str,
    operations: &mut Vec<Value>,
    conflicts: &mut Vec<Value>,
) {
    let matching = records.iter().filter(|record| {
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(name))
    });
    let existing = owned_id
        .and_then(|id| {
            matching
                .clone()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
        })
        .or_else(|| {
            matching
                .clone()
                .find(|record| is_fn_knock_dns(record, instance_id))
        })
        .or_else(|| {
            matching
                .clone()
                .find(|record| record.get("type").and_then(Value::as_str) == Some(record_type))
        })
        .or_else(|| matching.into_iter().next());
    match existing {
        None => operations.push(operation(
            &format!("dns:{logical_id}"),
            "dns",
            "create",
            name,
            false,
        )),
        Some(record) => {
            let matches = record.get("type").and_then(Value::as_str) == Some(record_type)
                && content.is_none_or(|content| dns_content_matches(record, record_type, content))
                && record.get("proxied").and_then(Value::as_bool) == Some(proxied);
            let owned = dns_record_owned_for_update(
                record,
                owned_id,
                instance_id,
                record_type,
                content,
                proxied,
            );
            if !owned {
                conflicts.push(json!({
                    "id": format!("dns:{logical_id}"),
                    "kind": "dns",
                    "target": name,
                    "messageCode": "unownedDns",
                    "message": "An unowned DNS record already uses this hostname",
                    "takeoverAllowed": true,
                }));
            } else {
                operations.push(operation(
                    &format!("dns:{logical_id}"),
                    "dns",
                    if matches { "keep" } else { "update" },
                    name,
                    true,
                ));
            }
        }
    }
}

fn merge_tunnel_config(
    current: &Value,
    ownership: &Value,
    dedicated: bool,
    hostname: &str,
    service: &str,
    takeover: bool,
) -> Result<Value, CloudflareApiError> {
    let desired_rule = json!({ "hostname": hostname, "service": service });
    if dedicated {
        return Ok(json!({
            "ingress": [desired_rule, { "service": "http_status:404" }]
        }));
    }
    let mut config = current.get("config").cloned().unwrap_or_else(|| json!({}));
    let mut ingress = config
        .get("ingress")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let owned_signature = ownership
        .pointer("/ingress/signature")
        .and_then(Value::as_str);
    if let Some(index) = ingress.iter().position(|rule| {
        rule.get("hostname")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(hostname))
    }) {
        let existing = &ingress[index];
        let owned = owned_signature.is_some_and(|value| value == value_fingerprint(existing));
        if !owned && !ingress_rule_matches(existing, &desired_rule) && !takeover {
            return Err(CloudflareApiError {
                status: Some(StatusCode::CONFLICT),
                message: format!("Tunnel ingress {hostname} is owned by another configuration"),
            });
        }
        ingress[index] = desired_rule;
    } else {
        let terminal = ingress
            .iter()
            .position(|rule| rule.get("hostname").is_none())
            .unwrap_or(ingress.len());
        ingress.insert(terminal, desired_rule);
    }
    if !ingress.iter().any(|rule| rule.get("hostname").is_none()) {
        ingress.push(json!({ "service": "http_status:404" }));
    }
    ensure_object(&mut config).insert("ingress".to_string(), Value::Array(ingress));
    Ok(config)
}

fn remove_owned_ingress(current: &Value, ownership: &Value, takeover: bool) -> Value {
    let mut config = current.get("config").cloned().unwrap_or_else(|| json!({}));
    let hostname = ownership
        .pointer("/ingress/hostname")
        .and_then(Value::as_str)
        .unwrap_or("");
    let signature = ownership
        .pointer("/ingress/signature")
        .and_then(Value::as_str)
        .unwrap_or("");
    let ingress = config
        .get("ingress")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|rule| {
            let same_host = rule
                .get("hostname")
                .and_then(Value::as_str)
                .is_some_and(|value| value.eq_ignore_ascii_case(hostname));
            !(same_host && (takeover || value_fingerprint(rule) == signature))
        })
        .collect::<Vec<_>>();
    ensure_object(&mut config).insert("ingress".to_string(), json!(ingress));
    config
}

pub(super) struct ManagedDnsRequest<'a> {
    pub(super) zone_id: &'a str,
    pub(super) name: &'a str,
    pub(super) record_type: &'a str,
    pub(super) content: &'a str,
    pub(super) proxied: bool,
    pub(super) owned_id: Option<&'a str>,
    pub(super) takeover: bool,
    pub(super) instance_id: &'a str,
}

pub(super) async fn upsert_managed_dns(
    api: &CloudflareApi,
    request: ManagedDnsRequest<'_>,
) -> Result<Value, CloudflareApiError> {
    let records = api
        .list_dns_records(request.zone_id, Some(request.name))
        .await?;
    let matching = records.iter().filter(|record| {
        record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(request.name))
    });
    let existing = request
        .owned_id
        .and_then(|id| {
            matching
                .clone()
                .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
        })
        .or_else(|| {
            matching.clone().find(|record| {
                is_fn_knock_dns(record, request.instance_id)
                    && (request.record_type != "TXT"
                        || (record.get("type").and_then(Value::as_str) == Some("TXT")
                            && dns_content_matches(record, "TXT", request.content)))
            })
        })
        .or_else(|| {
            // DNS permits multiple TXT records with the same owner name. In
            // particular, Cloudflare Custom Hostnames can require ownership
            // and certificate validation tokens at the same
            // `_acme-challenge` name. Never claim or overwrite an unrelated
            // TXT record merely because its name matches; create an additive,
            // instance-marked record for each distinct validation value.
            if request.record_type != "TXT" {
                matching
                    .clone()
                    .find(|record| {
                        record.get("type").and_then(Value::as_str) == Some(request.record_type)
                    })
                    .or_else(|| matching.into_iter().next())
            } else {
                None
            }
        });
    let mut body = json!({
        "type": request.record_type,
        "name": request.name,
        "content": request.content,
        "proxied": request.proxied,
        "ttl": if request.proxied { 1 } else { 60 },
        "comment": format!("{DNS_COMMENT_PREFIX} ({})", request.instance_id),
        "tags": ["fn-knock:managed", format!("fn-knock-instance:{}", request.instance_id)]
    });
    let existing_uses_comment_only = existing.is_some_and(|record| {
        record.get("comment").and_then(Value::as_str)
            == Some(format!("{DNS_COMMENT_PREFIX} ({})", request.instance_id).as_str())
            && record
                .get("tags")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
    });
    let existing_id = match existing {
        Some(record) => {
            let id = string_field(record, "id");
            let owned = dns_record_owned_for_update(
                record,
                request.owned_id,
                request.instance_id,
                request.record_type,
                Some(request.content),
                request.proxied,
            );
            if !owned && !request.takeover {
                return Err(CloudflareApiError {
                    status: Some(StatusCode::CONFLICT),
                    message: format!("DNS record {} is not owned by fn-knock", request.name),
                });
            }
            if owned && managed_dns_matches_desired(record, &request) {
                return Ok(managed_dns_result(record, &request));
            }
            Some(id)
        }
        None => None,
    };
    let write = |body| async {
        match existing_id.as_deref() {
            Some(id) => api.update_dns_record(request.zone_id, id, body).await,
            None => api.create_dns_record(request.zone_id, body).await,
        }
    };
    if existing_uses_comment_only && let Some(object) = body.as_object_mut() {
        object.remove("tags");
    }
    let result = match write(body.clone()).await {
        Ok(result) => result,
        Err(error) if dns_tag_quota_is_zero(&error) => {
            // DNS record tags are not available on every Cloudflare plan. The
            // instance-scoped comment remains a durable remote ownership marker,
            // while the saved record ID and desired-value checks continue to
            // protect updates if comments are changed outside fn-knock.
            let mut comment_only_body = body;
            if let Some(object) = comment_only_body.as_object_mut() {
                object.remove("tags");
            }
            tracing::warn!(
                dns_name = request.name,
                "Cloudflare DNS tag quota is zero; retrying with comment-only ownership metadata"
            );
            write(comment_only_body).await?
        }
        Err(error) => return Err(error),
    };
    Ok(managed_dns_result(&result, &request))
}

fn managed_dns_result(record: &Value, request: &ManagedDnsRequest<'_>) -> Value {
    json!({
        "id": record.get("id").cloned().unwrap_or(Value::Null),
        "name": request.name,
        "type": request.record_type,
        "content": request.content,
        "proxied": request.proxied,
    })
}

fn managed_dns_matches_desired(record: &Value, request: &ManagedDnsRequest<'_>) -> bool {
    let expected_comment = format!("{DNS_COMMENT_PREFIX} ({})", request.instance_id);
    let expected_ttl = if request.proxied { 1 } else { 60 };
    record.get("type").and_then(Value::as_str) == Some(request.record_type)
        && record
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(request.name))
        && dns_content_matches(record, request.record_type, request.content)
        && record.get("proxied").and_then(Value::as_bool) == Some(request.proxied)
        && record.get("ttl").and_then(Value::as_u64) == Some(expected_ttl)
        && record.get("comment").and_then(Value::as_str) == Some(expected_comment.as_str())
}

fn dns_tag_quota_is_zero(error: &CloudflareApiError) -> bool {
    error.status == Some(StatusCode::BAD_REQUEST)
        && error.message.contains("(9300)")
        && error.message.to_ascii_lowercase().contains("tag")
        && error.message.to_ascii_lowercase().contains("quota of 0")
}

fn select_tunnel_id(
    request: &ReconcileRequest,
    managed: &Value,
    tunnels: &[Value],
) -> Result<Option<String>, CloudflareApiError> {
    if request.tunnel_mode == "existing" {
        let requested = request
            .tunnel_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| bad_request_error("Select an existing Cloudflare Tunnel".to_string()))?;
        if !tunnels.iter().any(|item| {
            item.get("id").and_then(Value::as_str) == Some(requested)
                && item.get("config_src").and_then(Value::as_str) == Some("cloudflare")
        }) {
            return Err(bad_request_error(
                "The selected Tunnel is missing, deleted, or not remotely managed".to_string(),
            ));
        }
        return Ok(Some(requested.to_string()));
    }
    let stored = managed
        .pointer("/tunnel/id")
        .and_then(Value::as_str)
        .filter(|_| {
            managed.pointer("/tunnel/ownership").and_then(Value::as_str) == Some("dedicated")
        });
    Ok(stored
        .filter(|id| {
            tunnels.iter().any(|item| {
                item.get("id").and_then(Value::as_str) == Some(*id)
                    && item.get("config_src").and_then(Value::as_str) == Some("cloudflare")
            })
        })
        .map(str::to_string))
}

async fn cloudflare_api(state: &AppState) -> Result<CloudflareApi, CloudflareApiError> {
    let token = secret_store(state)
        .read(SecretKind::ApiToken)
        .map_err(local_message_error)?
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| bad_request_error("Cloudflare API Token is not configured".to_string()))?;
    Ok(CloudflareApi::new(state.fallback_client.clone(), token))
}

pub(super) async fn api_for_background(
    state: &AppState,
) -> Result<Option<CloudflareApi>, CloudflareApiError> {
    if !secret_store(state).configured(SecretKind::ApiToken) {
        return Ok(None);
    }
    cloudflare_api(state).await.map(Some)
}

pub(super) async fn load_managed_config(state: &AppState) -> Value {
    state
        .storage
        .store
        .get_json_value(MANAGED_CONFIG_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({ "mode": "manual", "optimizationEnabled": false }))
}

pub(super) async fn save_managed_config(
    state: &AppState,
    value: &Value,
) -> Result<(), CloudflareApiError> {
    state
        .storage
        .store
        .set_json_value(MANAGED_CONFIG_KEY, value)
        .await
        .map_err(local_error)
}

pub(super) async fn load_managed_state(state: &AppState) -> Value {
    state
        .storage
        .store
        .get_json_value(MANAGED_STATE_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| json!({}))
}

fn has_managed_resources(ownership: &Value) -> bool {
    ownership.pointer("/tunnel/id").is_some()
        || ownership.pointer("/wildcardDns/id").is_some()
        || ownership.pointer("/ingress/hostname").is_some()
        || ownership.pointer("/optimization").is_some()
}

pub(super) async fn save_managed_state(
    state: &AppState,
    value: &Value,
) -> Result<(), CloudflareApiError> {
    state
        .storage
        .store
        .set_json_value(MANAGED_STATE_KEY, value)
        .await
        .map_err(local_error)
}

async fn checkpoint_state(
    state: &AppState,
    ownership: &mut Value,
    key: &str,
    value: Value,
) -> Result<(), CloudflareApiError> {
    let object = ensure_object(ownership);
    object.insert(key.to_string(), value);
    object.insert("updatedAt".to_string(), json!(time_utils::now_iso()));
    save_managed_state(state, ownership).await
}

pub(super) fn managed_instance_id(managed: &Value) -> String {
    managed
        .get("instanceId")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("default")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .take(12)
        .collect()
}

pub(super) fn managed_root_domain(managed: &Value) -> &str {
    managed
        .get("rootDomain")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        // Backward compatibility: before parent-Zone discovery was added,
        // zoneName actually stored the configured fn-knock root domain.
        .or_else(|| managed.get("zoneName").and_then(Value::as_str))
        .unwrap_or("")
}

fn dedicated_tunnel_name(root: &str, managed: &Value) -> String {
    let normalized = root
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(40)
        .collect::<String>();
    format!("fn-knock-{normalized}-{}", managed_instance_id(managed))
}

pub(super) fn configured_hosts(config: &Value) -> Vec<String> {
    let root = root_domain(config).unwrap_or_default();
    let mut seen = HashSet::new();
    let mut auth = Vec::new();
    let mut others = Vec::new();
    for mapping in config
        .get("host_mappings")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let host = mapping
            .get("host")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .trim_end_matches('.')
            .to_ascii_lowercase();
        if host.is_empty() || !host.ends_with(&format!(".{root}")) || !seen.insert(host.clone()) {
            continue;
        }
        if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
            auth.push(host);
        } else {
            others.push(host);
        }
    }
    auth.extend(others);
    auth
}

fn root_domain(config: &Value) -> Result<String, String> {
    let raw = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .trim_end_matches('.')
        .to_ascii_lowercase();
    if raw.is_empty() {
        return Err(
            "Configure the reverse-proxy root domain before connecting Cloudflare".to_string(),
        );
    }
    if raw.contains('*') || raw.contains('/') || raw.contains(':') {
        return Err("The configured root domain is invalid".to_string());
    }
    idna::domain_to_ascii(&raw).map_err(|_| "The configured root domain is invalid".to_string())
}

fn local_gateway_service() -> String {
    format!("http://127.0.0.1:{MANAGED_CLOUDFLARE_INGRESS_PORT}")
}

fn operation(id: &str, kind: &str, action: &str, target: impl Into<String>, owned: bool) -> Value {
    json!({
        "id": id,
        "kind": kind,
        "action": action,
        "target": target.into(),
        "owned": owned,
    })
}

fn ingress_rule_matches(existing: &Value, desired: &Value) -> bool {
    existing.get("hostname").and_then(Value::as_str)
        == desired.get("hostname").and_then(Value::as_str)
        && existing.get("service").and_then(Value::as_str)
            == desired.get("service").and_then(Value::as_str)
}

fn value_fingerprint(value: &Value) -> String {
    crypto_utils::sha256_hex_bytes(serde_json::to_vec(value).unwrap_or_default())
}

fn reconcile_plan_fingerprint(
    snapshot: &Value,
    operations: &[Value],
    conflicts: &[Value],
) -> String {
    let mut desired_hosts = snapshot
        .get("desiredHosts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    sort_json_values(&mut desired_hosts);

    let mut stable_operations = operations
        .iter()
        .map(|value| canonical_json(value, false))
        .collect::<Vec<_>>();
    sort_json_values(&mut stable_operations);
    let mut stable_conflicts = conflicts
        .iter()
        .map(|value| canonical_json(value, false))
        .collect::<Vec<_>>();
    sort_json_values(&mut stable_conflicts);

    let tunnel_config = snapshot
        .pointer("/tunnelConfig/config")
        .or_else(|| snapshot.get("tunnelConfig"))
        .map(|value| canonical_json(value, false))
        .unwrap_or(Value::Null);
    let managed = snapshot
        .get("managed")
        .map(|value| canonical_json(value, true))
        .unwrap_or(Value::Null);

    value_fingerprint(&json!({
        "accountId": snapshot.get("accountId").cloned().unwrap_or(Value::Null),
        "zoneId": snapshot.get("zoneId").cloned().unwrap_or(Value::Null),
        "zoneName": snapshot.get("zoneName").cloned().unwrap_or(Value::Null),
        "root": snapshot.get("root").cloned().unwrap_or(Value::Null),
        "selectedTunnelId": snapshot
            .get("selectedTunnelId")
            .cloned()
            .unwrap_or(Value::Null),
        "desiredHosts": desired_hosts,
        "desiredService": snapshot
            .get("desiredService")
            .cloned()
            .unwrap_or(Value::Null),
        "tunnelConfig": tunnel_config,
        "wildcardDns": stable_dns_records(
            snapshot.get("wildcardDns").unwrap_or(&Value::Null)
        ),
        "optimizationRemote": stable_optimization_remote(
            snapshot.get("optimizationRemote").unwrap_or(&Value::Null)
        ),
        "managed": managed,
        "operations": stable_operations,
        "conflicts": stable_conflicts,
    }))
}

fn stable_optimization_remote(value: &Value) -> Value {
    let mut entries = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|entry| {
            let Some(object) = entry.as_object() else {
                return canonical_json(entry, true);
            };
            let mut stable = Map::new();
            for key in ["name", "hostname"] {
                if let Some(value) = object.get(key) {
                    stable.insert(key.to_string(), value.clone());
                }
            }
            if let Some(records) = object.get("dnsRecords") {
                stable.insert("dnsRecords".to_string(), stable_dns_records(records));
            }
            if let Some(hostnames) = object.get("customHostnames") {
                stable.insert(
                    "customHostnames".to_string(),
                    stable_custom_hostnames(hostnames),
                );
            }
            if let Some(fallback) = object.get("fallbackOrigin") {
                stable.insert(
                    "fallbackOrigin".to_string(),
                    fallback.get("origin").cloned().unwrap_or(Value::Null),
                );
            }
            Value::Object(stable)
        })
        .collect::<Vec<_>>();
    sort_json_values(&mut entries);
    Value::Array(entries)
}

fn stable_dns_records(value: &Value) -> Value {
    let mut records = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|record| {
            let mut tags = record
                .get("tags")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            sort_json_values(&mut tags);
            json!({
                "id": record.get("id").cloned().unwrap_or(Value::Null),
                "name": record.get("name").cloned().unwrap_or(Value::Null),
                "type": record.get("type").cloned().unwrap_or(Value::Null),
                "content": record.get("content").cloned().unwrap_or(Value::Null),
                "proxied": record.get("proxied").cloned().unwrap_or(Value::Null),
                "comment": record.get("comment").cloned().unwrap_or(Value::Null),
                "tags": tags,
            })
        })
        .collect::<Vec<_>>();
    sort_json_values(&mut records);
    Value::Array(records)
}

fn stable_custom_hostnames(value: &Value) -> Value {
    let mut hostnames = value
        .as_array()
        .into_iter()
        .flatten()
        .map(|hostname| {
            json!({
                "id": hostname.get("id").cloned().unwrap_or(Value::Null),
                "hostname": hostname
                    .get("hostname")
                    .cloned()
                    .unwrap_or(Value::Null),
                "customOriginServer": hostname
                    .get("custom_origin_server")
                    .cloned()
                    .unwrap_or(Value::Null),
            })
        })
        .collect::<Vec<_>>();
    sort_json_values(&mut hostnames);
    Value::Array(hostnames)
}

fn canonical_json(value: &Value, remove_volatile_fields: bool) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort_unstable();
            let mut canonical = Map::new();
            for key in keys {
                if remove_volatile_fields && volatile_reconcile_field(key) {
                    continue;
                }
                canonical.insert(
                    key.clone(),
                    canonical_json(&object[key], remove_volatile_fields),
                );
            }
            Value::Object(canonical)
        }
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| canonical_json(item, remove_volatile_fields))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn volatile_reconcile_field(key: &str) -> bool {
    matches!(
        key,
        "status"
            | "hostnameStatus"
            | "sslStatus"
            | "errors"
            | "created_at"
            | "updated_at"
            | "created_on"
            | "modified_on"
    ) || key.ends_with("At")
        || key.ends_with("AtMs")
}

fn sort_json_values(values: &mut [Value]) {
    values.sort_by_key(|value| serde_json::to_string(value).unwrap_or_default());
}

fn is_fn_knock_dns(record: &Value, instance_id: &str) -> bool {
    let expected_comment = format!("{DNS_COMMENT_PREFIX} ({instance_id})");
    let expected_tag = format!("fn-knock-instance:{instance_id}");
    record
        .get("comment")
        .and_then(Value::as_str)
        .is_some_and(|value| value == expected_comment)
        || record
            .get("tags")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .any(|value| value.as_str() == Some(expected_tag.as_str()))
}

pub(super) fn dns_record_owned_for_update(
    record: &Value,
    owned_id: Option<&str>,
    instance_id: &str,
    record_type: &str,
    content: Option<&str>,
    proxied: bool,
) -> bool {
    if dns_record_claimed_by_other_instance(record, instance_id) {
        return false;
    }
    if is_fn_knock_dns(record, instance_id) {
        return true;
    }
    owned_id == record.get("id").and_then(Value::as_str)
        && record.get("type").and_then(Value::as_str) == Some(record_type)
        && content.is_none_or(|value| dns_content_matches(record, record_type, value))
        && record.get("proxied").and_then(Value::as_bool) == Some(proxied)
}

fn dns_record_claimed_by_other_instance(record: &Value, instance_id: &str) -> bool {
    let expected_comment = format!("{DNS_COMMENT_PREFIX} ({instance_id})");
    let expected_tag = format!("fn-knock-instance:{instance_id}");
    let other_comment = record
        .get("comment")
        .and_then(Value::as_str)
        .is_some_and(|value| {
            value.starts_with(&format!("{DNS_COMMENT_PREFIX} (")) && value != expected_comment
        });
    let other_tag = record
        .get("tags")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .any(|value| value.starts_with("fn-knock-instance:") && value != expected_tag);
    other_comment || other_tag
}

fn dns_content_matches(record: &Value, record_type: &str, expected: &str) -> bool {
    record
        .get("content")
        .and_then(Value::as_str)
        .is_some_and(|value| {
            if record_type == "TXT" {
                value == expected
            } else {
                value.eq_ignore_ascii_case(expected)
            }
        })
}

fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = json!({});
    }
    value
        .as_object_mut()
        .expect("value was normalized to object")
}

fn string_field(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn cloudflare_error_response(error: CloudflareApiError) -> Response {
    let status = match error.status {
        Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => StatusCode::FORBIDDEN,
        Some(StatusCode::NOT_FOUND) => StatusCode::NOT_FOUND,
        Some(StatusCode::CONFLICT) => StatusCode::CONFLICT,
        Some(status) if status.is_client_error() => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    };
    response::error(status, error.message)
}

fn custom_hostname_access_conflict(error: CloudflareApiError) -> Value {
    if optimization::is_capability_unsupported_api_error(&error) {
        return json!({
            "id": "capability:cloudflare-for-saas",
            "kind": "capability",
            "target": "Cloudflare for SaaS",
            "messageCode": "cloudflareSaasUnavailable",
            "detail": error.to_string(),
            "message": format!(
                "Cloudflare for SaaS is not enabled or has no Custom Hostname quota for this Zone. Enable it in the Cloudflare dashboard before applying optimization: {error}"
            ),
            "takeoverAllowed": false,
        });
    }
    json!({
        "id": "permission:ssl-certificates",
        "kind": "permission",
        "target": "SSL and Certificates Edit",
        "messageCode": "permissionError",
        "detail": error.to_string(),
        "message": error.to_string(),
        "takeoverAllowed": false,
    })
}

fn missing_permission_response(permission: &str, error: CloudflareApiError) -> Response {
    if matches!(
        error.status,
        Some(StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
    ) {
        response::error(
            StatusCode::FORBIDDEN,
            format!("Cloudflare Token needs {permission}: {error}"),
        )
    } else {
        // DNS/Tunnel permission probes are also real network requests. Do not
        // turn timeouts, TLS failures, DNS failures, or Cloudflare 5xx replies
        // into a misleading missing-permission diagnosis.
        cloudflare_error_response(error)
    }
}

fn local_error(error: impl std::fmt::Display) -> CloudflareApiError {
    CloudflareApiError {
        status: None,
        message: error.to_string(),
    }
}

fn local_message_error(message: impl Into<String>) -> CloudflareApiError {
    CloudflareApiError {
        status: None,
        message: message.into(),
    }
}

fn bad_request_error(message: String) -> CloudflareApiError {
    CloudflareApiError {
        status: Some(StatusCode::BAD_REQUEST),
        message,
    }
}

fn ignore_not_found(result: Result<(), CloudflareApiError>) -> Result<(), CloudflareApiError> {
    match result {
        Err(error) if error.status == Some(StatusCode::NOT_FOUND) => Ok(()),
        other => other,
    }
}

pub(super) fn plan_wakeup_delay() -> Duration {
    Duration::from_secs(60)
}

#[cfg(test)]
mod tests {
    use axum::{
        Json, Router,
        routing::{get, patch, post},
    };
    use reqwest::Client;

    use super::*;

    #[test]
    fn managed_tunnel_uses_dedicated_loopback_ingress() {
        assert_eq!(local_gateway_service(), "http://127.0.0.1:17999");
    }

    #[test]
    fn reconcile_fingerprint_ignores_cloudflare_status_churn_and_collection_order() {
        let operations = vec![operation(
            "custom-hostname:auth.example.com",
            "custom-hostname",
            "recover",
            "auth.example.com",
            true,
        )];
        let conflicts = vec![json!({
            "id": "custom-hostname:app.example.com",
            "kind": "custom-hostname",
            "target": "app.example.com",
            "messageCode": "unownedCustomHostname",
            "takeoverAllowed": true,
        })];
        let first = json!({
            "accountId": "account-id",
            "zoneId": "zone-id",
            "zoneName": "example.com",
            "root": "example.com",
            "selectedTunnelId": "tunnel-id",
            "desiredHosts": ["app.example.com", "auth.example.com"],
            "desiredService": "http://127.0.0.1:17999",
            "tunnelConfig": {
                "version": 10,
                "config": { "ingress": [
                    { "hostname": "*.example.com", "service": "http://127.0.0.1:17999" },
                    { "service": "http_status:404" }
                ] }
            },
            "wildcardDns": [{
                "id": "wildcard-id",
                "name": "*.example.com",
                "type": "CNAME",
                "content": "tunnel-id.cfargotunnel.com",
                "proxied": true,
                "comment": "Managed by fn-knock (instance)",
                "tags": ["fn-knock:managed", "fn-knock-instance:instance"],
                "modified_on": "2026-08-12T00:00:00Z"
            }],
            "optimizationRemote": [
                { "customHostnames": [
                    {
                        "id": "custom-app",
                        "hostname": "app.example.com",
                        "custom_origin_server": "fnknock-origin-instance.example.com",
                        "status": "pending",
                        "ssl": { "status": "pending_validation" },
                        "created_at": "2026-08-12T00:00:00Z"
                    },
                    {
                        "id": "custom-auth",
                        "hostname": "auth.example.com",
                        "custom_origin_server": "fnknock-origin-instance.example.com",
                        "status": "active",
                        "ssl": { "status": "active" }
                    }
                ] },
                { "fallbackOrigin": {
                    "origin": "fnknock-origin-instance.example.com",
                    "status": "pending_deployment",
                    "updated_at": "2026-08-12T00:00:00Z"
                } },
                { "hostname": "auth.example.com", "dnsRecords": [{
                    "id": "auth-dns",
                    "name": "auth.example.com",
                    "type": "CNAME",
                    "content": "fnknock-edge-instance.example.com",
                    "proxied": false,
                    "comment": "Managed by fn-knock (instance)",
                    "tags": ["fn-knock-instance:instance", "fn-knock:managed"],
                    "modified_on": "2026-08-12T00:00:00Z"
                }] }
            ],
            "managed": { "optimization": {
                "selected": { "ip": "104.16.1.1", "selectedAt": "2026-08-12T00:00:00Z" },
                "customHostnames": { "auth.example.com": {
                    "id": "custom-auth",
                    "status": "pending",
                    "hostnameStatus": "pending",
                    "sslStatus": "pending_validation",
                    "updatedAt": "2026-08-12T00:00:00Z"
                } }
            } }
        });
        let mut second = first.clone();
        second["desiredHosts"] = json!(["auth.example.com", "app.example.com"]);
        second["tunnelConfig"]["version"] = json!(11);
        second["wildcardDns"][0]["tags"] =
            json!(["fn-knock-instance:instance", "fn-knock:managed"]);
        second["wildcardDns"][0]["modified_on"] = json!("2026-08-12T00:01:00Z");
        second["optimizationRemote"][0]["customHostnames"]
            .as_array_mut()
            .expect("custom hostnames")
            .reverse();
        second["optimizationRemote"][0]["customHostnames"][0]["status"] = json!("active");
        second["optimizationRemote"][0]["customHostnames"][0]["ssl"] =
            json!({ "status": "active" });
        second["optimizationRemote"][1]["fallbackOrigin"]["status"] = json!("active");
        second["optimizationRemote"][1]["fallbackOrigin"]["updated_at"] =
            json!("2026-08-12T00:01:00Z");
        second["optimizationRemote"][2]["dnsRecords"][0]["modified_on"] =
            json!("2026-08-12T00:01:00Z");
        second["managed"]["optimization"]["customHostnames"]["auth.example.com"]["status"] =
            json!("active");
        second["managed"]["optimization"]["customHostnames"]["auth.example.com"]["hostnameStatus"] =
            json!("active");
        second["managed"]["optimization"]["customHostnames"]["auth.example.com"]["sslStatus"] =
            json!("active");
        second["managed"]["optimization"]["customHostnames"]["auth.example.com"]["updatedAt"] =
            json!("2026-08-12T00:01:00Z");

        assert_eq!(
            reconcile_plan_fingerprint(&first, &operations, &conflicts),
            reconcile_plan_fingerprint(&second, &operations, &conflicts)
        );
    }

    #[test]
    fn reconcile_fingerprint_retains_security_relevant_changes() {
        let operations = vec![operation(
            "dns:wildcard-dns",
            "dns",
            "update",
            "*.example.com",
            true,
        )];
        let snapshot = json!({
            "accountId": "account-id",
            "zoneId": "zone-id",
            "zoneName": "example.com",
            "root": "example.com",
            "selectedTunnelId": "tunnel-id",
            "desiredHosts": ["auth.example.com"],
            "desiredService": "http://127.0.0.1:17999",
            "tunnelConfig": { "config": { "ingress": [
                { "hostname": "*.example.com", "service": "http://127.0.0.1:17999" },
                { "service": "http_status:404" }
            ] } },
            "wildcardDns": [{
                "id": "wildcard-id",
                "name": "*.example.com",
                "type": "CNAME",
                "content": "tunnel-id.cfargotunnel.com",
                "proxied": true,
                "comment": "Managed by fn-knock (instance)",
                "tags": ["fn-knock-instance:instance"]
            }],
            "optimizationRemote": [{ "customHostnames": [{
                "id": "custom-auth",
                "hostname": "auth.example.com",
                "custom_origin_server": "fnknock-origin-instance.example.com"
            }] }],
            "managed": { "optimization": { "selected": { "ip": "104.16.1.1" } } }
        });
        let original = reconcile_plan_fingerprint(&snapshot, &operations, &[]);

        for changed in [
            {
                let mut value = snapshot.clone();
                value["tunnelConfig"]["config"]["ingress"][0]["service"] =
                    json!("http://127.0.0.1:18000");
                value
            },
            {
                let mut value = snapshot.clone();
                value["wildcardDns"][0]["content"] = json!("other.cfargotunnel.com");
                value
            },
            {
                let mut value = snapshot.clone();
                value["optimizationRemote"][0]["customHostnames"][0]["custom_origin_server"] =
                    json!("third-party.example.net");
                value
            },
            {
                let mut value = snapshot.clone();
                value["managed"]["optimization"]["selected"]["ip"] = json!("104.16.2.2");
                value
            },
        ] {
            assert_ne!(
                original,
                reconcile_plan_fingerprint(&changed, &operations, &[])
            );
        }
    }

    #[test]
    fn permission_probe_only_labels_cloudflare_auth_failures_as_missing_scope() {
        let forbidden = missing_permission_response(
            "Zone DNS Edit",
            CloudflareApiError {
                status: Some(StatusCode::FORBIDDEN),
                message: "permission denied".to_string(),
            },
        );
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        let network_failure = missing_permission_response(
            "Zone DNS Edit",
            CloudflareApiError {
                status: None,
                message: "error sending request".to_string(),
            },
        );
        assert_eq!(network_failure.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn inserts_owned_ingress_before_existing_catch_all() {
        let current = json!({
            "config": {
                "warp-routing": { "enabled": true },
                "ingress": [
                    { "hostname": "other.example.com", "service": "http://127.0.0.1:80" },
                    { "service": "http_status:418" }
                ]
            }
        });
        let merged = merge_tunnel_config(
            &current,
            &json!({}),
            false,
            "*.example.com",
            "http://127.0.0.1:7999",
            false,
        )
        .unwrap();
        assert_eq!(
            merged.pointer("/ingress/1/hostname"),
            Some(&json!("*.example.com"))
        );
        assert_eq!(
            merged.pointer("/ingress/2/service"),
            Some(&json!("http_status:418"))
        );
        assert_eq!(merged.pointer("/warp-routing/enabled"), Some(&json!(true)));
    }

    #[test]
    fn refuses_to_replace_unowned_ingress_without_takeover() {
        let current = json!({
            "config": { "ingress": [
                { "hostname": "*.example.com", "service": "http://third-party" },
                { "service": "http_status:404" }
            ] }
        });
        let error = merge_tunnel_config(
            &current,
            &json!({}),
            false,
            "*.example.com",
            "http://127.0.0.1:7999",
            false,
        )
        .unwrap_err();
        assert_eq!(error.status, Some(StatusCode::CONFLICT));
    }

    #[test]
    fn matching_unowned_ingress_still_requires_explicit_takeover() {
        let current = json!({
            "config": { "ingress": [
                { "hostname": "*.example.com", "service": "http://127.0.0.1:7999" },
                { "service": "http_status:404" }
            ] }
        });
        let request = ReconcileRequest {
            action: "apply".to_string(),
            tunnel_mode: "existing".to_string(),
            tunnel_id: Some("third-party-tunnel".to_string()),
            optimization_enabled: false,
            delete_dedicated_tunnel: false,
        };
        let mut operations = Vec::new();
        let mut conflicts = Vec::new();
        inspect_ingress(
            &current,
            &json!({}),
            &request,
            "*.example.com",
            "http://127.0.0.1:7999",
            &mut operations,
            &mut conflicts,
        );

        assert!(operations.is_empty());
        assert_eq!(
            conflicts.first().and_then(|value| value.get("id")),
            Some(&json!("ingress:*.example.com"))
        );
    }

    #[test]
    fn cleanup_requires_takeover_when_the_owned_ingress_signature_drifted() {
        let original = json!({
            "hostname": "*.example.com",
            "service": "http://127.0.0.1:7999"
        });
        let current = json!({
            "config": { "ingress": [
                { "hostname": "*.example.com", "service": "http://third-party" },
                { "service": "http_status:404" }
            ] }
        });
        let ownership = json!({
            "ingress": {
                "hostname": "*.example.com",
                "signature": value_fingerprint(&original)
            }
        });
        let mut operations = Vec::new();
        let mut conflicts = Vec::new();
        inspect_cleanup_ingress(&current, &ownership, &mut operations, &mut conflicts);
        assert!(operations.is_empty());
        assert_eq!(conflicts[0]["id"], json!("ingress:cleanup"));
        assert_eq!(conflicts[0]["takeoverAllowed"], json!(true));

        let preserved = remove_owned_ingress(&current, &ownership, false);
        assert_eq!(preserved["ingress"].as_array().map(Vec::len), Some(2));
        let removed = remove_owned_ingress(&current, &ownership, true);
        assert_eq!(removed["ingress"].as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn orders_auth_host_before_other_optimized_hosts() {
        let config = json!({
            "subdomain_mode": { "root_domain": "example.com" },
            "host_mappings": [
                { "host": "app.example.com" },
                { "host": "auth.example.com", "service_role": "auth" },
                { "host": "outside.test" }
            ]
        });
        assert_eq!(
            configured_hosts(&config),
            vec!["auth.example.com", "app.example.com"]
        );
    }

    #[test]
    fn dns_ownership_never_crosses_fn_knock_instances() {
        let own = json!({
            "comment": "Managed by fn-knock (instance-a)",
            "tags": ["fn-knock:managed", "fn-knock-instance:instance-a"]
        });
        let other = json!({
            "comment": "Managed by fn-knock (instance-b)",
            "tags": ["fn-knock:managed", "fn-knock-instance:instance-b"]
        });
        let legacy_generic = json!({ "tags": ["fn-knock:managed"] });
        assert!(is_fn_knock_dns(&own, "instance-a"));
        assert!(!is_fn_knock_dns(&other, "instance-a"));
        assert!(!is_fn_knock_dns(&legacy_generic, "instance-a"));
    }

    #[test]
    fn a_known_dns_id_requires_a_marker_or_unchanged_desired_content() {
        let expected = json!({
            "id": "record-id",
            "type": "CNAME",
            "content": "tunnel.cfargotunnel.com",
            "proxied": true
        });
        assert!(dns_record_owned_for_update(
            &expected,
            Some("record-id"),
            "instance-a",
            "CNAME",
            Some("tunnel.cfargotunnel.com"),
            true,
        ));

        let drifted = json!({
            "id": "record-id",
            "type": "CNAME",
            "content": "third-party.example.net",
            "proxied": true
        });
        assert!(!dns_record_owned_for_update(
            &drifted,
            Some("record-id"),
            "instance-a",
            "CNAME",
            Some("tunnel.cfargotunnel.com"),
            true,
        ));

        let claimed_by_other = json!({
            "id": "record-id",
            "type": "CNAME",
            "content": "tunnel.cfargotunnel.com",
            "proxied": true,
            "tags": ["fn-knock-instance:instance-b"]
        });
        assert!(!dns_record_owned_for_update(
            &claimed_by_other,
            Some("record-id"),
            "instance-a",
            "CNAME",
            Some("tunnel.cfargotunnel.com"),
            true,
        ));
    }

    #[test]
    fn cleanup_requires_takeover_for_a_drifted_wildcard_dns_record() {
        let owned = json!({
            "id": "record-id",
            "name": "*.example.com",
            "type": "CNAME",
            "content": "tunnel.cfargotunnel.com",
            "proxied": true
        });
        let remote = vec![json!({
            "id": "record-id",
            "name": "*.example.com",
            "type": "CNAME",
            "content": "third-party.example.net",
            "proxied": true
        })];
        let mut conflicts = Vec::new();
        inspect_cleanup_dns(
            &remote,
            Some(&owned),
            "dns:wildcard-dns",
            "instance-a",
            &mut conflicts,
        );
        assert_eq!(conflicts[0]["id"], json!("dns:wildcard-dns"));
        assert_eq!(conflicts[0]["takeoverAllowed"], json!(true));
    }

    #[tokio::test]
    async fn dns_only_takeover_never_enables_cloudflare_proxying() {
        async fn list_records() -> Json<Value> {
            Json(json!({
                "success": true,
                "result": [{
                    "id": "third-party-record",
                    "name": "app.example.com",
                    "type": "CNAME",
                    "content": "old.example.net",
                    "proxied": true
                }],
                "result_info": { "total_pages": 1 }
            }))
        }

        async fn update_record(Json(body): Json<Value>) -> Json<Value> {
            assert_eq!(body.get("proxied"), Some(&json!(false)));
            assert_eq!(body.get("ttl"), Some(&json!(60)));
            Json(json!({
                "success": true,
                "result": {
                    "id": "third-party-record",
                    "name": "app.example.com",
                    "type": "CNAME",
                    "content": "fnknock-edge.example.com",
                    "proxied": false
                }
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/zones/zone-id/dns_records", get(list_records))
                    .route(
                        "/zones/zone-id/dns_records/third-party-record",
                        patch(update_record),
                    ),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        let record = upsert_managed_dns(
            &api,
            ManagedDnsRequest {
                zone_id: "zone-id",
                name: "app.example.com",
                record_type: "CNAME",
                content: "fnknock-edge.example.com",
                proxied: false,
                owned_id: None,
                takeover: true,
                instance_id: "test",
            },
        )
        .await
        .expect("take over exact DNS record");
        assert_eq!(record.get("proxied"), Some(&json!(false)));
        server.abort();
    }

    #[tokio::test]
    async fn retries_dns_writes_without_tags_when_the_zone_tag_quota_is_zero() {
        async fn list_records() -> Json<Value> {
            Json(json!({
                "success": true,
                "result": [],
                "result_info": { "total_pages": 1 }
            }))
        }

        async fn create_record(Json(body): Json<Value>) -> (StatusCode, Json<Value>) {
            use std::sync::atomic::{AtomicUsize, Ordering};

            static ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
            let attempt = ATTEMPTS.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                assert_eq!(body["tags"].as_array().map(Vec::len), Some(2));
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "errors": [{
                            "code": 9300,
                            "message": "DNS record has 2 tags, exceeding the quota of 0."
                        }]
                    })),
                );
            }

            assert!(body.get("tags").is_none());
            assert_eq!(body["comment"], json!("Managed by fn-knock (test)"));
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "result": {
                        "id": "record-id",
                        "name": "*.example.com",
                        "type": "CNAME",
                        "content": "tunnel.cfargotunnel.com",
                        "proxied": true,
                        "comment": "Managed by fn-knock (test)",
                        "tags": []
                    }
                })),
            )
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/zones/zone-id/dns_records", get(list_records))
                    .route("/zones/zone-id/dns_records", post(create_record)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        let record = upsert_managed_dns(
            &api,
            ManagedDnsRequest {
                zone_id: "zone-id",
                name: "*.example.com",
                record_type: "CNAME",
                content: "tunnel.cfargotunnel.com",
                proxied: true,
                owned_id: None,
                takeover: false,
                instance_id: "test",
            },
        )
        .await
        .expect("retry DNS creation without unsupported tags");
        assert_eq!(record["id"], json!("record-id"));
        server.abort();
    }

    #[tokio::test]
    async fn validation_txt_is_created_without_overwriting_same_name_records() {
        async fn list_records() -> Json<Value> {
            Json(json!({
                "success": true,
                "result": [
                    {
                        "id": "third-party-record",
                        "name": "_acme-challenge.app.example.com",
                        "type": "TXT",
                        "content": "third-party-token",
                        "proxied": false
                    },
                    {
                        "id": "existing-managed-token",
                        "name": "_acme-challenge.app.example.com",
                        "type": "TXT",
                        "content": "first-cloudflare-token",
                        "proxied": false,
                        "comment": "Managed by fn-knock (test)"
                    }
                ],
                "result_info": { "total_pages": 1 }
            }))
        }

        async fn create_record(Json(body): Json<Value>) -> Json<Value> {
            assert_eq!(body["type"], json!("TXT"));
            assert_eq!(body["content"], json!("second-cloudflare-token"));
            assert_eq!(body["proxied"], json!(false));
            Json(json!({
                "success": true,
                "result": {
                    "id": "new-managed-token",
                    "name": "_acme-challenge.app.example.com",
                    "type": "TXT",
                    "content": "second-cloudflare-token",
                    "proxied": false,
                    "ttl": 60,
                    "comment": "Managed by fn-knock (test)"
                }
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route(
                    "/zones/zone-id/dns_records",
                    get(list_records).post(create_record),
                ),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        let record = upsert_managed_dns(
            &api,
            ManagedDnsRequest {
                zone_id: "zone-id",
                name: "_acme-challenge.app.example.com",
                record_type: "TXT",
                content: "second-cloudflare-token",
                proxied: false,
                owned_id: None,
                takeover: false,
                instance_id: "test",
            },
        )
        .await
        .expect("create a distinct validation TXT record");
        assert_eq!(record["id"], json!("new-managed-token"));
        server.abort();
    }

    #[tokio::test]
    async fn unchanged_comment_only_dns_records_do_not_trigger_remote_writes() {
        async fn list_records() -> Json<Value> {
            Json(json!({
                "success": true,
                "result": [{
                    "id": "record-id",
                    "name": "*.example.com",
                    "type": "CNAME",
                    "content": "tunnel.cfargotunnel.com",
                    "proxied": true,
                    "ttl": 1,
                    "comment": "Managed by fn-knock (test)",
                    "tags": []
                }],
                "result_info": { "total_pages": 1 }
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/zones/zone-id/dns_records", get(list_records)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        let record = upsert_managed_dns(
            &api,
            ManagedDnsRequest {
                zone_id: "zone-id",
                name: "*.example.com",
                record_type: "CNAME",
                content: "tunnel.cfargotunnel.com",
                proxied: true,
                owned_id: Some("record-id"),
                takeover: false,
                instance_id: "test",
            },
        )
        .await
        .expect("keep unchanged comment-only DNS record");
        assert_eq!(record["id"], json!("record-id"));
        server.abort();
    }

    #[tokio::test]
    async fn drifted_comment_only_dns_records_update_without_retrying_tags() {
        async fn list_records() -> Json<Value> {
            Json(json!({
                "success": true,
                "result": [{
                    "id": "record-id",
                    "name": "edge.example.com",
                    "type": "A",
                    "content": "104.16.1.1",
                    "proxied": false,
                    "ttl": 60,
                    "comment": "Managed by fn-knock (test)",
                    "tags": []
                }],
                "result_info": { "total_pages": 1 }
            }))
        }

        async fn update_record(Json(body): Json<Value>) -> Json<Value> {
            assert!(body.get("tags").is_none());
            assert_eq!(body["content"], json!("104.16.2.2"));
            Json(json!({
                "success": true,
                "result": {
                    "id": "record-id",
                    "name": "edge.example.com",
                    "type": "A",
                    "content": "104.16.2.2",
                    "proxied": false,
                    "ttl": 60,
                    "comment": "Managed by fn-knock (test)",
                    "tags": []
                }
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/zones/zone-id/dns_records", get(list_records))
                    .route("/zones/zone-id/dns_records/record-id", patch(update_record)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        upsert_managed_dns(
            &api,
            ManagedDnsRequest {
                zone_id: "zone-id",
                name: "edge.example.com",
                record_type: "A",
                content: "104.16.2.2",
                proxied: false,
                owned_id: Some("record-id"),
                takeover: false,
                instance_id: "test",
            },
        )
        .await
        .expect("update comment-only DNS record without tags");
        server.abort();
    }

    #[test]
    fn missing_custom_hostname_quota_is_not_reported_as_a_token_permission() {
        let conflict = custom_hostname_access_conflict(CloudflareApiError {
            status: Some(StatusCode::BAD_REQUEST),
            message: "No quota has been allocated for this zone or for this account. (1404)"
                .to_string(),
        });
        assert_eq!(
            conflict.get("id"),
            Some(&json!("capability:cloudflare-for-saas"))
        );
        assert_eq!(conflict.get("kind"), Some(&json!("capability")));
    }
}
