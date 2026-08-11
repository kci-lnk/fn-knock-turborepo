use axum::{http::StatusCode, response::Response};
use serde_json::{Value, json};

use crate::{crypto_utils, response, state::AppState};

use super::super::{
    cloudflare_api::{CloudflareApi, CloudflareApiError},
    managed::dns_record_owned_for_update,
};
use super::{
    CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE, CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR,
    CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE, CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR,
    CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE, CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR,
    CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE, CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR,
    OPTIMIZATION_NOT_READY_ERROR_CODE, OPTIMIZATION_NOT_READY_SCAN_ERROR, OPTIMIZATION_RUNTIME_KEY,
    state_helpers::ensure_object,
};

pub(super) fn optimization_scan_error_code(error: &CloudflareApiError) -> Option<&'static str> {
    match error.message.as_str() {
        CLOUDFLARE_SAAS_REQUIRED_SCAN_ERROR => Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE),
        CLOUDFLARE_SAAS_VALIDATION_PENDING_SCAN_ERROR => {
            Some(CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE)
        }
        CLOUDFLARE_RESOURCE_CONFLICT_SCAN_ERROR => Some(CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE),
        OPTIMIZATION_NOT_READY_SCAN_ERROR => Some(OPTIMIZATION_NOT_READY_ERROR_CODE),
        CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR => {
            Some(CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE)
        }
        _ => None,
    }
}

pub(super) fn weekly_jitter_ms() -> i64 {
    let value = u64::from_le_bytes(crypto_utils::random_bytes::<8>());
    (value % (6 * 60 * 60 * 1000)) as i64
}

pub(super) async fn update_job(state: &AppState, id: &str, patch: Value) {
    let mut jobs = state.tunnel.cloudflared_scan_jobs.write().await;
    let Some(job) = jobs.get_mut(id) else {
        return;
    };
    let target = ensure_object(job);
    if let Some(patch) = patch.as_object() {
        for (key, value) in patch {
            target.insert(key.clone(), value.clone());
        }
    }
}

pub(super) async fn is_job_cancelled(state: &AppState, id: &str) -> bool {
    state
        .tunnel
        .cloudflared_scan_jobs
        .read()
        .await
        .get(id)
        .and_then(|job| job.get("cancelRequested"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

pub(super) async fn load_runtime(state: &AppState) -> Value {
    state
        .storage
        .store
        .get_json_value(OPTIMIZATION_RUNTIME_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            json!({
                "healthFailures": 0,
                "lastHealthAt": Value::Null,
                "lastFullScanAt": Value::Null,
                "nextFullScanAt": Value::Null,
                "lastError": Value::Null,
            })
        })
}

pub(super) async fn save_runtime(
    state: &AppState,
    value: &Value,
) -> Result<(), CloudflareApiError> {
    state
        .storage
        .store
        .set_json_value(OPTIMIZATION_RUNTIME_KEY, value)
        .await
        .map_err(local_error_display)
}

pub(super) fn api_error_response(error: CloudflareApiError) -> Response {
    let status = match error.status {
        Some(StatusCode::UNAUTHORIZED) | Some(StatusCode::FORBIDDEN) => StatusCode::FORBIDDEN,
        Some(StatusCode::CONFLICT) => StatusCode::CONFLICT,
        Some(StatusCode::NOT_FOUND) => StatusCode::NOT_FOUND,
        Some(status) if status.is_client_error() => StatusCode::BAD_REQUEST,
        _ => StatusCode::BAD_GATEWAY,
    };
    response::error(status, error.to_string())
}

pub(in crate::tunnels::cloudflared) fn is_capability_unsupported_api_error(
    error: &CloudflareApiError,
) -> bool {
    if !matches!(
        error.status,
        Some(StatusCode::BAD_REQUEST | StatusCode::FORBIDDEN | StatusCode::PAYMENT_REQUIRED)
    ) {
        return false;
    }
    let message = error.message.to_ascii_lowercase();
    [
        "not entitled",
        "not enabled for this zone",
        "not available on your plan",
        "plan does not support",
        "requires an enterprise plan",
        "upgrade your plan",
        "no quota has been allocated",
        "(1404)",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

pub(super) fn optimization_is_enabled(managed: &Value) -> bool {
    managed.get("mode").and_then(Value::as_str) == Some("managed")
        && managed.get("optimizationEnabled").and_then(Value::as_bool) == Some(true)
}

pub(super) fn local_error(message: impl Into<String>) -> CloudflareApiError {
    CloudflareApiError {
        status: None,
        message: message.into(),
    }
}

pub(super) fn local_error_display(error: impl std::fmt::Display) -> CloudflareApiError {
    local_error(error.to_string())
}

pub(super) fn ignore_not_found(
    result: Result<(), CloudflareApiError>,
) -> Result<(), CloudflareApiError> {
    match result {
        Err(error) if error.status == Some(StatusCode::NOT_FOUND) => Ok(()),
        other => other,
    }
}

pub(super) async fn delete_dns_if_owned(
    api: &CloudflareApi,
    zone_id: &str,
    owned: &Value,
    instance_id: &str,
) -> Result<(), CloudflareApiError> {
    let id = owned.get("id").and_then(Value::as_str).unwrap_or("");
    let name = owned.get("name").and_then(Value::as_str).unwrap_or("");
    let record_type = owned.get("type").and_then(Value::as_str).unwrap_or("");
    let content = owned.get("content").and_then(Value::as_str);
    let proxied = owned
        .get("proxied")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if id.is_empty() || name.is_empty() || record_type.is_empty() {
        return Err(CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: "Managed DNS ownership metadata is incomplete; refusing automatic deletion"
                .to_string(),
        });
    }
    let records = api.list_dns_records(zone_id, Some(name)).await?;
    let Some(remote) = records
        .iter()
        .find(|record| record.get("id").and_then(Value::as_str) == Some(id))
    else {
        return Ok(());
    };
    if !dns_record_owned_for_update(remote, Some(id), instance_id, record_type, content, proxied) {
        return Err(CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: format!(
                "DNS record {name} was claimed or changed by another configuration; refusing automatic deletion"
            ),
        });
    }
    ignore_not_found(api.delete_dns_record(zone_id, id).await)
}
