use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[test]
fn validates_supported_proxy_target_urls() {
    assert!(is_supported_proxy_target_url("http://127.0.0.1:8080"));
    assert!(is_supported_proxy_target_url("wss://example.com/socket"));
    assert!(!is_supported_proxy_target_url("ftp://example.com"));
    assert!(!is_supported_proxy_target_url("http://example.com:"));
    assert!(!is_supported_proxy_target_url("http://"));
}

#[test]
fn normalizes_proxy_mapping_targets_without_touching_other_fields() {
    let mappings = normalize_proxy_mappings(vec![json!({
        "path": "/",
        "target": " http://127.0.0.1:8080 ",
        "rewrite_html": true,
        "use_auth": false,
        "use_root_mode": false,
        "strip_path": false
    })])
    .unwrap();
    assert_eq!(
        mappings[0].get("target").and_then(Value::as_str),
        Some("http://127.0.0.1:8080")
    );
    assert_eq!(mappings[0].get("rewrite_html"), Some(&Value::Bool(true)));
}

#[test]
fn normalizes_host_mapping_route_shape() {
    let config = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "Old title",
            "favicon": "old.ico",
            "basic_auth": { "enabled": true, "username": "old", "password": "pw" }
        }]
    });
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "HTTPS://App.Example.Com/path",
            "target": " http://127.0.0.1:8080 ",
            "use_auth": true,
            "access_mode": "strict_whitelist",
            "locations": [{
                "path": "/api/../health",
                "match": "exact",
                "action": "response",
                "response": {
                    "status": 204,
                    "headers": { "X-Test": "ok" }
                }
            }]
        })],
        &config,
    )
    .unwrap();
    let mapping_value = &mappings[0];
    let mapping = mapping_value.as_object().unwrap();
    assert_eq!(
        mapping.get("host").and_then(Value::as_str),
        Some("app.example.com")
    );
    assert_eq!(
        mapping.get("title").and_then(Value::as_str),
        Some("Old title")
    );
    assert_eq!(
        mapping_value.pointer("/basic_auth/enabled"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        mapping_value
            .pointer("/locations/0/path")
            .and_then(Value::as_str),
        Some("/health")
    );
    assert_eq!(
        mapping_value.pointer("/locations/0/response/headers/X-Test"),
        Some(&Value::String("ok".to_string()))
    );
}

#[test]
fn extracts_host_mapping_metadata_helpers() {
    assert!(has_basic_auth_challenge(Some(
        "Bearer token, Basic realm=\"admin\""
    )));
    assert!(has_basic_auth_challenge(Some("basic")));
    assert!(!has_basic_auth_challenge(Some("Digest realm=\"admin\"")));
    assert_eq!(
        normalize_http_probe_url("https://example.com/app#fragment").as_deref(),
        Some("https://example.com/app")
    );
    assert_eq!(
        extract_html_title("<html><title> Fn &amp; Knock &#x4e2d; </title></html>"),
        "Fn & Knock 中"
    );
    assert_eq!(
        extract_favicon_url(
            r#"<link rel="shortcut icon" href="/assets/favicon.ico">"#,
            "https://example.com/ui/"
        )
        .as_deref(),
        Some("https://example.com/assets/favicon.ico")
    );
}

#[test]
fn extracts_favicon_candidates_like_node_metadata() {
    let html = r#"
            <base href="https://static.example.com/app/">
            <link rel="apple-touch-icon" sizes="180x180" href="touch.png">
            <link rel="icon" type="image/svg+xml" sizes="any" href="favicon.svg">
        "#;
    assert_eq!(
        extract_favicon_url(html, "https://example.com/ui/").as_deref(),
        Some("https://static.example.com/app/favicon.svg")
    );

    let heuristic_html = r#"
            <meta name="msapplication-TileImage" content="/mstile-150x150.png">
            <img src="/logo.png">
            <img data-favicon="/assets/favicon-32.png">
        "#;
    let candidates = extract_heuristic_favicon_urls_from_html(
        heuristic_html,
        "https://example.com/admin/",
        HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    assert_eq!(
        candidates.first().map(String::as_str),
        Some("https://example.com/assets/favicon-32.png")
    );
    assert!(
        candidates
            .iter()
            .any(|value| value == "https://example.com/mstile-150x150.png")
    );
}

#[test]
fn extracts_manifest_icons_like_node_metadata() {
    let manifest_url = "https://example.com/app/manifest.webmanifest";
    let manifest = json!({
        "icons": [
            { "src": "/icon-maskable.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" },
            { "src": "icon-any.png", "sizes": "192x192", "type": "image/png", "purpose": "any" },
            { "src": "/not-image.txt", "sizes": "512x512", "type": "text/plain" },
            { "src": "icon-any.png", "sizes": "192x192", "type": "image/png" }
        ]
    });
    assert_eq!(
        extract_manifest_icon_urls(&manifest, manifest_url),
        vec![
            "https://example.com/app/icon-any.png".to_string(),
            "https://example.com/icon-maskable.png".to_string(),
        ]
    );
    assert_eq!(
        extract_manifest_from_html(
            r#"<link rel="manifest" href="/site.webmanifest">"#,
            "https://example.com/app/"
        )
        .as_deref(),
        Some("https://example.com/site.webmanifest")
    );
}

#[test]
fn recognizes_openwrt_luci_and_fallback_favicon_paths() {
    let entrypoint = r#"
            <html><head>
              <meta http-equiv="refresh" content="0; url='/cgi-bin/luci/'">
            </head><body>LuCI - Lua Configuration Interface</body></html>
        "#;
    assert!(has_openwrt_luci_entrypoint_html(entrypoint));
    assert_eq!(
        extract_openwrt_luci_url_from_html(entrypoint, "https://router.example.com/").as_deref(),
        Some("https://router.example.com/cgi-bin/luci/")
    );

    let document = r#"
            <html><head>
              <title>OpenWrt LuCI</title>
              <link rel="stylesheet" href="/luci-static/bootstrap/cascade.css">
            </head></html>
        "#;
    assert!(has_openwrt_luci_document_html(document));
    assert_eq!(
        resolve_fallback_favicon_urls("https://example.com/path/page"),
        vec![
            "https://example.com/favicon.ico".to_string(),
            "https://example.com/img/favicon.ico".to_string(),
            "https://example.com/public/favicon.png".to_string(),
        ]
    );
}

#[test]
fn accepts_inline_and_same_origin_metadata_assets() {
    assert_eq!(
        normalize_favicon_url("data:image/png;base64,AA==", "https://example.com/").as_deref(),
        Some("data:image/png;base64,AA==")
    );

    let context = create_basic_auth_context(
        Some(&json!({
            "enabled": true,
            "username": "admin",
            "password": "pw"
        })),
        "https://example.com/app/",
    )
    .expect("basic auth context");
    assert!(has_same_origin(
        "https://example.com/assets/favicon.ico",
        &context.origin
    ));
    assert!(!has_same_origin(
        "https://cdn.example.com/assets/favicon.ico",
        &context.origin
    ));
}

#[tokio::test]
async fn fetches_metadata_manifest_icon_as_data_url_like_node() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for _ in 0..3 {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                let mut buffer = [0_u8; 2048];
                let Ok(read_len) = socket.read(&mut buffer).await else {
                    return;
                };
                let request = String::from_utf8_lossy(&buffer[..read_len]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("/");
                let (status, content_type, body): (&str, &str, Vec<u8>) = match path {
                        "/" => (
                            "200 OK",
                            "text/html; charset=utf-8",
                            br#"<!doctype html><title>Manifest App</title><link rel="manifest" href="/manifest.json">"#.to_vec(),
                        ),
                        "/manifest.json" => (
                            "200 OK",
                            "application/json",
                            br#"{"icons":[{"src":"/icon.png","sizes":"192x192","type":"image/png","purpose":"any"}]}"#.to_vec(),
                        ),
                        "/icon.png" => ("200 OK", "application/octet-stream", vec![1, 2, 3]),
                        _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
                    };
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });

    let metadata = fetch_host_mapping_metadata(&format!("http://{addr}/"), None)
        .await
        .unwrap();
    assert_eq!(
        metadata.get("title").and_then(Value::as_str),
        Some("Manifest App")
    );
    assert_eq!(
        metadata.get("favicon").and_then(Value::as_str),
        Some("data:image/png;base64,AQID")
    );
}

#[test]
fn host_mapping_metadata_refresh_decision_matches_node_save_rules() {
    let previous_mappings = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "title": "Old",
        "favicon": "old.ico",
        "basic_auth": disabled_host_basic_auth()
    })];
    let previous_by_host = previous_mappings
        .into_iter()
        .map(|mapping| (host_mapping_key(&mapping), mapping))
        .collect::<HashMap<_, _>>();

    assert_eq!(
        resolve_metadata_refresh_decision(
            &json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Old",
                "favicon": "old.ico",
                "basic_auth": disabled_host_basic_auth()
            }),
            &previous_by_host
        ),
        (false, false)
    );
    assert_eq!(
        resolve_metadata_refresh_decision(
            &json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "",
                "favicon": "old.ico",
                "basic_auth": disabled_host_basic_auth()
            }),
            &previous_by_host
        ),
        (true, false)
    );
    assert_eq!(
        resolve_metadata_refresh_decision(
            &json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:9090",
                "title": "Old",
                "favicon": "old.ico",
                "basic_auth": disabled_host_basic_auth()
            }),
            &previous_by_host
        ),
        (true, true)
    );
    assert_eq!(
        resolve_metadata_refresh_decision(
            &json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "Old",
                "favicon": "old.ico",
                "basic_auth": { "enabled": true, "username": "admin", "password": "pw" }
            }),
            &previous_by_host
        ),
        (true, true)
    );
    assert_eq!(
        resolve_metadata_refresh_decision(
            &json!({
                "host": "app.example.com",
                "target": "tcp://127.0.0.1:8080",
                "title": "",
                "favicon": "",
                "basic_auth": disabled_host_basic_auth()
            }),
            &previous_by_host
        ),
        (false, false)
    );
}

#[test]
fn host_mapping_metadata_merge_preserves_user_changes() {
    let refreshed = HostMappingMetadataRefreshItem {
        mapping: json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "Fetched",
            "favicon": "data:image/png;base64,AA==",
            "basic_auth": disabled_host_basic_auth()
        }),
        refresh_title: true,
        refresh_favicon: true,
    };

    let (changed_mappings, changed) = merge_metadata_into_current_mappings(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "Current",
            "favicon": "current.ico",
            "basic_auth": disabled_host_basic_auth()
        })],
        vec![refreshed.clone()],
    );
    assert!(changed);
    assert_eq!(
        changed_mappings[0].get("title").and_then(Value::as_str),
        Some("Fetched")
    );
    assert_eq!(
        changed_mappings[0].get("favicon").and_then(Value::as_str),
        Some("data:image/png;base64,AA==")
    );

    let (stale_target_mappings, changed) = merge_metadata_into_current_mappings(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:9090",
            "title": "Current",
            "favicon": "current.ico",
            "basic_auth": disabled_host_basic_auth()
        })],
        vec![refreshed.clone()],
    );
    assert!(!changed);
    assert_eq!(
        stale_target_mappings[0]
            .get("title")
            .and_then(Value::as_str),
        Some("Current")
    );

    let (stale_auth_mappings, changed) = merge_metadata_into_current_mappings(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "Current",
            "favicon": "current.ico",
            "basic_auth": { "enabled": true, "username": "admin", "password": "pw" }
        })],
        vec![refreshed],
    );
    assert!(!changed);
    assert_eq!(
        stale_auth_mappings[0]
            .get("favicon")
            .and_then(Value::as_str),
        Some("current.ico")
    );
}

#[test]
fn gateway_portal_title_mode_defaults_like_node() {
    assert!(is_gateway_portal_title_mode(&json!({})));
    assert!(is_gateway_portal_title_mode(&json!({
        "gateway_portal": { "display_style": "title" }
    })));
    assert!(!is_gateway_portal_title_mode(&json!({
        "gateway_portal": { "display_style": "domain" }
    })));
}

#[test]
fn builds_i18n_bookmarks_document_without_auth_mapping() {
    let config = json!({
        "run_type": 3,
        "ssl": {
            "cert": "-----BEGIN CERTIFICATE-----",
            "key": "-----BEGIN PRIVATE KEY-----"
        },
        "subdomain_mode": {
            "root_domain": "example.com",
            "public_https_port": 8443
        },
        "host_mappings": [
            {
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "title": "App",
                "title_override": "Portal"
            },
            {
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "title": "Auth"
            }
        ]
    });
    let document = build_bookmarks_document(&config, &crate::i18n::Translator::new("zh-CN"));

    assert!(document.contains("example.com 子域映射"));
    assert!(document.contains("https://app.example.com:8443/"));
    assert!(document.contains(">Portal</A>"));
    assert!(!document.contains("auth.example.com"));
    assert_eq!(
        build_bookmark_filename(&config),
        "fn-knock-bookmarks-example.com.html"
    );
}

#[test]
fn bookmark_url_port_suffix_matches_node_string_rules() {
    assert_eq!(
        build_bookmark_url("app.example.com", "https", Some("abc"), false),
        "https://app.example.com:abc/"
    );
    assert_eq!(
        build_bookmark_url("app.example.com", "https", Some("443x"), false),
        "https://app.example.com/"
    );
    assert_eq!(
        build_bookmark_url("app.example.com", "http", Some("80x"), false),
        "http://app.example.com/"
    );
    assert_eq!(
        build_bookmark_url("app.example.com", "https", Some(""), false),
        "https://app.example.com:7999/"
    );
    assert_eq!(
        build_bookmark_url("app.example.com", "https", Some("abc"), true),
        "https://app.example.com/"
    );
}

#[test]
fn auth_service_port_env_parser_matches_node_parse_int() {
    assert_eq!(parse_env_port_with_fallback_value(None, 7997), 7997);
    assert_eq!(
        parse_env_port_with_fallback_value(Some(String::new()), 7997),
        7997
    );
    assert_eq!(
        parse_env_port_with_fallback_value(Some(" 7997x ".to_string()), 7997),
        7997
    );
    assert_eq!(
        parse_env_port_with_fallback_value(Some("8000x".to_string()), 7997),
        8000
    );
    assert_eq!(
        parse_env_port_with_fallback_value(Some("0x10".to_string()), 7997),
        7997
    );
    assert_eq!(
        parse_env_port_with_fallback_value(Some("abc".to_string()), 7997),
        7997
    );
}

#[test]
fn validates_stream_mapping_duplicates() {
    let error = normalize_stream_mappings(vec![
        json!({ "protocol": "tcp", "listen_port": 2222, "target": "127.0.0.1:22" }),
        json!({ "listen_port": 2222, "target": "example.com:22" }),
    ])
    .unwrap_err();
    assert!(error.contains("Duplicate stream mapping"));
    assert!(
        normalize_stream_mappings(vec![json!({
            "protocol": "udp",
            "listen_port": 5353,
            "target": "[::1]:53",
            "use_auth": false
        })])
        .is_ok()
    );
}

#[test]
fn localizes_proxy_config_route_errors() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping app.example.com target must be a supported HTTP/WebSocket URL"
        ),
        "Host 映射 app.example.com 的目标必须以 http://、https://、ws:// 或 wss:// 开头并包含主机名"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping app.example.com location /api target must be a supported HTTP/WebSocket URL"
        ),
        "Host 映射 app.example.com 的路径规则 /api 目标必须以 http://、https://、ws:// 或 wss:// 开头并包含主机名"
    );
    assert_eq!(
        localize_proxy_config_error(&translator, "Duplicate stream mapping for TCP port 2222"),
        "TCP 监听端口 2222 重复，请保持协议 + 端口唯一"
    );
    assert_eq!(
        localize_proxy_config_error(&translator, "Only http/https targets are supported"),
        "仅支持 http/https 目标地址"
    );
}

#[test]
fn builds_gateway_auth_config_from_auth_mapping() {
    let config = json!({
        "run_type": 3,
        "reverse_proxy_submode": "host",
        "host_mappings": [{
            "host": "auth.example.com",
            "target": "http://127.0.0.1:7997"
        }],
        "subdomain_mode": {
            "auth_cache_ttl_seconds": 5,
            "auth_cache_unauthorized_ttl_seconds": 2,
            "edge_client_ip_enabled": true,
            "aliyun_esa_enabled": true,
            "tencent_edgeone_enabled": false,
            "public_auth_base_url": "",
            "public_http_port": 80,
            "public_https_port": 443
        }
    });
    let auth = build_gateway_auth_config(&config);
    assert_eq!(auth.get("auth_port").and_then(Value::as_i64), Some(7997));
    assert_eq!(
        auth.get("public_auth_base_url").and_then(Value::as_str),
        Some("https://auth.example.com")
    );
    assert_eq!(
        auth.get("auth_host").and_then(Value::as_str),
        Some("auth.example.com")
    );
    assert_eq!(
        auth.get("edge_client_ip_enabled").and_then(Value::as_bool),
        Some(true)
    );
}
