use super::*;

const SAMPLE_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIC3jCCAcagAwIBAgIJAMBvhSD/r2eYMA0GCSqGSIb3DQEBCwUAMBcxFTATBgNV
BAMMDGV4YW1wbGUudGVzdDAeFw0yNjA3MDQyMDUxMzdaFw0zNjA3MDEyMDUxMzda
MBcxFTATBgNVBAMMDGV4YW1wbGUudGVzdDCCASIwDQYJKoZIhvcNAQEBBQADggEP
ADCCAQoCggEBAK81U4+GWNLRNKq+y6/pslTHyvQZgX8bZQ68dijipzwyGLG5hnF0
ea+qXNicPvFb9MXGBL4i1GWnnp1x7T4d9WtFYi2Q+w2ZFoTJfmVtuwJhvmhFIBgI
nPyK3Sa/DDb886h5B/drD54wjpcFbEm7xbxHxzyRF3q1WZl69NevYDHiVhTa6n3s
x0XezCyuJ0GEgsqiJ5N61c3TLfwd1AJFV8WZnVUuUU4DzhSMadOrwSChd8s4jQ9A
+QZNrWYLBRxTAuJ2RYsPEgQ6sWw7k4//xJ4jhlzGi6AfS/FjvOGv+xCQlPhedSSM
/9qjo7m7oDhVXkbUJeIE7ZCWbGTW2B85fXECAwEAAaMtMCswKQYDVR0RBCIwIIIM
ZXhhbXBsZS50ZXN0ghBhbHQuZXhhbXBsZS50ZXN0MA0GCSqGSIb3DQEBCwUAA4IB
AQCTD4yYqhrVVL4pYaY1uyVqXV3/Ba6cFuXIExoe9XOljJu2M6I8D6KjWVtC9rVu
n+SwZed1BIdEKqv1sbdw45mMhJi1lYZe5QLFoRI+mB3/AjCx493ia8KSx7mrqO0y
Kc9jOEHzjkutbjTxoAhUdb9Pfwz6W9RIqZ2IpXxgIpDrQuRBp6yyw5/gpNQfPAt7
iQHXpmfpjC4kBqCEakPKpPURcBB4HY/tGg7tbqVLK6Q/Ujj/WAONeZuxB/mAtkiW
b6DS1sxh2TNX1zXA5idWls2foZDzzcC1XRB9iF+q7JCDdIYstLBgN23ZxJbDH3yS
uvwBvERVoHMCF4qFay/Qy8sf
-----END CERTIFICATE-----"#;

#[test]
fn ssl_certificate_ids_match_node_shape() {
    let id = build_ssl_certificate_id("cert", "key");
    assert!(id.starts_with("ssl_"));
    assert_eq!(id.len(), 20);
}

#[test]
fn normalizes_legacy_ssl_into_library() {
    let ssl = normalize_ssl_config(Some(&json!({
        "cert": "CERT",
        "key": "KEY",
        "deployment_mode": "multi_sni"
    })));
    assert_eq!(ssl["deployment_mode"], json!("multi_sni"));
    assert_eq!(ssl["certificates"].as_array().unwrap().len(), 1);
    assert_eq!(ssl["active_cert_id"], ssl["certificates"][0]["id"]);
    assert_eq!(ssl["certificates"][0]["label"], json!("当前证书"));
    let certificate = ssl["certificates"][0].as_object().unwrap();
    assert!(!certificate.contains_key("primary_domain"));
    assert!(!certificate.contains_key("source_ref_id"));
    assert_eq!(
        ssl["certificates"][0]["created_at"],
        json!("1970-01-01T00:00:00.000Z")
    );
    assert_eq!(
        ssl["certificates"][0]["updated_at"],
        json!("1970-01-01T00:00:00.000Z")
    );
}

#[test]
fn ssl_certificate_timestamps_match_node_normalization() {
    assert_eq!(
        normalize_timestamp(Some(&json!("2026-07-07T10:18:23.946511792Z"))),
        Some("2026-07-07T10:18:23.946Z".to_string())
    );
    assert_eq!(normalize_timestamp(Some(&json!("invalid"))), None);

    let ssl = normalize_ssl_config(Some(&json!({
        "certificates": [
            {
                "id": "cert-1",
                "label": "Manual",
                "source": "manual",
                "cert": "CERT",
                "key": "KEY",
                "created_at": "2026-07-07T10:18:23Z",
                "updated_at": "invalid"
            }
        ]
    })));

    assert_eq!(
        ssl["certificates"][0]["created_at"],
        json!("2026-07-07T10:18:23.000Z")
    );
    assert_eq!(
        ssl["certificates"][0]["updated_at"],
        json!("2026-07-07T10:18:23.000Z")
    );
}

#[test]
fn ssl_status_omits_node_undefined_fields() {
    let status = local_ssl_status(&json!({
        "deployment_mode": "single_active",
        "certificates": [
            {
                "id": "manual-1",
                "label": "Manual",
                "source": "manual",
                "cert": "not a certificate",
                "key": "not a key",
                "created_at": "2026-07-07T00:00:00Z",
                "updated_at": "2026-07-07T00:00:00Z"
            }
        ]
    }));
    let object = status.as_object().unwrap();
    assert!(!object.contains_key("activeCertId"));
    assert!(!object.contains_key("certInfo"));

    let certificate = status["certificates"][0].as_object().unwrap();
    assert!(!certificate.contains_key("primary_domain"));
    assert!(!certificate.contains_key("source_ref_id"));
    assert!(!certificate.contains_key("certInfo"));

    let zh = Translator::new("zh-CN");
    let gateway_ok = build_gateway_status_payload(
        Some(json!({
            "enabled": true,
            "deployment_mode": "multi_sni",
            "certificates": []
        })),
        None,
        &zh,
    );
    assert!(!gateway_ok.as_object().unwrap().contains_key("sync_error"));

    let gateway_failed = build_gateway_status_payload(None, Some("read failed".to_string()), &zh);
    assert_eq!(gateway_failed["sync_error"], json!("read failed"));
}

#[test]
fn ca_status_rejects_unparseable_existing_certificate() {
    assert!(build_ca_status_payload("not a certificate").is_none());

    let payload = build_ca_status_payload(SAMPLE_CERT).expect("valid status payload");
    assert_eq!(payload["initialized"], json!(true));
    assert_eq!(payload["info"]["subject"], json!("CN=example.test"));
}

#[test]
fn ssl_save_sync_condition_matches_node() {
    assert!(should_sync_ssl_deployment_after_save(true, "single_active"));
    assert!(should_sync_ssl_deployment_after_save(false, "multi_sni"));
    assert!(!should_sync_ssl_deployment_after_save(
        false,
        "single_active"
    ));
    assert!(!should_sync_ssl_deployment_after_save(false, "bad"));
}

#[test]
fn ca_issue_save_payload_matches_node_fields() {
    let body = build_ca_issue_certificate_body(
        &["nas.example.test".to_string(), "192.168.1.10".to_string()],
        "CERT".to_string(),
        "KEY".to_string(),
    );

    assert_eq!(body.label.as_deref(), Some("nas.example.test"));
    assert_eq!(body.source.as_deref(), Some("ca"));
    assert_eq!(body.primary_domain, None);
    assert_eq!(body.source_ref_id, None);
    assert_eq!(body.activate, Some(true));
}

#[test]
fn localizes_ssl_route_errors_and_default_labels() {
    let zh = Translator::new("zh-CN");
    assert_eq!(
        ssl_route_text(&zh, "rootCaNotInitialized"),
        "本地 CA 尚未初始化"
    );
    assert_eq!(
        localize_ssl_error(&zh, &anyhow!("Root CA not initialized")),
        "本地 CA 尚未初始化"
    );
    assert_eq!(
        localize_ssl_error(&zh, &anyhow!("No hosts configured")),
        "域名列表为空，请先添加域名或 IP"
    );
    assert_eq!(ssl_route_text(&zh, "certReadFailed"), "读取 SSL 证书失败");
    assert_eq!(
        ssl_route_text(&zh, "certZipCreateFailed"),
        "创建 SSL 证书压缩包失败"
    );
    assert_eq!(ssl_route_text(&zh, "caInitFailed"), "初始化本地 CA 失败");
    assert_eq!(
        ssl_route_text(&zh, "caHostLoadFailed"),
        "读取本地 CA Host 列表失败"
    );
    assert_eq!(
        ssl_error_or_route_text(&zh, "caInitFailed", &anyhow!("openssl command failed")),
        "初始化本地 CA 失败"
    );
    assert_eq!(
        ssl_error_or_route_text(
            &zh,
            "certSaveFailed",
            &anyhow!("Certificate format is invalid")
        ),
        "证书或私钥无效"
    );
    assert_eq!(
        validate_ssl_cert_for_response("", "", &zh).unwrap_err(),
        "证书内容不能为空"
    );
    assert_eq!(
        shared_file_error_status_and_message(&zh, &anyhow!("Invalid shared file path")),
        (StatusCode::BAD_REQUEST, "非法的共享文件路径".to_string())
    );
    assert_eq!(
        shared_file_error_status_and_message(&zh, &anyhow!("Shared directory is not configured")),
        (
            StatusCode::NOT_FOUND,
            "未找到飞牛共享目录，请确认应用资源已正确配置".to_string()
        )
    );
    assert_eq!(
        shared_file_error_status_and_message(&zh, &anyhow!("Shared path must be a file")),
        (
            StatusCode::BAD_REQUEST,
            "只能读取共享目录中的文件".to_string()
        )
    );
    assert_eq!(
        shared_file_error_status_and_message(&zh, &anyhow!("Shared file is too large")),
        (
            StatusCode::BAD_REQUEST,
            "文件过大，请仅放入证书或私钥文本文件".to_string()
        )
    );
    assert_eq!(
        shared_file_error_status_and_message(&zh, &anyhow!(SharedFileForbidden)),
        (StatusCode::FORBIDDEN, "读取共享目录文件失败".to_string())
    );
    assert_eq!(default_certificate_label("manual", None), "手动上传证书");
    assert_eq!(default_certificate_label("ca", None), "自签发证书");
    assert_eq!(
        default_certificate_label("acme", Some("example.com")),
        "example.com"
    );
}

#[test]
fn validates_ssl_certificate_private_key_match_like_node() {
    let Some((cert, key)) = generate_test_cert_pair("match.example.test") else {
        return;
    };
    let Some((_other_cert, other_key)) = generate_test_cert_pair("other.example.test") else {
        return;
    };
    let zh = Translator::new("zh-CN");

    assert!(validate_ssl_cert_for_response(&cert, &key, &zh).is_ok());
    assert_eq!(
        validate_ssl_cert_for_response(&cert, &other_key, &zh).unwrap_err(),
        "证书与私钥不匹配"
    );
}

fn generate_test_cert_pair(common_name: &str) -> Option<(String, String)> {
    if !Command::new("openssl")
        .arg("version")
        .stdin(Stdio::null())
        .output()
        .ok()?
        .status
        .success()
    {
        return None;
    }
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-ssl-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).ok()?;
    let key_path = temp_dir.join("key.pem");
    let cert_path = temp_dir.join("cert.pem");
    let result = (|| {
        run_openssl(vec![
            "req".to_string(),
            "-x509".to_string(),
            "-newkey".to_string(),
            "rsa:2048".to_string(),
            "-sha256".to_string(),
            "-days".to_string(),
            "1".to_string(),
            "-nodes".to_string(),
            "-keyout".to_string(),
            key_path.to_string_lossy().to_string(),
            "-out".to_string(),
            cert_path.to_string_lossy().to_string(),
            "-subj".to_string(),
            format!("/CN={common_name}"),
        ])
        .ok()?;
        Some((
            std::fs::read_to_string(&cert_path).ok()?,
            std::fs::read_to_string(&key_path).ok()?,
        ))
    })();
    let _ = std::fs::remove_dir_all(temp_dir);
    result
}

#[test]
fn builds_gateway_deployment_for_multi_sni_with_active_first() {
    let deployment = build_gateway_ssl_deployment(Some(&json!({
        "active_cert_id": "b",
        "deployment_mode": "multi_sni",
        "certificates": [
            {"id":"a","label":"A","cert":"CERTA","key":"KEYA"},
            {"id":"b","label":"B","cert":"CERTB","key":"KEYB"}
        ]
    })));
    assert_eq!(deployment["deployment_mode"], json!("multi_sni"));
    assert_eq!(deployment["certificates"][0]["id"], json!("b"));
    assert_eq!(deployment["certificates"][0]["is_default"], json!(true));
}

#[test]
fn parses_certificate_info_when_pem_is_valid() {
    let info = parse_cert_info(SAMPLE_CERT).expect("certificate should parse");
    assert_eq!(info["subject"], json!("CN=example.test"));
    assert_eq!(info["issuer"], json!("CN=example.test"));
    assert_eq!(info["validFrom"], json!("Jul  4 20:51:37 2026 GMT"));
    assert_eq!(info["validTo"], json!("Jul  1 20:51:37 2036 GMT"));
    assert_eq!(info["serialNumber"], json!("C06F8520FFAF6798"));
    assert_eq!(info["dnsNames"][0], json!("example.test"));
    assert_eq!(info["dnsNames"][1], json!("alt.example.test"));
}

#[test]
fn builds_subdomain_certificate_coverage_like_node() {
    let zh = Translator::new("zh-CN");
    let config = json!({
        "subdomain_mode": {
            "root_domain": "example.com"
        },
        "host_mappings": [
            {
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "service_role": "auth"
            },
            {
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080"
            }
        ]
    });
    let certificate_domains = vec!["example.com".to_string(), "*.example.com".to_string()];

    let coverage = build_subdomain_certificate_coverage(7997, &config, &certificate_domains, &zh);

    assert_eq!(coverage["status"], json!("ready"));
    assert_eq!(coverage["covers_auth_host"], json!(true));
    assert_eq!(
        coverage["covered_hosts"],
        json!(["auth.example.com", "app.example.com"])
    );
    assert_eq!(coverage["uncovered_hosts"], json!([]));
}

#[test]
fn subdomain_inventory_suggests_single_fully_covering_certificate() {
    let zh = Translator::new("zh-CN");
    let config = json!({
        "subdomain_mode": {
            "root_domain": "example.com"
        },
        "host_mappings": [
            {
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "service_role": "auth"
            },
            {
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080"
            }
        ]
    });
    let certificates = vec![
        CertificateCoverageInput {
            id: "old".to_string(),
            certificate_domains: vec!["auth.example.com".to_string()],
        },
        CertificateCoverageInput {
            id: "recommended".to_string(),
            certificate_domains: vec!["example.com".to_string(), "*.example.com".to_string()],
        },
    ];

    let coverage = build_subdomain_certificate_inventory_coverage(
        7997,
        &config,
        &certificates,
        Some("old"),
        "single_active",
        &zh,
    );

    assert_eq!(coverage["status"], json!("ready"));
    assert_eq!(coverage["can_auto_activate"], json!(true));
    assert_eq!(coverage["suggested_certificate_id"], json!("recommended"));
    assert_eq!(
        coverage["fully_covering_certificate_ids"],
        json!(["recommended"])
    );
    assert_eq!(
        coverage["partially_covering_certificate_ids"],
        json!(["old"])
    );
    assert!(
        coverage["warnings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|warning| {
                warning.as_str() == Some("当前活动证书与子域模式不完全匹配，建议切换到推荐证书。")
            })
    );
}

#[test]
fn builds_ca_server_cert_config_with_dns_and_ip_sans() {
    let config = openssl_server_cert_config(&[
        "example.test".to_string(),
        "192.168.1.10".to_string(),
        "alt.example.test".to_string(),
    ]);
    assert!(config.contains("CN = example.test"));
    assert!(config.contains("DNS.1 = example.test"));
    assert!(config.contains("IP.1 = 192.168.1.10"));
    assert!(config.contains("DNS.2 = alt.example.test"));
}

#[test]
fn cleans_openssl_dn_value_newlines() {
    assert_eq!(openssl_dn_value("example\n.test\r"), "example.test");
}
