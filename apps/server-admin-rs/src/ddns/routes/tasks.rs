use super::*;

pub(super) async fn ddns_update_interval_minutes(state: &AppState) -> anyhow::Result<i64> {
    let raw = state.storage.store.get_string_value(DDNS_SETTINGS).await?;
    Ok(parse_settings(raw.as_deref())
        .get("updateIntervalMinutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .clamp(
            MIN_DDNS_UPDATE_INTERVAL_MINUTES,
            MAX_DDNS_UPDATE_INTERVAL_MINUTES,
        ))
}

pub(super) async fn run_automatic_ddns_check(
    state: &AppState,
    trigger: &str,
    emit_skip_log: bool,
    emit_noop_log: bool,
) -> anyhow::Result<()> {
    if state
        .storage
        .store
        .get_string_value(DDNS_ENABLED)
        .await?
        .as_deref()
        != Some("true")
    {
        return Ok(());
    }

    let lock_key = format!("fn_knock:lock:{DDNS_UPDATE_LOCK_NAME}");
    let lock_id = Uuid::new_v4().to_string();
    let acquired = state
        .storage
        .store
        .set_json_value_nx_ex(
            &lock_key,
            &json!({ "lockId": lock_id, "createdAt": time_utils::now_iso() }),
            DDNS_UPDATE_LOCK_TTL_SECONDS,
        )
        .await?;
    if !acquired {
        return Ok(());
    }

    let translator = Translator::from_state(state).await;
    let result = async {
        let targets = list_targets(state).await?;
        let settings_raw = state.storage.store.get_string_value(DDNS_SETTINGS).await?;
        let settings = parse_settings(settings_raw.as_deref());
        for target in targets
            .into_iter()
            .filter(|target| target.meta.is_primary || target.meta.enabled)
        {
            if let Err(error) = run_automatic_ddns_target(
                state,
                &target,
                &settings,
                trigger,
                emit_skip_log,
                emit_noop_log,
                &translator,
            )
            .await
            {
                let task_error = ddns_text(
                    &translator,
                    "taskError",
                    &[("message", error.to_string())],
                );
                let message = trigger_message(&translator, trigger, &task_error);
                let _ = set_target_last_check(state, &target, "error", &message).await;
                let _ =
                    append_target_log(state, "error", &target, &message, &translator).await;
                tracing::warn!(target_id = %target.meta.id, %error, "automatic DDNS target check failed");
            }
            if let Err(error) = state.storage.store
                .set_json_lock_if_owned_ex(
                    &lock_key,
                    &lock_id,
                    &json!({ "lockId": lock_id.clone(), "createdAt": time_utils::now_iso() }),
                    DDNS_UPDATE_LOCK_TTL_SECONDS,
                )
                .await
            {
                tracing::warn!(%error, "failed to refresh DDNS update lock");
            }
        }
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(error) = state
        .storage
        .store
        .delete_lock_if_owned(&lock_key, &lock_id)
        .await
    {
        tracing::warn!(%error, "failed to release DDNS update lock");
    }
    result
}

pub(super) async fn run_automatic_ddns_target(
    state: &AppState,
    target: &DDNSTargetRecord,
    settings: &Value,
    trigger: &str,
    emit_skip_log: bool,
    emit_noop_log: bool,
    translator: &Translator,
) -> anyhow::Result<()> {
    let Some(provider) = target.meta.provider.as_deref() else {
        record_automatic_ddns_skip(
            state,
            target,
            trigger_message(
                translator,
                trigger,
                &ddns_text(translator, "skippedNoProvider", &[]),
            ),
            emit_skip_log,
            translator,
        )
        .await?;
        return Ok(());
    };

    if let Some(reason) = target_config_incomplete_reason(target, translator) {
        let base_message = ddns_text(translator, "skippedIncompleteConfig", &[]);
        let message = if reason.is_empty() {
            base_message
        } else {
            format!("{base_message}: {reason}")
        };
        record_automatic_ddns_skip(
            state,
            target,
            trigger_message(translator, trigger, &message),
            emit_skip_log,
            translator,
        )
        .await?;
        return Ok(());
    }

    let http_options = DDNSHttpClientOptions::from_settings_and_config(settings, &target.config);
    let mut update_plan = build_ddns_provider_update_plan(provider, &target.config)?;
    if ddns_preflight_required_before_auxiliary(provider, &update_plan) {
        preflight_ddns_provider_update(translator, provider, &mut update_plan, &http_options)
            .await?;
    }
    ensure_target_auxiliary_state(
        state,
        target,
        &http_options,
        true,
        Some(&trigger_label(translator, trigger)),
        translator,
    )
    .await?;

    let mut ips = resolve_target_ips(target, settings, translator).await?;
    stabilize_automatic_interface_ips(state, target, &mut ips, translator).await?;
    for warning in &ips.warnings {
        append_target_log(
            state,
            "warn",
            target,
            &trigger_message(translator, trigger, warning),
            translator,
        )
        .await?;
    }
    for message in &ips.selection_logs {
        append_target_log(
            state,
            "info",
            target,
            &trigger_message(translator, trigger, message),
            translator,
        )
        .await?;
    }

    if ips.source == "public" && ips.ipv4.is_none() && ips.ipv6.is_none() {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(translator, "skippedPublicIpUnavailable", &[]),
        );
        set_target_last_check(state, target, "error", &message).await?;
        append_target_log(state, "error", target, &message, translator).await?;
        return Ok(());
    }

    let (scoped_ipv4, scoped_ipv6) =
        apply_update_scope(ips.update_scope, ips.ipv4.clone(), ips.ipv6.clone());
    if scoped_ipv4.is_none() && scoped_ipv6.is_none() {
        let reason = target_ip_unavailable_message(translator, ips.source, ips.update_scope);
        let skipped = ddns_text(translator, "skippedReason", &[("reason", reason)]);
        let message = trigger_message(translator, trigger, &skipped);
        set_target_last_check(state, target, "skipped", &message).await?;
        append_target_log(state, "warn", target, &message, translator).await?;
        return Ok(());
    }

    let previous_ipv4 = target.last_ip.get("ipv4").and_then(Value::as_str);
    let previous_ipv6 = target.last_ip.get("ipv6").and_then(Value::as_str);
    let ipv4_changed = scoped_ipv4
        .as_deref()
        .is_some_and(|value| Some(value) != previous_ipv4);
    let ipv6_changed = scoped_ipv6
        .as_deref()
        .is_some_and(|value| Some(value) != previous_ipv6);

    if !ipv4_changed && !ipv6_changed {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(translator, "targetIpNoChange", &[]),
        );
        set_target_last_check(state, target, "noop", &message).await?;
        if emit_noop_log {
            append_target_log(state, "info", target, &message, translator).await?;
        }
        return Ok(());
    }

    let mut changes = Vec::new();
    if ipv4_changed {
        changes.push(ddns_text(
            translator,
            "ipChange",
            &[
                ("family", "IPv4".to_string()),
                (
                    "before",
                    previous_ipv4
                        .map(str::to_string)
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
                (
                    "after",
                    scoped_ipv4
                        .clone()
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
            ],
        ));
    }
    if ipv6_changed {
        changes.push(ddns_text(
            translator,
            "ipChange",
            &[
                ("family", "IPv6".to_string()),
                (
                    "before",
                    previous_ipv6
                        .map(str::to_string)
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
                (
                    "after",
                    scoped_ipv6
                        .clone()
                        .unwrap_or_else(|| ddns_text(translator, "none", &[])),
                ),
            ],
        ));
    }
    append_target_log(
        state,
        "info",
        target,
        &trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "targetIpChanged",
                &[("changes", changes.join(", "))],
            ),
        ),
        translator,
    )
    .await?;

    preflight_ddns_provider_update(translator, provider, &mut update_plan, &http_options).await?;

    let result = execute_ddns_provider_update(
        translator,
        provider,
        &update_plan,
        &http_options,
        scoped_ipv4.as_deref(),
        scoped_ipv6.as_deref(),
    )
    .await?;

    emit_ddns_update_completed_event(
        state,
        target,
        trigger,
        provider,
        &result,
        ips.source,
        previous_ipv4,
        previous_ipv6,
        scoped_ipv4.as_deref(),
        scoped_ipv6.as_deref(),
        translator,
    )
    .await;

    if result.success {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "dnsUpdateSuccess",
                &[
                    ("provider", provider.to_string()),
                    ("message", result.message.clone()),
                ],
            ),
        );
        set_target_last_ip(
            state,
            target,
            scoped_ipv4.as_deref(),
            scoped_ipv6.as_deref(),
        )
        .await?;
        set_target_last_check(state, target, "updated", &message).await?;
        append_target_log(state, "info", target, &message, translator).await?;
    } else {
        let message = trigger_message(
            translator,
            trigger,
            &ddns_text(
                translator,
                "dnsUpdateFailed",
                &[
                    ("provider", provider.to_string()),
                    ("message", result.message.clone()),
                ],
            ),
        );
        set_target_last_check(state, target, "error", &message).await?;
        append_target_log(state, "error", target, &message, translator).await?;
    }
    Ok(())
}

pub(super) async fn record_automatic_ddns_skip(
    state: &AppState,
    target: &DDNSTargetRecord,
    message: String,
    emit_log: bool,
    translator: &Translator,
) -> anyhow::Result<()> {
    set_target_last_check(state, target, "skipped", &message).await?;
    if emit_log {
        append_target_log(state, "warn", target, &message, translator).await?;
    }
    Ok(())
}

pub(super) async fn ensure_target_auxiliary_state(
    state: &AppState,
    target: &DDNSTargetRecord,
    http_options: &DDNSHttpClientOptions,
    emit_log: bool,
    log_prefix: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<()> {
    let Some(provider) = target.meta.provider.as_deref() else {
        return Ok(());
    };
    let result = ensure_edgeone_overseas_access_synced(
        state,
        translator,
        provider,
        &target.config,
        http_options,
    )
    .await?;
    if emit_log
        && result.changed
        && let Some(message) = result.message.as_deref().filter(|value| !value.is_empty())
    {
        let message = if let Some(prefix) = log_prefix.filter(|value| !value.is_empty()) {
            format!("{prefix}: {message}")
        } else {
            message.to_string()
        };
        append_target_log(state, "info", target, &message, translator).await?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) async fn emit_ddns_update_completed_event(
    state: &AppState,
    target: &DDNSTargetRecord,
    trigger: &str,
    provider: &str,
    result: &DDNSProviderUpdateResult,
    ip_source: &str,
    previous_ipv4: Option<&str>,
    previous_ipv6: Option<&str>,
    next_ipv4: Option<&str>,
    next_ipv6: Option<&str>,
    translator: &Translator,
) {
    let summary = target_summary(target, translator);
    if let Err(error) = system_events::publish_ddns_update_completed_event(
        state,
        json!({
            "trigger": trigger,
            "target_id": target.meta.id,
            "target_name": summary.get("name").and_then(Value::as_str).unwrap_or(&target.meta.name),
            "domain_summary": summary.get("domainSummary").and_then(Value::as_str).unwrap_or(""),
            "is_primary": target.meta.is_primary,
            "provider": provider,
            "success": result.success,
            "message": result.message,
            "update_scope": normalize_update_scope(target.config.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str)),
            "ip_source": ip_source,
            "previous_ipv4": previous_ipv4,
            "previous_ipv6": previous_ipv6,
            "next_ipv4": next_ipv4,
            "next_ipv6": next_ipv6,
        }),
    )
    .await
    {
        tracing::warn!(%error, "failed to publish DDNS update completed event");
    }
}

pub(super) fn trigger_message(translator: &Translator, trigger: &str, message: &str) -> String {
    ddns_text(
        translator,
        "triggerMessage",
        &[
            ("trigger", trigger_label(translator, trigger)),
            ("message", message.to_string()),
        ],
    )
}

pub(super) fn trigger_label(translator: &Translator, trigger: &str) -> String {
    match trigger {
        "startup" => ddns_text(translator, "triggerStartup", &[]),
        "enable" => ddns_text(translator, "triggerEnable", &[]),
        _ => ddns_text(translator, "triggerCron", &[]),
    }
}

pub(super) async fn resolve_target_ips(
    target: &DDNSTargetRecord,
    settings: &Value,
    translator: &Translator,
) -> anyhow::Result<ResolvedTargetIps> {
    let update_scope = normalize_update_scope(
        target
            .config
            .get(DDNS_UPDATE_SCOPE_FIELD)
            .map(String::as_str),
    );
    let source = normalize_ip_source(target.config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    let (enable_ipv4, enable_ipv6) = update_scope_flags(update_scope);

    match source {
        "static" => Ok(ResolvedTargetIps {
            ipv4: if enable_ipv4 {
                resolve_static_address(
                    target
                        .config
                        .get(DDNS_STATIC_IPV4_FIELD)
                        .map(String::as_str),
                    4,
                    translator,
                )?
            } else {
                None
            },
            ipv6: if enable_ipv6 {
                resolve_static_address(
                    target
                        .config
                        .get(DDNS_STATIC_IPV6_FIELD)
                        .map(String::as_str),
                    6,
                    translator,
                )?
            } else {
                None
            },
            source,
            source_label: ddns_text(translator, "staticSourceLabel", &[]),
            warnings: Vec::new(),
            selection_logs: Vec::new(),
            interface_resolutions: HashMap::new(),
            update_scope,
        }),
        "domain" => {
            let domain = normalize_domain(
                target
                    .config
                    .get(DDNS_SOURCE_DOMAIN_FIELD)
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            let (ipv4, ipv6) = resolve_source_domain_addresses(&domain, translator).await?;
            Ok(ResolvedTargetIps {
                ipv4: enable_ipv4.then_some(ipv4).flatten(),
                ipv6: enable_ipv6.then_some(ipv6).flatten(),
                source,
                source_label: if domain.is_empty() {
                    ddns_text(translator, "domainSourceLabelEmpty", &[])
                } else {
                    ddns_text(
                        translator,
                        "domainSourceLabel",
                        &[("domain", domain.clone())],
                    )
                },
                warnings: Vec::new(),
                selection_logs: Vec::new(),
                interface_resolutions: HashMap::new(),
                update_scope,
            })
        }
        "interface" => {
            let interface = normalize_network_interface(
                target
                    .config
                    .get(DDNS_NETWORK_INTERFACE_FIELD)
                    .map(String::as_str),
            );
            if interface.is_empty() {
                anyhow::bail!("{}", ddns_text(translator, "interfaceRequired", &[]));
            }
            let allow_private_addresses = config_flag_enabled(
                target
                    .config
                    .get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
                    .map(String::as_str),
            );
            let ipv4_resolution = if enable_ipv4 {
                Some(select_interface_address_detailed(
                    &interface,
                    "ipv4",
                    target
                        .config
                        .get(DDNS_INTERFACE_IPV4_SELECTOR_FIELD)
                        .map(String::as_str),
                    target
                        .config
                        .get(DDNS_INTERFACE_IPV4_INDEX_FIELD)
                        .map(String::as_str),
                    target.selection_anchor.get("ipv4").and_then(Value::as_str),
                    allow_private_addresses,
                    translator,
                )?)
            } else {
                None
            };
            let ipv6_resolution = if enable_ipv6 {
                Some(select_interface_address_detailed(
                    &interface,
                    "ipv6",
                    target
                        .config
                        .get(DDNS_INTERFACE_IPV6_SELECTOR_FIELD)
                        .map(String::as_str),
                    target
                        .config
                        .get(DDNS_INTERFACE_IPV6_INDEX_FIELD)
                        .map(String::as_str),
                    target.selection_anchor.get("ipv6").and_then(Value::as_str),
                    allow_private_addresses,
                    translator,
                )?)
            } else {
                None
            };
            let mut interface_resolutions = HashMap::new();
            if let Some(resolution) = ipv4_resolution.clone() {
                interface_resolutions.insert("ipv4".to_string(), resolution);
            }
            if let Some(resolution) = ipv6_resolution.clone() {
                interface_resolutions.insert("ipv6".to_string(), resolution);
            }
            Ok(ResolvedTargetIps {
                ipv4: ipv4_resolution
                    .as_ref()
                    .and_then(|resolution| resolution.address.clone()),
                ipv6: ipv6_resolution
                    .as_ref()
                    .and_then(|resolution| resolution.address.clone()),
                source,
                source_label: ddns_text(translator, "interfaceSourceLabel", &[("name", interface)]),
                warnings: Vec::new(),
                selection_logs: ipv4_resolution
                    .as_ref()
                    .into_iter()
                    .flat_map(|resolution| resolution.selection_logs.clone())
                    .chain(
                        ipv6_resolution
                            .as_ref()
                            .into_iter()
                            .flat_map(|resolution| resolution.selection_logs.clone()),
                    )
                    .collect(),
                interface_resolutions,
                update_scope,
            })
        }
        _ => {
            let network_interface = normalize_network_interface(
                target
                    .config
                    .get(DDNS_NETWORK_INTERFACE_FIELD)
                    .map(String::as_str),
            );
            let sources = settings
                .get("publicCheckSources")
                .map(normalize_public_check_sources)
                .unwrap_or_else(default_public_check_sources);
            let ips = detect_current_public_ips(
                &sources,
                settings
                    .get("httpTransport")
                    .and_then(Value::as_str)
                    .unwrap_or("node"),
                settings
                    .get("publicDnsProvider")
                    .and_then(Value::as_str)
                    .unwrap_or(DEFAULT_PUBLIC_DNS_PROVIDER),
                Some(network_interface.as_str()),
                enable_ipv4,
                enable_ipv6,
                translator,
            )
            .await;
            let mut warnings = Vec::new();
            if enable_ipv4 && let Some(message) = ips.ipv4_error.clone() {
                warnings.push(ddns_text(
                    translator,
                    if ips.ipv6.is_some() {
                        "ipv4FailedContinueIpv6"
                    } else {
                        "ipv4Failed"
                    },
                    &[("error", message)],
                ));
            }
            if enable_ipv6 && let Some(message) = ips.ipv6_error.clone() {
                warnings.push(ddns_text(
                    translator,
                    if ips.ipv4.is_some() {
                        "ipv6FailedContinueIpv4"
                    } else {
                        "ipv6Failed"
                    },
                    &[("error", message)],
                ));
            }
            if enable_ipv6
                && let Some(ipv6) = ips.ipv6.as_deref()
                && let Some(warning) =
                    public_ipv6_not_selectable_warning(&network_interface, ipv6, translator)
            {
                warnings.push(warning);
            }
            Ok(ResolvedTargetIps {
                ipv4: ips.ipv4,
                ipv6: ips.ipv6,
                source,
                source_label: ddns_text(translator, "publicSourceLabel", &[]),
                warnings,
                selection_logs: Vec::new(),
                interface_resolutions: HashMap::new(),
                update_scope,
            })
        }
    }
}

pub(super) async fn stabilize_automatic_interface_ips(
    state: &AppState,
    target: &DDNSTargetRecord,
    ips: &mut ResolvedTargetIps,
    translator: &Translator,
) -> anyhow::Result<()> {
    if ips.source != "interface" {
        return Ok(());
    }

    let interface = normalize_network_interface(
        target
            .config
            .get(DDNS_NETWORK_INTERFACE_FIELD)
            .map(String::as_str),
    );
    let mut recovery_data = state
        .storage
        .store
        .hgetall_string_map(&target_interface_recovery_key(&target.meta.id))
        .await?;
    let original_recovery_data = recovery_data.clone();

    for family in ["ipv4", "ipv6"] {
        let Some(mut resolution) = ips.interface_resolutions.get(family).cloned() else {
            continue;
        };
        let Some(selector) = resolution.selector.clone() else {
            clear_preferred_recovery_state(&mut recovery_data, family);
            continue;
        };
        let Some(preferred) = selector.preferred_address.as_deref() else {
            clear_preferred_recovery_state(&mut recovery_data, family);
            continue;
        };
        let current_address = target.selection_anchor.get(family).and_then(Value::as_str);
        let published_address = target.last_ip.get(family).and_then(Value::as_str);
        let recovery_current = current_address.filter(|current| {
            published_address.is_some_and(|published| ip_addresses_equal(current, published))
        });

        if let Some(current) = recovery_current
            && ip_addresses_equal(current, preferred)
            && resolution
                .address
                .as_deref()
                .is_some_and(|selected| !ip_addresses_equal(selected, current))
        {
            tokio_time::sleep(Duration::from_millis(
                DDNS_INTERFACE_FAILOVER_RECHECK_DELAY_MILLIS,
            ))
            .await;
            if let Ok(fresh) = select_interface_address_detailed(
                &interface,
                family,
                target
                    .config
                    .get(selector_field(family))
                    .map(String::as_str),
                target
                    .config
                    .get(if family == "ipv4" {
                        DDNS_INTERFACE_IPV4_INDEX_FIELD
                    } else {
                        DDNS_INTERFACE_IPV6_INDEX_FIELD
                    })
                    .map(String::as_str),
                current_address,
                config_flag_enabled(
                    target
                        .config
                        .get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
                        .map(String::as_str),
                ),
                translator,
            ) {
                resolution = fresh;
            }
        }

        let Some(selection) = resolution.selection.as_ref() else {
            clear_preferred_recovery_state(&mut recovery_data, family);
            continue;
        };
        let previous_state = preferred_recovery_state(&recovery_data, family);
        let decision = stabilize_preferred_recovery(
            selection,
            &selector,
            recovery_current,
            previous_state.as_ref(),
            DDNS_INTERFACE_PREFERRED_RECOVERY_CONFIRMATIONS,
        );

        if let Some(previous) = ips.interface_resolutions.get(family) {
            ips.selection_logs
                .retain(|message| !previous.selection_logs.contains(message));
        }
        if decision.deferred {
            if let (Some(current), Some(next_state)) = (recovery_current, decision.state.as_ref()) {
                ips.selection_logs.push(ddns_text(
                    translator,
                    "interfacePreferredRecoveryDeferred",
                    &[
                        ("family", family_label(family)),
                        ("preferred", next_state.address.clone()),
                        ("current", current.to_string()),
                        ("count", next_state.confirmations.to_string()),
                        (
                            "required",
                            DDNS_INTERFACE_PREFERRED_RECOVERY_CONFIRMATIONS.to_string(),
                        ),
                    ],
                ));
            }
        } else {
            ips.selection_logs.extend(interface_address_switch_logs(
                translator,
                family,
                &resolution.mode,
                selection.eligible.len(),
                decision.selected.as_deref(),
                selection.reason,
                current_address,
            ));
        }

        set_preferred_recovery_state(&mut recovery_data, family, decision.state.as_ref());
        if family == "ipv4" {
            ips.ipv4 = decision.selected.clone();
        } else {
            ips.ipv6 = decision.selected.clone();
        }
        resolution.address = decision.selected;
        resolution.selection_logs = Vec::new();
        ips.interface_resolutions
            .insert(family.to_string(), resolution);
    }

    if recovery_data != original_recovery_data {
        state
            .storage
            .store
            .replace_hash_string_map(
                &target_interface_recovery_key(&target.meta.id),
                &recovery_data,
            )
            .await?;
    }
    Ok(())
}

fn preferred_recovery_state(
    data: &HashMap<String, String>,
    family: &str,
) -> Option<PreferredRecoveryState> {
    let address = data
        .get(&format!("{family}_address"))
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())?
        .to_string();
    let confirmations = data
        .get(&format!("{family}_confirmations"))
        .and_then(|value| value.parse::<u8>().ok())
        .unwrap_or(0);
    Some(PreferredRecoveryState {
        address,
        confirmations,
    })
}

fn set_preferred_recovery_state(
    data: &mut HashMap<String, String>,
    family: &str,
    state: Option<&PreferredRecoveryState>,
) {
    clear_preferred_recovery_state(data, family);
    if let Some(state) = state {
        data.insert(format!("{family}_address"), state.address.clone());
        data.insert(
            format!("{family}_confirmations"),
            state.confirmations.to_string(),
        );
    }
}

fn clear_preferred_recovery_state(data: &mut HashMap<String, String>, family: &str) {
    data.remove(&format!("{family}_address"));
    data.remove(&format!("{family}_confirmations"));
}

fn ip_addresses_equal(left: &str, right: &str) -> bool {
    match (
        left.trim().parse::<IpAddr>(),
        right.trim().parse::<IpAddr>(),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => left.trim() == right.trim(),
    }
}

pub(super) fn public_ipv6_not_selectable_warning(
    interface: &str,
    ipv6: &str,
    translator: &Translator,
) -> Option<String> {
    let known_ipv6_addresses = list_known_selectable_ipv6_addresses(interface);
    public_ipv6_not_selectable_warning_from_known(&known_ipv6_addresses, ipv6, translator)
}

pub(super) fn public_ipv6_not_selectable_warning_from_known(
    known_ipv6_addresses: &[String],
    ipv6: &str,
    translator: &Translator,
) -> Option<String> {
    if known_ipv6_addresses.is_empty() || known_ipv6_addresses.iter().any(|value| value == ipv6) {
        return None;
    }
    Some(ddns_text(
        translator,
        "publicIpv6NotSelectable",
        &[("ip", ipv6.to_string())],
    ))
}

pub(super) fn list_known_selectable_ipv6_addresses(interface: &str) -> Vec<String> {
    list_ddns_network_interfaces()
        .into_iter()
        .filter(|item| {
            interface.is_empty() || item.get("name").and_then(Value::as_str) == Some(interface)
        })
        .flat_map(|item| {
            item.get("selectableAddresses")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
                .into_iter()
        })
        .filter_map(|item| {
            (item.get("family").and_then(Value::as_str) == Some("ipv6"))
                .then(|| {
                    item.get("address")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .flatten()
        })
        .collect()
}

pub(super) fn update_scope_flags(scope: &str) -> (bool, bool) {
    match scope {
        "ipv4_only" => (true, false),
        "ipv6_only" => (false, true),
        _ => (true, true),
    }
}

pub(super) fn apply_update_scope(
    scope: &str,
    ipv4: Option<String>,
    ipv6: Option<String>,
) -> (Option<String>, Option<String>) {
    match scope {
        "ipv4_only" => (ipv4, None),
        "ipv6_only" => (None, ipv6),
        _ => (ipv4, ipv6),
    }
}

pub(super) fn resolve_static_address(
    value: Option<&str>,
    family: u8,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
    let value = value.unwrap_or("").trim();
    if value.is_empty() {
        return Ok(None);
    }
    let ip = value.parse::<IpAddr>().map_err(|_| {
        anyhow::anyhow!(ddns_text(
            translator,
            if family == 4 {
                "staticIpv4Invalid"
            } else {
                "staticIpv6Invalid"
            },
            &[("value", value.to_string())],
        ))
    })?;
    match (family, ip) {
        (4, IpAddr::V4(_)) | (6, IpAddr::V6(_)) => Ok(Some(value.to_string())),
        (4, _) => anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "staticIpv4Invalid",
                &[("value", value.to_string())],
            )
        ),
        (6, _) => anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "staticIpv6Invalid",
                &[("value", value.to_string())],
            )
        ),
        _ => Ok(None),
    }
}

pub(super) async fn resolve_source_domain_addresses(
    domain: &str,
    translator: &Translator,
) -> anyhow::Result<(Option<String>, Option<String>)> {
    if domain.is_empty() {
        anyhow::bail!("{}", ddns_text(translator, "sourceDomainRequired", &[]));
    }
    if !is_valid_source_domain(domain) {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "sourceDomainInvalid",
                &[("domain", domain.to_string())],
            )
        );
    }
    let mut ipv4 = None;
    let mut ipv6 = None;
    for addr in lookup_host((domain, 0)).await.map_err(|error| {
        anyhow::anyhow!(ddns_text(
            translator,
            "sourceDomainResolveFailed",
            &[("domain", domain.to_string()), ("error", error.to_string()),],
        ))
    })? {
        match addr.ip() {
            IpAddr::V4(ip) if ipv4.is_none() => ipv4 = Some(ip.to_string()),
            IpAddr::V6(ip) if ipv6.is_none() => ipv6 = Some(ip.to_string()),
            _ => {}
        }
    }
    Ok((ipv4, ipv6))
}

pub(super) fn is_valid_source_domain(domain: &str) -> bool {
    if domain.is_empty()
        || domain.len() > 253
        || domain.starts_with("http://")
        || domain.starts_with("https://")
        || domain.contains('/')
        || domain.contains(':')
        || domain.contains('*')
        || domain.chars().any(char::is_whitespace)
    {
        return false;
    }
    domain.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}

#[cfg(test)]
pub(super) fn select_interface_address(
    interface: &str,
    family: &str,
    selector: Option<&str>,
    index: Option<&str>,
    current_address: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<(Option<String>, Vec<String>, Vec<String>)> {
    let resolution = select_interface_address_detailed(
        interface,
        family,
        selector,
        index,
        current_address,
        false,
        translator,
    )?;
    Ok((resolution.address, Vec::new(), resolution.selection_logs))
}

pub(super) fn select_interface_address_detailed(
    interface: &str,
    family: &str,
    selector: Option<&str>,
    index: Option<&str>,
    current_address: Option<&str>,
    allow_private_addresses: bool,
    translator: &Translator,
) -> anyhow::Result<InterfaceAddressResolution> {
    let Some(item) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface))
    else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "interfaceNotFound",
                &[("name", interface.to_string())],
            )
        );
    };
    let candidates = interface_candidate_addresses(&item, allow_private_addresses)
        .into_iter()
        .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "interfaceSelectorNoMatch",
                &[("family", family_label(family))],
            )
        );
    }
    if let Some(selector) = parse_interface_selector(selector, family)? {
        let selection = resolve_interface_selector_with_policy(
            &item,
            family,
            &selector,
            current_address,
            allow_private_addresses,
        );
        tracing::debug!(
            interface,
            family,
            mode = ?selector.mode,
            candidate_count = selection.eligible.len(),
            selected = selection.selected.as_deref().unwrap_or(""),
            reason = selection.reason,
            "resolved DDNS interface selector"
        );
        if selection.selected.is_none() {
            anyhow::bail!(
                "{}",
                ddns_text(
                    translator,
                    "interfaceSelectorNoMatch",
                    &[("family", family_label(family))],
                )
            );
        }
        let selection_logs = interface_address_switch_logs(
            translator,
            family,
            &format!("{:?}", selector.mode).to_ascii_lowercase(),
            selection.eligible.len(),
            selection.selected.as_deref(),
            selection.reason,
            current_address,
        );
        return Ok(InterfaceAddressResolution {
            address: selection.selected.clone(),
            selection_logs,
            selection: Some(selection),
            selector: Some(selector.clone()),
            mode: format!("{:?}", selector.mode).to_ascii_lowercase(),
        });
    }
    if let Some((address, reason)) =
        legacy_select_interface_address(&candidates, family, index, current_address)
    {
        tracing::debug!(
            interface,
            family,
            selected = address,
            reason,
            "resolved legacy DDNS interface address"
        );
        let selection_logs = interface_address_switch_logs(
            translator,
            family,
            "legacy",
            candidates.len(),
            Some(&address),
            reason,
            current_address,
        );
        return Ok(InterfaceAddressResolution {
            address: Some(address),
            selection_logs,
            selection: None,
            selector: None,
            mode: "legacy".to_string(),
        });
    }

    // A missing or stale legacy index must not stop unattended DDNS updates.
    // The semantic auto selector keeps the current address when possible and
    // otherwise chooses the highest-ranked stable candidate deterministically.
    let selector = InterfaceAddressSelector::default();
    let selection = resolve_interface_selector_with_policy(
        &item,
        family,
        &selector,
        current_address,
        allow_private_addresses,
    );
    tracing::debug!(
        interface,
        family,
        mode = ?selector.mode,
        candidate_count = selection.eligible.len(),
        selected = selection.selected.as_deref().unwrap_or(""),
        reason = selection.reason,
        "resolved DDNS interface address with implicit auto selector"
    );
    let Some(address) = selection.selected.clone() else {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "interfaceSelectorNoMatch",
                &[("family", family_label(family))],
            )
        );
    };
    let selection_logs = interface_address_switch_logs(
        translator,
        family,
        "auto",
        selection.eligible.len(),
        Some(&address),
        selection.reason,
        current_address,
    );
    Ok(InterfaceAddressResolution {
        address: Some(address),
        selection_logs,
        selection: Some(selection),
        selector: Some(selector),
        mode: "auto".to_string(),
    })
}

fn interface_address_switch_logs(
    translator: &Translator,
    family: &str,
    mode: &str,
    candidate_count: usize,
    selected_address: Option<&str>,
    reason: &str,
    current_address: Option<&str>,
) -> Vec<String> {
    let Some(selected_address) = selected_address else {
        return Vec::new();
    };
    let Some(current_address) = current_address
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Vec::new();
    };
    let unchanged = match (
        current_address.parse::<IpAddr>(),
        selected_address.parse::<IpAddr>(),
    ) {
        (Ok(current), Ok(selected)) => current == selected,
        _ => current_address == selected_address,
    };
    if unchanged {
        return Vec::new();
    }
    vec![ddns_text(
        translator,
        "interfaceSelectorResolved",
        &[
            ("family", family_label(family)),
            ("mode", mode.to_string()),
            ("count", candidate_count.to_string()),
            ("address", selected_address.to_string()),
            ("reason", reason.to_string()),
        ],
    )]
}

fn family_label(family: &str) -> String {
    if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string()
}

pub(super) fn target_ip_unavailable_message(
    translator: &Translator,
    source: &str,
    scope: &str,
) -> String {
    let key = match (source, scope) {
        ("static", "ipv6_only") => "staticIpv6Unavailable",
        ("static", "ipv4_only") => "staticIpv4Unavailable",
        ("static", _) => "staticDualStackUnavailable",
        ("domain", "ipv6_only") => "domainIpv6Unavailable",
        ("domain", "ipv4_only") => "domainIpv4Unavailable",
        ("domain", _) => "domainDualStackUnavailable",
        ("interface", "ipv6_only") => "interfaceIpv6Unavailable",
        ("interface", "ipv4_only") => "interfaceIpv4Unavailable",
        ("interface", _) => "interfaceDualStackUnavailable",
        (_, "ipv6_only") => "publicIpv6Unavailable",
        (_, "ipv4_only") => "publicIpv4Unavailable",
        _ => "publicDualStackUnavailable",
    };
    ddns_text(translator, key, &[])
}

pub(super) fn target_config_incomplete_reason(
    target: &DDNSTargetRecord,
    translator: &Translator,
) -> Option<String> {
    let provider_name = target.meta.provider.as_deref()?;
    let providers = provider_catalog(translator);
    let Some(provider) = providers
        .as_array()?
        .iter()
        .find(|provider| provider.get("name").and_then(Value::as_str) == Some(provider_name))
    else {
        return Some(ddns_text(translator, "notConfigured", &[]));
    };
    let missing = provider
        .get("fields")
        .and_then(Value::as_array)?
        .iter()
        .filter(|field| field.get("required").and_then(Value::as_bool) != Some(false))
        .filter_map(|field| {
            let key = field.get("key").and_then(Value::as_str)?;
            let value = target
                .config
                .get(key)
                .map(String::as_str)
                .unwrap_or("")
                .trim();
            value.is_empty().then(|| {
                field
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or(key)
                    .to_string()
            })
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        let provider_key = ddns_provider_i18n_key(provider_name);
        return Some(format!(
            "{}: {}",
            ddns_text(
                translator,
                &format!("providers.{provider_key}.configIncomplete"),
                &[],
            ),
            missing.join(", ")
        ));
    }

    if let Err(error) = validated_ddns_domain_targets(provider_name, &target.config) {
        return Some(localize_ddns_domain_config_error(translator, &error));
    }

    let update_scope = normalize_update_scope(
        target
            .config
            .get(DDNS_UPDATE_SCOPE_FIELD)
            .map(String::as_str),
    );
    let address_mode = provider
        .pointer("/capabilities/addressMode")
        .and_then(Value::as_str);
    if address_mode == Some("single_address") && update_scope == "dual_stack" {
        let provider_label = provider
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or(provider_name);
        return Some(ddns_text(
            translator,
            "singleAddressProviderUnsupported",
            &[("provider", provider_label.to_string())],
        ));
    }

    target_runtime_config_incomplete_reason(target, update_scope, translator)
}

pub(super) fn target_runtime_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let ip_source =
        normalize_ip_source(target.config.get(DDNS_IP_SOURCE_FIELD).map(String::as_str));
    match ip_source {
        "static" => static_config_incomplete_reason(target, update_scope, translator),
        "domain" => {
            let domain = normalize_domain(
                target
                    .config
                    .get(DDNS_SOURCE_DOMAIN_FIELD)
                    .map(String::as_str)
                    .unwrap_or(""),
            );
            domain
                .is_empty()
                .then(|| ddns_text(translator, "sourceDomainRequired", &[]))
        }
        "interface" => interface_config_incomplete_reason(target, update_scope, translator),
        _ => None,
    }
}

pub(super) fn static_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let ipv4 = target
        .config
        .get(DDNS_STATIC_IPV4_FIELD)
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let ipv6 = target
        .config
        .get(DDNS_STATIC_IPV6_FIELD)
        .map(String::as_str)
        .unwrap_or("")
        .trim();
    let has_valid_ipv4 = ipv4.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv4());
    let has_valid_ipv6 = ipv6.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6());

    if !ipv4.is_empty() && !has_valid_ipv4 {
        return Some(ddns_text(
            translator,
            "staticIpv4Invalid",
            &[("value", ipv4.to_string())],
        ));
    }
    if !ipv6.is_empty() && !has_valid_ipv6 {
        return Some(ddns_text(
            translator,
            "staticIpv6Invalid",
            &[("value", ipv6.to_string())],
        ));
    }

    match update_scope {
        "ipv4_only" if !has_valid_ipv4 => Some(ddns_text(translator, "staticIpv4Unavailable", &[])),
        "ipv6_only" if !has_valid_ipv6 => Some(ddns_text(translator, "staticIpv6Unavailable", &[])),
        "dual_stack" if !has_valid_ipv4 && !has_valid_ipv6 => {
            Some(ddns_text(translator, "staticDualStackUnavailable", &[]))
        }
        _ => None,
    }
}

pub(super) fn interface_config_incomplete_reason(
    target: &DDNSTargetRecord,
    update_scope: &str,
    translator: &Translator,
) -> Option<String> {
    let interface = normalize_network_interface(
        target
            .config
            .get(DDNS_NETWORK_INTERFACE_FIELD)
            .map(String::as_str),
    );
    if interface.is_empty() {
        return Some(ddns_text(translator, "interfaceRequired", &[]));
    }

    let Some(network) = list_ddns_network_interfaces()
        .into_iter()
        .find(|item| item.get("name").and_then(Value::as_str) == Some(interface.as_str()))
    else {
        return Some(ddns_text(
            translator,
            "interfaceNotFound",
            &[("name", interface)],
        ));
    };

    let requires_ipv4 = update_scope != "ipv6_only";
    let requires_ipv6 = update_scope != "ipv4_only";
    if requires_ipv4
        && let Some(reason) =
            selected_interface_address_incomplete_reason(target, &network, "ipv4", translator)
    {
        return Some(reason);
    }
    if requires_ipv6 {
        selected_interface_address_incomplete_reason(target, &network, "ipv6", translator)
    } else {
        None
    }
}

pub(super) fn selected_interface_address_incomplete_reason(
    target: &DDNSTargetRecord,
    network: &Value,
    family: &str,
    translator: &Translator,
) -> Option<String> {
    let allow_private_addresses = config_flag_enabled(
        target
            .config
            .get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
            .map(String::as_str),
    );
    let candidates = interface_candidate_addresses(network, allow_private_addresses)
        .into_iter()
        .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    let selector_field = selector_field(family);
    let selector_value = target.config.get(selector_field).map(String::as_str);
    if selector_value.is_some_and(|value| !value.trim().is_empty()) {
        match parse_interface_selector(selector_value, family) {
            Ok(Some(_)) => {}
            Ok(None) => return None,
            Err(error) => {
                return Some(ddns_text(
                    translator,
                    "interfaceSelectorInvalid",
                    &[("message", error.to_string())],
                ));
            }
        }
        return None;
    }

    let index_field = if family == "ipv4" {
        DDNS_INTERFACE_IPV4_INDEX_FIELD
    } else {
        DDNS_INTERFACE_IPV6_INDEX_FIELD
    };
    let index = normalize_interface_index(target.config.get(index_field).map(String::as_str));
    let family_label = if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string();
    if legacy_select_interface_address(
        &candidates,
        family,
        Some(index.as_str()),
        target.selection_anchor.get(family).and_then(Value::as_str),
    )
    .is_some()
    {
        return None;
    }
    let selection = resolve_interface_selector_with_policy(
        network,
        family,
        &InterfaceAddressSelector::default(),
        target.selection_anchor.get(family).and_then(Value::as_str),
        allow_private_addresses,
    );
    if selection.selected.is_some() {
        return None;
    }

    Some(ddns_text(
        translator,
        "interfaceSelectorNoMatch",
        &[("family", family_label)],
    ))
}

pub(super) fn target_config_incomplete_message(
    target: &DDNSTargetRecord,
    translator: &Translator,
) -> Option<String> {
    target_config_incomplete_reason(target, translator).map(|reason| {
        let base_key = if target.meta.is_primary {
            "primaryConfigIncomplete"
        } else {
            "targetConfigIncomplete"
        };
        let base_message = ddns_text(translator, base_key, &[]);
        if reason.is_empty() {
            base_message
        } else {
            format!("{base_message}: {reason}")
        }
    })
}

pub(super) async fn append_target_log(
    state: &AppState,
    level: &str,
    target: &DDNSTargetRecord,
    message: &str,
    translator: &Translator,
) -> anyhow::Result<()> {
    let summary = target_summary(target, translator);
    let name = summary
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let entry = json!({
        "time": time_utils::now_iso(),
        "level": level,
        "message": format!("{} {message}", target_log_label(target, &summary, translator)),
        "targetId": target.meta.id,
        "targetName": name,
        "provider": target.meta.provider,
        "isPrimary": target.meta.is_primary
    });
    state
        .storage
        .store
        .append_log_buffer(
            DDNS_LOGS,
            &[serde_json::to_string(&entry)?],
            DDNS_LOG_TTL_SECONDS,
            DDNS_LOG_MAX_LEN,
        )
        .await?;
    Ok(())
}

pub(super) fn target_log_label(
    target: &DDNSTargetRecord,
    summary: &Value,
    translator: &Translator,
) -> String {
    let scope = if target.meta.is_primary {
        ddns_text(translator, "primaryDomainScope", &[])
    } else {
        ddns_text(translator, "additionalDomainScope", &[])
    };
    let provider = provider_label(target.meta.provider.as_deref(), translator);
    let domain = summary
        .get("domainSummary")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or_else(|| summary.get("name").and_then(Value::as_str))
        .unwrap_or("");
    if domain.is_empty() {
        format!("[{scope}][{provider}]")
    } else {
        format!("[{scope}][{provider}][{domain}]")
    }
}

pub(super) async fn set_target_last_check(
    state: &AppState,
    target: &DDNSTargetRecord,
    outcome: &str,
    message: &str,
) -> anyhow::Result<()> {
    let payload = HashMap::from([
        ("checked_at".to_string(), time_utils::now_iso()),
        ("outcome".to_string(), outcome.to_string()),
        ("message".to_string(), message.to_string()),
    ]);
    state
        .storage
        .store
        .replace_hash_string_map(&target_last_check_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
            .storage
            .store
            .replace_hash_string_map(DDNS_LEGACY_LAST_CHECK, &payload)
            .await?;
    }
    Ok(())
}

pub(super) async fn set_target_last_ip(
    state: &AppState,
    target: &DDNSTargetRecord,
    ipv4: Option<&str>,
    ipv6: Option<&str>,
) -> anyhow::Result<()> {
    let mut payload = HashMap::new();
    if let Some(value) = target.last_ip.get("ipv4").and_then(Value::as_str) {
        payload.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = target.last_ip.get("ipv6").and_then(Value::as_str) {
        payload.insert("ipv6".to_string(), value.to_string());
    }
    if let Some(value) = ipv4 {
        payload.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = ipv6 {
        payload.insert("ipv6".to_string(), value.to_string());
    }
    payload.insert("updated_at".to_string(), time_utils::now_iso());
    state
        .storage
        .store
        .replace_hash_string_map(&target_last_ip_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
            .storage
            .store
            .replace_hash_string_map(DDNS_LEGACY_LAST_IP, &payload)
            .await?;
    }
    let mut anchor = HashMap::new();
    if let Some(value) = target.selection_anchor.get("ipv4").and_then(Value::as_str) {
        anchor.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = target.selection_anchor.get("ipv6").and_then(Value::as_str) {
        anchor.insert("ipv6".to_string(), value.to_string());
    }
    if let Some(value) = ipv4 {
        anchor.insert("ipv4".to_string(), value.to_string());
    }
    if let Some(value) = ipv6 {
        anchor.insert("ipv6".to_string(), value.to_string());
    }
    anchor.insert("updated_at".to_string(), time_utils::now_iso());
    state
        .storage
        .store
        .replace_hash_string_map(&target_selection_anchor_key(&target.meta.id), &anchor)
        .await?;
    Ok(())
}
