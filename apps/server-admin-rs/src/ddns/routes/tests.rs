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
