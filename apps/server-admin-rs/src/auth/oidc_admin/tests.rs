use super::*;

#[test]
fn normalizes_google_provider_config_with_defaults() {
    let translator = Translator::new("zh-CN");
    let config = normalize_connection_config(
        "google",
        map_from_values(&[
            ("client_id", json!("client")),
            ("client_secret", json!("secret")),
        ]),
        false,
        &translator,
    )
    .unwrap();
    assert_eq!(config["issuer"], "https://accounts.google.com");
    assert_eq!(config["scopes"], json!(["openid", "profile", "email"]));
}

#[test]
fn normalizes_oidc_scopes_like_node_array_vs_string_inputs() {
    assert_eq!(
        normalize_scopes(Some(&json!("openid profile,email")), &["fallback"]),
        vec!["openid", "profile", "email"]
    );
    assert_eq!(
        normalize_scopes(
            Some(&json!(["openid profile", "email", "email"])),
            &["fallback"]
        ),
        vec!["openid profile", "email"]
    );
}

#[test]
fn rejects_reserved_extra_auth_param() {
    let translator = Translator::new("zh-CN");
    let error = normalize_connection_config(
        "google",
        map_from_values(&[
            ("client_id", json!("client")),
            ("client_secret", json!("secret")),
            ("extra_auth_params", json!({ "state": "bad" })),
        ]),
        false,
        &translator,
    )
    .unwrap_err();
    assert_eq!(error, "extra_auth_params 包含 OIDC 保留参数: state");
}

#[test]
fn masks_provider_secret() {
    let provider = json!({
        "id": "oidc_provider_test",
        "type": "github",
        "protocol": "oauth2_profile",
        "name": "GitHub",
        "enabled": true,
        "created_at": "2026-07-05T00:00:00Z",
        "updated_at": "2026-07-05T00:00:00Z",
        "connection_config": {
            "client_id": "id",
            "client_secret": "verysecret"
        }
    });
    let view = mask_provider(provider, Some("https://auth.example.com"));
    assert_eq!(
        view.pointer("/connection_config_masked/client_secret"),
        Some(&Value::String("ve******".to_string()))
    );
    assert_eq!(
        view.get("callback_url").and_then(Value::as_str),
        Some("https://auth.example.com/api/auth/oidc/callback/oidc_provider_test")
    );
    assert!(view.get("connection_config").is_none());
}

#[test]
fn detects_missing_required_provider_fields() {
    let provider = json!({
        "id": "oidc_provider_test",
        "type": "custom_oidc",
        "protocol": "oidc",
        "connection_config": {
            "client_id": "client",
            "client_secret": ""
        }
    });
    assert_eq!(
        missing_required_provider_fields(&provider),
        vec!["client_secret", "issuer"]
    );
}

#[test]
fn localizes_oidc_catalog_and_validation_text() {
    let translator = Translator::new("zh-CN");
    let catalog = provider_catalog(&translator);
    let custom = catalog
        .iter()
        .find(|provider| provider.get("type").and_then(Value::as_str) == Some("custom_oidc"))
        .unwrap();
    assert_eq!(
        custom.get("label").and_then(Value::as_str),
        Some("自定义 OIDC")
    );
    assert_eq!(
        oidc_text_params(
            &translator,
            "providerMissingRequiredFields",
            &[("fields", "client_secret".to_string())]
        ),
        "外部登录提供商缺少必填配置 client_secret"
    );
}

#[test]
fn builds_invite_base_url_from_public_auth_config_or_auth_host() {
    assert_eq!(
        public_auth_base_url(&json!({
            "subdomain_mode": {
                "public_auth_base_url": "https://auth.example.com/auth/",
                "public_https_port": 8443
            }
        })),
        Some("https://auth.example.com:8443/auth".to_string())
    );
    assert_eq!(
        public_auth_base_url(&json!({
            "host_mappings": [{
                "host": "Auth.Example.Com",
                "target": "http://127.0.0.1:7997"
            }]
        })),
        Some("https://auth.example.com:7999".to_string())
    );
}

#[test]
fn builds_callback_base_url_from_public_auth_config_before_request_host() {
    let mut headers = HeaderMap::new();
    headers.insert("host", "admin.example.com:7999".parse().unwrap());
    let uri = Uri::from_static("/api/admin/auth/oidc/providers");
    assert_eq!(
        callback_base_url(
            &headers,
            &uri,
            &json!({
                "subdomain_mode": {
                    "public_auth_base_url": "https://auth.example.com/auth/"
                }
            })
        ),
        Some("https://auth.example.com:7999/auth".to_string())
    );
}

#[test]
fn callback_origin_uses_uri_or_host_like_node_fallback() {
    assert_eq!(
        callback_origin(
            &HeaderMap::new(),
            &Uri::from_static("https://auth.example.com/api/admin/auth/oidc/providers")
        ),
        Some("https://auth.example.com".to_string())
    );

    let mut headers = HeaderMap::new();
    headers.insert("host", "auth.example.com:7999".parse().unwrap());
    assert_eq!(
        callback_origin(
            &headers,
            &Uri::from_static("/api/admin/auth/oidc/providers")
        ),
        Some("http://auth.example.com:7999".to_string())
    );
}

fn map_from_values(values: &[(&str, Value)]) -> Map<String, Value> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}
