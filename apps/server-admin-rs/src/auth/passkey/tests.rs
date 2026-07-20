use super::*;
use crate::store::LoginSession;
use axum::body::to_bytes;
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

#[tokio::test]
async fn passkey_bind_token_rejects_expired_session_and_clears_all_cookie_scopes() {
    let (_directory, state) = passkey_test_state("expired-bind-session").await;
    let session_id = "expired-passkey-bind-session";
    state
        .store
        .add_session(
            session_id,
            &passkey_test_session("2000-01-01T00:00:00Z"),
            3600,
        )
        .await
        .expect("expired passkey bind session fixture");
    let headers = passkey_bind_test_headers(session_id);

    let response = bind_token(State(state.clone()), headers).await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_passkey_session_clear_cookie_scopes(&response);
    assert!(
        state
            .store
            .get_session(session_id)
            .await
            .expect("read expired passkey bind session")
            .is_none()
    );
}

#[tokio::test]
async fn passkey_bind_token_clears_cookie_when_session_key_is_missing() {
    let (_directory, state) = passkey_test_state("missing-bind-session").await;

    let response = bind_token(
        State(state),
        passkey_bind_test_headers("missing-passkey-bind-session"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_passkey_session_clear_cookie_scopes(&response);
}

#[tokio::test]
async fn passkey_bind_status_returns_account_credentials_without_a_bind_token() {
    let (_directory, state) = passkey_test_state("bind-status").await;
    let session_id = "valid-passkey-bind-session";
    state
        .store
        .add_session(
            session_id,
            &passkey_test_session("2099-01-01T00:00:00Z"),
            3600,
        )
        .await
        .expect("passkey bind session fixture");
    state
        .store
        .add_passkey(&json!({
            "id": "current-account-passkey",
            "totpId": "totp-1"
        }))
        .await
        .expect("current account passkey fixture");
    state
        .store
        .add_passkey(&json!({
            "id": "other-account-passkey",
            "totpId": "totp-2"
        }))
        .await
        .expect("other account passkey fixture");

    let response = bind_status(State(state), passkey_bind_test_headers(session_id)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read bind status response");
    let payload: Value = serde_json::from_slice(&body).expect("parse bind status response");
    assert_eq!(
        payload.pointer("/data/credential_ids"),
        Some(&json!(["current-account-passkey"]))
    );
    assert_eq!(payload.pointer("/data/can_bind"), Some(&json!(true)));
    assert_eq!(payload.pointer("/data/token"), None);
    assert_eq!(payload.pointer("/data/bind_token"), None);
    assert_eq!(payload.pointer("/data/current_session_credential_id"), None);
}

#[tokio::test]
async fn passkey_bind_status_identifies_the_passkey_used_by_the_current_session() {
    let (_directory, state) = passkey_test_state("passkey-session-bind-status").await;
    let session_id = "current-passkey-session";
    let mut session = passkey_test_session("2099-01-01T00:00:00Z");
    session.method = AuthMethod::Passkey.as_session_str().to_string();
    session.credential_id = "current-session-passkey".to_string();
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("current passkey session fixture");

    let response = bind_status(State(state), passkey_bind_test_headers(session_id)).await;

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read current passkey session status response");
    let payload: Value =
        serde_json::from_slice(&body).expect("parse current passkey session status response");
    assert_eq!(
        payload.pointer("/data/current_session_credential_id"),
        Some(&json!("current-session-passkey"))
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
fn registration_excludes_only_passkeys_for_the_binding_account() {
    let passkeys = vec![
        json!({
            "id": URL_SAFE_NO_PAD.encode([1u8, 2, 3]),
            "totpId": "totp-1"
        }),
        json!({
            "id": URL_SAFE_NO_PAD.encode([4u8, 5, 6]),
            "totpId": "totp-2"
        }),
        json!({
            "id": "not-base64url",
            "totpId": "totp-1"
        }),
    ];

    assert_eq!(
        registration_exclude_credentials(&passkeys, "totp-1"),
        vec![vec![1, 2, 3]]
    );
}

#[test]
fn android_registration_options_target_google_password_manager() {
    let rp_info = RpInfo {
        rp_id: "auth.example.com".to_string(),
        origin: "https://auth.example.com".to_string(),
        mode: "auth_host".to_string(),
    };
    let webauthn = build_webauthn(&rp_info).expect("valid rp config");
    let (options, registration_state) =
        start_passkey_registration_for_client(&webauthn, None, true).expect("registration options");
    let state = serde_json::to_value(registration_state).expect("registration state serializes");
    let options = serde_json::to_value(options.public_key).expect("options serialize");

    assert_eq!(
        options.pointer("/authenticatorSelection/userVerification"),
        Some(&json!("required"))
    );
    assert_eq!(
        options.pointer("/authenticatorSelection/residentKey"),
        Some(&json!("required"))
    );
    assert_eq!(
        options.pointer("/authenticatorSelection/requireResidentKey"),
        Some(&json!(true))
    );
    assert_eq!(
        options.pointer("/authenticatorSelection/authenticatorAttachment"),
        Some(&json!("platform"))
    );
    assert_eq!(options.pointer("/hints/0"), Some(&json!("client-device")));
    assert_eq!(
        options.pointer("/extensions/credentialProtectionPolicy"),
        None
    );
    assert_eq!(state.pointer("/rs/policy"), Some(&json!("required")));
}

#[test]
fn standard_registration_uses_passkey_policy() {
    let rp_info = RpInfo {
        rp_id: "auth.example.com".to_string(),
        origin: "https://auth.example.com".to_string(),
        mode: "auth_host".to_string(),
    };
    let webauthn = build_webauthn(&rp_info).expect("valid rp config");
    let (options, registration_state) =
        start_passkey_registration_for_client(&webauthn, None, false)
            .expect("registration options");
    let state = serde_json::to_value(registration_state).expect("registration state serializes");
    let options = serde_json::to_value(options.public_key).expect("options serialize");

    assert_eq!(
        options.pointer("/authenticatorSelection/userVerification"),
        Some(&json!("required"))
    );
    assert_eq!(options.pointer("/hints"), None);
    assert_eq!(state.pointer("/rs/policy"), Some(&json!("required")));
}

#[test]
fn detects_android_passkey_clients_from_client_hints_or_user_agent() {
    let mut client_hints = HeaderMap::new();
    client_hints.insert(
        "sec-ch-ua-platform",
        HeaderValue::from_static("\"Android\""),
    );
    assert!(is_android_passkey_client(&client_hints));

    let mut user_agent_headers = HeaderMap::new();
    user_agent_headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Linux; Android 15; 24129PN74C)"),
    );
    assert!(is_android_passkey_client(&user_agent_headers));

    let mut desktop_headers = HeaderMap::new();
    desktop_headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_5)"),
    );
    assert!(!is_android_passkey_client(&desktop_headers));
}

#[test]
fn malformed_stored_webauthn_credential_does_not_fallback_to_legacy_fields() {
    let legacy_key = test_cose_key();
    let mut passkey = json!({
        "id": URL_SAFE_NO_PAD.encode([1u8, 2, 3, 4]),
        "publicKey": cose_key_to_base64url(&legacy_key).expect("legacy public key encodes"),
        "webauthnCredential": {
            "registration_policy": "required"
        }
    });

    assert!(stored_passkey(&passkey).is_none());

    passkey
        .as_object_mut()
        .expect("fixture is object")
        .remove("webauthnCredential");
    assert!(stored_passkey(&passkey).is_some());
}

#[test]
fn legacy_stored_credentials_authenticate_with_required_user_verification() {
    let rp_info = RpInfo {
        rp_id: "auth.example.com".to_string(),
        origin: "https://auth.example.com".to_string(),
        mode: "auth_host".to_string(),
    };
    let webauthn = build_webauthn(&rp_info).expect("valid rp config");
    let legacy = json!({
        "id": URL_SAFE_NO_PAD.encode([1u8, 2, 3, 4]),
        "publicKey": cose_key_to_base64url(&test_cose_key()).expect("legacy public key encodes"),
        "counter": 0
    });
    let passkey = stored_passkey(&legacy).expect("legacy credential converts to passkey");
    let (options, auth_state) = webauthn
        .start_passkey_authentication(&[passkey])
        .expect("authentication options");
    let options = serde_json::to_value(options.public_key).expect("options serialize");
    let state = serde_json::to_value(auth_state).expect("authentication state serializes");

    assert_eq!(
        options.pointer("/userVerification"),
        Some(&json!("required"))
    );
    assert_eq!(options.pointer("/hints"), None);
    assert_eq!(state.pointer("/ast/policy"), Some(&json!("required")));
}

fn test_cose_key() -> COSEKey {
    COSEKey {
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
    }
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

async fn passkey_test_state(name: &str) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("temporary passkey database");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = format!("passkey-{name}-test");
    let state = AppState::new(settings).await.expect("passkey test state");
    state
        .store
        .save_config(&json!({
            "run_type": 3,
            "subdomain_mode": {
                "root_domain": "example.com",
                "cookie_domain": "example.com",
                "auth_host": "auth.example.com"
            }
        }))
        .await
        .expect("passkey test config");
    (directory, state)
}

fn passkey_test_session(expires_at: &str) -> LoginSession {
    LoginSession {
        totp_id: "totp-1".to_string(),
        method: "TOTP".to_string(),
        credential_id: "totp-1".to_string(),
        credential_name: "TOTP".to_string(),
        linked_totp_name: None,
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        stream_access_expires_at: None,
        comment: None,
        ip: "203.0.113.20".to_string(),
        user_agent: "passkey-test".to_string(),
        login_time: time_utils::now_iso(),
        expires_at: Some(expires_at.to_string()),
        ip_location: None,
    }
}

fn passkey_bind_test_headers(session_id: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_str(&format!("{}={session_id}", cookies::SESSION_COOKIE_NAME))
            .expect("passkey test cookie header"),
    );
    headers.insert(
        "x-forwarded-host",
        HeaderValue::from_static("auth.example.com:7999"),
    );
    headers
}

fn assert_passkey_session_clear_cookie_scopes(response: &Response) {
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|value| value.to_str().expect("valid Set-Cookie").to_string())
        .collect::<Vec<_>>();
    assert_eq!(cookies.len(), 3);
    assert!(cookies.iter().all(|cookie| cookie.contains("Max-Age=0")));
    assert!(cookies.iter().any(|cookie| !cookie.contains("Domain=")));
    assert!(
        cookies
            .iter()
            .any(|cookie| cookie.contains("Domain=example.com"))
    );
    assert!(
        cookies
            .iter()
            .any(|cookie| cookie.contains("Domain=auth.example.com"))
    );
}
