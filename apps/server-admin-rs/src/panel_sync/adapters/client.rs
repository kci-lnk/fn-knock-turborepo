use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde_json::Value;
use tokio::time::sleep;
use url::Url;

use crate::panel_sync::model::PanelConnection;

const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_ATTEMPTS: usize = 3;

#[derive(Clone)]
pub struct PanelHttpClient {
    client: reqwest::Client,
    base: Url,
    api_path: String,
}

impl PanelHttpClient {
    pub fn new(connection: &PanelConnection) -> Result<Self, String> {
        let base = validate_base_url(&connection.base_url)?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .danger_accept_invalid_certs(connection.allow_invalid_tls)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "无法创建面板 HTTP 客户端".to_string())?;
        Ok(Self {
            client,
            base,
            api_path: connection.api_path.clone(),
        })
    }

    pub fn endpoint(&self, suffix: &str) -> Result<Url, String> {
        let mut url = self.base.clone();
        let root = self.api_path.split('?').next().unwrap_or("");
        let base_path = self.base.path().trim_end_matches('/');
        let api_path = root.trim_matches('/');
        let suffix = suffix.trim_matches('/');
        let path = if suffix.is_empty() {
            format!("{base_path}/{api_path}")
        } else {
            format!("{base_path}/{api_path}/{suffix}")
        };
        url.set_path(&path);
        url.set_query(self.api_path.split_once('?').map(|(_, query)| query));
        Ok(url)
    }

    pub async fn json(
        &self,
        method: Method,
        url: Url,
        headers: &[(String, String)],
        body: Option<&Value>,
        form: Option<&[(String, String)]>,
    ) -> Result<Value, String> {
        self.json_with_retry(method, url, headers, body, form, true)
            .await
    }

    /// Sends a non-idempotent mutation once. Retrying a create request after a
    /// lost response can produce duplicate panel entries, so adapters use this
    /// for providers that do not expose an idempotency key.
    pub async fn json_once(
        &self,
        method: Method,
        url: Url,
        headers: &[(String, String)],
        body: Option<&Value>,
        form: Option<&[(String, String)]>,
    ) -> Result<Value, String> {
        self.json_with_retry(method, url, headers, body, form, false)
            .await
    }

    async fn json_with_retry(
        &self,
        method: Method,
        url: Url,
        headers: &[(String, String)],
        body: Option<&Value>,
        form: Option<&[(String, String)]>,
        retry_safe: bool,
    ) -> Result<Value, String> {
        if url.origin() != self.base.origin() {
            return Err("面板请求不得跨源".to_string());
        }
        let attempts = if retry_safe { MAX_ATTEMPTS } else { 1 };
        for attempt in 0..attempts {
            let mut request = self.client.request(method.clone(), url.clone());
            for (name, value) in headers {
                request = request.header(name, value);
            }
            if let Some(body) = body {
                request = request.json(body);
            }
            if let Some(form) = form {
                let mut encoded = url::form_urlencoded::Serializer::new(String::new());
                for (name, value) in form {
                    encoded.append_pair(name, value);
                }
                request = request
                    .header(
                        reqwest::header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded",
                    )
                    .body(encoded.finish());
            }
            let response = request.send().await;
            let mut response = match response {
                Ok(value) => value,
                Err(_) if attempt + 1 < attempts => {
                    retry_delay(attempt, None).await;
                    continue;
                }
                Err(_) => return Err("无法连接面板，请检查地址、TLS 与网络".to_string()),
            };
            let status = response.status();
            if status.is_redirection() {
                return Err("面板返回了重定向；为防止凭据泄露，请直接填写最终 API 地址".to_string());
            }
            if (status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error())
                && attempt + 1 < attempts
            {
                let retry_after = response
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|value| value.to_str().ok())
                    .and_then(parse_retry_after);
                retry_delay(attempt, retry_after).await;
                continue;
            }
            let mut bytes = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|_| "读取面板响应失败".to_string())?
            {
                if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                    return Err("面板响应超过 1 MiB 安全上限".to_string());
                }
                bytes.extend_from_slice(&chunk);
            }
            if !status.is_success() {
                return Err(format!("面板返回 HTTP {}", status.as_u16()));
            }
            if bytes.is_empty() {
                return Ok(Value::Null);
            }
            return serde_json::from_slice(&bytes)
                .map_err(|_| "面板返回的不是有效 JSON".to_string());
        }
        Err("面板请求重试耗尽".to_string())
    }
}

fn validate_base_url(value: &str) -> Result<Url, String> {
    let url = Url::parse(value.trim()).map_err(|_| "Base URL 无效".to_string())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Base URL 仅支持 HTTP 或 HTTPS".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("Base URL 不允许包含用户名或密码".to_string());
    }
    if url.host_str().is_none() || url.fragment().is_some() || url.query().is_some() {
        return Err("Base URL 必须包含主机，且不能包含查询参数或片段".to_string());
    }
    Ok(url)
}

async fn retry_delay(attempt: usize, retry_after: Option<u64>) {
    let base = retry_after.unwrap_or(1_u64 << attempt.min(3)).min(30);
    let jitter = u64::from(rand::random::<u8>() % 250);
    sleep(Duration::from_millis(base * 1000 + jitter)).await;
}

fn parse_retry_after(value: &str) -> Option<u64> {
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(seconds);
    }
    httpdate::parse_http_date(value)
        .ok()?
        .duration_since(std::time::SystemTime::now())
        .ok()
        .map(|duration| duration.as_secs().max(1))
}

pub fn ensure_api_success(value: &Value) -> Result<(), String> {
    if value.get("success").and_then(Value::as_bool) == Some(false)
        || value
            .get("code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0 && code != 200)
    {
        return Err("面板 API 拒绝了请求".to_string());
    }
    Ok(())
}

pub fn response_data(value: &Value) -> &Value {
    value.get("data").unwrap_or(value)
}

pub fn response_id(value: &Value) -> Option<String> {
    let value = response_data(value);
    [
        "id",
        "ID",
        "itemGroupID",
        "category_id",
        "catelogId",
        "tool_id",
    ]
    .into_iter()
    .find_map(|key| value.get(key))
    .or_else(|| value.as_i64().map(|_| value))
    .and_then(|value| {
        value
            .as_str()
            .map(str::to_string)
            .or_else(|| value.as_i64().map(|value| value.to_string()))
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{PanelHttpClient, parse_retry_after, validate_base_url};
    use crate::panel_sync::model::*;

    #[test]
    fn retry_after_accepts_seconds_and_http_dates() {
        assert_eq!(parse_retry_after("7"), Some(7));
        let value = httpdate::fmt_http_date(SystemTime::now() + Duration::from_secs(5));
        assert!(parse_retry_after(&value).is_some_and(|seconds| (1..=5).contains(&seconds)));
    }

    #[test]
    fn panel_urls_reject_credentials_and_unsupported_schemes() {
        assert!(validate_base_url("https://panel.example.test").is_ok());
        assert!(validate_base_url("ftp://panel.example.test").is_err());
        assert!(validate_base_url("https://token@panel.example.test").is_err());
        assert!(validate_base_url("https://panel.example.test?token=secret").is_err());
    }

    #[test]
    fn endpoint_preserves_reverse_proxy_base_paths_and_api_queries() {
        let connection = PanelConnection {
            id: "connection".to_string(),
            name: "OneNav".to_string(),
            provider: PanelProvider::OneNav,
            base_url: "https://panel.example.test/reverse-proxy".to_string(),
            api_path: "/index.php?c=api".to_string(),
            allow_invalid_tls: false,
            grouping: GroupingConfig::default(),
            auto_sync: AutoSyncConfig::default(),
            credential_configured: true,
            verified_at: None,
            verified_version: None,
            created_at: String::new(),
            updated_at: String::new(),
            last_run: None,
            next_sync_at: None,
        };
        let endpoint = PanelHttpClient::new(&connection)
            .unwrap()
            .endpoint("")
            .unwrap();
        assert_eq!(endpoint.path(), "/reverse-proxy/index.php");
        assert_eq!(endpoint.query(), Some("c=api"));
    }
}
