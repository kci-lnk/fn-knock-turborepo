use super::*;

/// Stored only in DDNS settings. Never serialize this type directly in an API response.
#[derive(Clone, Default, serde::Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(super) struct DDNSUpdateProxy {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for DDNSUpdateProxy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DDNSUpdateProxy")
            .field("enabled", &self.enabled)
            .finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DDNSUpdateProxyUpdate {
    enabled: Option<bool>,
    url: Option<String>,
    username: Option<String>,
    password: Option<String>,
    clear_password: Option<bool>,
}

impl DDNSUpdateProxy {
    pub fn from_settings(settings: &Value) -> Self {
        let Some(value) = settings.get("updateProxy").filter(|value| !value.is_null()) else {
            return Self::default();
        };
        // Corrupt stored proxy data must never silently switch inherited targets to direct.
        serde_json::from_value(value.clone()).unwrap_or_else(|_| Self {
            enabled: true,
            ..Self::default()
        })
    }

    pub fn public_value(&self) -> Value {
        json!({ "enabled": self.enabled, "url": self.url, "username": self.username,
            "passwordConfigured": !self.password.is_empty() })
    }

    pub fn validate(&mut self, required: bool) -> anyhow::Result<()> {
        self.url = self.url.trim().to_string();
        if self.url.is_empty() {
            anyhow::ensure!(!required, "DDNS update proxy missing");
            return Ok(());
        }
        let invalid = || anyhow::anyhow!("Invalid DDNS update proxy");
        let mut url = Url::parse(&self.url).map_err(|_| invalid())?;
        if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
            || url.host_str().is_none_or(str::is_empty)
            || url.port() == Some(0)
            || (url.scheme().starts_with("socks") && url.port().is_none())
            || !url.username().is_empty()
            || url.password().is_some()
            || !matches!(url.path(), "" | "/")
            || url.query().is_some()
            || url.fragment().is_some()
            || self.url.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        if url.scheme() == "socks5" {
            url.set_scheme("socks5h").map_err(|_| invalid())?;
        }
        // Common authentication subset supported by both HTTP Basic and SOCKS5.
        if self.username.contains([':', '\r', '\n', '\0'])
            || self.password.contains(['\r', '\n', '\0'])
            || (!self.password.is_empty() && self.username.is_empty())
            || (url.scheme() == "socks5h"
                && (self.username.len() > 255 || self.password.len() > 255))
        {
            return Err(invalid());
        }
        self.url = url.to_string();
        Ok(())
    }

    pub fn merge(&self, update: DDNSUpdateProxyUpdate) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !(update.clear_password == Some(true)
                && update.password.as_ref().is_some_and(|v| !v.is_empty())),
            "Invalid DDNS update proxy"
        );
        let mut next = self.clone();
        if let Some(value) = update.enabled {
            next.enabled = value;
        }
        if let Some(value) = update.url {
            next.url = value;
        }
        if let Some(value) = update.username {
            next.username = value;
        }
        if update.clear_password == Some(true) {
            next.password.clear();
        } else if let Some(value) = update.password.filter(|v| !v.is_empty()) {
            next.password = value;
        }
        next.validate(next.enabled)?;
        Ok(next)
    }

    pub fn reqwest_proxy(&self) -> anyhow::Result<reqwest::Proxy> {
        let mut url = Url::parse(&self.url)?;
        // URL userinfo is also used by reqwest for SOCKS authentication.
        if !self.username.is_empty() {
            url.set_username(&self.username)
                .map_err(|_| anyhow::anyhow!("Invalid DDNS update proxy"))?;
            url.set_password(Some(&self.password))
                .map_err(|_| anyhow::anyhow!("Invalid DDNS update proxy"))?;
        }
        Ok(reqwest::Proxy::all(url)?)
    }

    pub fn curl_config(&self) -> anyhow::Result<String> {
        fn quote(value: &str) -> String {
            format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
        }
        // URL serialization removes default ports; curl otherwise defaults HTTP proxies
        // to 1080, unlike reqwest. Always give curl the effective port explicitly.
        let invalid = || anyhow::anyhow!("Invalid DDNS update proxy");
        let url = Url::parse(&self.url).map_err(|_| invalid())?;
        let address = format!(
            "{}://{}:{}",
            url.scheme(),
            url.host_str().ok_or_else(invalid)?,
            url.port_or_known_default().ok_or_else(invalid)?
        );
        let mut config = format!("proxy = {}\nnoproxy = \"\"\n", quote(&address));
        if !self.username.is_empty() {
            config.push_str(&format!(
                "proxy-user = {}\n",
                quote(&format!("{}:{}", self.username, self.password))
            ));
        }
        Ok(config)
    }
}

pub(super) fn public_ddns_settings(mut settings: Value) -> Value {
    settings["updateProxy"] = DDNSUpdateProxy::from_settings(&settings).public_value();
    settings
}

pub(super) fn update_proxy_mode(config: &HashMap<String, String>) -> &str {
    config
        .get("update_proxy_mode")
        .map(String::as_str)
        .filter(|v| !v.is_empty())
        .unwrap_or("inherit")
}

pub(super) async fn validate_target_update_proxy(
    state: &AppState,
    config: &HashMap<String, String>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(update_proxy_mode(config), "inherit" | "direct" | "proxy"),
        "Invalid DDNS update proxy mode"
    );
    if update_proxy_mode(config) == "proxy" {
        let raw = state.storage.store.get_string_value(DDNS_SETTINGS).await?;
        DDNSUpdateProxy::from_settings(&parse_settings(raw.as_deref())).validate(true)?;
    }
    Ok(())
}

pub(super) fn proxy_reqwest_error_key(error: &reqwest::Error) -> &'static str {
    if error.is_timeout() {
        return "updateProxyTimeout";
    }
    // Inspect causes for classification only; never return raw errors containing credentials.
    let mut causes = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        causes.push_str(&cause.to_string());
        source = cause.source();
    }
    let causes = causes.to_ascii_lowercase();
    if causes.contains("authentication")
        || causes.contains("credentials not accepted")
        || causes.contains("proxy authorization required")
    {
        "updateProxyAuthFailed"
    } else if causes.contains("certificate") || causes.contains("tls") {
        "updateProxyTlsFailed"
    } else if error.is_connect() {
        "updateProxyConnectionFailed"
    } else {
        "updateProxyRequestFailed"
    }
}

pub(super) fn proxy_curl_error_key(code: Option<i32>) -> &'static str {
    match code {
        Some(1 | 4) => "updateProxyUnsupported",
        Some(5 | 7) => "updateProxyConnectionFailed",
        Some(28) => "updateProxyTimeout",
        Some(35 | 51 | 58 | 59 | 60 | 77 | 80 | 83 | 90 | 91) => "updateProxyTlsFailed",
        Some(67) => "updateProxyAuthFailed",
        _ => "updateProxyRequestFailed",
    }
}

/// Classify SOCKS negotiation diagnostics without exposing stderr or credentials.
pub(super) fn proxy_curl_failure_key(code: Option<i32>, stderr: &[u8]) -> &'static str {
    if code == Some(97) {
        let detail = String::from_utf8_lossy(stderr).to_ascii_lowercase();
        if detail.contains("user was rejected") || detail.contains("authentication") {
            return "updateProxyAuthFailed";
        }
    }
    proxy_curl_error_key(code)
}
