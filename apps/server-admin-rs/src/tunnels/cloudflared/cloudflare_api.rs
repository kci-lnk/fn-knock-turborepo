use std::{env, time::Duration};

use reqwest::{Client, Method, StatusCode};
use serde_json::{Value, json};
use url::Url;

const DEFAULT_API_BASE_URL: &str = "https://api.cloudflare.com/client/v4";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiTokenKind {
    User,
    Account,
    Legacy,
}

#[derive(Debug, Clone)]
pub(super) struct CloudflareApiError {
    pub(super) status: Option<StatusCode>,
    pub(super) message: String,
}

impl std::fmt::Display for CloudflareApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CloudflareApiError {}

#[derive(Clone)]
pub(super) struct CloudflareApi {
    client: Client,
    token: String,
    base_url: String,
}

impl CloudflareApi {
    pub(super) fn new(client: Client, token: impl Into<String>) -> Self {
        Self {
            client,
            token: token.into(),
            base_url: env::var("FN_KNOCK_CLOUDFLARE_API_BASE_URL")
                .ok()
                .map(|value| value.trim().trim_end_matches('/').to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_string()),
        }
    }

    #[cfg(test)]
    pub(super) fn with_base_url(
        client: Client,
        token: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            client,
            token: token.into(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
        }
    }

    pub(super) async fn verify_token(&self, account_id: &str) -> Result<Value, CloudflareApiError> {
        let account_path = || format!("/accounts/{account_id}/tokens/verify");
        let value = match api_token_kind(&self.token) {
            ApiTokenKind::User => {
                self.request(Method::GET, "/user/tokens/verify", &[], None)
                    .await?
            }
            ApiTokenKind::Account => {
                self.request(Method::GET, &account_path(), &[], None)
                    .await?
            }
            ApiTokenKind::Legacy => {
                match self
                    .request(Method::GET, "/user/tokens/verify", &[], None)
                    .await
                {
                    Ok(value) => value,
                    Err(error) if token_endpoint_mismatch_may_apply(&error) => {
                        self.request(Method::GET, &account_path(), &[], None)
                            .await?
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        if value.pointer("/result/status").and_then(Value::as_str) != Some("active") {
            return Err(CloudflareApiError {
                status: Some(StatusCode::UNAUTHORIZED),
                message: "Cloudflare API Token is not active".to_string(),
            });
        }
        Ok(value)
    }

    pub(super) async fn find_zone(&self, hostname: &str) -> Result<Value, CloudflareApiError> {
        let normalized = hostname.trim().trim_end_matches('.').to_ascii_lowercase();
        let candidates = enclosing_zone_candidates(&normalized);
        if candidates.is_empty() {
            return Err(CloudflareApiError {
                status: Some(StatusCode::BAD_REQUEST),
                message: format!("Cloudflare zone lookup hostname {hostname} is invalid"),
            });
        }

        // The configured fn-knock root may itself be a subdomain (for example,
        // edge.example.com) while the Cloudflare Zone is example.com. Prefer an
        // exact delegated child Zone when one exists, then walk towards the
        // parent so the narrowest enclosing active Zone wins.
        for candidate in candidates {
            let value = self
                .request(
                    Method::GET,
                    "/zones",
                    &[
                        ("name", candidate.to_string()),
                        ("status", "active".to_string()),
                        ("per_page", "50".to_string()),
                    ],
                    None,
                )
                .await?;
            let mut matching = value
                .get("result")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|zone| {
                    zone.get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value.eq_ignore_ascii_case(candidate))
                });
            let Some(zone) = matching.next() else {
                continue;
            };
            if matching.next().is_some() {
                return Err(CloudflareApiError {
                    status: Some(StatusCode::CONFLICT),
                    message: format!("Cloudflare returned multiple active zones named {candidate}"),
                });
            }
            return Ok(zone.clone());
        }

        Err(CloudflareApiError {
            status: Some(StatusCode::NOT_FOUND),
            message: format!(
                "No active Cloudflare zone containing {normalized} was found for this API Token"
            ),
        })
    }

    pub(super) async fn list_tunnels(
        &self,
        account_id: &str,
    ) -> Result<Vec<Value>, CloudflareApiError> {
        self.list_all(
            &format!("/accounts/{account_id}/cfd_tunnel"),
            &[("is_deleted", "false".to_string())],
            100,
        )
        .await
    }

    pub(super) async fn create_tunnel(
        &self,
        account_id: &str,
        name: &str,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::POST,
                &format!("/accounts/{account_id}/cfd_tunnel"),
                &[],
                Some(json!({ "name": name, "config_src": "cloudflare" })),
            )
            .await?;
        required_result(value, "create Cloudflare Tunnel")
    }

    pub(super) async fn delete_tunnel(
        &self,
        account_id: &str,
        tunnel_id: &str,
    ) -> Result<(), CloudflareApiError> {
        self.request(
            Method::DELETE,
            &format!("/accounts/{account_id}/cfd_tunnel/{tunnel_id}"),
            &[],
            None,
        )
        .await
        .map(drop)
    }

    pub(super) async fn get_tunnel_config(
        &self,
        account_id: &str,
        tunnel_id: &str,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::GET,
                &format!("/accounts/{account_id}/cfd_tunnel/{tunnel_id}/configurations"),
                &[],
                None,
            )
            .await?;
        Ok(value.get("result").cloned().unwrap_or_else(|| json!({})))
    }

    pub(super) async fn update_tunnel_config(
        &self,
        account_id: &str,
        tunnel_id: &str,
        config: Value,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::PUT,
                &format!("/accounts/{account_id}/cfd_tunnel/{tunnel_id}/configurations"),
                &[],
                Some(json!({ "config": config })),
            )
            .await?;
        required_result(value, "update Cloudflare Tunnel configuration")
    }

    pub(super) async fn get_tunnel_token(
        &self,
        account_id: &str,
        tunnel_id: &str,
    ) -> Result<String, CloudflareApiError> {
        let value = self
            .request(
                Method::GET,
                &format!("/accounts/{account_id}/cfd_tunnel/{tunnel_id}/token"),
                &[],
                None,
            )
            .await?;
        value
            .get("result")
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| CloudflareApiError {
                status: None,
                message: "Cloudflare returned an empty Tunnel token".to_string(),
            })
    }

    pub(super) async fn list_dns_records(
        &self,
        zone_id: &str,
        name: Option<&str>,
    ) -> Result<Vec<Value>, CloudflareApiError> {
        let mut query = Vec::new();
        if let Some(name) = name {
            query.push(("name", name.to_string()));
            query.push(("match", "all".to_string()));
        }
        self.list_all(&format!("/zones/{zone_id}/dns_records"), &query, 100)
            .await
    }

    pub(super) async fn create_dns_record(
        &self,
        zone_id: &str,
        body: Value,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::POST,
                &format!("/zones/{zone_id}/dns_records"),
                &[],
                Some(body),
            )
            .await?;
        required_result(value, "create Cloudflare DNS record")
    }

    pub(super) async fn update_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
        body: Value,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::PATCH,
                &format!("/zones/{zone_id}/dns_records/{record_id}"),
                &[],
                Some(body),
            )
            .await?;
        required_result(value, "update Cloudflare DNS record")
    }

    pub(super) async fn delete_dns_record(
        &self,
        zone_id: &str,
        record_id: &str,
    ) -> Result<(), CloudflareApiError> {
        self.request(
            Method::DELETE,
            &format!("/zones/{zone_id}/dns_records/{record_id}"),
            &[],
            None,
        )
        .await
        .map(drop)
    }

    pub(super) async fn list_custom_hostnames(
        &self,
        zone_id: &str,
        hostname: Option<&str>,
    ) -> Result<Vec<Value>, CloudflareApiError> {
        let mut query = Vec::new();
        if let Some(hostname) = hostname {
            query.push(("hostname", hostname.to_string()));
        }
        self.list_all(&format!("/zones/{zone_id}/custom_hostnames"), &query, 50)
            .await
    }

    pub(super) async fn get_custom_hostname(
        &self,
        zone_id: &str,
        custom_hostname_id: &str,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::GET,
                &format!("/zones/{zone_id}/custom_hostnames/{custom_hostname_id}"),
                &[],
                None,
            )
            .await?;
        required_result(value, "get Cloudflare custom hostname")
    }

    pub(super) async fn create_custom_hostname(
        &self,
        zone_id: &str,
        hostname: &str,
        origin: &str,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::POST,
                &format!("/zones/{zone_id}/custom_hostnames"),
                &[],
                Some(json!({
                    "hostname": hostname,
                    "custom_origin_server": origin,
                    "ssl": { "method": "txt", "type": "dv" }
                })),
            )
            .await?;
        required_result(value, "create Cloudflare custom hostname")
    }

    pub(super) async fn delete_custom_hostname(
        &self,
        zone_id: &str,
        custom_hostname_id: &str,
    ) -> Result<(), CloudflareApiError> {
        self.request(
            Method::DELETE,
            &format!("/zones/{zone_id}/custom_hostnames/{custom_hostname_id}"),
            &[],
            None,
        )
        .await
        .map(drop)
    }

    pub(super) async fn get_fallback_origin(
        &self,
        zone_id: &str,
    ) -> Result<Option<Value>, CloudflareApiError> {
        match self
            .request(
                Method::GET,
                &format!("/zones/{zone_id}/custom_hostnames/fallback_origin"),
                &[],
                None,
            )
            .await
        {
            Ok(value) => Ok(value
                .get("result")
                .cloned()
                .filter(|value| !value.is_null())),
            Err(error) if error.status == Some(StatusCode::NOT_FOUND) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) async fn update_fallback_origin(
        &self,
        zone_id: &str,
        origin: &str,
    ) -> Result<Value, CloudflareApiError> {
        let value = self
            .request(
                Method::PUT,
                &format!("/zones/{zone_id}/custom_hostnames/fallback_origin"),
                &[],
                Some(json!({ "origin": origin })),
            )
            .await?;
        required_result(value, "update Cloudflare for SaaS fallback origin")
    }

    pub(super) async fn delete_fallback_origin(
        &self,
        zone_id: &str,
    ) -> Result<(), CloudflareApiError> {
        self.request(
            Method::DELETE,
            &format!("/zones/{zone_id}/custom_hostnames/fallback_origin"),
            &[],
            None,
        )
        .await
        .map(drop)
    }

    async fn list_all(
        &self,
        path: &str,
        query: &[(&str, String)],
        per_page: usize,
    ) -> Result<Vec<Value>, CloudflareApiError> {
        let mut page = 1usize;
        let mut output = Vec::new();
        loop {
            let mut page_query = query
                .iter()
                .map(|&(key, ref value)| (key, value.clone()))
                .collect::<Vec<_>>();
            page_query.push(("page", page.to_string()));
            page_query.push(("per_page", per_page.to_string()));
            let value = self.request(Method::GET, path, &page_query, None).await?;
            let items = value
                .get("result")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let item_count = items.len();
            output.extend(items);
            let total_pages = value
                .pointer("/result_info/total_pages")
                .and_then(Value::as_u64)
                .map(|value| value as usize);
            let has_more = total_pages.is_some_and(|total| page < total)
                || (total_pages.is_none() && item_count == per_page);
            if has_more {
                page += 1;
                if page <= 50 {
                    continue;
                }
                return Err(CloudflareApiError {
                    status: Some(StatusCode::CONFLICT),
                    message: format!(
                        "Cloudflare returned more than 50 pages for {path}; refusing to reconcile an incomplete remote snapshot"
                    ),
                });
            }
            break;
        }
        Ok(output)
    }

    async fn request(
        &self,
        method: Method,
        path: &str,
        query: &[(&str, String)],
        body: Option<Value>,
    ) -> Result<Value, CloudflareApiError> {
        let mut url = Url::parse(&format!("{}{}", self.base_url, path)).map_err(|error| {
            CloudflareApiError {
                status: None,
                message: format!("Cloudflare API URL is invalid: {error}"),
            }
        })?;
        {
            let mut pairs = url.query_pairs_mut();
            for (key, value) in query {
                pairs.append_pair(key, value);
            }
        }
        let mut request = self
            .client
            .request(method, url)
            .bearer_auth(&self.token)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(Duration::from_secs(20));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.map_err(|error| CloudflareApiError {
            status: error.status(),
            message: if error.is_timeout() {
                "Cloudflare API request timed out".to_string()
            } else {
                format!("Cloudflare API request failed: {error}")
            },
        })?;
        let status = response.status();
        let raw = response.bytes().await.map_err(|error| CloudflareApiError {
            status: Some(status),
            message: format!("Cloudflare API response could not be read: {error}"),
        })?;
        let value = serde_json::from_slice::<Value>(&raw).unwrap_or_else(|_| {
            json!({
                "success": false,
                "errors": [{ "message": String::from_utf8_lossy(&raw).chars().take(500).collect::<String>() }]
            })
        });
        if status.is_success() && value.get("success").and_then(Value::as_bool) != Some(false) {
            return Ok(value);
        }
        let authentication_error = cloudflare_authentication_error(&value);
        Err(CloudflareApiError {
            // Cloudflare commonly reports an invalid, revoked, or unusable API
            // token as HTTP 403 + error 10000. Normalize that to an
            // authentication failure so callers do not misdiagnose it as a
            // missing Tunnel/DNS permission.
            status: Some(if authentication_error {
                StatusCode::UNAUTHORIZED
            } else {
                status
            }),
            message: cloudflare_error_message(&value, status),
        })
    }
}

fn api_token_kind(token: &str) -> ApiTokenKind {
    let token = token.trim();
    if token.starts_with("cfut_") {
        ApiTokenKind::User
    } else if token.starts_with("cfat_") {
        ApiTokenKind::Account
    } else {
        ApiTokenKind::Legacy
    }
}

fn token_endpoint_mismatch_may_apply(error: &CloudflareApiError) -> bool {
    matches!(
        error.status,
        Some(StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN)
    )
}

fn enclosing_zone_candidates(hostname: &str) -> Vec<&str> {
    if hostname.is_empty()
        || hostname.starts_with('.')
        || hostname.ends_with('.')
        || hostname.split('.').any(str::is_empty)
    {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    let mut candidate = hostname;
    while candidate.contains('.') {
        candidates.push(candidate);
        let Some((_, parent)) = candidate.split_once('.') else {
            break;
        };
        candidate = parent;
    }
    candidates
}

fn required_result(value: Value, action: &str) -> Result<Value, CloudflareApiError> {
    value
        .get("result")
        .cloned()
        .ok_or_else(|| CloudflareApiError {
            status: None,
            message: format!("Cloudflare did not return a result for {action}"),
        })
}

fn cloudflare_error_message(value: &Value, status: StatusCode) -> String {
    if cloudflare_authentication_error(value) {
        return "Cloudflare API Token authentication failed (10000). Replace the saved credential with a Cloudflare API Token that can access the current account and Zone, then retry; a Tunnel Token cannot manage Cloudflare resources."
            .to_string();
    }
    let messages = value
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let message = item.get("message").and_then(Value::as_str)?.trim();
            if message.is_empty() {
                return None;
            }
            let code = item.get("code").and_then(Value::as_i64);
            Some(match code {
                Some(code) => format!("{message} ({code})"),
                None => message.to_string(),
            })
        })
        .collect::<Vec<_>>();
    if messages.is_empty() {
        format!("Cloudflare API returned HTTP {status}")
    } else {
        messages.join("; ")
    }
}

fn cloudflare_authentication_error(value: &Value) -> bool {
    value
        .get("errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|item| {
            item.get("code").and_then(Value::as_i64) == Some(10_000)
                && item
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|message| {
                        message
                            .to_ascii_lowercase()
                            .contains("authentication error")
                    })
        })
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use axum::{
        Json, Router,
        extract::{Query, State},
        http::{HeaderMap, header::AUTHORIZATION},
        response::{IntoResponse, Response},
        routing::get,
    };

    use super::*;

    #[test]
    fn turns_cloudflare_authentication_error_into_actionable_guidance() {
        let value = json!({
            "errors": [
                { "code": 10000, "message": "Authentication error" },
                { "message": "Permission denied" }
            ]
        });
        assert_eq!(
            cloudflare_error_message(&value, StatusCode::FORBIDDEN),
            "Cloudflare API Token authentication failed (10000). Replace the saved credential with a Cloudflare API Token that can access the current account and Zone, then retry; a Tunnel Token cannot manage Cloudflare resources."
        );
    }

    #[test]
    fn distinguishes_user_account_and_legacy_api_tokens() {
        assert_eq!(api_token_kind("cfut_example"), ApiTokenKind::User);
        assert_eq!(api_token_kind("cfat_example"), ApiTokenKind::Account);
        assert_eq!(api_token_kind("legacy-token"), ApiTokenKind::Legacy);
    }

    #[tokio::test]
    async fn normalizes_cloudflare_error_10000_as_authentication_failure() {
        async fn reject() -> Response {
            (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "errors": [{ "code": 10000, "message": "Authentication error" }]
                })),
            )
                .into_response()
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/zones", get(reject)))
                .await
                .expect("serve mock Cloudflare API");
        });
        let api = CloudflareApi::with_base_url(
            Client::new(),
            "cfut_test-token",
            format!("http://{address}"),
        );

        let error = api
            .find_zone("example.com")
            .await
            .expect_err("reject invalid API token");
        assert_eq!(error.status, Some(StatusCode::UNAUTHORIZED));
        assert!(error.message.contains("Tunnel Token cannot manage"));
        server.abort();
    }

    #[tokio::test]
    async fn verifies_token_and_discovers_zone_against_mock_api() {
        async fn verify(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer cfut_test-token")
            );
            Json(json!({ "success": true, "result": { "status": "active" } }))
        }

        async fn zones(
            headers: HeaderMap,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer cfut_test-token")
            );
            assert_eq!(query.get("name").map(String::as_str), Some("example.com"));
            assert_eq!(query.get("status").map(String::as_str), Some("active"));
            Json(json!({
                "success": true,
                "result": [{
                    "id": "zone-id",
                    "name": "example.com",
                    "account": { "id": "account-id" }
                }]
            }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/user/tokens/verify", get(verify))
                    .route("/zones", get(zones)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api = CloudflareApi::with_base_url(
            Client::new(),
            "cfut_test-token",
            format!("http://{address}"),
        );

        let zone = api.find_zone("example.com").await.expect("find mock zone");
        api.verify_token("account-id")
            .await
            .expect("verify mock token");
        assert_eq!(zone.get("id").and_then(Value::as_str), Some("zone-id"));
        assert_eq!(
            zone.pointer("/account/id").and_then(Value::as_str),
            Some("account-id")
        );
        server.abort();
    }

    #[tokio::test]
    async fn verifies_account_api_token_against_the_account_endpoint() {
        async fn verify(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers
                    .get(AUTHORIZATION)
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer cfat_test-token")
            );
            Json(json!({ "success": true, "result": { "status": "active" } }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/accounts/account-id/tokens/verify", get(verify)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api = CloudflareApi::with_base_url(
            Client::new(),
            "cfat_test-token",
            format!("http://{address}"),
        );

        api.verify_token("account-id")
            .await
            .expect("verify mock account token");
        server.abort();
    }

    #[tokio::test]
    async fn falls_back_to_the_account_endpoint_for_a_legacy_token() {
        async fn reject_user_endpoint() -> Response {
            (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "errors": [{ "code": 10000, "message": "Authentication error" }]
                })),
            )
                .into_response()
        }

        async fn verify_account_endpoint() -> Json<Value> {
            Json(json!({ "success": true, "result": { "status": "active" } }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/user/tokens/verify", get(reject_user_endpoint))
                    .route(
                        "/accounts/account-id/tokens/verify",
                        get(verify_account_endpoint),
                    ),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api = CloudflareApi::with_base_url(
            Client::new(),
            "legacy-token",
            format!("http://{address}"),
        );

        api.verify_token("account-id")
            .await
            .expect("verify legacy account token");
        server.abort();
    }

    #[test]
    fn generates_enclosing_zone_candidates_from_most_to_least_specific() {
        assert_eq!(
            enclosing_zone_candidates("app.edge.example.com"),
            vec!["app.edge.example.com", "edge.example.com", "example.com"]
        );
        assert!(enclosing_zone_candidates("invalid").is_empty());
        assert!(enclosing_zone_candidates("bad..example.com").is_empty());
    }

    #[tokio::test]
    async fn discovers_parent_zone_for_a_subdomain_root() {
        async fn zones(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
            assert_eq!(query.get("status").map(String::as_str), Some("active"));
            let result = match query.get("name").map(String::as_str) {
                Some("edge.example.com") => Vec::new(),
                Some("example.com") => vec![json!({
                    "id": "parent-zone-id",
                    "name": "example.com",
                    "account": { "id": "account-id" }
                })],
                other => panic!("unexpected Zone lookup candidate: {other:?}"),
            };
            Json(json!({ "success": true, "result": result }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/zones", get(zones)))
                .await
                .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        let zone = api
            .find_zone("edge.example.com")
            .await
            .expect("find enclosing parent zone");
        assert_eq!(
            zone.get("id").and_then(Value::as_str),
            Some("parent-zone-id")
        );
        assert_eq!(
            zone.get("name").and_then(Value::as_str),
            Some("example.com")
        );
        server.abort();
    }

    #[tokio::test]
    async fn rejects_a_successful_verify_response_for_an_inactive_token() {
        async fn verify() -> Json<Value> {
            Json(json!({ "success": true, "result": { "status": "disabled" } }))
        }

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new().route("/user/tokens/verify", get(verify)),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api = CloudflareApi::with_base_url(
            Client::new(),
            "cfut_test-token",
            format!("http://{address}"),
        );

        let error = api
            .verify_token("account-id")
            .await
            .expect_err("inactive token");
        assert_eq!(error.status, Some(StatusCode::UNAUTHORIZED));
        server.abort();
    }

    #[tokio::test]
    async fn manages_the_zone_fallback_origin_against_mock_api() {
        type FallbackState = Arc<Mutex<Option<Value>>>;

        async fn read(State(state): State<FallbackState>) -> Response {
            match state.lock().expect("fallback state").clone() {
                Some(value) => Json(json!({ "success": true, "result": value })).into_response(),
                None => (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "success": false,
                        "errors": [{ "code": 1414, "message": "Fallback origin not found" }]
                    })),
                )
                    .into_response(),
            }
        }

        async fn update(
            State(state): State<FallbackState>,
            Json(body): Json<Value>,
        ) -> Json<Value> {
            assert_eq!(
                body.get("origin").and_then(Value::as_str),
                Some("origin.example.com")
            );
            let value = json!({
                "origin": "origin.example.com",
                "status": "pending_deployment",
                "errors": [],
            });
            *state.lock().expect("fallback state") = Some(value.clone());
            Json(json!({ "success": true, "result": value }))
        }

        async fn delete(State(state): State<FallbackState>) -> Json<Value> {
            *state.lock().expect("fallback state") = None;
            Json(json!({ "success": true, "result": null }))
        }

        let state = Arc::new(Mutex::new(None));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Cloudflare API");
        let address = listener.local_addr().expect("mock API address");
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route(
                        "/zones/zone-id/custom_hostnames/fallback_origin",
                        get(read).put(update).delete(delete),
                    )
                    .with_state(state),
            )
            .await
            .expect("serve mock Cloudflare API");
        });
        let api =
            CloudflareApi::with_base_url(Client::new(), "test-token", format!("http://{address}"));

        assert!(
            api.get_fallback_origin("zone-id")
                .await
                .expect("read missing fallback")
                .is_none()
        );
        let created = api
            .update_fallback_origin("zone-id", "origin.example.com")
            .await
            .expect("create fallback");
        assert_eq!(
            created.get("status").and_then(Value::as_str),
            Some("pending_deployment")
        );
        assert_eq!(
            api.get_fallback_origin("zone-id")
                .await
                .expect("read fallback")
                .and_then(|value| value.get("origin").cloned()),
            Some(json!("origin.example.com"))
        );
        api.delete_fallback_origin("zone-id")
            .await
            .expect("delete fallback");
        assert!(
            api.get_fallback_origin("zone-id")
                .await
                .expect("read deleted fallback")
                .is_none()
        );
        server.abort();
    }
}
