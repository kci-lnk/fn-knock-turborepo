use super::*;

pub(super) struct NotificationDetails {
    pub(super) summary: String,
    pub(super) body_text: String,
    pub(super) body_markdown: String,
    pub(super) facts: Vec<Value>,
}

pub(super) fn build_notification_details(
    event: &Value,
    rule: &Value,
    matched_count: i64,
    translator: &Translator,
) -> NotificationDetails {
    let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
    let window_seconds = rule
        .get("window_seconds")
        .and_then(Value::as_i64)
        .unwrap_or(60);
    let aggregation =
        build_notification_aggregation_text(matched_count, window_seconds, translator);
    let mut facts = Vec::new();
    let mut summary = default_string(
        format_notification_summary(event, translator),
        &format_notification_event_label(event_type, translator),
    );
    let mut overview = summary.clone();
    let mut advice = String::new();

    match event_type {
        "FN_EVENT_AUTH_LOGIN_SUCCESS" => {
            let credential_name = default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            );
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let auth_method_raw = read_payload_value(event, "auth_method");
            let auth_provider_name = read_payload_value(event, "auth_provider_name");
            let auth_method = format_auth_method_label(&auth_method_raw, translator);
            let is_oidc_login = auth_method_raw == "OIDC";
            let login_method_text = if is_oidc_login && !auth_provider_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.loginViaProvider",
                    &[("provider", auth_provider_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.loginWithMethod",
                    &[(
                        "method",
                        default_string(
                            auth_method.clone(),
                            &notification_detail_text(translator, "unknownMethod", &[]),
                        ),
                    )],
                )
            };
            let login_auth_text = if is_oidc_login && !auth_provider_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.authViaProvider",
                    &[("provider", auth_provider_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.authWithMethod",
                    &[(
                        "method",
                        default_string(
                            auth_method.clone(),
                            &notification_detail_text(translator, "unknownMethod", &[]),
                        ),
                    )],
                )
            };
            let grant_type =
                format_grant_type_label(&read_payload_value(event, "grant_type"), translator);
            let remember_me =
                format_notification_bool(&read_payload_value(event, "remember_me"), translator);
            let expires_at = format_notification_datetime(&read_payload_value(event, "expires_at"));

            let base_summary = if is_oidc_login {
                let totp_part = if linked_totp_name.is_empty() {
                    String::new()
                } else {
                    notification_detail_text(
                        translator,
                        "authLoginSuccess.linkedTotpPart",
                        &[("totp", linked_totp_name.clone())],
                    )
                };
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryOidc",
                    &[
                        ("credential", credential_name.clone()),
                        ("method", login_method_text),
                        ("ip", ip.clone()),
                        ("totpPart", totp_part),
                    ],
                )
            } else if !linked_totp_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryTotp",
                    &[
                        (
                            "method",
                            default_string(
                                auth_method.clone(),
                                &notification_template_text(translator, "credential", &[]),
                            ),
                        ),
                        ("credential", credential_name.clone()),
                        ("totp", linked_totp_name.clone()),
                        ("ip", ip.clone()),
                    ],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.summaryCredential",
                    &[("credential", credential_name.clone()), ("ip", ip.clone())],
                )
            };
            summary = append_session_comment(base_summary, &session_comment, translator);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginSuccess.locationPart",
                    &[("location", ip_location.clone())],
                )
            };
            let comment_part = session_comment_sentence(&session_comment, translator);
            overview = notification_detail_text(
                translator,
                "authLoginSuccess.overview",
                &[
                    ("auth", login_auth_text),
                    (
                        "grantType",
                        default_string(
                            grant_type.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    ("locationPart", location_part),
                    ("commentPart", comment_part),
                ],
            );
            advice = notification_detail_text(translator, "authLoginSuccess.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginProvider"),
                auth_provider_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "grantType"),
                grant_type,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "rememberLogin"),
                remember_me,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionExpiresAt"),
                expires_at,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_AUTH_LOGOUT" => {
            let credential_name = default_string(
                read_payload_value(event, "credential_name"),
                &notification_template_text(translator, "unknownCredential", &[]),
            );
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let auth_method =
                format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
            let logout_source =
                format_logout_source_label(&read_payload_value(event, "logout_source"), translator);

            let base_summary = if linked_totp_name.is_empty() {
                notification_detail_text(
                    translator,
                    "authLogout.summaryCredential",
                    &[("credential", credential_name.clone())],
                )
            } else {
                notification_detail_text(
                    translator,
                    "authLogout.summaryTotp",
                    &[
                        (
                            "method",
                            default_string(
                                auth_method.clone(),
                                &notification_template_text(translator, "credential", &[]),
                            ),
                        ),
                        ("credential", credential_name.clone()),
                        ("totp", linked_totp_name.clone()),
                    ],
                )
            };
            summary = append_session_comment(base_summary, &session_comment, translator);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "authLogout.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    (
                        "source",
                        default_string(
                            logout_source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "commentPart",
                        session_comment_sentence(&session_comment, translator),
                    ),
                ],
            );
            advice = notification_detail_text(translator, "authLogout.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "logoutSource"),
                logout_source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginTime"),
                format_notification_datetime(&read_payload_value(event, "login_time")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_AUTH_LOGIN_FAILURE" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let attempts = default_string(read_payload_value(event, "attempts"), "0");
            let retry_after = read_payload_value(event, "retry_after_seconds");
            let blocked_until =
                format_notification_datetime(&read_payload_value(event, "blocked_until"));
            let method = format_auth_method_label(&read_payload_value(event, "method"), translator);
            let credential_name = read_payload_value(event, "credential_name");
            let linked_totp_name = read_payload_value(event, "linked_totp_name");

            summary = notification_detail_text(
                translator,
                "authLoginFailure.summary",
                &[("ip", ip.clone()), ("attempts", attempts.clone())],
            );
            let retry_part = if retry_after.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginFailure.retryPart",
                    &[("seconds", retry_after.clone())],
                )
            };
            let blocked_part = if blocked_until.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "authLoginFailure.blockedPart",
                    &[("time", blocked_until.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "authLoginFailure.overview",
                &[
                    ("ip", ip.clone()),
                    ("retryPart", retry_part),
                    ("blockedPart", blocked_part),
                ],
            );
            advice = notification_detail_text(translator, "authLoginFailure.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                format_times(&attempts, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "retryWait"),
                format_seconds(&retry_after, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "limitUntil"),
                blocked_until,
            );
        }
        "FN_EVENT_AUTH_SESSION_IP_DRIFT" => {
            let credential_name = read_payload_value(event, "credential_name");
            let linked_totp_name = read_payload_value(event, "linked_totp_name");
            let session_comment = read_session_comment(event, translator);
            let auth_method =
                format_auth_method_label(&read_payload_value(event, "auth_method"), translator);
            let from_ip = default_string(
                read_payload_value(event, "from_ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let to_ip = default_string(
                read_payload_value(event, "to_ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let source =
                format_drift_source_label(&read_payload_value(event, "drift_source"), translator);
            let session_label = format_credential_context(
                event,
                &notification_detail_text(translator, "currentSession", &[]),
                translator,
            );

            summary = append_session_comment(
                notification_detail_text(
                    translator,
                    "authSessionIpDrift.summary",
                    &[
                        ("session", session_label.clone()),
                        ("fromIp", from_ip.clone()),
                        ("toIp", to_ip.clone()),
                    ],
                ),
                &session_comment,
                translator,
            );
            overview = notification_detail_text(
                translator,
                "authSessionIpDrift.overview",
                &[
                    ("session", session_label),
                    (
                        "source",
                        default_string(
                            source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "commentPart",
                        session_comment_sentence(&session_comment, translator),
                    ),
                ],
            );
            advice = notification_detail_text(translator, "authSessionIpDrift.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "credentialName"),
                credential_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "linkedTotp"),
                linked_totp_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionComment"),
                session_comment,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "originalIp"),
                from_ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "originalLocation"),
                read_payload_value(event, "from_ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentIp"),
                to_ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentLocation"),
                read_payload_value(event, "to_ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "driftSource"),
                source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "loginTime"),
                format_notification_datetime(&read_payload_value(event, "login_time")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sessionId"),
                read_payload_value(event, "session_id"),
            );
        }
        "FN_EVENT_SECURITY_SCANNER_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let window_minutes = default_string(read_payload_value(event, "window_minutes"), "0");
            let hit_count = default_string(read_payload_value(event, "hit_count"), "0");
            let threshold = default_string(read_payload_value(event, "threshold"), "0");
            let scanner_paths = get_scanner_paths(event)
                .into_iter()
                .take(3)
                .collect::<Vec<_>>();

            summary = notification_detail_text(
                translator,
                "securityScannerBlocked.summary",
                &[("ip", ip.clone())],
            );
            let paths_part = if scanner_paths.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "securityScannerBlocked.pathsPart",
                    &[("paths", join_localized_list(&scanner_paths, translator))],
                )
            };
            overview = notification_detail_text(
                translator,
                "securityScannerBlocked.overview",
                &[
                    ("minutes", window_minutes.clone()),
                    ("hits", hit_count.clone()),
                    ("threshold", threshold.clone()),
                    ("pathsPart", paths_part),
                ],
            );
            advice = notification_detail_text(translator, "securityScannerBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                read_payload_value(event, "ip_location"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "hitCount"),
                format_times(&hit_count, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "observationWindow"),
                format_minutes(&window_minutes, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "triggerThreshold"),
                format_times(&threshold, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "recentPaths"),
                join_localized_list(&scanner_paths, translator),
            );
        }
        "FN_EVENT_DDNS_UPDATE_COMPLETED" => {
            let target_name = default_string(
                read_payload_value(event, "target_name")
                    .if_empty(read_payload_value(event, "domain_summary")),
                &notification_detail_text(translator, "ddnsUpdateCompleted.defaultTarget", &[]),
            );
            let provider = default_string(
                read_payload_value(event, "provider"),
                &notification_detail_text(translator, "unknownProvider", &[]),
            );
            let success = read_payload_value(event, "success") == "true";
            let result_message = read_payload_value(event, "message");
            let trigger =
                format_ddns_trigger_label(&read_payload_value(event, "trigger"), translator);
            let update_scope = format_ddns_update_scope_label(
                &read_payload_value(event, "update_scope"),
                translator,
            );
            let ip_source =
                format_ddns_ip_source_label(&read_payload_value(event, "ip_source"), translator);
            let ipv4_change = format_ip_transition(
                &read_payload_value(event, "previous_ipv4"),
                &read_payload_value(event, "next_ipv4"),
            );
            let ipv6_change = format_ip_transition(
                &read_payload_value(event, "previous_ipv6"),
                &read_payload_value(event, "next_ipv6"),
            );

            summary = notification_detail_text(
                translator,
                if success {
                    "ddnsUpdateCompleted.summarySuccess"
                } else {
                    "ddnsUpdateCompleted.summaryFailure"
                },
                &[("target", target_name.clone())],
            );
            let result_part = if result_message.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "ddnsUpdateCompleted.resultPart",
                    &[("message", result_message.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "ddnsUpdateCompleted.overview",
                &[
                    (
                        "trigger",
                        default_string(
                            trigger.clone(),
                            &notification_detail_text(
                                translator,
                                "ddnsUpdateCompleted.currentTask",
                                &[],
                            ),
                        ),
                    ),
                    (
                        "scope",
                        default_string(
                            update_scope.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    (
                        "ipSource",
                        default_string(
                            ip_source.clone(),
                            &notification_detail_text(translator, "unknown", &[]),
                        ),
                    ),
                    ("resultPart", result_part),
                ],
            );
            advice = notification_detail_text(
                translator,
                if success {
                    "ddnsUpdateCompleted.adviceSuccess"
                } else {
                    "ddnsUpdateCompleted.adviceFailure"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "target"),
                target_name,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "provider"),
                provider,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "targetType"),
                if read_payload_value(event, "is_primary") == "true" {
                    notification_detail_text(translator, "ddnsUpdateCompleted.primaryDomain", &[])
                } else {
                    notification_detail_text(
                        translator,
                        "ddnsUpdateCompleted.additionalDomain",
                        &[],
                    )
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "trigger"),
                trigger,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "updateScope"),
                update_scope,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipSource"),
                ip_source,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipv4Change"),
                ipv4_change,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipv6Change"),
                ipv6_change,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "result"),
                result_message,
            );
        }
        "FN_EVENT_GATEWAY_THROTTLE_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let block_seconds = default_string(read_payload_value(event, "block_seconds"), "0");
            let requests_per_second =
                default_string(read_payload_value(event, "requests_per_second"), "0");
            let burst = default_string(read_payload_value(event, "burst"), "0");
            let host = read_payload_value(event, "host");
            let path = read_payload_value(event, "path");

            summary = notification_detail_text(
                translator,
                "gatewayThrottleBlocked.summary",
                &[("ip", ip.clone()), ("seconds", block_seconds.clone())],
            );
            let target_part = if host.is_empty() && path.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "gatewayThrottleBlocked.targetPart",
                    &[("target", join_compact_parts(&[host.clone(), path.clone()]))],
                )
            };
            overview = notification_detail_text(
                translator,
                "gatewayThrottleBlocked.overview",
                &[
                    ("rate", requests_per_second.clone()),
                    ("burst", burst.clone()),
                    ("targetPart", target_part),
                ],
            );
            advice = notification_detail_text(translator, "gatewayThrottleBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockDuration"),
                format_seconds(&block_seconds, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedUntil"),
                format_notification_datetime(&read_payload_value(event, "blocked_until")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "rateLimit"),
                format_rate_per_second(&requests_per_second, translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "burstCapacity"),
                burst,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "targetHost"),
                host,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestPath"),
                path,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "routeType"),
                read_payload_value(event, "route_type"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authRoute"),
                format_notification_bool(&read_payload_value(event, "is_auth_route"), translator),
            );
        }
        "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let host = read_payload_value(event, "host");
            let path = read_payload_value(event, "path");
            let method = read_payload_value(event, "method");
            let visibility_scope = match read_payload_value(event, "visibility_scope").as_str() {
                "host" => {
                    notification_detail_text(translator, "gatewayVisibilityBlocked.scopeHost", &[])
                }
                _ => notification_detail_text(
                    translator,
                    "gatewayVisibilityBlocked.scopeGateway",
                    &[],
                ),
            };
            let visibility_mode = match read_payload_value(event, "visibility_mode").as_str() {
                "custom" => {
                    notification_detail_text(translator, "gatewayVisibilityBlocked.modeCustom", &[])
                }
                _ => notification_detail_text(
                    translator,
                    "gatewayVisibilityBlocked.modeInherit",
                    &[],
                ),
            };

            summary = notification_detail_text(
                translator,
                "gatewayVisibilityBlocked.summary",
                &[("ip", ip.clone()), ("host", host.clone())],
            );
            let path_part = if path.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "gatewayVisibilityBlocked.pathPart",
                    &[("path", path.clone())],
                )
            };
            let method_part = if method.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "gatewayVisibilityBlocked.methodPart",
                    &[("method", method.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "gatewayVisibilityBlocked.overview",
                &[
                    ("ip", ip.clone()),
                    ("host", host.clone()),
                    ("pathPart", path_part),
                    ("methodPart", method_part),
                    ("scope", visibility_scope.clone()),
                    ("mode", visibility_mode.clone()),
                ],
            );
            advice = notification_detail_text(translator, "gatewayVisibilityBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestMethod"),
                method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestScheme"),
                read_payload_value(event, "scheme"),
            );
            push_notification_fact(&mut facts, "Host".to_string(), host);
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestPath"),
                path,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "routeType"),
                read_payload_value(event, "route_type"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "routeKey"),
                read_payload_value(event, "route_key"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "visibilityScope"),
                visibility_scope,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "visibilityMode"),
                visibility_mode,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "statusCode"),
                read_payload_value(event, "status"),
            );
        }
        "FN_EVENT_WAF_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let host = read_payload_value(event, "host");
            let path = read_payload_value(event, "request_uri")
                .if_empty(read_payload_value(event, "path"));
            let rule_ids = read_payload_value(event, "rule_ids");
            let trace_id = read_payload_value(event, "trace_id");
            let action = read_payload_value(event, "action");
            let mode = read_payload_value(event, "mode");
            let action_label = format_waf_action_label(&action, translator);
            let mode_label = format_waf_mode_label(&mode, translator);
            let outcome_label = format_waf_outcome_label(&action, &mode, translator);
            let is_blocking = is_waf_blocking_action(&action, &mode);

            summary = notification_detail_text(
                translator,
                "wafBlocked.summary",
                &[("ip", ip.clone()), ("outcome", outcome_label.clone())],
            );
            let host_part = if host.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.hostPart",
                    &[("host", host.clone())],
                )
            };
            let path_part = if path.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.pathPart",
                    &[("path", path.clone())],
                )
            };
            let action_part = if action_label.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.actionPart",
                    &[("action", action_label.clone())],
                )
            };
            let mode_part = if mode_label.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.modePart",
                    &[("mode", mode_label.clone())],
                )
            };
            let rules_part = if rule_ids.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "wafBlocked.rulesPart",
                    &[("rules", rule_ids.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "wafBlocked.overview",
                &[
                    ("outcome", outcome_label.clone()),
                    ("ip", ip.clone()),
                    ("hostPart", host_part),
                    ("pathPart", path_part),
                    ("actionPart", action_part),
                    ("modePart", mode_part),
                    ("rulesPart", rules_part),
                ],
            );
            advice = notification_detail_text(
                translator,
                if is_blocking {
                    "wafBlocked.adviceBlocked"
                } else {
                    "wafBlocked.adviceLogged"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "traceId"),
                trace_id,
            );
            push_notification_fact(&mut facts, "Host".to_string(), host);
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "requestAddress"),
                path,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "outcome"),
                outcome_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "wafAction"),
                action_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "wafMode"),
                mode_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ruleIds"),
                rule_ids,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ruleBundle"),
                read_payload_value(event, "bundle_id"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "statusCode"),
                read_payload_value(event, "status"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
        }
        "FN_EVENT_SSH_LOGIN_SUCCESS" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let username = default_string(
                read_payload_value(event, "username"),
                &notification_detail_text(translator, "unknownUser", &[]),
            );
            let auth_method = read_payload_value(event, "auth_method");

            summary = notification_detail_text(
                translator,
                "sshLoginSuccess.summary",
                &[("username", username.clone()), ("ip", ip.clone())],
            );
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            let auth_part = if auth_method.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "sshLoginSuccess.authPart",
                    &[("authMethod", auth_method.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshLoginSuccess.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    ("authPart", auth_part),
                ],
            );
            advice = notification_detail_text(translator, "sshLoginSuccess.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "user"),
                username,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                auth_method,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "port"),
                read_payload_value(event, "port"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "logTime"),
                format_notification_datetime(&read_payload_value(event, "log_time")),
            );
        }
        "FN_EVENT_SSH_LOGIN_FAILURE" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let username = default_string(
                read_payload_value(event, "username"),
                &notification_detail_text(translator, "unknownUser", &[]),
            );
            let attempts = default_string(read_payload_value(event, "attempts"), "0");
            let threshold = default_string(read_payload_value(event, "threshold"), "0");
            let window_minutes = default_string(read_payload_value(event, "window_minutes"), "0");

            summary = notification_detail_text(
                translator,
                "sshLoginFailure.summary",
                &[("username", username.clone()), ("ip", ip.clone())],
            );
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "sshLoginFailure.locationPart",
                    &[("location", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshLoginFailure.overview",
                &[
                    ("minutes", window_minutes.clone()),
                    ("attempts", attempts.clone()),
                    ("threshold", threshold.clone()),
                    ("locationPart", location_part),
                ],
            );
            advice = notification_detail_text(translator, "sshLoginFailure.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "user"),
                username,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "invalidUser"),
                format_notification_bool(&read_payload_value(event, "invalid_user"), translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "authMethod"),
                read_payload_value(event, "auth_method"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "port"),
                read_payload_value(event, "port"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                attempts,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "threshold"),
                threshold,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "window"),
                format_minutes(&window_minutes, translator),
            );
        }
        "FN_EVENT_SSH_IP_BLOCKED" => {
            let ip = default_string(
                read_payload_value(event, "ip"),
                &notification_detail_text(translator, "unknownIp", &[]),
            );
            let ip_location = read_payload_value(event, "ip_location");
            let reason = read_payload_value(event, "reason");
            let reason_label = if reason == "cidr_not_allowed" {
                notification_detail_text(translator, "sshIpBlocked.reasonCidrNotAllowed", &[])
            } else {
                notification_detail_text(translator, "sshIpBlocked.reasonFailedThreshold", &[])
            };

            summary =
                notification_detail_text(translator, "sshIpBlocked.summary", &[("ip", ip.clone())]);
            let location_part = if ip_location.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    "parenthesized",
                    &[("value", ip_location.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                "sshIpBlocked.overview",
                &[
                    ("ip", ip.clone()),
                    ("locationPart", location_part),
                    ("reason", reason_label.clone()),
                ],
            );
            advice = notification_detail_text(translator, "sshIpBlocked.advice", &[]);

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sourceIp"),
                ip,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "ipLocation"),
                ip_location,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedReason"),
                reason_label,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "relatedUser"),
                read_payload_value(event, "username"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "failureAttempts"),
                read_payload_value(event, "failed_count"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "window"),
                format_minutes(&read_payload_value(event, "window_minutes"), translator),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "threshold"),
                read_payload_value(event, "threshold"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedAt"),
                format_notification_datetime(&read_payload_value(event, "blocked_at")),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "blockedUntil"),
                format_notification_datetime(&read_payload_value(event, "blocked_until")),
            );
        }
        "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE" => {
            let local_version = default_string(
                read_payload_value(event, "local_version"),
                &notification_detail_text(
                    translator,
                    "appUpdateAvailable.currentVersionUnknown",
                    &[],
                ),
            );
            let latest_version = default_string(
                read_payload_value(event, "latest_version"),
                &notification_detail_text(
                    translator,
                    "appUpdateAvailable.targetVersionUnknown",
                    &[],
                ),
            );
            let force_update = read_payload_value(event, "force_update") == "true";
            let check_reason = format_update_check_reason_label(
                &read_payload_value(event, "check_reason"),
                translator,
            );
            let release_notes = truncate_notification_text(
                &read_payload_value(event, "release_notes"),
                APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH,
            );

            summary = notification_detail_text(
                translator,
                "appUpdateAvailable.summary",
                &[("version", latest_version.clone())],
            );
            let force_part = if force_update {
                notification_detail_text(translator, "appUpdateAvailable.forcePart", &[])
            } else {
                String::new()
            };
            overview = notification_detail_text(
                translator,
                "appUpdateAvailable.overview",
                &[
                    (
                        "reason",
                        default_string(
                            check_reason.clone(),
                            &notification_detail_text(
                                translator,
                                "appUpdateAvailable.currentCheck",
                                &[],
                            ),
                        ),
                    ),
                    ("localVersion", local_version.clone()),
                    ("latestVersion", latest_version.clone()),
                    ("forcePart", force_part),
                ],
            );
            advice = if release_notes.is_empty() {
                notification_detail_text(translator, "appUpdateAvailable.advice", &[])
            } else {
                notification_detail_text(
                    translator,
                    "appUpdateAvailable.releaseNotesAdvice",
                    &[("releaseNotes", release_notes.clone())],
                )
            };

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentVersion"),
                local_version,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "latestVersion"),
                latest_version,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "checkReason"),
                check_reason,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "forceUpdate"),
                if force_update {
                    notification_template_text(translator, "yes", &[])
                } else {
                    notification_template_text(translator, "no", &[])
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "releaseNotes"),
                release_notes,
            );
        }
        "FN_EVENT_SYSTEM_CPU_ALERT"
        | "FN_EVENT_SYSTEM_CPU_RECOVERED"
        | "FN_EVENT_SYSTEM_MEMORY_ALERT"
        | "FN_EVENT_SYSTEM_MEMORY_RECOVERED" => {
            let is_cpu_event = event_type == "FN_EVENT_SYSTEM_CPU_ALERT"
                || event_type == "FN_EVENT_SYSTEM_CPU_RECOVERED";
            let recovered = event_type == "FN_EVENT_SYSTEM_CPU_RECOVERED"
                || event_type == "FN_EVENT_SYSTEM_MEMORY_RECOVERED";
            let metric_label = if is_cpu_event {
                "CPU".to_string()
            } else {
                notification_detail_text(translator, "memoryMetric", &[])
            };
            let hostname = default_string(
                read_payload_value(event, "hostname"),
                &notification_detail_text(translator, "unknownHost", &[]),
            );
            let usage_percent = default_string(read_payload_value(event, "usage_percent"), "0");
            let threshold_percent =
                default_string(read_payload_value(event, "threshold_percent"), "0");
            let recover_percent = default_string(read_payload_value(event, "recover_percent"), "0");

            summary = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredSummary"
                } else {
                    "systemMetric.alertSummary"
                },
                &[
                    ("hostname", hostname.clone()),
                    ("metric", metric_label.clone()),
                    ("usage", usage_percent.clone()),
                ],
            );
            overview = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredOverview"
                } else {
                    "systemMetric.alertOverview"
                },
                &[
                    ("hostname", hostname.clone()),
                    ("metric", metric_label),
                    ("usage", usage_percent.clone()),
                    ("recover", recover_percent.clone()),
                    ("threshold", threshold_percent.clone()),
                ],
            );
            advice = notification_detail_text(
                translator,
                if recovered {
                    "systemMetric.recoveredAdvice"
                } else {
                    "systemMetric.alertAdvice"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "hostname"),
                hostname,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "currentUsage"),
                format!("{usage_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "alertThreshold"),
                format!("{threshold_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "recoverThreshold"),
                format!("{recover_percent}%"),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sampleInterval"),
                format_seconds(
                    &read_payload_value(event, "sample_interval_seconds"),
                    translator,
                ),
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "sustainDuration"),
                format_seconds(&read_payload_value(event, "sustain_seconds"), translator),
            );
        }
        "FN_EVENT_TUNNEL_FRP_CONNECTED"
        | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
        | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED" => {
            let tunnel = tunnel_label(&read_payload_value(event, "tunnel"), event_type);
            let connected = read_payload_value(event, "status") == "connected";
            let runtime_message =
                truncate_notification_text(&read_payload_value(event, "message"), 200);
            let pid = read_payload_value(event, "pid");

            summary = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedSummary"
                } else {
                    "tunnel.disconnectedSummary"
                },
                &[("tunnel", tunnel.clone())],
            );
            let message_part = if runtime_message.is_empty() {
                String::new()
            } else {
                notification_detail_text(
                    translator,
                    if connected {
                        "tunnel.connectedMessagePart"
                    } else {
                        "tunnel.disconnectedMessagePart"
                    },
                    &[("message", runtime_message.clone())],
                )
            };
            overview = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedOverview"
                } else {
                    "tunnel.disconnectedOverview"
                },
                &[("tunnel", tunnel.clone()), ("messagePart", message_part)],
            );
            advice = notification_detail_text(
                translator,
                if connected {
                    "tunnel.connectedAdvice"
                } else {
                    "tunnel.disconnectedAdvice"
                },
                &[],
            );

            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "tunnelType"),
                tunnel,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "connectionStatus"),
                if connected {
                    notification_detail_text(translator, "connected", &[])
                } else {
                    notification_detail_text(translator, "disconnected", &[])
                },
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "processPid"),
                pid,
            );
            push_notification_fact(
                &mut facts,
                notification_fact_label(translator, "runtimeFeedback"),
                runtime_message,
            );
        }
        _ => {}
    }

    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "eventType"),
        format_notification_event_label(event_type, translator),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "riskLevel"),
        format_notification_level_label(
            event.get("level").and_then(Value::as_str).unwrap_or("INFO"),
            translator,
        ),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "eventSource"),
        format_notification_source_label(
            event
                .get("source")
                .and_then(Value::as_str)
                .unwrap_or("SERVER_ADMIN"),
            translator,
        ),
    );
    push_notification_fact(
        &mut facts,
        notification_fact_label(translator, "happenedAt"),
        format_notification_datetime(
            event
                .get("happened_at")
                .and_then(Value::as_str)
                .unwrap_or(""),
        ),
    );
    if matched_count > 1 {
        push_notification_fact(
            &mut facts,
            notification_fact_label(translator, "aggregationStats"),
            notification_detail_text(
                translator,
                "aggregationStatsValue",
                &[
                    ("count", matched_count.to_string()),
                    ("seconds", window_seconds.to_string()),
                ],
            ),
        );
    }

    NotificationDetails {
        summary: summary.trim().to_string(),
        body_text: build_notification_body_text(&overview, &aggregation, &advice),
        body_markdown: build_notification_body_markdown(
            &overview,
            &aggregation,
            &advice,
            translator,
        ),
        facts,
    }
}
