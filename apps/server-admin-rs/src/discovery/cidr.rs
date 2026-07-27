use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use url::Url;

use crate::{http_utils, i18n::Translator, state::AppState};

mod service;

pub(crate) use service::{
    cities_payload, lookup_payload, lookup_region, lookup_regions, provinces_payload,
};
#[cfg(test)]
pub(crate) use service::{cities_total, lookup_payload_from_data, province_wide_label};

pub(crate) const DEFAULT_CIDR_API_URL: &str = "https://cidr.fnknock.cn/api/v1";
pub(crate) const CIDR_MINIMUM_OPERATOR_VERSION: &str = "0.1.3";
pub(crate) const CIDR_OPERATORS: [CidrOperator; 3] = [
    CidrOperator::Telecom,
    CidrOperator::Unicom,
    CidrOperator::Mobile,
];

const IP_LOCATION_API_SETTINGS_KEY: &str = "fn_knock:ip-location-api:settings";
const CIDR_USER_AGENT: &str = "fn-knock-server-admin/1.0";
const CAPABILITY_PROBE_VALUE: &str = "__fn_knock_operator_probe__";

#[derive(Debug, thiserror::Error)]
pub(crate) enum CidrError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Service(String),
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) struct CidrSelection {
    pub(crate) province: String,
    pub(crate) city: Option<String>,
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) query_city: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operator: Option<CidrOperator>,
    pub(crate) is_province_wide: bool,
    pub(crate) is_municipality: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CidrLookup {
    pub(crate) selection: CidrSelection,
    pub(crate) cidrs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum CidrOperator {
    #[serde(rename = "电信")]
    Telecom,
    #[serde(rename = "联通")]
    Unicom,
    #[serde(rename = "移动")]
    Mobile,
}

impl CidrOperator {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Telecom => "电信",
            Self::Unicom => "联通",
            Self::Mobile => "移动",
        }
    }

    pub(crate) fn parse_optional(value: Option<&str>) -> Result<Option<Self>, String> {
        let value = value.map(str::trim).unwrap_or("");
        match value {
            "" => Ok(None),
            "电信" => Ok(Some(Self::Telecom)),
            "联通" => Ok(Some(Self::Unicom)),
            "移动" => Ok(Some(Self::Mobile)),
            _ => Err(format!("invalid operator: {value}")),
        }
    }

    pub(crate) fn parse_value(value: Option<&Value>) -> Result<Option<Self>, String> {
        match value {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(value)) => Self::parse_optional(Some(value)),
            Some(_) => Err("invalid operator: expected a string or null".to_string()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CidrRegionQuery {
    pub(crate) province: String,
    pub(crate) query_city: Option<String>,
    pub(crate) operator: Option<CidrOperator>,
}

impl CidrRegionQuery {
    pub(crate) fn new(
        province: impl Into<String>,
        query_city: Option<impl Into<String>>,
        operator: Option<CidrOperator>,
    ) -> Self {
        Self {
            province: province.into().trim().to_string(),
            query_city: query_city
                .map(Into::into)
                .map(|value: String| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            operator,
        }
    }

    pub(crate) fn key(&self) -> String {
        format!(
            "{}::{}::{}",
            self.province,
            self.query_city.as_deref().unwrap_or(""),
            self.operator.map(CidrOperator::as_str).unwrap_or("")
        )
    }

    pub(crate) fn query_pairs(&self) -> Vec<(&str, &str)> {
        let mut query = vec![("province", self.province.as_str())];
        if let Some(city) = self.query_city.as_deref() {
            query.push(("city", city));
        }
        if let Some(operator) = self.operator {
            query.push(("operator", operator.as_str()));
        }
        query
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CidrOperatorCapability {
    pub(crate) supported: bool,
    pub(crate) operators: Vec<CidrOperator>,
    pub(crate) minimum_container_version: &'static str,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CidrCapabilities {
    pub(crate) source: String,
    pub(crate) operator_filtering: CidrOperatorCapability,
}

pub(crate) async fn configured_cidr_source(state: &AppState) -> Result<(String, String), String> {
    let settings = state
        .store
        .get_json_value(IP_LOCATION_API_SETTINGS_KEY)
        .await
        .map_err(|error| error.to_string())?
        .unwrap_or_else(|| {
            json!({
                "cidr_mode": "online",
                "cidr_url": DEFAULT_CIDR_API_URL,
            })
        });
    let mode = settings
        .get("cidr_mode")
        .and_then(Value::as_str)
        .filter(|value| *value == "custom")
        .unwrap_or("online");
    let raw_url = if mode == "custom" {
        settings
            .get("cidr_url")
            .and_then(Value::as_str)
            .unwrap_or("")
    } else {
        DEFAULT_CIDR_API_URL
    };
    let base_url = normalize_cidr_api_base_url(raw_url)?;
    Ok((mode.to_string(), base_url))
}

pub(crate) fn normalize_cidr_api_base_url(value: &str) -> Result<String, String> {
    http_utils::normalize_api_base_url(value, "/api/v1")
        .map_err(|error| format!("Invalid CIDR API URL: {error}"))
}

pub(crate) fn source_fingerprint(base_url: &str) -> String {
    let digest = Sha256::digest(base_url.trim_end_matches('/').as_bytes());
    hex::encode(&digest[..8])
}

pub(crate) async fn probe_configured_capabilities(
    state: &AppState,
) -> Result<CidrCapabilities, String> {
    let (source, base_url) = configured_cidr_source(state).await?;
    probe_capabilities(state, &base_url, source).await
}

pub(crate) async fn probe_capabilities(
    state: &AppState,
    base_url: &str,
    source: impl Into<String>,
) -> Result<CidrCapabilities, String> {
    let base_url = normalize_cidr_api_base_url(base_url)?;
    let provinces = fetch_data(state, &base_url, "provinces", &[]).await?;
    let province = provinces
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("name"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "CIDR upstream response missing province data".to_string())?;

    let url = build_url(
        &base_url,
        "cidrs",
        &[
            ("province", province),
            ("city", CAPABILITY_PROBE_VALUE),
            ("operator", CAPABILITY_PROBE_VALUE),
            ("ip_version", "4"),
        ],
    )?;
    let response = state
        .fallback_client
        .get(url)
        .header(reqwest::header::USER_AGENT, CIDR_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("CIDR upstream request failed: {error}"))?;
    let status = response.status();
    let raw_body = response.text().await.unwrap_or_default();
    let payload = serde_json::from_str::<Value>(raw_body.trim_start_matches('\u{feff}'))
        .unwrap_or(Value::Null);
    let message = payload
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let supported = classify_operator_probe(status, message)?;

    Ok(CidrCapabilities {
        source: source.into(),
        operator_filtering: CidrOperatorCapability {
            supported,
            operators: CIDR_OPERATORS.to_vec(),
            minimum_container_version: CIDR_MINIMUM_OPERATOR_VERSION,
        },
    })
}

fn classify_operator_probe(status: reqwest::StatusCode, message: &str) -> Result<bool, String> {
    if status == reqwest::StatusCode::BAD_REQUEST && message.starts_with("invalid operator") {
        return Ok(true);
    }
    if status == reqwest::StatusCode::NOT_FOUND && message.starts_with("city not found") {
        return Ok(false);
    }
    Err(format!(
        "CIDR operator capability probe returned HTTP {}: {}",
        status.as_u16(),
        if message.is_empty() {
            "unexpected response"
        } else {
            message
        }
    ))
}

pub(crate) async fn fetch_data(
    state: &AppState,
    base_url: &str,
    path: &str,
    query: &[(&str, &str)],
) -> Result<Value, String> {
    let url = build_url(base_url, path, query)?;
    let response = state
        .fallback_client
        .get(url)
        .header(reqwest::header::USER_AGENT, CIDR_USER_AGENT)
        .send()
        .await
        .map_err(|error| format!("CIDR upstream request failed: {error}"))?;
    let status = response.status();
    let raw_body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(format!(
            "CIDR upstream request failed: HTTP {}",
            status.as_u16()
        ));
    }
    let payload: Value = serde_json::from_str(raw_body.trim_start_matches('\u{feff}'))
        .map_err(|error| format!("CIDR upstream returned invalid JSON: {error}"))?;
    if payload.get("code").and_then(Value::as_i64) != Some(0) {
        return Err(payload
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("CIDR upstream returned unexpected payload")
            .to_string());
    }
    payload
        .get("data")
        .cloned()
        .ok_or_else(|| "CIDR upstream response missing data".to_string())
}

pub(crate) fn validate_operator_echo(
    data: &Value,
    operator: Option<CidrOperator>,
) -> Result<(), String> {
    let Some(operator) = operator else {
        return Ok(());
    };
    if data.get("operator").and_then(Value::as_str) == Some(operator.as_str()) {
        Ok(())
    } else {
        Err("CIDR operator filtering is unsupported".to_string())
    }
}

pub(crate) fn localize_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    let text = |key: &str| translator.t(&format!("server.cidr.{key}"));
    let text_params = |key: &str, params: &[(&str, String)]| {
        translator.t_params(&format!("server.cidr.{key}"), params)
    };

    if message.is_empty() || message == "CIDR service failed" {
        return text("serviceError");
    }
    if message == "CIDR operator filtering is unsupported" {
        return text("operatorUnsupported");
    }
    if message == "province-wide CIDR selection is unavailable" {
        return text("provinceWideUnsupported");
    }
    if message.starts_with("invalid operator: ") {
        return text("operatorInvalid");
    }
    if message == "CIDR upstream response missing data"
        || message == "CIDR upstream response missing province data"
        || message.starts_with("CIDR operator capability probe returned HTTP ")
    {
        return text("upstreamUnexpected");
    }
    if let Some(detail) = message.strip_prefix("Invalid CIDR API URL: ") {
        return text_params("invalidApiUrl", &[("error", detail.to_string())]);
    }
    if let Some(status) = message.strip_prefix("CIDR upstream request failed: HTTP ") {
        return text_params("upstreamRequestFailed", &[("status", status.to_string())]);
    }
    if let Some(detail) = message.strip_prefix("CIDR upstream request failed: ") {
        return text_params(
            "upstreamRequestFailedGeneric",
            &[("error", detail.to_string())],
        );
    }
    if message.starts_with("CIDR upstream returned invalid JSON") {
        return text("invalidJson");
    }
    message.to_string()
}

fn build_url(base_url: &str, path: &str, query: &[(&str, &str)]) -> Result<Url, String> {
    let base_url = normalize_cidr_api_base_url(base_url)?;
    let mut url = Url::parse(&format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    ))
    .map_err(|error| format!("Invalid CIDR API URL: {error}"))?;
    for (key, value) in query {
        if !value.trim().is_empty() {
            url.query_pairs_mut().append_pair(key, value.trim());
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use axum::{
        Json, Router,
        extract::{Query, State},
        http::StatusCode,
        response::{IntoResponse, Response},
        routing::get,
    };

    use super::*;

    #[derive(Clone)]
    struct MockCidrState {
        supports_operator: bool,
        cidr_suffix: u8,
        lookup_requests: Arc<AtomicUsize>,
    }

    async fn mock_provinces() -> Json<Value> {
        Json(json!({
            "code": 0,
            "data": { "items": [{ "name": "浙江", "city_count": 2 }], "total": 1 }
        }))
    }

    async fn mock_cidrs(
        State(state): State<MockCidrState>,
        Query(query): Query<HashMap<String, String>>,
    ) -> Response {
        if query.get("operator").map(String::as_str) == Some(CAPABILITY_PROBE_VALUE) {
            if state.supports_operator {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "code": 400,
                        "message": format!("invalid operator: {CAPABILITY_PROBE_VALUE}")
                    })),
                )
                    .into_response();
            }
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "code": 404,
                    "message": format!("city not found: {CAPABILITY_PROBE_VALUE}")
                })),
            )
                .into_response();
        }

        state.lookup_requests.fetch_add(1, Ordering::SeqCst);
        let operator = query.get("operator").cloned();
        let mut data = json!({
            "province": query.get("province").cloned().unwrap_or_default(),
            "city": query.get("city").cloned(),
            "cidr_groups": { "4": [format!("10.{}.0.0/16", state.cidr_suffix)], "6": [] },
            "counts": { "4": 1, "6": 0 }
        });
        if state.supports_operator {
            data["operator"] = operator.map_or(Value::Null, Value::String);
        }
        Json(json!({ "code": 0, "data": data })).into_response()
    }

    async fn spawn_mock_cidr(
        supports_operator: bool,
        cidr_suffix: u8,
    ) -> (String, Arc<AtomicUsize>, tokio::task::JoinHandle<()>) {
        let requests = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/v1/provinces", get(mock_provinces))
            .route("/api/v1/cidrs", get(mock_cidrs))
            .with_state(MockCidrState {
                supports_operator,
                cidr_suffix,
                lookup_requests: requests.clone(),
            });
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (format!("http://{address}/api/v1"), requests, task)
    }

    async fn cidr_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.runtime_target = "linux".to_string();
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.go_backend_grpc_addr = "http://127.0.0.1:1".to_string();
        settings.internal_rpc_token = "test-internal-rpc-token".to_string();
        settings.request_timeout = std::time::Duration::from_secs(2);
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    async fn configure_custom_source(state: &AppState, url: &str) {
        state
            .store
            .set_json_value(
                IP_LOCATION_API_SETTINGS_KEY,
                &json!({ "cidr_mode": "custom", "cidr_url": url }),
            )
            .await
            .unwrap();
    }

    #[test]
    fn operator_parser_and_query_key_are_stable() {
        assert_eq!(
            CidrOperator::parse_optional(Some(" 移动 ")).unwrap(),
            Some(CidrOperator::Mobile)
        );
        assert!(CidrOperator::parse_optional(Some("广电")).is_err());
        assert_eq!(CidrOperator::parse_value(Some(&Value::Null)).unwrap(), None);
        assert!(CidrOperator::parse_value(Some(&json!(123))).is_err());
        assert!(CidrOperator::parse_value(Some(&json!(false))).is_err());
        assert_eq!(
            CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Telecom)).key(),
            "浙江::杭州::电信"
        );
    }

    #[test]
    fn cache_source_fingerprint_changes_with_source() {
        assert_ne!(
            source_fingerprint("https://one.example/api/v1"),
            source_fingerprint("https://two.example/api/v1")
        );
    }

    #[test]
    fn validates_operator_echo_without_affecting_legacy_queries() {
        assert!(validate_operator_echo(&json!({}), None).is_ok());
        assert!(
            validate_operator_echo(&json!({ "operator": "移动" }), Some(CidrOperator::Mobile))
                .is_ok()
        );
        assert!(validate_operator_echo(&json!({}), Some(CidrOperator::Mobile)).is_err());
    }

    #[test]
    fn distinguishes_new_and_legacy_operator_probe_responses() {
        assert!(
            classify_operator_probe(
                reqwest::StatusCode::BAD_REQUEST,
                "invalid operator: __fn_knock_operator_probe__",
            )
            .unwrap()
        );
        assert!(
            !classify_operator_probe(
                reqwest::StatusCode::NOT_FOUND,
                "city not found: __fn_knock_operator_probe__",
            )
            .unwrap()
        );
        assert!(
            classify_operator_probe(reqwest::StatusCode::BAD_GATEWAY, "upstream failed").is_err()
        );
    }

    #[tokio::test]
    async fn behavior_probe_distinguishes_new_and_legacy_containers() {
        let (_directory, state) = cidr_test_state().await;
        let (new_url, _, new_task) = spawn_mock_cidr(true, 1).await;
        let (legacy_url, _, legacy_task) = spawn_mock_cidr(false, 2).await;

        let current = probe_capabilities(&state, &new_url, "custom")
            .await
            .unwrap();
        let legacy = probe_capabilities(&state, &legacy_url, "custom")
            .await
            .unwrap();

        assert!(current.operator_filtering.supported);
        assert!(!legacy.operator_filtering.supported);
        assert_eq!(
            current.operator_filtering.minimum_container_version,
            "0.1.3"
        );
        new_task.abort();
        legacy_task.abort();
    }

    #[tokio::test]
    async fn operator_echo_validation_blocks_legacy_lookup_results() {
        let (_directory, state) = cidr_test_state().await;
        let (legacy_url, requests, task) = spawn_mock_cidr(false, 3).await;
        configure_custom_source(&state, &legacy_url).await;

        let query = CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Mobile));
        let error = lookup_region(&state, &query).await.unwrap_err();

        assert_eq!(error.to_string(), "CIDR operator filtering is unsupported");
        assert!(lookup_region(&state, &query).await.is_err());
        assert_eq!(requests.load(Ordering::SeqCst), 2);
        task.abort();
    }

    #[tokio::test]
    async fn validated_cache_is_isolated_by_source_and_operator() {
        let (_directory, state) = cidr_test_state().await;
        let (first_url, first_requests, first_task) = spawn_mock_cidr(true, 11).await;
        let (second_url, second_requests, second_task) = spawn_mock_cidr(true, 22).await;
        let mobile = CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Mobile));
        let telecom = CidrRegionQuery::new("浙江", Some("杭州"), Some(CidrOperator::Telecom));

        configure_custom_source(&state, &first_url).await;
        let first = lookup_region(&state, &mobile).await.unwrap();
        let cached = lookup_region(&state, &mobile).await.unwrap();
        let other_operator = lookup_region(&state, &telecom).await.unwrap();
        assert_eq!(first.cidrs, vec!["10.11.0.0/16"]);
        assert_eq!(cached.cidrs, first.cidrs);
        assert_eq!(
            other_operator.selection.operator,
            Some(CidrOperator::Telecom)
        );
        assert_eq!(first_requests.load(Ordering::SeqCst), 2);

        configure_custom_source(&state, &second_url).await;
        let other_source = lookup_region(&state, &mobile).await.unwrap();
        assert_eq!(other_source.cidrs, vec!["10.22.0.0/16"]);
        assert_eq!(second_requests.load(Ordering::SeqCst), 1);

        first_task.abort();
        second_task.abort();
    }
}
