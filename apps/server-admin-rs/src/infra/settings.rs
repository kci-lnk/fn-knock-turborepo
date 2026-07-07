use std::{env, net::SocketAddr, path::PathBuf, time::Duration};

use anyhow::Context;

use crate::runtime_profile;

#[derive(Clone, Debug)]
pub struct Settings {
    pub backend_host: String,
    pub backend_port: u16,
    pub auth_host: String,
    pub auth_port: u16,
    pub admin_view_host: String,
    pub admin_view_port: Option<u16>,
    pub admin_static_path: PathBuf,
    pub auth_static_path: PathBuf,
    #[allow(dead_code)]
    pub data_dir: PathBuf,
    pub gateway_config_dir: PathBuf,
    pub redis_url: String,
    pub go_backend_base_url: String,
    pub hmac_secret: String,
    pub altcha_hmac_key: Option<String>,
    #[allow(dead_code)]
    pub admin_proxy_secret: String,
    pub expose_runtime_hmac_secret: bool,
    pub request_timeout: Duration,
    pub runtime_target: String,
    pub traffic_user_id: String,
    pub traffic_keep_seconds: i64,
    pub traffic_collect_interval: Duration,
    pub traffic_collect_lock_ttl_seconds: usize,
    pub traffic_cleanup_interval: Duration,
    pub traffic_cleanup_lock_ttl_seconds: usize,
}

impl Settings {
    pub fn from_env() -> Self {
        let runtime_target =
            normalize_runtime_target_env(&env_string("FN_KNOCK_RUNTIME_TARGET", ""));
        let detected_runtime_target =
            runtime_profile::detect_deployment_target(Some(&runtime_target));
        let protected_admin_runtime =
            matches!(detected_runtime_target.as_str(), "docker" | "openwrt");
        let backend_port_default = if detected_runtime_target == "openwrt" {
            17998
        } else {
            7998
        };
        let backend_host = env_string("BACKEND_HOST", "127.0.0.1");
        let backend_port = env_port("BACKEND_PORT", backend_port_default);
        let auth_port = env_port("AUTH_PORT", 7997);
        let admin_view_port = if protected_admin_runtime {
            Some(env_optional_port("ADMIN_VIEW_PORT", 7991))
        } else {
            None
        };
        let redis_host = env::var("REDIS_HOST").unwrap_or_else(|_| {
            if runtime_target == "docker" {
                "redis".to_string()
            } else {
                "127.0.0.1".to_string()
            }
        });
        let redis_port = env_port("REDIS_PORT", 6379);
        let redis_password = env::var("REDIS_PASSWORD")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let redis_url = if let Some(password) = redis_password {
            format!(
                "redis://:{}@{}:{}/",
                url_escape(&password),
                redis_host,
                redis_port
            )
        } else {
            format!("redis://{}:{}/", redis_host, redis_port)
        };

        let data_dir = env_path("FN_KNOCK_DATA_DIR", &default_data_dir());
        let gateway_config_dir = env::var("FN_KNOCK_GATEWAY_CONFIG_DIR")
            .or_else(|_| env::var("GATEWAY_CONFIG_DIR"))
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.clone());

        let expose_runtime_hmac_secret = should_expose_runtime_hmac_secret();

        Self {
            backend_host: backend_host.clone(),
            backend_port,
            auth_host: env_string("AUTH_HOST", "127.0.0.1"),
            auth_port,
            admin_view_host: env::var("ADMIN_VIEW_HOST")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    if protected_admin_runtime {
                        "0.0.0.0".to_string()
                    } else {
                        backend_host.clone()
                    }
                }),
            admin_view_port,
            admin_static_path: env_path("ADMIN_STATIC_PATH", "ui/www"),
            auth_static_path: env_path("AUTH_STATIC_PATH", "server-auth-view/dist"),
            data_dir,
            gateway_config_dir,
            redis_url,
            go_backend_base_url: env::var("GO_BACKEND_BASE_URL")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    format!("http://localhost:{}", env_string("GO_BACKEND_PORT", "7996"))
                }),
            hmac_secret: env::var("HMAC_SECRET").unwrap_or_default(),
            altcha_hmac_key: env::var("ALTCHA_HMAC_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            admin_proxy_secret: env::var("ADMIN_PROXY_SECRET").unwrap_or_default(),
            expose_runtime_hmac_secret,
            request_timeout: Duration::from_millis(env_u64_like_node(
                "GO_BACKEND_TIMEOUT_MS",
                5000,
            )),
            runtime_target,
            traffic_user_id: traffic_user_id_from_env(),
            traffic_keep_seconds: env_i64_like_node("TRAFFIC_KEEP_SECONDS", 7 * 24 * 3600)
                .clamp(60, 365 * 24 * 3600),
            traffic_collect_interval: Duration::from_secs(parse_cron_interval_seconds(
                &env_string("TRAFFIC_COLLECT_CRON", "*/30 * * * * *"),
                30,
            )),
            traffic_collect_lock_ttl_seconds: env_i64_like_node("TRAFFIC_COLLECT_LOCK_TTL", 60)
                .clamp(1, 3600) as usize,
            traffic_cleanup_interval: Duration::from_secs(parse_cron_interval_seconds(
                &env_string("TRAFFIC_CLEANUP_CRON", "0 * * * *"),
                3600,
            )),
            traffic_cleanup_lock_ttl_seconds: env_i64_like_node("TRAFFIC_CLEANUP_LOCK_TTL", 300)
                .clamp(30, 3600) as usize,
        }
    }

    pub fn backend_addr(&self) -> anyhow::Result<SocketAddr> {
        parse_addr(&self.backend_host, self.backend_port)
    }

    pub fn auth_addr(&self) -> anyhow::Result<SocketAddr> {
        parse_addr(&self.auth_host, self.auth_port)
    }

    pub fn admin_view_addr(&self) -> anyhow::Result<Option<SocketAddr>> {
        self.admin_view_port
            .map(|port| parse_addr(&self.admin_view_host, port))
            .transpose()
    }
}

fn env_string(name: &str, fallback: &str) -> String {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn traffic_user_id_from_env() -> String {
    normalize_traffic_user_id(env::var("TRAFFIC_USER_ID").ok())
}

fn normalize_traffic_user_id(value: Option<String>) -> String {
    value
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "global".to_string())
}

fn normalize_runtime_target_env(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "docker" => "docker".to_string(),
        "fpk" => "fpk".to_string(),
        "openwrt" => "openwrt".to_string(),
        "dev" | "development" => "dev".to_string(),
        _ => String::new(),
    }
}

fn env_path(name: &str, fallback: &str) -> PathBuf {
    env::var(name)
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(fallback))
}

fn default_data_dir() -> String {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    if cfg!(target_os = "macos") {
        format!("{home}/Library/Application Support/fn-knock")
    } else {
        format!("{home}/.local/share/fn-knock")
    }
}

fn env_port(name: &str, fallback: u16) -> u16 {
    env::var(name)
        .ok()
        .and_then(|value| parse_optional_port(&value))
        .unwrap_or(fallback)
}

fn env_optional_port(name: &str, fallback: u16) -> u16 {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .and_then(|value| parse_optional_port(&value))
        .unwrap_or(fallback)
}

fn env_u64_like_node(name: &str, fallback: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| parse_int_prefix_like_node(&value))
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

fn env_i64_like_node(name: &str, fallback: i64) -> i64 {
    crate::node_compat::env_i64(name, fallback)
}

fn parse_int_prefix_like_node(value: &str) -> Option<i64> {
    crate::node_compat::parse_i64_prefix_trim_start(value)
}

fn parse_cron_interval_seconds(value: &str, fallback: u64) -> u64 {
    let trimmed = value.trim();
    if let Some(seconds) = parse_duration_seconds(trimmed) {
        return seconds.max(1);
    }

    let fields = trimmed.split_whitespace().collect::<Vec<_>>();
    match fields.as_slice() {
        [seconds, ..] if fields.len() == 6 => {
            if let Some(value) = parse_every_field(seconds) {
                return value.max(1);
            }
        }
        [minutes, ..] if fields.len() == 5 => {
            if let Some(value) = parse_every_field(minutes) {
                return value.saturating_mul(60).max(60);
            }
            if *minutes == "0" {
                if let Some(hours) = fields.get(1).and_then(|field| parse_every_field(field)) {
                    return hours.saturating_mul(3600).max(3600);
                }
                return 3600;
            }
        }
        _ => {}
    }
    fallback.max(1)
}

fn parse_duration_seconds(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if let Some(raw) = value.strip_suffix('s') {
        return raw.trim().parse::<u64>().ok();
    }
    if let Some(raw) = value.strip_suffix('m') {
        return raw.trim().parse::<u64>().ok().map(|minutes| minutes * 60);
    }
    if let Some(raw) = value.strip_suffix('h') {
        return raw.trim().parse::<u64>().ok().map(|hours| hours * 3600);
    }
    value.parse::<u64>().ok()
}

fn parse_every_field(value: &str) -> Option<u64> {
    value
        .trim()
        .strip_prefix("*/")
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|value| *value > 0)
}

fn parse_optional_port(value: &str) -> Option<u16> {
    let parsed = value.trim().parse::<u16>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn should_expose_runtime_hmac_secret() -> bool {
    env::var("EXPOSE_RUNTIME_HMAC_SECRET").as_deref() == Ok("1")
        || env::var("NODE_ENV").as_deref() != Ok("production")
}

fn parse_addr(host: &str, port: u16) -> anyhow::Result<SocketAddr> {
    let value = format!("{host}:{port}");
    value
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid listen address: {value}"))
}

fn url_escape(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::{OsStr, OsString};
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn set_env(key: &str, value: impl AsRef<OsStr>) {
        unsafe {
            env::set_var(key, value);
        }
    }

    fn remove_env(key: &str) {
        unsafe {
            env::remove_var(key);
        }
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            set_env(key, value);
        } else {
            remove_env(key);
        }
    }

    fn with_env_vars<T>(keys: &[&str], run: impl FnOnce() -> T) -> T {
        let previous = keys
            .iter()
            .map(|key| ((*key).to_string(), env::var_os(key)))
            .collect::<Vec<_>>();
        let result = run();
        for (key, value) in previous {
            restore_env(&key, value);
        }
        result
    }

    #[test]
    fn parses_common_traffic_cron_intervals() {
        assert_eq!(parse_cron_interval_seconds("*/30 * * * * *", 1), 30);
        assert_eq!(parse_cron_interval_seconds("*/5 * * * *", 1), 300);
        assert_eq!(parse_cron_interval_seconds("0 * * * *", 1), 3600);
        assert_eq!(parse_cron_interval_seconds("45s", 1), 45);
        assert_eq!(parse_cron_interval_seconds("2m", 1), 120);
    }

    #[test]
    fn traffic_user_id_matches_node_truthy_env_fallback() {
        assert_eq!(normalize_traffic_user_id(None), "global");
        assert_eq!(normalize_traffic_user_id(Some(String::new())), "global");
        assert_eq!(normalize_traffic_user_id(Some("  ".to_string())), "  ");
        assert_eq!(
            normalize_traffic_user_id(Some(" user ".to_string())),
            " user "
        );
    }

    #[test]
    fn normalizes_runtime_target_env_values() {
        assert_eq!(normalize_runtime_target_env(" FPK "), "fpk");
        assert_eq!(normalize_runtime_target_env("development"), "dev");
        assert_eq!(normalize_runtime_target_env("unknown"), "");
    }

    #[test]
    fn openwrt_startup_defaults_match_node() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "BACKEND_PORT",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            || {
                set_env("FN_KNOCK_RUNTIME_TARGET", "openwrt");
                remove_env("BACKEND_PORT");
                remove_env("ADMIN_VIEW_PORT");
                remove_env("ADMIN_VIEW_HOST");
                remove_env("BACKEND_HOST");

                let settings = Settings::from_env();

                assert_eq!(settings.backend_port, 17998);
                assert_eq!(settings.admin_view_port, Some(7991));
                assert_eq!(settings.admin_view_host, "0.0.0.0");
            },
        );
    }

    #[test]
    fn non_protected_runtime_ignores_admin_view_port_like_node() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            || {
                set_env("FN_KNOCK_RUNTIME_TARGET", "dev");
                set_env("ADMIN_VIEW_PORT", "7991");
                remove_env("ADMIN_VIEW_HOST");
                set_env("BACKEND_HOST", "127.0.0.2");

                let settings = Settings::from_env();

                assert_eq!(settings.admin_view_port, None);
                assert_eq!(settings.admin_view_host, "127.0.0.2");
            },
        );
    }

    #[test]
    fn runtime_secret_exposure_matches_node_env_logic() {
        let _guard = ENV_LOCK.lock().unwrap();
        with_env_vars(&["EXPOSE_RUNTIME_HMAC_SECRET", "NODE_ENV"], || {
            remove_env("EXPOSE_RUNTIME_HMAC_SECRET");
            set_env("NODE_ENV", "production");
            assert!(!should_expose_runtime_hmac_secret());

            set_env("EXPOSE_RUNTIME_HMAC_SECRET", "1");
            assert!(should_expose_runtime_hmac_secret());

            remove_env("EXPOSE_RUNTIME_HMAC_SECRET");
            set_env("NODE_ENV", "development");
            assert!(should_expose_runtime_hmac_secret());
        });
    }

    #[test]
    fn env_int_parser_matches_node_parse_int_prefixes() {
        assert_eq!(parse_int_prefix_like_node("60s"), Some(60));
        assert_eq!(parse_int_prefix_like_node("  +3.9"), Some(3));
        assert_eq!(parse_int_prefix_like_node("-1x"), Some(-1));
        assert_eq!(parse_int_prefix_like_node("0x10"), Some(0));
        assert_eq!(parse_int_prefix_like_node("nope"), None);
        assert_eq!(parse_int_prefix_like_node("+"), None);
    }
}
