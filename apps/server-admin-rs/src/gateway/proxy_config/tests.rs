use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn proxy_config_test_state(go_backend_grpc_addr: String) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, go_backend_grpc_addr);
    let state = AppState::new(settings).await.unwrap();
    (directory, state)
}

fn proxy_config_test_settings(
    directory: &tempfile::TempDir,
    go_backend_grpc_addr: String,
) -> crate::settings::Settings {
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.runtime_target = "linux".to_string();
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = go_backend_grpc_addr;
    settings.internal_rpc_token = "test-internal-rpc-token".to_string();
    settings.request_timeout = Duration::from_millis(100);
    settings
}

fn config_without_internal_metadata(mut config: Value) -> Value {
    crate::store::strip_internal_config_metadata(&mut config);
    config
}

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
            "protocol_mode": "http1",
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
        mapping.get("protocol_mode").and_then(Value::as_str),
        Some("http1")
    );
    assert_eq!(
        mapping_value.pointer("/basic_auth/enabled"),
        Some(&Value::Bool(true))
    );
    let payload = build_host_rules_payload(&mappings);
    assert_eq!(
        payload.pointer("/0/protocol_mode").and_then(Value::as_str),
        Some("http1")
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
fn rejects_explicit_invalid_host_protocol_mode() {
    for protocol_mode in [Value::Null, json!("h3"), json!(1)] {
        let error = normalize_host_mappings_for_route(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "protocol_mode": protocol_mode,
            })],
            &json!({}),
        )
        .unwrap_err();
        assert!(error.contains("protocol mode must be auto, http1 or http2"));
    }
}

#[test]
fn defaults_missing_host_protocol_mode_to_auto() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
        })],
        &json!({}),
    )
    .unwrap();
    assert_eq!(
        mappings[0].get("protocol_mode").and_then(Value::as_str),
        Some("auto")
    );
}

#[test]
fn normalizes_host_protocol_mode_case_and_whitespace() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": " HTTP2 ",
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(
        mappings[0].get("protocol_mode").and_then(Value::as_str),
        Some("http2")
    );
}

#[test]
fn preserves_previous_host_protocol_mode_when_legacy_request_omits_it() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
        })],
        &json!({
            "host_mappings": [{
                "host": "video.example.com",
                "target": "http://127.0.0.1:8080",
                "protocol_mode": "http1",
            }]
        }),
    )
    .unwrap();

    assert_eq!(
        mappings[0].get("protocol_mode").and_then(Value::as_str),
        Some("http1")
    );
}

#[test]
fn canonicalizes_host_ports_and_rejects_duplicates() {
    assert_eq!(
        normalize_host_value("HTTPS://Video.Example.com:443/path"),
        "video.example.com"
    );
    let error = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "video.example.com",
                "target": "http://127.0.0.1:8080",
            }),
            json!({
                "host": "VIDEO.EXAMPLE.COM:443",
                "target": "http://127.0.0.1:8081",
            }),
        ],
        &json!({}),
    )
    .unwrap_err();
    assert!(error.contains("Duplicate host mapping video.example.com"));
}

#[test]
fn validates_go_backend_echoed_protocol_modes() {
    let requested = json!([{
        "host": "video.example.com",
        "protocol_mode": "http1"
    }]);
    let applied = json!({
        "success": true,
        "data": [{"host": "video.example.com", "protocol_mode": "http1"}]
    });
    ensure_go_host_protocol_modes_applied(&requested, &applied).unwrap();

    let old_backend = json!({
        "success": true,
        "data": [{"host": "video.example.com"}]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &old_backend).unwrap_err();
    assert!(error.contains("did not apply HTTPS protocol mode http1"));

    let automatic = json!([{
        "host": "video.example.com",
        "protocol_mode": "auto"
    }]);
    ensure_go_host_protocol_modes_applied(&automatic, &old_backend).unwrap();
}

#[test]
fn rejects_go_backend_echoed_unexpected_hosts() {
    let requested = json!([{
        "host": "video.example.com",
        "protocol_mode": "http1"
    }]);
    let response_with_stale_host = json!({
        "success": true,
        "data": [
            {"host": "video.example.com", "protocol_mode": "http1"},
            {"host": "stale.example.com", "protocol_mode": "auto"}
        ]
    });
    let error =
        ensure_go_host_protocol_modes_applied(&requested, &response_with_stale_host).unwrap_err();
    assert!(error.contains("retained unexpected host mapping stale.example.com"));

    let empty_replacement = json!([]);
    let stale_only_response = json!({
        "success": true,
        "data": [{"host": "stale.example.com", "protocol_mode": "auto"}]
    });
    let error = ensure_go_host_protocol_modes_applied(&empty_replacement, &stale_only_response)
        .unwrap_err();
    assert!(error.contains("retained unexpected host mapping stale.example.com"));
}

#[test]
fn rejects_duplicate_canonical_hosts_in_go_host_rules_exchange() {
    let duplicate_request = json!([
        {"host": "video.example.com", "protocol_mode": "http1"},
        {"host": "VIDEO.EXAMPLE.COM:443", "protocol_mode": "http1"}
    ]);
    let error = ensure_go_host_protocol_modes_applied(
        &duplicate_request,
        &json!({"success": true, "data": []}),
    )
    .unwrap_err();
    assert!(error.contains(
        "Host-rules request contains duplicate canonical host mapping video.example.com"
    ));

    let requested = json!([{"host": "video.example.com", "protocol_mode": "http1"}]);
    let duplicate_response = json!({
        "success": true,
        "data": [
            {"host": "video.example.com", "protocol_mode": "http1"},
            {"host": "VIDEO.EXAMPLE.COM:443", "protocol_mode": "http1"}
        ]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &duplicate_response).unwrap_err();
    assert!(error.contains(
        "Go backend response contains duplicate canonical host mapping video.example.com"
    ));
}

#[test]
fn host_mapping_revision_tracks_user_config_but_ignores_fetched_metadata() {
    let initial = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1",
        "title": "Old title",
        "favicon": "old.ico"
    })];
    let metadata_only = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1",
        "title": "New title",
        "favicon": "new.ico"
    })];
    let changed_mode = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http2",
        "title": "New title",
        "favicon": "new.ico"
    })];

    assert_eq!(
        host_mappings_revision(&initial),
        host_mappings_revision(&metadata_only)
    );
    assert_ne!(
        host_mappings_revision(&initial),
        host_mappings_revision(&changed_mode)
    );
}

#[test]
fn normalizes_host_mapping_disabled_and_availability() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "disabled": true,
            "availability": {
                "enabled": true,
                "start_time": " 22:00 ",
                "end_time": "06:00"
            }
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(
        mappings[0].get("disabled").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        mappings[0]
            .pointer("/availability/start_time")
            .and_then(Value::as_str),
        Some("22:00")
    );
    assert_eq!(
        mappings[0]
            .pointer("/availability/end_time")
            .and_then(Value::as_str),
        Some("06:00")
    );

    let payload = build_host_rules_payload(&mappings);
    assert_eq!(
        payload.pointer("/0/disabled").and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        payload
            .pointer("/0/availability/start_time")
            .and_then(Value::as_str),
        Some("22:00")
    );
}

#[test]
fn normalizes_disabled_availability_to_null() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "availability": {
                "enabled": false,
                "start_time": "09:00",
                "end_time": "18:00"
            }
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(mappings[0].get("availability"), Some(&Value::Null));
}

#[test]
fn validates_host_availability_window_once() {
    assert_eq!(validate_host_availability_window("09:00", "18:00"), Ok(()));
    assert_eq!(validate_host_availability_window("22:00", "06:00"), Ok(()));
    assert_eq!(
        validate_host_availability_window("9:00", "18:00"),
        Err(HostAvailabilityWindowError::InvalidStart)
    );
    assert_eq!(
        validate_host_availability_window("09:00", "09:00"),
        Err(HostAvailabilityWindowError::Same)
    );
}

#[test]
fn rejects_equal_host_mapping_availability_times() {
    let error = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "availability": {
                "enabled": true,
                "start_time": "09:00",
                "end_time": "09:00"
            }
        })],
        &json!({}),
    )
    .unwrap_err();

    assert!(error.contains("must be different"));
}

#[test]
fn preserves_host_location_target_path_for_gateway_payload() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "locations": [
                {
                    "path": "/api/http",
                    "match": "prefix",
                    "action": "proxy",
                    "target": " http://192.168.9.100:3043/ ",
                    "strip_path": true,
                    "rewrite_html": false
                },
                {
                    "path": "/api/base",
                    "match": "prefix",
                    "action": "proxy",
                    "target": "http://192.168.9.100:3043/base/",
                    "strip_path": true,
                    "rewrite_html": false
                }
            ]
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(
        mappings[0]
            .pointer("/locations/0/target")
            .and_then(Value::as_str),
        Some("http://192.168.9.100:3043/")
    );
    assert_eq!(
        mappings[0]
            .pointer("/locations/1/target")
            .and_then(Value::as_str),
        Some("http://192.168.9.100:3043/base/")
    );

    let payload = build_host_rules_payload(&mappings);
    assert_eq!(
        payload
            .pointer("/0/locations/0/target")
            .and_then(Value::as_str),
        Some("http://192.168.9.100:3043/")
    );
    assert_eq!(
        payload
            .pointer("/0/locations/0/strip_path")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert_eq!(
        payload
            .pointer("/0/locations/1/target")
            .and_then(Value::as_str),
        Some("http://192.168.9.100:3043/base/")
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

#[tokio::test]
async fn manual_metadata_refresh_rolls_back_config_when_runtime_sync_fails() {
    let metadata_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let metadata_addr = metadata_listener.local_addr().unwrap();
    let (metadata_requested_tx, metadata_requested_rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = metadata_listener.accept().await else {
            return;
        };
        let mut buffer = [0_u8; 2048];
        let Ok(read_len) = socket.read(&mut buffer).await else {
            return;
        };
        if read_len == 0 {
            return;
        }
        let _ = metadata_requested_tx.send(());
        let body = br#"<!doctype html><title>After refresh</title><link rel="icon" href="data:image/png;base64,AQID">"#;
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = socket.write_all(header.as_bytes()).await;
        let _ = socket.write_all(body).await;
    });

    let unavailable_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable_grpc_addr = unavailable_listener.local_addr().unwrap();
    drop(unavailable_listener);

    let (_directory, state) = proxy_config_test_state(unavailable_grpc_addr.to_string()).await;

    let previous_config = json!({
        "run_type": 3,
        "host_mappings": [{
            "host": "video.example.com",
            "target": format!("http://{metadata_addr}/"),
            "protocol_mode": "http1",
            "title": "Before refresh",
            "favicon": "before.ico"
        }]
    });
    state.store.save_config(&previous_config).await.unwrap();

    let response = refresh_host_mapping_titles(State(state.clone())).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    metadata_requested_rx.await.unwrap();
    assert_eq!(
        config_without_internal_metadata(state.store.get_config().await.unwrap()),
        previous_config
    );
}

#[tokio::test]
async fn host_mapping_rollback_replays_previous_runtime_payload_after_restoring_store() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let previous_config = json!({
        "run_type": 3,
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "http1",
            "title": "Before refresh"
        }]
    });
    let changed_config = json!({
        "run_type": 3,
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "http2",
            "title": "After refresh"
        }]
    });
    state.store.save_config(&changed_config).await.unwrap();

    let runtime_calls = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let captured_calls = runtime_calls.clone();
    let changed_mappings = changed_config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    rollback_host_mappings_with_runtime_sync(
        &state,
        &previous_config,
        &changed_mappings,
        move |state, config, mappings| async move {
            let stored = state
                .store
                .get_config()
                .await
                .map_err(|error| error.to_string())?;
            captured_calls.lock().await.push((config, mappings, stored));
            Ok(())
        },
    )
    .await;

    let calls = runtime_calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(
        config_without_internal_metadata(calls[0].0.clone()),
        previous_config
    );
    assert_eq!(
        calls[0].1,
        previous_config
            .get("host_mappings")
            .and_then(Value::as_array)
            .cloned()
            .unwrap()
    );
    assert_eq!(
        config_without_internal_metadata(calls[0].2.clone()),
        previous_config
    );
    assert_eq!(
        config_without_internal_metadata(state.store.get_config().await.unwrap()),
        previous_config
    );
}

#[tokio::test]
async fn host_mapping_rollback_does_not_overwrite_a_newer_mapping_commit() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let previous_config = json!({
        "run_type": 3,
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "auto"
        }]
    });
    let failed_update = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    let newer_config = json!({
        "run_type": 3,
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "http2"
        }]
    });
    state.store.save_config(&newer_config).await.unwrap();

    let runtime_calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let captured_calls = runtime_calls.clone();
    rollback_host_mappings_with_runtime_sync(
        &state,
        &previous_config,
        &failed_update,
        move |_state, _config, _mappings| async move {
            captured_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        },
    )
    .await;

    assert_eq!(runtime_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert_eq!(
        config_without_internal_metadata(state.store.get_config().await.unwrap()),
        newer_config
    );
}

#[tokio::test]
async fn host_mapping_cas_is_shared_across_app_states_and_preserves_other_sections() {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, "127.0.0.1:1".to_string());
    let first_state = AppState::new(settings.clone()).await.unwrap();
    let second_state = AppState::new(settings).await.unwrap();

    let expected = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "auto"
    })];
    first_state
        .store
        .save_config(&json!({
            "host_mappings": expected,
            "unrelated": { "generation": 1 },
            "run_type": 3
        }))
        .await
        .unwrap();
    let expected = first_state
        .store
        .get_config()
        .await
        .unwrap()
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let first_revision = host_mappings_revision(&expected);
    let second_revision = host_mappings_revision(
        second_state.store.get_config().await.unwrap()["host_mappings"]
            .as_array()
            .unwrap(),
    );
    assert_eq!(first_revision, second_revision);

    // Commit an unrelated section from the second state after both writers
    // obtained the same host-mapping revision. The section CAS must merge into
    // this latest full document instead of restoring generation 1.
    let mut unrelated_update = second_state.store.get_config().await.unwrap();
    unrelated_update["unrelated"]["generation"] = json!(2);
    second_state
        .store
        .save_config(&unrelated_update)
        .await
        .unwrap();

    let first_replacement = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    let second_replacement = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http2"
    })];
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(3));
    let first_task = {
        let state = first_state.clone();
        let barrier = barrier.clone();
        let expected = expected.clone();
        let replacement = first_replacement.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            state
                .store
                .compare_and_set_host_mappings(&expected, &replacement)
                .await
                .unwrap()
        })
    };
    let second_task = {
        let state = second_state.clone();
        let barrier = barrier.clone();
        let expected = expected.clone();
        let replacement = second_replacement.clone();
        tokio::spawn(async move {
            barrier.wait().await;
            state
                .store
                .compare_and_set_host_mappings(&expected, &replacement)
                .await
                .unwrap()
        })
    };
    barrier.wait().await;
    let first_result = first_task.await.unwrap();
    let second_result = second_task.await.unwrap();

    assert_ne!(first_result.is_some(), second_result.is_some());
    let final_config = first_state.store.get_config().await.unwrap();
    assert_eq!(final_config["unrelated"]["generation"], json!(2));
    let final_mappings = final_config["host_mappings"].as_array().unwrap();
    assert!(final_mappings == &first_replacement || final_mappings == &second_replacement);
}

#[tokio::test]
async fn host_mapping_transaction_lease_serializes_independent_app_states() {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, "127.0.0.1:1".to_string());
    let first_state = AppState::new(settings.clone()).await.unwrap();
    let second_state = AppState::new(settings).await.unwrap();

    let first_lease = acquire_host_mappings_transaction_lease(&first_state)
        .await
        .unwrap()
        .expect("first state acquires transaction lease");
    let second_task = tokio::spawn(async move {
        acquire_host_mappings_transaction_lease(&second_state)
            .await
            .unwrap()
            .expect("second state eventually acquires transaction lease")
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(
        !second_task.is_finished(),
        "a distinct AppState must not enter the config/runtime transaction concurrently"
    );

    assert!(first_lease.release().await.unwrap());
    let second_lease = tokio::time::timeout(Duration::from_secs(1), second_task)
        .await
        .expect("second state acquires after release")
        .unwrap();
    assert!(second_lease.ensure_valid().await.unwrap());
    assert!(second_lease.release().await.unwrap());
}

#[test]
fn host_rules_runtime_payload_follows_run_type_and_reverse_proxy_submode() {
    let mappings = json!([{
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    }]);
    let config = |run_type, reverse_proxy_submode| {
        json!({
            "run_type": run_type,
            "reverse_proxy_submode": reverse_proxy_submode,
            "host_mappings": mappings
        })
    };

    assert!(
        super::runtime::host_rules_payload_for_config(&config(3, "path")).is_some(),
        "protocol-mapping mode must install HostRules"
    );
    assert!(
        super::runtime::host_rules_payload_for_config(&config(1, "subdomain")).is_some(),
        "reverse-proxy subdomain mode must install HostRules"
    );
    assert!(
        super::runtime::host_rules_payload_for_config(&config(1, "path")).is_none(),
        "reverse-proxy path mode must flush HostRules"
    );
    assert!(
        super::runtime::host_rules_payload_for_config(&config(0, "subdomain")).is_none(),
        "direct mode must flush HostRules"
    );
}

#[tokio::test]
async fn host_mapping_lease_loss_is_reported_by_release_and_runtime_transaction() {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, "127.0.0.1:1".to_string());
    let state = AppState::new(settings).await.unwrap();

    let lease = acquire_host_mappings_transaction_lease(&state)
        .await
        .unwrap()
        .unwrap();
    state
        .store
        .set_json_value(
            HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
            &json!({ "lockId": "new-owner" }),
        )
        .await
        .unwrap();
    assert!(lease.release().await.is_err());
    state
        .store
        .delete_key(HOST_MAPPINGS_TRANSACTION_LOCK_KEY)
        .await
        .unwrap();

    let result = with_host_mappings_runtime_transaction(&state, |state| async move {
        state
            .store
            .set_json_value(
                HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
                &json!({ "lockId": "replacement-owner" }),
            )
            .await
            .map_err(|error| error.to_string())?;
        Ok(())
    })
    .await;
    assert!(result.unwrap_err().contains("lease ownership was lost"));
    state
        .store
        .delete_key(HOST_MAPPINGS_TRANSACTION_LOCK_KEY)
        .await
        .unwrap();
}

#[tokio::test]
async fn delayed_runtime_host_sync_reads_latest_mapping_after_acquiring_lease() {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, "127.0.0.1:1".to_string());
    let mutation_state = AppState::new(settings.clone()).await.unwrap();
    let runtime_state = AppState::new(settings).await.unwrap();
    let initial_mappings = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "auto"
    })];
    mutation_state
        .store
        .save_config(&json!({ "host_mappings": initial_mappings }))
        .await
        .unwrap();
    let initial_mappings = mutation_state.store.get_config().await.unwrap()["host_mappings"]
        .as_array()
        .cloned()
        .unwrap();
    let mutation_lease = acquire_host_mappings_transaction_lease(&mutation_state)
        .await
        .unwrap()
        .unwrap();
    let (captured_tx, captured_rx) = tokio::sync::oneshot::channel();
    let runtime_task = tokio::spawn(async move {
        with_host_mappings_runtime_transaction(&runtime_state, move |state| async move {
            let mappings = state.store.get_config().await.unwrap()["host_mappings"]
                .as_array()
                .cloned()
                .unwrap();
            captured_tx.send(mappings).unwrap();
            Ok(())
        })
        .await
    });
    tokio::time::sleep(Duration::from_millis(40)).await;
    assert!(!runtime_task.is_finished());

    let latest_mappings = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    mutation_state
        .store
        .compare_and_set_host_mappings(&initial_mappings, &latest_mappings)
        .await
        .unwrap()
        .unwrap();
    mutation_lease.release().await.unwrap();

    runtime_task.await.unwrap().unwrap();
    assert_eq!(captured_rx.await.unwrap(), latest_mappings);
}

#[tokio::test]
async fn stale_full_config_writer_preserves_new_host_mapping_across_app_states() {
    let directory = tempfile::tempdir().unwrap();
    let settings = proxy_config_test_settings(&directory, "127.0.0.1:1".to_string());
    let host_state = AppState::new(settings.clone()).await.unwrap();
    let full_writer_state = AppState::new(settings).await.unwrap();
    let initial_mappings = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "auto"
    })];
    host_state
        .store
        .save_config(&json!({
            "host_mappings": initial_mappings,
            "unrelated": { "generation": 1 }
        }))
        .await
        .unwrap();

    // This snapshot carries generation N and is intentionally held until
    // after another AppState commits host generation N+1.
    let mut stale_full_config = full_writer_state.store.get_config().await.unwrap();
    let initial_mappings = stale_full_config["host_mappings"]
        .as_array()
        .cloned()
        .unwrap();
    let next_mappings = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1"
    })];
    host_state
        .store
        .compare_and_set_host_mappings(&initial_mappings, &next_mappings)
        .await
        .unwrap()
        .expect("host section CAS succeeds");

    stale_full_config["unrelated"]["generation"] = json!(2);
    let stale_save = full_writer_state
        .store
        .save_config(&stale_full_config)
        .await;
    assert!(stale_save.is_err());

    let final_config = host_state.store.get_config().await.unwrap();
    assert_eq!(final_config["host_mappings"], json!(next_mappings));
    assert_eq!(final_config["unrelated"]["generation"], json!(1));
    assert_eq!(
        host_state
            .store
            .get_string_value("fn_knock:config:host_mappings:generation")
            .await
            .unwrap()
            .as_deref(),
        Some("2")
    );
    let persisted_raw = host_state
        .store
        .get_string_value("fn_knock:config")
        .await
        .unwrap()
        .unwrap();
    assert!(!persisted_raw.contains(crate::store::CONFIG_GENERATION_MARKER));
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
    assert_eq!(
        localize_proxy_config_error(&translator, "Duplicate host mapping video.example.com"),
        "Host 映射域名 video.example.com 重复"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping video.example.com HTTPS protocol mode must be auto, http1 or http2"
        ),
        "Host 映射 video.example.com 的 HTTPS 协议必须是 auto、http1 或 http2"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Go backend did not apply HTTPS protocol mode http1 for video.example.com (reported auto); upgrade the gateway backend"
        ),
        "网关后端未应用 video.example.com 的 HTTPS 协议 http1，请升级网关后端"
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
