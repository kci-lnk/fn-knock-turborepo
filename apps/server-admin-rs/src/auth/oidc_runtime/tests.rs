use super::*;

#[test]
fn validates_oidc_flow_token_with_state_hash() {
    let state = "state-token";
    assert!(oidc_flow_token_valid(state, Some(&hash_oidc_token(state))));
    assert!(!oidc_flow_token_valid(state, Some("wrong")));
}

#[test]
fn resolves_auth_cookie_path_from_prefixed_routes() {
    assert_eq!(auth_view_prefix("/auth/api/auth/oidc/start"), Some("/auth"));
    assert_eq!(
        auth_view_prefix("/__auth__/api/auth/oidc/start"),
        Some("/__auth__")
    );
    assert_eq!(auth_view_prefix("/api/auth/oidc/start"), None);
}

#[test]
fn parses_json_and_form_payloads() {
    let translator = Translator::new(DEFAULT_LOCALE);
    assert_eq!(
        parse_json_or_form(r#"{"access_token":"abc"}"#, "application/json", &translator).unwrap()["access_token"],
        json!("abc")
    );
    assert_eq!(
        parse_json_or_form(
            "access_token=abc&token_type=bearer",
            "text/plain",
            &translator
        )
        .unwrap()["token_type"],
        json!("bearer")
    );
}

#[test]
fn detects_oidc_operation_aborted_errors_like_node() {
    assert!(is_oidc_operation_aborted_error(
        "The operation was aborted before completion"
    ));
    assert!(is_oidc_operation_aborted_error(
        "AbortError: request aborted"
    ));
    assert!(!is_oidc_operation_aborted_error("invalid_grant"));
}

#[test]
fn oidc_outbound_requests_include_fetch_like_user_agent() {
    let client = reqwest::Client::new();
    let token_request = oidc_http_request(
        client.post("https://example.test/token"),
        "application/json",
    )
    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
    .body("grant_type=authorization_code")
    .build()
    .unwrap();
    assert_eq!(
        token_request
            .headers()
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some(OIDC_HTTP_USER_AGENT)
    );

    let github_request = github_api_request(&client, "https://api.github.com/user", "access-token")
        .build()
        .unwrap();
    let headers = github_request.headers();
    assert_eq!(
        headers
            .get(header::USER_AGENT)
            .and_then(|value| value.to_str().ok()),
        Some(OIDC_HTTP_USER_AGENT)
    );
    assert_eq!(
        headers
            .get(header::ACCEPT)
            .and_then(|value| value.to_str().ok()),
        Some("application/vnd.github+json")
    );
    assert_eq!(
        headers
            .get("X-GitHub-Api-Version")
            .and_then(|value| value.to_str().ok()),
        Some("2022-11-28")
    );
    assert_eq!(
        headers
            .get(header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok()),
        Some("Bearer access-token")
    );
}

#[test]
fn localizes_oidc_runtime_text() {
    let translator = Translator::new("zh-CN");
    assert_eq!(
        provider_error_message("access_denied", &translator),
        "你取消了外部登录授权，或授权请求被提供商拒绝。"
    );
    assert_eq!(
        normalize_login_error_message("   ", &translator),
        "外部登录失败，请重新发起登录。"
    );
    assert_eq!(
        oidc_login_failed_retry_after_message(&translator, "invalid_grant", 3),
        "invalid_grant，请在 3 秒后重试"
    );
    assert_eq!(
        request_origin(
            &HeaderMap::new(),
            &Uri::from_static("/api/auth/oidc/start"),
            &translator
        )
        .unwrap_err(),
        "无法生成外部登录回调地址，请配置 public_auth_base_url"
    );
    assert_eq!(
        request_origin(
            &HeaderMap::new(),
            &Uri::from_static("https://auth.example.com/api/auth/oidc/start"),
            &translator
        )
        .unwrap(),
        "https://auth.example.com"
    );
    let mut headers = HeaderMap::new();
    headers.insert("host", HeaderValue::from_static("auth.example.com:7999"));
    assert_eq!(
        request_origin(
            &headers,
            &Uri::from_static("/api/auth/oidc/start"),
            &translator
        )
        .unwrap(),
        "http://auth.example.com:7999"
    );
    assert!(
        parse_json_or_form("{bad", "application/json", &translator)
            .unwrap_err()
            .starts_with("外部登录响应不是有效 JSON")
    );
}
