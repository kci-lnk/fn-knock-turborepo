use super::{
    BINDINGS_DATA_KEY_PREFIX, BINDINGS_SUBJECT_KEY_PREFIX, INVITE_KEY_PREFIX,
    LOGIN_ERROR_KEY_PREFIX, PROVIDERS_DATA_KEY_PREFIX, STATE_KEY_PREFIX,
};

pub(super) use crate::auth::oidc_tokens::{create_oidc_id, create_public_token};

pub(super) use crate::crypto_utils::sha256_hex_str as sha256_hex;

pub(super) fn provider_key(id: &str) -> String {
    format!("{PROVIDERS_DATA_KEY_PREFIX}{id}")
}

pub(super) fn binding_key(id: &str) -> String {
    format!("{BINDINGS_DATA_KEY_PREFIX}{id}")
}

pub(super) fn subject_binding_key(subject_key: &str) -> String {
    format!("{BINDINGS_SUBJECT_KEY_PREFIX}{subject_key}")
}

pub(super) fn invite_key(token_hash: &str) -> String {
    format!("{INVITE_KEY_PREFIX}{token_hash}")
}

pub(super) fn state_key(state_hash: &str) -> String {
    format!("{STATE_KEY_PREFIX}{state_hash}")
}

pub(super) fn login_error_key(token_hash: &str) -> String {
    format!("{LOGIN_ERROR_KEY_PREFIX}{token_hash}")
}
