use super::*;
use axum::http::HeaderMap;

#[test]
fn extracts_challenge_from_client_data_json() {
    let client_data = URL_SAFE_NO_PAD.encode(br#"{"challenge":"abc123"}"#);
    let value = json!({ "response": { "clientDataJSON": client_data } });
    assert_eq!(extract_challenge(&value).as_deref(), Some("abc123"));
}

#[test]
fn extracts_backup_flags_from_authenticator_data() {
    let value = json!({
        "response": {
            "authenticatorData": "YHmfwk2i1yU-o4eqPt1XZz6jzv9gGcnnYy-M1BqdLQQdAAAAAA"
        }
    });
    assert_eq!(
        authenticator_backup_flags(&value),
        Some(AuthenticatorBackupFlags {
            backup_eligible: true,
            backup_state: true
        })
    );
}

#[test]
fn patches_serialized_auth_state_backup_flags() {
    let mut state = json!({
        "state": {
            "ast": {
                "credentials": [
                    {
                        "cred_id": "abc123",
                        "backup_eligible": false,
                        "backup_state": false
                    }
                ]
            }
        }
    });
    patch_authentication_state_backup_flags(
        &mut state,
        "abc123",
        AuthenticatorBackupFlags {
            backup_eligible: true,
            backup_state: true,
        },
    );
    assert_eq!(
        state.pointer("/state/ast/credentials/0/backup_eligible"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        state.pointer("/state/ast/credentials/0/backup_state"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn resolves_parent_domain_rp() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("auth.example.com"));
    let config = json!({
        "subdomain_mode": {
            "passkey_rp_mode": "parent_domain",
            "root_domain": "example.com"
        }
    });
    let info = rp_info_with_configured_host(&config, &headers, None);
    assert_eq!(info.rp_id, "example.com");
    assert_eq!(info.mode, "parent_domain");
}

#[test]
fn rp_info_prefers_origin_over_host_like_node() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("127.0.0.1:7997"));
    headers.insert(
        "origin",
        HeaderValue::from_static("https://auth.example.com/login"),
    );
    let config = json!({});
    let info = rp_info_with_configured_host(&config, &headers, None);
    assert_eq!(info.rp_id, "auth.example.com");
    assert_eq!(info.origin, "https://auth.example.com");
    assert_eq!(info.mode, "auth_host");
}

#[test]
fn rp_info_uses_public_auth_base_when_request_hosts_are_loopback() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("127.0.0.1:7997"));
    let config = json!({
        "subdomain_mode": {
            "public_auth_base_url": "https://auth.example.com",
            "public_https_port": 443
        }
    });
    let info = rp_info_with_configured_host(&config, &headers, None);
    assert_eq!(info.rp_id, "auth.example.com");
    assert_eq!(info.origin, "https://auth.example.com");
}

#[test]
fn normalizes_request_host_with_port_like_node() {
    assert_eq!(normalize_host("auth.r.wxlnk.com:7999"), "auth.r.wxlnk.com");
    assert_eq!(
        normalize_host("https://auth.r.wxlnk.com:7999/login"),
        "auth.r.wxlnk.com"
    );
    assert_eq!(normalize_host("[::1]:7999"), "[::1]");
}

#[test]
fn request_hostname_strips_host_header_port_like_node() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("auth.r.wxlnk.com:7999"));
    assert_eq!(request_hostname(&headers), "auth.r.wxlnk.com");
}

#[test]
fn passkey_bind_cookie_parser_matches_node_last_value_rules() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static(
            "flag; x-go-reauth-proxy-session-id=old; x-go-reauth-proxy-session-id=\"new%201\"",
        ),
    );
    assert_eq!(
        parse_passkey_cookie_value(&headers, cookies::SESSION_COOKIE_NAME).as_deref(),
        Some("new 1")
    );
}

#[test]
fn passkey_device_name_matches_node_fallback_without_trimming() {
    assert_eq!(passkey_device_name(String::new()), "Unknown Device");
    assert_eq!(
        passkey_device_name("  Desk Key  ".to_string()),
        "  Desk Key  "
    );
}

#[test]
fn registration_options_require_uv_when_credprotect_requires_uv() {
    let rp_info = RpInfo {
        rp_id: "auth.example.com".to_string(),
        origin: "https://auth.example.com".to_string(),
        mode: "auth_host".to_string(),
    };
    let webauthn = build_webauthn(&rp_info).expect("valid rp config");
    let (mut options, registration_state) = webauthn
        .start_securitykey_registration(PASSKEY_ADMIN_UUID, "admin", "admin", None, None, None)
        .expect("registration options");

    let state = require_registration_user_verification(&mut options, registration_state)
        .expect("registration state serializes");
    let options = serde_json::to_value(options.public_key).expect("options serialize");

    assert_eq!(
        options.pointer("/authenticatorSelection/userVerification"),
        Some(&json!("required"))
    );
    assert_eq!(
        options.pointer("/extensions/credentialProtectionPolicy"),
        Some(&json!("userVerificationRequired"))
    );
    assert_eq!(state.pointer("/rs/policy"), Some(&json!("required")));
}

#[test]
fn malformed_stored_webauthn_credential_does_not_fallback_to_legacy_fields() {
    let legacy_key = COSEKey {
        type_: COSEAlgorithm::ES256,
        key: COSEKeyType::EC_EC2(COSEEC2Key {
            curve: ECDSACurve::SECP256R1,
            x: vec![
                194, 126, 127, 109, 252, 23, 131, 21, 252, 6, 223, 99, 44, 254, 140, 27, 230, 17,
                94, 5, 133, 28, 104, 41, 144, 69, 171, 149, 161, 26, 200, 243,
            ],
            y: vec![
                143, 123, 183, 156, 24, 178, 21, 248, 117, 159, 162, 69, 171, 52, 188, 252, 26, 59,
                6, 47, 103, 92, 19, 58, 117, 103, 249, 0, 219, 8, 95, 196,
            ],
        }),
    };
    let mut passkey = json!({
        "id": URL_SAFE_NO_PAD.encode([1u8, 2, 3, 4]),
        "publicKey": cose_key_to_base64url(&legacy_key).expect("legacy public key encodes"),
        "webauthnCredential": {
            "registration_policy": "required"
        }
    });

    assert!(passkey_to_security_key(&passkey).is_none());

    passkey
        .as_object_mut()
        .expect("fixture is object")
        .remove("webauthnCredential");
    assert!(passkey_to_security_key(&passkey).is_some());
}

#[test]
fn rejects_unrelated_cookie_domain() {
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("auth.example.com"));
    let mut config = json!({
        "subdomain_mode": {
            "cookie_domain": "other.example.net",
            "root_domain": "example.com"
        }
    });
    assert_eq!(resolve_cookie_domain(&config, &headers).as_deref(), None);
    config["run_type"] = json!(3);
    assert_eq!(
        resolve_cookie_domain(&config, &headers).as_deref(),
        Some("example.com")
    );
}

#[test]
fn localizes_passkey_route_text() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        passkey_text(&translator, "invalidResponse"),
        "Passkey 响应无效"
    );
    assert_eq!(
        passkey_text_params(
            &translator,
            "notFoundWithRetry",
            &[("seconds", "30".to_string())]
        ),
        "未找到 Passkey，请在 30 秒后重试"
    );
}
