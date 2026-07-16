use super::*;

pub(super) async fn create_provider_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let provider_type = trimmed_string(body.get("type")).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(&provider_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let mut raw_config = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_config);
    let connection_config = normalize_schema_config(&raw_config, &definition.connection_schema)?;
    validate_required_fields(&connection_config, &definition.connection_schema)?;
    validate_provider_connection_config(&definition, &connection_config)?;

    let existing = load_providers(state).await?;
    let names = existing
        .iter()
        .filter_map(|provider| provider.get("name").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();
    let requested_name = trimmed_string(body.get("name"));
    let translator = Translator::from_state(state).await;
    let default_name_base = provider_definition_label(&definition, &translator);
    let name =
        requested_name.unwrap_or_else(|| build_next_sequential_name(&default_name_base, &names));
    let now = time_utils::now_iso();
    let provider = json!({
        "id": create_id("ntfprov"),
        "name": name,
        "type": definition.provider_type,
        "enabled": bool_field(&body, "enabled", true),
        "connection_config": Value::Object(connection_config),
        "created_at": now,
        "updated_at": now,
        "last_test_status": "idle",
        "last_error": Value::Null
    });
    save_provider_raw(state, &provider).await?;
    mask_provider(&provider).map_err(NotifyError::BadRequest)
}

pub(super) async fn update_provider_value(
    state: &AppState,
    id: &str,
    body: Value,
) -> NotifyResult<Value> {
    let current = load_provider(state, id)
        .await?
        .ok_or_bad(notification_service_default_text("providerNotFound", &[]))?;
    let provider_type = current.get("type").and_then(Value::as_str).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(provider_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;

    let mut raw_patch = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_patch);
    drop_masked_sensitive_patch_values(&definition, &mut raw_patch);
    let patch = normalize_schema_patch(&raw_patch, &definition.connection_schema)?;
    let mut merged = current
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in patch {
        merged.insert(key, value);
    }
    apply_schema_defaults(&mut merged, &definition.connection_schema);
    validate_required_fields(&merged, &definition.connection_schema)?;
    validate_provider_connection_config(&definition, &merged)?;

    let mut updated = current
        .as_object()
        .cloned()
        .ok_or_bad(notification_service_default_text(
            "invalidProviderRecord",
            &[],
        ))?;
    if let Some(name) = trimmed_string(body.get("name")) {
        updated.insert("name".to_string(), Value::String(name));
    }
    if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
        updated.insert("enabled".to_string(), Value::Bool(enabled));
    }
    updated.insert("connection_config".to_string(), Value::Object(merged));
    updated.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let updated = Value::Object(updated);
    save_provider_raw(state, &updated).await?;
    mask_provider(&updated).map_err(NotifyError::BadRequest)
}

pub(super) async fn draft_provider_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let requested_id = trimmed_string(body.get("id"));
    let requested_type = trimmed_string(body.get("type")).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let definition = provider_definition(&requested_type).ok_or_bad(
        notification_service_default_text("unsupportedProviderType", &[]),
    )?;
    let existing = if let Some(id) = requested_id.as_deref() {
        Some(
            load_provider(state, id)
                .await?
                .ok_or_bad(notification_service_default_text("providerNotFound", &[]))?,
        )
    } else {
        None
    };
    if let Some(existing) = existing.as_ref()
        && existing.get("type").and_then(Value::as_str) != Some(definition.provider_type)
    {
        return Err(NotifyError::BadRequest(notification_service_default_text(
            "providerTypeMismatch",
            &[],
        )));
    }

    let mut raw_patch = object_field(&body, "connection_config");
    normalize_provider_connection_aliases(definition.provider_type, &mut raw_patch);
    drop_masked_sensitive_patch_values(&definition, &mut raw_patch);
    let patch = normalize_schema_patch(&raw_patch, &definition.connection_schema)?;
    let mut connection_config = existing
        .as_ref()
        .and_then(|provider| provider.get("connection_config"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (key, value) in patch {
        connection_config.insert(key, value);
    }
    apply_schema_defaults(&mut connection_config, &definition.connection_schema);
    validate_required_fields(&connection_config, &definition.connection_schema)?;
    validate_provider_connection_config(&definition, &connection_config)?;

    let now = time_utils::now_iso();
    Ok(json!({
        "id": existing.as_ref().and_then(|provider| provider.get("id")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| create_id("ntfprovtest")),
        "name": trimmed_string(body.get("name"))
            .or_else(|| existing.as_ref().and_then(|provider| provider.get("name")).and_then(Value::as_str).map(str::to_string))
            .unwrap_or_else(|| notification_service_text(&translator, "providerTestName", &[("provider", definition.label.to_string())])),
        "type": definition.provider_type,
        "enabled": body.get("enabled").and_then(Value::as_bool)
            .or_else(|| existing.as_ref().and_then(|provider| provider.get("enabled")).and_then(Value::as_bool))
            .unwrap_or(true),
        "connection_config": Value::Object(connection_config),
        "created_at": existing.as_ref().and_then(|provider| provider.get("created_at")).and_then(Value::as_str).unwrap_or(&now),
        "updated_at": now,
        "last_test_at": existing.as_ref().and_then(|provider| provider.get("last_test_at")).cloned().unwrap_or(Value::Null),
        "last_test_status": existing.as_ref().and_then(|provider| provider.get("last_test_status")).cloned().unwrap_or(Value::Null),
        "last_error": existing.as_ref().and_then(|provider| provider.get("last_error")).cloned().unwrap_or(Value::Null)
    }))
}

pub(super) async fn delete_provider_value(state: &AppState, id: &str) -> NotifyResult<()> {
    let rules = load_rules(state).await?;
    let referenced_by = rules.iter().find_map(|rule| {
        let targets = rule.get("targets").and_then(Value::as_array)?;
        let referenced = targets
            .iter()
            .any(|target| target.get("provider_id").and_then(Value::as_str) == Some(id));
        if referenced {
            rule.get("name").and_then(Value::as_str).map(str::to_string)
        } else {
            None
        }
    });
    if let Some(rule_name) = referenced_by {
        return Err(NotifyError::BadRequest(notification_service_default_text(
            "providerReferencedByRule",
            &[("rule", rule_name)],
        )));
    }
    let key = provider_key(id);
    state.store.delete_keys(&[key]).await?;
    state
        .store
        .zrem_string_member(PROVIDERS_INDEX_KEY, id)
        .await?;
    Ok(())
}
