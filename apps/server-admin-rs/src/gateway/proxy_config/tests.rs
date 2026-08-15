use super::*;
use crate::cidr::compile_ip_set;
use std::collections::BTreeSet;
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
fn rejects_wildcards_in_subdomain_roots_and_host_mappings() {
    assert_eq!(
        validate_subdomain_root_domain(&json!({
            "root_domain": "*.example.com"
        })),
        Err("Subdomain root domain cannot contain wildcard")
    );
    assert!(
        validate_subdomain_root_domain(&json!({
            "root_domain": "example.com"
        }))
        .is_ok()
    );

    let error = normalize_host_mappings_for_route(
        vec![json!({
            "host": "auth.*.example.com",
            "target": "http://127.0.0.1:7997",
            "use_auth": false
        })],
        &json!({}),
    )
    .unwrap_err();
    assert_eq!(
        error,
        "Host mapping auth.*.example.com cannot contain wildcard"
    );
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
    assert_eq!(mapping.get("waf_enabled"), Some(&Value::Bool(true)));
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
fn validates_and_applies_custom_host_mapping_icons() {
    let custom_icon = "data:image/png;base64,iVBORw0KGgo=";
    assert_eq!(
        normalize_favicon_override(Some(&Value::String(custom_icon.to_string()))).unwrap(),
        custom_icon
    );
    assert!(
        normalize_favicon_override(Some(&Value::String(
            "data:image/svg+xml;base64,PHN2Zz4=".to_string()
        )))
        .is_err()
    );
    assert!(
        normalize_favicon_override(Some(&Value::String(
            "data:image/png;base64,UklGRgAAAABXRUJQ".to_string()
        )))
        .is_err()
    );
    let mut oversized_png = b"\x89PNG\r\n\x1a\n".to_vec();
    oversized_png.resize(MAX_FAVICON_BYTES + 1, 0);
    assert!(
        normalize_favicon_override(Some(&Value::String(format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(oversized_png)
        ))))
        .is_err()
    );

    let previous_config = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "title": "App",
            "favicon": "data:image/png;base64,iVBORw0KGgo=",
            "favicon_override": ""
        }]
    });
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "use_auth": true,
            "favicon_override": custom_icon
        })],
        &previous_config,
    )
    .unwrap();
    assert_eq!(
        mappings[0].get("favicon_override").and_then(Value::as_str),
        Some(custom_icon)
    );
    assert_eq!(
        build_host_rules_payload(&mappings)
            .pointer("/0/favicon")
            .and_then(Value::as_str),
        Some(custom_icon)
    );
}

#[test]
fn ordinary_host_mapping_updates_cannot_inject_uncompiled_advanced_auth() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "new.example.com",
            "target": "http://127.0.0.1:8080",
            "use_auth": true,
            "advanced_auth": {
                "enabled": true,
                "policy_version": "client-controlled",
                "groups": [{ "id": "g", "conditions": [{
                    "id": "c", "target": "url_path", "operator": "prefix", "values": ["/"]
                }] }]
            }
        })],
        &json!({ "host_mappings": [] }),
    )
    .unwrap();
    assert_eq!(mappings[0]["advanced_auth"]["enabled"], json!(false));
    assert_ne!(
        mappings[0]["advanced_auth"]["policy_version"],
        json!("client-controlled")
    );
}

#[test]
fn normalizes_host_mapping_waf_defaults_and_auth_inheritance() {
    let mappings = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "legacy.example.com",
                "target": "http://127.0.0.1:8080"
            }),
            json!({
                "host": "excluded.example.com",
                "target": "http://127.0.0.1:8081",
                "waf_enabled": false
            }),
            json!({
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "use_auth": false,
                "waf_enabled": false
            }),
        ],
        &json!({}),
    )
    .unwrap();

    assert_eq!(mappings[0].get("waf_enabled"), Some(&Value::Bool(true)));
    assert_eq!(mappings[1].get("waf_enabled"), Some(&Value::Bool(false)));
    assert_eq!(mappings[2].get("waf_enabled"), Some(&Value::Bool(true)));
}

#[test]
fn normalizes_host_mapping_visibility_and_preserves_legacy_updates() {
    let previous = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "custom",
                "selections": [{
                    "province": "浙江省",
                    "city": "杭州市",
                    "label": "杭州市 · 移动",
                    "value": "杭州市",
                    "query_city": "杭州市",
                    "operator": "移动",
                    "is_province_wide": false,
                    "is_municipality": false
                }],
                "custom_cidrs": ["203.0.113.0/24"],
                "cidrs": ["203.0.113.0/24"]
            }
        }]
    });
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080"
        })],
        &previous,
    )
    .unwrap();

    assert_eq!(
        mappings[0]["visibility"],
        previous["host_mappings"][0]["visibility"]
    );

    let legacy = normalize_host_mappings_for_route(
        vec![json!({
            "host": "legacy.example.com",
            "target": "http://127.0.0.1:8081"
        })],
        &json!({}),
    )
    .unwrap();
    assert_eq!(legacy[0]["visibility"]["mode"], json!("inherit"));
    assert_eq!(legacy[0]["visibility"]["cidrs"], json!([]));
}

#[test]
fn normalizes_disabled_host_visibility_and_preserves_custom_draft() {
    let previous = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "custom",
                "selections": [{
                    "province": "浙江省",
                    "city": "杭州市",
                    "label": "杭州市 · 移动",
                    "value": "杭州市",
                    "query_city": "杭州市",
                    "operator": "移动",
                    "is_province_wide": false,
                    "is_municipality": false
                }],
                "custom_cidrs": ["203.0.113.0/24"],
                "cidrs": ["203.0.113.0/24"]
            }
        }]
    });
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "disabled",
                "selections": [{ "province": "浙江省", "query_city": "杭州市", "operator": "移动" }],
                "custom_cidrs": ["203.0.113.0/24"]
            }
        })],
        &previous,
    )
    .unwrap();

    assert_eq!(mappings[0]["visibility"]["mode"], json!("disabled"));
    assert_eq!(
        mappings[0]["visibility"]["selections"],
        previous["host_mappings"][0]["visibility"]["selections"]
    );
    assert_eq!(
        mappings[0]["visibility"]["custom_cidrs"],
        json!(["203.0.113.0/24"])
    );
    assert_eq!(
        mappings[0]["visibility"]["cidrs"],
        json!(["203.0.113.0/24"])
    );
}

#[test]
fn disabled_visibility_rejects_invalid_operator_without_losing_previous_draft() {
    let previous = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "custom",
                "selections": [{ "province": "浙江", "query_city": "杭州", "operator": "移动" }],
                "custom_cidrs": [],
                "cidrs": ["10.0.0.0/8"]
            }
        }]
    });
    let result = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "disabled",
                "selections": [{ "province": "浙江", "query_city": "杭州", "operator": false }]
            }
        })],
        &previous,
    );
    assert!(result.is_err());
    assert_eq!(
        previous["host_mappings"][0]["visibility"]["selections"][0]["operator"],
        json!("移动")
    );
}

#[test]
fn host_visibility_ignores_client_derived_cidrs_and_forces_auth_inheritance() {
    let mappings = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "visibility": {
                    "mode": "custom",
                    "selections": [],
                    "custom_cidrs": ["203.0.113.0/24"],
                    "cidrs": ["1.1.1.0/24"]
                }
            }),
            json!({
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "use_auth": false,
                "visibility": {
                    "mode": "custom",
                    "custom_cidrs": ["203.0.113.0/24"],
                    "cidrs": ["203.0.113.0/24"]
                }
            }),
        ],
        &json!({}),
    )
    .unwrap();

    assert_eq!(mappings[0]["visibility"]["cidrs"], json!([]));
    assert_eq!(mappings[1]["visibility"]["mode"], json!("inherit"));
    assert_eq!(mappings[1]["visibility"]["custom_cidrs"], json!([]));
    assert_eq!(mappings[1]["visibility"]["cidrs"], json!([]));
}

#[test]
fn rejects_malformed_explicit_host_visibility_without_erasing_previous_rules() {
    let previous = json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "visibility": {
                "mode": "custom",
                "selections": [],
                "custom_cidrs": ["203.0.113.0/24"],
                "cidrs": ["203.0.113.0/24"]
            }
        }]
    });

    for visibility in [
        json!("custom"),
        json!({ "mode": "merge" }),
        json!({ "mode": "custom", "selections": {} }),
        json!({ "mode": "custom", "custom_cidrs": "203.0.113.0/24" }),
    ] {
        let error = normalize_host_mappings_for_route(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8081",
                "visibility": visibility
            })],
            &previous,
        )
        .unwrap_err();
        assert!(error.contains("Host mapping app.example.com visibility"));
    }

    let preserved = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8081"
        })],
        &previous,
    )
    .unwrap();
    assert_eq!(
        preserved[0]["visibility"],
        previous["host_mappings"][0]["visibility"]
    );
}

#[tokio::test]
async fn compiles_custom_host_visibility_and_rejects_invalid_or_empty_rules() {
    let (_directory, state) = proxy_config_test_state("http://127.0.0.1:1".to_string()).await;
    let compiled = compile_host_mapping_visibilities(
        &state,
        vec![json!({
            "host": "app.example.com",
            "visibility": {
                "mode": "custom",
                "selections": [],
                "custom_cidrs": [" 203.0.113.7/24 ", "203.0.113.0/24"],
                "cidrs": ["1.1.1.0/24"]
            }
        })],
        &json!({}),
    )
    .await
    .unwrap();
    assert_eq!(
        compiled.mappings[0]["visibility"]["custom_cidrs"],
        json!(["203.0.113.0/24"])
    );
    assert!(compiled.mappings[0]["visibility"].get("cidrs").is_none());
    let policy_id = compiled.mappings[0]["visibility"]["policy_id"]
        .as_str()
        .unwrap();
    assert!(policy_id.starts_with("ipset-v2:"));
    assert_eq!(compiled.visibility_policies.len(), 1);
    assert!(compiled.visibility_policies.contains_key(policy_id));

    let preserved = json!({
        "host_mappings": [{
            "host": "legacy.example.com",
            "visibility": {
                "mode": "custom",
                "selections": [{ "province": "legacy-region", "query_city": null }],
                "custom_cidrs": [],
                "cidrs": ["198.51.100.0/24"]
            }
        }]
    });
    let legacy = compile_host_mapping_visibilities(
        &state,
        vec![preserved["host_mappings"][0].clone()],
        &preserved,
    )
    .await
    .unwrap();
    assert!(legacy.mappings[0]["visibility"].get("cidrs").is_none());
    assert!(
        legacy.mappings[0]["visibility"]["policy_id"]
            .as_str()
            .unwrap()
            .starts_with("ipset-v2:")
    );
    assert_eq!(legacy.visibility_policies.len(), 1);

    for custom_cidrs in [json!([]), json!(["not-a-cidr"])] {
        let error = compile_host_mapping_visibilities(
            &state,
            vec![json!({
                "host": "app.example.com",
                "visibility": {
                    "mode": "custom",
                    "selections": [],
                    "custom_cidrs": custom_cidrs
                }
            })],
            &json!({}),
        )
        .await
        .unwrap_err();
        assert!(error.contains("Host mapping app.example.com visibility"));
    }
}

#[tokio::test]
async fn six_hosts_share_one_content_addressed_visibility_policy() {
    let (_directory, state) = proxy_config_test_state("http://127.0.0.1:1".to_string()).await;
    let mappings = (0..6)
        .map(|index| {
            json!({
                "host": format!("app-{index}.example.com"),
                "visibility": {
                    "mode": "custom",
                    "selections": [],
                    "custom_cidrs": ["203.0.113.0/25", "203.0.113.128/25"]
                }
            })
        })
        .collect::<Vec<_>>();
    let compiled = compile_host_mapping_visibilities(&state, mappings, &json!({}))
        .await
        .unwrap();
    assert_eq!(compiled.mappings.len(), 6);
    assert_eq!(compiled.visibility_policies.len(), 1);
    let ids = compiled
        .mappings
        .iter()
        .filter_map(|mapping| mapping.pointer("/visibility/policy_id"))
        .filter_map(Value::as_str)
        .collect::<BTreeSet<_>>();
    assert_eq!(ids.len(), 1);
    assert!(
        compiled
            .mappings
            .iter()
            .all(|mapping| mapping.pointer("/visibility/cidrs").is_none())
    );
}

#[test]
fn host_rules_payload_contains_only_compiled_visibility_fields() {
    let payload = build_host_rules_payload(&[json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "visibility": {
            "mode": "custom",
            "selections": [{ "province": "浙江省" }],
            "custom_cidrs": ["203.0.113.0/24"],
            "cidrs": ["203.0.113.0/24"],
            "policy_id": "ipset-v1:test"
        }
    })]);

    assert_eq!(
        payload[0]["visibility"],
        json!({
            "mode": "custom",
            "policy_id": "ipset-v1:test"
        })
    );
}

#[test]
fn host_mapping_responses_backfill_legacy_defaults() {
    let mut mappings = vec![
        json!({"host": "app.example.com"}),
        json!({"host": "auth.example.com", "service_role": "auth", "waf_enabled": false}),
        json!({"host": "legacy-auth.example.com", "target": "http://localhost:7997", "waf_enabled": false}),
        json!({
            "host": "custom.example.com",
            "target_path_mode": "prefix",
            "visibility": {
                "mode": "custom",
                "selections": [],
                "custom_cidrs": ["203.0.113.0/24"],
                "cidrs": ["203.0.113.0/24"]
            }
        }),
    ];

    normalize_host_mapping_response_defaults(&mut mappings);

    assert_eq!(mappings[0]["service_role"], json!("app"));
    assert_eq!(mappings[1]["service_role"], json!("auth"));
    assert_eq!(mappings[2]["service_role"], json!("auth"));
    assert_eq!(mappings[0]["waf_enabled"], json!(true));
    assert_eq!(mappings[1]["waf_enabled"], json!(true));
    assert_eq!(mappings[2]["waf_enabled"], json!(true));
    assert_eq!(mappings[0]["target_path_mode"], json!("entry"));
    assert_eq!(mappings[1]["target_path_mode"], json!("entry"));
    assert_eq!(mappings[2]["target_path_mode"], json!("entry"));
    assert_eq!(mappings[3]["target_path_mode"], json!("prefix"));
    assert_eq!(mappings[0]["visibility"]["mode"], json!("inherit"));
    assert_eq!(mappings[1]["visibility"]["mode"], json!("inherit"));
    assert_eq!(mappings[2]["visibility"]["mode"], json!("inherit"));
    assert_eq!(mappings[3]["visibility"]["mode"], json!("custom"));
    assert_eq!(mappings[0]["favicon_override"], json!(""));
    assert_eq!(
        mappings[3]["visibility"]["cidrs"],
        json!(["203.0.113.0/24"])
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
fn rejects_explicit_invalid_host_target_path_mode() {
    for target_path_mode in [Value::Null, json!("replace"), json!(1)] {
        let error = normalize_host_mappings_for_route(
            vec![json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080/base",
                "target_path_mode": target_path_mode,
            })],
            &json!({}),
        )
        .unwrap_err();
        assert!(error.contains("target path mode must be entry or prefix"));
    }
}

#[test]
fn defaults_missing_host_target_path_mode_to_entry() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080/base",
        })],
        &json!({}),
    )
    .unwrap();
    assert_eq!(
        mappings[0].get("target_path_mode").and_then(Value::as_str),
        Some("entry")
    );
}

#[test]
fn normalizes_host_target_path_mode_case_and_whitespace() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "dav.example.com",
            "target": "http://127.0.0.1:8080/webdav",
            "target_path_mode": " PREFIX ",
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(
        mappings[0].get("target_path_mode").and_then(Value::as_str),
        Some("prefix")
    );
    assert_eq!(
        build_host_rules_payload(&mappings)
            .pointer("/0/target_path_mode")
            .and_then(Value::as_str),
        Some("prefix")
    );
}

#[test]
fn auth_host_mapping_forces_entry_target_path_mode() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "auth.example.com",
            "target": "http://127.0.0.1:7997/auth",
            "service_role": "auth",
            "target_path_mode": "prefix",
            "use_auth": false,
        })],
        &json!({}),
    )
    .unwrap();

    assert_eq!(mappings[0]["target_path_mode"], json!("entry"));

    let stale_persisted_payload = build_host_rules_payload(&[json!({
        "host": "auth.example.com",
        "target": "http://127.0.0.1:7997/auth",
        "target_path_mode": "prefix",
        "use_auth": false,
    })]);
    assert_eq!(
        stale_persisted_payload[0]["target_path_mode"],
        json!("entry")
    );
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
fn preserves_previous_host_target_path_mode_when_legacy_request_omits_it() {
    let mappings = normalize_host_mappings_for_route(
        vec![json!({
            "host": "dav.example.com",
            "target": "http://127.0.0.1:8080/webdav",
        })],
        &json!({
            "host_mappings": [{
                "host": "dav.example.com",
                "target": "http://127.0.0.1:8080/webdav",
                "target_path_mode": "prefix",
            }]
        }),
    )
    .unwrap();

    assert_eq!(
        mappings[0].get("target_path_mode").and_then(Value::as_str),
        Some("prefix")
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
fn validates_go_backend_echoed_target_path_modes() {
    let requested = json!([{
        "host": "dav.example.com",
        "protocol_mode": "auto",
        "target_path_mode": "prefix"
    }]);
    let applied = json!({
        "success": true,
        "data": [{
            "host": "dav.example.com",
            "protocol_mode": "auto",
            "target_path_mode": "prefix"
        }]
    });
    ensure_go_host_protocol_modes_applied(&requested, &applied).unwrap();

    let old_backend = json!({
        "success": true,
        "data": [{"host": "dav.example.com", "protocol_mode": "auto"}]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &old_backend).unwrap_err();
    assert!(error.contains("did not apply target path mode prefix"));

    let entry = json!([{
        "host": "dav.example.com",
        "protocol_mode": "auto",
        "target_path_mode": "entry"
    }]);
    ensure_go_host_protocol_modes_applied(&entry, &old_backend).unwrap();
}

#[test]
fn validates_go_backend_echoed_host_rule_groups() {
    let requested = json!([{
        "host": "video.example.com",
        "protocol_mode": "auto",
        "group_id": "11111111-1111-4111-8111-111111111111",
        "group_name": "Media"
    }]);
    let applied = json!({
        "success": true,
        "data": [{
            "host": "video.example.com",
            "protocol_mode": "auto",
            "group_id": "11111111-1111-4111-8111-111111111111",
            "group_name": "Media"
        }]
    });
    ensure_go_host_protocol_modes_applied(&requested, &applied).unwrap();

    let old_backend = json!({
        "success": true,
        "data": [{"host": "video.example.com", "protocol_mode": "auto"}]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &old_backend).unwrap_err();
    assert!(error.contains("did not apply host rule group for video.example.com"));
}

#[test]
fn validates_go_backend_echoed_host_visibility() {
    let requested = json!([{
        "host": "video.example.com",
        "protocol_mode": "auto",
        "visibility": { "mode": "custom", "cidrs": ["203.0.113.7/24"] }
    }]);
    let applied = json!({
        "success": true,
        "data": [{
            "host": "video.example.com",
            "protocol_mode": "auto",
            "visibility": { "mode": "custom", "cidrs": ["203.0.113.0/24"] }
        }]
    });
    ensure_go_host_protocol_modes_applied(&requested, &applied).unwrap();

    let old_backend = json!({
        "success": true,
        "data": [{ "host": "video.example.com", "protocol_mode": "auto" }]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &old_backend).unwrap_err();
    assert!(error.contains("did not apply host visibility for video.example.com"));

    let disabled = json!([{
        "host": "video.example.com",
        "protocol_mode": "auto",
        "visibility": { "mode": "disabled", "cidrs": [] }
    }]);
    let disabled_applied = json!({
        "success": true,
        "data": [{
            "host": "video.example.com",
            "protocol_mode": "auto",
            "visibility": { "mode": "disabled", "cidrs": [] }
        }]
    });
    ensure_go_host_protocol_modes_applied(&disabled, &disabled_applied).unwrap();
    let error = ensure_go_host_protocol_modes_applied(&disabled, &old_backend).unwrap_err();
    assert!(error.contains("did not apply host visibility for video.example.com"));
}

#[test]
fn validates_advanced_auth_echo_without_comparing_control_metadata() {
    let requested = json!([{
        "host": "video.example.com",
        "protocol_mode": "auto",
        "advanced_auth": {
            "enabled": true,
            "idle_ttl_seconds": 86400,
            "max_lifetime_seconds": 2592000,
            "policy_version": "policy-v1",
            "compiled_at": "2026-01-01T00:00:00Z",
            "cidr_source": "online",
            "cidr_source_fingerprint": "abc123",
            "groups": [{
                "id": "region",
                "conditions": [{
                    "id": "src",
                    "target": "source_region",
                    "operator": "in",
                    "name": "",
                    "policy_id": "ipset-v2:expected",
                    "values": [],
                    "selections": [{ "province": "浙江" }],
                    "cidrs": ["192.0.2.0/24"]
                }]
            }]
        }
    }]);
    let applied = json!({
        "success": true,
        "data": [{
            "host": "video.example.com",
            "protocol_mode": "auto",
            "advanced_auth": {
                "enabled": true,
                "idle_ttl_seconds": 86400,
                "max_lifetime_seconds": 2592000,
                "policy_version": "policy-v1",
                "groups": [{
                    "id": "region",
                    "conditions": [{
                        "id": "src",
                        "target": "source_region",
                        "operator": "in",
                        "name": "",
                        "policy_id": "ipset-v2:expected",
                        "values": null,
                        "cidrs": ["192.0.2.0/24"]
                    }]
                }]
            }
        }]
    });
    ensure_go_host_protocol_modes_applied(&requested, &applied).unwrap();

    let mut wrong_policy = applied.clone();
    wrong_policy["data"][0]["advanced_auth"]["groups"][0]["conditions"][0]["policy_id"] =
        json!("ipset-v2:wrong");
    let error = ensure_go_host_protocol_modes_applied(&requested, &wrong_policy).unwrap_err();
    assert!(error.contains("did not apply advanced authentication"));

    let stale_version = json!({
        "success": true,
        "data": [{
            "host": "video.example.com",
            "protocol_mode": "auto",
            "advanced_auth": {
                "enabled": true,
                "idle_ttl_seconds": 86400,
                "max_lifetime_seconds": 2592000,
                "policy_version": "stale",
                "groups": []
            }
        }]
    });
    let error = ensure_go_host_protocol_modes_applied(&requested, &stale_version).unwrap_err();
    assert!(error.contains("did not apply advanced authentication"));
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
    let changed_icon_override = vec![json!({
        "host": "video.example.com",
        "target": "http://127.0.0.1:8080",
        "protocol_mode": "http1",
        "title": "New title",
        "favicon": "new.ico",
        "favicon_override": "data:image/png;base64,iVBORw0KGgo="
    })];

    assert_eq!(
        host_mappings_revision(&initial),
        host_mappings_revision(&metadata_only)
    );
    assert_ne!(
        host_mappings_revision(&initial),
        host_mappings_revision(&changed_mode)
    );
    assert_ne!(
        host_mappings_revision(&initial),
        host_mappings_revision(&changed_icon_override)
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
        "visibility_policies": {},
        "host_mappings": [{
            "host": "video.example.com",
            "target": format!("http://{metadata_addr}/"),
            "protocol_mode": "http1",
            "title": "Before refresh",
            "favicon": "before.ico"
        }]
    });
    state
        .storage
        .store
        .save_config(&previous_config)
        .await
        .unwrap();

    let response = refresh_host_mapping_titles(State(state.clone())).await;

    assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    metadata_requested_rx.await.unwrap();
    assert_eq!(
        config_without_internal_metadata(state.storage.store.get_config().await.unwrap()),
        previous_config
    );
}

#[tokio::test]
async fn host_mapping_rollback_replays_previous_runtime_payload_after_restoring_store() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let previous_config = json!({
        "run_type": 3,
        "visibility_policies": {},
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "http1",
            "title": "Before refresh"
        }]
    });
    let changed_config = json!({
        "run_type": 3,
        "visibility_policies": {},
        "host_mappings": [{
            "host": "video.example.com",
            "target": "http://127.0.0.1:8080",
            "protocol_mode": "http2",
            "title": "After refresh"
        }]
    });
    state
        .storage
        .store
        .save_config(&changed_config)
        .await
        .unwrap();

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
                .storage
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
        config_without_internal_metadata(state.storage.store.get_config().await.unwrap()),
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
    state
        .storage
        .store
        .save_config(&newer_config)
        .await
        .unwrap();

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
        config_without_internal_metadata(state.storage.store.get_config().await.unwrap()),
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
        .storage
        .store
        .save_config(&json!({
            "host_mappings": expected,
            "unrelated": { "generation": 1 },
            "run_type": 3
        }))
        .await
        .unwrap();
    let expected = first_state
        .storage
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
        second_state.storage.store.get_config().await.unwrap()["host_mappings"]
            .as_array()
            .unwrap(),
    );
    assert_eq!(first_revision, second_revision);

    // Commit an unrelated section from the second state after both writers
    // obtained the same host-mapping revision. The section CAS must merge into
    // this latest full document instead of restoring generation 1.
    let mut unrelated_update = second_state.storage.store.get_config().await.unwrap();
    unrelated_update["unrelated"]["generation"] = json!(2);
    second_state
        .storage
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
                .storage
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
                .storage
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
    let final_config = first_state.storage.store.get_config().await.unwrap();
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
        .storage
        .store
        .set_json_value(
            HOST_MAPPINGS_TRANSACTION_LOCK_KEY,
            &json!({ "lockId": "new-owner" }),
        )
        .await
        .unwrap();
    assert!(lease.release().await.is_err());
    state
        .storage
        .store
        .delete_key(HOST_MAPPINGS_TRANSACTION_LOCK_KEY)
        .await
        .unwrap();

    let result = with_host_mappings_runtime_transaction(&state, |state| async move {
        state
            .storage
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
        .storage
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
        .storage
        .store
        .save_config(&json!({ "host_mappings": initial_mappings }))
        .await
        .unwrap();
    let initial_mappings =
        mutation_state.storage.store.get_config().await.unwrap()["host_mappings"]
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
            let mappings = state.storage.store.get_config().await.unwrap()["host_mappings"]
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
        .storage
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
        .storage
        .store
        .save_config(&json!({
            "host_mappings": initial_mappings,
            "unrelated": { "generation": 1 }
        }))
        .await
        .unwrap();

    // This snapshot carries generation N and is intentionally held until
    // after another AppState commits host generation N+1.
    let mut stale_full_config = full_writer_state.storage.store.get_config().await.unwrap();
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
        .storage
        .store
        .compare_and_set_host_mappings(&initial_mappings, &next_mappings)
        .await
        .unwrap()
        .expect("host section CAS succeeds");

    stale_full_config["unrelated"]["generation"] = json!(2);
    let stale_save = full_writer_state
        .storage
        .store
        .save_config(&stale_full_config)
        .await;
    assert!(stale_save.is_err());

    let final_config = host_state.storage.store.get_config().await.unwrap();
    assert_eq!(final_config["host_mappings"], json!(next_mappings));
    assert_eq!(final_config["unrelated"]["generation"], json!(1));
    assert_eq!(
        host_state
            .storage
            .store
            .get_string_value("fn_knock:config:host_mappings:generation")
            .await
            .unwrap()
            .as_deref(),
        Some("2")
    );
    let persisted_raw = host_state
        .storage
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
fn validates_and_normalizes_host_mapping_groups() {
    let groups = normalize_host_mapping_groups(vec![
        json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "name": "  媒体服务  "
        }),
        json!({
            "id": "22222222-2222-4222-8222-222222222222",
            "name": "Tools"
        }),
    ])
    .unwrap();
    assert_eq!(groups[0]["name"], json!("媒体服务"));

    let duplicate = normalize_host_mapping_groups(vec![
        json!({
            "id": "11111111-1111-4111-8111-111111111111",
            "name": "Tools"
        }),
        json!({
            "id": "22222222-2222-4222-8222-222222222222",
            "name": " tools "
        }),
    ])
    .unwrap_err();
    assert_eq!(duplicate, "Duplicate host mapping group name tools");
}

#[test]
fn legacy_host_mapping_update_preserves_existing_group_assignment() {
    let group_id = "11111111-1111-4111-8111-111111111111";
    let previous = json!({
        "host_mapping_groups": [{ "id": group_id, "name": "Media" }],
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "group_id": group_id
        }]
    });
    let normalized = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8081",
            "use_auth": true
        })],
        &previous,
    )
    .unwrap();
    assert_eq!(normalized[0]["group_id"], json!(group_id));

    let explicitly_ungrouped = normalize_host_mappings_for_route(
        vec![json!({
            "host": "app.example.com",
            "target": "http://127.0.0.1:8081",
            "use_auth": true,
            "group_id": null
        })],
        &previous,
    )
    .unwrap();
    assert_eq!(explicitly_ungrouped[0]["group_id"], Value::Null);

    let renamed = normalize_host_mappings_for_route(
        vec![json!({
            "host": "renamed.example.com",
            "target": "http://127.0.0.1:8080",
            "use_auth": true
        })],
        &previous,
    )
    .unwrap();
    assert_eq!(renamed[0]["group_id"], json!(group_id));

    let added_alias = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "app.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
            json!({
                "host": "alias.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
        ],
        &previous,
    )
    .unwrap();
    assert_eq!(added_alias[0]["group_id"], json!(group_id));
    assert_eq!(added_alias[1]["group_id"], Value::Null);
}

#[test]
fn host_mapping_rename_preserves_advanced_auth_across_identity_edges() {
    let advanced_auth = json!({
        "enabled": true,
        "idle_ttl_seconds": 3_600,
        "max_lifetime_seconds": 86_400,
        "policy_version": "advanced-policy-v1",
        "groups": [{
            "id": "group-1",
            "conditions": [{
                "id": "condition-1",
                "target": "url_path",
                "operator": "prefix",
                "values": ["/private"]
            }]
        }]
    });
    let previous = json!({
        "host_mappings": [
            {
                "host": "old.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true,
                "advanced_auth": advanced_auth
            },
            {
                "host": "alias.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }
        ]
    });

    let explicit = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "alias.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
            json!({
                "host": "new.example.com",
                "previous_host": "old.example.com",
                "target": "http://127.0.0.1:9090",
                "use_auth": true
            }),
        ],
        &previous,
    )
    .unwrap();
    assert_eq!(explicit[1]["advanced_auth"], advanced_auth);
    assert!(explicit[1].get("previous_host").is_none());

    let legacy_shared_target = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "alias.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
            json!({
                "host": "new.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
        ],
        &previous,
    )
    .unwrap();
    assert_eq!(legacy_shared_target[1]["advanced_auth"], advanced_auth);

    let disabled_auth = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "alias.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
            json!({
                "host": "new.example.com",
                "previous_host": "old.example.com",
                "target": "http://127.0.0.1:9090",
                "use_auth": false
            }),
        ],
        &previous,
    )
    .unwrap();
    assert_eq!(disabled_auth[1]["advanced_auth"]["enabled"], json!(false));
    assert_eq!(
        disabled_auth[1]["advanced_auth"]["groups"],
        advanced_auth["groups"]
    );
}

#[test]
fn explicit_previous_host_rejects_ambiguous_or_stale_rename_claims() {
    let previous = json!({
        "host_mappings": [{
            "host": "old.example.com",
            "target": "http://127.0.0.1:8080",
            "use_auth": true
        }]
    });

    let still_present = normalize_host_mappings_for_route(
        vec![
            json!({
                "host": "old.example.com",
                "target": "http://127.0.0.1:8080",
                "use_auth": true
            }),
            json!({
                "host": "new.example.com",
                "previous_host": "old.example.com",
                "target": "http://127.0.0.1:9090",
                "use_auth": true
            }),
        ],
        &previous,
    )
    .unwrap_err();
    assert_eq!(
        still_present,
        "Previous host mapping old.example.com is still present"
    );

    let missing = normalize_host_mappings_for_route(
        vec![json!({
            "host": "new.example.com",
            "previous_host": "missing.example.com",
            "target": "http://127.0.0.1:9090",
            "use_auth": true
        })],
        &previous,
    )
    .unwrap_err();
    assert_eq!(
        missing,
        "Previous host mapping missing.example.com does not exist"
    );
}

#[test]
fn host_rule_payload_uses_group_and_mapping_order_with_ungrouped_last() {
    let media = "11111111-1111-4111-8111-111111111111";
    let tools = "22222222-2222-4222-8222-222222222222";
    let payload = build_host_rules_payload_for_config(&json!({
        "host_mapping_grouped_view": true,
        "host_mapping_groups": [
            { "id": media, "name": "Media" },
            { "id": tools, "name": "Tools" }
        ],
        "host_mappings": [
            { "host": "tool.example.com", "target": "http://127.0.0.1:8081", "group_id": tools },
            { "host": "loose.example.com", "target": "http://127.0.0.1:8082", "group_id": null },
            { "host": "media.example.com", "target": "http://127.0.0.1:8080", "group_id": media }
        ]
    }));
    let rules = payload["items"].as_array().unwrap();
    assert_eq!(
        rules
            .iter()
            .map(|rule| rule["host"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["media.example.com", "tool.example.com", "loose.example.com"]
    );
    assert_eq!(rules[0]["group_name"], json!("Media"));
    assert_eq!(rules[2]["group_id"], json!(""));
}

#[test]
fn host_rule_payload_stays_flat_when_grouped_view_is_disabled() {
    let group_id = "11111111-1111-4111-8111-111111111111";
    let payload = build_host_rules_payload_for_config(&json!({
        "host_mapping_grouped_view": false,
        "host_mapping_groups": [{ "id": group_id, "name": "Media" }],
        "host_mappings": [
            { "host": "loose.example.com", "target": "http://127.0.0.1:8081", "group_id": null },
            { "host": "media.example.com", "target": "http://127.0.0.1:8080", "group_id": group_id }
        ]
    }));
    let rules = payload["items"].as_array().unwrap();
    assert_eq!(
        rules
            .iter()
            .map(|rule| rule["host"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["loose.example.com", "media.example.com"]
    );
    assert_eq!(rules[0]["group_id"], json!(""));
    assert_eq!(rules[1]["group_id"], json!(""));
    assert_eq!(rules[1]["group_name"], json!(""));
}

#[test]
fn disabled_advanced_auth_draft_is_not_sent_to_gateway() {
    let policy = compile_ip_set(["203.0.113.0/24"]).unwrap();
    let policy_id = policy.id.clone();
    let payload = build_host_rules_payload_for_config(&json!({
        "host_mappings": [{
            "host": "app.example.com",
            "target": "http://127.0.0.1:8080",
            "advanced_auth": {
                "enabled": false,
                "policy_version": "draft-v1",
                "groups": [{
                    "id": "group-1",
                    "conditions": [{
                        "id": "condition-1",
                        "target": "source_region",
                        "operator": "in",
                        "policy_id": policy_id,
                        "selections": [{ "province": "甘肃", "city": "定西", "operator": "移动" }]
                    }]
                }]
            }
        }],
        "visibility_policies": {
            (policy.id.clone()): policy.to_config_value()
        }
    }));

    assert_eq!(
        payload.pointer("/items/0/advanced_auth"),
        Some(&json!({ "enabled": false }))
    );
    assert_eq!(
        payload["visibility_policies"].as_array().map(Vec::len),
        Some(0)
    );
}

#[test]
fn catalog_revision_tracks_group_order_and_names() {
    let mappings = vec![json!({
        "host": "app.example.com",
        "target": "http://127.0.0.1:8080",
        "group_id": "11111111-1111-4111-8111-111111111111"
    })];
    let first = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Media"
    })];
    let renamed = vec![json!({
        "id": "11111111-1111-4111-8111-111111111111",
        "name": "Media apps"
    })];
    assert_ne!(
        host_mapping_catalog_revision(&mappings, &first, false),
        host_mapping_catalog_revision(&mappings, &renamed, false)
    );
    assert_ne!(
        host_mapping_catalog_revision(&mappings, &first, false),
        host_mapping_catalog_revision(&mappings, &first, true)
    );
}

#[test]
fn catalog_revision_normalizes_stored_group_shape() {
    let config = json!({
        "host_mapping_grouped_view": true,
        "host_mapping_groups": [{
            "id": "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA",
            "name": "  Media  "
        }],
        "host_mappings": []
    });
    let normalized_groups =
        normalize_host_mapping_groups(host_mapping_groups_from_config(&config)).unwrap();
    assert_eq!(
        host_mapping_catalog_revision_from_config(&config),
        host_mapping_catalog_revision(&[], &normalized_groups, true)
    );
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
                "title_override": "Portal",
                "favicon": "data:image/png;base64,YXV0bw==",
                "favicon_override": "data:image/webp;base64,Y3VzdG9tJm1vcmU="
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
    assert!(document.contains("ICON=\"data:image/webp;base64,Y3VzdG9tJm1vcmU=\""));
    assert!(!document.contains("ICON=\"data:image/png;base64,YXV0bw==\""));
    assert!(!document.contains("auth.example.com"));
    assert_eq!(
        build_bookmark_filename(&config),
        "fn-knock-bookmarks-example.com.html"
    );
}

#[test]
fn builds_nested_group_bookmarks_with_escaping_and_ungrouped_last() {
    let group_id = "11111111-1111-4111-8111-111111111111";
    let config = json!({
        "host_mapping_grouped_view": true,
        "host_mapping_groups": [{
            "id": group_id,
            "name": "Media <script>"
        }],
        "host_mappings": [
            {
                "host": "loose.example.com",
                "target": "http://127.0.0.1:8081",
                "group_id": null
            },
            {
                "host": "media.example.com",
                "target": "http://127.0.0.1:8080",
                "group_id": group_id
            },
            {
                "host": "auth.example.com",
                "target": "http://127.0.0.1:7997",
                "group_id": null
            }
        ]
    });
    let document = build_bookmarks_document(&config, &crate::i18n::Translator::new("en"));
    assert!(document.contains("Media &lt;script&gt;"));
    assert!(!document.contains("auth.example.com"));
    assert!(document.find("media.example.com").unwrap() < document.find("Ungrouped").unwrap());
    assert!(document.find("Ungrouped").unwrap() < document.find("loose.example.com").unwrap());
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
    assert_eq!(
        normalize_stream_mappings(vec![json!({
            "protocol": "udp",
            "listen_port": 5353,
            "target": "[::1]:53",
            "use_auth": false,
            "comment": "  Local DNS  "
        })])
        .unwrap(),
        vec![json!({
            "protocol": "udp",
            "listen_port": 5353,
            "target": "[::1]:53",
            "use_auth": false,
            "comment": "Local DNS"
        })]
    );
    assert_eq!(
        normalize_stream_mappings(vec![json!({
            "protocol": "tcp",
            "listen_port": 2222,
            "target": "127.0.0.1:22"
        })])
        .unwrap()[0]
            .get("comment"),
        Some(&json!(""))
    );
}

#[test]
fn rejects_stream_mappings_that_loop_to_the_same_local_port() {
    let local_addresses = HashSet::from([
        "192.0.2.10".parse().expect("local IPv4"),
        "2001:db8::10".parse().expect("local IPv6"),
    ]);
    for target in [
        "127.0.0.1:5555",
        "localhost:5555",
        "192.0.2.10:5555",
        "[::1]:5555",
        "[2001:db8::10]:5555",
    ] {
        let mappings = normalize_stream_mappings(vec![json!({
            "protocol": "tcp",
            "listen_port": 5555,
            "target": target
        })])
        .expect("structurally valid stream mapping");
        assert_eq!(
            validate_stream_mapping_local_loops_with_addresses(&mappings, &local_addresses)
                .expect_err("same-port local target should be rejected"),
            format!(
                "Stream mapping TCP listen_port 5555 cannot target the same local port {target}"
            )
        );
    }

    let safe_mappings = normalize_stream_mappings(vec![
        json!({
            "protocol": "tcp",
            "listen_port": 15555,
            "target": "127.0.0.1:5555"
        }),
        json!({
            "protocol": "udp",
            "listen_port": 5555,
            "target": "192.0.2.20:5555"
        }),
    ])
    .expect("safe stream mappings");
    validate_stream_mapping_local_loops_with_addresses(&safe_mappings, &local_addresses)
        .expect("different port or remote host should be allowed");
}

#[test]
fn disabled_stream_mappings_can_repair_legacy_local_loops_incrementally() {
    let legacy = normalize_stream_mappings(vec![
        json!({
            "protocol": "tcp",
            "listen_port": 5555,
            "target": "127.0.0.1:5555",
            "comment": "legacy TCP"
        }),
        json!({
            "protocol": "udp",
            "listen_port": 5555,
            "target": "127.0.0.1:5555",
            "comment": "legacy UDP"
        }),
    ])
    .expect("legacy mapping");
    let one_removed = normalize_stream_mappings(vec![json!({
        "protocol": "udp",
        "listen_port": 5555,
        "target": "127.0.0.1:5555",
        "comment": "needs repair"
    })])
    .expect("comment update");

    assert!(stream_mapping_update_only_removes_entries(
        &legacy,
        &one_removed
    ));
    validate_stream_mapping_update_safety(&legacy, &one_removed, true)
        .expect("repair should preserve the remaining unchanged legacy loop");
    assert!(
        validate_stream_mapping_update_safety(&legacy, &one_removed, false).is_err(),
        "enabled feature must reject every local loop"
    );
    validate_stream_mapping_update_safety(&legacy, &[], true)
        .expect("all legacy loops can be removed at once");

    let repaired = normalize_stream_mappings(vec![json!({
        "protocol": "tcp",
        "listen_port": 15555,
        "target": "127.0.0.1:5555",
        "comment": "repaired"
    })])
    .expect("repaired mapping");
    validate_stream_mapping_update_safety(&legacy, &repaired, false)
        .expect("repaired mapping should be accepted");
    assert!(
        !stream_mapping_update_only_removes_entries(&legacy, &repaired),
        "changing a forwarding identity is an edit, not a removal-only update"
    );
}

#[tokio::test]
async fn disabled_stream_mapping_deletion_does_not_depend_on_gateway_runtime() {
    let unavailable_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let unavailable_grpc_addr = unavailable_listener.local_addr().unwrap();
    drop(unavailable_listener);
    let (_directory, state) = proxy_config_test_state(unavailable_grpc_addr.to_string()).await;

    let mappings = json!([
        {
            "protocol": "tcp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true,
            "comment": "legacy TCP"
        },
        {
            "protocol": "udp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true,
            "comment": "legacy UDP"
        }
    ]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    ensure_object(&mut config).insert("stream_mappings".to_string(), mappings);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:protocol-mapping:feature",
            &json!({ "enabled": false }),
        )
        .await
        .expect("disable protocol mappings");

    let remaining = json!({
        "protocol": "udp",
        "listen_port": 12333,
        "target": "127.0.0.1:12333",
        "use_auth": true,
        "comment": "legacy UDP"
    });
    let response = update_stream_mappings(
        State(state.clone()),
        Json(MappingsBody {
            mappings: vec![remaining.clone()],
            revision: None,
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("stream_mappings"),
        Some(&json!([remaining]))
    );
}

#[tokio::test]
async fn enabled_stream_mapping_can_delete_the_only_legacy_udp_loop() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let mappings = json!([{
        "protocol": "udp",
        "listen_port": 12333,
        "target": "127.0.0.1:12333",
        "use_auth": true,
        "comment": "legacy UDP"
    }]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    ensure_object(&mut config).insert("stream_mappings".to_string(), mappings);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:protocol-mapping:feature",
            &json!({ "enabled": true }),
        )
        .await
        .expect("enable protocol mappings");

    let response = update_stream_mappings_with_runtime_sync(
        state.clone(),
        MappingsBody {
            mappings: vec![],
            revision: None,
        },
        |_state, updated_config| async move {
            assert_eq!(updated_config.get("stream_mappings"), Some(&json!([])));
            Ok(())
        },
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("stream_mappings"),
        Some(&json!([]))
    );
    assert_eq!(
        runtime_config::load_protocol_mapping_feature(&state, None)
            .await
            .expect("reload feature"),
        json!({ "enabled": true, "availability": null })
    );
}

#[tokio::test]
async fn stream_mapping_update_waits_for_the_protocol_mapping_transaction_lock() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let mappings = json!([
        {
            "protocol": "tcp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true
        },
        {
            "protocol": "udp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true
        }
    ]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    ensure_object(&mut config).insert("stream_mappings".to_string(), mappings);
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:protocol-mapping:feature",
            &json!({ "enabled": true }),
        )
        .await
        .expect("enable protocol mappings");

    let guard = state.gateway.protocol_mapping_update_lock.lock().await;
    let task_state = state.clone();
    let mut task = tokio::spawn(async move {
        update_stream_mappings(
            State(task_state),
            Json(MappingsBody {
                mappings: vec![json!({
                    "protocol": "udp",
                    "listen_port": 12333,
                    "target": "127.0.0.1:12333",
                    "use_auth": true
                })],
                revision: None,
            }),
        )
        .await
    });

    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut task)
            .await
            .is_err(),
        "mapping update must wait while the shared transaction lock is held"
    );
    drop(guard);
    let response = tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .expect("mapping update should finish after releasing the lock")
        .expect("mapping update task");
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn enabled_legacy_loop_cleanup_requires_disabling_before_persisting() {
    let (_directory, state) = proxy_config_test_state("127.0.0.1:1".to_string()).await;
    let mappings = json!([
        {
            "protocol": "tcp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true,
            "comment": "legacy TCP"
        },
        {
            "protocol": "udp",
            "listen_port": 12333,
            "target": "127.0.0.1:12333",
            "use_auth": true,
            "comment": "legacy UDP"
        }
    ]);
    let mut config = state.storage.store.get_config().await.expect("load config");
    ensure_object(&mut config).insert("stream_mappings".to_string(), mappings.clone());
    state
        .storage
        .store
        .save_config(&config)
        .await
        .expect("save config");
    state
        .storage
        .store
        .set_json_value(
            "fn_knock:protocol-mapping:feature",
            &json!({ "enabled": true }),
        )
        .await
        .expect("enable protocol mappings");

    let response = update_stream_mappings(
        State(state.clone()),
        Json(MappingsBody {
            mappings: vec![json!({
                "protocol": "udp",
                "listen_port": 12333,
                "target": "127.0.0.1:12333",
                "use_auth": true,
                "comment": "legacy UDP"
            })],
            revision: None,
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::CONFLICT);
    let response_body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    let response_json: Value = serde_json::from_slice(&response_body).expect("parse response body");
    assert_eq!(
        response_json.get("code"),
        Some(&json!(STREAM_MAPPING_LEGACY_REPAIR_REQUIRED_CODE))
    );
    assert_eq!(
        state
            .storage
            .store
            .get_config()
            .await
            .expect("reload config")
            .get("stream_mappings"),
        Some(&mappings)
    );
    assert_eq!(
        runtime_config::load_protocol_mapping_feature(&state, None)
            .await
            .expect("reload feature"),
        json!({ "enabled": true, "availability": null })
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
        localize_proxy_config_error(
            &translator,
            "Stream mapping TCP listen_port 5555 cannot target the same local port 127.0.0.1:5555"
        ),
        "TCP 监听端口 5555 不能转发到本机同一端口（127.0.0.1:5555），否则会形成循环；请修改对外端口或目标端口"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            r#"go backend gRPC request failed: {"message":"failed to set stream rules: cannot target the same local listen_port 5555"}"#
        ),
        "监听端口 5555 不能转发到本机同一端口，否则会形成循环；请进入协议映射修改对外端口或目标端口"
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
            "Host mapping new.example.com previous host is invalid"
        ),
        "Host 映射 new.example.com 的原域名无效"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping new.example.com already exists and cannot be renamed from old.example.com"
        ),
        "Host 映射 new.example.com 已存在，不能从 old.example.com 重命名"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Previous host mapping old.example.com is still present"
        ),
        "原 Host 映射 old.example.com 仍在列表中，无法作为重命名来源"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Previous host mapping old.example.com does not exist"
        ),
        "原 Host 映射 old.example.com 不存在"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Previous host mapping old.example.com is claimed more than once"
        ),
        "原 Host 映射 old.example.com 被多条映射重复认领"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping new.example.com is submitted more than once"
        ),
        "Host 映射域名 new.example.com 重复"
    );
    assert_eq!(
        localize_proxy_config_error(&translator, "Subdomain root domain cannot contain wildcard"),
        "根域名不能包含通配符 *。请填写 example.com，而不是 *.example.com。"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping auth.*.example.com cannot contain wildcard"
        ),
        "Host 映射 auth.*.example.com 不能包含通配符 *，请填写精确域名"
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
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping dav.example.com target path mode must be entry or prefix"
        ),
        "Host 映射 dav.example.com 的目标路径模式必须是 entry 或 prefix"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Go backend did not apply target path mode prefix for dav.example.com (reported entry); upgrade the gateway backend"
        ),
        "网关后端未应用 dav.example.com 的目标路径模式 prefix，请升级网关后端"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Host mapping video.example.com visibility: 自定义可见性至少需要一个地区或一条 CIDR"
        ),
        "Host 映射 video.example.com 的可见性配置无效：自定义可见性至少需要一个地区或一条 CIDR"
    );
    assert_eq!(
        localize_proxy_config_error(
            &translator,
            "Go backend did not apply host visibility for video.example.com; upgrade the gateway backend"
        ),
        "网关后端未应用 video.example.com 的可见性规则，请升级网关后端"
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

#[test]
fn gateway_auth_config_omits_origin_port_for_edge_providers() {
    for (provider, aliyun_esa_enabled, tencent_edgeone_enabled) in [
        ("Aliyun ESA", true, false),
        ("Tencent EdgeOne", false, true),
    ] {
        let config = json!({
            "run_type": 3,
            "host_mappings": [{
                "host": "auth.edge.example",
                "target": "http://127.0.0.1:7997"
            }],
            "subdomain_mode": {
                "edge_client_ip_enabled": true,
                "aliyun_esa_enabled": aliyun_esa_enabled,
                "tencent_edgeone_enabled": tencent_edgeone_enabled,
                "public_auth_base_url": "https://auth.edge.example:7999",
                "public_https_port": 7999
            }
        });

        let auth = build_gateway_auth_config(&config);
        assert_eq!(
            auth.get("public_auth_base_url").and_then(Value::as_str),
            Some("https://auth.edge.example"),
            "provider={provider}"
        );
        assert_eq!(
            auth.get("public_https_port").and_then(Value::as_i64),
            Some(0),
            "provider={provider}"
        );
    }
}

#[test]
fn gateway_auth_config_omits_stale_origin_port_for_cloudflared() {
    let config = json!({
        "run_type": 1,
        "reverse_proxy_submode": "subdomain",
        "default_tunnel": "cloudflared",
        "host_mappings": [{
            "host": "auth.tunnel.example",
            "target": "http://127.0.0.1:7997"
        }],
        "subdomain_mode": {
            "public_auth_base_url": "https://auth.tunnel.example:7999",
            "public_https_port": 7999
        }
    });

    let auth = build_gateway_auth_config(&config);
    assert_eq!(
        auth.get("public_auth_base_url").and_then(Value::as_str),
        Some("https://auth.tunnel.example")
    );
    assert_eq!(
        auth.get("public_https_port").and_then(Value::as_i64),
        Some(0)
    );
}
