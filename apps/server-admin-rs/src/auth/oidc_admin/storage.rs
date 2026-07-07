use serde_json::{Value, json};

use crate::{state::AppState, time_utils};

use super::{
    BINDINGS_INDEX_KEY, PROVIDERS_INDEX_KEY,
    provider::missing_required_provider_fields,
    tokens::{
        binding_key, invite_key, login_error_key, provider_key, sha256_hex, state_key,
        subject_binding_key,
    },
};

pub(super) async fn oidc_list_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(PROVIDERS_INDEX_KEY).await?;
    let mut providers = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match oidc_get_provider(state, &id).await? {
            Some(provider) => providers.push(provider),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .redis
            .zrem_string_member(PROVIDERS_INDEX_KEY, &id)
            .await?;
    }
    Ok(providers)
}

pub(crate) async fn oidc_public_providers(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    Ok(oidc_list_providers(state)
        .await?
        .into_iter()
        .filter(|provider| provider.get("enabled").and_then(Value::as_bool) == Some(true))
        .filter(|provider| missing_required_provider_fields(provider).is_empty())
        .map(|provider| {
            json!({
                "id": provider.get("id").cloned().unwrap_or(Value::String(String::new())),
                "type": provider.get("type").cloned().unwrap_or(Value::String(String::new())),
                "name": provider.get("name").cloned().unwrap_or(Value::String(String::new())),
                "protocol": provider.get("protocol").cloned().unwrap_or(Value::String(String::new())),
            })
        })
        .collect())
}

pub(crate) async fn oidc_inspect_invite(
    state: &AppState,
    token: &str,
) -> redis::RedisResult<Option<Value>> {
    let normalized_token = token.trim();
    if normalized_token.is_empty() {
        return Ok(None);
    }
    let token_hash = sha256_hex(normalized_token);
    let Some(invite) = state.redis.get_json_value(&invite_key(&token_hash)).await? else {
        return Ok(None);
    };
    let expires_at = invite
        .get("expires_at")
        .and_then(Value::as_str)
        .unwrap_or("");
    if invite.get("used_at").is_some()
        || time_utils::parse_iso_ms(expires_at).unwrap_or(0) <= time_utils::now_ms()
    {
        return Ok(None);
    }
    let totp_id = invite.get("totp_id").and_then(Value::as_str).unwrap_or("");
    let Some(totp) = state
        .redis
        .get_totps()
        .await?
        .into_iter()
        .find(|credential| credential.id == totp_id)
    else {
        return Ok(None);
    };
    let invite_provider_id = invite.get("provider_id").and_then(Value::as_str);
    let providers = oidc_public_providers(state)
        .await?
        .into_iter()
        .filter(|provider| {
            invite_provider_id
                .map(|id| provider.get("id").and_then(Value::as_str) == Some(id))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    Ok(Some(json!({
        "totp": { "id": totp.id, "comment": totp.comment },
        "provider_id": invite_provider_id,
        "expires_at": expires_at,
        "note": invite.get("note").cloned().unwrap_or(Value::Null),
        "providers": providers,
    })))
}

pub(crate) async fn oidc_get_provider(
    state: &AppState,
    id: &str,
) -> redis::RedisResult<Option<Value>> {
    state.redis.get_json_value(&provider_key(id)).await
}

pub(super) async fn oidc_save_provider(
    state: &AppState,
    provider: &Value,
) -> redis::RedisResult<()> {
    let id = provider.get("id").and_then(Value::as_str).unwrap_or("");
    state
        .redis
        .set_json_value(&provider_key(id), provider)
        .await?;
    state
        .redis
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

pub(super) async fn oidc_delete_provider(state: &AppState, id: &str) -> redis::RedisResult<()> {
    let bindings = oidc_list_bindings(state).await?;
    state.redis.delete_keys(&[provider_key(id)]).await?;
    state
        .redis
        .zrem_string_member(PROVIDERS_INDEX_KEY, id)
        .await?;
    for binding in bindings
        .iter()
        .filter(|binding| binding.get("provider_id").and_then(Value::as_str) == Some(id))
    {
        if let Some(binding_id) = binding.get("id").and_then(Value::as_str) {
            let _ = oidc_delete_binding(state, binding_id).await?;
        }
    }
    Ok(())
}

pub(crate) async fn oidc_list_bindings(state: &AppState) -> redis::RedisResult<Vec<Value>> {
    let ids = state.redis.zrevrange_strings(BINDINGS_INDEX_KEY).await?;
    let mut bindings = Vec::new();
    let mut stale = Vec::new();
    for id in ids {
        match state.redis.get_json_value(&binding_key(&id)).await? {
            Some(binding) => bindings.push(binding),
            None => stale.push(id),
        }
    }
    for id in stale {
        state
            .redis
            .zrem_string_member(BINDINGS_INDEX_KEY, &id)
            .await?;
    }
    Ok(bindings)
}

pub(crate) async fn oidc_get_binding_by_subject(
    state: &AppState,
    subject_key: &str,
) -> redis::RedisResult<Option<Value>> {
    let Some(binding_id) = state
        .redis
        .get_string_value(&subject_binding_key(subject_key))
        .await?
    else {
        return Ok(None);
    };
    state.redis.get_json_value(&binding_key(&binding_id)).await
}

pub(crate) async fn oidc_save_binding(state: &AppState, binding: &Value) -> redis::RedisResult<()> {
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value(&binding_key(id), binding)
        .await?;
    if !subject_key.is_empty() {
        state
            .redis
            .set_string_value(&subject_binding_key(subject_key), id)
            .await?;
    }
    state
        .redis
        .zadd_string_member(
            BINDINGS_INDEX_KEY,
            id,
            binding
                .get("updated_at")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms)
                .unwrap_or_else(time_utils::now_ms),
        )
        .await
}

pub(crate) async fn oidc_save_binding_if_subject_available(
    state: &AppState,
    binding: &Value,
) -> redis::RedisResult<bool> {
    let subject_key = binding
        .get("subject_key")
        .and_then(Value::as_str)
        .unwrap_or("");
    if subject_key.is_empty() {
        return Ok(false);
    }
    let id = binding.get("id").and_then(Value::as_str).unwrap_or("");
    if let Some(existing_id) = state
        .redis
        .get_string_value(&subject_binding_key(subject_key))
        .await?
        && existing_id != id
    {
        return Ok(false);
    }
    oidc_save_binding(state, binding).await?;
    Ok(true)
}

pub(super) async fn oidc_delete_binding(state: &AppState, id: &str) -> redis::RedisResult<bool> {
    let Some(binding) = state.redis.get_json_value(&binding_key(id)).await? else {
        return Ok(false);
    };
    let mut keys = vec![binding_key(id)];
    if let Some(subject_key) = binding.get("subject_key").and_then(Value::as_str) {
        keys.push(subject_binding_key(subject_key));
    }
    state.redis.delete_keys(&keys).await?;
    state
        .redis
        .zrem_string_member(BINDINGS_INDEX_KEY, id)
        .await?;
    Ok(true)
}

pub(crate) async fn oidc_delete_bindings_by_totp(
    state: &AppState,
    totp_id: &str,
) -> redis::RedisResult<usize> {
    let bindings = oidc_list_bindings(state).await?;
    let mut deleted = 0usize;
    for binding in bindings {
        if binding.get("totp_id").and_then(Value::as_str) != Some(totp_id) {
            continue;
        }
        let Some(binding_id) = binding.get("id").and_then(Value::as_str) else {
            continue;
        };
        if oidc_delete_binding(state, binding_id).await? {
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub(crate) async fn oidc_consume_invite(
    state: &AppState,
    token_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .consume_json_value(&invite_key(token_hash))
        .await
}

pub(crate) async fn oidc_save_state(
    state: &AppState,
    auth_state: &Value,
    ttl_seconds: usize,
) -> redis::RedisResult<()> {
    let state_hash = auth_state
        .get("state_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value_ex(&state_key(state_hash), auth_state, ttl_seconds)
        .await
}

pub(crate) async fn oidc_consume_state(
    state: &AppState,
    state_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state.redis.consume_json_value(&state_key(state_hash)).await
}

pub(crate) async fn oidc_save_login_error_notice(
    state: &AppState,
    notice: &Value,
    ttl_seconds: usize,
) -> redis::RedisResult<()> {
    let token_hash = notice
        .get("token_hash")
        .and_then(Value::as_str)
        .unwrap_or("");
    state
        .redis
        .set_json_value_ex(&login_error_key(token_hash), notice, ttl_seconds)
        .await
}

pub(crate) async fn oidc_consume_login_error_notice(
    state: &AppState,
    token_hash: &str,
) -> redis::RedisResult<Option<Value>> {
    state
        .redis
        .consume_json_value(&login_error_key(token_hash))
        .await
}
