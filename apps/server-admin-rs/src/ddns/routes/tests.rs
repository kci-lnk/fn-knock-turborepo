use super::*;

async fn ddns_test_state() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

#[tokio::test]
async fn runtime_reset_preserves_interface_selection_anchor() {
    let (_directory, state) = ddns_test_state().await;
    let meta = DDNSTargetMeta {
        id: "selection-anchor-test".to_string(),
        name: "Selection anchor".to_string(),
        is_primary: false,
        enabled: true,
        provider: Some("cloudflare".to_string()),
        created_at: time_utils::now_iso(),
        updated_at: time_utils::now_iso(),
        sort_order: 1,
    };
    let last_ip = HashMap::from([("ipv6".to_string(), "2001:4860::20".to_string())]);
    state
        .storage
        .store
        .replace_hash_string_map(&target_last_ip_key(&meta.id), &last_ip)
        .await
        .unwrap();
    state
        .storage
        .store
        .replace_hash_string_map(&target_selection_anchor_key(&meta.id), &last_ip)
        .await
        .unwrap();
    state
        .storage
        .store
        .replace_hash_string_map(
            &target_interface_recovery_key(&meta.id),
            &HashMap::from([
                ("ipv6_address".to_string(), "2001:4860::10".to_string()),
                ("ipv6_confirmations".to_string(), "2".to_string()),
            ]),
        )
        .await
        .unwrap();

    reset_target_runtime_state(&state, &meta).await.unwrap();

    assert!(
        state
            .storage
            .store
            .hgetall_string_map(&target_last_ip_key(&meta.id))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&target_selection_anchor_key(&meta.id))
            .await
            .unwrap(),
        last_ip
    );
    assert!(
        state
            .storage
            .store
            .hgetall_string_map(&target_interface_recovery_key(&meta.id))
            .await
            .unwrap()
            .is_empty()
    );

    let target = DDNSTargetRecord {
        meta,
        config: HashMap::new(),
        last_ip: empty_last_ip(),
        selection_anchor: parse_last_ip(&last_ip),
        last_check: empty_last_check(),
    };
    set_target_last_ip(&state, &target, Some("8.8.8.8"), None)
        .await
        .unwrap();
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&target_selection_anchor_key(&target.meta.id))
            .await
            .unwrap()
            .get("ipv6")
            .map(String::as_str),
        Some("2001:4860::20")
    );
}

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
            let mut capabilities = provider.get("capabilities").cloned().unwrap_or(Value::Null);
            if let Some(object) = capabilities.as_object_mut() {
                object.remove("domainTargets");
                if object.is_empty() {
                    capabilities = Value::Null;
                }
            }
            json!({
                "name": provider.get("name").and_then(Value::as_str).unwrap(),
                "capabilities": capabilities,
                "fields": fields,
            })
        })
        .collect::<Vec<_>>();
    json!(items)
}

#[test]
fn parses_ddns_settings_with_defaults() {
    let value = parse_settings(Some(
        r#"{"updateIntervalMinutes":2,"httpTransport":"fetch","publicCheckSources":{"ipv4":["4.example.com","https://4.example.com"],"ipv6":["https://6.example.com"]}}"#,
    ));
    assert_eq!(value["updateIntervalMinutes"], json!(2));
    assert_eq!(value["httpTransport"], json!("node"));
    assert_eq!(value["publicDnsProvider"], json!("alidns"));
    assert_eq!(
        value["publicCheckSources"]["ipv4"],
        json!(["https://4.example.com"])
    );
}

#[test]
fn ddns_settings_http_transport_defaults_to_builtin_http() {
    assert_eq!(parse_settings(None)["httpTransport"], json!("node"));
    assert_eq!(
        parse_settings(Some(r#"{"httpTransport":"invalid"}"#))["httpTransport"],
        json!("node")
    );
    assert_eq!(
        parse_settings(Some(r#"{"httpTransport":"curl"}"#))["httpTransport"],
        json!("curl")
    );
}

#[test]
fn ddns_settings_public_dns_provider_defaults_and_normalizes() {
    for provider in ["none", "alidns", "tencent", "cloudflare", "google"] {
        let raw = json!({ "publicDnsProvider": provider }).to_string();
        assert_eq!(parse_settings(Some(&raw))["publicDnsProvider"], provider);
    }
    assert_eq!(
        parse_settings(Some(r#"{"publicDnsProvider":"invalid"}"#))["publicDnsProvider"],
        json!("alidns")
    );
    assert_eq!(
        merge_public_dns_provider_update(None, &json!({ "publicDnsProvider": "google" })),
        "google"
    );
    assert_eq!(
        merge_public_dns_provider_update(Some("invalid"), &json!({ "publicDnsProvider": "none" })),
        "alidns"
    );
}

#[test]
fn public_dns_catalog_and_curl_resolve_entries_are_stable() {
    assert_eq!(public_dns_server_addresses("alidns"), &PUBLIC_DNS_ALIDNS);
    assert_eq!(public_dns_server_addresses("tencent"), &PUBLIC_DNS_TENCENT);
    assert_eq!(
        public_dns_server_addresses("cloudflare"),
        &PUBLIC_DNS_CLOUDFLARE
    );
    assert_eq!(public_dns_server_addresses("google"), &PUBLIC_DNS_GOOGLE);
    assert!(public_dns_server_addresses("none").is_empty());
    assert_eq!(
        format_curl_resolve_entry("example.com", 443, "192.0.2.1".parse().unwrap()),
        "example.com:443:192.0.2.1"
    );
    assert_eq!(
        format_curl_resolve_entry("example.com", 443, "2001:db8::1".parse().unwrap()),
        "example.com:443:[2001:db8::1]"
    );
}

#[derive(Clone)]
struct FailingPublicDnsResolver;

impl reqwest::dns::Resolve for FailingPublicDnsResolver {
    fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        Box::pin(async {
            Err(Box::new(std::io::Error::other(
                "使用公共 DNS 解析 probe.example 的 IPv6 地址失败: no records found",
            )) as Box<dyn std::error::Error + Send + Sync>)
        })
    }
}

#[tokio::test]
async fn public_dns_reqwest_errors_surface_the_localized_root_cause() {
    let client = reqwest::Client::builder()
        .dns_resolver(FailingPublicDnsResolver)
        .no_proxy()
        .build()
        .unwrap();
    let error = client
        .get("http://probe.example/")
        .send()
        .await
        .unwrap_err();
    assert_eq!(
        deepest_error_message(&error),
        "使用公共 DNS 解析 probe.example 的 IPv6 地址失败: no records found"
    );
}

#[tokio::test]
async fn public_dns_curl_deadline_bounds_dns_work() {
    let translator = Translator::new("zh-CN");
    let deadline = tokio_time::Instant::now() + Duration::from_millis(10);
    let error = await_with_public_check_deadline(
        deadline,
        std::future::pending::<anyhow::Result<()>>(),
        &translator,
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        ddns_text(&translator, "publicCheckTimeout", &[])
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
        parse_settings(Some(r#"{"updateIntervalMinutes":2.0}"#))["updateIntervalMinutes"],
        json!(2)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"2.0"}"#))["updateIntervalMinutes"],
        json!(2)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":"2e0"}"#))["updateIntervalMinutes"],
        json!(2)
    );
    assert_eq!(
        parse_settings(Some(r#"{"updateIntervalMinutes":1}"#))["updateIntervalMinutes"],
        json!(10)
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
        "node"
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
        parse_legacy_ddns_cron_interval_minutes(Some("*/2 * * * *")),
        Some(2)
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/30 * * * *")),
        Some(30)
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("0 */15 * * * *")),
        Some(15)
    );
    assert_eq!(
        parse_legacy_ddns_cron_interval_minutes(Some("*/1 * * * *")),
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
        "none",
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
    assert!(!prepared.contains_key(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD));

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

    let interface_config = HashMap::from([
        (DDNS_IP_SOURCE_FIELD.to_string(), "interface".to_string()),
        (
            DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD.to_string(),
            "TRUE".to_string(),
        ),
    ]);
    let prepared = prepare_config_for_storage(
        Some("cloudflare"),
        normalize_config_map(Some("cloudflare"), &interface_config),
    );
    assert_eq!(
        prepared
            .get(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD)
            .map(String::as_str),
        Some("true")
    );

    let static_with_private_flag = HashMap::from([
        (DDNS_IP_SOURCE_FIELD.to_string(), "static".to_string()),
        (
            DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD.to_string(),
            "true".to_string(),
        ),
    ]);
    let prepared = prepare_config_for_storage(
        Some("cloudflare"),
        normalize_config_map(Some("cloudflare"), &static_with_private_flag),
    );
    assert!(!prepared.contains_key(DDNS_ALLOW_PRIVATE_ADDRESSES_FIELD));
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
        selection_anchor: empty_last_ip(),
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
        selection_anchor: empty_last_ip(),
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
        selection_anchor: empty_last_ip(),
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
        "20014860000000000000000000000001 02 40 00 00 eth0\nfe800000000000000000000000000001 02 40 20 00 eth1",
    );
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], json!("docker-host:eth0"));
    assert_eq!(items[0]["hasIpv6"], json!(true));
    assert_eq!(items[0]["addresses"][0]["address"], json!("2001:4860::1"));
    assert_eq!(items[0]["addresses"][0]["temporary"], json!(false));
    assert_eq!(items[0]["addresses"][0]["prefixLength"], json!(64));
}

#[test]
fn parses_linux_ipv6_address_flags() {
    let metadata = parse_if_inet6_metadata("20010db8000000000000000000000001 02 40 00 69 eth0");
    let status = metadata
        .get(&("eth0".to_string(), "2001:db8::1".to_string()))
        .unwrap();
    assert_eq!(status["temporary"], json!(true));
    assert_eq!(status["dadFailed"], json!(true));
    assert_eq!(status["deprecated"], json!(true));
    assert_eq!(status["tentative"], json!(true));
}

fn selector_network(addresses: Vec<Value>) -> Value {
    json!({ "selectableAddresses": addresses })
}

fn ipv6_candidate(address: &str, temporary: Value) -> Value {
    json!({
        "family": "ipv6",
        "address": address,
        "temporary": temporary,
        "deprecated": false,
        "tentative": false,
        "dadFailed": false
    })
}

#[test]
fn interface_selector_is_stable_across_candidate_order() {
    let selector = InterfaceAddressSelector::default();
    let first = selector_network(vec![
        ipv6_candidate("2001:db8::20", json!(false)),
        ipv6_candidate("2001:db8::10", json!(false)),
    ]);
    let reversed = selector_network(vec![
        ipv6_candidate("2001:db8::10", json!(false)),
        ipv6_candidate("2001:db8::20", json!(false)),
    ]);
    assert_eq!(
        resolve_interface_selector(&first, "ipv6", &selector, None).selected,
        Some("2001:db8::10".to_string())
    );
    assert_eq!(
        resolve_interface_selector(&reversed, "ipv6", &selector, None).selected,
        Some("2001:db8::10".to_string())
    );
}

#[test]
fn interface_selector_keeps_current_address_before_ranking() {
    let network = selector_network(vec![
        ipv6_candidate("2001:db8::10", json!(false)),
        ipv6_candidate("2001:db8::20", json!(false)),
    ]);
    let selection = resolve_interface_selector(
        &network,
        "ipv6",
        &InterfaceAddressSelector::default(),
        Some("2001:db8::20"),
    );
    assert_eq!(selection.selected.as_deref(), Some("2001:db8::20"));
    assert_eq!(selection.reason, "current");
}

#[test]
fn interface_selector_private_policy_is_opt_in_and_public_first() {
    let public = json!({
        "family": "ipv4",
        "address": "8.8.8.8",
        "temporary": false,
        "deprecated": false,
        "tentative": false,
        "dadFailed": false
    });
    let private = json!({
        "family": "ipv4",
        "address": "10.0.0.8",
        "temporary": false,
        "deprecated": false,
        "tentative": false,
        "dadFailed": false
    });
    let network = json!({
        "selectableAddresses": [public],
        "privateAddresses": [private]
    });

    let disabled = resolve_interface_selector_with_policy(
        &network,
        "ipv4",
        &InterfaceAddressSelector::default(),
        None,
        false,
    );
    assert_eq!(disabled.selected.as_deref(), Some("8.8.8.8"));
    assert_eq!(disabled.eligible.len(), 1);

    let enabled = resolve_interface_selector_with_policy(
        &network,
        "ipv4",
        &InterfaceAddressSelector::default(),
        None,
        true,
    );
    assert_eq!(enabled.selected.as_deref(), Some("8.8.8.8"));
    assert_eq!(enabled.eligible.len(), 2);

    let current_private = resolve_interface_selector_with_policy(
        &network,
        "ipv4",
        &InterfaceAddressSelector::default(),
        Some("10.0.0.8"),
        true,
    );
    assert_eq!(current_private.selected.as_deref(), Some("10.0.0.8"));
    assert_eq!(current_private.reason, "current");

    let filtered_current_private = resolve_interface_selector_with_policy(
        &network,
        "ipv4",
        &InterfaceAddressSelector::default(),
        Some("10.0.0.8"),
        false,
    );
    assert_eq!(
        filtered_current_private.selected.as_deref(),
        Some("8.8.8.8")
    );
    assert_eq!(filtered_current_private.reason, "ranked");

    let selector = InterfaceAddressSelector {
        preferred_address: Some("10.0.0.8".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let preferred = resolve_interface_selector_with_policy(&network, "ipv4", &selector, None, true);
    assert_eq!(preferred.selected.as_deref(), Some("10.0.0.8"));
    assert_eq!(preferred.reason, "preferred");

    let private_only = json!({
        "selectableAddresses": [],
        "privateAddresses": [network["privateAddresses"][0].clone()]
    });
    assert_eq!(
        resolve_interface_selector_with_policy(
            &private_only,
            "ipv4",
            &InterfaceAddressSelector::default(),
            None,
            false,
        )
        .selected,
        None
    );
    assert_eq!(
        resolve_interface_selector_with_policy(
            &private_only,
            "ipv4",
            &InterfaceAddressSelector::default(),
            None,
            true,
        )
        .selected
        .as_deref(),
        Some("10.0.0.8")
    );
}

#[test]
fn interface_selector_prefers_manual_address_before_current() {
    let network = selector_network(vec![
        ipv6_candidate("2001:db8::10", json!(false)),
        ipv6_candidate("2001:db8::20", json!(false)),
    ]);
    let selector = InterfaceAddressSelector {
        preferred_address: Some("2001:db8::10".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let selection = resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8::20"));
    assert_eq!(selection.selected.as_deref(), Some("2001:db8::10"));
    assert_eq!(selection.reason, "preferred");
}

#[test]
fn preferred_address_recovery_waits_for_consecutive_confirmations() {
    let network = selector_network(vec![
        ipv6_candidate("2001:db8::10", json!(false)),
        ipv6_candidate("2001:db8::20", json!(false)),
    ]);
    let selector = InterfaceAddressSelector {
        preferred_address: Some("2001:db8::10".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let selection = resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8::20"));

    let first = stabilize_preferred_recovery(&selection, &selector, Some("2001:db8::20"), None, 3);
    assert_eq!(first.selected.as_deref(), Some("2001:db8::20"));
    assert!(first.deferred);
    assert_eq!(first.state.as_ref().unwrap().confirmations, 1);

    let second = stabilize_preferred_recovery(
        &selection,
        &selector,
        Some("2001:db8::20"),
        first.state.as_ref(),
        3,
    );
    assert_eq!(second.selected.as_deref(), Some("2001:db8::20"));
    assert!(second.deferred);
    assert_eq!(second.state.as_ref().unwrap().confirmations, 2);

    let third = stabilize_preferred_recovery(
        &selection,
        &selector,
        Some("2001:db8::20"),
        second.state.as_ref(),
        3,
    );
    assert_eq!(third.selected.as_deref(), Some("2001:db8::10"));
    assert!(!third.deferred);
    assert_eq!(third.state.as_ref().unwrap().confirmations, 3);
}

#[test]
fn preferred_address_recovery_is_immediate_when_fallback_is_unavailable() {
    let network = selector_network(vec![ipv6_candidate("2001:db8::10", json!(false))]);
    let selector = InterfaceAddressSelector {
        preferred_address: Some("2001:db8::10".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let selection = resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8::20"));
    let decision =
        stabilize_preferred_recovery(&selection, &selector, Some("2001:db8::20"), None, 3);

    assert_eq!(decision.selected.as_deref(), Some("2001:db8::10"));
    assert!(!decision.deferred);
    assert!(decision.state.is_none());
}

#[test]
fn preferred_address_loss_fails_over_without_recovery_delay() {
    let network = selector_network(vec![ipv6_candidate("2001:db8::20", json!(false))]);
    let selector = InterfaceAddressSelector {
        preferred_address: Some("2001:db8::10".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let selection = resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8::10"));
    let previous_state = PreferredRecoveryState {
        address: "2001:db8::10".to_string(),
        confirmations: 2,
    };
    let decision = stabilize_preferred_recovery(
        &selection,
        &selector,
        Some("2001:db8::10"),
        Some(&previous_state),
        3,
    );

    assert_eq!(decision.selected.as_deref(), Some("2001:db8::20"));
    assert!(!decision.deferred);
    assert!(decision.state.is_none());
}

#[tokio::test]
async fn automatic_preferred_recovery_confirmations_persist_across_checks() {
    let (_directory, state) = ddns_test_state().await;
    let selector = InterfaceAddressSelector {
        preferred_address: Some("2001:db8::10".to_string()),
        ..InterfaceAddressSelector::default()
    };
    let network = selector_network(vec![
        ipv6_candidate("2001:db8::10", json!(false)),
        ipv6_candidate("2001:db8::20", json!(false)),
    ]);
    let selection = resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8::20"));
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "preferred-recovery-persistence".to_string(),
            name: "Preferred recovery".to_string(),
            is_primary: false,
            enabled: true,
            provider: Some("cloudflare".to_string()),
            created_at: time_utils::now_iso(),
            updated_at: time_utils::now_iso(),
            sort_order: 1,
        },
        config: HashMap::from([
            (DDNS_IP_SOURCE_FIELD.to_string(), "interface".to_string()),
            (
                DDNS_NETWORK_INTERFACE_FIELD.to_string(),
                "test0".to_string(),
            ),
        ]),
        last_ip: json!({
            "ipv4": null,
            "ipv6": "2001:db8::20",
            "updated_at": time_utils::now_iso()
        }),
        selection_anchor: json!({
            "ipv4": null,
            "ipv6": "2001:db8::20",
            "updated_at": time_utils::now_iso()
        }),
        last_check: empty_last_check(),
    };

    for (index, expected) in ["2001:db8::20", "2001:db8::20", "2001:db8::10"]
        .into_iter()
        .enumerate()
    {
        let resolution = InterfaceAddressResolution {
            address: selection.selected.clone(),
            selection_logs: Vec::new(),
            selection: Some(selection.clone()),
            selector: Some(selector.clone()),
            mode: "auto".to_string(),
        };
        let mut ips = ResolvedTargetIps {
            ipv4: None,
            ipv6: selection.selected.clone(),
            source: "interface",
            source_label: "test0".to_string(),
            warnings: Vec::new(),
            selection_logs: Vec::new(),
            interface_resolutions: HashMap::from([("ipv6".to_string(), resolution)]),
            update_scope: "ipv6_only",
        };

        stabilize_automatic_interface_ips(&state, &target, &mut ips, &Translator::new("zh-CN"))
            .await
            .unwrap();

        assert_eq!(ips.ipv6.as_deref(), Some(expected), "check {}", index + 1);
    }

    let recovery = state
        .storage
        .store
        .hgetall_string_map(&target_interface_recovery_key(&target.meta.id))
        .await
        .unwrap();
    assert_eq!(
        recovery.get("ipv6_confirmations").map(String::as_str),
        Some("3")
    );

    let mut reset_target = target.clone();
    reset_target.meta.id = "preferred-recovery-after-config-change".to_string();
    reset_target.last_ip = empty_last_ip();
    let resolution = InterfaceAddressResolution {
        address: selection.selected.clone(),
        selection_logs: Vec::new(),
        selection: Some(selection.clone()),
        selector: Some(selector),
        mode: "auto".to_string(),
    };
    let mut ips = ResolvedTargetIps {
        ipv4: None,
        ipv6: selection.selected.clone(),
        source: "interface",
        source_label: "test0".to_string(),
        warnings: Vec::new(),
        selection_logs: Vec::new(),
        interface_resolutions: HashMap::from([("ipv6".to_string(), resolution)]),
        update_scope: "ipv6_only",
    };

    stabilize_automatic_interface_ips(&state, &reset_target, &mut ips, &Translator::new("zh-CN"))
        .await
        .unwrap();

    assert_eq!(ips.ipv6.as_deref(), Some("2001:db8::10"));
    assert!(
        state
            .storage
            .store
            .hgetall_string_map(&target_interface_recovery_key(&reset_target.meta.id))
            .await
            .unwrap()
            .is_empty()
    );
}

#[test]
fn interface_selector_follows_rotating_prefix_by_interface_id() {
    let network = selector_network(vec![
        ipv6_candidate("2001:db8:2::1234", json!(false)),
        ipv6_candidate("2001:db8:2::9999", json!(false)),
    ]);
    let selector = normalize_interface_selector(
        InterfaceAddressSelector {
            mode: InterfaceSelectorMode::Rules,
            ipv6_interface_id: Some("0000:0000:0000:1234".to_string()),
            ..InterfaceAddressSelector::default()
        },
        "ipv6",
    )
    .unwrap();
    assert_eq!(
        resolve_interface_selector(&network, "ipv6", &selector, Some("2001:db8:1::1234"))
            .selected
            .as_deref(),
        Some("2001:db8:2::1234")
    );
}

#[test]
fn interface_selector_applies_status_and_cidr_rules() {
    let mut deprecated = ipv6_candidate("2001:db8:1::1", json!(false));
    deprecated["deprecated"] = json!(true);
    let mut tentative = ipv6_candidate("2001:db8:1::5", json!(false));
    tentative["tentative"] = json!(true);
    let mut dad_failed = ipv6_candidate("2001:db8:1::6", json!(false));
    dad_failed["dadFailed"] = json!(true);
    let network = selector_network(vec![
        deprecated,
        tentative,
        dad_failed,
        ipv6_candidate("2001:db8:1::2", json!(true)),
        ipv6_candidate("2001:db8:2::3", Value::Null),
        ipv6_candidate("2001:db8:1::4", json!(false)),
    ]);
    let selector = normalize_interface_selector(
        InterfaceAddressSelector {
            mode: InterfaceSelectorMode::Rules,
            include_cidrs: vec!["2001:db8::/32".to_string()],
            exclude_cidrs: vec!["2001:db8:2::/48".to_string()],
            ..InterfaceAddressSelector::default()
        },
        "ipv6",
    )
    .unwrap();
    let selection = resolve_interface_selector(&network, "ipv6", &selector, None);
    assert_eq!(selection.selected.as_deref(), Some("2001:db8:1::4"));
    assert_eq!(selection.eligible.len(), 1);
    assert_eq!(selection.rejected.len(), 5);
}

#[test]
fn interface_selector_allows_unknown_status_as_a_fallback() {
    let network = selector_network(vec![ipv6_candidate("2001:db8::1", Value::Null)]);
    let selection =
        resolve_interface_selector(&network, "ipv6", &InterfaceAddressSelector::default(), None);
    assert_eq!(selection.selected.as_deref(), Some("2001:db8::1"));
}

#[test]
fn interface_selector_can_allow_temporary_addresses() {
    let network = selector_network(vec![ipv6_candidate("2001:db8::1", json!(true))]);
    assert!(
        resolve_interface_selector(&network, "ipv6", &InterfaceAddressSelector::default(), None)
            .selected
            .is_none()
    );
    let selector = InterfaceAddressSelector {
        allow_temporary: true,
        ..InterfaceAddressSelector::default()
    };
    assert_eq!(
        resolve_interface_selector(&network, "ipv6", &selector, None)
            .selected
            .as_deref(),
        Some("2001:db8::1")
    );
}

#[test]
fn interface_selection_returns_an_error_when_rules_have_no_candidate() {
    let directory = tempfile::tempdir().unwrap();
    let proc_path = directory.path().join("if_inet6");
    fs::write(
        &proc_path,
        "20014860000000000000000000000001 02 40 00 01 test0\n",
    )
    .unwrap();
    let environment = crate::test_support::EnvGuard::new(&["DDNS_HOST_IF_INET6_PATH"]);
    environment.set("DDNS_HOST_IF_INET6_PATH", &proc_path);
    let selector = serde_json::to_string(&InterfaceAddressSelector::default()).unwrap();

    let error = select_interface_address(
        "docker-host:test0",
        "ipv6",
        Some(&selector),
        None,
        None,
        &Translator::new("zh-CN"),
    )
    .unwrap_err();

    let message = error.to_string();
    assert!(message.contains("未匹配"));
    assert!(message.contains("IPv6"));
}

#[test]
fn interface_selection_uses_implicit_auto_when_no_selection_is_stored() {
    let directory = tempfile::tempdir().unwrap();
    let proc_path = directory.path().join("if_inet6");
    fs::write(
        &proc_path,
        concat!(
            "24098a745c903730020c29fffe0f5c24 02 40 00 00 test0\n",
            "24098a745c903730020c29fffe0f5c10 02 40 00 00 test0\n"
        ),
    )
    .unwrap();
    let environment = crate::test_support::EnvGuard::new(&["DDNS_HOST_IF_INET6_PATH"]);
    environment.set("DDNS_HOST_IF_INET6_PATH", &proc_path);

    let (selected, warnings, selection_logs) = select_interface_address(
        "docker-host:test0",
        "ipv6",
        None,
        None,
        None,
        &Translator::new("zh-CN"),
    )
    .unwrap();

    assert_eq!(
        selected.as_deref(),
        Some("2409:8a74:5c90:3730:20c:29ff:fe0f:5c10")
    );
    assert!(warnings.is_empty());
    assert!(selection_logs.is_empty());
}

#[test]
fn stable_multi_candidate_selection_does_not_emit_a_warning() {
    let directory = tempfile::tempdir().unwrap();
    let proc_path = directory.path().join("if_inet6");
    fs::write(
        &proc_path,
        concat!(
            "24098a745c903730020c29fffe0f5c24 02 40 00 00 test0\n",
            "24098a745c903730020c29fffe0f5c10 02 40 00 00 test0\n"
        ),
    )
    .unwrap();
    let environment = crate::test_support::EnvGuard::new(&["DDNS_HOST_IF_INET6_PATH"]);
    environment.set("DDNS_HOST_IF_INET6_PATH", &proc_path);
    let selector = serde_json::to_string(&InterfaceAddressSelector::default()).unwrap();

    let (selected, warnings, selection_logs) = select_interface_address(
        "docker-host:test0",
        "ipv6",
        Some(&selector),
        None,
        Some("2409:8a74:5c90:3730:20c:29ff:fe0f:5c24"),
        &Translator::new("zh-CN"),
    )
    .unwrap();

    assert_eq!(
        selected.as_deref(),
        Some("2409:8a74:5c90:3730:20c:29ff:fe0f:5c24")
    );
    assert!(warnings.is_empty());
    assert!(selection_logs.is_empty());
}

#[test]
fn address_selection_logs_only_a_forced_switch() {
    let directory = tempfile::tempdir().unwrap();
    let proc_path = directory.path().join("if_inet6");
    fs::write(
        &proc_path,
        concat!(
            "24098a745c903730020c29fffe0f5c24 02 40 00 00 test0\n",
            "24098a745c903730020c29fffe0f5c10 02 40 00 00 test0\n"
        ),
    )
    .unwrap();
    let environment = crate::test_support::EnvGuard::new(&["DDNS_HOST_IF_INET6_PATH"]);
    environment.set("DDNS_HOST_IF_INET6_PATH", &proc_path);
    let selector = serde_json::to_string(&InterfaceAddressSelector::default()).unwrap();

    let (selected, warnings, selection_logs) = select_interface_address(
        "docker-host:test0",
        "ipv6",
        Some(&selector),
        None,
        Some("2409:8a74:ffff:3730:20c:29ff:fe0f:5c24"),
        &Translator::new("zh-CN"),
    )
    .unwrap();

    assert_eq!(
        selected.as_deref(),
        Some("2409:8a74:5c90:3730:20c:29ff:fe0f:5c10")
    );
    assert!(warnings.is_empty());
    assert_eq!(selection_logs.len(), 1);
    assert!(selection_logs[0].contains("2409:8a74:5c90:3730:20c:29ff:fe0f:5c10"));
}

#[test]
fn legacy_selector_prefers_current_and_interface_id_before_index() {
    let candidates = vec![
        ipv6_candidate("2001:db8:2::1234", json!(false)),
        ipv6_candidate("2001:db8:2::9999", json!(false)),
    ];
    assert_eq!(
        legacy_select_interface_address(&candidates, "ipv6", Some("1"), Some("2001:db8:1::1234")),
        Some(("2001:db8:2::1234".to_string(), "legacy_interface_id"))
    );
}

#[test]
fn legacy_index_keeps_original_position_when_an_earlier_address_is_deprecated() {
    let mut deprecated = ipv6_candidate("2001:db8::1", json!(false));
    deprecated["deprecated"] = json!(true);
    let candidates = vec![
        deprecated,
        ipv6_candidate("2001:db8::2", json!(false)),
        ipv6_candidate("2001:db8::3", json!(false)),
    ];
    assert_eq!(
        legacy_select_interface_address(&candidates, "ipv6", Some("1"), None),
        Some(("2001:db8::2".to_string(), "legacy_index"))
    );
    assert_eq!(
        legacy_select_interface_address(&candidates, "ipv6", Some("0"), None),
        None
    );
}

#[test]
fn interface_selector_validation_rejects_family_mismatch() {
    let raw =
        r#"{"version":1,"mode":"rules","includeCidrs":["192.0.2.0/24"],"allowTemporary":false}"#;
    assert!(parse_interface_selector(Some(raw), "ipv6").is_err());
}

#[test]
fn stored_selector_replaces_legacy_index_for_the_same_family() {
    let selector = serde_json::to_string(&InterfaceAddressSelector::default()).unwrap();
    let prepared = prepare_config_for_storage(
        Some("cloudflare"),
        HashMap::from([
            (DDNS_IP_SOURCE_FIELD.to_string(), "interface".to_string()),
            (DDNS_INTERFACE_IPV6_INDEX_FIELD.to_string(), "1".to_string()),
            (
                DDNS_INTERFACE_IPV6_SELECTOR_FIELD.to_string(),
                selector.clone(),
            ),
        ]),
    );
    assert_eq!(
        prepared
            .get(DDNS_INTERFACE_IPV6_SELECTOR_FIELD)
            .map(String::as_str),
        Some(selector.as_str())
    );
    assert!(!prepared.contains_key(DDNS_INTERFACE_IPV6_INDEX_FIELD));
}

#[test]
fn selector_no_match_is_runtime_unavailable_not_incomplete_config() {
    let now = time_utils::now_iso();
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "target-selector".to_string(),
            name: "Selector".to_string(),
            is_primary: false,
            enabled: true,
            provider: Some("cloudflare".to_string()),
            created_at: now.clone(),
            updated_at: now,
            sort_order: 1,
        },
        config: HashMap::from([(
            DDNS_INTERFACE_IPV6_SELECTOR_FIELD.to_string(),
            serde_json::to_string(&InterfaceAddressSelector::default()).unwrap(),
        )]),
        last_ip: empty_last_ip(),
        selection_anchor: empty_last_ip(),
        last_check: empty_last_check(),
    };
    let network = selector_network(vec![ipv6_candidate("2001:db8::1", json!(true))]);
    assert!(
        selected_interface_address_incomplete_reason(
            &target,
            &network,
            "ipv6",
            &Translator::new("zh-CN")
        )
        .is_none()
    );
}

#[test]
fn implicit_auto_selection_is_complete_when_a_stable_candidate_exists() {
    let now = time_utils::now_iso();
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "target-implicit-auto".to_string(),
            name: "Implicit auto".to_string(),
            is_primary: false,
            enabled: true,
            provider: Some("cloudflare".to_string()),
            created_at: now.clone(),
            updated_at: now,
            sort_order: 1,
        },
        config: HashMap::new(),
        last_ip: empty_last_ip(),
        selection_anchor: empty_last_ip(),
        last_check: empty_last_check(),
    };
    let network = selector_network(vec![ipv6_candidate("2001:db8::1", json!(false))]);

    assert!(
        selected_interface_address_incomplete_reason(
            &target,
            &network,
            "ipv6",
            &Translator::new("zh-CN")
        )
        .is_none()
    );
}

#[test]
fn interface_runtime_validation_allows_immediate_refresh_with_implicit_auto() {
    let directory = tempfile::tempdir().unwrap();
    let proc_path = directory.path().join("if_inet6");
    fs::write(
        &proc_path,
        "24098a745c903730020c29fffe0f5c24 02 40 00 00 test0\n",
    )
    .unwrap();
    let environment = crate::test_support::EnvGuard::new(&["DDNS_HOST_IF_INET6_PATH"]);
    environment.set("DDNS_HOST_IF_INET6_PATH", &proc_path);
    let now = time_utils::now_iso();
    let target = DDNSTargetRecord {
        meta: DDNSTargetMeta {
            id: "target-immediate-refresh".to_string(),
            name: "Immediate refresh".to_string(),
            is_primary: true,
            enabled: true,
            provider: Some("cloudflare".to_string()),
            created_at: now.clone(),
            updated_at: now,
            sort_order: 0,
        },
        config: HashMap::from([(
            DDNS_NETWORK_INTERFACE_FIELD.to_string(),
            "docker-host:test0".to_string(),
        )]),
        last_ip: empty_last_ip(),
        selection_anchor: empty_last_ip(),
        last_check: empty_last_check(),
    };

    assert!(
        interface_config_incomplete_reason(&target, "ipv6_only", &Translator::new("zh-CN"))
            .is_none()
    );
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
    for address in ["100.64.0.1", "192.0.2.1", "198.18.0.1", "224.0.0.1"] {
        assert!(!is_selectable_interface_address(&json!({
            "family": "ipv4",
            "address": address
        })));
    }
    for address in ["2001:db8::1", "ff02::1"] {
        assert!(!is_selectable_interface_address(&json!({
            "family": "ipv6",
            "address": address
        })));
    }
    assert!(is_selectable_interface_address(&json!({
        "family": "ipv6",
        "address": "2001:4860::1"
    })));
    for address in [
        "10.0.0.1",
        "10.255.255.254",
        "172.16.0.1",
        "172.31.255.254",
        "192.168.0.1",
        "192.168.255.254",
    ] {
        assert!(is_private_interface_address(&json!({
            "family": "ipv4",
            "address": address
        })));
    }
    for address in ["fc00::1", "fdff:ffff:ffff:ffff:ffff:ffff:ffff:fffe"] {
        assert!(is_private_interface_address(&json!({
            "family": "ipv6",
            "address": address
        })));
    }
    for address in [
        "9.255.255.255",
        "11.0.0.0",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.1.1",
        "172.15.255.255",
        "172.32.0.0",
        "192.0.2.1",
        "192.167.255.255",
        "192.169.0.0",
        "224.0.0.1",
        "240.0.0.1",
    ] {
        assert!(!is_private_interface_address(&json!({
            "family": "ipv4",
            "address": address
        })));
    }
    for address in [
        "::1",
        "fbff:ffff:ffff:ffff:ffff:ffff:ffff:ffff",
        "fe00::1",
        "fe80::1",
        "2001:db8::1",
        "ff02::1",
    ] {
        assert!(!is_private_interface_address(&json!({
            "family": "ipv6",
            "address": address
        })));
    }
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
    assert_eq!(item["privateAddresses"].as_array().unwrap().len(), 1);
    let docker_private = interface_option(
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
    .unwrap();
    assert_eq!(
        docker_private["privateAddresses"][0]["address"],
        json!("fd00::1")
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
fn cloud_provider_timestamps_omit_fractional_seconds() {
    for (input, expected) in [
        ("2026-07-15T08:09:10.123Z", "2026-07-15T08:09:10Z"),
        ("2026-07-15T08:09:10.123456789Z", "2026-07-15T08:09:10Z"),
        ("2026-07-15T08:09:10Z", "2026-07-15T08:09:10Z"),
    ] {
        assert_eq!(strip_fractional_seconds(input), expected);
    }

    let assert_iso8601 = |timestamp: &str| {
        assert_eq!(timestamp.len(), 20);
        assert_eq!(timestamp.as_bytes()[10], b'T');
        assert_eq!(timestamp.as_bytes()[19], b'Z');
        assert!(!timestamp.contains('.'));
    };
    let assert_compact = |timestamp: &str| {
        assert_eq!(timestamp.len(), 16);
        assert_eq!(timestamp.as_bytes()[8], b'T');
        assert_eq!(timestamp.as_bytes()[15], b'Z');
        assert!(!timestamp.contains(['-', ':', '.']));
    };

    // ESA signs this shared ISO8601 value directly as x-acs-date.
    assert_iso8601(&iso8601_utc_without_millis());

    let alidns_query = build_aliyun_signed_params("access-key", "secret", Vec::new(), "GET");
    let alidns_timestamp = url::form_urlencoded::parse(alidns_query.as_bytes())
        .find_map(|(key, value)| (key == "Timestamp").then(|| value.into_owned()))
        .unwrap();
    assert_iso8601(&alidns_timestamp);

    let (baidu_timestamp, _) = baidu_bce_authorization(
        "GET",
        "https://bcd.baidubce.com/v1/domain",
        "access-key",
        "secret",
    )
    .unwrap();
    assert_iso8601(&baidu_timestamp);

    let (huawei_timestamp, _) = huawei_sdk_authorization(
        "GET",
        "https://dns.cn-north-4.myhuaweicloud.com/v2/zones",
        "application/json",
        "access-key",
        "secret",
        "",
    )
    .unwrap();
    assert_compact(&huawei_timestamp);
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
fn dnshe_subdomain_lookup_matches_exact_normalized_domains_and_ids() {
    let response = json!({
        "success": true,
        "subdomains": [
            {
                "id": 41,
                "full_domain": "other.example.com",
                "status": "active"
            },
            {
                "id": "42",
                "full_domain": "Managed.Example.COM.",
                "status": "active"
            },
            {
                "id": 43,
                "full_domain": "inactive.example.com",
                "status": "suspended"
            }
        ]
    });
    assert_eq!(
        find_dnshe_subdomain(&response, "managed.example.com"),
        Some(DnsheSubdomainMatch {
            id: 42,
            status: "active".to_string(),
        })
    );
    assert_eq!(
        find_dnshe_subdomain(&response, "INACTIVE.EXAMPLE.COM."),
        Some(DnsheSubdomainMatch {
            id: 43,
            status: "suspended".to_string(),
        })
    );
    assert_eq!(find_dnshe_subdomain(&response, "example.com"), None);
}

#[test]
fn dnshe_subdomain_status_accepts_documented_and_live_usable_values() {
    for status in ["active", "ACTIVE", "Registered", " registered "] {
        assert!(dnshe_subdomain_is_usable(status), "status={status}");
    }
    for status in ["", "suspended", "expired", "pending", "unknown"] {
        assert!(!dnshe_subdomain_is_usable(status), "status={status}");
    }
}

#[test]
fn dnshe_create_record_names_are_relative_to_the_managed_domain() {
    let translator = Translator::new("en");
    for (fqdn, expected) in [
        ("managed.example.com", "@"),
        ("host.managed.example.com", "host"),
        ("nested.host.managed.example.com", "nested.host"),
        ("*.managed.example.com", "*"),
    ] {
        let domain = split_domain(&translator, fqdn, "managed.example.com").unwrap();
        assert_eq!(dnshe_create_record_name(&domain), expected);
    }
}

#[test]
fn dnshe_subdomain_pagination_uses_explicit_metadata_and_full_pages() {
    assert!(dnshe_has_more_subdomains(
        &json!({ "pagination": { "has_more": true } }),
        1
    ));
    assert!(!dnshe_has_more_subdomains(
        &json!({ "pagination": { "has_more": false } }),
        500
    ));
    assert!(dnshe_has_more_subdomains(&json!({}), 500));
    assert!(!dnshe_has_more_subdomains(&json!({}), 499));
}

#[test]
fn dnshe_subdomain_pagination_rejects_no_progress_and_excessive_pages() {
    assert_eq!(
        dnshe_next_subdomain_page(1, &json!({ "pagination": { "has_more": false } }), 500).unwrap(),
        None
    );
    assert_eq!(
        dnshe_next_subdomain_page(
            DNSHE_MAX_SUBDOMAIN_PAGES - 1,
            &json!({ "pagination": { "has_more": true } }),
            1
        )
        .unwrap(),
        Some(DNSHE_MAX_SUBDOMAIN_PAGES)
    );
    assert!(
        dnshe_next_subdomain_page(1, &json!({ "pagination": { "has_more": true } }), 0)
            .unwrap_err()
            .to_string()
            .contains("without returning any items")
    );
    assert!(
        dnshe_next_subdomain_page(
            DNSHE_MAX_SUBDOMAIN_PAGES,
            &json!({ "pagination": { "has_more": true } }),
            1
        )
        .unwrap_err()
        .to_string()
        .contains("page limit")
    );
}

#[tokio::test]
async fn ddns_curl_body_reader_rejects_oversized_files() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("oversized-response.bin");
    let file = tokio::fs::File::create(&path).await.unwrap();
    file.set_len(MAX_DDNS_PROVIDER_RESPONSE_BYTES as u64 + 1)
        .await
        .unwrap();

    let error = read_ddns_curl_body(&path).await.unwrap_err();
    assert!(error.to_string().contains("response body exceeds"));
}

#[test]
fn dnshe_record_lookup_normalizes_apex_relative_and_wildcard_names() {
    let response = json!({
        "success": true,
        "records": [
            {
                "id": 10,
                "name": "@",
                "type": "A",
                "content": "192.0.2.10"
            },
            {
                "id": "11",
                "name": "www",
                "type": "A",
                "content": "192.0.2.11"
            },
            {
                "id": 12,
                "name": "*.managed.example.com.",
                "type": "AAAA",
                "content": "2001:db8::12"
            },
            {
                "id": 13,
                "name": "managed.example.com",
                "type": "A",
                "content": "192.0.2.13"
            }
        ]
    });

    assert_eq!(
        find_dnshe_record(&response, "managed.example.com", "managed.example.com", "A"),
        DnsheRecordLookup::Found(DnsheRecordMatch {
            id: 10,
            content: "192.0.2.10".to_string(),
        })
    );
    assert_eq!(
        find_dnshe_record(
            &response,
            "www.managed.example.com",
            "managed.example.com",
            "a"
        ),
        DnsheRecordLookup::Found(DnsheRecordMatch {
            id: 11,
            content: "192.0.2.11".to_string(),
        })
    );
    assert_eq!(
        find_dnshe_record(
            &response,
            "*.managed.example.com",
            "managed.example.com",
            "AAAA"
        ),
        DnsheRecordLookup::Found(DnsheRecordMatch {
            id: 12,
            content: "2001:db8::12".to_string(),
        })
    );
    assert_eq!(
        find_dnshe_record(
            &response,
            "missing.managed.example.com",
            "managed.example.com",
            "A"
        ),
        DnsheRecordLookup::Missing
    );
}

#[test]
fn dnshe_record_update_plan_distinguishes_noop_update_create_and_missing_id() {
    let existing = json!({
        "records": [{
            "id": "21",
            "name": "host.managed.example.com",
            "type": "A",
            "content": "192.0.2.21"
        }]
    });
    assert_eq!(
        plan_dnshe_record_update(
            &existing,
            "host.managed.example.com",
            "managed.example.com",
            "A",
            "192.0.2.21"
        ),
        DnsheRecordUpdatePlan::Noop
    );
    assert_eq!(
        plan_dnshe_record_update(
            &existing,
            "host.managed.example.com",
            "managed.example.com",
            "A",
            "192.0.2.22"
        ),
        DnsheRecordUpdatePlan::Update(21)
    );
    assert_eq!(
        plan_dnshe_record_update(
            &existing,
            "new.managed.example.com",
            "managed.example.com",
            "A",
            "192.0.2.23"
        ),
        DnsheRecordUpdatePlan::Create
    );
    assert_eq!(
        plan_dnshe_record_update(
            &json!({
                "records": [{
                    "name": "host.managed.example.com",
                    "type": "A",
                    "content": "192.0.2.21"
                }]
            }),
            "host.managed.example.com",
            "managed.example.com",
            "A",
            "192.0.2.22"
        ),
        DnsheRecordUpdatePlan::MissingId
    );
}

#[test]
fn dnshe_api_errors_use_stable_safe_field_precedence() {
    assert_eq!(
        format_dnshe_error(&json!({
            "message": "human message",
            "error": "legacy error",
            "error_code": "stable_code"
        })),
        "human message"
    );
    assert_eq!(
        format_dnshe_error(&json!({
            "error": "legacy error",
            "error_code": "stable_code"
        })),
        "legacy error"
    );
    assert_eq!(
        format_dnshe_error(&json!({ "error_code": "stable_code" })),
        "stable_code"
    );
    assert!(assert_dnshe_success(StatusCode::OK, &json!({ "success": true })).is_ok());
    assert!(
        assert_dnshe_success(
            StatusCode::UNAUTHORIZED,
            &json!({
                "success": false,
                "error_code": "auth_invalid_credentials"
            })
        )
        .unwrap_err()
        .to_string()
        .contains("auth_invalid_credentials")
    );
}

#[test]
fn dnshe_request_specs_keep_credentials_in_headers_and_match_api_contract() {
    let api_key = "offline-api-key";
    let api_secret = "offline-api-secret";
    let list = dnshe_request_spec(
        api_key,
        api_secret,
        "subdomains",
        "list",
        &[("page", "2".to_string()), ("per_page", "500".to_string())],
        None,
    )
    .unwrap();
    assert_eq!(list.method, reqwest::Method::GET);
    assert!(list.body.is_none());
    assert!(!list.url.contains(api_key));
    assert!(!list.url.contains(api_secret));
    let list_url = Url::parse(&list.url).unwrap();
    assert_eq!(list_url.scheme(), "https");
    assert_eq!(list_url.host_str(), Some("api005.dnshe.com"));
    assert_eq!(list_url.path(), "/index.php");
    let query = list_url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<HashMap<_, _>>();
    assert_eq!(query.get("m").map(String::as_str), Some("domain_hub"));
    assert_eq!(
        query.get("endpoint").map(String::as_str),
        Some("subdomains")
    );
    assert_eq!(query.get("action").map(String::as_str), Some("list"));
    assert_eq!(query.get("page").map(String::as_str), Some("2"));
    assert_eq!(query.get("per_page").map(String::as_str), Some("500"));
    let list_headers = list.headers.into_iter().collect::<HashMap<_, _>>();
    assert_eq!(
        list_headers.get("X-API-Key").map(String::as_str),
        Some(api_key)
    );
    assert_eq!(
        list_headers.get("X-API-Secret").map(String::as_str),
        Some(api_secret)
    );
    assert!(!list_headers.contains_key("content-type"));

    let body = json!({
        "subdomain_id": 7,
        "type": "AAAA",
        "name": "host",
        "content": "2001:db8::7",
        "ttl": 600
    });
    let create = dnshe_request_spec(
        api_key,
        api_secret,
        "dns_records",
        "create",
        &[],
        Some(body.clone()),
    )
    .unwrap();
    assert_eq!(create.method, reqwest::Method::POST);
    assert_eq!(create.body, Some(body));
    assert!(!create.url.contains(api_key));
    assert!(!create.url.contains(api_secret));
    let create_headers = create.headers.into_iter().collect::<HashMap<_, _>>();
    assert_eq!(
        create_headers.get("content-type").map(String::as_str),
        Some("application/json")
    );

    let update_body = json!({
        "id": 9,
        "content": "192.0.2.9",
        "ttl": 600
    });
    let update = dnshe_request_spec(
        api_key,
        api_secret,
        "dns_records",
        "update",
        &[],
        Some(update_body.clone()),
    )
    .unwrap();
    assert_eq!(update.method, reqwest::Method::POST);
    assert_eq!(update.body, Some(update_body));
    let update_url = Url::parse(&update.url).unwrap();
    let update_query = update_url
        .query_pairs()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect::<HashMap<_, _>>();
    assert_eq!(
        update_query.get("endpoint").map(String::as_str),
        Some("dns_records")
    );
    assert_eq!(
        update_query.get("action").map(String::as_str),
        Some("update")
    );
}

#[tokio::test]
async fn ddns_no_redirect_client_does_not_forward_sensitive_headers() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let redirect_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_address = redirect_listener.local_addr().unwrap();
    let target_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target_address = target_listener.local_addr().unwrap();

    let redirect_server = tokio::spawn(async move {
        let (mut socket, _) = redirect_listener.accept().await.unwrap();
        let mut request = [0_u8; 2048];
        let _ = socket.read(&mut request).await.unwrap();
        let response = format!(
            "HTTP/1.1 302 Found\r\nLocation: http://{target_address}/capture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    });
    let target_server = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_millis(500), target_listener.accept())
            .await
            .is_ok()
    });

    let client =
        ddns_http_client_no_redirects(&Translator::new("en"), &DDNSHttpClientOptions::default())
            .unwrap();
    let response = client
        .get(format!("http://{redirect_address}/start"))
        .header("X-Test-Sensitive", "offline-secret")
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    redirect_server.await.unwrap();
    assert!(!target_server.await.unwrap());
}

#[tokio::test]
async fn curl_headers_are_serialized_to_a_private_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("headers.txt");
    let headers = vec![
        ("X-API-Key".to_string(), "offline-api-key".to_string()),
        ("X-API-Secret".to_string(), "offline-api-secret".to_string()),
    ];
    write_private_curl_headers(&path, &headers).await.unwrap();
    assert_eq!(
        tokio::fs::read_to_string(&path).await.unwrap(),
        "x-api-key: offline-api-key\nx-api-secret: offline-api-secret\n"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mode = tokio::fs::metadata(&path)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
    }
    assert!(
        serialize_curl_headers(&[(
            "X-API-Key".to_string(),
            "value\r\nInjected: true".to_string()
        )])
        .is_err()
    );
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
                "name": "dnshe",
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
            "dnshe",
            &["api_key", "api_secret", "root_domain", "domain", "ttl"],
        ),
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
    let dnshe = provider_by_name(&providers, "dnshe");
    assert_eq!(
        provider_field(dnshe, "root_domain").get("label"),
        Some(&json!("DNSHE 托管域名"))
    );
    for locale in ["zh-CN", "zh-Hant", "en", "ko-KR", "ja-JP"] {
        let localized_providers = provider_catalog(&Translator::new(locale));
        let localized_dnshe = provider_by_name(&localized_providers, "dnshe");
        assert_eq!(
            provider_field(localized_dnshe, "api_key").get("label"),
            Some(&json!("API Key")),
            "{locale} DNSHE api_key label must use the provider's exact term"
        );
        assert_eq!(
            provider_field(localized_dnshe, "api_secret").get("label"),
            Some(&json!("API Secret")),
            "{locale} DNSHE api_secret label must use the provider's exact term"
        );
    }

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

#[test]
fn ddns_domain_targets_parser_canonicalizes_supported_inputs() {
    let cases = [
        ("Home.Example.COM.", "home.example.com"),
        ("*.Example.COM.", "*.example.com"),
        ("*.Example.COM， Example.COM.", "*.example.com,example.com"),
        ("example.com   *.example.com", "*.example.com,example.com"),
        (
            " ,，  *.xn--fsqu00a.xn--0zwm56d,,,xn--fsqu00a.xn--0zwm56d ",
            "*.xn--fsqu00a.xn--0zwm56d,xn--fsqu00a.xn--0zwm56d",
        ),
    ];
    for (input, expected) in cases {
        assert_eq!(
            parse_ddns_domain_targets(input).unwrap().canonical(),
            expected,
            "input={input}"
        );
    }
}

#[test]
fn ddns_domain_targets_parser_rejects_invalid_inputs() {
    let invalid = [
        "",
        ", ， ",
        "example",
        "*.com",
        "https://example.com",
        "example.com:443",
        "example.com/path",
        "exa_mple.com",
        "-bad.example.com",
        "bad-.example.com",
        "bad..example.com",
        "foo.*.example.com",
        "**.example.com",
        "例子.测试",
        "192.0.2.1",
        "01.02.03.04",
        "example.com,other.example.com",
        "*.example.com,*.other.example.com",
        "*.example.com,other.example.com",
        "*.example.com,example.com,third.example.com",
    ];
    for input in invalid {
        assert!(
            parse_ddns_domain_targets(input).is_err(),
            "input should be rejected: {input}"
        );
    }
    let long_label = format!("{}.example.com", "a".repeat(64));
    assert!(parse_ddns_domain_targets(&long_label).is_err());
    let long_domain = format!(
        "{}.{}.{}.{}.com",
        "a".repeat(63),
        "b".repeat(63),
        "c".repeat(63),
        "d".repeat(63)
    );
    assert!(parse_ddns_domain_targets(&long_domain).is_err());
}

#[test]
fn ddns_domain_target_policy_validates_explicit_roots_and_single_only_providers() {
    for provider in [
        "alidns",
        "baiducloud",
        "dnshe",
        "dnspod",
        "godaddy",
        "huaweicloud",
        "porkbun",
        "tencentcloud",
    ] {
        let mut valid = HashMap::from([
            (
                "domain".to_string(),
                "Example.COM *.Example.COM.".to_string(),
            ),
            ("root_domain".to_string(), "EXAMPLE.com.".to_string()),
        ]);
        let targets = normalize_and_validate_ddns_domain_config(provider, &mut valid)
            .unwrap()
            .unwrap();
        assert!(targets.is_pair(), "provider={provider}");
        assert_eq!(valid["domain"], "*.example.com,example.com");
        assert_eq!(valid["root_domain"], "example.com");

        let mut subdomain = HashMap::from([
            (
                "domain".to_string(),
                "*.R.Example.COM,R.Example.COM".to_string(),
            ),
            ("root_domain".to_string(), "EXAMPLE.com.".to_string()),
        ]);
        normalize_and_validate_ddns_domain_config(provider, &mut subdomain).unwrap();
        assert_eq!(
            subdomain["domain"], "*.r.example.com,r.example.com",
            "provider={provider}"
        );

        let mut wrong = valid.clone();
        wrong.insert("root_domain".to_string(), "other.example.com".to_string());
        assert_eq!(
            normalize_and_validate_ddns_domain_config(provider, &mut wrong),
            Err(DDNSDomainConfigError::PairRootMismatch {
                field: "root_domain".to_string(),
                expected: "other.example.com".to_string(),
                actual: "example.com".to_string(),
            })
        );
    }

    let mut esa = HashMap::from([
        (
            "domain".to_string(),
            "*.r.sub.example.com,r.sub.example.com".to_string(),
        ),
        ("site_name".to_string(), "SUB.EXAMPLE.COM.".to_string()),
    ]);
    normalize_and_validate_ddns_domain_config("esa", &mut esa).unwrap();
    assert_eq!(esa["site_name"], "sub.example.com");

    let mut edgeone_cname = HashMap::from([(
        "domain".to_string(),
        "*.example.com,example.com".to_string(),
    )]);
    assert!(matches!(
        normalize_and_validate_ddns_domain_config("edgeone_cname", &mut edgeone_cname),
        Err(DDNSDomainConfigError::PairUnsupported { .. })
    ));
}

#[test]
fn ddns_zone_containment_uses_strict_label_boundaries() {
    assert!(ddns_domain_is_same_or_subdomain("wxlnk.com", "wxlnk.com"));
    assert!(ddns_domain_is_same_or_subdomain("r.wxlnk.com", "wxlnk.com"));
    assert!(ddns_domain_is_same_or_subdomain(
        "deep.r.wxlnk.com",
        "wxlnk.com"
    ));
    assert!(!ddns_domain_is_same_or_subdomain(
        "evilsuffixwxlnk.com",
        "wxlnk.com"
    ));
    assert!(!ddns_domain_is_same_or_subdomain(
        "wxlnk.com.evil",
        "wxlnk.com"
    ));
    assert!(!ddns_domain_is_same_or_subdomain("", "wxlnk.com"));
    assert!(!ddns_domain_is_same_or_subdomain("wxlnk.com", ""));

    let mut explicit_root = HashMap::from([
        (
            "domain".to_string(),
            "*.evilsuffixwxlnk.com,evilsuffixwxlnk.com".to_string(),
        ),
        ("root_domain".to_string(), "wxlnk.com".to_string()),
    ]);
    assert!(matches!(
        normalize_and_validate_ddns_domain_config("alidns", &mut explicit_root),
        Err(DDNSDomainConfigError::PairRootMismatch { .. })
    ));
}

#[test]
fn ddns_domain_config_keeps_empty_drafts_and_non_domain_providers_compatible() {
    let empty = HashMap::from([("domain".to_string(), "   ".to_string())]);
    let normalized = normalize_and_validate_config("cloudflare", empty).unwrap();
    assert_eq!(normalized.get("domain").map(String::as_str), Some(""));

    let duckdns = HashMap::from([("domains".to_string(), "one,two".to_string())]);
    assert_eq!(
        normalize_and_validate_config("duckdns", duckdns.clone()).unwrap()["domains"],
        duckdns["domains"]
    );
}

#[test]
fn ddns_single_domain_normalizes_explicit_root_for_provider_splitters() {
    for (provider, root_field) in [
        ("alidns", "root_domain"),
        ("baiducloud", "root_domain"),
        ("dnshe", "root_domain"),
        ("dnspod", "root_domain"),
        ("godaddy", "root_domain"),
        ("huaweicloud", "root_domain"),
        ("porkbun", "root_domain"),
        ("tencentcloud", "root_domain"),
        ("esa", "site_name"),
    ] {
        let mut config = HashMap::from([
            ("domain".to_string(), "Home.Example.COM.".to_string()),
            (root_field.to_string(), "Example.COM.".to_string()),
        ]);
        normalize_and_validate_ddns_domain_config(provider, &mut config).unwrap();
        assert_eq!(config["domain"], "home.example.com", "provider={provider}");
        assert_eq!(config[root_field], "example.com", "provider={provider}");
    }
}

#[test]
fn ddns_domain_update_plans_select_fanout_and_dynu_wildcard_alias() {
    let alidns = build_ddns_provider_update_plan(
        "alidns",
        &HashMap::from([
            (
                "domain".to_string(),
                "example.com，*.example.com".to_string(),
            ),
            ("root_domain".to_string(), "example.com".to_string()),
        ]),
    )
    .unwrap();
    assert_eq!(alidns.execution, DdnsDomainUpdateExecution::FanOut);
    assert_eq!(alidns.config["domain"], "*.example.com,example.com");
    assert_eq!(
        alidns.targets.unwrap().domains(),
        vec!["*.example.com", "example.com"]
    );

    let dynu = build_ddns_provider_update_plan(
        "dynu",
        &HashMap::from([(
            "domain".to_string(),
            "*.example.com example.com".to_string(),
        )]),
    )
    .unwrap();
    assert_eq!(dynu.execution, DdnsDomainUpdateExecution::DynuWildcardAlias);

    let single = build_ddns_provider_update_plan(
        "cloudflare",
        &HashMap::from([("domain".to_string(), "home.example.com".to_string())]),
    )
    .unwrap();
    assert_eq!(single.execution, DdnsDomainUpdateExecution::Single);
}

#[test]
fn edgeone_pair_preflight_is_required_before_auxiliary_writes() {
    let pair_config = HashMap::from([(
        "domain".to_string(),
        "*.example.com,example.com".to_string(),
    )]);
    let edgeone = build_ddns_provider_update_plan("edgeone", &pair_config).unwrap();
    assert!(ddns_preflight_required_before_auxiliary(
        "edgeone", &edgeone
    ));

    for provider in ["cloudflare", "esa"] {
        let mut config = pair_config.clone();
        if provider == "esa" {
            config.insert("site_name".to_string(), "example.com".to_string());
        }
        let plan = build_ddns_provider_update_plan(provider, &config).unwrap();
        assert!(
            !ddns_preflight_required_before_auxiliary(provider, &plan),
            "provider={provider}"
        );
    }
}

#[test]
fn ddns_pair_results_are_aggregated_without_hiding_partial_failure() {
    let translator = Translator::new("en");
    let result = aggregate_domain_update_results(
        &translator,
        vec![
            (
                "*.example.com".to_string(),
                DDNSProviderUpdateResult {
                    success: false,
                    message: "first failed".to_string(),
                },
            ),
            (
                "example.com".to_string(),
                DDNSProviderUpdateResult {
                    success: true,
                    message: "second succeeded".to_string(),
                },
            ),
        ],
    );
    assert!(!result.success);
    assert!(result.message.contains("*.example.com"));
    assert!(result.message.contains("example.com"));
    assert!(result.message.contains("first failed"));
    assert!(!result.message.contains("second succeeded"));
}

#[test]
fn ddns_pair_all_success_summary_uses_only_the_target_count() {
    let translator = Translator::new("en");
    let result = aggregate_domain_update_results(
        &translator,
        vec![
            (
                "*.example.com".to_string(),
                DDNSProviderUpdateResult {
                    success: true,
                    message: "Cloudflare DNS update succeeded".to_string(),
                },
            ),
            (
                "example.com".to_string(),
                DDNSProviderUpdateResult {
                    success: true,
                    message: "Cloudflare DNS update succeeded".to_string(),
                },
            ),
        ],
    );
    assert!(result.success);
    assert!(result.message.contains('2'));
    assert!(!result.message.contains("example.com"));
    assert!(!result.message.contains("Cloudflare"));
    assert!(!result.message.contains("DNS update succeeded"));

    let zh = Translator::new("zh-CN");
    let zh_result = aggregate_domain_update_results(
        &zh,
        vec![
            (
                "*.r.wxlnk.com".to_string(),
                DDNSProviderUpdateResult {
                    success: true,
                    message: "Cloudflare DNS 更新成功".to_string(),
                },
            ),
            (
                "r.wxlnk.com".to_string(),
                DDNSProviderUpdateResult {
                    success: true,
                    message: "Cloudflare DNS 更新成功".to_string(),
                },
            ),
        ],
    );
    assert_eq!(zh_result.message, "共 2 个域名");
    assert_eq!(
        ddns_text(&zh, "updateSuccess", &[("message", zh_result.message)]),
        "更新成功: 共 2 个域名"
    );
}

#[test]
fn manual_test_result_message_wraps_provider_result_once() {
    let translator = Translator::new("en");
    assert_eq!(
        manual_test_result_message(
            &translator,
            &DDNSProviderUpdateResult {
                success: true,
                message: "2 domains".to_string(),
            },
        ),
        "Update succeeded: 2 domains"
    );
    assert_eq!(
        manual_test_result_message(
            &translator,
            &DDNSProviderUpdateResult {
                success: false,
                message: "*.example.com: failed (denied)".to_string(),
            },
        ),
        "Update failed: *.example.com: failed (denied)"
    );
}

#[tokio::test]
async fn ddns_domain_fanout_preserves_order_and_continues_after_error() {
    let translator = Translator::new("en");
    let targets = parse_ddns_domain_targets("example.com,*.example.com").unwrap();
    let attempts = std::cell::RefCell::new(Vec::new());

    let result = execute_ddns_domain_fanout(&translator, &targets, |domain| {
        attempts.borrow_mut().push(domain.clone());
        async move {
            if domain.starts_with("*.") {
                anyhow::bail!("wildcard failed");
            }
            Ok(DDNSProviderUpdateResult {
                success: true,
                message: "root succeeded".to_string(),
            })
        }
    })
    .await;

    assert_eq!(
        attempts.into_inner(),
        vec!["*.example.com".to_string(), "example.com".to_string()]
    );
    assert!(!result.success);
    assert!(result.message.contains("wildcard failed"));
    assert!(!result.message.contains("root succeeded"));
}

#[test]
fn ddns_domain_target_sets_detect_pair_single_overlap() {
    let pair = HashMap::from([
        (
            "domain".to_string(),
            "*.example.com,example.com".to_string(),
        ),
        ("root_domain".to_string(), "example.com".to_string()),
    ]);
    let root = HashMap::from([("domain".to_string(), "example.com".to_string())]);
    let wildcard = HashMap::from([("domain".to_string(), "*.example.com".to_string())]);
    let other = HashMap::from([("domain".to_string(), "other.example.com".to_string())]);
    let pair_set = ddns_domain_target_set("alidns", &pair).unwrap();
    assert!(!pair_set.is_disjoint(&ddns_domain_target_set("alidns", &root).unwrap()));
    assert!(!pair_set.is_disjoint(&ddns_domain_target_set("alidns", &wildcard).unwrap()));
    assert!(pair_set.is_disjoint(&ddns_domain_target_set("alidns", &other).unwrap()));
}

#[test]
fn provider_catalog_exposes_domain_target_capabilities_from_policy() {
    let providers = provider_catalog(&Translator::new("en"));
    for provider in [
        "alidns",
        "baiducloud",
        "dnshe",
        "dnspod",
        "godaddy",
        "huaweicloud",
        "porkbun",
        "tencentcloud",
    ] {
        assert_eq!(
            provider_by_name(&providers, provider).pointer("/capabilities/domainTargets"),
            Some(&json!({
                "mode": "single_or_wildcard_root_pair",
                "rootField": "root_domain"
            })),
            "provider={provider}"
        );
    }
    assert_eq!(
        provider_by_name(&providers, "esa").pointer("/capabilities/domainTargets"),
        Some(&json!({
            "mode": "single_or_wildcard_root_pair",
            "rootField": "site_name"
        }))
    );
    for provider in ["cloudflare", "edgeone", "dynu"] {
        assert_eq!(
            provider_by_name(&providers, provider).pointer("/capabilities/domainTargets"),
            Some(&json!({ "mode": "single_or_wildcard_root_pair" })),
            "provider={provider}"
        );
    }
    assert_eq!(
        provider_by_name(&providers, "edgeone_cname").pointer("/capabilities/domainTargets/mode"),
        Some(&json!("single"))
    );
    for provider in ["duckdns", "dynv6", "noip"] {
        assert!(
            provider_by_name(&providers, provider)
                .pointer("/capabilities/domainTargets")
                .is_none(),
            "provider={provider}"
        );
    }
}

#[test]
fn remote_zone_and_site_response_parsers_require_exact_ids_and_names() {
    let cloudflare = json!({
        "result": { "id": "zone-1", "name": "Example.COM." },
        "success": true
    });
    assert_eq!(
        cloudflare_zone_name_from_response(&cloudflare, "zone-1").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        cloudflare_zone_name_from_response(&cloudflare, "zone-2"),
        None
    );
    assert_eq!(
        cloudflare_zone_name_from_response(
            &json!({ "result": { "name": "example.com" }, "success": true }),
            "zone-1"
        ),
        None
    );

    let edgeone = json!({
        "Zones": [
            { "ZoneId": "zone-a", "ZoneName": "a.example.com" },
            { "ZoneId": "zone-b", "ZoneName": "Example.COM." }
        ]
    });
    assert_eq!(
        edgeone_zone_name_from_response(&edgeone, "zone-b").as_deref(),
        Some("example.com")
    );
    assert_eq!(edgeone_zone_name_from_response(&edgeone, "zone-c"), None);

    let esa = json!({
        "Sites": [
            { "SiteId": 123, "SiteName": "Example.COM." },
            { "SiteId": "456", "SiteName": "other.example.com" }
        ]
    });
    assert_eq!(
        esa_site_id_from_response(&esa, "Example.COM").as_deref(),
        Some("123")
    );
    assert_eq!(esa_site_id_from_response(&esa, "missing.example.com"), None);
}

#[test]
fn typed_domain_config_errors_are_http_bad_requests() {
    let translator = Translator::new("en");
    let error = DDNSDomainConfigError::PairUnsupported {
        provider: "edgeone_cname".to_string(),
    };
    let response = ddns_error_response(&translator, anyhow::Error::new(error));
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn wildcard_root_pair_is_stored_canonically_and_conflicts_with_each_member() {
    let (_directory, state) = ddns_test_state().await;
    let translator = Translator::new("en");
    let created = create_ddns_target(
        &state,
        TargetBody {
            name: Some("pair".to_string()),
            provider: "alidns".to_string(),
            enabled: Some(true),
            config: Some(HashMap::from([
                (
                    "domain".to_string(),
                    "Example.COM.，*.Example.COM.".to_string(),
                ),
                ("root_domain".to_string(), "Example.COM.".to_string()),
            ])),
        },
        &translator,
    )
    .await
    .unwrap();
    let id = created.get("id").and_then(Value::as_str).unwrap();
    let stored = state
        .storage
        .store
        .hgetall_string_map(&target_config_key(id))
        .await
        .unwrap();
    assert_eq!(stored["domain"], "*.example.com,example.com");
    assert_eq!(stored["root_domain"], "example.com");

    for domain in ["*.example.com", "example.com"] {
        let error = create_ddns_target(
            &state,
            TargetBody {
                name: Some(domain.to_string()),
                provider: "alidns".to_string(),
                enabled: Some(true),
                config: Some(HashMap::from([
                    ("domain".to_string(), domain.to_string()),
                    ("root_domain".to_string(), "example.com".to_string()),
                ])),
            },
            &translator,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("Duplicate DDNS target"));
    }
}

#[tokio::test]
async fn invalid_domain_target_writes_are_atomic() {
    let (_directory, state) = ddns_test_state().await;
    let translator = Translator::new("en");

    let ids_before = state
        .storage
        .store
        .smembers_strings(DDNS_TARGET_IDS)
        .await
        .unwrap();
    let create_error = create_ddns_target(
        &state,
        TargetBody {
            name: Some("invalid".to_string()),
            provider: "edgeone_cname".to_string(),
            enabled: Some(true),
            config: Some(HashMap::from([(
                "domain".to_string(),
                "*.example.com,example.com".to_string(),
            )])),
        },
        &translator,
    )
    .await
    .unwrap_err();
    assert!(
        create_error
            .downcast_ref::<DDNSDomainConfigError>()
            .is_some()
    );
    assert_eq!(
        state
            .storage
            .store
            .smembers_strings(DDNS_TARGET_IDS)
            .await
            .unwrap(),
        ids_before
    );

    let update_error = update_ddns_target(
        &state,
        "missing-target",
        TargetBody {
            name: Some("invalid".to_string()),
            provider: "edgeone_cname".to_string(),
            enabled: Some(true),
            config: Some(HashMap::from([(
                "domain".to_string(),
                "*.example.com,example.com".to_string(),
            )])),
        },
        &translator,
    )
    .await
    .unwrap_err();
    assert!(
        update_error
            .downcast_ref::<DDNSDomainConfigError>()
            .is_some()
    );
    assert_eq!(
        state
            .storage
            .store
            .smembers_strings(DDNS_TARGET_IDS)
            .await
            .unwrap(),
        ids_before
    );

    let created = create_ddns_target(
        &state,
        TargetBody {
            name: Some("valid".to_string()),
            provider: "alidns".to_string(),
            enabled: Some(true),
            config: Some(HashMap::from([
                ("domain".to_string(), "home.example.com".to_string()),
                ("root_domain".to_string(), "example.com".to_string()),
            ])),
        },
        &translator,
    )
    .await
    .unwrap();
    let id = created.get("id").and_then(Value::as_str).unwrap();
    let meta_key = target_meta_key(id);
    let config_key = target_config_key(id);
    let last_ip_key = target_last_ip_key(id);
    let last_check_key = target_last_check_key(id);
    let before_meta = state
        .storage
        .store
        .hgetall_string_map(&meta_key)
        .await
        .unwrap();
    let before_config = state
        .storage
        .store
        .hgetall_string_map(&config_key)
        .await
        .unwrap();
    let before_last_ip = state
        .storage
        .store
        .hgetall_string_map(&last_ip_key)
        .await
        .unwrap();
    let before_last_check = state
        .storage
        .store
        .hgetall_string_map(&last_check_key)
        .await
        .unwrap();

    let update_error = update_ddns_target(
        &state,
        id,
        TargetBody {
            name: Some("must not be saved".to_string()),
            provider: "alidns".to_string(),
            enabled: Some(false),
            config: Some(HashMap::from([
                (
                    "domain".to_string(),
                    "*.example.com,other.example.com".to_string(),
                ),
                ("root_domain".to_string(), "example.com".to_string()),
            ])),
        },
        &translator,
    )
    .await
    .unwrap_err();
    assert!(
        update_error
            .downcast_ref::<DDNSDomainConfigError>()
            .is_some()
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&meta_key)
            .await
            .unwrap(),
        before_meta
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&config_key)
            .await
            .unwrap(),
        before_config
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&last_ip_key)
            .await
            .unwrap(),
        before_last_ip
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&last_check_key)
            .await
            .unwrap(),
        before_last_check
    );

    let draft_key = format!("{DDNS_LEGACY_CONFIG_PREFIX}cloudflare");
    let before_draft = state
        .storage
        .store
        .hgetall_string_map(&draft_key)
        .await
        .unwrap();
    assert!(
        save_primary_config(
            &state,
            "cloudflare",
            HashMap::from([(
                "domain".to_string(),
                "*.example.com,other.example.com".to_string(),
            )]),
        )
        .await
        .unwrap_err()
        .downcast_ref::<DDNSDomainConfigError>()
        .is_some()
    );
    assert_eq!(
        state
            .storage
            .store
            .hgetall_string_map(&draft_key)
            .await
            .unwrap(),
        before_draft
    );
}

#[tokio::test]
async fn changed_config_resets_runtime_before_write_and_stops_when_reset_fails() {
    use std::sync::{Arc, Mutex};

    let steps = Arc::new(Mutex::new(Vec::new()));
    let reset_steps = steps.clone();
    let write_steps = steps.clone();
    write_config_after_runtime_reset(
        true,
        async move {
            reset_steps.lock().unwrap().push("reset");
            Ok(())
        },
        async move {
            write_steps.lock().unwrap().push("write");
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(*steps.lock().unwrap(), vec!["reset", "write"]);

    let steps = Arc::new(Mutex::new(Vec::new()));
    let reset_steps = steps.clone();
    let write_steps = steps.clone();
    let result = write_config_after_runtime_reset(
        true,
        async move {
            reset_steps.lock().unwrap().push("reset");
            anyhow::bail!("reset failed")
        },
        async move {
            write_steps.lock().unwrap().push("write");
            Ok(())
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(*steps.lock().unwrap(), vec!["reset"]);

    let steps = Arc::new(Mutex::new(Vec::new()));
    let reset_steps = steps.clone();
    let write_steps = steps.clone();
    write_config_after_runtime_reset(
        false,
        async move {
            reset_steps.lock().unwrap().push("reset");
            Ok(())
        },
        async move {
            write_steps.lock().unwrap().push("write");
            Ok(())
        },
    )
    .await
    .unwrap();
    assert_eq!(*steps.lock().unwrap(), vec!["write"]);
}
