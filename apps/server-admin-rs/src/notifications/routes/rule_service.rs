use super::*;

pub(super) async fn create_rule_value(state: &AppState, body: Value) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let event_type = trimmed_string(body.get("event_type")).ok_or_bad(
        notification_service_text(&translator, "unsupportedEventType", &[]),
    )?;
    if !SYSTEM_EVENT_TYPES.contains(&event_type.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "unsupportedEventType",
            &[],
        )));
    }
    let group_by = trimmed_string(body.get("group_by")).ok_or_bad(notification_service_text(
        &translator,
        "invalidGroupBy",
        &[],
    ))?;
    if !GROUP_BY_VALUES.contains(&group_by.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidGroupBy",
            &[],
        )));
    }
    let message_template_mode =
        trimmed_string(body.get("message_template_mode")).unwrap_or_else(|| "default".to_string());
    if !MESSAGE_TEMPLATE_MODES.contains(&message_template_mode.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidMessageTemplateMode",
            &[],
        )));
    }
    let event_level_filter = unique_string_array(body.get("event_level_filter"));
    if !event_level_filter
        .iter()
        .all(|value| SYSTEM_EVENT_LEVELS.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventLevelFilter",
            &[],
        )));
    }
    let event_source_filter = unique_string_array(body.get("event_source_filter"));
    if !event_source_filter
        .iter()
        .all(|value| SYSTEM_EVENT_SOURCES.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventSourceFilter",
            &[],
        )));
    }

    let targets = normalize_rule_targets(state, body.get("targets"), &[], &translator).await?;
    if targets.is_empty() {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "targetRequired",
            &[],
        )));
    }
    let existing_rules = load_rules(state).await?;
    if existing_rules
        .iter()
        .any(|rule| rule.get("event_type").and_then(Value::as_str) == Some(&event_type))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "duplicateEventRule",
            &[],
        )));
    }

    let now = time_utils::now_iso();
    let mut rule = Map::new();
    rule.insert("id".to_string(), Value::String(create_id("ntfrule")));
    rule.insert(
        "name".to_string(),
        Value::String(build_notification_rule_name(&event_type, &translator)),
    );
    rule.insert(
        "enabled".to_string(),
        Value::Bool(bool_field(&body, "enabled", true)),
    );
    rule.insert("event_type".to_string(), Value::String(event_type));
    if !event_level_filter.is_empty() {
        rule.insert("event_level_filter".to_string(), json!(event_level_filter));
    }
    if !event_source_filter.is_empty() {
        rule.insert(
            "event_source_filter".to_string(),
            json!(event_source_filter),
        );
    }
    rule.insert(
        "window_seconds".to_string(),
        json!(number_field(&body, "window_seconds", 60, 1, 86400)),
    );
    rule.insert(
        "threshold_count".to_string(),
        json!(number_field(&body, "threshold_count", 1, 1, 9999)),
    );
    rule.insert("group_by".to_string(), Value::String(group_by));
    rule.insert(
        "cooldown_seconds".to_string(),
        json!(number_field(&body, "cooldown_seconds", 60, 0, 86400)),
    );
    rule.insert("targets".to_string(), Value::Array(targets));
    rule.insert(
        "message_template_mode".to_string(),
        Value::String(message_template_mode),
    );
    rule.insert(
        "message_template".to_string(),
        body.get("message_template").cloned().unwrap_or(Value::Null),
    );
    rule.insert("created_at".to_string(), Value::String(now.clone()));
    rule.insert("updated_at".to_string(), Value::String(now));
    rule.insert("last_triggered_at".to_string(), Value::Null);
    let rule = Value::Object(rule);
    save_rule_raw(state, &rule).await?;
    Ok(rule)
}

pub(super) async fn update_rule_value(
    state: &AppState,
    id: &str,
    body: Value,
) -> NotifyResult<Value> {
    let translator = Translator::from_state(state).await;
    let current = load_rule(state, id)
        .await?
        .ok_or_bad(notification_service_text(&translator, "ruleNotFound", &[]))?;
    let current_object = current
        .as_object()
        .cloned()
        .ok_or_bad(notification_service_text(
            &translator,
            "invalidRuleRecord",
            &[],
        ))?;
    let event_type = trimmed_string(body.get("event_type")).unwrap_or_else(|| {
        current
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    });
    if !SYSTEM_EVENT_TYPES.contains(&event_type.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "unsupportedEventType",
            &[],
        )));
    }
    let group_by = trimmed_string(body.get("group_by")).unwrap_or_else(|| {
        current
            .get("group_by")
            .and_then(Value::as_str)
            .unwrap_or("GLOBAL")
            .to_string()
    });
    if !GROUP_BY_VALUES.contains(&group_by.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidGroupBy",
            &[],
        )));
    }
    let message_template_mode =
        trimmed_string(body.get("message_template_mode")).unwrap_or_else(|| {
            current
                .get("message_template_mode")
                .and_then(Value::as_str)
                .unwrap_or("default")
                .to_string()
        });
    if !MESSAGE_TEMPLATE_MODES.contains(&message_template_mode.as_str()) {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidMessageTemplateMode",
            &[],
        )));
    }
    let event_level_filter = if body.get("event_level_filter").is_some() {
        unique_string_array(body.get("event_level_filter"))
    } else {
        unique_string_array(current.get("event_level_filter"))
    };
    if !event_level_filter
        .iter()
        .all(|value| SYSTEM_EVENT_LEVELS.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventLevelFilter",
            &[],
        )));
    }
    let event_source_filter = if body.get("event_source_filter").is_some() {
        unique_string_array(body.get("event_source_filter"))
    } else {
        unique_string_array(current.get("event_source_filter"))
    };
    if !event_source_filter
        .iter()
        .all(|value| SYSTEM_EVENT_SOURCES.contains(&value.as_str()))
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "invalidEventSourceFilter",
            &[],
        )));
    }
    let current_targets = current
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let targets = if body.get("targets").is_some() {
        normalize_rule_targets(state, body.get("targets"), &current_targets, &translator).await?
    } else {
        current_targets
    };
    if targets.is_empty() {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "targetRequired",
            &[],
        )));
    }
    if event_type
        != current
            .get("event_type")
            .and_then(Value::as_str)
            .unwrap_or_default()
        && load_rules(state).await?.iter().any(|rule| {
            rule.get("id").and_then(Value::as_str) != Some(id)
                && rule.get("event_type").and_then(Value::as_str) == Some(&event_type)
        })
    {
        return Err(NotifyError::BadRequest(notification_service_text(
            &translator,
            "duplicateEventRule",
            &[],
        )));
    }

    let mut updated = current_object;
    updated.insert(
        "name".to_string(),
        Value::String(build_notification_rule_name(&event_type, &translator)),
    );
    if let Some(enabled) = body.get("enabled").and_then(Value::as_bool) {
        updated.insert("enabled".to_string(), Value::Bool(enabled));
    }
    updated.insert("event_type".to_string(), Value::String(event_type));
    if event_level_filter.is_empty() {
        updated.remove("event_level_filter");
    } else {
        updated.insert("event_level_filter".to_string(), json!(event_level_filter));
    }
    if event_source_filter.is_empty() {
        updated.remove("event_source_filter");
    } else {
        updated.insert(
            "event_source_filter".to_string(),
            json!(event_source_filter),
        );
    }
    if body.get("window_seconds").is_some() {
        updated.insert(
            "window_seconds".to_string(),
            json!(number_field(
                &body,
                "window_seconds",
                current
                    .get("window_seconds")
                    .and_then(Value::as_i64)
                    .unwrap_or(60),
                1,
                86400
            )),
        );
    }
    if body.get("threshold_count").is_some() {
        updated.insert(
            "threshold_count".to_string(),
            json!(number_field(
                &body,
                "threshold_count",
                current
                    .get("threshold_count")
                    .and_then(Value::as_i64)
                    .unwrap_or(1),
                1,
                9999
            )),
        );
    }
    updated.insert("group_by".to_string(), Value::String(group_by));
    if body.get("cooldown_seconds").is_some() {
        updated.insert(
            "cooldown_seconds".to_string(),
            json!(number_field(
                &body,
                "cooldown_seconds",
                current
                    .get("cooldown_seconds")
                    .and_then(Value::as_i64)
                    .unwrap_or(60),
                0,
                86400
            )),
        );
    }
    updated.insert("targets".to_string(), Value::Array(targets));
    updated.insert(
        "message_template_mode".to_string(),
        Value::String(message_template_mode),
    );
    if let Some(message_template) = body.get("message_template") {
        updated.insert("message_template".to_string(), message_template.clone());
    }
    updated.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    let updated = Value::Object(updated);
    save_rule_raw(state, &updated).await?;
    Ok(updated)
}

pub(super) async fn delete_rule_value(state: &AppState, id: &str) -> NotifyResult<()> {
    state.storage.store.delete_notification_rule(id).await?;
    Ok(())
}

pub(super) async fn normalize_rule_targets(
    state: &AppState,
    raw_targets: Option<&Value>,
    current_targets: &[Value],
    translator: &Translator,
) -> NotifyResult<Vec<Value>> {
    let Some(raw_targets) = raw_targets.and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let providers = load_providers(state).await?;
    let provider_map = providers
        .into_iter()
        .filter_map(|provider| {
            provider
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), provider.clone()))
        })
        .collect::<HashMap<_, _>>();
    let current_map = current_targets
        .iter()
        .filter_map(|target| {
            target
                .get("id")
                .and_then(Value::as_str)
                .map(|id| (id.to_string(), target.clone()))
        })
        .collect::<HashMap<_, _>>();
    let mut targets = Vec::new();
    for raw_target in raw_targets {
        let provider_id = trimmed_string(raw_target.get("provider_id")).ok_or_bad(
            notification_service_text(translator, "ruleProviderMissing", &[]),
        )?;
        let provider = provider_map
            .get(&provider_id)
            .ok_or_bad(notification_service_text(
                translator,
                "ruleProviderMissing",
                &[],
            ))?;
        let provider_type =
            provider
                .get("type")
                .and_then(Value::as_str)
                .ok_or_bad(notification_service_text(
                    translator,
                    "unsupportedProviderType",
                    &[],
                ))?;
        let definition = provider_definition(provider_type).ok_or_bad(
            notification_service_text(translator, "unsupportedProviderType", &[]),
        )?;
        let existing = raw_target
            .get("id")
            .and_then(Value::as_str)
            .and_then(|id| current_map.get(id));
        let mut raw_config = object_field(raw_target, "target_config");
        normalize_provider_target_aliases(definition.provider_type, &mut raw_config);
        if provider_type == "webhook"
            && let Some(body) = raw_config.get("body_override")
        {
            parse_webhook_body_config(body, WebhookBodyScope::Target)
                .map_err(|error| NotifyError::BadRequest(error.text(translator)))?;
        }
        let mut target_config = normalize_schema_config(&raw_config, &definition.target_schema)?;
        let provider_uses_new_webhook_headers = provider_type == "webhook"
            && provider
                .pointer("/connection_config/custom_headers")
                .is_some();
        if provider_type == "webhook" && !provider_uses_new_webhook_headers {
            let legacy_headers = raw_config.get("extra_headers_json").cloned().or_else(|| {
                existing
                    .filter(|target| {
                        target.get("provider_id").and_then(Value::as_str)
                            == Some(provider_id.as_str())
                    })
                    .and_then(|target| target.pointer("/target_config/extra_headers_json"))
                    .cloned()
            });
            if let Some(legacy_headers) = legacy_headers {
                target_config.insert("extra_headers_json".to_string(), legacy_headers);
            }
        }
        if provider_type == "webhook" {
            let legacy_extra_body = raw_config.get("extra_body_json").cloned().or_else(|| {
                existing
                    .filter(|target| {
                        target.get("provider_id").and_then(Value::as_str)
                            == Some(provider_id.as_str())
                    })
                    .and_then(|target| target.pointer("/target_config/extra_body_json"))
                    .cloned()
            });
            if let Some(legacy_extra_body) = legacy_extra_body {
                target_config.insert("extra_body_json".to_string(), legacy_extra_body);
            }
        }
        validate_required_fields(&target_config, &definition.target_schema)?;
        let mode = trimmed_string(raw_target.get("template_override_mode"))
            .or_else(|| {
                existing
                    .and_then(|target| target.get("template_override_mode"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_else(|| "inherit".to_string());
        if !TEMPLATE_OVERRIDE_MODES.contains(&mode.as_str()) {
            return Err(NotifyError::BadRequest(notification_service_text(
                translator,
                "invalidTemplateOverrideMode",
                &[],
            )));
        }
        let now = time_utils::now_iso();
        targets.push(json!({
            "id": raw_target.get("id").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| create_id("ntftarget")),
            "provider_id": provider_id,
            "enabled": raw_target.get("enabled").and_then(Value::as_bool)
                .or_else(|| existing.and_then(|target| target.get("enabled")).and_then(Value::as_bool))
                .unwrap_or(true),
            "target_config": Value::Object(target_config),
            "template_override_mode": mode,
            "template_override": raw_target.get("template_override")
                .cloned()
                .or_else(|| existing.and_then(|target| target.get("template_override")).cloned())
                .unwrap_or(Value::Null),
            "delivery_policy": raw_target.get("delivery_policy")
                .cloned()
                .or_else(|| existing.and_then(|target| target.get("delivery_policy")).cloned())
                .unwrap_or(Value::Null),
            "created_at": existing.and_then(|target| target.get("created_at")).and_then(Value::as_str).unwrap_or(&now),
            "updated_at": now
        }));
    }
    Ok(targets)
}
