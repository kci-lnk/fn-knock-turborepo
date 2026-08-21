#[test]
fn recovers_only_a_fully_verified_previous_fn_knock_lineage() {
    let custom = json!({
        "id": "custom-id",
        "hostname": "auth.tu.example.com",
        "custom_origin_server": "fnknock-origin-7f531e6dd1e4.tu.example.com",
        "status": "active",
        "ssl": { "status": "active" },
    });
    let exact = json!([{
        "id": "exact-id",
        "name": "auth.tu.example.com",
        "type": "CNAME",
        "content": "fnknock-edge-7f531e6dd1e4.tu.example.com",
        "proxied": false,
        "comment": "Managed by fn-knock (7f531e6dd1e4)",
        "tags": [],
    }]);
    let origin = json!([{
        "id": "origin-id",
        "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
        "type": "CNAME",
        "content": "b8e3c226-e512-4232-a5a1-3fbdc590e880.cfargotunnel.com",
        "proxied": true,
        "comment": "Managed by fn-knock (7f531e6dd1e4)",
        "tags": [],
    }]);
    let recovered = recoverable_fn_knock_custom_hostname_from_snapshot(
        &custom,
        exact.as_array().unwrap(),
        origin.as_array().unwrap(),
        Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
        "tu.example.com",
        "f63f7fcb2f0f",
        None,
    )
    .expect("verified previous fn-knock lineage should be recoverable");
    assert_eq!(recovered.legacy_instance_id, "7f531e6dd1e4");
    assert_eq!(recovered.exact_dns["id"], json!("exact-id"));
    assert_eq!(recovered.origin_dns["id"], json!("origin-id"));

    assert!(
        recoverable_fn_knock_custom_hostname_from_snapshot(
            &custom,
            exact.as_array().unwrap(),
            origin.as_array().unwrap(),
            Some("fnknock-origin-another000000.tu.example.com"),
            "tu.example.com",
            "f63f7fcb2f0f",
            None,
        )
        .is_none()
    );
    let unrelated_exact = json!([{
        "id": "exact-id",
        "name": "auth.tu.example.com",
        "type": "CNAME",
        "content": "fnknock-edge-7f531e6dd1e4.tu.example.com",
        "proxied": false,
        "comment": "managed manually",
        "tags": [],
    }]);
    assert!(
        recoverable_fn_knock_custom_hostname_from_snapshot(
            &custom,
            unrelated_exact.as_array().unwrap(),
            origin.as_array().unwrap(),
            Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
            "tu.example.com",
            "f63f7fcb2f0f",
            None,
        )
        .is_none()
    );

    let recovered_origin = json!({
        "id": "origin-id",
        "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
        "type": "CNAME",
        "content": "eda45cde-5a2b-4a6e-9f0f-52ca0c75254f.cfargotunnel.com",
        "proxied": true,
        "comment": "Managed by fn-knock (f63f7fcb2f0f)",
        "tags": [],
        "recoveredFromInstance": "7f531e6dd1e4",
    });
    let current_origin = json!([{
        "id": "origin-id",
        "name": "fnknock-origin-7f531e6dd1e4.tu.example.com",
        "type": "CNAME",
        "content": "eda45cde-5a2b-4a6e-9f0f-52ca0c75254f.cfargotunnel.com",
        "proxied": true,
        "comment": "Managed by fn-knock (f63f7fcb2f0f)",
        "tags": [],
    }]);
    assert!(
        recoverable_fn_knock_custom_hostname_from_snapshot(
            &custom,
            exact.as_array().unwrap(),
            current_origin.as_array().unwrap(),
            Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
            "tu.example.com",
            "f63f7fcb2f0f",
            Some(&recovered_origin),
        )
        .is_some()
    );
    let mut changed_origin = current_origin.clone();
    changed_origin[0]["content"] = json!("b8e3c226-e512-4232-a5a1-3fbdc590e880.cfargotunnel.com");
    assert!(
        recoverable_fn_knock_custom_hostname_from_snapshot(
            &custom,
            exact.as_array().unwrap(),
            changed_origin.as_array().unwrap(),
            Some("fnknock-origin-7f531e6dd1e4.tu.example.com"),
            "tu.example.com",
            "f63f7fcb2f0f",
            Some(&recovered_origin),
        )
        .is_none()
    );
}

#[test]
fn managed_custom_hostname_rejects_id_hostname_and_origin_drift() {
    let owned = json!({
        "id": "custom-id",
        "customOriginServer": "fnknock-origin-old.tu.example.com",
    });
    let remote = json!({
        "id": "custom-id",
        "hostname": "auth.tu.example.com",
        "custom_origin_server": "fnknock-origin-old.tu.example.com",
    });
    assert!(managed_custom_hostname_matches(
        &remote,
        "auth.tu.example.com",
        &owned,
        Some("fnknock-origin-current.tu.example.com"),
    ));

    for (field, value) in [
        ("id", "different-id"),
        ("hostname", "other.tu.example.com"),
        (
            "custom_origin_server",
            "fnknock-origin-other.tu.example.com",
        ),
    ] {
        let mut drifted = remote.clone();
        drifted[field] = json!(value);
        assert!(!managed_custom_hostname_matches(
            &drifted,
            "auth.tu.example.com",
            &owned,
            Some("fnknock-origin-current.tu.example.com"),
        ));
    }

    let legacy_owned = json!({ "id": "custom-id" });
    assert!(managed_custom_hostname_matches(
        &json!({
            "id": "custom-id",
            "hostname": "auth.tu.example.com",
            "custom_origin_server": "fnknock-origin-current.tu.example.com",
        }),
        "auth.tu.example.com",
        &legacy_owned,
        Some("fnknock-origin-current.tu.example.com"),
    ));
    assert!(!managed_custom_hostname_matches(
        &remote,
        "auth.tu.example.com",
        &legacy_owned,
        None,
    ));
}

#[test]
fn scan_validation_requires_both_hostname_and_certificate_readiness() {
    let pending_business_hostname = json!({
        "optimization": {
            "customHostnames": {
                "pending.example.com": {
                    "status": "pending",
                    "sslStatus": "active",
                }
            }
        }
    });
    assert_eq!(scan_validation_hostname(&pending_business_hostname), None);

    let ready_business_hostname = json!({
        "optimization": {
            "customHostnames": {
                "ready.example.com": {
                    "id": "custom-ready",
                    "status": "ready",
                    "sslStatus": "active",
                    "exactDnsId": "dns-ready",
                }
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&ready_business_hostname).as_deref(),
        Some("ready.example.com")
    );

    let pending_capability_hostname = json!({
        "optimization": {
            "capabilityProbe": {
                "hostname": "probe.example.com",
                "status": "pending",
                "hostnameStatus": "pending",
                "sslStatus": "active",
                "activationDns": { "id": "dns-probe" },
            }
        }
    });
    assert_eq!(scan_validation_hostname(&pending_capability_hostname), None);

    let ready_capability_hostname = json!({
        "optimization": {
            "capabilityProbe": {
                "hostname": "probe.example.com",
                "status": "pending",
                "hostnameStatus": "active",
                "sslStatus": "active",
                "activationDns": { "id": "dns-probe" },
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&ready_capability_hostname).as_deref(),
        Some("probe.example.com")
    );

    let capability_without_activation_dns = json!({
        "optimization": {
            "capabilityProbe": {
                "hostname": "probe.example.com",
                "status": "awaiting-candidate",
                "hostnameStatus": "active",
                "sslStatus": "active",
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&capability_without_activation_dns),
        None
    );

    let cleaned_capability_hostname = json!({
        "optimization": {
            "capabilityProbe": {
                "hostname": "deleted-probe.example.com",
                "status": "compatible",
            }
        }
    });
    assert_eq!(scan_validation_hostname(&cleaned_capability_hostname), None);

    let partially_reconciled = json!({
        "optimization": {
            "customHostnames": {
                "ready.example.com": {
                    "status": "optimized",
                    "sslStatus": "active",
                    "exactDnsId": "dns-ready",
                },
                "conflict.example.com": { "status": "conflict" },
            }
        }
    });
    assert_eq!(scan_validation_hostname(&partially_reconciled), None);
    assert_eq!(
        optimization_scan_error_code(&scan_validation_hostname_error(&partially_reconciled)),
        Some(CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE)
    );

    let failed_route_with_active_hostname = json!({
        "optimization": {
            "customHostnames": {
                "retry.example.com": {
                    "id": "custom-retry",
                    "status": "probe-failed",
                    "hostnameStatus": "active",
                    "sslStatus": "active",
                    "message": "Cloudflare edge returned HTTP 530",
                }
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&failed_route_with_active_hostname).as_deref(),
        Some("retry.example.com")
    );
    assert_eq!(
        active_probe_hostnames(&failed_route_with_active_hostname),
        vec!["retry.example.com"]
    );

    let legacy_fallback_without_exact_dns = json!({
        "optimization": {
            "customHostnames": {
                "fallback.example.com": {
                    "id": "custom-fallback",
                    "status": "fallback",
                    "sslStatus": "active",
                }
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&legacy_fallback_without_exact_dns).as_deref(),
        Some("fallback.example.com")
    );

    let conflict_with_active_hostname = json!({
        "optimization": {
            "customHostnames": {
                "conflict.example.com": {
                    "id": "custom-conflict",
                    "status": "conflict",
                    "hostnameStatus": "active",
                    "sslStatus": "active",
                }
            }
        }
    });
    assert_eq!(
        scan_validation_hostname(&conflict_with_active_hostname),
        None
    );
    assert!(active_probe_hostnames(&conflict_with_active_hostname).is_empty());
}
use super::*;
