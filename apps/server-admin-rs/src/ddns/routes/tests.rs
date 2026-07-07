use super::*;

fn provider_by_name<'a>(providers: &'a Value, name: &str) -> &'a Value {
    providers
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider.get("name").and_then(Value::as_str) == Some(name))
        .unwrap()
}

fn provider_field<'a>(provider: &'a Value, key: &str) -> &'a Value {
    provider
        .get("fields")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|field| field.get("key").and_then(Value::as_str) == Some(key))
        .unwrap()
}

fn catalog_signature(providers: &Value) -> Value {
    let items = providers
        .as_array()
        .unwrap()
        .iter()
        .map(|provider| {
            let fields = provider
                .get("fields")
                .and_then(Value::as_array)
                .unwrap()
                .iter()
                .map(|field| {
                    let options = field.get("options").and_then(Value::as_array).map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.get("value").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                    });
                    json!({
                        "key": field.get("key").and_then(Value::as_str).unwrap(),
                        "type": field.get("type").and_then(Value::as_str).unwrap(),
                        "required": field.get("required").and_then(Value::as_bool) != Some(false),
                        "options": options,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "name": provider.get("name").and_then(Value::as_str).unwrap(),
                "capabilities": provider.get("capabilities").cloned().unwrap_or(Value::Null),
                "fields": fields,
            })
        })
        .collect::<Vec<_>>();
    json!(items)
}

#[test]
fn parses_ddns_settings_with_defaults() {
    let value = parse_settings(Some(
        r#"{"updateIntervalMinutes":5,"httpTransport":"fetch","publicCheckSources":{"ipv4":["4.example.com","https://4.example.com"],"ipv6":["https://6.example.com"]}}"#,
    ));
    assert_eq!(value["updateIntervalMinutes"], json!(5));
    assert_eq!(value["httpTransport"], json!("node"));
    assert_eq!(
        value["publicCheckSources"]["ipv4"],
        json!(["https://4.example.com"])
    );
}

#[test]
fn ddns_settings_record_interval_fallback_matches_node() {
    assert_eq!(
        parse_settings(Some("{}"))["updateIntervalMinutes"],
        json!(10)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"bad"}"#))["updateIntervalMinutes"],
        json!(10)
    );
}

#[test]
fn ddns_settings_record_interval_uses_node_number_coercion() {
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":5.0}"#))["updateIntervalMinutes"],
        json!(5)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"5.0"}"#))["updateIntervalMinutes"],
        json!(5)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"5e0"}"#))["updateIntervalMinutes"],
        json!(5)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"0x10"}"#))["updateIntervalMinutes"],
        json!(16)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"+0x10"}"#))["updateIntervalMinutes"],
        json!(10)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"10x"}"#))["updateIntervalMinutes"],
        json!(10)
    );
}

#[test]
fn ddns_settings_http_transport_update_matches_node_merge() {
    let current_node = json!({ "httpTransport": "node" });
    let current_curl = json!({ "httpTransport": "curl" });
    assert_eq!(
        merge_http_transport_update(Some("curl"), &current_node),
        "curl"
    );
    assert_eq!(
        merge_http_transport_update(Some("node"), &current_curl),
        "node"
    );
    assert_eq!(
        merge_http_transport_update(Some("fetch"), &current_curl),
        "node"
    );
    assert_eq!(merge_http_transport_update(None, &current_node), "node");
    assert_eq!(
        merge_http_transport_update(Some("invalid"), &current_node),
        "curl"
    );
}

#[test]
fn ddns_provider_retry_options_follow_node_number_coercion() {
    assert_eq!(ddns_provider_retry_max_attempts(None), 2);
    assert_eq!(ddns_provider_retry_max_attempts(Some("")), 2);
    assert_eq!(ddns_provider_retry_max_attempts(Some("0")), 1);
    assert_eq!(ddns_provider_retry_max_attempts(Some("-1")), 1);
    assert_eq!(ddns_provider_retry_max_attempts(Some("1.9")), 2);
    assert_eq!(ddns_provider_retry_max_attempts(Some("  ")), 1);
    assert_eq!(ddns_provider_retry_max_attempts(Some("0x10")), 17);
    assert_eq!(ddns_provider_retry_max_attempts(Some("0b10")), 3);
    assert_eq!(ddns_provider_retry_max_attempts(Some("0o10")), 9);
    assert_eq!(ddns_provider_retry_max_attempts(Some("+0x10")), 0);
    assert_eq!(ddns_provider_retry_max_attempts(Some("1x")), 0);

    assert_eq!(ddns_provider_retry_delay_ms(None), 600);
    assert_eq!(ddns_provider_retry_delay_ms(Some("")), 600);
    assert_eq!(ddns_provider_retry_delay_ms(Some("250.9")), 250);
    assert_eq!(ddns_provider_retry_delay_ms(Some("  ")), 0);
    assert_eq!(ddns_provider_retry_delay_ms(Some("0b10")), 2);
    assert_eq!(ddns_provider_retry_delay_ms(Some("+0x10")), 0);
    assert_eq!(ddns_provider_retry_delay_ms(Some("bad")), 0);
}

#[test]
fn ddns_provider_http_defaults_match_node() {
    assert_eq!(DEFAULT_DDNS_PROVIDER_TIMEOUT_MS, 10_000);
    assert_eq!(
        noip_user_agent(),
        format!(
            "fn-knock/{} ({})",
            crate::app_version::APP_LOCAL_VERSION,
            crate::app_version::APP_GITHUB_URL
        )
    );
}

#[test]
fn ddns_provider_timeout_env_matches_node_number_and_abortsignal_edges() {
    assert_eq!(provider_timeout_ms_from_env_value(None).unwrap(), 10_000);
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("")).unwrap(),
        10_000
    );
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("  ")).unwrap(),
        10_000
    );
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("0")).unwrap(),
        10_000
    );
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("bad")).unwrap(),
        10_000
    );
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("0x10")).unwrap(),
        16
    );
    assert_eq!(
        provider_timeout_ms_from_env_value(Some("250")).unwrap(),
        250
    );
    assert!(
        provider_timeout_ms_from_env_value(Some("250.9"))
            .unwrap_err()
            .to_string()
            .contains("must be an integer")
    );
    assert!(
        provider_timeout_ms_from_env_value(Some("4294967296"))
            .unwrap_err()
            .to_string()
            .contains("<= 4294967295")
    );
}

#[test]
fn curl_header_parser_uses_final_response_block_like_node() {
    let zh = Translator::new("zh-CN");
    let (status, status_text) = parse_curl_headers_for_response(
        &zh,
        "HTTP/1.1 100 Continue\r\n\r\nHTTP/2 204 No Content\r\nserver: test\r\n\r\n",
    )
    .unwrap();
    assert_eq!(status.as_u16(), 204);
    assert_eq!(status_text, "No Content");
    let error = parse_curl_headers_for_response(&zh, "").unwrap_err();
    assert_eq!(error.to_string(), "curl 未返回任何响应头");
}

#[test]
fn tencentcloud_tc3_canonical_headers_lowercase_values_like_node() {
    assert_eq!(
        tencentcloud_tc3_canonical_headers(
            "application/json; charset=utf-8",
            "Teo.TencentCloudAPI.Com",
            "DescribeDnsRecords",
        ),
        "content-type:application/json; charset=utf-8\nhost:teo.tencentcloudapi.com\nx-tc-action:describednsrecords\n"
    );
}

#[test]
fn huawei_canonical_uri_decodes_segments_like_node() {
    assert_eq!(
        canonical_huawei_uri("/v2/zones/abc%2Fdef/recordsets"),
        "/v2/zones/abc%2Fdef/recordsets/"
    );
    assert_eq!(
        canonical_huawei_uri("/v2/zones/bad%ZZ/recordsets"),
        "/v2/zones/bad%25ZZ/recordsets/"
    );
}

#[test]
fn huawei_error_detail_matches_node_safe_json_stringify() {
    let translator = Translator::new("en");
    assert_eq!(
        huawei_error_detail(r#"{ "code": 1, "error": "bad" }"#),
        r#"{"code":1,"error":"bad"}"#
    );
    assert_eq!(huawei_error_detail("plain failure"), "plain failure");
    assert!(
        huawei_request_failed_message(&translator, 403, "Forbidden", r#"{"error":"bad"}"#)
            .contains(r#"HTTP 403 Forbidden, {"error":"bad"}"#)
    );
}

#[test]
fn alidns_change_response_fails_on_code_like_node() {
    assert!(!alidns_change_response_failed(
        &json!({ "RecordId": "123" })
    ));
    assert!(!alidns_change_response_failed(&json!({ "RecordId": 123 })));
    assert!(alidns_change_response_failed(&json!({
        "RecordId": "123",
        "Code": "InvalidParameter",
        "Message": "bad request"
    })));
    assert!(!json_value_js_truthy(Some(&json!(""))));
    assert!(!json_value_js_truthy(Some(&json!(0))));
    assert!(!json_value_js_truthy(Some(&json!(false))));
    assert!(json_value_js_truthy(Some(&json!("0"))));
    assert!(json_value_js_truthy(Some(&json!(123))));
    assert!(alidns_change_response_failed(&json!({ "Code": "Missing" })));
}

#[test]
fn dynu_provider_errors_are_failure_results_like_node() {
    let translator = Translator::new("zh-CN");
    let result = dynu_request_error_result(&translator, "network boom");
    assert!(!result.success);
    assert!(result.message.contains("network boom"));
}

#[test]
fn ddns_default_interval_parses_legacy_cron_like_node() {
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/30 * * * *")),
        Some(30)
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("0 */15 * * * *")),
        Some(15)
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/4 * * * *")),
        None
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/1441 * * * *")),
        None
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("1 */15 * * * *")),
        None
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/15 * * * 1")),
        None
    );
}

#[test]
fn strict_public_check_sources_match_node_validation() {
    let zh = Translator::new("zh-CN");
    let fallback = json!({ "ipv4": ["https://fallback4.example.com"], "ipv6": ["https://fallback6.example.com"] });
    let normalized = normalize_public_check_sources_strict(
        &json!({
            "ipv4": ["4.example.com", "https://4.example.com"],
            "ipv6": []
        }),
        &fallback,
        &zh,
    )
    .unwrap();
    assert_eq!(normalized["ipv4"], json!(["https://4.example.com"]));
    assert_eq!(normalized["ipv6"], json!([]));

    assert_eq!(
        normalize_public_check_sources_strict(
            &json!({ "ipv4": [""], "ipv6": [] }),
            &fallback,
            &zh,
        )
        .expect_err("empty source should fail"),
        "IPv4 公网探测地址不能为空"
    );
    assert_eq!(
        normalize_public_check_sources_strict(
            &json!({ "ipv4": ["ftp://example.com"], "ipv6": [] }),
            &fallback,
            &zh,
        )
        .expect_err("unsupported protocol should fail"),
        "IPv4 公网探测地址仅支持 HTTP/HTTPS: ftp://example.com"
    );
}

#[tokio::test]
async fn automatic_public_ip_detection_reports_empty_sources_like_node() {
    let zh = Translator::new("zh-CN");
    let detected = detect_current_public_ips(
        &json!({ "ipv4": [], "ipv6": [] }),
        "curl",
        None,
        true,
        true,
        &zh,
    )
    .await;
    assert_eq!(detected.ipv4, None);
    assert_eq!(detected.ipv6, None);
    assert_eq!(
        detected.ipv4_error.as_deref(),
        Some("未配置 IPv4 公网探测地址")
    );
    assert_eq!(
        detected.ipv6_error.as_deref(),
        Some("未配置 IPv6 公网探测地址")
    );
}

#[test]
fn builds_target_summary_fallbacks() {
    let zh = Translator::new("zh-CN");
    let target = default_primary_target();
    let summary = target_summary(&target, &zh);
    assert_eq!(summary["id"], json!("primary"));
    assert_eq!(summary["enabled"], json!(true));
    assert_eq!(summary["updateScope"], json!("dual_stack"));
    assert_eq!(summary["name"], json!("主域名"));
    assert_eq!(summary["providerLabel"], json!("未配置"));
    assert_eq!(summary["domainSummary"], json!("未选择提供商"));
    assert_eq!(
        target_log_label(&target, &summary, &zh),
        "[主域][未配置][未选择提供商]"
    );

    let mut extra = target;
    extra.meta.is_primary = false;
    extra.meta.name = "备用域名".to_string();
    let extra_summary = target_summary(&extra, &zh);
    assert_eq!(
        target_log_label(&extra, &extra_summary, &zh),
        "[附加域][未配置][未选择提供商]"
    );
}

#[test]
fn normalizes_last_check_outcomes() {
    assert_eq!(
        normalize_last_check_outcome(Some("updated")),
        Some("updated")
    );
    assert_eq!(normalize_last_check_outcome(Some("bad")), None);
}

#[test]
fn ddns_log_limit_parser_matches_node_parse_int_prefixes() {
    assert_eq!(parse_ddns_log_limit(None), 200);
    assert_eq!(parse_ddns_log_limit(Some("")), 200);
    assert_eq!(parse_ddns_log_limit(Some("10x")), 10);
    assert_eq!(parse_ddns_log_limit(Some("0x10")), 1);
    assert_eq!(parse_ddns_log_limit(Some("-5")), 1);
    assert_eq!(parse_ddns_log_limit(Some("5000")), 1000);
    assert_eq!(parse_ddns_log_limit(Some("abc")), 200);
}

#[test]
fn prepares_config_for_storage_like_node() {
    let config = HashMap::from([
        ("domain".to_string(), " home.example.com ".to_string()),
        (DDNS_IP_SOURCE_FIELD.to_string(), "public".to_string()),
        (DDNS_STATIC_IPV4_FIELD.to_string(), "1.2.3.4".to_string()),
        (DDNS_INTERFACE_IPV4_INDEX_FIELD.to_string(), "2".to_string()),
        (" custom ".to_string(), " keep spaces ".to_string()),
        ("".to_string(), "blank-key".to_string()),
    ]);
    let prepared = prepare_config_for_storage(
        Some("cloudflare"),
        normalize_config_map(Some("cloudflare"), &config),
    );
    assert_eq!(
        prepared.get("domain").map(String::as_str),
        Some(" home.example.com ")
    );
    assert_eq!(
        prepared.get(" custom ").map(String::as_str),
        Some(" keep spaces ")
    );
    assert_eq!(prepared.get("").map(String::as_str), Some("blank-key"));
    assert_eq!(
        prepared.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str),
        Some("dual_stack")
    );
    assert!(!prepared.contains_key(DDNS_IP_SOURCE_FIELD));
    assert!(!prepared.contains_key(DDNS_STATIC_IPV4_FIELD));
    assert!(!prepared.contains_key(DDNS_INTERFACE_IPV4_INDEX_FIELD));

    let static_config = HashMap::from([
        (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
        (
            DDNS_STATIC_IPV4_FIELD.to_string(),
            " not-an-ip ".to_string(),
        ),
    ]);
    let prepared = prepare_config_for_storage(
        Some("cloudflare"),
        normalize_config_map(Some("cloudflare"), &static_config),
    );
    assert_eq!(
        prepared.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str),
        Some("not-an-ip")
    );
}

#[test]
fn normalizes_only_ddns_common_config_fields_like_node() {
    let config = HashMap::from([
        ("domain".to_string(), " home.example.com ".to_string()),
        ("api_token".to_string(), " token-with-spaces ".to_string()),
        (DDNS_UPDATE_SCOPE_FIELD.to_string(), "ipv4_only".to_string()),
        (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
        (
            DDNS_STATIC_IPV4_FIELD.to_string(),
            " 203.0.113.10 ".to_string(),
        ),
    ]);
    let normalized = normalize_config_map(Some("cloudflare"), &config);
    assert_eq!(
        normalized.get("domain").map(String::as_str),
        Some(" home.example.com ")
    );
    assert_eq!(
        normalized.get("api_token").map(String::as_str),
        Some(" token-with-spaces ")
    );
    assert_eq!(
        normalized.get(DDNS_UPDATE_SCOPE_FIELD).map(String::as_str),
        Some("ipv4_only")
    );
    assert_eq!(
        normalized.get(DDNS_IP_SOURCE_FIELD).map(String::as_str),
        Some("static")
    );
    assert_eq!(
        normalized.get(DDNS_STATIC_IPV4_FIELD).map(String::as_str),
        Some("203.0.113.10")
    );
}

#[test]
fn duplicate_key_uses_provider_and_domain_summary() {
    let config = HashMap::from([("domain".to_string(), "Home.Example.com".to_string())]);
    assert_eq!(
        duplicate_key("cloudflare", &config),
        "cloudflare::home.example.com"
    );
    assert_eq!(duplicate_key("", &config), "");
}

#[test]
fn parses_public_check_ip_payloads_like_node_detector() {
    assert_eq!(
        parse_detected_ip_text(r#"{"ip":"203.0.113.8"}"#, 4),
        Some("203.0.113.8".to_string())
    );
    assert_eq!(
        parse_detected_ip_text("2001:db8::8\n", 6),
        Some("2001:db8::8".to_string())
    );
    assert_eq!(parse_detected_ip_text(r#"{"ip":"2001:db8::8"}"#, 4), None);
}

#[test]
fn applies_update_scope_to_resolved_ddns_ips() {
    assert_eq!(
        apply_update_scope(
            "ipv4_only",
            Some("203.0.113.8".to_string()),
            Some("2001:db8::8".to_string())
        ),
        (Some("203.0.113.8".to_string()), None)
    );
    assert_eq!(
        apply_update_scope(
            "ipv6_only",
            Some("203.0.113.8".to_string()),
            Some("2001:db8::8".to_string())
        ),
        (None, Some("2001:db8::8".to_string()))
    );
}

#[test]
fn validates_ddns_source_domain_like_node() {
    assert!(is_valid_source_domain("home.example.com"));
    assert!(!is_valid_source_domain("https://home.example.com"));
    assert!(!is_valid_source_domain("*.example.com"));
    assert!(!is_valid_source_domain("-bad.example.com"));
}

#[test]
fn detects_incomplete_ddns_target_config() {
    let now = time_utils::now_iso();
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "primary".to_string(),
            name: "Primary".to_string(),
            is_primary: true,
            enabled: true,
            provider: Some("cloudflare".to_string()),
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config: HashMap::from([("domain".to_string(), "home.example.com".to_string())]),
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    };
    let translator = Translator::new(crate::i18n::DEFAULT_LOCALE);
    let message = target_config_incomplete_message(&target, &translator).unwrap();
    assert!(message.contains("API 令牌"));
    assert!(message.contains("Zone ID"));
    assert!(message.contains("当前主域配置不完整"));
}

#[test]
fn detects_single_address_provider_dual_stack_like_node() {
    let now = time_utils::now_iso();
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "target-1".to_string(),
            name: "EdgeOne CNAME".to_string(),
            is_primary: false,
            enabled: true,
            provider: Some("edgeone_cname".to_string()),
            created_at: now.clone(),
            updated_at: now,
            sort_order: 1,
        },
        config: HashMap::from([
            ("secret_id".to_string(), "sid".to_string()),
            ("secret_key".to_string(), "skey".to_string()),
            ("zone_id".to_string(), "zone-1".to_string()),
            ("domain".to_string(), "home.example.com".to_string()),
            (
                DDNS_UPDATE_SCOPE_FIELD.to_string(),
                "dual_stack".to_string(),
            ),
        ]),
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    };
    let translator = Translator::new("zh-CN");
    let message = target_config_incomplete_message(&target, &translator).unwrap();

    assert_eq!(
        message,
        "当前条目配置不完整，请填写所有必填字段: 腾讯云 EdgeOne（CNAME 接入） 一次只能更新一个地址，请将更新范围设置为仅 IPv4 或仅 IPv6"
    );
}

#[test]
fn target_config_completeness_matches_node_runtime_inputs() {
    let now = time_utils::now_iso();
    let mut target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "target-1".to_string(),
            name: "Static".to_string(),
            is_primary: false,
            enabled: true,
            provider: Some("duckdns".to_string()),
            created_at: now.clone(),
            updated_at: now.clone(),
            sort_order: 1,
        },
        config: HashMap::from([
            ("domains".to_string(), "home".to_string()),
            ("token".to_string(), "token".to_string()),
            (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
            (DDNS_STATIC_IPV4_FIELD.to_string(), "not-an-ip".to_string()),
        ]),
        last_ip: empty_last_ip(),
        last_check: empty_last_check(),
    };
    let translator = Translator::new("zh-CN");
    let message = target_config_incomplete_message(&target, &translator).unwrap();
    assert_eq!(
        message,
        "当前条目配置不完整，请填写所有必填字段: 静态 IPv4 地址无效: not-an-ip"
    );

    target.config.insert(
        DDNS_STATIC_IPV4_FIELD.to_string(),
        "203.0.113.10".to_string(),
    );
    assert!(target_config_incomplete_message(&target, &translator).is_none());

    target.meta.provider = Some("missing-provider".to_string());
    let message = target_config_incomplete_message(&target, &translator).unwrap();
    assert_eq!(message, "当前条目配置不完整，请填写所有必填字段: 未配置");
}

#[test]
fn localizes_ddns_route_and_provider_messages() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        ddns_text(&zh, "statusLoadFailed", &[]),
        "读取 DDNS 状态失败"
    );
    assert_eq!(
        localize_ddns_error(&zh, "Primary DDNS target cannot be disabled"),
        "主域条目不可单独停用"
    );
    assert_eq!(
        localize_ddns_error(&zh, "Unknown DDNS provider: unknown"),
        "未知的 DDNS 提供商: unknown"
    );
    assert_eq!(
        public_check_request_failed_message(&zh, "https://ip.example.com", 503),
        "探测源 https://ip.example.com 请求失败: HTTP 503"
    );
    assert_eq!(
        public_check_invalid_payload_message(&zh, "https://ip.example.com", 6),
        "探测源 https://ip.example.com 未返回有效的 IPv6 地址"
    );
    assert_eq!(provider_label(Some("tencentcloud"), &zh), "腾讯云 DNS");
    assert_eq!(provider_label(Some("edgeone"), &zh), "腾讯云 EdgeOne");
    assert_eq!(
        noip_status_message(&zh, "badauth", ""),
        "badauth (用户名或密码错误)"
    );
    assert_eq!(
        noip_status_message(&zh, "custom", "raw detail"),
        "custom (raw detail)"
    );
    assert_eq!(
        ddns_text(&zh, "providers.dynu.wildcardUnchanged", &[]),
        "Dynu Wildcard Alias IP 未变化"
    );
    assert_eq!(
        ddns_text(&zh, "providers.alidns.requestFailed", &[]),
        "请求失败"
    );
    assert_eq!(
        ddns_text(&zh, "providers.alidns.recordIdMissing", &[]),
        "阿里云 DNS 返回的记录缺少 RecordId"
    );
    assert_eq!(
        ddns_text(&zh, "providers.huawei.recordsetIdMissing", &[]),
        "华为云 DNS 返回的记录集缺少 ID"
    );
    assert_eq!(
        ddns_text(&zh, "providers.esa.recordIdMissing", &[]),
        "UpdateFailed: 记录缺少 RecordId"
    );
    assert_eq!(
        ddns_text(&zh, "providers.dynu.invalidRootInfo", &[]),
        "Dynu 未返回有效的根域信息"
    );
    assert_eq!(
        ddns_text(&zh, "providers.dnspod.queryRecordFailed", &[]),
        "查询记录失败"
    );
    assert_eq!(
        ddns_text(&zh, "providers.baidu.updateFailed", &[]),
        "更新失败"
    );
    assert_eq!(
        ddns_text(&zh, "providers.porkbun.createRecordFailed", &[]),
        "创建记录失败"
    );
    assert_eq!(
        ddns_text(&zh, "providers.tencentcloud.missingCreatedRecordId", &[]),
        "腾讯云未返回创建后的 RecordId"
    );
    assert_eq!(
        ddns_text(&zh, "providers.edgeone.missingRecordId", &[]),
        "EdgeOne 返回的记录缺少 RecordId"
    );
    assert!(
        ddns_text(
            &zh,
            "providers.dynu.wildcardUnsupported",
            &[("domain", "example.com".to_string())],
        )
        .contains("example.com")
    );
    assert_eq!(
        ddns_text(
            &zh,
            "domainNotInZone",
            &[
                ("fqdn", "app.other.com".to_string()),
                ("zone", "example.com".to_string())
            ],
        ),
        "域名 app.other.com 不属于根域 example.com"
    );
    assert_eq!(
        ddns_text(
            &zh,
            "invalidJsonResponse",
            &[("text", "<html>bad</html>".to_string())],
        ),
        "响应不是合法 JSON: <html>bad</html>"
    );
    let config = HashMap::from([("ipv6prefix".to_string(), "2001:db8:1234::/64".to_string())]);
    assert_eq!(
        dynv6_sent_params(&zh, None, Some("2001:db8::8"), &config),
        "ipv4=(空), ipv6=2001:db8::8, ipv6prefix=2001:db8:1234::/64"
    );
}

#[test]
fn parses_docker_host_ipv6_interfaces() {
    let items = parse_host_if_inet6(
        "20010db8000000000000000000000001 02 40 00 00 eth0\nfe800000000000000000000000000001 02 40 20 00 eth1",
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], json!("docker-host:eth0"));
    assert_eq!(items[0]["hasIpv6"], json!(true));
    assert_eq!(items[0]["addresses"][0]["address"], json!("2001:db8::1"));
}

#[test]
fn interface_selectability_filters_private_ranges() {
    assert!(!is_selectable_interface_address(&json!({
        "family": "ipv4",
        "address": "192.168.1.10"
    })));
    assert!(is_selectable_interface_address(&json!({
        "family": "ipv4",
        "address": "8.8.8.8"
    })));
    assert!(!is_selectable_interface_address(&json!({
        "family": "ipv6",
        "address": "fd00::1"
    })));
}

#[test]
fn runtime_interfaces_keep_private_addresses_like_node() {
    let item = interface_option(
        "eth-private",
        "runtime",
        vec![json!({
            "family": "ipv4",
            "address": "192.168.1.10",
            "cidr": "192.168.1.10/24",
            "internal": false,
            "source": "runtime"
        })],
    )
    .unwrap();
    assert_eq!(item["name"], json!("eth-private"));
    assert_eq!(item["addresses"].as_array().unwrap().len(), 1);
    assert_eq!(item["selectableAddresses"].as_array().unwrap().len(), 0);
    assert!(
        interface_option(
            "docker-host:eth0",
            "docker_host",
            vec![json!({
                "family": "ipv6",
                "address": "fd00::1",
                "cidr": "fd00::1/64",
                "internal": false,
                "source": "docker_host"
            })],
        )
        .is_none()
    );
}

#[test]
fn node_transport_local_addresses_follow_node_order() {
    let item = json!({
        "source": "runtime",
        "addresses": [
            { "family": "ipv6", "address": "2001:db8::8" },
            { "family": "ipv4", "address": "192.168.1.10" },
            { "family": "ipv4", "address": "bad" }
        ]
    });
    let addresses = node_transport_local_addresses_from_interface(&item)
        .into_iter()
        .map(|ip| ip.to_string())
        .collect::<Vec<_>>();
    assert_eq!(addresses, vec!["192.168.1.10", "2001:db8::8"]);
    assert_eq!(
        first_interface_ip_from_option(&item, 4)
            .map(|ip| ip.to_string())
            .as_deref(),
        Some("192.168.1.10")
    );
}

#[test]
fn public_ipv6_selectability_warning_matches_node() {
    let zh = Translator::new("zh-CN");
    assert!(public_ipv6_not_selectable_warning_from_known(&[], "2001:db8::2", &zh).is_none());
    assert!(
        public_ipv6_not_selectable_warning_from_known(
            &["2001:db8::2".to_string()],
            "2001:db8::2",
            &zh,
        )
        .is_none()
    );
    let warning = public_ipv6_not_selectable_warning_from_known(
        &["2001:db8::1".to_string()],
        "2001:db8::2",
        &zh,
    )
    .unwrap();
    assert!(warning.contains("2001:db8::2"));
}

#[test]
fn builds_ddns_provider_query_urls() {
    let url = build_query_url(
        "https://example.com/update",
        &[
            ("hostname", "home.example.com".to_string()),
            ("myip", "203.0.113.8,2001:db8::8".to_string()),
        ],
    );
    assert_eq!(
        url,
        "https://example.com/update?hostname=home.example.com&myip=203.0.113.8%2C2001%3Adb8%3A%3A8"
    );
    let config = HashMap::from([("token".to_string(), " secret ".to_string())]);
    assert_eq!(config_value(&config, "token"), "secret");
}

#[test]
fn edgeone_cname_origin_payload_and_host_header_errors_match_node() {
    assert_eq!(
        edgeone_cname_origin_info("203.0.113.8", Some("origin.example.com")),
        json!({
            "OriginType": "IP_DOMAIN",
            "Origin": "203.0.113.8",
            "HostHeader": "origin.example.com"
        })
    );
    assert_eq!(
        edgeone_cname_origin_info("203.0.113.8", None),
        json!({
            "OriginType": "IP_DOMAIN",
            "Origin": "203.0.113.8"
        })
    );
    assert!(is_edgeone_host_header_format_error(&anyhow::anyhow!(
        "InvalidHostHeaderFormat: bad host"
    )));
    assert!(is_edgeone_host_header_format_error(&anyhow::anyhow!(
        "HostHeaderInvalid"
    )));
    assert!(!is_edgeone_host_header_format_error(&anyhow::anyhow!(
        "OtherError"
    )));
}

#[test]
fn provider_catalog_signature_matches_node_definitions() {
    let providers = provider_catalog(&Translator::new("en"));
    assert_eq!(
        catalog_signature(&providers),
        json!([
            {
                "name": "alidns",
                "capabilities": null,
                "fields": [
                    { "key": "access_key_id", "type": "text", "required": true, "options": null },
                    { "key": "access_key_secret", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "line", "type": "text", "required": false, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "baiducloud",
                "capabilities": null,
                "fields": [
                    { "key": "access_key_id", "type": "text", "required": true, "options": null },
                    { "key": "secret_access_key", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "cloudflare",
                "capabilities": null,
                "fields": [
                    { "key": "api_token", "type": "password", "required": true, "options": null },
                    { "key": "zone_id", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "proxied", "type": "select", "required": false, "options": ["false", "true"] }
                ]
            },
            {
                "name": "dnspod",
                "capabilities": null,
                "fields": [
                    { "key": "token_id", "type": "text", "required": true, "options": null },
                    { "key": "token_key", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "record_line", "type": "text", "required": false, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "duckdns",
                "capabilities": null,
                "fields": [
                    { "key": "domains", "type": "text", "required": true, "options": null },
                    { "key": "token", "type": "password", "required": true, "options": null }
                ]
            },
            {
                "name": "dynu",
                "capabilities": null,
                "fields": [
                    { "key": "api_key", "type": "password", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null },
                    { "key": "group", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "dynv6",
                "capabilities": null,
                "fields": [
                    { "key": "token", "type": "password", "required": true, "options": null },
                    { "key": "zone", "type": "text", "required": true, "options": null },
                    { "key": "ipv6prefix", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "edgeone_cname",
                "capabilities": { "addressMode": "single_address" },
                "fields": [
                    { "key": "secret_id", "type": "text", "required": true, "options": null },
                    { "key": "secret_key", "type": "password", "required": true, "options": null },
                    { "key": "zone_id", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "edgeone_overseas_access", "type": "select", "required": false, "options": ["off", "block_overseas"] },
                    { "key": "endpoint", "type": "text", "required": false, "options": null },
                    { "key": "region", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "edgeone",
                "capabilities": null,
                "fields": [
                    { "key": "secret_id", "type": "text", "required": true, "options": null },
                    { "key": "secret_key", "type": "password", "required": true, "options": null },
                    { "key": "zone_id", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "location", "type": "text", "required": false, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null },
                    { "key": "edgeone_overseas_access", "type": "select", "required": false, "options": ["off", "block_overseas"] },
                    { "key": "endpoint", "type": "text", "required": false, "options": null },
                    { "key": "region", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "esa",
                "capabilities": null,
                "fields": [
                    { "key": "access_key_id", "type": "text", "required": true, "options": null },
                    { "key": "access_key_secret", "type": "password", "required": true, "options": null },
                    { "key": "site_name", "type": "text", "required": true, "options": null },
                    { "key": "site_id", "type": "text", "required": false, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "proxied", "type": "select", "required": false, "options": ["false", "true"] },
                    { "key": "biz_name", "type": "select", "required": false, "options": ["web", "api", "image_video"] },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "godaddy",
                "capabilities": null,
                "fields": [
                    { "key": "api_key", "type": "text", "required": true, "options": null },
                    { "key": "api_secret", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "huaweicloud",
                "capabilities": null,
                "fields": [
                    { "key": "access_key_id", "type": "text", "required": true, "options": null },
                    { "key": "secret_access_key", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "noip",
                "capabilities": null,
                "fields": [
                    { "key": "hostname", "type": "text", "required": true, "options": null },
                    { "key": "username", "type": "text", "required": true, "options": null },
                    { "key": "password", "type": "password", "required": true, "options": null }
                ]
            },
            {
                "name": "porkbun",
                "capabilities": null,
                "fields": [
                    { "key": "api_key", "type": "text", "required": true, "options": null },
                    { "key": "secret_api_key", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            },
            {
                "name": "tencentcloud",
                "capabilities": null,
                "fields": [
                    { "key": "secret_id", "type": "text", "required": true, "options": null },
                    { "key": "secret_key", "type": "password", "required": true, "options": null },
                    { "key": "root_domain", "type": "text", "required": true, "options": null },
                    { "key": "domain", "type": "text", "required": true, "options": null },
                    { "key": "record_line", "type": "text", "required": false, "options": null },
                    { "key": "record_line_id", "type": "text", "required": false, "options": null },
                    { "key": "ttl", "type": "text", "required": false, "options": null }
                ]
            }
        ])
    );
}

#[test]
fn provider_catalog_localizes_edgeone_overseas_access_alias() {
    let zh_providers = provider_catalog(&Translator::new("zh-CN"));
    for provider_name in ["edgeone", "edgeone_cname"] {
        let provider = provider_by_name(&zh_providers, provider_name);
        let field = provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
        assert_eq!(field.get("label"), Some(&json!("海外访问控制")));
        assert_eq!(
            field.get("description"),
            Some(&json!(
                "当开启时，将调用 EdgeOne 安全策略 API 屏蔽海外 IP 访问；港澳台不属于海外。该设置只会在配置变更时同步一次，不会随每次 DDNS 更新重复执行。"
            ))
        );
        assert_eq!(
            field.get("options"),
            Some(&json!([
                { "label": "不使用", "value": "off" },
                { "label": "屏蔽海外 IP", "value": "block_overseas" }
            ]))
        );
    }

    let en_providers = provider_catalog(&Translator::new("en"));
    for provider_name in ["edgeone", "edgeone_cname"] {
        let provider = provider_by_name(&en_providers, provider_name);
        let field = provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD);
        assert_eq!(field.get("label"), Some(&json!("Overseas access control")));
        assert_eq!(
            field.get("options"),
            Some(&json!([
                { "label": "Off", "value": "off" },
                { "label": "Block overseas IPs", "value": "block_overseas" }
            ]))
        );
    }
}

#[test]
fn provider_catalog_localizes_select_option_aliases() {
    let zh_providers = provider_catalog(&Translator::new("zh-CN"));
    let zh_cloudflare = provider_by_name(&zh_providers, "cloudflare");
    assert_eq!(
        provider_field(zh_cloudflare, "proxied").get("options"),
        Some(&json!([
            { "label": "仅解析", "value": "false" },
            { "label": "橙色云朵", "value": "true" }
        ]))
    );
    for provider_name in ["edgeone", "edgeone_cname"] {
        let provider = provider_by_name(&zh_providers, provider_name);
        assert_eq!(
            provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD).get("options"),
            Some(&json!([
                { "label": "不使用", "value": "off" },
                { "label": "屏蔽海外 IP", "value": "block_overseas" }
            ]))
        );
    }
    let zh_esa = provider_by_name(&zh_providers, "esa");
    assert_eq!(
        provider_field(zh_esa, "proxied").get("options"),
        Some(&json!([
            { "label": "仅解析", "value": "false" },
            { "label": "开启代理", "value": "true" }
        ]))
    );
    assert_eq!(
        provider_field(zh_esa, "biz_name").get("options"),
        Some(&json!([
            { "label": "网页", "value": "web" },
            { "label": "接口", "value": "api" },
            { "label": "音视频", "value": "image_video" }
        ]))
    );

    let en_providers = provider_catalog(&Translator::new("en"));
    let en_cloudflare = provider_by_name(&en_providers, "cloudflare");
    assert_eq!(
        provider_field(en_cloudflare, "proxied").get("options"),
        Some(&json!([
            { "label": "DNS only", "value": "false" },
            { "label": "Orange cloud", "value": "true" }
        ]))
    );
    for provider_name in ["edgeone", "edgeone_cname"] {
        let provider = provider_by_name(&en_providers, provider_name);
        assert_eq!(
            provider_field(provider, DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD).get("options"),
            Some(&json!([
                { "label": "Off", "value": "off" },
                { "label": "Block overseas IPs", "value": "block_overseas" }
            ]))
        );
    }
    let en_esa = provider_by_name(&en_providers, "esa");
    assert_eq!(
        provider_field(en_esa, "proxied").get("options"),
        Some(&json!([
            { "label": "DNS only", "value": "false" },
            { "label": "Enable proxy", "value": "true" }
        ]))
    );
    assert_eq!(
        provider_field(en_esa, "biz_name").get("options"),
        Some(&json!([
            { "label": "Web", "value": "web" },
            { "label": "API", "value": "api" },
            { "label": "Audio/video", "value": "image_video" }
        ]))
    );
}

#[test]
fn provider_catalog_preserves_node_field_descriptions() {
    let described_fields: &[(&str, &[&str])] = &[
        (
            "alidns",
            &[
                "access_key_id",
                "access_key_secret",
                "root_domain",
                "domain",
                "line",
                "ttl",
            ],
        ),
        (
            "baiducloud",
            &[
                "access_key_id",
                "secret_access_key",
                "root_domain",
                "domain",
                "ttl",
            ],
        ),
        ("cloudflare", &["api_token", "zone_id", "domain", "proxied"]),
        (
            "dnspod",
            &[
                "token_id",
                "token_key",
                "root_domain",
                "domain",
                "record_line",
                "ttl",
            ],
        ),
        ("duckdns", &["domains", "token"]),
        ("dynu", &["api_key", "domain", "ttl", "group"]),
        ("dynv6", &["token", "zone", "ipv6prefix"]),
        (
            "edgeone_cname",
            &[
                "secret_id",
                "secret_key",
                "zone_id",
                "domain",
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "endpoint",
                "region",
            ],
        ),
        (
            "edgeone",
            &[
                "secret_id",
                "secret_key",
                "zone_id",
                "domain",
                "location",
                "ttl",
                DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD,
                "endpoint",
                "region",
            ],
        ),
        (
            "esa",
            &[
                "access_key_id",
                "access_key_secret",
                "site_name",
                "site_id",
                "domain",
                "proxied",
                "biz_name",
                "ttl",
            ],
        ),
        (
            "godaddy",
            &["api_key", "api_secret", "root_domain", "domain", "ttl"],
        ),
        (
            "huaweicloud",
            &[
                "access_key_id",
                "secret_access_key",
                "root_domain",
                "domain",
                "ttl",
            ],
        ),
        ("noip", &["hostname", "username", "password"]),
        (
            "porkbun",
            &["api_key", "secret_api_key", "root_domain", "domain", "ttl"],
        ),
        (
            "tencentcloud",
            &[
                "secret_id",
                "secret_key",
                "root_domain",
                "domain",
                "record_line",
                "record_line_id",
                "ttl",
            ],
        ),
    ];

    for locale in ["zh-CN", "en"] {
        let providers = provider_catalog(&Translator::new(locale));
        for &(provider_name, field_keys) in described_fields {
            let provider = provider_by_name(&providers, provider_name);
            for &key in field_keys {
                let field = provider_field(provider, key);
                assert!(
                    field
                        .get("description")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty()),
                    "{locale} {provider_name}.{key} missing description"
                );
            }
        }
    }
}

#[test]
fn provider_catalog_localizes_required_field_help() {
    for locale in ["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"] {
        let providers = provider_catalog(&Translator::new(locale));
        for provider in providers.as_array().unwrap() {
            let provider_name = provider.get("name").and_then(Value::as_str).unwrap();
            for field in provider.get("fields").and_then(Value::as_array).unwrap() {
                if field.get("required").and_then(Value::as_bool) == Some(false) {
                    continue;
                }
                let key = field.get("key").and_then(Value::as_str).unwrap();
                assert!(
                    field
                        .get("label")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty()),
                    "{locale} {provider_name}.{key} missing label"
                );
                assert!(
                    field
                        .get("description")
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.trim().is_empty()),
                    "{locale} {provider_name}.{key} missing description"
                );
            }
        }
    }
}

#[test]
fn provider_catalog_contains_all_node_providers() {
    let providers = provider_catalog(&Translator::new("zh-CN"));
    let names = providers
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|provider| provider.get("name").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert_eq!(names, provider_names());
    assert!(providers.as_array().unwrap().iter().all(|provider| {
        provider
            .get("fields")
            .and_then(Value::as_array)
            .is_some_and(|fields| !fields.is_empty())
    }));
    let cloudflare = provider_by_name(&providers, "cloudflare");
    let proxied = provider_field(cloudflare, "proxied");
    assert_eq!(proxied.get("label"), Some(&json!("Cloudflare 代理")));
    assert!(proxied.get("description").and_then(Value::as_str).is_some());
    assert_eq!(
        provider_field(cloudflare, "domain").get("label"),
        Some(&json!("域名"))
    );
    let alidns = provider_by_name(&providers, "alidns");
    assert_eq!(
        provider_field(alidns, "domain").get("description"),
        Some(&json!("要更新的完整主机名"))
    );
    assert_eq!(
        provider_field(alidns, "access_key_id").get("label"),
        Some(&json!("访问密钥 ID"))
    );

    let en_providers = provider_catalog(&Translator::new("en"));
    let dnspod = provider_by_name(&en_providers, "dnspod");
    assert_eq!(
        provider_field(dnspod, "record_line").get("placeholder"),
        Some(&json!("Default"))
    );
    assert_eq!(
        provider_field(dnspod, "token_id").get("description"),
        Some(&json!("API Token ID generated in the DNSPod console"))
    );

    let zh_tencentcloud = provider_by_name(&providers, "tencentcloud");
    assert_eq!(
        provider_field(zh_tencentcloud, "secret_id").get("label"),
        Some(&json!("SecretId（密钥 ID）"))
    );
    assert_eq!(
        provider_field(zh_tencentcloud, "secret_id").get("description"),
        Some(&json!(
            "腾讯云 API 访问密钥 SecretId，需具备对应 DNS 服务权限"
        ))
    );
    let tencentcloud = provider_by_name(&en_providers, "tencentcloud");
    assert_eq!(
        provider_field(tencentcloud, "record_line").get("placeholder"),
        Some(&json!("Default"))
    );
    let esa = provider_by_name(&en_providers, "esa");
    assert_eq!(
        provider_field(esa, "proxied").get("options"),
        Some(&json!([
            { "label": "DNS only", "value": "false" },
            { "label": "Enable proxy", "value": "true" }
        ]))
    );
    assert_eq!(
        provider_field(esa, "biz_name").get("options"),
        Some(&json!([
            { "label": "Web", "value": "web" },
            { "label": "API", "value": "api" },
            { "label": "Audio/video", "value": "image_video" }
        ]))
    );
}

#[test]
fn provider_updater_map_covers_catalog() {
    let providers = provider_catalog(&Translator::new("en"));
    for provider in providers.as_array().unwrap() {
        let name = provider.get("name").and_then(Value::as_str).unwrap();
        assert!(is_known_ddns_provider(name), "missing updater for {name}");
    }
}
