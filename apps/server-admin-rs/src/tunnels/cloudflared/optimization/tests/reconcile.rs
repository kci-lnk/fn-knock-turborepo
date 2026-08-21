#[test]
fn cloudflare_route_rejections_preserve_the_actionable_cause() {
    assert_eq!(
        cloudflare_route_rejection_message(
            403,
            "cloudflare error 1000: dns points to prohibited ip"
        )
        .as_deref(),
        Some("Cloudflare Error 1000: DNS points to a prohibited Cloudflare IP")
    );
    assert_eq!(
        cloudflare_route_rejection_message(530, "error 1016").as_deref(),
        Some("Cloudflare Error 1016: origin DNS resolution failed")
    );
    assert_eq!(
        cloudflare_route_rejection_message(522, "gateway unavailable").as_deref(),
        Some("Cloudflare edge returned HTTP 522")
    );
    assert_eq!(cloudflare_route_rejection_message(200, "ok"), None);
}

#[test]
fn control_plane_refresh_does_not_republish_a_suppressed_route() {
    let mut host_state = json!({
        "id": "custom-fallback",
        "status": "fallback",
        "hostnameStatus": "pending",
        "sslStatus": "pending_validation",
    });
    let changed = update_custom_hostname_activation(
        &mut host_state,
        &json!({
            "status": "active",
            "ssl": { "status": "active" },
        }),
    );

    assert!(changed);
    assert_eq!(
        host_state.get("status").and_then(Value::as_str),
        Some("fallback")
    );
    assert_eq!(
        host_state.get("hostnameStatus").and_then(Value::as_str),
        Some("active")
    );
    assert_eq!(
        host_state.get("sslStatus").and_then(Value::as_str),
        Some("active")
    );
    assert!(host_state.get("exactDnsId").is_none());
    assert!(custom_hostname_can_validate_candidates(&host_state));
}

#[test]
fn capability_route_failure_remains_retryable() {
    let failed = capability_probe_failure_state(
        &json!({
            "id": "capability-id",
            "hostname": "probe.example.com",
            "status": "pending",
            "hostnameStatus": "active",
            "sslStatus": "active",
            "activationDns": { "id": "activation-id" },
        }),
        "Cloudflare edge returned HTTP 530",
    );

    assert_eq!(
        failed.get("status").and_then(Value::as_str),
        Some("probe-failed")
    );
    assert_eq!(
        failed.get("messageCode").and_then(Value::as_str),
        Some("preferredEdgeProbeFailed")
    );
    assert!(failed.get("reasonCode").is_none());
    assert!(capability_probe_hostname_is_ready(&failed));
    assert!(!capability_probe_is_definitively_unsupported(&json!({
        "status": "unsupported",
        "message": "Cloudflare edge returned HTTP 530",
    })));
    assert!(capability_probe_is_definitively_unsupported(&json!({
        "status": "unsupported",
        "reasonCode": CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE,
    })));
}

#[test]
fn failed_edge_route_state_is_safe_for_origin_fallback_and_rescan() {
    let mut host_state = json!({
        "id": "custom-id",
        "status": "optimized",
        "hostnameStatus": "active",
        "sslStatus": "active",
        "exactDnsId": "edge-record",
        "exactDnsTarget": "edge",
    });
    assert!(exact_route_is_optimized(&host_state));

    set_exact_dns_route(&mut host_state, &json!({ "id": "origin-record" }), "origin");
    record_preferred_edge_probe_failure(&mut host_state, "Cloudflare edge returned HTTP 522");

    assert!(!exact_route_is_optimized(&host_state));
    assert_eq!(
        host_state.get("exactDnsTarget").and_then(Value::as_str),
        Some("origin")
    );
    assert_eq!(
        host_state.get("status").and_then(Value::as_str),
        Some("probe-failed")
    );
    assert!(custom_hostname_can_validate_candidates(&host_state));
}

#[test]
fn scans_require_an_applied_managed_optimization_plan() {
    assert!(!optimization_is_enabled(&json!({})));
    assert!(!optimization_is_enabled(&json!({
        "mode": "managed",
        "optimizationEnabled": false,
    })));
    assert!(!optimization_is_enabled(&json!({
        "mode": "manual",
        "optimizationEnabled": true,
    })));
    assert!(optimization_is_enabled(&json!({
        "mode": "managed",
        "optimizationEnabled": true,
    })));
}

#[test]
fn activation_cname_is_not_reported_as_an_optimized_route() {
    assert!(!exact_route_is_optimized(&json!({
        "exactDnsId": "dns-id",
        "exactDnsTarget": "origin",
        "status": "pending",
    })));
    assert!(exact_route_is_optimized(&json!({
        "exactDnsId": "dns-id",
        "exactDnsTarget": "edge",
        "status": "optimized",
    })));
    assert!(exact_route_is_optimized(&json!({
        "exactDnsId": "legacy-dns-id",
        "status": "optimized",
    })));
}

#[test]
fn health_checks_ignore_origin_activation_and_unready_certificates() {
    let ownership = json!({
        "optimization": {
            "customHostnames": {
                "activation.example.com": {
                    "exactDnsId": "activation-id",
                    "exactDnsTarget": "origin",
                    "status": "pending",
                    "sslStatus": "pending_validation"
                },
                "unready.example.com": {
                    "exactDnsId": "unready-id",
                    "exactDnsTarget": "edge",
                    "status": "optimized",
                    "sslStatus": "pending_validation"
                },
                "ready.example.com": {
                    "exactDnsId": "ready-id",
                    "exactDnsTarget": "edge",
                    "status": "optimized",
                    "sslStatus": "active"
                }
            }
        }
    });
    assert_eq!(
        optimized_health_hostname(&ownership).as_deref(),
        Some("ready.example.com")
    );

    let only_activation = json!({
        "optimization": {
            "customHostnames": {
                "activation.example.com": {
                    "exactDnsId": "activation-id",
                    "exactDnsTarget": "origin",
                    "sslStatus": "active"
                }
            }
        }
    });
    assert_eq!(optimized_health_hostname(&only_activation), None);
}

#[test]
fn legacy_publish_suppression_preserves_only_explicit_fallbacks() {
    let fallback = json!({ "optimization": { "fallbackActive": true } });
    assert!(legacy_publish_suppression(
        &fallback,
        &json!({ "lastSwitchReason": "health-fallback" })
    ));
    assert!(legacy_publish_suppression(
        &fallback,
        &json!({ "lastSwitchReason": "manual-fallback" })
    ));
    assert!(!legacy_publish_suppression(
        &fallback,
        &json!({ "lastSwitchReason": "manual-speed-test" })
    ));
    assert!(!legacy_publish_suppression(
        &json!({ "optimization": { "fallbackActive": false } }),
        &json!({ "lastSwitchReason": "health-fallback" })
    ));
}

#[test]
fn dns_ownership_is_scoped_to_the_current_instance() {
    let own = json!({
        "comment": "Managed by fn-knock (instance-a)",
        "tags": ["fn-knock:managed", "fn-knock-instance:instance-a"]
    });
    let other = json!({
        "comment": "Managed by fn-knock (instance-b)",
        "tags": ["fn-knock:managed", "fn-knock-instance:instance-b"]
    });
    let legacy_generic = json!({ "tags": ["fn-knock:managed"] });
    assert!(is_managed_dns(&own, "instance-a"));
    assert!(!is_managed_dns(&other, "instance-a"));
    assert!(!is_managed_dns(&legacy_generic, "instance-a"));
}

#[test]
fn fallback_suppresses_automatic_exact_route_republication() {
    let ownership = json!({ "optimization": { "publishSuppressed": true } });
    assert!(!should_publish_exact_routes(&ownership, false));
    assert!(should_publish_exact_routes(&ownership, true));
    assert!(should_publish_exact_routes(
        &json!({ "optimization": { "fallbackActive": true } }),
        false
    ));
    assert!(should_publish_exact_routes(&json!({}), false));
}

#[test]
fn fallback_keeps_only_the_activation_dns_needed_for_hostname_provisioning() {
    assert!(custom_hostname_needs_activation_dns(
        false,
        "pending",
        "pending_validation"
    ));
    assert!(custom_hostname_needs_activation_dns(
        false,
        "active",
        "pending_validation"
    ));
    assert!(!custom_hostname_needs_activation_dns(
        false, "active", "active"
    ));
    assert!(custom_hostname_needs_activation_dns(
        true, "active", "active"
    ));
}

#[test]
fn stale_fn_knock_custom_hostname_requires_explicit_takeover() {
    let conflict = custom_hostname_ownership_conflict(
        &json!({
            "hostname": "app.tu.example.com",
            "custom_origin_server": "fnknock-origin-7f531e6dd1e4.tu.example.com"
        }),
        "app.tu.example.com",
        "tu.example.com",
    );
    assert_eq!(
        conflict.get("status").and_then(Value::as_str),
        Some("conflict")
    );
    assert_eq!(
        conflict.get("messageDetail").and_then(Value::as_str),
        Some("7f531e6dd1e4")
    );
    assert_eq!(
        conflict.get("conflictResourceId").and_then(Value::as_str),
        Some("custom-hostname:app.tu.example.com")
    );
}
use super::*;
