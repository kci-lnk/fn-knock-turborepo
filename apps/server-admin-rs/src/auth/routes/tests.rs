use serde_json::json;

use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

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
fn safe_redirect_rejects_network_path_and_unsafe_url_variants() {
    let config = json!({});
    let headers = forwarded_headers("app.example.com");
    for unsafe_redirect in [
        "//example.com",
        "///example.com",
        r"\\example.com",
        r"/\example.com",
        r"\/example.com",
        r"https:\example.com",
        "/\t/example.com",
        "/\n/example.com",
        "/\r/example.com",
    ] {
        assert!(
            safe_redirect(&config, &headers, Some(unsafe_redirect)).is_none(),
            "network-path redirect must be rejected: {unsafe_redirect}"
        );
    }
    assert_eq!(
        safe_redirect(&config, &headers, Some("/example.com/app")).as_deref(),
        Some("/example.com/app")
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
            "cookie_domain": "example.com",
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
fn shared_auth_redirect_does_not_redirect_to_same_hostname_with_mismatched_origin() {
    let config = shared_auth_test_config();
    let mut headers = forwarded_headers("auth.example.com:7999");
    headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));

    assert!(
        resolve_shared_auth_login_redirect(
            &config,
            &headers,
            Some("https://app.example.com/dashboard")
        )
        .is_none()
    );
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
fn cookie_clear_domains_cover_host_only_shared_and_current_host_scopes() {
    let config = shared_auth_test_config();
    let headers = forwarded_headers("auth.example.com:7999");
    assert_eq!(
        resolve_cookie_clear_domains(Some(&config), &headers),
        vec![
            None,
            Some("example.com".to_string()),
            Some("auth.example.com".to_string())
        ]
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

#[tokio::test]
async fn authorized_normal_access_bypasses_scanner_blacklist() {
    let (_directory, state) = auth_route_test_state("trusted-scanner-bypass").await;
    let client_ip = "203.0.113.77";
    let now = time_utils::now_ms();
    state
        .store
        .save_scanner_settings(&json!({
            "enabled": true,
            "windowMinutes": 10,
            "threshold": 3,
            "blacklistTtlSeconds": 3600
        }))
        .await
        .expect("enable scanner");
    state
        .store
        .add_scanner_blacklist_record(
            client_ip,
            &json!({"ip": client_ip, "source": "test"}),
            now,
            3600,
        )
        .await
        .expect("blacklist client IP");
    state
        .store
        .insert_whitelist_record(&crate::store::WhitelistRecord {
            id: "manual-scanner-bypass".to_string(),
            ip: client_ip.to_string(),
            target_type: "ip".to_string(),
            expire_at: Some(now.div_euclid(1_000) + 3_600),
            source: "manual".to_string(),
            created_at: now.div_euclid(1_000),
            status: "active".to_string(),
            comment: None,
            ip_location: None,
            resolved_targets: None,
            check_interval_minutes: None,
            last_checked_at: None,
            last_resolved_at: None,
            resolve_status: None,
            resolve_message: None,
        })
        .await
        .expect("store manual whitelist record");
    crate::whitelist::rebuild_whitelist_ipset_snapshots(&state)
        .await
        .expect("publish whitelist snapshot");

    let config = json!({"run_type": 1});
    let mut headers = forwarded_headers("app.example.com");
    headers.insert(
        "x-forwarded-path",
        HeaderValue::from_static("/definitely-uncommon-scanner-probe"),
    );
    let normal_access = resolve_preflight_normal_access(
        &state,
        &headers,
        &Uri::from_static("/"),
        &config,
        client_ip,
        RequestedAccessMode::LoginFirst,
    )
    .await
    .expect("resolve authoritative manual whitelist access");
    assert!(normal_access.authorized);
    assert_eq!(
        normal_access.grant_type.as_deref(),
        Some("manual_whitelist")
    );

    let mut response = Response::new(Body::empty());
    apply_preflight_behavior_with_normal_access(
        &state,
        &headers,
        &Uri::from_static("/"),
        &mut response,
        &config,
        client_ip,
        RequestedAccessMode::LoginFirst,
        &normal_access,
        None,
        None,
        None,
    )
    .await
    .expect("apply authorized preflight");

    assert!(response.headers().get("X-Option").is_none());
    assert!(
        state
            .store
            .scanner_suspicious_hits_since(client_ip, 0)
            .await
            .expect("read suspicious path hits")
            .is_empty(),
        "authoritatively whitelisted traffic must not increment scanner path counters"
    );
}

#[tokio::test]
async fn unauthorized_normal_access_still_honors_scanner_blacklist() {
    let (_directory, state) = auth_route_test_state("ordinary-scanner-block").await;
    let client_ip = "203.0.113.78";
    let now = time_utils::now_ms();
    state
        .store
        .save_scanner_settings(&json!({
            "enabled": true,
            "windowMinutes": 10,
            "threshold": 3,
            "blacklistTtlSeconds": 3600
        }))
        .await
        .expect("enable scanner");
    state
        .store
        .add_scanner_blacklist_record(
            client_ip,
            &json!({"ip": client_ip, "source": "test"}),
            now,
            3600,
        )
        .await
        .expect("blacklist client IP");

    let config = json!({"run_type": 1});
    let mut response = Response::new(Body::empty());
    apply_preflight_behavior_with_normal_access(
        &state,
        &forwarded_headers("app.example.com"),
        &Uri::from_static("/"),
        &mut response,
        &config,
        client_ip,
        RequestedAccessMode::LoginFirst,
        &PreflightNormalAccess::default(),
        None,
        None,
        None,
    )
    .await
    .expect("apply unauthorized preflight");

    assert_eq!(
        response
            .headers()
            .get("X-Option")
            .and_then(|value| value.to_str().ok()),
        Some("Deny")
    );
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
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("browser_session".to_string()),
        post_login_ip_grant_mode: None,
        post_login_ip_grant_record_id: None,
        stream_access_expires_at: None,
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
fn presented_session_expiry_preserves_unparseable_legacy_values() {
    let mut session = auth_route_test_session("203.0.113.10", "2999-01-01T00:00:00Z");
    assert!(!login_session_has_expired(&session));

    session.expires_at = Some("2000-01-01T00:00:00Z".to_string());
    assert!(login_session_has_expired(&session));

    session.expires_at = Some("legacy-value".to_string());
    assert!(!login_session_has_expired(&session));

    session.expires_at = None;
    assert!(!login_session_has_expired(&session));
}

#[tokio::test]
async fn automatic_ip_grant_respects_owner_subdomain_scope() {
    let (_directory, state) = auth_route_test_state("auto-ip-subdomain-scope").await;
    let client_ip = "203.0.113.30";
    let config = json!({
        "run_type": 3,
        "host_mappings": [
            {"host": "allowed.example.com", "use_auth": true},
            {"host": "denied.example.com", "use_auth": true}
        ]
    });
    state
        .store
        .add_totp(TotpCredential {
            id: "scoped-totp".to_string(),
            secret: "secret".to_string(),
            comment: "Scoped credential".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: Value::Null,
            subdomain_access: json!({
                "mode": "custom",
                "hosts": ["allowed.example.com"]
            }),
        })
        .await
        .expect("store scoped credential");

    let mut session = auth_route_test_session(client_ip, &time_utils::iso_after_seconds(3_600));
    session.totp_id = "scoped-totp".to_string();
    session.credential_id = "scoped-totp".to_string();
    session.grant_type = Some("login_ip_grant".to_string());
    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    state
        .store
        .add_session("scoped-session", &session, 3_600)
        .await
        .expect("store scoped session");
    state
        .store
        .insert_whitelist_record(&crate::store::WhitelistRecord {
            id: "scoped-auto-whitelist".to_string(),
            ip: client_ip.to_string(),
            target_type: "ip".to_string(),
            expire_at: Some(time_utils::now_ms().div_euclid(1_000) + 3_600),
            source: "auto".to_string(),
            created_at: time_utils::now_ms().div_euclid(1_000),
            status: "active".to_string(),
            comment: None,
            ip_location: None,
            resolved_targets: None,
            check_interval_minutes: None,
            last_checked_at: None,
            last_resolved_at: None,
            resolve_status: None,
            resolve_message: None,
        })
        .await
        .expect("store automatic whitelist");
    crate::whitelist::rebuild_whitelist_ipset_snapshots(&state)
        .await
        .expect("publish automatic whitelist snapshot");

    let allowed = resolve_preflight_normal_access(
        &state,
        &forwarded_headers("allowed.example.com"),
        &Uri::from_static("/"),
        &config,
        client_ip,
        RequestedAccessMode::LoginFirst,
    )
    .await
    .expect("resolve allowed host");
    assert!(allowed.authorized);
    assert_eq!(allowed.grant_type.as_deref(), Some("login_ip_grant"));

    let denied = resolve_preflight_normal_access(
        &state,
        &forwarded_headers("denied.example.com"),
        &Uri::from_static("/"),
        &config,
        client_ip,
        RequestedAccessMode::LoginFirst,
    )
    .await
    .expect("resolve denied host");
    assert!(!denied.authorized);
    assert_eq!(denied.deny_reason.as_deref(), Some(REAUTH_SCOPE_DENIED));
}

#[tokio::test]
async fn matched_rule_takes_precedence_over_automatic_ip_grant() {
    let (_directory, state) = auth_route_test_state("rule-before-auto-ip").await;
    let config = json!({
        "host_mappings": [{
            "host": "allowed.example.com",
            "use_auth": true,
            "advanced_auth": {
                "enabled": true,
                "idle_ttl_seconds": 86_400,
                "max_lifetime_seconds": 2_592_000,
                "policy_version": "policy-v1",
                "groups": [{"id": "group-v1", "conditions": []}]
            }
        }]
    });
    let normal_access = PreflightNormalAccess {
        authorized: true,
        grant_type: Some("login_ip_grant".to_string()),
        ..Default::default()
    };
    let matched = crate::grpc_proto::SubdomainRuleMatch {
        host: "allowed.example.com".to_string(),
        policy_version: "policy-v1".to_string(),
        group_id: "group-v1".to_string(),
    };

    let access = resolve_auth_access_with_normal_access_and_rule_match(
        &state,
        &forwarded_headers("allowed.example.com"),
        &Uri::from_static("/"),
        &Translator::new(crate::i18n::DEFAULT_LOCALE),
        &config,
        "203.0.113.31",
        &normal_access,
        Some(&matched),
        None,
        None,
        None,
    )
    .await
    .expect("resolve matched rule");

    assert!(access.authenticated);
    assert_eq!(access.grant_type.as_deref(), Some("subdomain_rule"));
    assert_eq!(access.set_cookies.len(), 1);
    assert!(access.set_cookies[0].starts_with(&format!(
        "{}=p1.",
        cookies::SUBDOMAIN_RULE_GRANT_COOKIE_NAME
    )));
    assert_eq!(
        access
            .response_headers
            .iter()
            .find(|(name, _)| name == "X-Reauth-Auth-Grant-State")
            .map(|(_, value)| value.as_str()),
        Some("transient")
    );
    assert!(!access.set_cookies[0].contains("Domain="));
}

#[test]
fn advanced_rule_precedes_each_ip_or_mobility_grant_type() {
    for grant_type in [
        "login_ip_grant",
        "fnos_fingerprint_session",
        "session_migration",
    ] {
        assert!(is_ip_or_mobility_grant(grant_type));
    }
    for grant_type in ["browser_session", "manual_whitelist", "local_network"] {
        assert!(!is_ip_or_mobility_grant(grant_type));
    }
}

#[tokio::test]
async fn verify_clears_all_stale_cookie_scopes_even_when_local_access_is_allowed() {
    let (_directory, state) = auth_route_test_state("missing-session").await;
    let mut headers = forwarded_headers("auth.example.com:7999");
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("x-go-reauth-proxy-session-id=missing-session"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("127.0.0.1"));

    let response = verify(State(state), headers, Uri::from_static("/api/auth/verify")).await;

    assert_eq!(response.status(), StatusCode::OK);
    let cookies = response_set_cookies(&response);
    assert_eq!(cookies.len(), 3);
    assert_clear_cookie_scopes(&cookies);
}

#[tokio::test]
async fn logout_reports_unconfirmed_revocation_and_clears_all_cookie_scopes() {
    let (_directory, state) = auth_route_test_state("logout-cookie-scopes").await;
    let headers = forwarded_headers("auth.example.com:7999");

    let response = logout(State(state), headers, Uri::from_static("/api/auth/logout")).await;

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let cookies = response_set_cookies(&response);
    assert_eq!(cookies.len(), 6);
    let session_cookies = cookies
        .iter()
        .filter(|cookie| cookie.starts_with(cookies::SESSION_COOKIE_NAME))
        .cloned()
        .collect::<Vec<_>>();
    let share_cookies = cookies
        .iter()
        .filter(|cookie| cookie.starts_with(cookies::FNOS_SHARE_SESSION_COOKIE_NAME))
        .cloned()
        .collect::<Vec<_>>();
    assert_clear_cookie_scopes(&session_cookies);
    assert_clear_cookie_scopes(&share_cookies);
}

#[tokio::test]
async fn verify_rejects_and_destroys_expired_presented_session() {
    let (_directory, state) = auth_route_test_state("expired-session").await;
    let session_id = "expired-session";
    state
        .store
        .add_session(
            session_id,
            &auth_route_test_session("203.0.113.10", "2000-01-01T00:00:00Z"),
            3600,
        )
        .await
        .expect("store expired session fixture");
    let mut headers = forwarded_headers("auth.example.com:7999");
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("x-go-reauth-proxy-session-id=expired-session"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.10"));

    let response = verify(
        State(state.clone()),
        headers,
        Uri::from_static("/api/auth/verify"),
    )
    .await;

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_clear_cookie_scopes(&response_set_cookies(&response));
    assert!(
        state
            .store
            .get_session(session_id)
            .await
            .expect("read expired session after verify")
            .is_none()
    );
}

#[tokio::test]
async fn expired_session_response_is_not_blocked_by_held_mobility_lease() {
    let (_directory, state) = auth_route_test_state("expired-session-lock-contention").await;
    let session_id = "expired-session-with-held-lock";
    state
        .store
        .add_session(
            session_id,
            &auth_route_test_session("203.0.113.12", "2000-01-01T00:00:00Z"),
            3600,
        )
        .await
        .expect("store expired session fixture");
    let lock_key = crate::auth_mobility_keys::session_mutation_lock_key(session_id);
    assert!(
        state
            .store
            .set_json_value_nx_ex(
                &lock_key,
                &json!({ "lockId": "held-by-another-request" }),
                60,
            )
            .await
            .expect("hold session mutation lock")
    );
    let mut headers = forwarded_headers("auth.example.com:7999");
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("x-go-reauth-proxy-session-id=expired-session-with-held-lock"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.12"));

    let response = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        verify(
            State(state.clone()),
            headers,
            Uri::from_static("/api/auth/verify"),
        ),
    )
    .await
    .expect("expired-session response must not wait for mobility lease cleanup");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_clear_cookie_scopes(&response_set_cookies(&response));
    assert!(
        state
            .store
            .get_session(session_id)
            .await
            .expect("authoritative expired session was deleted")
            .is_none()
    );
}

#[tokio::test]
async fn bootstrap_migrates_valid_auth_host_session_to_shared_cookie_domain() {
    let (_directory, state) = auth_route_test_state("shared-cookie-migration").await;
    let session_id = "valid-auth-host-session";
    let session = auth_route_test_session("203.0.113.11", &time_utils::iso_after_seconds(3600));
    state
        .store
        .add_session(session_id, &session, 3600)
        .await
        .expect("store valid session fixture");
    let mut headers = forwarded_headers("auth.example.com:7999");
    headers.insert("x-forwarded-proto", HeaderValue::from_static("http"));
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("x-go-reauth-proxy-session-id=valid-auth-host-session"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("203.0.113.11"));

    let response = bootstrap(
        State(state),
        headers,
        Uri::from_static("/api/auth/bootstrap"),
        Query(BootstrapQuery {
            redirect_uri: Some("https://app.example.com/dashboard".to_string()),
        }),
    )
    .await;

    assert_eq!(response.status(), StatusCode::OK);
    let cookies = response_set_cookies(&response);
    assert_eq!(cookies.len(), 1);
    assert!(cookies[0].contains("x-go-reauth-proxy-session-id=valid-auth-host-session"));
    assert!(cookies[0].contains("Domain=example.com"));
    assert!(!cookies[0].contains("Max-Age=0"));
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
    assert_eq!(pow_secret_number_from_random(0, POW_MAX_NUMBER), 0);
    assert_eq!(
        pow_secret_number_from_random(POW_MAX_NUMBER, POW_MAX_NUMBER),
        0
    );
    assert!(pow_secret_number_from_random(u32::MAX, 300_000) < 300_000);
}

#[test]
fn pow_difficulty_uses_uncommon_tier_only_for_uncommon_locations() {
    let settings = json!({
        "pow": {
            "base_max_number": 120000,
            "uncommon_location": { "enabled": true, "max_number": 360000 }
        }
    });
    assert_eq!(
        pow_max_number_for_classification(
            &settings,
            common_auth_locations::CommonAuthLocationClassification::Common
        ),
        120000
    );
    assert_eq!(
        pow_max_number_for_classification(
            &settings,
            common_auth_locations::CommonAuthLocationClassification::Unknown
        ),
        120000
    );
    assert_eq!(
        pow_max_number_for_classification(
            &settings,
            common_auth_locations::CommonAuthLocationClassification::Uncommon
        ),
        360000
    );
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
        access_scopes: None,
        subdomain_access: None,
        grant_type: Some("login_ip_grant".to_string()),
        post_login_ip_grant_mode: Some("custom".to_string()),
        post_login_ip_grant_record_id: None,
        stream_access_expires_at: None,
        comment: None,
        ip: "203.0.113.10".to_string(),
        user_agent: "ua".to_string(),
        login_time: "2026-01-01T00:00:00Z".to_string(),
        expires_at: Some("2026-01-02T00:00:00Z".to_string()),
        ip_location: None,
    };
    assert!(auth_mobility::should_revoke_custom_post_login_ip_grant(
        Some(&session),
        Some(&json!({}))
    ));
    assert!(auth_mobility::should_revoke_custom_post_login_ip_grant(
        Some(&session),
        None
    ));

    session.post_login_ip_grant_mode = Some("follow_session".to_string());
    session.comment = Some("Automatically authorized after sign-in".to_string());
    assert!(!auth_mobility::should_revoke_custom_post_login_ip_grant(
        Some(&session),
        None
    ));
    assert!(auth_mobility::should_revoke_custom_post_login_ip_grant(
        Some(&session),
        Some(&json!({"auth_credential_settings": {"post_login_ip_grant_mode": "custom"}}))
    ));
    assert!(!auth_mobility::should_revoke_custom_post_login_ip_grant(
        Some(&session),
        Some(&json!({"auth_credential_settings": {"post_login_ip_grant_mode": "follow_session"}}))
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

fn shared_auth_test_config() -> Value {
    json!({
        "run_type": 3,
        "subdomain_mode": {
            "root_domain": "example.com",
            "auth_host": "auth.example.com",
            "public_https_port": 443
        }
    })
}

async fn auth_route_test_state(name: &str) -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("temporary auth route database");
    let mut settings = {
        let _environment = crate::test_support::EnvGuard::new(&[]);
        crate::settings::Settings::from_env()
    };
    settings.data_dir = directory.path().join("data");
    settings.gateway_config_dir = directory.path().join("gateway");
    settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
    settings.legacy_redis_url = String::new();
    settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
    settings.internal_rpc_token = format!("auth-route-{name}-test");
    settings.altcha_hmac_key = None;
    settings
        .ensure_altcha_hmac_key()
        .expect("persist auth route ALTCHA test key");
    let state = AppState::new(settings)
        .await
        .expect("auth route test state");
    state
        .store
        .save_config(&shared_auth_test_config())
        .await
        .expect("auth route test config");
    (directory, state)
}

#[tokio::test]
async fn wol_auth_api_requires_live_login_feature_portal_and_permission() {
    let (_directory, state) = auth_route_test_state("wol-auth-api").await;
    let app = Router::new()
        .merge(crate::wol::wol_routes(state.clone()))
        .nest("/api/auth", auth_api_routes())
        .with_state(state.clone());
    let disabled_admin = app
        .clone()
        .oneshot(
            Request::get("/api/admin/wol/targets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled_admin.status(), StatusCode::FORBIDDEN);

    let mut config = state.store.get_config().await.unwrap();
    config["wol_feature"] = json!({ "enabled": true });
    config["gateway_portal"] = json!({ "show_wol": true });
    state.store.save_config(&config).await.unwrap();

    for (name, mac, enabled) in [
        ("Visible workstation", "02:11:22:33:44:55", true),
        ("Disabled workstation", "02:11:22:33:44:66", false),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/admin/wol/targets")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": name,
                            "mac": mac,
                            "relayId": null,
                            "broadcastAddress": "127.0.0.1",
                            "ipAddress": "127.0.0.1",
                            "enabled": enabled,
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let anonymous = app
        .clone()
        .oneshot(
            Request::get("/api/auth/wol/targets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    state
        .store
        .add_totp(TotpCredential {
            id: "wol-user".to_string(),
            secret: "secret".to_string(),
            comment: "WoL user".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: Value::Null,
            subdomain_access: json!({ "mode": "custom", "hosts": [] }),
        })
        .await
        .unwrap();
    let mut session = auth_route_test_session("203.0.113.40", &time_utils::iso_after_seconds(3600));
    session.totp_id = "wol-user".to_string();
    session.credential_id = "wol-user".to_string();
    state
        .store
        .add_session("wol-session", &session, 3600)
        .await
        .unwrap();
    let request = || {
        Request::get("/api/auth/wol/targets")
            .header(header::COOKIE, "x-go-reauth-proxy-session-id=wol-session")
            .body(Body::empty())
            .unwrap()
    };

    let denied = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    state
        .store
        .update_totp_subdomain_access(
            "wol-user",
            json!({ "mode": "custom", "hosts": ["__builtin_wol__"] }),
        )
        .await
        .unwrap();
    let allowed = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
    assert!(
        allowed
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.contains("no-store"))
    );
    let payload = response_json(allowed).await;
    assert_eq!(payload.pointer("/data/total"), Some(&json!(1)));
    assert_eq!(
        payload.pointer("/data/items/0/name"),
        Some(&json!("Visible workstation"))
    );
    assert!(payload.pointer("/data/items/0/mac").is_none());
    assert!(payload.pointer("/data/items/0/broadcastAddress").is_none());
    assert!(payload.pointer("/data/items/0/ipAddress").is_none());
    assert!(payload.pointer("/data/items/0/status/observedIp").is_none());
    assert!(payload.pointer("/data/items/0/status/lastError").is_none());

    state.store.delete_totp("wol-user").await.unwrap();
    let deleted_account = app.clone().oneshot(request()).await.unwrap();
    assert_eq!(deleted_account.status(), StatusCode::UNAUTHORIZED);

    let mut config = state.store.get_config().await.unwrap();
    config["gateway_portal"]["show_wol"] = json!(false);
    state.store.save_config(&config).await.unwrap();
    let portal_disabled = app.oneshot(request()).await.unwrap();
    assert_eq!(portal_disabled.status(), StatusCode::FORBIDDEN);
}

fn captcha_route_test_app(state: AppState) -> Router {
    Router::new()
        .merge(runtime_config::runtime_config_routes())
        .nest("/api/auth", auth_api_routes())
        .with_state(state)
}

async fn response_json(response: Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("read response body");
    serde_json::from_slice(&body).expect("parse response body")
}

#[tokio::test]
async fn turnstile_admin_setting_drives_all_auth_captcha_routes() {
    let (_directory, state) = auth_route_test_state("turnstile-settings").await;
    let app = captcha_route_test_app(state);
    let turnstile_settings = json!({
        "provider": "turnstile",
        "widget_mode": "normal",
        "pow": {
            "base_max_number": 100000,
            "uncommon_location": { "enabled": false, "max_number": 300000 }
        },
        "turnstile": {
            "site_key": "turnstile-site-key",
            "secret_key": "turnstile-secret-key"
        }
    });

    let update_response = app
        .clone()
        .oneshot(
            Request::post("/api/admin/config/captcha")
                .header("content-type", "application/json")
                .body(Body::from(turnstile_settings.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(update_response).await["data"],
        turnstile_settings
    );

    let bootstrap_response = app
        .clone()
        .oneshot(
            Request::get("/api/auth/bootstrap")
                .header("host", "auth.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrap_response.status(), StatusCode::OK);
    let bootstrap = response_json(bootstrap_response).await;
    let public_settings = &bootstrap["data"]["captcha"];
    assert_eq!(public_settings["provider"], "turnstile");
    assert_eq!(public_settings["available"], true);
    assert_eq!(
        public_settings["turnstile"]["site_key"],
        "turnstile-site-key"
    );
    assert!(
        public_settings["turnstile"].get("secret_key").is_none(),
        "the public captcha payload must not expose the Turnstile secret"
    );

    let config_response = app
        .clone()
        .oneshot(
            Request::get("/api/auth/captcha/config")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(config_response.status(), StatusCode::OK);
    assert_eq!(
        response_json(config_response).await["data"],
        *public_settings
    );

    let challenge_response = app
        .clone()
        .oneshot(
            Request::get("/api/auth/challenge")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(challenge_response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        response_json(challenge_response).await["message"],
        captcha_text(
            &Translator::new(crate::i18n::DEFAULT_LOCALE),
            "powNotEnabled"
        )
    );

    let login_response = app
        .oneshot(
            Request::post("/api/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "token": "000000",
                        "captcha": { "provider": "pow", "proof": "unused" },
                        "rememberMe": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login_response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response_json(login_response).await["message"],
        captcha_text(
            &Translator::new(crate::i18n::DEFAULT_LOCALE),
            "providerConfigMismatch"
        )
    );
}

#[tokio::test]
async fn default_pow_setting_still_drives_bootstrap_and_challenge() {
    let (_directory, state) = auth_route_test_state("default-pow-settings").await;
    let app = captcha_route_test_app(state);

    let bootstrap_response = app
        .clone()
        .oneshot(
            Request::get("/api/auth/bootstrap")
                .header("host", "auth.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bootstrap_response.status(), StatusCode::OK);
    let bootstrap = response_json(bootstrap_response).await;
    assert_eq!(bootstrap["data"]["captcha"]["provider"], "pow");
    assert_eq!(bootstrap["data"]["captcha"]["available"], true);

    let challenge_response = app
        .oneshot(
            Request::get("/api/auth/challenge")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(challenge_response.status(), StatusCode::OK);
    let challenge = response_json(challenge_response).await;
    assert_eq!(challenge["algorithm"], "SHA-256");
    assert_eq!(challenge["maxnumber"], POW_MAX_NUMBER);
    assert!(
        challenge["challenge"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        challenge["signature"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
}

#[tokio::test]
async fn pow_challenge_selects_difficulty_from_common_location_classification() {
    let (_directory, state) = auth_route_test_state("pow-location-difficulty").await;
    state
        .store
        .set_json_value(
            "fn_knock:captcha:settings",
            &json!({
                "provider": "pow",
                "pow": {
                    "base_max_number": 100000,
                    "uncommon_location": { "enabled": true, "max_number": 300000 }
                },
                "turnstile": { "site_key": "", "secret_key": "" }
            }),
        )
        .await
        .unwrap();
    state
        .store
        .set_string_value(
            "fn_knock:common_auth_locations:runtime",
            &json!({
                "enabled": true,
                "locations": [{ "key": "中国|广东|深圳" }]
            })
            .to_string(),
        )
        .await
        .unwrap();
    for (ip, location) in [
        (
            "8.8.8.8",
            json!({ "country": "中国", "province": "广东", "city": "深圳" }),
        ),
        (
            "1.1.1.1",
            json!({ "country": "日本", "province": "东京", "city": "东京" }),
        ),
        (
            "2.2.2.2",
            json!({ "country": "日本", "province": "大阪", "city": "大阪" }),
        ),
    ] {
        state
            .store
            .complete_ip_location_lookup(ip, &location, &json!({ "status": "success" }), 60)
            .await
            .unwrap();
    }
    let inspection_state = state.clone();
    let app = captcha_route_test_app(state);

    for (ip, expected) in [
        ("8.8.8.8", 100000),
        ("1.1.1.1", 300000),
        ("9.9.9.9", 100000),
        ("127.0.0.1", 100000),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/auth/challenge")
                    .header("x-forwarded-for", ip)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["maxnumber"], expected);
    }
    assert_eq!(
        inspection_state
            .store
            .get_ip_location_state("9.9.9.9")
            .await
            .unwrap()
            .and_then(|state| state.get("status").cloned()),
        Some(json!("queued"))
    );
    assert!(
        inspection_state
            .store
            .get_ip_location_state("127.0.0.1")
            .await
            .unwrap()
            .is_none()
    );

    inspection_state
        .store
        .set_string_value(
            "fn_knock:common_auth_locations:runtime",
            &json!({
                "enabled": false,
                "locations": [{ "key": "中国|广东|深圳" }]
            })
            .to_string(),
        )
        .await
        .unwrap();
    for ip in ["2.2.2.2", "4.4.4.4"] {
        let response = app
            .clone()
            .oneshot(
                Request::get("/api/auth/challenge")
                    .header("x-forwarded-for", ip)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response_json(response).await["maxnumber"], 100000);
    }
    assert_eq!(
        inspection_state
            .store
            .get_ip_location_state("4.4.4.4")
            .await
            .unwrap()
            .and_then(|state| state.get("status").cloned()),
        Some(json!("queued"))
    );
    inspection_state
        .store
        .set_string_value(
            "fn_knock:common_auth_locations:runtime",
            &json!({
                "enabled": true,
                "locations": [{ "key": "中国|广东|深圳" }]
            })
            .to_string(),
        )
        .await
        .unwrap();

    let update_response = app
        .clone()
        .oneshot(
            Request::post("/api/admin/config/captcha")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "provider": "pow",
                        "pow": {
                            "base_max_number": 150000,
                            "uncommon_location": { "enabled": true, "max_number": 450000 }
                        },
                        "turnstile": { "site_key": "", "secret_key": "" }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let challenge_response = app
        .oneshot(
            Request::get("/api/auth/challenge")
                .header("x-forwarded-for", "1.1.1.1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response_json(challenge_response).await["maxnumber"], 450000);
}

#[tokio::test]
async fn captcha_admin_rejects_invalid_pow_difficulty() {
    let (_directory, state) = auth_route_test_state("invalid-pow-difficulty").await;
    let app = captcha_route_test_app(state);

    for payload in [
        json!({ "pow": { "base_max_number": 9999 } }),
        json!({ "pow": { "base_max_number": 400000 } }),
        json!({ "pow": { "uncommon_location": { "enabled": "yes" } } }),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::post("/api/admin/config/captcha")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

fn auth_route_test_session(ip: &str, expires_at: &str) -> LoginSession {
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
        ip: ip.to_string(),
        user_agent: "auth-route-test".to_string(),
        login_time: time_utils::now_iso(),
        expires_at: Some(expires_at.to_string()),
        ip_location: None,
    }
}

fn response_set_cookies(response: &Response) -> Vec<String> {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(ToString::to_string)
        .collect()
}

fn assert_clear_cookie_scopes(cookies: &[String]) {
    assert_eq!(cookies.len(), 3);
    assert!(cookies.iter().all(|cookie| cookie.contains("Max-Age=0")));
    assert!(
        cookies
            .iter()
            .all(|cookie| cookie.contains("Expires=Thu, 01 Jan 1970 00:00:00 GMT"))
    );
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
