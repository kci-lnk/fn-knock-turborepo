use std::{env, sync::Arc};

use axum::{
    Router,
    http::{HeaderMap, HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

use crate::{i18n::Translator, runtime_profile, state::AppState, time_utils};

const AUTO_HTTPS_LISTEN_PORT: u16 = 80;
const AUTO_HTTPS_DUAL_STACK_LISTEN_HOST: &str = "::";
const AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST: &str = "0.0.0.0";
const LISTEN_EACCES_ERROR: &str = "Permission denied while binding HTTP redirect port";
const LISTEN_EADDRINUSE_ERROR: &str = "HTTP redirect port is already in use";
const LISTEN_FAILED_ERROR: &str = "Failed to start HTTP redirect server";
const LISTEN_FAILED_WITH_MESSAGE_PREFIX: &str = "Failed to start HTTP redirect server: ";

#[derive(Clone)]
pub struct AutoHttpsRedirectManager {
    inner: Arc<Mutex<AutoHttpsInner>>,
    listen_port: u16,
    configured_listen_host: String,
}

struct AutoHttpsInner {
    state: Value,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
}

#[derive(Clone)]
struct AutoHttpsListenTarget {
    host: String,
}

impl AutoHttpsRedirectManager {
    pub fn new() -> Self {
        let configured_listen_host = env::var("FN_KNOCK_AUTO_HTTPS_HOST")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_default();
        let default_host = if configured_listen_host.is_empty() {
            AUTO_HTTPS_DUAL_STACK_LISTEN_HOST.to_string()
        } else {
            configured_listen_host.clone()
        };
        Self {
            inner: Arc::new(Mutex::new(AutoHttpsInner {
                state: build_state_value(false, false, None, &default_host, AUTO_HTTPS_LISTEN_PORT),
                shutdown: None,
                task: None,
            })),
            listen_port: AUTO_HTTPS_LISTEN_PORT,
            configured_listen_host,
        }
    }

    pub async fn runtime_state(&self) -> Value {
        self.inner.lock().await.state.clone()
    }

    pub async fn apply_config(&self, enabled: bool) -> Value {
        let mut inner = self.inner.lock().await;
        if !enabled {
            self.stop_server_locked(&mut inner).await;
            inner.state = self.build_state(false, false, None, &self.default_listen_host());
            return inner.state.clone();
        }

        if inner.shutdown.is_some() {
            let host = inner
                .state
                .get("listen_host")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| self.default_listen_host())
                .to_string();
            inner.state = self.build_state(true, true, None, &host);
            return inner.state.clone();
        }

        self.stop_server_locked(&mut inner).await;
        let mut last_error = None;
        for target in self.listen_targets() {
            match TcpListener::bind((target.host.as_str(), self.listen_port)).await {
                Ok(listener) => {
                    let listen_host = listener
                        .local_addr()
                        .ok()
                        .map(|addr| addr.ip().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| target.host.clone());
                    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();
                    inner.shutdown = Some(shutdown_tx);
                    inner.task = Some(tokio::spawn(serve_redirect(listener, shutdown_rx)));
                    inner.state = self.build_state(true, true, None, &listen_host);
                    return inner.state.clone();
                }
                Err(error) => {
                    let should_fallback = self.configured_listen_host.is_empty()
                        && target.host == AUTO_HTTPS_DUAL_STACK_LISTEN_HOST
                        && matches!(
                            error.kind(),
                            std::io::ErrorKind::AddrNotAvailable | std::io::ErrorKind::Unsupported
                        );
                    let message = normalize_listen_error(&error);
                    last_error = Some(message.clone());
                    if should_fallback {
                        continue;
                    }
                    inner.state = self.build_error_state(&message, &target.host);
                    return inner.state.clone();
                }
            }
        }

        let message = last_error.unwrap_or_else(|| "Failed to start HTTP redirect server".into());
        inner.state = self.build_error_state(&message, &self.default_listen_host());
        inner.state.clone()
    }

    async fn stop_server_locked(&self, inner: &mut AutoHttpsInner) {
        if let Some(shutdown) = inner.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(task) = inner.task.take() {
            let _ = task.await;
        }
    }

    fn build_state(
        &self,
        enabled: bool,
        active: bool,
        error: Option<&str>,
        listen_host: &str,
    ) -> Value {
        build_state_value(enabled, active, error, listen_host, self.listen_port)
    }

    fn build_error_state(&self, error: &str, listen_host: &str) -> Value {
        self.build_state(false, false, Some(error), listen_host)
    }

    fn listen_targets(&self) -> Vec<AutoHttpsListenTarget> {
        let host = self.configured_listen_host.trim();
        if !host.is_empty() {
            return vec![AutoHttpsListenTarget {
                host: host.to_string(),
            }];
        }
        vec![
            AutoHttpsListenTarget {
                host: AUTO_HTTPS_DUAL_STACK_LISTEN_HOST.to_string(),
            },
            AutoHttpsListenTarget {
                host: AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST.to_string(),
            },
        ]
    }

    fn default_listen_host(&self) -> String {
        self.listen_targets()
            .first()
            .map(|target| target.host.clone())
            .unwrap_or_else(|| AUTO_HTTPS_FALLBACK_IPV4_LISTEN_HOST.to_string())
    }
}

fn build_state_value(
    enabled: bool,
    active: bool,
    error: Option<&str>,
    listen_host: &str,
    listen_port: u16,
) -> Value {
    let status = if error.is_some() {
        "error"
    } else if !enabled {
        "disabled"
    } else if active {
        "active"
    } else {
        "error"
    };

    json!({
        "enabled": enabled,
        "active": active,
        "status": status,
        "listen_host": listen_host,
        "listen_port": listen_port,
        "redirect_scheme": "https",
        "last_error": error.map(|value| Value::String(value.to_string())).unwrap_or(Value::Null),
        "last_error_at": error.map(|_| Value::String(time_utils::now_iso())).unwrap_or(Value::Null),
        "updated_at": time_utils::now_iso(),
    })
}

impl Default for AutoHttpsRedirectManager {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn sync_auto_https_on_boot(state: AppState) {
    if matches!(
        runtime_profile::deployment_target(&state).as_str(),
        "docker" | "openwrt"
    ) {
        let _ = state.auto_https.apply_config(false).await;
        return;
    }
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to read config for auto HTTPS boot sync");
            return;
        }
    };
    let auto_https = normalize_auto_https_config(config.get("auto_https"));
    let runtime = state
        .auto_https
        .apply_config(auto_https["enabled"].as_bool().unwrap_or(false))
        .await;
    if runtime.get("status").and_then(Value::as_str) == Some("error") {
        let mut next_config = config;
        if let Some(object) = next_config.as_object_mut() {
            object.insert("auto_https".to_string(), json!({ "enabled": false }));
        }
        if let Err(error) = state.store.save_config(&next_config).await {
            tracing::warn!(%error, "failed to disable auto HTTPS after boot sync error");
        }
    }
}

pub fn normalize_auto_https_config(value: Option<&Value>) -> Value {
    json!({
        "enabled": value
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    })
}

pub fn localize_runtime_state(mut runtime: Value, translator: &Translator) -> Value {
    let Some(object) = runtime.as_object_mut() else {
        return runtime;
    };
    let Some(message) = object
        .get("last_error")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return runtime;
    };
    object.insert(
        "last_error".to_string(),
        Value::String(localize_listen_error_message(&message, translator)),
    );
    runtime
}

fn normalize_listen_error(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => LISTEN_EACCES_ERROR.to_string(),
        std::io::ErrorKind::AddrInUse => LISTEN_EADDRINUSE_ERROR.to_string(),
        _ => format!("{LISTEN_FAILED_WITH_MESSAGE_PREFIX}{error}"),
    }
}

fn localize_listen_error_message(message: &str, translator: &Translator) -> String {
    match message {
        LISTEN_EACCES_ERROR => translator.t("server.autoHttps.listenEacces"),
        LISTEN_EADDRINUSE_ERROR => translator.t("server.autoHttps.listenEaddrinuse"),
        LISTEN_FAILED_ERROR => translator.t("server.autoHttps.listenFailed"),
        value => value
            .strip_prefix(LISTEN_FAILED_WITH_MESSAGE_PREFIX)
            .map(|detail| {
                translator.t_params(
                    "server.autoHttps.listenFailedWithMessage",
                    &[("message", detail.to_string())],
                )
            })
            .unwrap_or_else(|| value.to_string()),
    }
}

async fn serve_redirect(listener: TcpListener, shutdown_rx: tokio::sync::oneshot::Receiver<()>) {
    let app = Router::new().fallback(redirect);
    if let Err(error) = axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .await
    {
        tracing::warn!(%error, "auto HTTPS redirect server stopped with error");
    }
}

async fn redirect(headers: HeaderMap, uri: Uri) -> Response {
    let host = headers
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(normalize_request_host)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let location = format!("https://{host}{path}");
    let mut response = StatusCode::PERMANENT_REDIRECT.into_response();
    if let Ok(value) = HeaderValue::from_str(&location) {
        response.headers_mut().insert(header::LOCATION, value);
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::CONNECTION, HeaderValue::from_static("close"));
    response
}

fn normalize_request_host(value: &str) -> String {
    let host = value.replace(['\r', '\n'], "").trim().to_string();
    strip_default_http_port(&host)
}

fn strip_default_http_port(host: &str) -> String {
    if host.starts_with('[') && host.ends_with("]:80") {
        return host[..host.len() - 3].to_string();
    }
    if !host.starts_with('[') && host.ends_with(":80") {
        return host[..host.len() - 3].to_string();
    }
    host.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_auto_https_config_like_node() {
        assert_eq!(
            normalize_auto_https_config(Some(&json!({ "enabled": true }))),
            json!({ "enabled": true })
        );
        assert_eq!(
            normalize_auto_https_config(Some(&json!({ "enabled": "true" }))),
            json!({ "enabled": false })
        );
    }

    #[test]
    fn strips_default_http_port_like_node() {
        assert_eq!(normalize_request_host("example.com:80"), "example.com");
        assert_eq!(normalize_request_host("[::1]:80"), "[::1]");
        assert_eq!(
            normalize_request_host("example.com:8080"),
            "example.com:8080"
        );
    }

    #[test]
    fn localizes_auto_https_runtime_errors_like_node() {
        let translator = Translator::new("zh-CN");
        let runtime = localize_runtime_state(
            build_state_value(
                false,
                false,
                Some(LISTEN_EADDRINUSE_ERROR),
                AUTO_HTTPS_DUAL_STACK_LISTEN_HOST,
                AUTO_HTTPS_LISTEN_PORT,
            ),
            &translator,
        );
        assert_eq!(runtime.get("status").and_then(Value::as_str), Some("error"));
        assert_eq!(
            runtime.get("last_error").and_then(Value::as_str),
            Some(
                "80 端口已被其他程序占用，自动 HTTPS 无法启动。请尝试飞牛系统设置，安全性，端口设置，编辑，取消勾选：重定向 80 与 443 端口"
            )
        );

        let runtime = localize_runtime_state(
            build_state_value(
                false,
                false,
                Some("Failed to start HTTP redirect server: boom"),
                AUTO_HTTPS_DUAL_STACK_LISTEN_HOST,
                AUTO_HTTPS_LISTEN_PORT,
            ),
            &translator,
        );
        assert_eq!(
            runtime.get("last_error").and_then(Value::as_str),
            Some("监听 80 端口失败：boom")
        );
    }
}
