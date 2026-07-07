use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};
use url::Url;

use crate::{i18n::Translator, response, state::AppState};

const IP_LOCATION_API_SETTINGS_KEY: &str = "fn_knock:ip-location-api:settings";
const DEFAULT_IP_LOOKUP_URL: &str = "https://ipaddress.fnknock.cn/api/v1";
const DEFAULT_CIDR_URL: &str = "https://cidr.fnknock.cn/api/v1";
const USER_AGENT: &str = "fn-knock-server-admin/1.0";

fn admin_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

fn admin_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

#[derive(Deserialize)]
struct IpLocationApiSettingsBody {
    ip_lookup_mode: String,
    ip_lookup_url: String,
    cidr_mode: String,
    cidr_url: String,
}

#[derive(Deserialize)]
struct TestUrlBody {
    url: String,
}

pub fn ip_location_config_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/ip_location_api",
            get(get_settings).post(update_settings),
        )
        .route(
            "/api/admin/config/ip_location_api/test-ip-lookup",
            post(test_ip_lookup),
        )
        .route(
            "/api/admin/config/ip_location_api/test-cidr",
            post(test_cidr),
        )
}

async fn get_settings(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .redis
        .get_json_value(IP_LOCATION_API_SETTINGS_KEY)
        .await
    {
        Ok(raw) => response::ok(normalize_ip_location_api_config(raw.as_ref())).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to load IP location API settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "ipLocation.loadSettingsFailed"),
            )
        }
    }
}

async fn update_settings(
    State(state): State<AppState>,
    Json(body): Json<IpLocationApiSettingsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let settings = match normalize_settings_body(body, &translator) {
        Ok(settings) => settings,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    match state
        .redis
        .set_json_value(IP_LOCATION_API_SETTINGS_KEY, &settings)
        .await
    {
        Ok(()) => response::ok(settings).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to save IP location API settings");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                admin_text(&translator, "ipLocation.saveSettingsFailed"),
            )
        }
    }
}

async fn test_ip_lookup(State(state): State<AppState>, Json(body): Json<TestUrlBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    let base_url = match validate_base_url(&body.url, "URL", &translator) {
        Ok(url) => url,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let result = async {
        let mut url = build_ip_location_api_url(&base_url, "ip/lookup")?;
        url.query_pairs_mut().append_pair("ip", "8.8.8.8");
        let payload = fetch_test_payload(&state, &translator, url).await?;
        if payload.get("success").and_then(Value::as_bool) == Some(false) {
            return Ok(payload);
        }
        if payload.get("code").and_then(Value::as_i64) != Some(0)
            || payload.get("result").is_none_or(Value::is_null)
        {
            return Ok(test_failure(
                payload
                    .get("msg")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| admin_text(&translator, "connectionTest.invalidData")),
            ));
        }
        Ok(test_success(&translator))
    }
    .await;
    test_result_response(result, &translator)
}

async fn test_cidr(State(state): State<AppState>, Json(body): Json<TestUrlBody>) -> Response {
    let translator = Translator::from_state(&state).await;
    let base_url = match validate_base_url(&body.url, "URL", &translator) {
        Ok(url) => url,
        Err(message) => return response::error(StatusCode::BAD_REQUEST, message),
    };
    let result = async {
        let url = build_ip_location_api_url(&base_url, "provinces")?;
        let payload = fetch_test_payload(&state, &translator, url).await?;
        if payload.get("success").and_then(Value::as_bool) == Some(false) {
            return Ok(payload);
        }
        if payload.get("code").and_then(Value::as_i64) != Some(0)
            || payload.get("data").is_none_or(Value::is_null)
        {
            return Ok(test_failure(
                payload
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| admin_text(&translator, "connectionTest.invalidData")),
            ));
        }
        Ok(test_success(&translator))
    }
    .await;
    test_result_response(result, &translator)
}

async fn fetch_test_payload(
    state: &AppState,
    translator: &Translator,
    url: Url,
) -> Result<Value, String> {
    let response = state
        .fallback_client
        .get(url)
        .header(reqwest::header::USER_AGENT, USER_AGENT)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Ok(test_failure(admin_text_params(
            translator,
            "connectionTest.httpStatus",
            &[("status", status.as_u16().to_string())],
        )));
    }
    response
        .json::<Value>()
        .await
        .map_err(|_| admin_text(translator, "connectionTest.invalidData"))
}

fn test_result_response(result: Result<Value, String>, translator: &Translator) -> Response {
    match result {
        Ok(payload) => Json(payload).into_response(),
        Err(message) => Json(test_failure(localize_connection_test_error(
            &message, translator,
        )))
        .into_response(),
    }
}

fn test_success(translator: &Translator) -> Value {
    json!({ "success": true, "message": admin_text(translator, "connectionTest.success") })
}

fn test_failure(message: impl Into<String>) -> Value {
    json!({ "success": false, "message": message.into() })
}

fn localize_connection_test_error(message: &str, translator: &Translator) -> String {
    match message.trim() {
        "connectionTest.invalidData" | "Invalid response data" => {
            admin_text(translator, "connectionTest.invalidData")
        }
        value if value.is_empty() => admin_text(translator, "connectionTest.failed"),
        value => value.to_string(),
    }
}

fn normalize_settings_body(
    body: IpLocationApiSettingsBody,
    translator: &Translator,
) -> Result<Value, String> {
    let ip_lookup_mode = normalize_mode(&body.ip_lookup_mode, translator)?;
    let cidr_mode = normalize_mode(&body.cidr_mode, translator)?;
    let ip_lookup_url = if ip_lookup_mode == "custom" {
        validate_base_url(
            &body.ip_lookup_url,
            &admin_text(translator, "ipLocation.ipLookupUrlLabel"),
            translator,
        )?
    } else {
        DEFAULT_IP_LOOKUP_URL.to_string()
    };
    let cidr_url = if cidr_mode == "custom" {
        validate_base_url(
            &body.cidr_url,
            &admin_text(translator, "ipLocation.cidrUrlLabel"),
            translator,
        )?
    } else {
        DEFAULT_CIDR_URL.to_string()
    };
    Ok(json!({
        "ip_lookup_mode": ip_lookup_mode,
        "ip_lookup_url": ip_lookup_url,
        "cidr_mode": cidr_mode,
        "cidr_url": cidr_url,
    }))
}

fn normalize_ip_location_api_config(raw: Option<&Value>) -> Value {
    let ip_lookup_mode = mode_from_raw(raw.and_then(|value| value.get("ip_lookup_mode")));
    let cidr_mode = mode_from_raw(raw.and_then(|value| value.get("cidr_mode")));
    let ip_lookup_url = if ip_lookup_mode == "custom" {
        raw.and_then(|value| value.get("ip_lookup_url"))
            .and_then(Value::as_str)
            .map(normalize_service_url)
            .unwrap_or_default()
    } else {
        DEFAULT_IP_LOOKUP_URL.to_string()
    };
    let cidr_url = if cidr_mode == "custom" {
        raw.and_then(|value| value.get("cidr_url"))
            .and_then(Value::as_str)
            .map(normalize_service_url)
            .unwrap_or_default()
    } else {
        DEFAULT_CIDR_URL.to_string()
    };
    json!({
        "ip_lookup_mode": ip_lookup_mode,
        "ip_lookup_url": ip_lookup_url,
        "cidr_mode": cidr_mode,
        "cidr_url": cidr_url,
    })
}

fn normalize_mode(value: &str, translator: &Translator) -> Result<&'static str, String> {
    match value.trim() {
        "online" => Ok("online"),
        "custom" => Ok("custom"),
        _ => Err(admin_text(translator, "ipLocation.modeInvalid")),
    }
}

fn mode_from_raw(value: Option<&Value>) -> &'static str {
    match value.and_then(Value::as_str) {
        Some("custom") => "custom",
        _ => "online",
    }
}

fn validate_base_url(value: &str, label: &str, translator: &Translator) -> Result<String, String> {
    let url = normalize_service_url(value);
    if url.is_empty() {
        return Err(admin_text_params(
            translator,
            "validation.required",
            &[("label", label.to_string())],
        ));
    }
    let parsed = Url::parse(&url).map_err(|_| {
        admin_text_params(
            translator,
            "validation.invalidFormat",
            &[("label", label.to_string())],
        )
    })?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(admin_text_params(
            translator,
            "validation.httpUrlRequired",
            &[("label", label.to_string())],
        ));
    }
    Ok(url)
}

fn normalize_service_url(value: &str) -> String {
    value.trim().trim_end_matches('/').to_string()
}

fn build_ip_location_api_url(base_url: &str, path: &str) -> Result<Url, String> {
    let api_base = resolve_ip_location_api_base_url(base_url)?;
    Url::parse(&format!("{api_base}/{}", path.trim_start_matches('/')))
        .map_err(|error| error.to_string())
}

fn resolve_ip_location_api_base_url(value: &str) -> Result<String, String> {
    crate::http_utils::normalize_api_base_url(value, "/api/v1")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_ip_location_api_config_defaults() {
        let config = normalize_ip_location_api_config(Some(&json!({
            "ip_lookup_mode": "online",
            "ip_lookup_url": "https://custom.example",
            "cidr_mode": "custom",
            "cidr_url": "https://cidr.example///"
        })));

        assert_eq!(config["ip_lookup_url"], DEFAULT_IP_LOOKUP_URL);
        assert_eq!(config["cidr_url"], "https://cidr.example");
    }

    #[test]
    fn validates_base_urls() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            validate_base_url(" https://example.test/api/ ", "URL", &translator).unwrap(),
            "https://example.test/api"
        );
        assert_eq!(
            validate_base_url("ftp://example.test", "URL", &translator).unwrap_err(),
            "URL必须以 http:// 或 https:// 开头"
        );
        assert_eq!(
            validate_base_url("", "URL", &translator).unwrap_err(),
            "URL不能为空"
        );
    }

    #[test]
    fn localizes_ip_location_config_messages() {
        let translator = Translator::new("zh-CN");
        assert_eq!(
            admin_text(&translator, "ipLocation.loadSettingsFailed"),
            "读取 IP 属地 API 配置失败"
        );
        assert_eq!(
            normalize_mode("bad", &translator).unwrap_err(),
            "模式必须是 online 或 custom"
        );
        assert_eq!(
            test_success(&translator)["message"].as_str(),
            Some("连接成功")
        );
        assert_eq!(
            localize_connection_test_error("Invalid response data", &translator),
            "服务返回数据异常"
        );
    }

    #[test]
    fn builds_ip_location_api_urls_with_default_api_path() {
        assert_eq!(
            build_ip_location_api_url("https://example.test", "ip/lookup")
                .unwrap()
                .as_str(),
            "https://example.test/api/v1/ip/lookup"
        );
        assert_eq!(
            build_ip_location_api_url("https://example.test/custom/", "/provinces")
                .unwrap()
                .as_str(),
            "https://example.test/custom/provinces"
        );
    }
}
