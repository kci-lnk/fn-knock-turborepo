use super::*;
#[cfg(target_os = "macos")]
use crate::infra::system_resources::host_memory_bytes;
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
fn discovery_host_groups_use_full_range_and_self_skip() {
    let scan_cidrs = vec!["127.0.0.1/32".to_string(), "192.168.1.0/30".to_string()];
    let self_scan_hosts = vec!["192.168.1.1".to_string()];

    let groups = build_discovery_host_groups(&scan_cidrs, None, &self_scan_hosts);

    assert_eq!(groups.len(), 3);
    assert_eq!(
        count_discovery_scan_ports_for_groups(&groups, &[]),
        59_920 + 59_920 + 59_921
    );
    assert_eq!(build_discovery_port_mode_label(), "80-60000");
}

#[test]
fn network_discovery_uses_full_range_and_excluded_ports() {
    let scan_cidrs = vec!["192.168.2.0/30".to_string()];
    let groups = build_discovery_host_groups(&scan_cidrs, None, &[]);

    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].hosts.len(), 2);
    assert_eq!(
        count_discovery_scan_ports_for_groups(&groups, &[7_999]),
        2 * (59_921 - 1)
    );
    assert_eq!(build_discovery_port_mode_label(), "80-60000");
}

#[test]
fn discovery_reserves_managed_cloudflare_ingress_port() {
    assert!(DISCOVERY_RESERVED_PORTS.contains(&crate::tunnels::MANAGED_CLOUDFLARE_INGRESS_PORT));
}

#[test]
fn every_intensity_uses_the_same_port_range() {
    for level in [
        ScanIntensityLevel::Low,
        ScanIntensityLevel::Medium,
        ScanIntensityLevel::High,
        ScanIntensityLevel::Extreme,
    ] {
        assert_eq!(discovery_port_range().start, 80, "{}", level.as_str());
        assert_eq!(discovery_port_range().end, 60_000, "{}", level.as_str());
    }
}

#[test]
fn discovery_port_list_merges_self_and_service_exclusions() {
    let ports = build_port_list(
        discovery_port_range(),
        &merge_discovery_skip_ports(LOCAL_SELF_DISCOVERY_SKIP_PORTS, &[7_999, 7_999]),
    );

    assert_eq!(ports.first().copied(), Some(81));
    assert!(!ports.contains(&7_999));
    assert_eq!(ports.len(), 59_921 - 2);
}

fn test_discover_job(state: &str, cancelled: bool, updated_at: i64) -> DiscoverJobHandle {
    Arc::new(Mutex::new(DiscoverJob {
        id: "test-job".to_string(),
        cancel: Arc::new(AtomicBool::new(cancelled)),
        created_at: 1,
        updated_at,
        state: state.to_string(),
        meta: None,
        progress: None,
        service_events: Vec::new(),
        service_map: Vec::new(),
        result: None,
        error: None,
    }))
}

#[test]
fn active_discovery_ttl_tracks_last_progress_instead_of_creation() {
    let now = DISCOVER_JOB_ACTIVE_TTL_MS + 10_000;
    let recently_updated = test_discover_job("running", false, now - 1_000);
    let inactive = test_discover_job("running", false, now - DISCOVER_JOB_ACTIVE_TTL_MS - 1);

    assert!(!discover_job_inactive_expired(
        &discover_job_guard(&recently_updated),
        now
    ));
    assert!(discover_job_inactive_expired(
        &discover_job_guard(&inactive),
        now
    ));
}

#[test]
fn cancelled_discovery_cannot_transition_back_to_running() {
    for job in [
        test_discover_job("cancelled", true, 1),
        test_discover_job("cancelled", false, 1),
    ] {
        assert!(!mark_discover_job_running(
            &job,
            json!({ "foundServices": 0 }),
            json!({ "scannedPorts": 0 })
        ));
        assert_eq!(discover_job_guard(&job).state, "cancelled");
    }
}

#[test]
fn scan_intensity_defaults_and_concurrency_are_stable() {
    let (mode, level) = read_scan_intensity_config(&json!({}));
    assert_eq!(mode, ScanIntensityMode::Auto);
    assert_eq!(level, ScanIntensityLevel::Medium);
    assert_eq!(ScanIntensityLevel::Low.concurrency(), 32);
    assert_eq!(ScanIntensityLevel::Medium.concurrency(), 115);
    assert_eq!(ScanIntensityLevel::High.concurrency(), 256);
    assert_eq!(ScanIntensityLevel::Extreme.concurrency(), 512);
    assert_eq!(
        ScanIntensityLevel::Medium.concurrency(),
        (6 * 64 * 30 + 50) / 100
    );
    assert!(ScanIntensityLevel::Extreme.concurrency() > 6 * 64);
    assert_eq!(
        effective_concurrency_for_level(ScanIntensityLevel::Extreme, 96),
        96
    );
}

#[test]
fn scan_capacity_recommendation_respects_resource_thresholds() {
    let capacity = |cpu_cores, total, available, safe_concurrency| ScanDeviceCapacity {
        cpu_cores,
        total_memory_bytes: Some(total),
        available_memory_bytes: Some(available),
        file_descriptor_limit: Some(65_536),
        safe_concurrency,
    };
    assert_eq!(
        recommend_scan_intensity(&capacity(1, 512 * 1024 * 1024, 128 * 1024 * 1024, 64)),
        ScanIntensityLevel::Low
    );
    assert_eq!(
        recommend_scan_intensity(&capacity(
            2,
            2 * 1024 * 1024 * 1024,
            1024 * 1024 * 1024,
            115
        )),
        ScanIntensityLevel::Medium
    );
    assert_eq!(
        recommend_scan_intensity(&capacity(
            4,
            4 * 1024 * 1024 * 1024,
            2 * 1024 * 1024 * 1024,
            256
        )),
        ScanIntensityLevel::High
    );
    assert_eq!(
        recommend_scan_intensity(&capacity(
            8,
            8 * 1024 * 1024 * 1024,
            4 * 1024 * 1024 * 1024,
            512
        )),
        ScanIntensityLevel::Extreme
    );
    assert_eq!(
        recommend_scan_intensity(&capacity(
            8,
            8 * 1024 * 1024 * 1024,
            4 * 1024 * 1024 * 1024,
            511
        )),
        ScanIntensityLevel::High
    );
}

#[cfg(target_os = "macos")]
#[test]
fn macos_scan_capacity_reports_available_memory() {
    let (total, available) = host_memory_bytes();
    let total = total.expect("macOS total memory");
    let available = available.expect("macOS available memory");
    assert!(available > 0);
    assert!(available <= total);
}

#[test]
fn scan_safe_concurrency_uses_the_tightest_resource_budget() {
    assert_eq!(
        calculate_safe_concurrency(8, Some(128 * 1024 * 1024), Some(65_536)),
        32
    );
    assert_eq!(
        calculate_safe_concurrency(1, Some(8 * 1024 * 1024 * 1024), Some(65_536)),
        128
    );
    assert_eq!(
        calculate_safe_concurrency(16, Some(8 * 1024 * 1024 * 1024), Some(320)),
        32
    );
    assert_eq!(calculate_safe_concurrency(32, None, None), 1024);
    assert_eq!(
        calculate_safe_concurrency(8, Some(4 * 1024 * 1024 * 1024), Some(65_536)),
        1024
    );
}

#[tokio::test]
async fn expanded_global_budget_has_room_for_two_extreme_tasks() {
    let global_budget = Arc::new(GlobalProbeBudget::new(1024));
    let _registration = global_budget.register(1024).await;

    assert_eq!(global_budget.current_limit(), 1024);
    assert_eq!(ScanIntensityLevel::Extreme.concurrency() * 2, 1024);
}

async fn exercise_probe_wave(
    global_budget: Arc<GlobalProbeBudget>,
    task_budget: Arc<Semaphore>,
    probe_count: usize,
    active: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
) {
    let mut probes = JoinSet::new();
    for _ in 0..probe_count {
        let global_budget = global_budget.clone();
        let task_budget = task_budget.clone();
        let active = active.clone();
        let peak = peak.clone();
        probes.spawn(async move {
            let _task_permit = task_budget.acquire_owned().await.expect("task permit");
            let _global_permit = global_budget.acquire().await.expect("global permit");
            let current = active.fetch_add(1, Ordering::SeqCst) + 1;
            peak.fetch_max(current, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(2)).await;
            active.fetch_sub(1, Ordering::SeqCst);
        });
    }
    while let Some(result) = probes.join_next().await {
        result.expect("probe task");
    }
}

#[tokio::test]
async fn per_task_probe_budget_caps_a_single_scan() {
    let global_budget = Arc::new(GlobalProbeBudget::new(256));
    let _registration = global_budget.register(128).await;
    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));

    exercise_probe_wave(
        global_budget,
        Arc::new(Semaphore::new(32)),
        160,
        active,
        peak.clone(),
    )
    .await;

    assert!(peak.load(Ordering::SeqCst) <= 32);
}

#[tokio::test]
async fn global_probe_budget_caps_multiple_scans_at_the_safest_active_limit() {
    let global_budget = Arc::new(GlobalProbeBudget::new(256));
    let _first_registration = global_budget.register(128).await;
    let _second_registration = global_budget.register(48).await;
    assert_eq!(global_budget.current_limit(), 48);

    let active = Arc::new(AtomicUsize::new(0));
    let peak = Arc::new(AtomicUsize::new(0));
    let first = exercise_probe_wave(
        global_budget.clone(),
        Arc::new(Semaphore::new(64)),
        160,
        active.clone(),
        peak.clone(),
    );
    let second = exercise_probe_wave(
        global_budget,
        Arc::new(Semaphore::new(64)),
        160,
        active,
        peak.clone(),
    );
    tokio::join!(first, second);

    assert!(peak.load(Ordering::SeqCst) <= 48);
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
        StaticRuleCase {
            case_name: "certd",
            port: 7001,
            status: 200,
            headers: vec![],
            body: "<!doctype html><html lang=\"en\"><head><meta charset=\"UTF-8\"/><link rel=\"icon\" href=\"api/app/favicon\"/><meta name=\"viewport\" content=\"width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no\"/><title>Loading</title><script src=\"static/icons/iconfont.js?v=1.43.0\"></script><link rel=\"stylesheet\" href=\"static/index.css?v=1.43.0\"/><script type=\"module\" crossorigin src=\"./assets/index-C8EIeze_.js\"></script></head><body><div id=\"app\"></div></body></html>",
            expected_name: "certd",
            expected_path: "/certd",
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
fn docker_discover_candidates_merge_config_proxy_and_request_host_in_priority_order() {
    let env = EnvGuard::new(&["DOCKER_DISCOVER_LAN_CIDRS", "DOCKER_DISCOVER_LAN_IP"]);
    env.set("DOCKER_DISCOVER_LAN_CIDRS", "10.20.0.8/23, 10.21.0.8/16");
    env.set("DOCKER_DISCOVER_LAN_IP", "10.30.0.9");

    let mut headers = HeaderMap::new();
    headers.insert(
        DOCKER_DISCOVER_IP_HEADER,
        axum::http::HeaderValue::from_static("192.168.1.9"),
    );
    headers.insert(
        DOCKER_DISCOVER_CIDRS_HEADER,
        axum::http::HeaderValue::from_static("192.168.1.9/23,192.168.50.9/24"),
    );
    headers.insert(
        "x-forwarded-host",
        axum::http::HeaderValue::from_static("192.168.60.9:7991"),
    );

    let candidates = resolve_docker_discover_candidates(&headers);
    assert_eq!(
        candidates,
        vec![
            DiscoverHostCandidate {
                address: "10.20.0.8".to_string(),
                cidr: "10.20.0.0/23".to_string(),
                source: "configured",
            },
            DiscoverHostCandidate {
                address: "10.21.0.8".to_string(),
                cidr: "10.21.0.0/16".to_string(),
                source: "configured",
            },
            DiscoverHostCandidate {
                address: "10.30.0.9".to_string(),
                cidr: "10.30.0.0/24".to_string(),
                source: "configured",
            },
            DiscoverHostCandidate {
                address: "192.168.1.9".to_string(),
                cidr: "192.168.0.0/23".to_string(),
                source: "proxy",
            },
            DiscoverHostCandidate {
                address: "192.168.50.9".to_string(),
                cidr: "192.168.50.0/24".to_string(),
                source: "proxy",
            },
            DiscoverHostCandidate {
                address: "192.168.60.9".to_string(),
                cidr: "192.168.60.0/24".to_string(),
                source: "request_host",
            },
        ]
    );
}

#[test]
fn native_host_candidates_keep_loopback_first_and_include_private_interfaces() {
    let interface_candidates = [
        net_utils::PrivateIpv4Candidate {
            interface: "br-lan".to_string(),
            address: Ipv4Addr::new(192, 168, 50, 8),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            prefix: Some(24),
        },
        net_utils::PrivateIpv4Candidate {
            interface: "eth0".to_string(),
            address: Ipv4Addr::new(10, 20, 0, 8),
            netmask: Ipv4Addr::new(255, 255, 0, 0),
            prefix: Some(16),
        },
        net_utils::PrivateIpv4Candidate {
            interface: "duplicate".to_string(),
            address: Ipv4Addr::new(192, 168, 50, 8),
            netmask: Ipv4Addr::new(255, 255, 255, 0),
            prefix: Some(24),
        },
    ];
    let candidates = build_native_discover_host_candidates(&interface_candidates);
    assert_eq!(
        candidates,
        vec![
            DiscoverHostCandidate {
                address: "127.0.0.1".to_string(),
                cidr: "127.0.0.1/32".to_string(),
                source: "loopback",
            },
            DiscoverHostCandidate {
                address: "192.168.50.8".to_string(),
                cidr: "192.168.50.0/24".to_string(),
                source: "interface",
            },
            DiscoverHostCandidate {
                address: "10.20.0.8".to_string(),
                cidr: "10.20.0.0/24".to_string(),
                source: "interface",
            },
        ]
    );

    let payload = build_discover_host_candidates_payload(
        &candidates,
        &BTreeSet::from(["127.0.0.1/32".to_string(), "192.168.50.0/24".to_string()]),
    );
    assert_eq!(payload[0]["recommended"], json!(true));
    assert_eq!(payload[0]["source"], json!("loopback"));
    assert_eq!(payload[1]["includedInAutomaticScan"], json!(true));
    assert_eq!(payload[2]["includedInAutomaticScan"], json!(false));
}

#[test]
fn automatic_target_limit_keeps_priority_order_without_exceeding_host_budget() {
    let targets = ["10.0.0.0/22", "10.1.0.0/24", "10.2.0.1/32"]
        .into_iter()
        .filter_map(|cidr| to_discover_target(cidr, cidr, "docker", true))
        .collect::<Vec<_>>();

    let limited = limit_automatic_targets(targets);
    let cidrs = limited
        .iter()
        .filter_map(|target| target.get("cidr").and_then(Value::as_str))
        .collect::<Vec<_>>();
    assert_eq!(cidrs, vec!["10.0.0.0/22", "10.2.0.1/32"]);
    assert!(
        count_scan_hosts(
            &cidrs
                .iter()
                .map(|value| value.to_string())
                .collect::<Vec<_>>()
        )
        .is_ok()
    );
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
