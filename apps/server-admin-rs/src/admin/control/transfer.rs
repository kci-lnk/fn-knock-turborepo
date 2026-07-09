use std::collections::HashSet;

use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::{
    app_version::APP_LOCAL_VERSION,
    auth::password::is_supported_auth_password_credential,
    store::{
        AuthAccount, AuthPasswordCredential, TotpCredential, normalize_totp_access_scopes,
        normalize_totp_subdomain_access,
    },
    time_utils,
};

use super::{
    MAX_AUTH_ACCOUNT_IMPORT_COUNT, MAX_TOTP_IMPORT_COUNT, PASSWORD_TRANSFER_KIND,
    PASSWORD_TRANSFER_VERSION, TOTP_TRANSFER_KIND, TOTP_TRANSFER_VERSION, TotpImportRouteError,
    text::{totp_import_error, totp_import_error_with_max},
};

pub(super) use crate::http_utils::url_encode_component as percent_encode;

#[derive(Debug)]
pub(super) enum CredentialImportPlan {
    Totp(TotpCredentialImportPlan),
    Password(PasswordCredentialImportPlan),
}

#[derive(Debug)]
pub(super) struct TotpCredentialImportPlan {
    pub credentials: Vec<TotpCredential>,
    pub summary: Value,
}

#[derive(Debug)]
pub(super) struct PasswordCredentialImportPlan {
    pub accounts: Vec<AuthAccount>,
    pub password_credentials: Vec<AuthPasswordCredential>,
    pub totp_credentials: Vec<TotpCredential>,
    pub summary: Value,
}

pub(super) fn build_credential_import_plan(
    existing_totps: &[TotpCredential],
    existing_accounts: &[AuthAccount],
    existing_password_account_ids: &HashSet<String>,
    payload: &Value,
) -> Result<CredentialImportPlan, TotpImportRouteError> {
    if !payload.is_object() {
        return Err(totp_import_error(StatusCode::BAD_REQUEST, "payloadObject"));
    }
    match payload.get("kind").and_then(Value::as_str) {
        Some(TOTP_TRANSFER_KIND) => {
            let (credentials, summary) = build_totp_import_plan(existing_totps, payload)?;
            Ok(CredentialImportPlan::Totp(TotpCredentialImportPlan {
                credentials,
                summary,
            }))
        }
        Some(PASSWORD_TRANSFER_KIND) => build_password_import_plan(
            existing_totps,
            existing_accounts,
            existing_password_account_ids,
            payload,
        )
        .map(CredentialImportPlan::Password),
        _ => Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedKind",
        )),
    }
}

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
        .map(|credential| totp_credential_export_value(credential, exported_at))
        .collect::<Vec<_>>();
    let mut payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "login_mode": "totp",
        "exported_at": exported_at,
        "credentials": credentials
    });
    add_app_version(&mut payload);
    payload
}

pub(super) fn build_password_export_payload(
    accounts: &[AuthAccount],
    password_credentials: &[AuthPasswordCredential],
    totps: &[TotpCredential],
    exported_at: &str,
) -> Value {
    let account_ids = accounts
        .iter()
        .map(|account| account.id.trim().to_string())
        .collect::<HashSet<_>>();
    let source_totp_ids = accounts
        .iter()
        .map(|account| account.source_totp_id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect::<HashSet<_>>();
    let accounts = accounts
        .iter()
        .map(|account| auth_account_export_value(account, exported_at))
        .collect::<Vec<_>>();
    let password_credentials = password_credentials
        .iter()
        .filter(|credential| account_ids.contains(credential.account_id.trim()))
        .map(|credential| auth_password_export_value(credential, exported_at))
        .collect::<Vec<_>>();
    let totp_credentials = totps
        .iter()
        .filter(|credential| source_totp_ids.contains(credential.id.trim()))
        .map(|credential| totp_credential_export_value(credential, exported_at))
        .collect::<Vec<_>>();
    let mut payload = json!({
        "kind": PASSWORD_TRANSFER_KIND,
        "version": PASSWORD_TRANSFER_VERSION,
        "login_mode": "password",
        "exported_at": exported_at,
        "accounts": accounts,
        "password_credentials": password_credentials,
        "totp_credentials": totp_credentials
    });
    add_app_version(&mut payload);
    payload
}

fn build_password_import_plan(
    existing_totps: &[TotpCredential],
    existing_accounts: &[AuthAccount],
    existing_password_account_ids: &HashSet<String>,
    payload: &Value,
) -> Result<PasswordCredentialImportPlan, TotpImportRouteError> {
    if payload.get("version").and_then(Value::as_u64) != Some(PASSWORD_TRANSFER_VERSION) {
        return Err(totp_import_error(
            StatusCode::BAD_REQUEST,
            "unsupportedVersion",
        ));
    }
    let Some(account_items) = payload.get("accounts").and_then(Value::as_array) else {
        return Err(totp_import_error(StatusCode::BAD_REQUEST, "accountsArray"));
    };
    if account_items.len() > MAX_AUTH_ACCOUNT_IMPORT_COUNT {
        return Err(totp_import_error_with_max(
            StatusCode::BAD_REQUEST,
            "accountCountExceeded",
            MAX_AUTH_ACCOUNT_IMPORT_COUNT,
        ));
    }
    let password_items = optional_array_field(payload, "password_credentials", "passwordArray")?;
    if password_items.len() > MAX_AUTH_ACCOUNT_IMPORT_COUNT {
        return Err(totp_import_error_with_max(
            StatusCode::BAD_REQUEST,
            "passwordCountExceeded",
            MAX_AUTH_ACCOUNT_IMPORT_COUNT,
        ));
    }
    let totp_items = optional_array_field(payload, "totp_credentials", "credentialsArray")?;
    if totp_items.len() > MAX_TOTP_IMPORT_COUNT {
        return Err(totp_import_error_with_max(
            StatusCode::BAD_REQUEST,
            "countExceeded",
            MAX_TOTP_IMPORT_COUNT,
        ));
    }

    let totp_payload = json!({
        "kind": TOTP_TRANSFER_KIND,
        "version": TOTP_TRANSFER_VERSION,
        "credentials": totp_items
    });
    let (totp_credentials, totp_summary) = build_totp_import_plan(existing_totps, &totp_payload)?;
    let mut available_totp_ids = existing_totps
        .iter()
        .map(|credential| credential.id.clone())
        .collect::<HashSet<_>>();
    available_totp_ids.extend(
        totp_credentials
            .iter()
            .map(|credential| credential.id.clone()),
    );

    let mut summary = json!({
        "kind": "password",
        "login_mode": "password",
        "imported": 0,
        "skipped_existing_id": 0,
        "skipped_existing_username": 0,
        "skipped_file_duplicate": 0,
        "invalid": 0,
        "total": account_items.len(),
        "password_total": password_items.len(),
        "password_imported": 0,
        "password_skipped_existing": 0,
        "password_skipped_missing_account": 0,
        "password_skipped_file_duplicate": 0,
        "password_invalid": 0,
        "totp_total": summary_count(&totp_summary, "total"),
        "totp_imported": summary_count(&totp_summary, "imported"),
        "totp_skipped_existing_id": summary_count(&totp_summary, "skipped_existing_id"),
        "totp_skipped_existing_secret": summary_count(&totp_summary, "skipped_existing_secret"),
        "totp_skipped_file_duplicate": summary_count(&totp_summary, "skipped_file_duplicate"),
        "totp_invalid": summary_count(&totp_summary, "invalid")
    });
    let mut existing_ids = existing_accounts
        .iter()
        .map(|account| account.id.clone())
        .collect::<HashSet<_>>();
    let existing_usernames = existing_accounts
        .iter()
        .map(|account| normalize_username_for_compare(&account.username))
        .collect::<HashSet<_>>();
    let mut known_usernames = existing_usernames;
    let mut file_ids = HashSet::new();
    let mut accounts = Vec::new();

    for item in account_items {
        let Some(mut account) = parse_auth_account_import_item(item) else {
            increment_summary(&mut summary, "invalid");
            continue;
        };
        if !file_ids.insert(account.id.clone()) {
            increment_summary(&mut summary, "skipped_file_duplicate");
            continue;
        }
        let normalized_username = normalize_username_for_compare(&account.username);
        if normalized_username.is_empty() {
            increment_summary(&mut summary, "invalid");
            continue;
        }
        if existing_ids.contains(&account.id) {
            increment_summary(&mut summary, "skipped_existing_id");
            continue;
        }
        if known_usernames.contains(&normalized_username) {
            if existing_accounts.iter().any(|existing| {
                normalize_username_for_compare(&existing.username) == normalized_username
            }) {
                increment_summary(&mut summary, "skipped_existing_username");
            } else {
                increment_summary(&mut summary, "skipped_file_duplicate");
            }
            continue;
        }
        if !account.source_totp_id.is_empty()
            && !available_totp_ids.contains(account.source_totp_id.as_str())
        {
            account.source_totp_id.clear();
        }
        existing_ids.insert(account.id.clone());
        known_usernames.insert(normalized_username);
        increment_summary(&mut summary, "imported");
        accounts.push(account);
    }

    let available_account_ids = existing_accounts
        .iter()
        .map(|account| account.id.clone())
        .chain(accounts.iter().map(|account| account.id.clone()))
        .collect::<HashSet<_>>();
    let mut password_file_ids = HashSet::new();
    let mut password_credentials = Vec::new();
    let imported_password_account_ids = existing_password_account_ids.clone();
    for item in password_items {
        let Some(credential) = parse_auth_password_import_item(item) else {
            increment_summary(&mut summary, "password_invalid");
            continue;
        };
        if !password_file_ids.insert(credential.account_id.clone()) {
            increment_summary(&mut summary, "password_skipped_file_duplicate");
            continue;
        }
        if !available_account_ids.contains(&credential.account_id) {
            increment_summary(&mut summary, "password_skipped_missing_account");
            continue;
        }
        if imported_password_account_ids.contains(&credential.account_id) {
            increment_summary(&mut summary, "password_skipped_existing");
            continue;
        }
        increment_summary(&mut summary, "password_imported");
        password_credentials.push(credential);
    }

    Ok(PasswordCredentialImportPlan {
        accounts,
        password_credentials,
        totp_credentials,
        summary,
    })
}

fn add_app_version(payload: &mut Value) {
    if !APP_LOCAL_VERSION.trim().is_empty()
        && let Some(object) = payload.as_object_mut()
    {
        object.insert(
            "app_version".to_string(),
            Value::String(APP_LOCAL_VERSION.to_string()),
        );
    }
}

pub(super) fn increment_summary(summary: &mut Value, key: &str) {
    let next = summary.get(key).and_then(Value::as_i64).unwrap_or(0) + 1;
    if let Some(object) = summary.as_object_mut() {
        object.insert(key.to_string(), Value::from(next));
    }
}

fn summary_count(summary: &Value, key: &str) -> i64 {
    summary.get(key).and_then(Value::as_i64).unwrap_or(0)
}

pub(super) fn normalize_totp_created_at(value: Option<&Value>, fallback: &str) -> String {
    let created_at = value.and_then(Value::as_str).unwrap_or("").trim();
    if !created_at.is_empty() && time_utils::parse_iso_ms(created_at).is_some() {
        created_at.to_string()
    } else {
        fallback.to_string()
    }
}

fn optional_array_field<'a>(
    payload: &'a Value,
    key: &str,
    error_key: &'static str,
) -> Result<Vec<&'a Value>, TotpImportRouteError> {
    match payload.get(key) {
        Some(Value::Array(items)) => Ok(items.iter().collect()),
        Some(_) => Err(totp_import_error(StatusCode::BAD_REQUEST, error_key)),
        None => Ok(Vec::new()),
    }
}

fn totp_credential_export_value(credential: &TotpCredential, exported_at: &str) -> Value {
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
}

fn auth_account_export_value(account: &AuthAccount, exported_at: &str) -> Value {
    let created_at = normalize_time_field(&account.created_at, exported_at);
    let updated_at = normalize_time_field(&account.updated_at, &created_at);
    json!({
        "id": account.id.trim(),
        "username": account.username.trim(),
        "displayName": account.display_name.trim(),
        "sourceTotpId": account.source_totp_id.trim(),
        "createdAt": created_at,
        "updatedAt": updated_at,
        "access_scopes": normalize_totp_access_scopes(account.access_scopes.clone()),
        "subdomain_access": normalize_totp_subdomain_access(account.subdomain_access.clone())
    })
}

fn auth_password_export_value(credential: &AuthPasswordCredential, exported_at: &str) -> Value {
    let created_at = normalize_time_field(&credential.created_at, exported_at);
    let updated_at = normalize_time_field(&credential.updated_at, &created_at);
    json!({
        "accountId": credential.account_id.trim(),
        "algorithm": credential.algorithm.trim(),
        "salt": credential.salt.trim(),
        "hash": credential.hash.trim(),
        "n": credential.n,
        "r": credential.r,
        "p": credential.p,
        "key_length": credential.key_length,
        "created_at": created_at,
        "updated_at": updated_at
    })
}

fn parse_auth_account_import_item(value: &Value) -> Option<AuthAccount> {
    let object = value.as_object()?;
    let id = json_string(object.get("id")?).trim().to_string();
    let username = normalize_import_username(&json_string(object.get("username")?))?;
    if id.is_empty() {
        return None;
    }
    let display_name = json_string_opt(
        object
            .get("displayName")
            .or_else(|| object.get("display_name")),
    )
    .trim()
    .to_string();
    let source_totp_id = json_string_opt(
        object
            .get("sourceTotpId")
            .or_else(|| object.get("source_totp_id")),
    )
    .trim()
    .to_string();
    let fallback_time = time_utils::now_iso();
    let created_at = normalize_import_time(
        object.get("createdAt").or_else(|| object.get("created_at")),
        &fallback_time,
    );
    let updated_at = normalize_import_time(
        object.get("updatedAt").or_else(|| object.get("updated_at")),
        &created_at,
    );
    Some(AuthAccount {
        id,
        username: username.clone(),
        display_name: if display_name.is_empty() {
            username
        } else {
            display_name
        },
        source_totp_id,
        created_at,
        updated_at,
        access_scopes: normalize_totp_access_scopes(
            object.get("access_scopes").cloned().unwrap_or(Value::Null),
        ),
        subdomain_access: normalize_totp_subdomain_access(
            object
                .get("subdomain_access")
                .cloned()
                .unwrap_or(Value::Null),
        ),
    })
}

fn parse_auth_password_import_item(value: &Value) -> Option<AuthPasswordCredential> {
    let object = value.as_object()?;
    let account_id = json_string_opt(object.get("accountId").or_else(|| object.get("account_id")))
        .trim()
        .to_string();
    let algorithm = json_string(object.get("algorithm")?).trim().to_string();
    let salt = json_string(object.get("salt")?).trim().to_string();
    let hash = json_string(object.get("hash")?).trim().to_string();
    if account_id.is_empty()
        || algorithm != "scrypt"
        || salt.is_empty()
        || hash.is_empty()
        || !is_hex_string(&salt)
        || !is_hex_string(&hash)
    {
        return None;
    }
    let n = json_u32(object.get("n")).filter(|value| *value >= 2)?;
    let r = json_u32(object.get("r")).filter(|value| *value >= 1)?;
    let p = json_u32(object.get("p")).filter(|value| *value >= 1)?;
    let key_length = json_usize(object.get("key_length")).filter(|value| *value >= 1)?;
    let fallback_time = time_utils::now_iso();
    let created_at = normalize_import_time(
        object.get("created_at").or_else(|| object.get("createdAt")),
        &fallback_time,
    );
    let updated_at = normalize_import_time(
        object.get("updated_at").or_else(|| object.get("updatedAt")),
        &created_at,
    );
    let credential = AuthPasswordCredential {
        account_id,
        algorithm,
        salt,
        hash,
        n,
        r,
        p,
        key_length,
        created_at,
        updated_at,
    };
    is_supported_auth_password_credential(&credential).then_some(credential)
}

fn normalize_import_username(value: &str) -> Option<String> {
    let username = value.trim().to_lowercase();
    if username.len() < 3 || username.len() > 64 {
        return None;
    }
    if username
        .chars()
        .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return None;
    }
    username
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
        .then_some(username)
}

fn normalize_username_for_compare(value: &str) -> String {
    value.trim().to_lowercase()
}

fn normalize_time_field(value: &str, fallback: &str) -> String {
    if !value.trim().is_empty() && time_utils::parse_iso_ms(value.trim()).is_some() {
        value.trim().to_string()
    } else {
        fallback.to_string()
    }
}

fn normalize_import_time(value: Option<&Value>, fallback: &str) -> String {
    normalize_time_field(&value.map(json_string).unwrap_or_default(), fallback)
}

fn json_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => items.iter().map(json_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

fn json_string_opt(value: Option<&Value>) -> String {
    value.map(json_string).unwrap_or_default()
}

fn json_u32(value: Option<&Value>) -> Option<u32> {
    match value? {
        Value::Number(number) => number.as_u64().and_then(|value| u32::try_from(value).ok()),
        Value::String(value) => value.trim().parse::<u32>().ok(),
        _ => None,
    }
}

fn json_usize(value: Option<&Value>) -> Option<usize> {
    match value? {
        Value::Number(number) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok()),
        Value::String(value) => value.trim().parse::<usize>().ok(),
        _ => None,
    }
}

fn is_hex_string(value: &str) -> bool {
    !value.is_empty() && value.len() % 2 == 0 && value.chars().all(|ch| ch.is_ascii_hexdigit())
}
