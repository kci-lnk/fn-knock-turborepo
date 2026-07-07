use super::*;

#[test]
fn normalizes_acme_application_like_node() {
    let value = normalize_acme_application(json!({
        "id": " app ",
        "domains": ["Example.com", "example.com", "www.example.com"],
        "dnsType": " dns_cf ",
        "credentials": { " CF_Key ": " secret ", "empty": "" },
        "createdAt": "2026-07-05T01:02:03Z",
        "latestJobStatus": "bad"
    }))
    .expect("application");
    assert_eq!(value["id"], json!("app"));
    assert_eq!(value["primaryDomain"], json!("example.com"));
    assert_eq!(value["domains"], json!(["example.com", "www.example.com"]));
    assert_eq!(value["credentials"], json!({ "CF_Key": "secret" }));
    assert_eq!(value["renewEnabled"], json!(true));
    assert_eq!(value["latestJobStatus"], Value::Null);
}

#[test]
fn acme_renew_interval_prefers_node_cron_env() {
    assert_eq!(
        acme_renew_interval_from_values(None, None).as_secs(),
        6 * 3600
    );
    assert_eq!(
        acme_renew_interval_from_values(Some("0 */6 * * *"), Some("7200")).as_secs(),
        6 * 3600
    );
    assert_eq!(
        acme_renew_interval_from_values(Some("*/30 * * * *"), None).as_secs(),
        30 * 60
    );
    assert_eq!(
        acme_renew_interval_from_values(None, Some("7200")).as_secs(),
        7200
    );
}

#[test]
fn detects_issued_certificate_compatibility_by_domain_set() {
    let application = json!({
        "primaryDomain": "example.com",
        "domains": ["example.com", "www.example.com"],
    });
    let certificate = json!({
        "primaryDomain": "example.com",
        "certInfo": { "dnsNames": ["www.example.com", "example.com"] },
    });
    assert!(issued_certificate_compatible(&application, &certificate));
}

#[test]
fn builds_stable_legacy_application_id() {
    assert_eq!(
        build_application_id(Some("Example.com")),
        build_application_id(Some("example.com"))
    );
    assert!(build_application_id(Some("example.com")).starts_with("acme_app_"));
}

#[test]
fn normalizes_log_limit_bounds() {
    assert_eq!(normalize_log_limit(None), DEFAULT_ACME_LOG_LIMIT);
    assert_eq!(normalize_log_limit(Some("")), 1);
    assert_eq!(normalize_log_limit(Some("   ")), 1);
    assert_eq!(normalize_log_limit(Some("0")), 1);
    assert_eq!(normalize_log_limit(Some("-5")), 1);
    assert_eq!(normalize_log_limit(Some("2000")), MAX_ACME_LOG_LIMIT);
    assert_eq!(normalize_log_limit(Some("10")), 10);
    assert_eq!(normalize_log_limit(Some("3.9")), 3);
    assert_eq!(normalize_log_limit(Some("10x")), DEFAULT_ACME_LOG_LIMIT);
}

#[test]
fn localizes_queued_job_domain_validation() {
    let t = Translator::new("zh-CN");
    let error = build_queued_acme_job(&json!({ "domains": [] }), "manual_request", &t)
        .expect_err("empty domains should be rejected");
    assert_eq!(error.to_string(), "域名列表不能为空或格式无效");

    let job = build_queued_acme_job(
        &json!({
            "id": "app-1",
            "domains": ["Example.com"],
            "dnsType": "dns_cf"
        }),
        "auto_renew",
        &t,
    )
    .expect("valid job");
    assert_eq!(job["status"], json!("queued"));
    assert_eq!(job["message"], json!("queued for renew"));
}

#[test]
fn builds_pending_application_for_submit_now_update_like_node() {
    let existing = json!({
        "id": "app-1",
        "name": "Old name",
        "domains": ["old.example.com"],
        "primaryDomain": "old.example.com",
        "dnsType": "dns_cf",
        "credentials": { "CF_Token": "old" },
        "renewEnabled": true,
        "latestJobId": "job-1"
    });
    let normalized = NormalizedAcmeRequest {
        domains: vec!["example.com".to_string(), "*.example.com".to_string()],
        dns_type: "dns_ali".to_string(),
        credentials: json!({ "Ali_Key": "key", "Ali_Secret": "secret" }),
    };
    let pending = build_pending_acme_application_for_update(
        &existing,
        &json!({
            "name": "  ",
            "renewEnabled": false
        }),
        &normalized,
    );

    assert_eq!(pending["id"], json!("app-1"));
    assert!(pending.get("name").is_none());
    assert_eq!(pending["domains"], json!(["example.com", "*.example.com"]));
    assert_eq!(pending["primaryDomain"], json!("example.com"));
    assert_eq!(pending["dnsType"], json!("dns_ali"));
    assert_eq!(
        pending["credentials"],
        json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
    );
    assert_eq!(pending["renewEnabled"], json!(false));
    assert_eq!(pending["latestJobId"], json!("job-1"));
}

#[test]
fn provider_catalog_contains_node_dns_types() {
    let t = Translator::new("en");
    let providers = acme_dns_providers(&t);
    let dns_types = providers
        .iter()
        .filter_map(|item| item.get("dnsType").and_then(Value::as_str))
        .collect::<BTreeSet<_>>();
    assert!(dns_types.contains("dns_cf"));
    assert!(dns_types.contains("dns_azure"));
    assert!(dns_types.contains("dns_opnsense"));
    assert_eq!(
        providers
            .iter()
            .find(|item| item.get("dnsType").and_then(Value::as_str) == Some("dns_cf"))
            .and_then(|item| item.get("credentialSchemes"))
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
}

#[test]
fn validates_acme_request_with_alias_and_filters_credentials() {
    let t = Translator::new("en");
    let normalized = validate_acme_request(
        &json!({
            "domains": ["Example.com", "bad host", "*.example.com", "example.com"],
            "dnsType": "aliyun",
            "credentials": {
                "Ali_Key": " key ",
                "Ali_Secret": " secret ",
                "Ignored": "value"
            }
        }),
        &t,
    )
    .expect("valid request");

    assert_eq!(normalized.domains, vec!["example.com", "*.example.com"]);
    assert_eq!(normalized.dns_type, "dns_ali");
    assert_eq!(
        normalized.credentials,
        json!({ "Ali_Key": "key", "Ali_Secret": "secret" })
    );
}

#[test]
fn validates_netlify_credential_alias() {
    let t = Translator::new("en");
    let normalized = validate_acme_request(
        &json!({
            "domains": ["example.com"],
            "provider": "netlify",
            "credentials": {
                "NETLIFY_TOKEN": "token"
            }
        }),
        &t,
    )
    .expect("valid request");

    assert_eq!(normalized.dns_type, "dns_netlify");
    assert_eq!(
        normalized.credentials,
        json!({ "NETLIFY_ACCESS_TOKEN": "token" })
    );
}

#[test]
fn rejects_missing_acme_credentials() {
    let t = Translator::new("en");
    let error = validate_acme_request(
        &json!({
            "domains": ["example.com"],
            "dnsType": "dns_ali",
            "credentials": {
                "Ali_Key": "key"
            }
        }),
        &t,
    )
    .expect_err("credentials should be incomplete");

    assert!(error.contains("DNS API credentials are missing"));
    assert!(error.contains("Ali_Secret"));
}

#[test]
fn localizes_acme_route_errors() {
    let t = Translator::new("zh-CN");
    assert_eq!(acme_route_text(&t, "invalidRequestBody"), "请求体不正确");
    assert_eq!(acme_route_text(&t, "loadJobFailed"), "读取 ACME 任务失败");
    assert_eq!(
        acme_route_text(&t, "createCertificateZipFailed"),
        "创建 ACME 证书压缩包失败"
    );
    assert_eq!(
        acme_route_text(&t, "updateApplicationFailed"),
        "更新 ACME 申请项失败"
    );
    assert_eq!(
        acme_route_text(&t, "saveClientSettingsFailed"),
        "保存 ACME 客户端设置失败"
    );
    assert_eq!(
        acme_route_text(&t, "syncLibraryFailed"),
        "同步 ACME 证书到证书库失败"
    );
    assert_eq!(
        acme_route_text(&t, "deployCertificateFailed"),
        "部署 ACME 证书失败"
    );
    assert_eq!(acme_route_text(&t, "stopJobFailed"), "停止 ACME 任务失败");
}

#[test]
fn detects_submit_now_requests_for_fallback() {
    assert!(submit_now_requested(&json!({ "submitNow": true })));
    assert!(!submit_now_requested(&json!({ "submitNow": false })));
    assert!(!submit_now_requested(&json!({})));
}

#[test]
fn validates_acme_domain_like_node() {
    assert!(is_valid_acme_domain("example.com"));
    assert!(is_valid_acme_domain("*.example.com"));
    assert!(!is_valid_acme_domain("example"));
    assert!(!is_valid_acme_domain("deep.*.example.com"));
    assert!(!is_valid_acme_domain("bad host.example.com"));
}

#[test]
fn wildcard_domains_cover_single_label_subdomains_only() {
    let domains = vec!["example.com".to_string(), "*.example.com".to_string()];
    assert!(is_requirement_covered_by_certificate_domains(
        "app.example.com",
        &domains
    ));
    assert!(is_requirement_covered_by_certificate_domains(
        "example.com",
        &domains
    ));
    assert!(!is_requirement_covered_by_certificate_domains(
        "deep.app.example.com",
        &domains
    ));
}
