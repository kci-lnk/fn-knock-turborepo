use super::*;
use crate::test_support::EnvGuard;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[test]
fn normalizes_allowed_scan_cidrs_like_node() {
    assert_eq!(
        normalize_allowed_scan_cidrs([
            "192.168.1.99/24".to_string(),
            "8.8.8.8/32".to_string(),
            "192.168.1.0/24".to_string(),
        ]),
        vec!["192.168.1.0/24".to_string()]
    );
}

#[test]
fn validates_scan_limits() {
    let cidrs = vec!["10.0.0.0/16".to_string()];
    let error = validate_scan_cidrs(&cidrs).unwrap_err();
    assert!(error.contains("1024"));
}

#[test]
fn validates_scan_host_count_after_dedupe_like_node() {
    let cidrs = vec!["10.0.0.0/22".to_string(), "10.0.0.0/23".to_string()];
    assert_eq!(
        validate_scan_cidrs(&cidrs).unwrap(),
        vec!["10.0.0.0/22".to_string(), "10.0.0.0/23".to_string()]
    );
}

#[test]
fn interface_cidr_prefers_reported_prefix_before_node_fallback() {
    assert_eq!(
        build_interface_ipv4_cidr("192.168.1.2", Some(30)).as_deref(),
        Some("192.168.1.0/30")
    );
    assert_eq!(
        build_interface_ipv4_cidr("192.168.1.2", Some(16)).as_deref(),
        Some("192.168.1.0/24")
    );
}

#[test]
fn expands_scan_cidrs_and_scope() {
    let cidrs = vec!["192.168.1.0/30".to_string()];
    assert_eq!(
        expand_scan_cidrs(&cidrs),
        vec!["192.168.1.1".to_string(), "192.168.1.2".to_string()]
    );
    assert_eq!(build_scan_scope(&cidrs).as_deref(), Some("192.168.1.0/30"));
}

#[test]
fn discovery_host_groups_match_node_port_modes_and_self_skip() {
    let scan_cidrs = vec![
        LOOPBACK_DISCOVERY_CIDR.to_string(),
        "192.168.1.0/30".to_string(),
    ];
    let full_range_cidrs = vec!["192.168.1.0/30".to_string()];
    let self_scan_hosts = vec!["192.168.1.1".to_string()];

    let groups =
        build_discovery_host_groups(&scan_cidrs, &full_range_cidrs, None, &self_scan_hosts);

    assert_eq!(groups.len(), 3);
    assert!(
        groups
            .iter()
            .all(|group| group.mode == DiscoveryPortRangeMode::Full)
    );
    assert_eq!(
        count_discovery_scan_ports_for_groups(&groups, &[]),
        59_920 + 59_920 + 59_921
    );
    assert_eq!(
        build_discovery_port_mode_label(&scan_cidrs, &full_range_cidrs),
        "80-60000"
    );
}

#[test]
fn limited_discovery_uses_node_range_and_excluded_ports() {
    let scan_cidrs = vec!["192.168.2.0/30".to_string()];
    let groups = build_discovery_host_groups(&scan_cidrs, &[], None, &[]);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].mode, DiscoveryPortRangeMode::Limited);
    assert_eq!(groups[0].hosts.len(), 2);
    assert_eq!(
        count_discovery_scan_ports_for_groups(&groups, &[7_999]),
        2 * (9_920 - 1)
    );
    assert_eq!(build_discovery_port_mode_label(&scan_cidrs, &[]), "80-9999");
}

#[test]
fn mixed_discovery_label_matches_node_copy() {
    let scan_cidrs = vec![
        LOOPBACK_DISCOVERY_CIDR.to_string(),
        "192.168.2.0/30".to_string(),
    ];

    assert_eq!(
        build_discovery_port_mode_label(&scan_cidrs, &[]),
        "local=80-60000, other=80-9999"
    );
}

#[test]
fn discovery_port_list_merges_self_and_service_exclusions() {
    let ports = build_port_list(
        limited_discovery_port_range(),
        &merge_discovery_skip_ports(LOCAL_SELF_DISCOVERY_SKIP_PORTS, &[7_999, 7_999]),
    );

    assert_eq!(ports.first().copied(), Some(81));
    assert!(!ports.contains(&7_999));
    assert_eq!(ports.len(), 9_920 - 2);
}

#[test]
fn discovered_generic_http_service_matches_node_fallback_rule() {
    let service = build_discovered_http_service(
        "192.168.31.1",
        8_080,
        200,
        None,
        "<html><title>Login</title></html>",
    )
    .expect("service");

    assert_eq!(
        service.get("serviceKey").and_then(Value::as_str),
        Some("192.168.31.1::http-8080")
    );
    assert_eq!(
        service.pointer("/detail/name").and_then(Value::as_str),
        Some("http-8080")
    );
    assert_eq!(
        service.pointer("/detail/label").and_then(Value::as_str),
        Some("Login")
    );
    assert_eq!(
        service.pointer("/detail/rule/path").and_then(Value::as_str),
        Some("/app-8080")
    );
    assert_eq!(
        service
            .pointer("/detail/rule/strip_path")
            .and_then(Value::as_bool),
        Some(true)
    );
    assert!(service.get("requiresBasicAuth").is_none());
}

#[test]
fn discovered_generic_http_service_uses_node_label_and_basic_auth_rules() {
    let service = build_discovered_http_service(
        "192.168.31.1",
        80,
        302,
        Some("Digest realm=\"admin\", Basic realm=\"admin\""),
        "",
    )
    .expect("service");

    assert_eq!(
        service.pointer("/detail/label").and_then(Value::as_str),
        Some("HTTP 80")
    );
    assert_eq!(
        service.get("requiresBasicAuth").and_then(Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn discovery_analyzer_matches_node_static_rules() {
    let client = reqwest::Client::new();
    let translator = Translator::new("zh-CN");
    let fnos = analyze_discovered_http_service(
        &client,
        DiscoveryHttpResult {
            host: "192.168.31.2".to_string(),
            port: 5666,
            status: 200,
            headers: HashMap::new(),
            body: "<title>飞牛 fnOS</title>".to_string(),
        },
        &translator,
    )
    .await
    .expect("fnos service");
    assert_eq!(
        fnos.pointer("/detail/name").and_then(Value::as_str),
        Some("fnos")
    );
    assert_eq!(
        fnos.pointer("/detail/rule/path").and_then(Value::as_str),
        Some("/fnos")
    );
    assert_eq!(
        fnos.pointer("/detail/isDefault").and_then(Value::as_bool),
        Some(true)
    );

    let mut luci_headers = HashMap::new();
    luci_headers.insert("x-luci-login-required".to_string(), "yes".to_string());
    let openwrt = analyze_discovered_http_service(
        &client,
        DiscoveryHttpResult {
            host: "192.168.31.1".to_string(),
            port: 80,
            status: 403,
            headers: luci_headers,
            body: String::new(),
        },
        &translator,
    )
    .await
    .expect("openwrt service");
    assert_eq!(
        openwrt.pointer("/detail/name").and_then(Value::as_str),
        Some("openwrt")
    );
    assert_eq!(
        openwrt.pointer("/detail/rule/path").and_then(Value::as_str),
        Some("/openwrt")
    );

    let mut webdav_headers = HashMap::new();
    webdav_headers.insert(
        "www-authenticate".to_string(),
        "Basic realm=\"Restricted\"".to_string(),
    );
    let webdav = analyze_discovered_http_service(
        &client,
        DiscoveryHttpResult {
            host: "192.168.31.3".to_string(),
            port: 5005,
            status: 401,
            headers: webdav_headers,
            body: String::new(),
        },
        &translator,
    )
    .await
    .expect("webdav service");
    assert_eq!(
        webdav.pointer("/detail/name").and_then(Value::as_str),
        Some("webdav")
    );
    assert_eq!(
        webdav.get("requiresBasicAuth").and_then(Value::as_bool),
        Some(true)
    );
}

#[tokio::test]
async fn discovery_analyzer_matches_all_node_static_rule_shapes() {
    struct StaticRuleCase {
        case_name: &'static str,
        port: u16,
        status: u16,
        headers: Vec<(&'static str, &'static str)>,
        body: &'static str,
        expected_name: &'static str,
        expected_path: &'static str,
        expected_rewrite_html: bool,
        expected_use_root_mode: bool,
        expected_is_default: bool,
    }

    let client = reqwest::Client::new();
    let translator = Translator::new("zh-CN");
    let cases = vec![
        StaticRuleCase {
            case_name: "mongo-express",
            port: 8081,
            status: 200,
            headers: vec![("set-cookie", "mongo-express=sid")],
            body: "",
            expected_name: "mongoexpress",
            expected_path: "/mongoe",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "redis-insight",
            port: 5540,
            status: 200,
            headers: vec![],
            body: "<html><title>Redis Insight</title></html>",
            expected_name: "redisinsight",
            expected_path: "/redisi",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "go2rtc",
            port: 1984,
            status: 200,
            headers: vec![],
            body: "<html><title>go2rtc</title></html>",
            expected_name: "go2rtc",
            expected_path: "/go2rtc",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "openwrt",
            port: 80,
            status: 403,
            headers: vec![("x-luci-login-required", "yes")],
            body: "",
            expected_name: "openwrt",
            expected_path: "/openwrt",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "fnos",
            port: 5666,
            status: 200,
            headers: vec![],
            body: "<html><title>飞牛 fnOS</title></html>",
            expected_name: "fnos",
            expected_path: "/fnos",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: true,
        },
        StaticRuleCase {
            case_name: "lucky",
            port: 16601,
            status: 200,
            headers: vec![],
            body: "<html><title>Lucky</title></html>",
            expected_name: "lucky",
            expected_path: "/lucky",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "homeassistant",
            port: 8123,
            status: 200,
            headers: vec![],
            body: "<html><title>Home Assistant</title></html>",
            expected_name: "homeassistant",
            expected_path: "/ha",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "sun-panel",
            port: 3002,
            status: 200,
            headers: vec![],
            body: "<html><title>Sun-Panel</title></html>",
            expected_name: "sun-panel",
            expected_path: "/sp",
            expected_rewrite_html: true,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "webdav",
            port: 5005,
            status: 401,
            headers: vec![("www-authenticate", "Basic realm=\"Restricted\"")],
            body: "",
            expected_name: "webdav",
            expected_path: "/webdav",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "xunlei",
            port: 2345,
            status: 200,
            headers: vec![],
            body: "<html><title>迅雷下载</title></html>",
            expected_name: "xunlei",
            expected_path: "/xunlei",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "minidlna",
            port: 8200,
            status: 200,
            headers: vec![],
            body: "<HTML><TITLE>MiniDLNA status</TITLE></HTML>",
            expected_name: "miniDLNA",
            expected_path: "/dlna",
            expected_rewrite_html: true,
            expected_use_root_mode: false,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "nowen",
            port: 8080,
            status: 200,
            headers: vec![],
            body: "<html><title>Digital Zen Garden</title></html>",
            expected_name: "nowen",
            expected_path: "/nowen",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: true,
        },
        StaticRuleCase {
            case_name: "fnys",
            port: 5667,
            status: 200,
            headers: vec![],
            body: "<html><title>飞牛影视</title></html>",
            expected_name: "fnys",
            expected_path: "/v",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "dpanel",
            port: 8807,
            status: 200,
            headers: vec![],
            body: "<script src=\"/dpanel/ui/main.js\"></script>",
            expected_name: "DPanel",
            expected_path: "/dp",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "lottery",
            port: 8088,
            status: 200,
            headers: vec![],
            body: "<html><title>彩票助手</title></html>",
            expected_name: "cpzs",
            expected_path: "/cpzs",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "kuake",
            port: 5005,
            status: 200,
            headers: vec![],
            body: "<html><title>登录</title></html>",
            expected_name: "Kuake",
            expected_path: "/kuake",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "jellyfin",
            port: 8096,
            status: 200,
            headers: vec![],
            body: "<html><title>Jellyfin</title></html>",
            expected_name: "Jellyfin",
            expected_path: "/jellyfin",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "mefrp",
            port: 7000,
            status: 200,
            headers: vec![],
            body: "<html><title>WebUI 登录 | ME Frp</title></html>",
            expected_name: "ME Frp",
            expected_path: "/mefrp",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "moontv",
            port: 3000,
            status: 200,
            headers: vec![],
            body: "<html><title>MoonTV</title></html>",
            expected_name: "MoonTV",
            expected_path: "/moontv",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "fnosapps",
            port: 3180,
            status: 200,
            headers: vec![],
            body: "<html><title>fnOS Apps</title></html>",
            expected_name: "fnOS Apps",
            expected_path: "/fnosapps",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "emby",
            port: 8097,
            status: 200,
            headers: vec![],
            body: "<script src=\"emby-elements/emby-collapse/emby-collapse.js\"></script>",
            expected_name: "Emby",
            expected_path: "/emby",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
        StaticRuleCase {
            case_name: "dlymusic",
            port: 4567,
            status: 200,
            headers: vec![],
            body: "<html><title>道理鱼音乐管理</title></html>",
            expected_name: "DLYMusic",
            expected_path: "/music",
            expected_rewrite_html: false,
            expected_use_root_mode: true,
            expected_is_default: false,
        },
    ];

    for case in cases {
        let service = analyze_discovered_http_service(
            &client,
            DiscoveryHttpResult {
                host: "192.168.31.2".to_string(),
                port: case.port,
                status: case.status,
                headers: case
                    .headers
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect(),
                body: case.body.to_string(),
            },
            &translator,
        )
        .await
        .unwrap_or_else(|| panic!("{} should match", case.case_name));

        assert_eq!(
            service.pointer("/detail/name").and_then(Value::as_str),
            Some(case.expected_name),
            "{} name",
            case.case_name
        );
        assert_eq!(
            service.pointer("/detail/rule/path").and_then(Value::as_str),
            Some(case.expected_path),
            "{} path",
            case.case_name
        );
        assert_eq!(
            service
                .pointer("/detail/rule/rewrite_html")
                .and_then(Value::as_bool),
            Some(case.expected_rewrite_html),
            "{} rewrite_html",
            case.case_name
        );
        assert_eq!(
            service
                .pointer("/detail/rule/use_root_mode")
                .and_then(Value::as_bool),
            Some(case.expected_use_root_mode),
            "{} use_root_mode",
            case.case_name
        );
        assert_eq!(
            service
                .pointer("/detail/isDefault")
                .and_then(Value::as_bool),
            Some(case.expected_is_default),
            "{} isDefault",
            case.case_name
        );
    }
}

#[tokio::test]
async fn discovery_analyzer_fetches_alist_public_settings_like_node() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for _ in 0..2 {
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
                let (content_type, body) = if path == "/api/public/settings" {
                    (
                        "application/json",
                        br#"{"code":200,"data":{"site_title":"OpenList","version":"1.0"}}"#
                            .to_vec(),
                    )
                } else {
                    (
                        "text/html",
                        br#"<html><title>OpenList</title></html>"#.to_vec(),
                    )
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let service = probe_discovery_service(
        &client,
        &client,
        "127.0.0.1",
        addr.port(),
        &Translator::new("zh-CN"),
    )
    .await
    .expect("openlist service");
    assert_eq!(
        service.pointer("/detail/name").and_then(Value::as_str),
        Some("openlist")
    );
    assert_eq!(
        service.pointer("/detail/rule/path").and_then(Value::as_str),
        Some("/op")
    );
}

#[tokio::test]
async fn discovery_probe_retries_manual_redirect_like_node() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let request_count = Arc::new(AtomicUsize::new(0));
    let request_count_for_server = request_count.clone();
    tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let request_count = request_count_for_server.clone();
            tokio::spawn(async move {
                request_count.fetch_add(1, Ordering::SeqCst);
                let mut buffer = [0_u8; 2048];
                let _ = socket.read(&mut buffer).await;
                let body = br#"<html><title>Redirect Login</title></html>"#;
                let header = format!(
                    "HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(body).await;
            });
        }
    });

    let follow_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            attempt.error("redirect blocked for test")
        }))
        .build()
        .unwrap();
    let manual_client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let service = probe_discovery_service(
        &follow_client,
        &manual_client,
        "127.0.0.1",
        addr.port(),
        &Translator::new("zh-CN"),
    )
    .await
    .expect("manual redirect fallback service");

    let expected_name = format!("http-{}", addr.port());
    assert_eq!(request_count.load(Ordering::SeqCst), 2);
    assert_eq!(
        service.pointer("/detail/name").and_then(Value::as_str),
        Some(expected_name.as_str())
    );
    assert_eq!(
        service.pointer("/detail/label").and_then(Value::as_str),
        Some("Redirect Login")
    );
}

#[tokio::test]
async fn discovery_analyzer_detects_onepanel_public_favicon_like_node() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        for _ in 0..2 {
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
                let (content_type, body) = if path == "/public/favicon.png" {
                    ("application/octet-stream", vec![1, 2, 3])
                } else {
                    (
                        "text/html",
                        br#"<html><title>loading...</title></html>"#.to_vec(),
                    )
                };
                let header = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
        }
    });

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let service = probe_discovery_service(
        &client,
        &client,
        "127.0.0.1",
        addr.port(),
        &Translator::new("zh-CN"),
    )
    .await
    .expect("1panel service");
    assert_eq!(
        service.pointer("/detail/name").and_then(Value::as_str),
        Some("1Panel")
    );
    assert_eq!(
        service.pointer("/detail/rule/path").and_then(Value::as_str),
        Some("/1panel")
    );
}

#[test]
fn discovered_services_keep_lowest_port_for_same_service_key_like_node() {
    let job = Arc::new(Mutex::new(DiscoverJob {
        id: "job".to_string(),
        cancel: Arc::new(AtomicBool::new(false)),
        created_at: now_millis(),
        updated_at: now_millis(),
        state: "running".to_string(),
        meta: Some(json!({ "foundServices": 0 })),
        progress: None,
        service_events: Vec::new(),
        service_map: Vec::new(),
        result: None,
        error: None,
    }));
    push_discovered_service(
        &job,
        json!({ "serviceKey": "host::fnos", "port": 5666, "detail": { "name": "fnos" } }),
    );
    push_discovered_service(
        &job,
        json!({ "serviceKey": "host::fnos", "port": 80, "detail": { "name": "fnos" } }),
    );
    push_discovered_service(
        &job,
        json!({ "serviceKey": "host::fnos", "port": 8080, "detail": { "name": "fnos" } }),
    );

    let locked = job.lock().unwrap();
    assert_eq!(locked.service_events.len(), 2);
    assert_eq!(
        locked.service_events[0].get("port").and_then(Value::as_u64),
        Some(5666)
    );
    assert_eq!(
        locked.service_events[1].get("port").and_then(Value::as_u64),
        Some(80)
    );
    assert_eq!(locked.service_map.len(), 1);
    assert_eq!(
        locked.service_map[0].1.get("port").and_then(Value::as_u64),
        Some(80)
    );
    assert_eq!(
        locked
            .meta
            .as_ref()
            .and_then(|value| value.pointer("/foundServices"))
            .and_then(Value::as_u64),
        Some(1)
    );
}

#[test]
fn plain_http_to_https_port_response_is_not_discoverable_like_node() {
    assert!(
        build_discovered_http_service(
            "192.168.31.1",
            443,
            400,
            None,
            "<title>400 The plain HTTP request was sent to HTTPS port</title>",
        )
        .is_none()
    );
    assert!(
        build_discovered_http_service(
            "192.168.31.1",
            8443,
            400,
            None,
            "Client sent an HTTP request to an HTTPS server",
        )
        .is_none()
    );
}

#[test]
fn serializes_discover_job_from_cursor() {
    let job = Arc::new(Mutex::new(DiscoverJob {
        id: "job-1".to_string(),
        cancel: Arc::new(AtomicBool::new(false)),
        created_at: 10,
        updated_at: 20,
        state: "running".to_string(),
        meta: Some(json!({ "foundServices": 2 })),
        progress: Some(json!({ "scannedPorts": 1 })),
        service_events: vec![json!({ "port": 80 }), json!({ "port": 443 })],
        service_map: Vec::new(),
        result: None,
        error: None,
    }));
    let data = serialize_discover_job(&job, Some("1"));

    assert_eq!(data.get("jobId").and_then(Value::as_str), Some("job-1"));
    assert_eq!(data.get("nextCursor").and_then(Value::as_u64), Some(2));
    assert_eq!(
        data.pointer("/services/0/port").and_then(Value::as_u64),
        Some(443)
    );
}

#[test]
fn service_cursor_parser_matches_node_parse_int() {
    assert_eq!(normalize_service_cursor(Some("1x"), 5), 1);
    assert_eq!(normalize_service_cursor(Some("  +2.9"), 5), 2);
    assert_eq!(normalize_service_cursor(Some("-1"), 5), 0);
    assert_eq!(normalize_service_cursor(Some("0x10"), 5), 0);
    assert_eq!(normalize_service_cursor(Some("99"), 5), 5);
    assert_eq!(normalize_service_cursor(Some("nope"), 5), 0);
}

#[test]
fn localizes_scan_discovery_route_text() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        translator.t("server.scanDiscovery.loadTargetsFailed"),
        "读取扫描目标失败"
    );
    assert_eq!(translator.t("server.apiPathNotFound"), "接口不存在");
}

#[test]
fn localizes_scan_discovery_validation_errors() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        localize_scan_discovery_error(
            &translator,
            "Only local IPv4 CIDR ranges are supported: 8.8.8.0/24",
        ),
        "扫描网段仅支持本地 IPv4 CIDR：8.8.8.0/24"
    );
    assert_eq!(
        localize_scan_discovery_error(
            &translator,
            "At most 1024 hosts can be scanned, current selection has 65534",
        ),
        "单次最多扫描 1024 台主机，当前为 65534 台"
    );
}

#[test]
fn localizes_scan_discovery_target_labels() {
    let translator = Translator::new("zh-CN");

    assert_eq!(
        scan_discovery_target_label(
            &translator,
            "custom",
            &[("cidr", "192.168.1.0/24".to_string())],
        ),
        "192.168.1.0/24（自定义）"
    );
    assert_eq!(
        scan_discovery_target_label(
            &translator,
            "interface",
            &[
                ("cidr", "192.168.2.0/24".to_string()),
                ("name", "en0".to_string()),
            ],
        ),
        "192.168.2.0/24（en0）"
    );
    assert_eq!(
        build_custom_discover_targets(["192.168.3.99/24".to_string()], &translator)
            .first()
            .and_then(|target| target.get("label"))
            .and_then(Value::as_str),
        Some("192.168.3.0/24（自定义）")
    );
    assert_eq!(
        build_saved_discover_targets(["10.0.0.0/24".to_string()], &translator)
            .first()
            .and_then(|target| target.get("label"))
            .and_then(Value::as_str),
        Some("10.0.0.0/24（已保存）")
    );
}

#[test]
fn extracts_mapping_ipv4_targets() {
    assert_eq!(
        extract_ipv4_from_target("http://192.168.2.10:8080/app"),
        Some("192.168.2.10".to_string())
    );
    assert_eq!(extract_ipv4_from_target("https://example.com"), None);
}

#[test]
fn normalizes_host_mapping_probe_keys_like_node() {
    assert_eq!(
        normalize_host_key("HTTPS://Example.COM:8443/path?q=1."),
        "example.com:8443"
    );
    assert_eq!(normalize_host_key("Example.COM."), "example.com");
    assert_eq!(normalize_host_key("1://Example.COM/path"), "1:");
    assert_eq!(
        normalize_host_key("[2001:db8::1]:8443"),
        "[2001:db8::1]:8443"
    );
}

#[test]
fn docker_discover_ip_filter_excludes_loopback_like_node() {
    assert!(is_usable_private_discover_ipv4("192.168.31.10"));
    assert!(is_usable_private_discover_ipv4("100.64.1.2"));
    assert!(!is_usable_private_discover_ipv4("127.0.0.1"));
    assert!(!is_usable_private_discover_ipv4("8.8.8.8"));
}

#[test]
fn scan_excluded_env_ports_match_node_truthy_parse_int() {
    assert_eq!(excluded_env_port_value(None, 7_997), Some(7_997));
    assert_eq!(
        excluded_env_port_value(Some(String::new()), 7_997),
        Some(7_997)
    );
    assert_eq!(
        excluded_env_port_value(Some(" 8080x ".to_string()), 7_997),
        Some(8080)
    );
    assert_eq!(
        excluded_env_port_value(Some("abc".to_string()), 7_997),
        None
    );
    assert_eq!(
        excluded_env_port_value(Some("0x10".to_string()), 7_997),
        None
    );
    assert_eq!(
        resolve_env_port_with_fallback_value(Some(" 8080x ".to_string()), 7_997),
        8080
    );
    assert_eq!(
        resolve_env_port_with_fallback_value(Some("abc".to_string()), 7_997),
        7_997
    );
}

#[test]
fn detects_auth_service_target_by_port() {
    let env = EnvGuard::new(&["AUTH_PORT"]);
    env.set("AUTH_PORT", "7997");
    assert!(is_auth_service_target("http://127.0.0.1:7997"));
    assert!(!is_auth_service_target("ws://127.0.0.1:7997"));
    assert!(!is_auth_service_target("http://127.0.0.1:8080"));
}
