use super::*;

pub fn default_config() -> Value {
    let gateway_config_dir = default_gateway_config_dir();
    let waf_rules_dir = format!("{}/waf", gateway_config_dir.trim_end_matches('/'));

    let subdomain_mode = json!({
        "root_domain": "",
        "auth_host": "",
        "auth_target": crate::proxy_utils::default_auth_service_target(),
        "cookie_domain": "",
        "edge_client_ip_enabled": false,
        "aliyun_esa_enabled": false,
        "tencent_edgeone_enabled": false,
        "public_auth_base_url": "",
        "public_http_port": 0,
        "public_https_port": 0,
        "auth_cache_ttl_seconds": 1,
        "auth_cache_unauthorized_ttl_seconds": 1,
        "default_access_mode": "login_first",
        "auto_add_whitelist_on_login": true,
        "passkey_rp_mode": "auth_host",
        "passkey_rp_id": ""
    });
    let ssl = json!({
        "cert": "",
        "key": "",
        "active_cert_id": "",
        "deployment_mode": "single_active",
        "certificates": []
    });
    let fnos_share_bypass = json!({
        "enabled": false,
        "upstream_timeout_ms": 2500,
        "validation_cache_ttl_seconds": 30,
        "validation_lock_ttl_seconds": 5,
        "session_ttl_seconds": 300
    });
    let fnos_port_icon_hijack = json!({
        "enabled": false,
        "updated_at": null
    });
    let fnos_connect_waf = json!({
        "enabled": false,
        "updated_at": null
    });
    let fnos_network_tuning = json!({
        "bbr_enabled": false,
        "mtu_probing_enabled": false,
        "previous_tcp_congestion_control": null,
        "previous_default_qdisc": null,
        "previous_tcp_mtu_probing": null,
        "updated_at": null,
        "last_error": null
    });
    let gateway_logging = json!({
        "enabled": false,
        "record_localhost": false,
        "max_days": 7
    });
    let waf = json!({
        "enabled": false,
        "system_rules_auto_update_enabled": true,
        "common_location_exempt_enabled": false,
        "private_ip_exempt_enabled": false,
        "mode": "blocking",
        "active_bundle_id": "local",
        "rules_dir": waf_rules_dir,
        "paranoia_level": 1,
        "executing_paranoia_level": 1,
        "inbound_anomaly_threshold": 5,
        "outbound_anomaly_threshold": 4,
        "request_body_access": true,
        "request_body_limit_bytes": 131072,
        "request_body_in_memory_limit_bytes": 65536,
        "response_body_access": false,
        "disabled_hosts": [],
        "disabled_path_prefixes": [],
        "log_retention_days": 7,
        "drain_interval_seconds": 2,
        "updated_at": null
    });
    let reverse_proxy_throttle = crate::gateway_settings::default_reverse_proxy_throttle();
    let gateway_visibility = json!({
        "enabled": false,
        "selections": [],
        "custom_cidrs": []
    });
    let gateway_proxy_headers = json!({ "disabled_hosts": [] });
    let gateway_host_response = json!({ "disabled_hosts": [] });
    let gateway_crawler_blocker = json!({
        "enabled": false,
        "updated_at": null
    });
    let gateway_portal = json!({
        "enabled": true,
        "display_style": "title",
        "show_app_icon": true,
        "show_wol": true,
        "icon_drag_mode": "corners",
        "version": "v1"
    });
    let wol_feature = json!({ "enabled": false });
    let gateway_unmatched_route = json!({
        "behavior": "error_page",
        "upstream_error_detail": "less"
    });
    let appearance = json!({ "theme_color_preset": "default" });
    let dashboard_display = json!({
        "show_entry_status_module": true,
        "show_console_app_list": false,
        "date_time_display_mode": "human_friendly",
        "sidebar_menu_order": [
            "dashboard",
            "route_mapping",
            "tunnel",
            "protocol_mapping",
            "sessions",
            "ip_whitelist",
            "ssl_certificate",
            "ddns",
            "wol",
            "auth",
            "ssh_security",
            "events",
            "gateway_request_logs",
            "waf_logs",
            "web_terminal",
            "system_settings"
        ]
    });
    let auto_https = json!({ "enabled": false });
    let smart_connect = json!({
        "enabled": false,
        "selected_ipv4": ""
    });
    let scan_discovery = json!({
        "custom_cidrs": [],
        "selected_cidrs": [],
        "intensity_mode": "auto",
        "intensity_level": "medium"
    });
    let locale = json!({ "default_locale": "zh-CN" });
    let auth_credential_settings = json!({
        "session_ttl_seconds": 86400,
        "remember_me_ttl_seconds": 31536000,
        "post_login_ip_grant_mode": "follow_session",
        "post_login_ip_grant_ttl_seconds": 3600,
        "session_ip_mobility_enabled": false,
        "session_ip_mobility_window_seconds": 1200,
        "passkey_bind_prompt_enabled": true
    });
    let event_system = json!({
        "enabled": true,
        "retention_days": 30,
        "max_records": 10000,
        "rules": {
            "login_failure": { "enabled": true },
            "ip_drift": { "enabled": true },
            "scanner_blocked": { "enabled": true },
            "ddns_update": { "enabled": true },
            "wol_wake": { "enabled": true },
            "wol_shutdown": { "enabled": true },
            "gateway_throttle_block": { "enabled": true },
            "gateway_visibility_block": { "enabled": true },
            "waf_blocked": { "enabled": true },
            "app_update_available": { "enabled": true },
            "frp_tunnel": { "enabled": true },
            "cloudflared_tunnel": { "enabled": true },
            "ssh_login_success": { "enabled": true },
            "ssh_login_failure": { "enabled": true },
            "ssh_ip_blocked": { "enabled": true },
            "runtime_lifecycle": { "enabled": true },
            "runtime_health": { "enabled": true },
            "terminal_audit": { "enabled": true },
            "cpu_alert": {
                "enabled": true,
                "threshold_percent": 80,
                "recover_percent": 60,
                "sample_interval_seconds": 5,
                "sustain_seconds": 30
            },
            "memory_alert": {
                "enabled": true,
                "threshold_percent": 80,
                "recover_percent": 60,
                "sample_interval_seconds": 5,
                "sustain_seconds": 30
            }
        }
    });
    let ssh_security = json!({
        "enabled": false,
        "window_minutes": 10,
        "failed_login_threshold": 5,
        "block_duration_value": 1,
        "block_duration_unit": "day",
        "allowed_regions": [],
        "custom_cidrs": [],
        "configured_at": null,
        "updated_at": null
    });

    json!({
        "run_type": 3,
        "reverse_proxy_submode": "host",
        "auto_manage_firewall": true,
        "firewall_additional_ports": [],
        "whitelist_ips": [],
        "proxy_mappings": [],
        "host_mappings": [],
        "host_mapping_groups": [],
        "host_mapping_grouped_view": false,
        "stream_mappings": [],
        "subdomain_mode": subdomain_mode,
        "ssl": ssl,
        "default_route": "/__select__",
        "default_tunnel": "frp",
        "fnos_share_bypass": fnos_share_bypass,
        "fnos_port_icon_hijack": fnos_port_icon_hijack,
        "fnos_connect_waf": fnos_connect_waf,
        "fnos_network_tuning": fnos_network_tuning,
        "gateway_logging": gateway_logging,
        "waf": waf,
        "reverse_proxy_throttle": reverse_proxy_throttle,
        "gateway_visibility": gateway_visibility,
        "visibility_policies": {},
        "gateway_proxy_headers": gateway_proxy_headers,
        "gateway_host_response": gateway_host_response,
        "gateway_crawler_blocker": gateway_crawler_blocker,
        "gateway_portal": gateway_portal,
        "gateway_unmatched_route": gateway_unmatched_route,
        "appearance": appearance,
        "dashboard_display": dashboard_display,
        "auto_https": auto_https,
        "smart_connect": smart_connect,
        "scan_discovery": scan_discovery,
        "locale": locale,
        "auth_credential_settings": auth_credential_settings,
        "event_system": event_system,
        "wol_feature": wol_feature,
        "ssh_security": ssh_security
    })
}

fn default_gateway_config_dir() -> String {
    std::env::var("FN_KNOCK_GATEWAY_CONFIG_DIR")
        .or_else(|_| std::env::var("GATEWAY_CONFIG_DIR"))
        .or_else(|_| std::env::var("FN_KNOCK_DATA_DIR"))
        .unwrap_or_else(|_| "/tmp/fn-knock".to_string())
}
