use super::*;

#[test]
fn normalizes_acme_application_like_node() {
    let value = normalize_acme_application(json!({
        "id": " app ",
        "domains": ["Example.com", "example.com", "www.example.com"],
        "dnsType": " dns_cf ",
        "credentials": { " CF_Key ": " secret ", "empty": "" },
        "createdAt": "2026-07-05T01:02:03.946511792Z",
        "updatedAt": "2026-07-05T09:02:03+08:00",
        "latestJobStatus": "bad"
    }))
    .expect("application");
    assert_eq!(value["id"], json!("app"));
    assert_eq!(value["primaryDomain"], json!("example.com"));
    assert_eq!(value["domains"], json!(["example.com", "www.example.com"]));
    assert_eq!(value["credentials"], json!({ "CF_Key": "secret" }));
    assert_eq!(value["renewEnabled"], json!(true));
    assert_eq!(value["createdAt"], json!("2026-07-05T01:02:03.946Z"));
    assert_eq!(value["updatedAt"], json!("2026-07-05T01:02:03.000Z"));
    assert_eq!(value["latestJobStatus"], Value::Null);
}

#[test]
fn normalizes_acme_timestamps_to_node_iso_shape() {
    assert_eq!(
        normalize_timestamp("2026-07-07T10:18:23.946511792Z"),
        Some("2026-07-07T10:18:23.946Z".to_string())
    );
    assert_eq!(
        normalize_timestamp("2026-07-07T18:18:23+08:00"),
        Some("2026-07-07T10:18:23.000Z".to_string())
    );
    assert_eq!(normalize_timestamp("not-a-date"), None);
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

#[test]
fn windows_wildcard_certificate_uses_safe_paths_and_apex_issue_storage() {
    let application = json!({
        "primaryDomain": "*.fs.wxlnk.com",
        "domains": ["*.fs.wxlnk.com", "fs.wxlnk.com"],
    });
    assert_eq!(
        acme_data_dir_name_for_target("*.fs.wxlnk.com", true),
        "wildcard_fs.wxlnk.com"
    );
    assert_eq!(
        acme_issued_storage_domain_for_target(&application, true),
        "fs.wxlnk.com"
    );
    assert_eq!(
        acme_issued_storage_domain_for_target(&application, false),
        "*.fs.wxlnk.com"
    );
}

#[test]
fn acme_zip_entry_names_preserve_requested_domain_like_node() {
    let bytes = zip_acme_cert_pair("Example.COM", "CERT", "KEY").expect("zip should build");
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("zip should parse");

    assert!(archive.by_name("Example.COM.cert.pem").is_ok());
    assert!(archive.by_name("Example.COM.key.pem").is_ok());
}

#[test]
fn acme_zip_uses_portable_names_and_non_empty_entries_for_wildcards() {
    use std::io::Read as _;

    assert_eq!(
        acme_certificate_archive_stem("*.Example.COM."),
        "wildcard.Example.COM"
    );
    let bytes = zip_acme_cert_pair("*.example.com", "CERT", "KEY").expect("zip should build");
    let cursor = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(cursor).expect("zip should parse");
    assert_eq!(archive.len(), 2);

    let mut cert = String::new();
    archive
        .by_name("wildcard.example.com.cert.pem")
        .expect("certificate entry")
        .read_to_string(&mut cert)
        .expect("certificate contents");
    assert_eq!(cert, "CERT");

    let mut key = String::new();
    archive
        .by_name("wildcard.example.com.key.pem")
        .expect("private key entry")
        .read_to_string(&mut key)
        .expect("private key contents");
    assert_eq!(key, "KEY");
}

#[test]
fn acme_init_payload_matches_node_shape() {
    let payload = build_init_acme_payload(
        PathBuf::from("/data/.acme.sh/acme.sh"),
        &json!({
            "certificateAuthority": "letsencrypt",
            "updatedAt": "2026-07-07T00:00:00Z",
        }),
    );
    let keys = payload
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();

    assert_eq!(keys, vec!["certificateAuthority", "executablePath"]);
    assert_eq!(payload["certificateAuthority"], json!("letsencrypt"));
    assert_eq!(payload["executablePath"], json!("/data/.acme.sh/acme.sh"));
    assert!(payload.get("state").is_none());
}

#[test]
fn analyzes_cloudflare_invalid_key_like_node() {
    let t = Translator::new("en");
    let logs = vec![
        json!("Cloudflare API request failed"),
        json!("{\"code\":6103,\"message\":\"Invalid format for X-Auth-Key header\"}"),
    ];
    let analysis = analyze_acme_logs(&json!({ "provider": "dns_cf" }), &logs, &t);

    assert_eq!(analysis["reason"], json!("dns_credentials_invalid"));
    assert_eq!(analysis["provider"], json!("dns_cf"));
    assert!(analysis["message"].as_str().unwrap().contains("Cloudflare"));
    assert_eq!(analysis["evidence"].as_array().unwrap().len(), 1);
}

#[test]
fn analyzes_retry_after_frequency_limit_like_node() {
    let t = Translator::new("en");
    let logs = vec![
        json!("server asks retryafter=601, too large, will not retry"),
        json!("final error"),
    ];
    let analysis = analyze_acme_logs(&json!({ "provider": "dns_ali" }), &logs, &t);

    assert_eq!(analysis["reason"], json!("acme_frequency_limited"));
    assert_eq!(analysis["provider"], json!("dns_ali"));
    assert!(analysis["message"].as_str().unwrap().contains("601"));
    assert_eq!(analysis["evidence"].as_array().unwrap().len(), 1);
}
