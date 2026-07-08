use serde_json::json;

use super::*;

#[test]
fn safe_redirect_allows_relative_current_origin_and_configured_hosts() {
    let config = json!({
        "subdomain_mode": {
            "root_domain": "example.com",
            "public_auth_base_url": "https://login.example.net",
            "public_https_port": 443
        },
        "host_mappings": [
            { "host": "mapped.example.net" }
        ]
    });
    let headers = forwarded_headers("app.example.com");
    assert_eq!(
        safe_redirect(&config, &headers, Some("/app")).as_deref(),
        Some("/app")
    );
    assert_eq!(
        safe_redirect(&config, &headers, Some("https://app.example.com/app")).as_deref(),
        Some("https://app.example.com/app")
    );
    assert_eq!(
        safe_redirect(&config, &headers, Some("https://tools.example.com/app")).as_deref(),
        Some("https://tools.example.com/app")
    );
    assert_eq!(
        safe_redirect(&config, &headers, Some("https://mapped.example.net/app")).as_deref(),
        Some("https://mapped.example.net/app")
    );
    assert_eq!(
        safe_redirect(&config, &headers, Some("https://login.example.net/app")).as_deref(),
        Some("https://login.example.net/app")
    );
}

#[test]
fn safe_redirect_matches_node_scheme_relative_and_unknown_scheme_rules() {
    let config = json!({});
    let headers = forwarded_headers("app.example.com");
    assert_eq!(
        safe_redirect(&config, &headers, Some("//example.com")).as_deref(),
        Some("//example.com")
    );
    assert!(safe_redirect(&config, &headers, Some("javascript:alert(1)")).is_none());
    assert!(safe_redirect(&config, &headers, Some("https://evil.example/app")).is_none());
}

#[test]
fn browser_session_redirect_must_be_reachable_by_cookie_scope() {
    let config = json!({
        "host_mappings": [
            { "host": "app.example.net" }
        ]
    });
    let headers = forwarded_headers("auth.example.net");
    assert_eq!(
        safe_redirect(&config, &headers, Some("https://app.example.net/app")).as_deref(),
        Some("https://app.example.net/app")
    );
    assert!(
        effective_login_redirect(
            &config,
            &headers,
            "browser_session",
            Some("https://app.example.net/app")
        )
        .is_none()
    );

    let config = json!({
        "run_type": 3,
        "subdomain_mode": { "root_domain": "example.net" }
    });
    assert_eq!(
        effective_login_redirect(
            &config,
            &headers,
            "browser_session",
            Some("https://app.example.net/app")
        )
        .as_deref(),
        Some("https://app.example.net/app")
    );
}

#[test]
fn shared_auth_redirect_targets_public_auth_origin() {
    let config = json!({
        "run_type": 3,
        "subdomain_mode": {
            "root_domain": "example.com",
            "auth_host": "auth.example.com",
            "public_https_port": 443
        }
    });
    let headers = forwarded_headers("app.example.com");
    let redirect = resolve_shared_auth_login_redirect(
        &config,
        &headers,
        Some("https://app.example.com/dashboard"),
    )
    .unwrap();
    assert!(redirect.starts_with("https://auth.example.com/?redirect_uri="));
    assert!(redirect.ends_with("#/login"));
    assert!(redirect.contains("https%3A%2F%2Fapp.example.com%2Fdashboard"));
}

#[test]
fn public_auth_base_url_applies_configured_public_https_port() {
    let config = json!({
        "run_type": 3,
        "subdomain_mode": {
            "root_domain": "example.com",
            "auth_host": "auth.example.com",
            "public_https_port": 8443
        }
    });
    let headers = forwarded_headers("app.example.com");
    let redirect =
        resolve_shared_auth_login_redirect(&config, &headers, Some("/dashboard")).unwrap();
    assert!(redirect.starts_with("https://auth.example.com:8443/?redirect_uri="));
    assert!(redirect.contains("%2Fdashboard"));
}

#[test]
fn forwarded_header_parsing_matches_node_fallbacks() {
    let config = json!({});
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("app.example.com"));
    assert_eq!(
        safe_redirect(&config, &headers, Some("http://app.example.com/app")).as_deref(),
        Some("http://app.example.com/app")
    );
    assert!(safe_redirect(&config, &headers, Some("https://app.example.com/app")).is_none());

    headers.insert(
        "forwarded",
        HeaderValue::from_static("for=192.0.2.1; bad; proto=https; host=auth.example.com"),
    );
    assert_eq!(resolve_forwarded_proto(&headers), "https");
    assert_eq!(
        resolve_forwarded_host(&headers).as_deref(),
        Some("auth.example.com")
    );
}

#[test]
fn resolve_cookie_domain_matches_subdomain_mode_scope() {
    let config = json!({
        "run_type": 3,
        "subdomain_mode": { "root_domain": "example.com" }
    });
    let headers = forwarded_headers("auth.example.com");
    assert_eq!(
        resolve_cookie_domain(&config, &headers).as_deref(),
        Some("example.com")
    );
}

#[test]
fn post_logout_location_matches_node_prefix_resolution() {
    let headers = HeaderMap::new();
    assert_eq!(
        post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
        "/login?logged_out=1"
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-path",
        HeaderValue::from_static("/__auth__/api/auth/logout"),
    );
    assert_eq!(
        post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
        "/__auth__/login?logged_out=1"
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::REFERER,
        HeaderValue::from_static("https://example.com/auth/settings"),
    );
    assert_eq!(
        post_logout_location(&headers, &Uri::from_static("/api/auth/logout")),
        "/auth/login?logged_out=1"
    );
}

fn forwarded_headers(host: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-proto", HeaderValue::from_static("https"));
    headers.insert("x-forwarded-host", HeaderValue::from_str(host).unwrap());
    headers
}

#[test]
fn strict_whitelist_access_mode_matches_node_header_parsing() {
    let mut headers = HeaderMap::new();
    assert!(!is_strict_whitelist_request(&headers));

    headers.insert(
        "X-Reauth-Access-Mode",
        HeaderValue::from_static(" strict_whitelist "),
    );
    assert!(is_strict_whitelist_request(&headers));

    headers.insert(
        "X-Reauth-Access-Mode",
        HeaderValue::from_static("fnos-share"),
    );
    assert!(!is_strict_whitelist_request(&headers));
}

#[test]
fn client_ip_for_auth_matches_node_header_extraction() {
    let headers = HeaderMap::new();
    assert_eq!(client_ip_for_auth(&headers), "");

    let mut headers = HeaderMap::new();
    headers.insert("x-real-ip", HeaderValue::from_static("198.51.100.20"));
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("203.0.113.10, 198.51.100.20"),
    );
    assert_eq!(client_ip_for_auth(&headers), "203.0.113.10");

    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_static("[::1]:443"));
    assert_eq!(client_ip_for_auth(&headers), "127.0.0.1");
}

#[test]
fn inspect_auth_mobility_request_matches_node_cookie_rules() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_str(
            "x-go-reauth-proxy-session-id=old; fnos-token=old%201; mode=relay; \
                 x-go-reauth-proxy-session-id=\"session%202\"; fnos-token=token%202",
        )
        .unwrap(),
    );
    headers.insert("x-forwarded-path", HeaderValue::from_static("trimcon?x=1"));
    headers.insert(header::USER_AGENT, HeaderValue::from_static("Dart:io"));

    let identity = inspect_auth_mobility_request(&headers);

    assert_eq!(identity.session_id.as_deref(), Some("session 2"));
    assert_eq!(identity.fnos_token.as_deref(), Some("token 2"));
    assert_eq!(identity.app_binding, Some("fnos-app"));
    assert_eq!(identity.trim_media_token, None);
}

#[test]
fn inspect_auth_mobility_request_extracts_trim_media_token() {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        HeaderValue::from_static("com.trim.media"),
    );
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_static("Bearer media-token"),
    );

    let identity = inspect_auth_mobility_request(&headers);

    assert_eq!(identity.app_binding, Some("trim-media-app"));
    assert_eq!(identity.trim_media_token.as_deref(), Some("media-token"));
    assert_eq!(identity.fnos_token, None);
}

#[test]
fn auth_mobility_resolvable_access_requires_live_owner_session_like_node() {
    let mut session = LoginSession {
        totp_id: "totp-1".to_string(),
        method: "TOTP".to_string(),
        credential_id: "totp-1".to_string(),
        credential_name: "TOTP".to_string(),
        linked_totp_name: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        comment: None,
        ip: "203.0.113.10".to_string(),
        user_agent: "ua".to_string(),
        login_time: "2026-01-01T00:00:00Z".to_string(),
        expires_at: Some("2999-01-01T00:00:00Z".to_string()),
        ip_location: None,
    };

    assert!(auth_mobility_session_has_remaining_ttl(&session));

    session.expires_at = Some("2000-01-01T00:00:00Z".to_string());
    assert!(!auth_mobility_session_has_remaining_ttl(&session));

    session.expires_at = Some("not-a-date".to_string());
    assert!(!auth_mobility_session_has_remaining_ttl(&session));

    session.expires_at = None;
    assert!(!auth_mobility_session_has_remaining_ttl(&session));
}

#[test]
fn whitelist_target_matching_supports_ip_and_cidr_targets() {
    assert!(whitelist_target_matches_ip(
        "192.0.2.10",
        "ip",
        "192.0.2.10".parse().unwrap()
    ));
    assert!(whitelist_target_matches_ip(
        "[2001:db8::10]",
        "ip",
        "2001:db8::10".parse().unwrap()
    ));
    assert!(whitelist_target_matches_ip(
        "2001:db8::/32",
        "cidr",
        "2001:db8:1::1".parse().unwrap()
    ));
    assert!(!whitelist_target_matches_ip(
        "2001:db8::/32",
        "cidr",
        "2001:db9::1".parse().unwrap()
    ));
}

#[test]
fn parses_pow_expiry_from_salt() {
    assert_eq!(parse_pow_expires("abc?expires=123"), Some(123));
    assert_eq!(parse_pow_expires("abc?x=1&expires=456"), Some(456));
    assert_eq!(parse_pow_expires("abc?x&expires=789"), Some(789));
    assert_eq!(parse_pow_expires("abc"), None);
}

#[test]
fn pow_challenge_generation_uses_node_exclusive_max_number() {
    assert_eq!(pow_secret_number_from_random(0), 0);
    assert_eq!(pow_secret_number_from_random(POW_MAX_NUMBER), 0);
    assert!(pow_secret_number_from_random(u32::MAX) < POW_MAX_NUMBER);
}

#[test]
fn pow_number_text_matches_node_number_only_rule() {
    assert_eq!(pow_number_text(Some(&json!(42))), "42");
    assert_eq!(pow_number_text(Some(&json!(42.0))), "42");
    assert_eq!(pow_number_text(Some(&json!(42.5))), "42.5");
    assert_eq!(pow_number_text(Some(&json!("42"))), "");
    assert_eq!(pow_number_text(None), "");
}

#[test]
fn pow_validation_uses_original_challenge_for_signature_and_nonce_like_node() {
    let translator = Translator::new("en");
    let key = "secret";
    let salt = "abc?expires=9999999999";
    let number = 7;
    let challenge = sha256_hex(format!("{salt}{number}").as_bytes()).to_ascii_uppercase();
    let signature = hmac_sha256_hex(key.as_bytes(), challenge.as_bytes());

    let validation = validate_pow_proof(
        PowProof {
            algorithm: Some("SHA-256".to_string()),
            challenge: Some(challenge.clone()),
            number: Some(json!(number)),
            salt: Some(salt.to_string()),
            signature: Some(signature),
        },
        key,
        1,
        &translator,
    )
    .unwrap();
    assert_eq!(validation.nonce, challenge);

    let rejected = validate_pow_proof(
        PowProof {
            algorithm: Some("SHA-256".to_string()),
            challenge: Some(challenge.clone()),
            number: Some(json!(number)),
            salt: Some(salt.to_string()),
            signature: Some(hmac_sha256_hex(
                key.as_bytes(),
                challenge.to_ascii_lowercase().as_bytes(),
            )),
        },
        key,
        1,
        &translator,
    );
    assert!(rejected.is_err());
}

#[test]
fn turnstile_error_reason_matches_node_error_codes_join() {
    assert_eq!(
        turnstile_error_reason(&json!({ "error-codes": ["a", "", "b"] })),
        Some("a, b".to_string())
    );
    assert_eq!(turnstile_error_reason(&json!({ "error-codes": [] })), None);
    assert_eq!(turnstile_error_reason(&json!({})), None);
}

#[test]
fn totp_verification_does_not_trim_token_like_node_otplib() {
    let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
    let bytes = Secret::Encoded(secret.to_string()).to_bytes().unwrap();
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes).unwrap();
    let token = totp.generate_current().unwrap();

    assert!(verify_totp_token(secret, &token).unwrap());
    assert!(!verify_totp_token(secret, &format!(" {token}")).unwrap_or(false));
    assert!(!verify_totp_token(secret, &format!("{token} ")).unwrap_or(false));
}

#[test]
fn verify_denied_status_matches_node_scope_boundary() {
    let scoped = AuthAccess {
        authenticated: false,
        message: "Access denied by credential scope".to_string(),
        grant_type: None,
        deny_reason: Some(REAUTH_SCOPE_DENIED.to_string()),
        set_cookies: Vec::new(),
        response_headers: Vec::new(),
    };
    assert_eq!(auth_verify_denied_status(&scoped), StatusCode::FORBIDDEN);

    let ordinary = AuthAccess {
        authenticated: false,
        message: "Unauthorized".to_string(),
        grant_type: None,
        deny_reason: None,
        set_cookies: Vec::new(),
        response_headers: Vec::new(),
    };
    assert_eq!(
        auth_verify_denied_status(&ordinary),
        StatusCode::UNAUTHORIZED
    );
}

#[test]
fn logout_custom_post_login_grant_revoke_predicate_matches_node() {
    let mut session = LoginSession {
        totp_id: "totp-1".to_string(),
        method: "TOTP".to_string(),
        credential_id: "totp-1".to_string(),
        credential_name: "TOTP".to_string(),
        linked_totp_name: None,
        grant_type: Some("login_ip_grant".to_string()),
        post_login_ip_grant_mode: Some("custom".to_string()),
        post_login_ip_grant_record_id: None,
        comment: None,
        ip: "203.0.113.10".to_string(),
        user_agent: "ua".to_string(),
        login_time: "2026-01-01T00:00:00Z".to_string(),
        expires_at: Some("2026-01-02T00:00:00Z".to_string()),
        ip_location: None,
    };
    assert!(should_revoke_custom_post_login_ip_grant(
        Some(&session),
        &json!({})
    ));

    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    session.comment = Some("Automatically authorized after sign-in".to_string());
    assert!(should_revoke_custom_post_login_ip_grant(
        Some(&session),
        &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "custom"}})
    ));
    assert!(!should_revoke_custom_post_login_ip_grant(
        Some(&session),
        &json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}})
    ));
}

#[test]
fn localizes_auth_route_text() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        auth_route_text(&translator, "authenticationRequired"),
        "需要先完成认证"
    );
    let credential = TotpCredential {
        id: "totp-1".to_string(),
        secret: "secret".to_string(),
        comment: "".to_string(),
        created_at: String::new(),
        access_scopes: Value::Null,
        subdomain_access: Value::Null,
    };
    assert_eq!(credential_name(&credential, &translator), "未知 TOTP");
}
