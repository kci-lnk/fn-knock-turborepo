use super::*;

pub(super) fn build_notification_aggregation_text(
    matched_count: i64,
    window_seconds: i64,
    translator: &Translator,
) -> String {
    if matched_count <= 1 {
        return String::new();
    }
    notification_template_text(
        translator,
        "aggregationText",
        &[
            ("count", matched_count.to_string()),
            ("seconds", window_seconds.to_string()),
        ],
    )
}

pub(super) fn build_notification_body_text(
    overview: &str,
    aggregation: &str,
    advice: &str,
) -> String {
    [overview, aggregation, advice]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(super) fn build_notification_body_markdown(
    overview: &str,
    aggregation: &str,
    advice: &str,
    translator: &Translator,
) -> String {
    let mut sections = Vec::new();
    if !overview.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.overview", &[]),
            overview.trim()
        ));
    }
    if !aggregation.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.aggregation", &[]),
            aggregation.trim()
        ));
    }
    if !advice.trim().is_empty() {
        sections.push(format!(
            "**{}**\n{}",
            notification_template_text(translator, "sections.advice", &[]),
            advice.trim()
        ));
    }
    sections.join("\n\n")
}

pub(super) fn read_payload_value(event: &Value, key: &str) -> String {
    event
        .get("payload")
        .and_then(Value::as_object)
        .and_then(|payload| payload.get(key))
        .map(value_to_notification_text)
        .unwrap_or_default()
}

pub(super) fn value_to_notification_text(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.trim().to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(values) => values
            .iter()
            .map(value_to_notification_text)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
    }
}

pub(super) fn push_notification_fact(facts: &mut Vec<Value>, label: String, value: String) {
    let label = label.trim();
    let value = value.trim();
    if label.is_empty() && value.is_empty() {
        return;
    }
    facts.push(json!({ "label": label, "value": value }));
}

pub(super) fn format_seconds(value: &str, translator: &Translator) -> String {
    format_unit("seconds", value, translator)
}

pub(super) fn format_minutes(value: &str, translator: &Translator) -> String {
    format_unit("minutes", value, translator)
}

pub(super) fn format_times(value: &str, translator: &Translator) -> String {
    format_unit("times", value, translator)
}

pub(super) fn format_rate_per_second(value: &str, translator: &Translator) -> String {
    format_unit("ratePerSecond", value, translator)
}

pub(super) fn format_unit(key: &str, value: &str, translator: &Translator) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    notification_detail_text(
        translator,
        &format!("units.{key}"),
        &[("count", value.to_string())],
    )
}

pub(super) fn join_localized_list(values: &[String], translator: &Translator) -> String {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(&notification_detail_text(translator, "listSeparator", &[]))
}

pub(super) fn join_compact_parts(parts: &[String]) -> String {
    parts
        .iter()
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}

pub(super) fn format_ip_transition(previous_ip: &str, next_ip: &str) -> String {
    let previous_ip = previous_ip.trim();
    let next_ip = next_ip.trim();
    if !previous_ip.is_empty() && !next_ip.is_empty() {
        format!("{previous_ip} -> {next_ip}")
    } else if !previous_ip.is_empty() {
        previous_ip.to_string()
    } else {
        next_ip.to_string()
    }
}

pub(super) fn read_session_comment(event: &Value, translator: &Translator) -> String {
    normalize_auto_ip_grant_comment(&read_payload_value(event, "session_comment"), translator)
}

pub(super) fn normalize_auto_ip_grant_comment(value: &str, translator: &Translator) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let known = [
        "auth.autoIpGrantComment",
        "server.auth.autoIpGrantComment",
        "登录后自动授权",
        "登入後自動授權",
        "Automatically authorized after sign-in",
        "로그인 후 자동 승인됨",
        "ログイン後自動認証",
    ];
    if known.contains(&value) {
        translator.t("auth.autoIpGrantComment")
    } else {
        value.to_string()
    }
}

pub(super) fn append_session_comment(
    text: String,
    session_comment: &str,
    translator: &Translator,
) -> String {
    if session_comment.trim().is_empty() {
        text
    } else {
        notification_template_text(
            translator,
            "appendSessionComment",
            &[
                ("text", text),
                (
                    "comment",
                    normalize_auto_ip_grant_comment(session_comment, translator),
                ),
            ],
        )
    }
}

pub(super) fn session_comment_sentence(session_comment: &str, translator: &Translator) -> String {
    if session_comment.trim().is_empty() {
        String::new()
    } else {
        notification_detail_text(
            translator,
            "sessionCommentSentence",
            &[(
                "comment",
                normalize_auto_ip_grant_comment(session_comment, translator),
            )],
        )
    }
}

pub(super) fn format_session_comment_compact(value: &str, translator: &Translator) -> String {
    if value.trim().is_empty() {
        String::new()
    } else {
        notification_template_text(
            translator,
            "sessionCommentCompact",
            &[(
                "comment",
                normalize_auto_ip_grant_comment(value, translator),
            )],
        )
    }
}

pub(super) fn format_credential_context(
    event: &Value,
    fallback: &str,
    translator: &Translator,
) -> String {
    let credential_name = read_payload_value(event, "credential_name");
    let linked_totp_name = read_payload_value(event, "linked_totp_name");
    let auth_method =
        format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
    if !linked_totp_name.is_empty() {
        return notification_template_text(
            translator,
            "credentialLinkedTotp",
            &[
                (
                    "authMethod",
                    default_string(
                        auth_method,
                        &notification_template_text(translator, "credential", &[]),
                    ),
                ),
                (
                    "credential",
                    default_string(
                        credential_name,
                        &notification_template_text(translator, "unknownCredential", &[]),
                    ),
                ),
                ("totp", linked_totp_name),
            ],
        );
    }
    if !credential_name.is_empty() {
        return notification_template_text(
            translator,
            "credentialName",
            &[("credential", credential_name)],
        );
    }
    fallback.to_string()
}

pub(super) fn format_notification_bool(value: &str, translator: &Translator) -> String {
    match value.trim() {
        "true" => notification_template_text(translator, "yes", &[]),
        "false" => notification_template_text(translator, "no", &[]),
        other => other.to_string(),
    }
}

pub(super) fn format_notification_datetime(value: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        return String::new();
    }
    let Some(ms) = time_utils::parse_iso_ms(value) else {
        return value.to_string();
    };
    let Ok(utc) = ::time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000)) else {
        return value.to_string();
    };
    let local = ::time::UtcOffset::current_local_offset()
        .map(|offset| utc.to_offset(offset))
        .unwrap_or(utc);
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute(),
        local.second()
    )
}

pub(super) fn get_scanner_paths(event: &Value) -> Vec<String> {
    event
        .get("payload")
        .and_then(|payload| payload.get("hits"))
        .and_then(Value::as_array)
        .map(|hits| {
            hits.iter()
                .filter_map(|hit| hit.get("path"))
                .map(value_to_notification_text)
                .filter(|value| !value.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn format_notification_summary(event: &Value, translator: &Translator) -> String {
    match event.get("type").and_then(Value::as_str).unwrap_or("") {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => {
            let auth_method = read_payload_value(event, "auth_method");
            let auth_provider_name = read_payload_value(event, "auth_provider_name");
            if auth_method == "OIDC" && !auth_provider_name.is_empty() {
                return join_compact_parts(&[
                    notification_detail_text(
                        translator,
                        "authLoginSuccess.loginViaProvider",
                        &[("provider", auth_provider_name)],
                    ),
                    default_string(
                        read_payload_value(event, "credential_name"),
                        &notification_template_text(translator, "unknownCredential", &[]),
                    ),
                    format_session_comment_compact(
                        &read_session_comment(event, translator),
                        translator,
                    ),
                    read_payload_value(event, "ip"),
                ]);
            }
            join_compact_parts(&[
                default_string(
                    read_payload_value(event, "credential_name"),
                    &notification_template_text(translator, "unknownCredential", &[]),
                ),
                format_session_comment_compact(
                    &read_session_comment(event, translator),
                    translator,
                ),
                read_payload_value(event, "ip"),
            ])
        }
        "FN_EVENT_AUTH_LOGOUT" => join_compact_parts(&[
            default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            ),
            format_session_comment_compact(&read_session_comment(event, translator), translator),
            read_payload_value(event, "ip"),
        ]),
        "FN_EVENT_AUTH_LOGIN_FAILURE" => {
            let attempts = read_payload_value(event, "attempts");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if attempts.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "short.loginFailureAttempts",
                        &[("count", attempts)],
                    )
                },
            ])
        }
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => join_compact_parts(&[
            format_credential_context(event, "", translator),
            format_session_comment_compact(&read_session_comment(event, translator), translator),
            format_ip_transition(
                &read_payload_value(event, "from_ip"),
                &read_payload_value(event, "to_ip"),
            ),
        ]),
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => {
            let hit_count = read_payload_value(event, "hit_count");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if hit_count.is_empty() {
                    notification_detail_text(translator, "short.scanBlocked", &[])
                } else {
                    notification_detail_text(translator, "short.scanHits", &[("count", hit_count)])
                },
            ])
        }
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => join_compact_parts(&[
            read_payload_value(event, "target_name")
                .if_empty(read_payload_value(event, "domain_summary"))
                .if_empty(read_payload_value(event, "provider")),
            if read_payload_value(event, "success") == "true" {
                notification_detail_text(translator, "short.success", &[])
            } else {
                notification_detail_text(translator, "short.failure", &[])
            },
        ]),
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => {
            let seconds = read_payload_value(event, "block_seconds");
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                if seconds.is_empty() {
                    notification_detail_text(translator, "short.blockTriggered", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "short.blockSeconds",
                        &[("seconds", seconds)],
                    )
                },
            ])
        }
        "FN_EVENT_WAF_BLOCKED" => {
            let rule_ids = read_payload_value(event, "rule_ids");
            let outcome = format_waf_outcome_label(
                &read_payload_value(event, "action"),
                &read_payload_value(event, "mode"),
                translator,
            );
            join_compact_parts(&[
                read_payload_value(event, "ip"),
                read_payload_value(event, "host"),
                format!("WAF {outcome}"),
                if rule_ids.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(translator, "short.rules", &[("rules", rule_ids)])
                },
            ])
        }
        "FN_EVENT_SSH_LOGIN_SUCCESS" => join_compact_parts(&[
            read_payload_value(event, "username"),
            read_payload_value(event, "ip"),
            notification_detail_text(translator, "short.sshLoginSuccess", &[]),
        ]),
        "FN_EVENT_SSH_LOGIN_FAILURE" => {
            let attempts = read_payload_value(event, "attempts");
            join_compact_parts(&[
                read_payload_value(event, "username"),
                read_payload_value(event, "ip"),
                if attempts.is_empty() {
                    notification_detail_text(translator, "short.sshLoginFailure", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "short.loginFailureAttempts",
                        &[("count", attempts)],
                    )
                },
            ])
        }
        "FN_EVENT_SSH_IP_BLOCKED" => join_compact_parts(&[
            read_payload_value(event, "ip"),
            if read_payload_value(event, "reason") == "cidr_not_allowed" {
                notification_detail_text(translator, "short.regionNotAllowed", &[])
            } else {
                notification_detail_text(translator, "short.failureThreshold", &[])
            },
        ]),
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => {
            let local_version = read_payload_value(event, "local_version");
            join_compact_parts(&[
                read_payload_value(event, "latest_version"),
                if local_version.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "short.currentVersion",
                        &[("version", local_version)],
                    )
                },
            ])
        }
        "FN_EVENT_SYSTEM_CPU_ALERT"
        | "FN_EVENT_SYSTEM_CPU_RECOVERED"
        | "FN_EVENT_SYSTEM_MEMORY_ALERT"
        | "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => join_compact_parts(&[
            read_payload_value(event, "hostname"),
            if read_payload_value(event, "usage_percent").is_empty() {
                String::new()
            } else {
                format!("{}%", read_payload_value(event, "usage_percent"))
            },
        ]),
        "FN_EVENT_TUNNEL_FRP_CONNECTED"
        | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => join_compact_parts(&[
            tunnel_label(
                &read_payload_value(event, "tunnel"),
                event.get("type").and_then(Value::as_str).unwrap_or(""),
            ),
            if read_payload_value(event, "status") == "connected" {
                notification_detail_text(translator, "connected", &[])
            } else {
                notification_detail_text(translator, "disconnected", &[])
            },
        ]),
        _ => String::new(),
    }
}

pub(super) fn translate_notification_label(
    value: &str,
    labels: &[(&str, &str)],
    translator: &Translator,
) -> String {
    let value = value.trim();
    labels
        .iter()
        .find_map(|(candidate, key)| {
            (*candidate == value).then(|| notification_template_text(translator, key, &[]))
        })
        .unwrap_or_else(|| value.to_string())
}

pub(super) fn format_auth_method_label(value: &str, translator: &Translator) -> String {
    match value.trim() {
        "TOTP" => "TOTP".to_string(),
        "PASSKEY" => "Passkey".to_string(),
        "OIDC" => notification_template_text(translator, "authMethods.oidc", &[]),
        other => other.to_string(),
    }
}

pub(super) fn format_grant_type_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("browser_session", "grantTypes.browserSession"),
            ("login_ip_grant", "grantTypes.loginIpGrant"),
        ],
        translator,
    )
}

pub(super) fn format_logout_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("user_logout", "logoutSources.userLogout"),
            ("admin_session_delete", "logoutSources.adminSessionDelete"),
        ],
        translator,
    )
}

pub(super) fn format_drift_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("proxy-session", "driftSources.proxySession"),
            ("fnos-token", "driftSources.fnosToken"),
            ("session-refresh", "driftSources.sessionRefresh"),
            ("browser-session", "driftSources.browserSession"),
        ],
        translator,
    )
}

pub(super) fn format_ddns_trigger_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("cron", "ddnsTriggers.cron"),
            ("enable", "ddnsTriggers.enable"),
            ("startup", "ddnsTriggers.startup"),
            ("manual_test", "ddnsTriggers.manualTest"),
        ],
        translator,
    )
}

pub(super) fn format_ddns_update_scope_label(value: &str, translator: &Translator) -> String {
    if value.trim() == "dual_stack" {
        "IPv4 + IPv6".to_string()
    } else {
        translate_notification_label(
            value,
            &[
                ("ipv4_only", "ddnsUpdateScopes.ipv4Only"),
                ("ipv6_only", "ddnsUpdateScopes.ipv6Only"),
            ],
            translator,
        )
    }
}

pub(super) fn format_ddns_ip_source_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("public", "ddnsIpSources.public"),
            ("interface", "ddnsIpSources.interface"),
            ("static", "ddnsIpSources.static"),
            ("domain", "ddnsIpSources.domain"),
        ],
        translator,
    )
}

pub(super) fn format_update_check_reason_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("cron", "updateCheckReasons.cron"),
            ("manual", "updateCheckReasons.manual"),
            (
                "manual-check-and-download",
                "updateCheckReasons.manualCheckAndDownload",
            ),
            ("download-bootstrap", "updateCheckReasons.downloadBootstrap"),
        ],
        translator,
    )
}

pub(super) fn format_waf_action_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("block", "wafActions.block"),
            ("deny", "wafActions.deny"),
            ("detect", "wafActions.detect"),
            ("log", "wafActions.log"),
            ("pass", "wafActions.pass"),
        ],
        translator,
    )
}

pub(super) fn format_waf_mode_label(value: &str, translator: &Translator) -> String {
    translate_notification_label(
        value,
        &[
            ("detection", "wafModes.detection"),
            ("blocking", "wafModes.blocking"),
            ("off", "wafModes.off"),
        ],
        translator,
    )
}

pub(super) fn is_waf_blocking_action(action: &str, mode: &str) -> bool {
    let action = action.trim().to_ascii_lowercase();
    if action == "block" || action == "deny" {
        return true;
    }
    if matches!(action.as_str(), "detect" | "log" | "pass") {
        return false;
    }
    mode.trim().eq_ignore_ascii_case("blocking")
}

pub(super) fn format_waf_outcome_label(
    action: &str,
    mode: &str,
    translator: &Translator,
) -> String {
    if is_waf_blocking_action(action, mode) {
        notification_template_text(translator, "wafOutcomeBlocked", &[])
    } else {
        let action_label = format_waf_action_label(action, translator);
        default_string(
            action_label,
            &notification_template_text(translator, "wafOutcomeLogged", &[]),
        )
    }
}

pub(super) fn tunnel_label(value: &str, event_type: &str) -> String {
    match value.trim() {
        "frp" => "FRP".to_string(),
        "cloudflared" => "Cloudflared".to_string(),
        "" if event_type.contains("CLOUDFLARED") => "Cloudflared".to_string(),
        "" => "FRP".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn truncate_notification_text(value: &str, max_len: usize) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_len {
        normalized
    } else {
        format!(
            "{}...",
            normalized.chars().take(max_len).collect::<String>().trim()
        )
    }
}

pub(super) fn brand_notification_title(title: &str, translator: &Translator) -> String {
    let prefix = translator.t("server.notifications.brand.prefix");
    let default_title = translator.t("server.notifications.brand.defaultTitle");
    let title = title.trim();
    if title.is_empty() {
        default_title
    } else if title.starts_with(&prefix) {
        title.to_string()
    } else {
        format!("{prefix}{title}")
    }
}
