use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{
    app_version::APP_LOCAL_VERSION,
    store::{TotpCredential, normalize_totp_access_scopes, normalize_totp_subdomain_access},
    time_utils,
};

use super::{
    MAX_TOTP_IMPORT_COUNT, TOTP_TRANSFER_KIND, TOTP_TRANSFER_VERSION, TotpImportRouteError,
    text::{totp_import_error, totp_import_error_with_max},
};

pub(super) use crate::http_utils::url_encode_component as percent_encode;

pub(super) fn build_totp_import_plan(
    existing: &[TotpCredential],
    payload: &Value,
) -> Result<(Vec<TotpCredential>, Value), TotpImportRouteError> {
    if !payload.is_object() {
        return Err(totp_import_error(StatusCode::BAD_REQUEST, "payloadObject"));
    }
    if payload.get("kind").and_then(Value::as_str) != Some(TOTP_TRANSFER_KIND) {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedKind",
        ));
    }
    if payload.get("version").and_then(Value::as_u64) != Some(TOTP_TRANSFER_VERSION) {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedVersion",
        ));
    }
    let Some(items) = payload.get("credentials").and_then(Value::as_array) else {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "credentialsArray",
        ));
    };
    if items.len() > MAX_TOTP_IMPORT_COUNT {
        return Err(totp_import_error_with_max(
            StatusCode::BAD_REQUEST,
            "countExceeded",
            MAX_TOTP_IMPORT_COUNT,
        ));
    }

    let mut summary = json!({
        "imported": 0,
        "skipped_existing_id": 0,
        "skipped_existing_secret": 0,
        "skipped_file_duplicate": 0,
        "invalid": 0,
        "total": items.len()
    });
    let mut existing_ids = existing
        .iter()
        .map(|item| item.id.clone())
        .collect::<HashSet<_>>();
    let mut known_secrets = existing
        .iter()
        .map(|item| item.secret.clone())
        .collect::<HashSet<_>>();
    let mut file_ids = HashSet::new();
    let mut credentials = Vec::new();
    let imported_at = time_utils::now_iso();

    for item in items {
        let Some(object) = item.as_object() else {
            increment_summary(&mut summary, "invalid");
            continue;
        };
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        let secret = object
            .get("secret")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_string();
        if id.is_empty() || secret.is_empty() {
            increment_summary(&mut summary, "invalid");
            continue;
        }
        if !file_ids.insert(id.clone()) {
            increment_summary(&mut summary, "skipped_file_duplicate");
            continue;
        }
        if existing_ids.contains(&id) {
            increment_summary(&mut summary, "skipped_existing_id");
            continue;
        }
        if known_secrets.contains(&secret) {
            increment_summary(&mut summary, "skipped_existing_secret");
            continue;
        }

        existing_ids.insert(id.clone());
        known_secrets.insert(secret.clone());
        increment_summary(&mut summary, "imported");
        credentials.push(TotpCredential {
            id,
            secret,
            comment: object
                .get("comment")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string(),
            created_at: normalize_totp_created_at(object.get("createdAt"), &imported_at),
            access_scopes: normalize_totp_access_scopes(
                object.get("access_scopes").cloned().unwrap_or(Value::Null),
            ),
            subdomain_access: normalize_totp_subdomain_access(
                object
                    .get("subdomain_access")
                    .cloned()
                    .unwrap_or(Value::Null),
            ),
        });
    }
    Ok((credentials, summary))
}

pub(super) fn build_totp_export_payload(
    credentials: &[TotpCredential],
    exported_at: &str,
) -> Value {
    let credentials = credentials
        .iter()
        .map(|credential| {
            json!({
                "id": credential.id.trim(),
                "secret": credential.secret.trim(),
                "comment": credential.comment.trim(),
                "createdAt": normalize_totp_created_at(
                    Some(&Value::String(credential.created_at.clone())),
                    exported_at,
                ),
                "access_scopes": normalize_totp_access_scopes(credential.access_scopes.clone()),
                "subdomain_access": normalize_totp_subdomain_access(
                    credential.subdomain_access.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    let mut payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "exported_at": exported_at,
        "credentials": credentials
    });
    if !APP_LOCAL_VERSION.trim().is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert(
            "app_version".to_string(),
            Value::String(APP_LOCAL_VERSION.to_string()),
        );
    }
    payload
}

pub(super) fn increment_summary(summary: &mut Value, key: &str) {
    let next = summary.get(key).and_then(Value::as_i64).unwrap_or(0) + 1;
    if let Some(object) = summary.as_object_mut() {
        object.insert(key.to_string(), Value::from(next));
    }
}

pub(super) fn normalize_totp_created_at(value: Option<&Value>, fallback: &str) -> String {
    let created_at = value.and_then(Value::as_str).unwrap_or("").trim();
    if !created_at.is_empty() && time_utils::parse_iso_ms(created_at).is_some() {
        created_at.to_string()
    } else {
        fallback.to_string()
    }
}
