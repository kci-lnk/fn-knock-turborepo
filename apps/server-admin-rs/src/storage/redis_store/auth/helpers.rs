use super::*;

pub(super) fn normalize_auth_login_mode(value: Option<&str>) -> crate::auth::mode::AuthLoginMode {
    crate::auth::mode::AuthLoginMode::from_storage(value)
}

pub(super) fn auth_password_credential_key(account_id: &str) -> String {
    format!("fn_knock:auth:password_credentials:v1:{account_id}")
}

pub(crate) fn normalize_auth_username(value: &str) -> String {
    value.trim().to_lowercase()
}

pub(super) fn normalize_auth_accounts(accounts: &[AuthAccount]) -> Vec<AuthAccount> {
    accounts
        .iter()
        .cloned()
        .map(normalize_auth_account)
        .collect()
}

pub(super) fn normalize_auth_accounts_value(value: &Value) -> Vec<AuthAccount> {
    match value {
        Value::Array(items) => items
            .iter()
            .filter_map(normalize_auth_account_value)
            .collect(),
        _ => Vec::new(),
    }
}

pub(super) fn normalize_auth_accounts_for_comparison(
    value: &Value,
    expected: &[AuthAccount],
) -> Vec<AuthAccount> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|value| serde_json::from_value::<AuthAccount>(value.clone()).ok())
        .filter(|account| !account.id.trim().is_empty() && !account.username.trim().is_empty())
        .map(|mut account| {
            if let Some(expected) = expected.iter().find(|item| item.id == account.id.trim()) {
                // Legacy records can omit these timestamps. Reads synthesize
                // them, so reusing that read's defaults avoids a false conflict
                // caused only by the clock advancing before the CAS. Persisted
                // timestamps and every other account field still compare exactly.
                if account.created_at.trim().is_empty() {
                    account.created_at = expected.created_at.clone();
                }
                if account.updated_at.trim().is_empty() {
                    account.updated_at = expected.updated_at.clone();
                }
            }
            normalize_auth_account(account)
        })
        .collect()
}

pub(super) fn normalize_auth_account_value(value: &Value) -> Option<AuthAccount> {
    serde_json::from_value::<AuthAccount>(value.clone())
        .ok()
        .map(normalize_auth_account)
        .filter(|account| !account.id.trim().is_empty() && !account.username.trim().is_empty())
}

pub(super) fn normalize_auth_account(mut account: AuthAccount) -> AuthAccount {
    account.id = account.id.trim().to_string();
    account.username = account.username.trim().to_string();
    account.display_name = account.display_name.trim().to_string();
    account.source_totp_id = account.source_totp_id.trim().to_string();
    if account.created_at.trim().is_empty() {
        account.created_at = now_iso();
    }
    if account.updated_at.trim().is_empty() {
        account.updated_at = account.created_at.clone();
    }
    account.access_scopes = normalize_totp_access_scopes(account.access_scopes);
    account.subdomain_access = normalize_totp_subdomain_access(account.subdomain_access);
    account
}
