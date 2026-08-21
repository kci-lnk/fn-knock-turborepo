#[test]
fn extracts_txt_dcv_records_from_both_cloudflare_shapes() {
    let value = json!({
        "ownership_verification": { "type": "txt", "name": "_cf.example.com", "value": "owner" },
        "ssl": { "validation_records": [
            { "status": "pending", "txt_name": "_acme.example.com", "txt_record": "ssl" }
        ] }
    });
    assert_eq!(
        extract_validation_records(&value),
        vec![
            ("_acme.example.com".to_string(), "ssl".to_string()),
            ("_cf.example.com".to_string(), "owner".to_string()),
        ]
    );
}

#[test]
fn weekly_jitter_is_bounded_to_six_hours() {
    for _ in 0..32 {
        assert!((0..6 * 60 * 60 * 1000).contains(&weekly_jitter_ms()));
    }
}

#[test]
fn capability_errors_only_disable_known_unsupported_plans() {
    let unsupported = CloudflareApiError {
        status: Some(StatusCode::FORBIDDEN),
        message: "This feature is not available on your plan".to_string(),
    };
    assert!(is_capability_unsupported_api_error(&unsupported));

    let missing_quota = CloudflareApiError {
        status: Some(StatusCode::BAD_REQUEST),
        message: "No quota has been allocated for this zone or for this account. (1404)"
            .to_string(),
    };
    assert!(is_capability_unsupported_api_error(&missing_quota));

    for error in [
        CloudflareApiError {
            status: Some(StatusCode::TOO_MANY_REQUESTS),
            message: "rate limited".to_string(),
        },
        CloudflareApiError {
            status: Some(StatusCode::FORBIDDEN),
            message: "permission denied".to_string(),
        },
        CloudflareApiError {
            status: Some(StatusCode::CONFLICT),
            message: "hostname already exists".to_string(),
        },
    ] {
        assert!(!is_capability_unsupported_api_error(&error));
    }
}

#[test]
fn scan_errors_distinguish_saas_setup_validation_and_readiness() {
    let saas_required = scan_validation_hostname_error(&json!({
        "optimization": {
            "capabilityProbe": {
                "status": "unsupported",
                "reasonCode": CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE,
            }
        }
    }));
    assert_eq!(
        optimization_scan_error_code(&saas_required),
        Some(CLOUDFLARE_SAAS_REQUIRED_ERROR_CODE)
    );

    let validation_pending = scan_validation_hostname_error(&json!({
        "optimization": {
            "capabilityProbe": {
                "status": "pending",
                "hostnameStatus": "active",
                "sslStatus": "pending_validation",
            }
        }
    }));
    assert_eq!(
        optimization_scan_error_code(&validation_pending),
        Some(CLOUDFLARE_SAAS_VALIDATION_PENDING_ERROR_CODE)
    );

    let ownership_conflict = scan_validation_hostname_error(&json!({
        "optimization": {
            "capabilityProbe": { "status": "compatible" },
            "customHostnames": {
                "auth.example.com": { "status": "conflict" }
            }
        }
    }));
    assert_eq!(
        optimization_scan_error_code(&ownership_conflict),
        Some(CLOUDFLARE_RESOURCE_CONFLICT_ERROR_CODE)
    );

    let compatible_probe_without_live_hostname = scan_validation_hostname_error(&json!({
        "optimization": {
            "capabilityProbe": { "status": "compatible" }
        }
    }));
    assert_eq!(
        optimization_scan_error_code(&compatible_probe_without_live_hostname),
        Some(OPTIMIZATION_NOT_READY_ERROR_CODE)
    );

    let not_ready = scan_validation_hostname_error(&json!({}));
    assert_eq!(
        optimization_scan_error_code(&not_ready),
        Some(OPTIMIZATION_NOT_READY_ERROR_CODE)
    );

    let resolution_unavailable = local_error(CANDIDATE_RESOLUTION_UNAVAILABLE_SCAN_ERROR);
    assert_eq!(
        optimization_scan_error_code(&resolution_unavailable),
        Some(CANDIDATE_RESOLUTION_UNAVAILABLE_ERROR_CODE)
    );

    let unrelated = local_error("latency probe failed");
    assert_eq!(optimization_scan_error_code(&unrelated), None);
}
use super::*;
