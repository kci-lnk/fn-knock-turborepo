use super::*;

pub(super) async fn ddns_update_interval_minutes(state: &AppState) -> anyhow::Result<i64> {
    let raw = state.store.get_string_value(DDNS_SETTINGS).await?;
    Ok(parse_settings(raw.as_deref())
        .get("updateIntervalMinutes")
        .and_then(Value::as_i64)
        .unwrap_or(10)
        .clamp(5, 1440))
}

pub(super) async fn run_automatic_ddns_check(
    state: &AppState,
    trigger: &str,
    emit_skip_log: bool,
    emit_noop_log: bool,
) -> anyhow::Result<()> {
    if state.store.get_string_value(DDNS_ENABLED).await?.as_deref() != Some("true") {
        return Ok(());
    }

    let lock_key = format!("fn_knock:lock:{DDNS_UPDATE_LOCK_NAME}");
    let lock_id = Uuid::new_v4().to_string();
    let acquired = state
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
        let settings_raw = state.store.get_string_value(DDNS_SETTINGS).await?;
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
            if let Err(error) = state
                .store
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

    if let Err(error) = state.store.delete_lock_if_owned(&lock_key, &lock_id).await {
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
    ensure_target_auxiliary_state(
        state,
        target,
        &http_options,
        true,
        Some(&trigger_label(translator, trigger)),
        translator,
    )
    .await?;

    let ips = resolve_target_ips(target, settings, translator).await?;
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

    let result = update_ddns_provider(
        translator,
        provider,
        &target.config,
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
            Ok(ResolvedTargetIps {
                ipv4: if enable_ipv4 {
                    select_interface_address(
                        &interface,
                        "ipv4",
                        target
                            .config
                            .get(DDNS_INTERFACE_IPV4_INDEX_FIELD)
                            .map(String::as_str),
                        translator,
                    )?
                } else {
                    None
                },
                ipv6: if enable_ipv6 {
                    select_interface_address(
                        &interface,
                        "ipv6",
                        target
                            .config
                            .get(DDNS_INTERFACE_IPV6_INDEX_FIELD)
                            .map(String::as_str),
                        translator,
                    )?
                } else {
                    None
                },
                source,
                source_label: ddns_text(translator, "interfaceSourceLabel", &[("name", interface)]),
                warnings: Vec::new(),
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
                    .unwrap_or("curl"),
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
                update_scope,
            })
        }
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

pub(super) fn select_interface_address(
    interface: &str,
    family: &str,
    index: Option<&str>,
    translator: &Translator,
) -> anyhow::Result<Option<String>> {
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
    let candidates = item
        .get("selectableAddresses")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(None);
    }
    let raw_index = index.unwrap_or("").trim();
    if raw_index.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "selectInterfaceAddress",
                &[(
                    "family",
                    if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string(),
                )],
            )
        );
    }
    let index = raw_index.parse::<usize>().map_err(|_| {
        anyhow::anyhow!(ddns_text(
            translator,
            "selectedInterfaceAddressUnavailable",
            &[
                ("index", raw_index.to_string()),
                (
                    "family",
                    if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string()
                ),
            ]
        ))
    })?;
    candidates
        .get(index)
        .and_then(|item| item.get("address").and_then(Value::as_str))
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| {
            anyhow::anyhow!(ddns_text(
                translator,
                "selectedInterfaceAddressUnavailable",
                &[
                    ("index", (index + 1).to_string()),
                    (
                        "family",
                        if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string(),
                    ),
                ],
            ))
        })
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
    let candidates = network
        .get("selectableAddresses")
        .and_then(Value::as_array)
        .map(|addresses| {
            addresses
                .iter()
                .filter(|item| item.get("family").and_then(Value::as_str) == Some(family))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if candidates.is_empty() {
        return None;
    }

    let index_field = if family == "ipv4" {
        DDNS_INTERFACE_IPV4_INDEX_FIELD
    } else {
        DDNS_INTERFACE_IPV6_INDEX_FIELD
    };
    let index = normalize_interface_index(target.config.get(index_field).map(String::as_str));
    let family_label = if family == "ipv4" { "IPv4" } else { "IPv6" }.to_string();
    if index.is_empty() {
        return Some(ddns_text(
            translator,
            "selectInterfaceAddress",
            &[("family", family_label)],
        ));
    }

    let index = index.parse::<usize>().unwrap_or(usize::MAX);
    if candidates.get(index).is_some() {
        None
    } else {
        Some(ddns_text(
            translator,
            "selectedInterfaceAddressUnavailable",
            &[("index", (index + 1).to_string()), ("family", family_label)],
        ))
    }
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
        .store
        .replace_hash_string_map(&target_last_check_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
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
        .store
        .replace_hash_string_map(&target_last_ip_key(&target.meta.id), &payload)
        .await?;
    if target.meta.is_primary {
        state
            .store
            .replace_hash_string_map(DDNS_LEGACY_LAST_IP, &payload)
            .await?;
    }
    Ok(())
}
