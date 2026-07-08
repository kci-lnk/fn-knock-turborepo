use super::*;

pub(super) async fn notification_dispatch_tick(state: &AppState) -> anyhow::Result<()> {
    let token = create_runtime_token("dispatch");
    let acquired = state
        .store
        .acquire_notification_runtime_lease("dispatch", &token, DISPATCH_LEASE_TTL_SECONDS)
        .await?;
    if !acquired {
        return Ok(());
    }

    let result = notification_dispatch_tick_locked(state).await;
    let release_result = state
        .store
        .release_notification_runtime_lease("dispatch", &token)
        .await;
    if let Err(error) = release_result {
        tracing::warn!(%error, "failed to release notification dispatch lease");
    }
    result
}

pub(super) async fn notification_dispatch_tick_locked(state: &AppState) -> anyhow::Result<()> {
    let mut last_stream_id = state.store.get_notification_last_stream_id().await?;
    if last_stream_id.is_none() {
        let latest = state
            .store
            .latest_system_event_stream_id()
            .await?
            .unwrap_or_else(|| "0-0".to_string());
        state.store.set_notification_last_stream_id(&latest).await?;
        last_stream_id = Some(latest);
    }
    let Some(last_stream_id) = last_stream_id else {
        return Ok(());
    };

    let items = state
        .store
        .read_system_event_stream_after(&last_stream_id, STREAM_BATCH_SIZE)
        .await?;
    for (stream_id, event) in items {
        if let Err(error) = handle_notification_event(state, &event).await {
            tracing::warn!(%error, stream_id, "failed to fan out notification event");
        }
        state
            .store
            .set_notification_last_stream_id(&stream_id)
            .await?;
    }
    Ok(())
}

pub(super) async fn handle_notification_event(
    state: &AppState,
    event: &Value,
) -> anyhow::Result<()> {
    let rules = load_rules(state).await?;
    let matching_rules = rules
        .into_iter()
        .filter(|rule| event_matches_notification_rule(event, rule))
        .collect::<Vec<_>>();
    if matching_rules.is_empty() {
        return Ok(());
    }

    for rule in matching_rules {
        fanout_notification_rule(state, event, rule).await?;
    }
    Ok(())
}

pub(super) async fn fanout_notification_rule(
    state: &AppState,
    event: &Value,
    rule: Value,
) -> anyhow::Result<()> {
    let rule_id = rule.get("id").and_then(Value::as_str).unwrap_or_default();
    let event_id = event.get("id").and_then(Value::as_str).unwrap_or_default();
    if rule_id.is_empty() || event_id.is_empty() {
        return Ok(());
    }

    let trigger_id = create_stable_id("ntftrig", &[rule_id, event_id]);
    let mut trigger = load_trigger(state, &trigger_id).await?;
    let mut trigger_created = false;
    if trigger.is_none() {
        let group_by = rule
            .get("group_by")
            .and_then(Value::as_str)
            .unwrap_or("GLOBAL");
        let group_key = build_notification_group_key(event, group_by);
        let happened_at_ms = event
            .get("happened_at")
            .and_then(Value::as_str)
            .and_then(time_utils::parse_iso_ms)
            .unwrap_or_else(time_utils::now_ms);
        let window_seconds = rule
            .get("window_seconds")
            .and_then(Value::as_i64)
            .unwrap_or(60)
            .max(1);
        let matched_count = state
            .store
            .append_notification_window_hit(
                rule_id,
                &group_key,
                event_id,
                happened_at_ms,
                window_seconds,
            )
            .await?;
        let threshold_count = rule
            .get("threshold_count")
            .and_then(Value::as_i64)
            .unwrap_or(1)
            .max(1);
        if matched_count < threshold_count {
            return Ok(());
        }
        if let Some(cooldown_until) = state
            .store
            .get_notification_cooldown_until(rule_id, &group_key)
            .await?
            && time_utils::parse_iso_ms(&cooldown_until).unwrap_or_default() > time_utils::now_ms()
        {
            return Ok(());
        }

        let translator = Translator::from_state(state).await;
        let now = time_utils::now_iso();
        let draft = json!({
            "id": trigger_id,
            "rule_id": rule_id,
            "event_id": event_id,
            "group_key": group_key,
            "matched_count": matched_count,
            "message_snapshot": build_notification_message(event, &rule, matched_count, &group_key, &translator),
            "rule_snapshot": rule,
            "status": "created",
            "created_at": now
        });
        trigger_created = save_trigger_if_absent(state, &draft).await?;
        trigger = if trigger_created {
            Some(draft)
        } else {
            load_trigger(state, &trigger_id).await?
        };
    }

    let Some(trigger) = trigger else {
        return Ok(());
    };
    fanout_trigger_targets(state, event, &trigger, trigger_created).await?;
    refresh_trigger_status(
        state,
        trigger.get("id").and_then(Value::as_str).unwrap_or(""),
    )
    .await?;

    if trigger_created {
        let fanout_rule = trigger
            .get("rule_snapshot")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let cooldown_seconds = fanout_rule
            .get("cooldown_seconds")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        if cooldown_seconds > 0 {
            let until = time_utils::iso_after_seconds(cooldown_seconds);
            let rule_id = fanout_rule
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let group_key = trigger
                .get("group_key")
                .and_then(Value::as_str)
                .unwrap_or("global");
            state
                .store
                .set_notification_cooldown_until(rule_id, group_key, &until, cooldown_seconds)
                .await?;
        }
    }

    Ok(())
}

pub(super) async fn fanout_trigger_targets(
    state: &AppState,
    event: &Value,
    trigger: &Value,
    trigger_created: bool,
) -> anyhow::Result<()> {
    let trigger_id = trigger
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mut fanout_rule = trigger
        .get("rule_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let targets = fanout_rule
        .get("targets")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let translator = Translator::from_state(state).await;
    let message = trigger.get("message_snapshot").cloned().unwrap_or_else(|| {
        build_notification_message(event, &fanout_rule, 1, "global", &translator)
    });
    let trigger_created_at = trigger
        .get("created_at")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            message
                .get("occurred_at")
                .and_then(Value::as_str)
                .unwrap_or("")
        });
    let event_id = trigger
        .get("event_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let rule_id = trigger.get("rule_id").and_then(Value::as_str).unwrap_or("");

    for target in targets {
        let target_id = target.get("id").and_then(Value::as_str).unwrap_or_default();
        if target_id.is_empty() {
            continue;
        }
        let provider_id = target
            .get("provider_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let provider = if provider_id.is_empty() {
            None
        } else {
            load_provider(state, provider_id).await?
        };
        let delivery_id = create_stable_id("ntfdel", &[trigger_id, target_id]);

        let provider_enabled = provider
            .as_ref()
            .and_then(|provider| provider.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let target_enabled = target
            .get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        if provider.is_none() || !provider_enabled || !target_enabled {
            let reason = if provider.is_none() {
                "provider_missing"
            } else if !provider_enabled {
                "provider_disabled"
            } else {
                "target_disabled"
            };
            let skipped = build_delivery_value(DeliveryBuildArgs {
                id: delivery_id,
                trigger_id: trigger_id.to_string(),
                rule_id: rule_id.to_string(),
                target_id: target_id.to_string(),
                provider_id: provider_id.to_string(),
                event_id: event_id.to_string(),
                status: "skipped".to_string(),
                reason: Some(reason.to_string()),
                provider_type: provider
                    .as_ref()
                    .and_then(|provider| provider.get("type"))
                    .and_then(Value::as_str)
                    .unwrap_or("webhook")
                    .to_string(),
                message_snapshot: message.clone(),
                target_snapshot: target.clone(),
                provider_snapshot: provider
                    .as_ref()
                    .and_then(|provider| mask_provider(provider).ok())
                    .unwrap_or_else(|| {
                        deleted_provider_snapshot(provider_id, trigger_created_at, &translator)
                    }),
                attempt_count: 0,
                triggered_at: trigger_created_at.to_string(),
                next_retry_at: None,
            });
            let _ = save_delivery_if_absent(state, &skipped).await?;
            continue;
        }

        let provider = provider.unwrap_or_else(|| json!({}));
        let delivery = build_delivery_value(DeliveryBuildArgs {
            id: delivery_id.clone(),
            trigger_id: trigger_id.to_string(),
            rule_id: rule_id.to_string(),
            target_id: target_id.to_string(),
            provider_id: provider_id.to_string(),
            event_id: event_id.to_string(),
            status: "queued".to_string(),
            reason: None,
            provider_type: provider
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("webhook")
                .to_string(),
            message_snapshot: message.clone(),
            target_snapshot: target.clone(),
            provider_snapshot: mask_provider(&provider).unwrap_or_else(|_| json!({})),
            attempt_count: 0,
            triggered_at: trigger_created_at.to_string(),
            next_retry_at: Some(trigger_created_at.to_string()),
        });
        let delivery_created = save_delivery_if_absent(state, &delivery).await?;
        if delivery_created {
            state
                .store
                .enqueue_notification_delivery(&delivery_id, time_utils::now_ms())
                .await?;
            continue;
        }

        if let Some(existing) = load_delivery(state, &delivery_id).await?
            && !is_terminal_delivery_status(existing.get("status").and_then(Value::as_str))
        {
            state
                .store
                .enqueue_notification_delivery(
                    &delivery_id,
                    resolve_delivery_ready_at_ms(&existing),
                )
                .await?;
        }
    }

    if trigger_created {
        if let Some(object) = fanout_rule.as_object_mut() {
            object.insert(
                "last_triggered_at".to_string(),
                Value::String(trigger_created_at.to_string()),
            );
            object.insert(
                "updated_at".to_string(),
                Value::String(trigger_created_at.to_string()),
            );
        }
        if fanout_rule.get("id").and_then(Value::as_str).is_some() {
            save_rule_raw(state, &fanout_rule).await?;
        }
    }

    if let Some(latest) = load_trigger(state, trigger_id).await?
        && latest.get("status").and_then(Value::as_str) == Some("created")
    {
        let mut updated = latest.as_object().cloned().unwrap_or_default();
        updated.insert(
            "status".to_string(),
            Value::String("fanout_done".to_string()),
        );
        save_trigger_raw(state, &Value::Object(updated)).await?;
    }

    Ok(())
}

pub(super) async fn process_ready_deliveries(
    state: &AppState,
    limit: usize,
) -> anyhow::Result<usize> {
    let ids = state
        .store
        .pull_ready_notification_delivery_ids(limit, time_utils::now_ms())
        .await?;
    let count = ids.len();
    for id in ids {
        if let Err(error) = process_delivery(state, &id).await {
            tracing::warn!(%error, delivery_id = id, "failed to process notification delivery");
        }
    }
    Ok(count)
}

pub(super) async fn process_delivery(state: &AppState, delivery_id: &str) -> anyhow::Result<()> {
    let Some(delivery) = load_delivery(state, delivery_id).await? else {
        return Ok(());
    };
    if is_terminal_delivery_status(delivery.get("status").and_then(Value::as_str)) {
        return Ok(());
    }

    let trigger_id = delivery
        .get("trigger_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let rule_id = delivery
        .get("rule_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let provider_id = delivery
        .get("provider_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let trigger = load_trigger(state, trigger_id).await?;
    let rule = load_rule(state, rule_id).await?;
    let provider = load_provider(state, provider_id).await?;
    if trigger.is_none() || rule.is_none() || provider.is_none() {
        let mut updated = delivery.as_object().cloned().unwrap_or_default();
        updated.insert("status".to_string(), Value::String("gave_up".to_string()));
        updated.insert(
            "reason".to_string(),
            Value::String("missing_trigger_rule_or_provider".to_string()),
        );
        save_delivery_raw(state, &Value::Object(updated)).await?;
        if !trigger_id.is_empty() {
            refresh_trigger_status(state, trigger_id).await?;
        }
        return Ok(());
    }

    let trigger = trigger.unwrap_or_else(|| json!({}));
    let rule = rule.unwrap_or_else(|| json!({}));
    let provider = provider.unwrap_or_else(|| json!({}));
    let target_id = delivery
        .get("target_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let target = find_rule_target(&rule, target_id)
        .or_else(|| delivery.get("target_snapshot").cloned())
        .unwrap_or_else(|| json!({}));
    let policy = resolve_delivery_policy(target.get("delivery_policy"));
    let attempt_count = delivery
        .get("attempt_count")
        .and_then(Value::as_i64)
        .unwrap_or_default()
        + 1;
    let mut sending = delivery.as_object().cloned().unwrap_or_default();
    sending.insert("status".to_string(), Value::String("sending".to_string()));
    sending.insert("attempt_count".to_string(), json!(attempt_count));
    sending.insert("reason".to_string(), Value::Null);
    sending.insert("next_retry_at".to_string(), Value::Null);
    let sending = Value::Object(sending);
    save_delivery_raw(state, &sending).await?;

    let message = sending
        .get("message_snapshot")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let translator = Translator::from_state(state).await;
    let result = match provider.get("type").and_then(Value::as_str) {
        Some("webhook") => {
            send_webhook_delivery(
                state,
                &provider,
                &target,
                &sending,
                &trigger,
                &rule,
                policy.timeout_seconds,
                &translator,
            )
            .await
        }
        Some(provider_type) if is_http_notification_provider(provider_type) => {
            send_http_notification_provider(
                state,
                &provider,
                &target,
                &message,
                policy.timeout_seconds,
            )
            .await
        }
        Some("email") => {
            send_email_notification(
                &provider,
                &target,
                &message,
                policy.timeout_seconds,
                &translator,
            )
            .await
        }
        Some(provider_type) => ProviderTestResult {
            success: false,
            retryable: false,
            message: format!("unsupported_provider:{provider_type}"),
            request_summary: None,
            response_summary: None,
        },
        None => ProviderTestResult {
            success: false,
            retryable: false,
            message: "unsupported_provider".to_string(),
            request_summary: None,
            response_summary: None,
        },
    };
    let result = localize_provider_test_result(result, &translator);

    let mut updated = sending.as_object().cloned().unwrap_or_default();
    let retryable = result.retryable;
    updated.insert(
        "request_summary".to_string(),
        result.request_summary.clone().unwrap_or(Value::Null),
    );
    updated.insert(
        "response_summary".to_string(),
        result.response_summary.clone().unwrap_or(Value::Null),
    );
    if result.success {
        updated.insert("status".to_string(), Value::String("success".to_string()));
        updated.insert("sent_at".to_string(), Value::String(time_utils::now_iso()));
        updated.insert("next_retry_at".to_string(), Value::Null);
        save_delivery_raw(state, &Value::Object(updated)).await?;
        refresh_trigger_status(state, trigger_id).await?;
        return Ok(());
    }

    if retryable && attempt_count < policy.max_attempts {
        let next_retry_at = time_utils::iso_after_seconds(policy.backoff_seconds);
        updated.insert("status".to_string(), Value::String("failed".to_string()));
        updated.insert("reason".to_string(), Value::String(result.message));
        updated.insert(
            "next_retry_at".to_string(),
            Value::String(next_retry_at.clone()),
        );
        save_delivery_raw(state, &Value::Object(updated)).await?;
        state
            .store
            .enqueue_notification_delivery(
                delivery_id,
                time_utils::parse_iso_ms(&next_retry_at).unwrap_or_else(time_utils::now_ms),
            )
            .await?;
        return Ok(());
    }

    updated.insert("status".to_string(), Value::String("gave_up".to_string()));
    updated.insert("reason".to_string(), Value::String(result.message));
    updated.insert("next_retry_at".to_string(), Value::Null);
    save_delivery_raw(state, &Value::Object(updated)).await?;
    refresh_trigger_status(state, trigger_id).await?;
    Ok(())
}
