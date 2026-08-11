use serde_json::{Value, json};

use crate::{state::AppState, time_utils};

use super::{
    BINDINGS_INDEX_KEY, DEFAULT_INVITE_TTL_SECONDS, PROVIDERS_INDEX_KEY, binding_key, invite_key,
    provider::provider_ready, provider_key, subject_binding_key,
};

async fn verify_identity_shadow(state: &AppState) -> crate::storage::StorageResult<()> {
    state
        .storage
        .store
        .verify_identity_runtime_shadow("ldap")
        .await
}

async fn get_provider_unverified(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.get_json_value(&provider_key(id)).await
}

pub(super) async fn list_providers(state: &AppState) -> crate::storage::StorageResult<Vec<Value>> {
    verify_identity_shadow(state).await?;
    let ids = state
        .storage
        .store
        .zrevrange_strings(PROVIDERS_INDEX_KEY)
        .await?;
    let mut providers = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match get_provider_unverified(state, &id).await? {
            Some(provider) => providers.push(provider),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .storage
            .store
            .zrem_string_member(PROVIDERS_INDEX_KEY, &id)
            .await?;
    }
    Ok(providers)
}

pub(super) async fn public_providers(
    state: &AppState,
) -> crate::storage::StorageResult<Vec<Value>> {
    Ok(list_providers(state)
        .await?
        .into_iter()
        .filter(|provider| provider.get("enabled").and_then(Value::as_bool) == Some(true))
        .filter(provider_ready)
        .map(|provider| {
            json!({
                "id": provider.get("id").cloned().unwrap_or_default(),
                "type": provider.get("type").cloned().unwrap_or_default(),
                "protocol": "ldap",
                "name": provider.get("name").cloned().unwrap_or_default(),
            })
        })
        .collect())
}

pub(super) async fn get_provider(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    verify_identity_shadow(state).await?;
    get_provider_unverified(state, id).await
}

pub(super) async fn save_provider(
    state: &AppState,
    provider: &Value,
) -> crate::storage::StorageResult<()> {
    let id = provider.get("id").and_then(Value::as_str).unwrap_or("");
    state
        .storage
        .store
        .set_json_value(&provider_key(id), provider)
        .await?;
    state
        .storage
        .store
        .zadd_string_member(
            PROVIDERS_INDEX_KEY,
            id,
            provider
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        )
        .await
}

pub(super) async fn delete_provider(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<()> {
    let bindings = list_bindings(state).await?;
    state.storage.store.delete_keys(&[provider_key(id)]).await?;
    state
        .storage
        .store
        .zrem_string_member(PROVIDERS_INDEX_KEY, id)
        .await?;
    for binding in bindings
        .iter()
        .filter(|binding| binding.get("provider_id").and_then(Value::as_str) == Some(id))
    {
        if let Some(binding_id) = binding.get("id").and_then(Value::as_str) {
            let _ = delete_binding(state, binding_id).await?;
        }
    }
    Ok(())
}

pub(super) async fn list_bindings(state: &AppState) -> crate::storage::StorageResult<Vec<Value>> {
    verify_identity_shadow(state).await?;
    let ids = state
        .storage
        .store
        .zrevrange_strings(BINDINGS_INDEX_KEY)
        .await?;
    let mut bindings = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match state
            .storage
            .store
            .get_json_value(&binding_key(&id))
            .await?
        {
            Some(binding) => bindings.push(binding),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .storage
            .store
            .zrem_string_member(BINDINGS_INDEX_KEY, &id)
            .await?;
    }
    Ok(bindings)
}

pub(super) async fn get_binding_by_subject(
    state: &AppState,
    subject_key: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    verify_identity_shadow(state).await?;
    let subject_index_key = subject_binding_key(subject_key);
    let Some(id) = state
        .storage
        .store
        .get_string_value(&subject_index_key)
        .await?
    else {
        return Ok(None);
    };
    let binding = state
        .storage
        .store
        .get_json_value(&binding_key(&id))
        .await?;
    if binding.is_none() {
        state
            .storage
            .store
            .delete_key_if_value(&subject_index_key, &id)
            .await?;
    }
    Ok(binding)
}

pub(super) async fn update_binding_if_owned(
    state: &AppState,
    binding: &Value,
) -> crate::storage::StorageResult<bool> {
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .storage
        .store
        .update_binding_if_owned(crate::storage::redis_store::OwnedBindingUpdate {
            subject_key: &subject_binding_key(subject_key),
            binding_key: &binding_key(id),
            bindings_index_key: BINDINGS_INDEX_KEY,
            binding_id: id,
            binding,
            score: binding
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        })
        .await
}

pub(super) async fn claim_binding_and_consume_invite(
    state: &AppState,
    token_hash: &str,
    binding: &Value,
) -> crate::storage::StorageResult<Option<Value>> {
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    let provider_id = binding
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let totp_id = binding.get("totp_id").and_then(Value::as_str).unwrap_or("");
    let claimed = state
        .storage
        .store
        .claim_ldap_binding_and_consume_invite(crate::storage::redis_store::LdapBindingClaim {
            invite_key: &invite_key(token_hash),
            subject_key: &subject_binding_key(subject_key),
            binding_key: &binding_key(id),
            bindings_index_key: BINDINGS_INDEX_KEY,
            binding_id: id,
            binding,
            provider_id,
            totp_id,
            score: binding
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        })
        .await?;
    Ok(claimed.then(|| binding.clone()))
}

pub(super) async fn delete_binding(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<bool> {
    let Some(binding) = state.storage.store.get_json_value(&binding_key(id)).await? else {
        return Ok(false);
    };
    let binding_data_key = binding_key(id);
    let subject_index_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(subject_binding_key)
        .unwrap_or_else(|| binding_data_key.clone());
    state
        .storage
        .store
        .delete_binding_if_owned(crate::storage::redis_store::OwnedBindingDelete {
            subject_key: &subject_index_key,
            binding_key: &binding_data_key,
            bindings_index_key: BINDINGS_INDEX_KEY,
            binding_id: id,
        })
        .await
}

pub(crate) async fn ldap_delete_bindings_by_totp(
    state: &AppState,
    totp_id: &str,
) -> crate::storage::StorageResult<usize> {
    let mut deleted = 0;
    for binding in list_bindings(state).await? {
        if binding.get("totp_id").and_then(Value::as_str) == Some(totp_id)
            && let Some(id) = binding.get("id").and_then(Value::as_str)
            && delete_binding(state, id).await?
        {
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub(super) async fn save_invite(
    state: &AppState,
    token_hash: &str,
    invite: &Value,
) -> crate::storage::StorageResult<()> {
    state
        .storage
        .store
        .set_json_value_ex(&invite_key(token_hash), invite, DEFAULT_INVITE_TTL_SECONDS)
        .await
}

pub(super) async fn inspect_invite(
    state: &AppState,
    token_hash: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    verify_identity_shadow(state).await?;
    state
        .storage
        .store
        .get_json_value(&invite_key(token_hash))
        .await
}
