use std::time::Duration;

use anyhow::Context;
use axum::http::Method;
use reqwest::Url;
use serde::Serialize;
use serde_json::{Value, json};

#[allow(dead_code)]
#[derive(Clone)]
pub struct GoBackendClient {
    base_url: Url,
    client: reqwest::Client,
}

#[allow(dead_code)]
impl GoBackendClient {
    pub fn new(base_url: String, timeout: Duration) -> anyhow::Result<Self> {
        let base_url = Url::parse(base_url.trim_end_matches('/'))
            .with_context(|| format!("invalid GO_BACKEND_BASE_URL: {base_url}"))?;
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .context("build go backend http client")?;
        Ok(Self { base_url, client })
    }

    pub async fn request_json<T: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&T>,
    ) -> anyhow::Result<Value> {
        let url = self.url(path)?;
        let method = reqwest::Method::from_bytes(method.as_str().as_bytes())
            .context("convert http method for go backend request")?;
        let mut request = self.client.request(method, url);
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await.context("send go backend request")?;
        let status = response.status();
        let value = response.json::<Value>().await.with_context(|| {
            format!("decode go backend JSON response from {path}, status {status}")
        })?;
        if !status.is_success() {
            anyhow::bail!("go backend request failed: {path} returned {status}: {value}");
        }
        Ok(value)
    }

    pub async fn request_json_with_status<T: Serialize + ?Sized>(
        &self,
        method: Method,
        path: &str,
        body: Option<&T>,
    ) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        let url = self.url(path)?;
        let method = reqwest::Method::from_bytes(method.as_str().as_bytes())
            .context("convert http method for go backend request")?;
        let mut request = self.client.request(method, url);
        if let Some(body) = body {
            request = request.json(body);
        }

        let response = request.send().await.context("send go backend request")?;
        let status = response.status();
        let text = response.text().await.with_context(|| {
            format!("read go backend response body from {path}, status {status}")
        })?;
        let value = if text.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str::<Value>(&text).unwrap_or_else(|_| {
                json!({
                    "success": false,
                    "code": status.as_u16(),
                    "message": text
                })
            })
        };
        Ok((status, value))
    }

    pub async fn allow_ip(&self, ip: &str) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/iptables/allow",
            Some(&json!({ "ip": ip })),
        )
        .await
    }

    pub async fn remove_ip(&self, ip: &str) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/iptables/remove",
            Some(&json!({ "ip": ip })),
        )
        .await
    }

    pub async fn set_reverse_proxy_throttle_exempt_ips(
        &self,
        runtime: &Value,
    ) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/runtime/reverse-proxy-throttle-exempt-ips",
            Some(runtime),
        )
        .await
    }

    pub async fn set_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/rules", Some(rules))
            .await
    }

    pub async fn set_host_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/host-rules", Some(rules))
            .await
    }

    pub async fn set_stream_rules(&self, rules: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/stream-rules", Some(rules))
            .await
    }

    pub async fn flush_rules(&self) -> anyhow::Result<Value> {
        self.request_json(Method::DELETE, "/api/rules", Option::<&Value>::None)
            .await
    }

    pub async fn flush_host_rules(&self) -> anyhow::Result<Value> {
        self.request_json(Method::DELETE, "/api/host-rules", Option::<&Value>::None)
            .await
    }

    pub async fn flush_stream_rules(&self) -> anyhow::Result<Value> {
        self.request_json(Method::DELETE, "/api/stream-rules", Option::<&Value>::None)
            .await
    }

    pub async fn set_auth_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/auth", Some(config))
            .await
    }

    pub async fn get_proxy_protocol_force(&self) -> anyhow::Result<Value> {
        self.request_json(
            Method::GET,
            "/api/config/proxy-protocol",
            Option::<&Value>::None,
        )
        .await
    }

    pub async fn set_proxy_protocol_force(&self, force: bool) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/config/proxy-protocol",
            Some(&json!({ "proxy_protocol_force": force })),
        )
        .await
    }

    pub async fn set_reverse_proxy_throttle(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/config/reverse-proxy-throttle",
            Some(config),
        )
        .await
    }

    pub async fn set_gateway_visibility(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/config/visibility", Some(config))
            .await
    }

    pub async fn set_forwarded_headers_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/config/forwarded-headers", Some(config))
            .await
    }

    pub async fn set_preserve_host_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/config/preserve-host", Some(config))
            .await
    }

    pub async fn set_crawler_blocker_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/config/crawler-blocker", Some(config))
            .await
    }

    pub async fn set_gateway_portal_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/config/portal", Some(config))
            .await
    }

    pub async fn set_gateway_logging_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/logging", Some(config))
            .await
    }

    pub async fn set_fnos_port_icon_hijack_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/config/fnos-port-icon-hijack",
            Some(config),
        )
        .await
    }

    pub async fn set_common_location_exemptions(
        &self,
        runtime: &Value,
    ) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        self.request_json_with_status(
            Method::POST,
            "/api/runtime/common-location-exemptions",
            Some(runtime),
        )
        .await
    }

    pub async fn set_locale_config(
        &self,
        config: &Value,
    ) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        self.request_json_with_status(Method::POST, "/api/config/locale", Some(config))
            .await
    }

    pub async fn set_default_route(
        &self,
        route: &str,
    ) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        self.request_json_with_status(
            Method::POST,
            "/api/config/default-route",
            Some(&json!({ "default_route": route })),
        )
        .await
    }

    pub async fn get_server_info(&self) -> anyhow::Result<Value> {
        self.request_json(Method::GET, "/api/info", Option::<&Value>::None)
            .await
    }

    pub async fn set_waf_config(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/waf/config", Some(config))
            .await
    }

    pub async fn reload_waf_rules(&self, config: &Value) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/waf/reload",
            Some(&json!({ "config": config })),
        )
        .await
    }

    pub async fn drain_waf_events(&self, limit: i64) -> anyhow::Result<Value> {
        self.request_json(
            Method::POST,
            "/api/waf/events/drain",
            Some(&json!({ "limit": limit })),
        )
        .await
    }

    pub async fn sync_ssh_firewall(&self, payload: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/iptables/ssh/sync", Some(payload))
            .await
    }

    pub async fn init_iptables(&self, payload: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/iptables/init", Some(payload))
            .await
    }

    pub async fn clean_iptables(&self) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/iptables/clean", Option::<&Value>::None)
            .await
    }

    pub async fn clear_tcp_redirect(
        &self,
        listen_port: i64,
        target_port: i64,
    ) -> anyhow::Result<(reqwest::StatusCode, Value)> {
        self.request_json_with_status(
            Method::DELETE,
            "/api/iptables/tcp-redirect",
            Some(&json!({
                "listen_port": listen_port,
                "target_port": target_port,
            })),
        )
        .await
    }

    pub async fn clear_ssh_firewall(&self, payload: &Value) -> anyhow::Result<Value> {
        self.request_json(Method::POST, "/api/iptables/ssh/clear", Some(payload))
            .await
    }

    fn url(&self, path: &str) -> anyhow::Result<Url> {
        let normalized = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("/{path}")
        };
        self.base_url
            .join(normalized.trim_start_matches('/'))
            .with_context(|| format!("join go backend URL path: {path}"))
    }
}
