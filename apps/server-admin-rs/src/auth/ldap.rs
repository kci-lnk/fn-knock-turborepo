mod admin;
mod client;
mod provider;
mod runtime;
mod storage;

pub(crate) use admin::{ldap_admin_openapi_routes, ldap_admin_routes};
pub(crate) use runtime::{ldap_public_providers, ldap_runtime_routes, login};
pub(crate) use storage::ldap_delete_bindings_by_totp;

const PROVIDERS_INDEX_KEY: &str = "fn_knock:ldap:providers:index";
const PROVIDERS_DATA_KEY_PREFIX: &str = "fn_knock:ldap:providers:data:";
const BINDINGS_INDEX_KEY: &str = "fn_knock:ldap:bindings:index";
const BINDINGS_DATA_KEY_PREFIX: &str = "fn_knock:ldap:bindings:data:";
const BINDINGS_SUBJECT_KEY_PREFIX: &str = "fn_knock:ldap:bindings:subject:";
const INVITE_KEY_PREFIX: &str = "fn_knock:ldap:invite:";
const DEFAULT_INVITE_TTL_SECONDS: usize = 30 * 60;

fn provider_key(id: &str) -> String {
    format!("{PROVIDERS_DATA_KEY_PREFIX}{id}")
}

fn binding_key(id: &str) -> String {
    format!("{BINDINGS_DATA_KEY_PREFIX}{id}")
}

fn subject_binding_key(subject_key: &str) -> String {
    format!("{BINDINGS_SUBJECT_KEY_PREFIX}{subject_key}")
}

fn invite_key(token_hash: &str) -> String {
    format!("{INVITE_KEY_PREFIX}{token_hash}")
}

#[cfg(test)]
mod tests;
