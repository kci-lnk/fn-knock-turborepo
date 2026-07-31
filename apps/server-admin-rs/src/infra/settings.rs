use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, bail};

use crate::{crypto_utils::random_bytes, runtime_profile};

const ALTCHA_HMAC_KEY_FILE: &str = "altcha_hmac_key";
const ALTCHA_HMAC_KEY_LOCK_FILE: &str = ".altcha_hmac_key.lock";

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
    #[allow(dead_code)]
    pub gateway_config_dir: PathBuf,
    pub waf_dir: PathBuf,
    pub sqlite_path: PathBuf,
    #[allow(dead_code)]
    pub legacy_redis_url: String,
    pub go_backend_grpc_addr: String,
    pub internal_rpc_token: String,
    pub hmac_secret: String,
    pub altcha_hmac_key: Option<String>,
    #[allow(dead_code)]
    pub admin_proxy_secret: String,
    pub expose_runtime_hmac_secret: bool,
    pub request_timeout: Duration,
    pub asset_download_connect_timeout: Duration,
    pub asset_download_read_timeout: Duration,
    pub asset_download_total_timeout: Duration,
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
        let protected_admin_runtime = matches!(
            detected_runtime_target.as_str(),
            "docker" | "openwrt" | "linux" | "windows"
        );
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
        let legacy_redis_url = env::var("FN_KNOCK_LEGACY_REDIS_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(legacy_redis_url_from_redis_env);

        let data_dir = env_path("FN_KNOCK_DATA_DIR", &default_data_dir());
        let gateway_config_dir = env::var("FN_KNOCK_GATEWAY_CONFIG_DIR")
            .or_else(|_| env::var("GATEWAY_CONFIG_DIR"))
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| data_dir.clone());
        let waf_dir =
            env_optional_path("FN_KNOCK_WAF_DIR").unwrap_or_else(|| gateway_config_dir.join("waf"));
        let default_sqlite_path = default_sqlite_path(&gateway_config_dir);
        let sqlite_path = env_optional_path("FN_KNOCK_SQLITE_PATH").unwrap_or(default_sqlite_path);

        let expose_runtime_hmac_secret = should_expose_runtime_hmac_secret();

        let hmac_secret = env::var("HMAC_SECRET").unwrap_or_default();
        let internal_rpc_token = internal_rpc_token_from_env();

        Self {
            backend_host: backend_host.clone(),
            backend_port,
            auth_host: env_string("AUTH_HOST", "127.0.0.1"),
            auth_port,
            admin_view_host: if detected_runtime_target == "windows" {
                "127.0.0.1".to_string()
            } else {
                env::var("ADMIN_VIEW_HOST")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| {
                        if protected_admin_runtime {
                            "0.0.0.0".to_string()
                        } else {
                            backend_host.clone()
                        }
                    })
            },
            admin_view_port,
            admin_static_path: env_path("ADMIN_STATIC_PATH", "ui/www"),
            auth_static_path: env_path("AUTH_STATIC_PATH", "server-auth-view/dist"),
            data_dir,
            gateway_config_dir,
            waf_dir,
            sqlite_path,
            legacy_redis_url,
            go_backend_grpc_addr: env::var("GO_BACKEND_GRPC_ADDR")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| format!("127.0.0.1:{}", env_string("GO_BACKEND_PORT", "7996"))),
            internal_rpc_token,
            hmac_secret,
            altcha_hmac_key: env::var("ALTCHA_HMAC_KEY")
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            admin_proxy_secret: env::var("ADMIN_PROXY_SECRET").unwrap_or_default(),
            expose_runtime_hmac_secret,
            request_timeout: Duration::from_millis(env_u64_like_node(
                "GO_BACKEND_TIMEOUT_MS",
                DEFAULT_REQUEST_TIMEOUT_MS,
            )),
            asset_download_connect_timeout: Duration::from_millis(env_u64_like_node(
                "FN_KNOCK_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS",
                DEFAULT_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS,
            )),
            asset_download_read_timeout: Duration::from_millis(env_u64_like_node(
                "FN_KNOCK_ASSET_DOWNLOAD_READ_TIMEOUT_MS",
                DEFAULT_ASSET_DOWNLOAD_READ_TIMEOUT_MS,
            )),
            asset_download_total_timeout: Duration::from_millis(env_u64_like_node(
                "FN_KNOCK_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS",
                DEFAULT_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS,
            )),
            runtime_target,
            traffic_user_id: traffic_user_id_from_env(),
            traffic_keep_seconds: crate::node_compat::env_i64(
                "TRAFFIC_KEEP_SECONDS",
                7 * 24 * 3600,
            )
            .clamp(60, 365 * 24 * 3600),
            traffic_collect_interval: Duration::from_secs(parse_cron_interval_seconds(
                &env_string("TRAFFIC_COLLECT_CRON", "*/30 * * * * *"),
                30,
            )),
            traffic_collect_lock_ttl_seconds: crate::node_compat::env_i64(
                "TRAFFIC_COLLECT_LOCK_TTL",
                60,
            )
            .clamp(1, 3600) as usize,
            traffic_cleanup_interval: Duration::from_secs(parse_cron_interval_seconds(
                &env_string("TRAFFIC_CLEANUP_CRON", "0 * * * *"),
                3600,
            )),
            traffic_cleanup_lock_ttl_seconds: crate::node_compat::env_i64(
                "TRAFFIC_CLEANUP_LOCK_TTL",
                300,
            )
            .clamp(30, 3600) as usize,
        }
    }

    pub(crate) fn ensure_altcha_hmac_key(&mut self) -> anyhow::Result<()> {
        if self
            .altcha_hmac_key
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            return Ok(());
        }

        let path = self.data_dir.join(ALTCHA_HMAC_KEY_FILE);
        if let Some(value) = read_altcha_hmac_key(&path)? {
            secure_altcha_hmac_key_permissions(&path)?;
            self.altcha_hmac_key = Some(value);
            return Ok(());
        }

        fs::create_dir_all(&self.data_dir).with_context(|| {
            format!(
                "create ALTCHA HMAC key directory {}",
                self.data_dir.display()
            )
        })?;
        let _lock = lock_altcha_hmac_key_generation(&self.data_dir)?;
        if let Some(value) = read_altcha_hmac_key(&path)? {
            secure_altcha_hmac_key_permissions(&path)?;
            self.altcha_hmac_key = Some(value);
            return Ok(());
        }

        let value = hex::encode(random_bytes::<32>());
        write_altcha_hmac_key_atomically(&path, &value)?;
        self.altcha_hmac_key = Some(value);
        Ok(())
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

fn read_altcha_hmac_key(path: &Path) -> anyhow::Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(value) => {
            let value = value.trim().to_string();
            Ok((!value.is_empty()).then_some(value))
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error)
            .with_context(|| format!("read persisted ALTCHA HMAC key from {}", path.display())),
    }
}

fn lock_altcha_hmac_key_generation(data_dir: &Path) -> anyhow::Result<File> {
    let path = data_dir.join(ALTCHA_HMAC_KEY_LOCK_FILE);
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&path)
        .with_context(|| format!("open ALTCHA HMAC key lock file {}", path.display()))?;
    file.lock()
        .with_context(|| format!("lock ALTCHA HMAC key generation at {}", path.display()))?;
    Ok(file)
}

fn write_altcha_hmac_key_atomically(path: &Path, value: &str) -> anyhow::Result<()> {
    let Some(parent) = path.parent() else {
        bail!("ALTCHA HMAC key path has no parent: {}", path.display());
    };
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(ALTCHA_HMAC_KEY_FILE);
    let temporary_path = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        hex::encode(random_bytes::<8>())
    ));

    let result = (|| -> anyhow::Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary_path).with_context(|| {
            format!(
                "create temporary ALTCHA HMAC key file {}",
                temporary_path.display()
            )
        })?;
        file.write_all(value.as_bytes()).with_context(|| {
            format!(
                "write temporary ALTCHA HMAC key file {}",
                temporary_path.display()
            )
        })?;
        file.sync_all().with_context(|| {
            format!(
                "sync temporary ALTCHA HMAC key file {}",
                temporary_path.display()
            )
        })?;
        drop(file);

        replace_altcha_hmac_key_file(&temporary_path, path)?;
        secure_altcha_hmac_key_permissions(path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

#[cfg(not(windows))]
fn replace_altcha_hmac_key_file(temporary_path: &Path, path: &Path) -> anyhow::Result<()> {
    fs::rename(temporary_path, path).with_context(|| {
        format!(
            "persist ALTCHA HMAC key from {} to {}",
            temporary_path.display(),
            path.display()
        )
    })
}

#[cfg(windows)]
fn replace_altcha_hmac_key_file(temporary_path: &Path, path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        fs::remove_file(path)
            .with_context(|| format!("replace empty ALTCHA HMAC key file {}", path.display()))?;
    }
    fs::rename(temporary_path, path).with_context(|| {
        format!(
            "persist ALTCHA HMAC key from {} to {}",
            temporary_path.display(),
            path.display()
        )
    })
}

#[cfg(unix)]
fn secure_altcha_hmac_key_permissions(path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("secure ALTCHA HMAC key permissions for {}", path.display()))
}

#[cfg(not(unix))]
fn secure_altcha_hmac_key_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

const SQLITE_FILE_NAME: &str = "fn-knock.sqlite3";
const SQLITE_STORAGE_DIR: &str = "storage";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_ASSET_DOWNLOAD_READ_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS: u64 = 30 * 60 * 1_000;

fn env_string(name: &str, fallback: &str) -> String {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn legacy_redis_url_from_redis_env() -> String {
    let redis_host = env_string("REDIS_HOST", "127.0.0.1");
    let redis_port = env_port("REDIS_PORT", 6379);
    let redis_password = env::var("REDIS_PASSWORD")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(password) = redis_password {
        format!(
            "redis://:{}@{}:{}/",
            crate::http_utils::url_encode_component(&password),
            redis_host,
            redis_port
        )
    } else {
        format!("redis://{}:{}/", redis_host, redis_port)
    }
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
        "fpk-lite" | "fpk_lite" => "fpk-lite".to_string(),
        "openwrt" => "openwrt".to_string(),
        "linux" => "linux".to_string(),
        "synology" | "dsm" => "synology".to_string(),
        "windows" => "windows".to_string(),
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

fn env_optional_path(name: &str) -> Option<PathBuf> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

fn default_sqlite_path(gateway_config_dir: &Path) -> PathBuf {
    gateway_config_dir
        .join(SQLITE_STORAGE_DIR)
        .join(SQLITE_FILE_NAME)
}

fn default_data_dir() -> String {
    if cfg!(target_os = "windows") {
        return env::var("PROGRAMDATA")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\ProgramData"))
            .join("FnKnock")
            .to_string_lossy()
            .into_owned();
    }
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
        .and_then(|value| crate::node_compat::parse_i64_prefix_trim_start(&value))
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0)
        .unwrap_or(fallback)
}

pub(crate) fn parse_cron_interval_seconds(value: &str, fallback: u64) -> u64 {
    let trimmed = value.trim();
    if let Some(seconds) = parse_duration_seconds(trimmed) {
        return seconds.max(1);
    }

    let fields = trimmed.split_whitespace().collect::<Vec<_>>();
    match fields.as_slice() {
        [seconds, minutes, hours, day_of_month, ..] if fields.len() == 6 => {
            if let Some(value) = parse_every_field(seconds) {
                return value.max(1);
            }
            if *seconds == "0"
                && let Some(value) =
                    interval_from_minute_hour_day(minutes, hours, Some(*day_of_month))
            {
                return value;
            }
        }
        [minutes, hours, day_of_month, ..] if fields.len() == 5 => {
            if let Some(value) = interval_from_minute_hour_day(minutes, hours, Some(*day_of_month))
            {
                return value;
            }
        }
        _ => {}
    }
    fallback.max(1)
}

fn interval_from_minute_hour_day(
    minutes: &str,
    hours: &str,
    day_of_month: Option<&str>,
) -> Option<u64> {
    if let Some(value) = parse_every_field(minutes) {
        return Some(value.saturating_mul(60).max(60));
    }
    if !is_fixed_cron_field(minutes) {
        return None;
    }

    if let Some(value) = parse_every_field(hours) {
        return Some(value.saturating_mul(3600).max(3600));
    }
    if hours == "*" {
        return Some(3600);
    }
    if !is_fixed_cron_field(hours) {
        return None;
    }

    if let Some(value) = day_of_month.and_then(parse_every_field) {
        return Some(value.saturating_mul(24 * 3600).max(24 * 3600));
    }
    Some(24 * 3600)
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

fn is_fixed_cron_field(value: &str) -> bool {
    value.trim().parse::<u64>().is_ok()
}

fn parse_optional_port(value: &str) -> Option<u16> {
    let parsed = value.trim().parse::<u16>().ok()?;
    (parsed > 0).then_some(parsed)
}

fn should_expose_runtime_hmac_secret() -> bool {
    env::var("EXPOSE_RUNTIME_HMAC_SECRET").as_deref() == Ok("1")
        || env::var("NODE_ENV").as_deref() != Ok("production")
}

fn internal_rpc_token_from_env() -> String {
    env::var("FN_KNOCK_INTERNAL_RPC_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

fn parse_addr(host: &str, port: u16) -> anyhow::Result<SocketAddr> {
    let value = format!("{host}:{port}");
    value
        .parse::<SocketAddr>()
        .with_context(|| format!("invalid listen address: {value}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvGuard;

    fn with_env_vars<T>(keys: &[&str], run: impl FnOnce(&EnvGuard) -> T) -> T {
        let env = EnvGuard::new(keys);
        run(&env)
    }

    #[test]
    fn parses_common_traffic_cron_intervals() {
        assert_eq!(parse_cron_interval_seconds("*/30 * * * * *", 1), 30);
        assert_eq!(parse_cron_interval_seconds("*/5 * * * *", 1), 300);
        assert_eq!(parse_cron_interval_seconds("0 * * * *", 1), 3600);
        assert_eq!(parse_cron_interval_seconds("0 */6 * * *", 1), 21600);
        assert_eq!(parse_cron_interval_seconds("0 0 */2 * *", 1), 2 * 24 * 3600);
        assert_eq!(
            parse_cron_interval_seconds("0 0 0 */2 * *", 1),
            2 * 24 * 3600
        );
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
        assert_eq!(normalize_runtime_target_env("FPK_LITE"), "fpk-lite");
        assert_eq!(normalize_runtime_target_env("development"), "dev");
        assert_eq!(normalize_runtime_target_env("linux"), "linux");
        assert_eq!(normalize_runtime_target_env("synology"), "synology");
        assert_eq!(normalize_runtime_target_env("DSM"), "synology");
        assert_eq!(normalize_runtime_target_env("windows"), "windows");
        assert_eq!(normalize_runtime_target_env("unknown"), "");
    }

    #[test]
    fn openwrt_startup_defaults_match_node() {
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "BACKEND_PORT",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            |env| {
                env.set("FN_KNOCK_RUNTIME_TARGET", "openwrt");
                env.remove("BACKEND_PORT");
                env.remove("ADMIN_VIEW_PORT");
                env.remove("ADMIN_VIEW_HOST");
                env.remove("BACKEND_HOST");

                let settings = Settings::from_env();

                assert_eq!(settings.backend_port, 17998);
                assert_eq!(settings.admin_view_port, Some(7991));
                assert_eq!(settings.admin_view_host, "0.0.0.0");
            },
        );
    }

    #[test]
    fn generic_linux_exposes_only_the_protected_admin_view() {
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "BACKEND_PORT",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            |env| {
                env.set("FN_KNOCK_RUNTIME_TARGET", "linux");
                env.remove("BACKEND_PORT");
                env.remove("ADMIN_VIEW_PORT");
                env.set("ADMIN_VIEW_HOST", "0.0.0.0");
                env.set("BACKEND_HOST", "127.0.0.1");

                let settings = Settings::from_env();

                assert_eq!(settings.backend_port, 7998);
                assert_eq!(settings.backend_host, "127.0.0.1");
                assert_eq!(settings.admin_view_port, Some(7991));
                assert_eq!(settings.admin_view_host, "0.0.0.0");
            },
        );
    }

    #[test]
    fn synology_uses_the_loopback_backend_for_dsm_cgi() {
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "BACKEND_PORT",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            |env| {
                env.set("FN_KNOCK_RUNTIME_TARGET", "synology");
                env.remove("BACKEND_PORT");
                env.remove("ADMIN_VIEW_PORT");
                env.remove("ADMIN_VIEW_HOST");
                env.remove("BACKEND_HOST");

                let settings = Settings::from_env();

                assert_eq!(settings.runtime_target, "synology");
                assert_eq!(settings.backend_host, "127.0.0.1");
                assert_eq!(settings.backend_port, 7998);
                assert_eq!(settings.admin_view_port, None);
                assert_eq!(settings.admin_view_host, "127.0.0.1");
            },
        );
    }

    #[test]
    fn non_protected_runtime_ignores_admin_view_port_like_node() {
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
            ],
            |env| {
                env.set("FN_KNOCK_RUNTIME_TARGET", "dev");
                env.set("ADMIN_VIEW_PORT", "7991");
                env.remove("ADMIN_VIEW_HOST");
                env.set("BACKEND_HOST", "127.0.0.2");

                let settings = Settings::from_env();

                assert_eq!(settings.admin_view_port, None);
                assert_eq!(settings.admin_view_host, "127.0.0.2");
            },
        );
    }

    #[test]
    fn windows_uses_protected_loopback_admin_view() {
        with_env_vars(
            &[
                "FN_KNOCK_RUNTIME_TARGET",
                "ADMIN_VIEW_PORT",
                "ADMIN_VIEW_HOST",
                "BACKEND_HOST",
                "FN_KNOCK_DATA_DIR",
            ],
            |env| {
                env.set("FN_KNOCK_RUNTIME_TARGET", "windows");
                env.remove("ADMIN_VIEW_PORT");
                env.set("ADMIN_VIEW_HOST", "0.0.0.0");
                env.remove("BACKEND_HOST");
                env.set("FN_KNOCK_DATA_DIR", r"C:\ProgramData\FnKnock");

                let settings = Settings::from_env();

                assert_eq!(settings.backend_host, "127.0.0.1");
                assert_eq!(settings.backend_port, 7998);
                assert_eq!(settings.auth_host, "127.0.0.1");
                assert_eq!(settings.auth_port, 7997);
                assert_eq!(settings.admin_view_host, "127.0.0.1");
                assert_eq!(settings.admin_view_port, Some(7991));
                assert_eq!(settings.go_backend_grpc_addr, "127.0.0.1:7996");
                assert_eq!(settings.data_dir, PathBuf::from(r"C:\ProgramData\FnKnock"));
            },
        );
    }

    #[test]
    fn asset_download_timeouts_are_independent_from_backend_timeout() {
        with_env_vars(
            &[
                "GO_BACKEND_TIMEOUT_MS",
                "FN_KNOCK_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS",
                "FN_KNOCK_ASSET_DOWNLOAD_READ_TIMEOUT_MS",
                "FN_KNOCK_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS",
            ],
            |env| {
                env.set("GO_BACKEND_TIMEOUT_MS", "5000");
                env.remove("FN_KNOCK_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS");
                env.remove("FN_KNOCK_ASSET_DOWNLOAD_READ_TIMEOUT_MS");
                env.remove("FN_KNOCK_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.request_timeout,
                    Duration::from_millis(DEFAULT_REQUEST_TIMEOUT_MS)
                );
                assert_eq!(
                    settings.asset_download_connect_timeout,
                    Duration::from_millis(DEFAULT_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS)
                );
                assert_eq!(
                    settings.asset_download_read_timeout,
                    Duration::from_millis(DEFAULT_ASSET_DOWNLOAD_READ_TIMEOUT_MS)
                );
                assert_eq!(
                    settings.asset_download_total_timeout,
                    Duration::from_millis(DEFAULT_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS)
                );

                env.set("FN_KNOCK_ASSET_DOWNLOAD_CONNECT_TIMEOUT_MS", "45000ms");
                env.set("FN_KNOCK_ASSET_DOWNLOAD_READ_TIMEOUT_MS", "180000ms");
                env.set("FN_KNOCK_ASSET_DOWNLOAD_TOTAL_TIMEOUT_MS", "3600000ms");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.asset_download_connect_timeout,
                    Duration::from_millis(45_000)
                );
                assert_eq!(
                    settings.asset_download_read_timeout,
                    Duration::from_millis(180_000)
                );
                assert_eq!(
                    settings.asset_download_total_timeout,
                    Duration::from_millis(3_600_000)
                );
            },
        );
    }

    #[test]
    fn sqlite_default_path_uses_gateway_config_dir() {
        with_env_vars(
            &[
                "FN_KNOCK_DATA_DIR",
                "FN_KNOCK_GATEWAY_CONFIG_DIR",
                "GATEWAY_CONFIG_DIR",
                "FN_KNOCK_SQLITE_PATH",
            ],
            |env| {
                env.set("FN_KNOCK_DATA_DIR", "/tmp/fn-knock-runtime");
                env.set("FN_KNOCK_GATEWAY_CONFIG_DIR", "/usr/local/etc/fn-knock");
                env.set("GATEWAY_CONFIG_DIR", "/ignored/gateway");
                env.remove("FN_KNOCK_SQLITE_PATH");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.sqlite_path,
                    PathBuf::from("/usr/local/etc/fn-knock/storage/fn-knock.sqlite3")
                );
            },
        );
    }

    #[test]
    fn waf_dir_defaults_to_gateway_config_subdirectory() {
        with_env_vars(
            &[
                "FN_KNOCK_DATA_DIR",
                "FN_KNOCK_GATEWAY_CONFIG_DIR",
                "GATEWAY_CONFIG_DIR",
                "FN_KNOCK_WAF_DIR",
            ],
            |env| {
                env.set("FN_KNOCK_DATA_DIR", "/tmp/fn-knock-runtime");
                env.set("FN_KNOCK_GATEWAY_CONFIG_DIR", "/etc/fn-knock/gateway");
                env.remove("GATEWAY_CONFIG_DIR");
                env.remove("FN_KNOCK_WAF_DIR");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.waf_dir,
                    PathBuf::from("/etc/fn-knock/gateway").join("waf")
                );
            },
        );
    }

    #[test]
    fn waf_dir_uses_explicit_override() {
        with_env_vars(
            &[
                "FN_KNOCK_DATA_DIR",
                "FN_KNOCK_GATEWAY_CONFIG_DIR",
                "GATEWAY_CONFIG_DIR",
                "FN_KNOCK_WAF_DIR",
            ],
            |env| {
                env.set("FN_KNOCK_DATA_DIR", "/tmp/fn-knock-runtime");
                env.set("FN_KNOCK_GATEWAY_CONFIG_DIR", "/etc/fn-knock/gateway");
                env.remove("GATEWAY_CONFIG_DIR");
                env.set("FN_KNOCK_WAF_DIR", "/var/lib/fn-knock/waf");

                let settings = Settings::from_env();

                assert_eq!(settings.waf_dir, PathBuf::from("/var/lib/fn-knock/waf"));
            },
        );
    }

    #[test]
    fn sqlite_default_path_falls_back_to_gateway_config_dir_env() {
        with_env_vars(
            &[
                "FN_KNOCK_DATA_DIR",
                "FN_KNOCK_GATEWAY_CONFIG_DIR",
                "GATEWAY_CONFIG_DIR",
                "FN_KNOCK_SQLITE_PATH",
            ],
            |env| {
                env.set("FN_KNOCK_DATA_DIR", "/tmp/fn-knock-runtime");
                env.remove("FN_KNOCK_GATEWAY_CONFIG_DIR");
                env.set("GATEWAY_CONFIG_DIR", "/etc/fn-knock/gateway");
                env.remove("FN_KNOCK_SQLITE_PATH");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.sqlite_path,
                    PathBuf::from("/etc/fn-knock/gateway/storage/fn-knock.sqlite3")
                );
            },
        );
    }

    #[test]
    fn sqlite_path_override_uses_explicit_path() {
        with_env_vars(
            &[
                "FN_KNOCK_DATA_DIR",
                "FN_KNOCK_GATEWAY_CONFIG_DIR",
                "GATEWAY_CONFIG_DIR",
                "FN_KNOCK_SQLITE_PATH",
            ],
            |env| {
                env.set("FN_KNOCK_DATA_DIR", "/tmp/fn-knock-runtime");
                env.set("FN_KNOCK_GATEWAY_CONFIG_DIR", "/usr/local/etc/fn-knock");
                env.remove("GATEWAY_CONFIG_DIR");
                env.set("FN_KNOCK_SQLITE_PATH", "/custom/fn-knock.sqlite3");

                let settings = Settings::from_env();

                assert_eq!(
                    settings.sqlite_path,
                    PathBuf::from("/custom/fn-knock.sqlite3")
                );
            },
        );
    }

    #[test]
    fn explicit_altcha_hmac_key_takes_precedence_without_touching_disk() {
        with_env_vars(&["ALTCHA_HMAC_KEY", "FN_KNOCK_DATA_DIR"], |env| {
            let directory = tempfile::tempdir().unwrap();
            let data_dir = directory.path().join("data");
            env.set("ALTCHA_HMAC_KEY", " explicit-altcha-key ");
            env.set("FN_KNOCK_DATA_DIR", &data_dir);

            let mut settings = Settings::from_env();
            settings.ensure_altcha_hmac_key().unwrap();

            assert_eq!(
                settings.altcha_hmac_key.as_deref(),
                Some("explicit-altcha-key")
            );
            assert!(!data_dir.join(ALTCHA_HMAC_KEY_FILE).exists());
        });
    }

    #[test]
    fn missing_altcha_hmac_key_is_generated_once_and_reused() {
        with_env_vars(&["ALTCHA_HMAC_KEY", "FN_KNOCK_DATA_DIR"], |env| {
            let directory = tempfile::tempdir().unwrap();
            let data_dir = directory.path().join("data");
            env.remove("ALTCHA_HMAC_KEY");
            env.set("FN_KNOCK_DATA_DIR", &data_dir);

            let mut first = Settings::from_env();
            first.ensure_altcha_hmac_key().unwrap();
            let generated = first.altcha_hmac_key.clone().unwrap();
            assert_eq!(generated.len(), 64);
            assert!(generated.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert_eq!(
                std::fs::read_to_string(data_dir.join(ALTCHA_HMAC_KEY_FILE)).unwrap(),
                generated
            );

            let mut second = Settings::from_env();
            second.ensure_altcha_hmac_key().unwrap();
            assert_eq!(second.altcha_hmac_key.as_deref(), Some(generated.as_str()));

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(data_dir.join(ALTCHA_HMAC_KEY_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777;
                assert_eq!(mode, 0o600);
            }
        });
    }

    #[test]
    fn concurrent_altcha_hmac_key_generation_reuses_the_winning_key() {
        use std::sync::{Arc, Barrier};

        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path().join("data");
        let mut template = Settings::from_env();
        template.data_dir = data_dir.clone();
        template.altcha_hmac_key = None;

        let worker_count = 16;
        let barrier = Arc::new(Barrier::new(worker_count));
        let handles = (0..worker_count)
            .map(|_| {
                let barrier = Arc::clone(&barrier);
                let mut settings = template.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    settings.ensure_altcha_hmac_key().unwrap();
                    settings.altcha_hmac_key.unwrap()
                })
            })
            .collect::<Vec<_>>();

        let generated = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>();
        assert!(
            generated.windows(2).all(|keys| keys[0] == keys[1]),
            "concurrent startups did not reuse one persisted key"
        );
        assert_eq!(
            std::fs::read_to_string(data_dir.join(ALTCHA_HMAC_KEY_FILE)).unwrap(),
            generated[0]
        );
    }

    #[test]
    fn persisted_altcha_hmac_key_is_trimmed_and_secured() {
        with_env_vars(&["ALTCHA_HMAC_KEY", "FN_KNOCK_DATA_DIR"], |env| {
            let directory = tempfile::tempdir().unwrap();
            let data_dir = directory.path().join("data");
            std::fs::create_dir_all(&data_dir).unwrap();
            let key_path = data_dir.join(ALTCHA_HMAC_KEY_FILE);
            std::fs::write(&key_path, " persisted-altcha-key \n").unwrap();
            env.remove("ALTCHA_HMAC_KEY");
            env.set("FN_KNOCK_DATA_DIR", &data_dir);

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o644))
                    .unwrap();
            }

            let mut settings = Settings::from_env();
            settings.ensure_altcha_hmac_key().unwrap();
            assert_eq!(
                settings.altcha_hmac_key.as_deref(),
                Some("persisted-altcha-key")
            );

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mode = std::fs::metadata(&key_path).unwrap().permissions().mode() & 0o777;
                assert_eq!(mode, 0o600);
            }
        });
    }

    #[test]
    fn empty_altcha_hmac_key_file_is_repaired_atomically() {
        with_env_vars(&["ALTCHA_HMAC_KEY", "FN_KNOCK_DATA_DIR"], |env| {
            let directory = tempfile::tempdir().unwrap();
            let data_dir = directory.path().join("data");
            std::fs::create_dir_all(&data_dir).unwrap();
            let key_path = data_dir.join(ALTCHA_HMAC_KEY_FILE);
            std::fs::write(&key_path, " \n").unwrap();
            env.remove("ALTCHA_HMAC_KEY");
            env.set("FN_KNOCK_DATA_DIR", &data_dir);

            let mut settings = Settings::from_env();
            settings.ensure_altcha_hmac_key().unwrap();

            let generated = settings.altcha_hmac_key.unwrap();
            assert_eq!(generated.len(), 64);
            assert_eq!(std::fs::read_to_string(key_path).unwrap(), generated);
        });
    }

    #[test]
    fn altcha_hmac_key_persistence_failure_is_a_startup_error() {
        with_env_vars(&["ALTCHA_HMAC_KEY", "FN_KNOCK_DATA_DIR"], |env| {
            let directory = tempfile::tempdir().unwrap();
            let data_dir = directory.path().join("not-a-directory");
            std::fs::write(&data_dir, "file").unwrap();
            env.remove("ALTCHA_HMAC_KEY");
            env.set("FN_KNOCK_DATA_DIR", &data_dir);

            let mut settings = Settings::from_env();
            let error = settings.ensure_altcha_hmac_key().unwrap_err();

            assert!(
                error.to_string().contains("ALTCHA HMAC key"),
                "unexpected error: {error:#}"
            );
        });
    }

    #[test]
    fn runtime_secret_exposure_matches_node_env_logic() {
        with_env_vars(&["EXPOSE_RUNTIME_HMAC_SECRET", "NODE_ENV"], |env| {
            env.remove("EXPOSE_RUNTIME_HMAC_SECRET");
            env.set("NODE_ENV", "production");
            assert!(!should_expose_runtime_hmac_secret());

            env.set("EXPOSE_RUNTIME_HMAC_SECRET", "1");
            assert!(should_expose_runtime_hmac_secret());

            env.remove("EXPOSE_RUNTIME_HMAC_SECRET");
            env.set("NODE_ENV", "development");
            assert!(should_expose_runtime_hmac_secret());
        });
    }

    #[test]
    fn internal_rpc_token_uses_only_explicit_env() {
        with_env_vars(&["FN_KNOCK_INTERNAL_RPC_TOKEN", "HMAC_SECRET"], |env| {
            env.set("FN_KNOCK_INTERNAL_RPC_TOKEN", " explicit-token ");
            env.set("HMAC_SECRET", "hmac-token");
            assert_eq!(Settings::from_env().internal_rpc_token, "explicit-token");

            env.remove("FN_KNOCK_INTERNAL_RPC_TOKEN");
            env.set("HMAC_SECRET", " hmac-token ");
            assert_eq!(Settings::from_env().internal_rpc_token, "");

            env.set("FN_KNOCK_INTERNAL_RPC_TOKEN", " ");
            env.set("HMAC_SECRET", " fallback-token ");
            assert_eq!(Settings::from_env().internal_rpc_token, "");
        });
    }

    #[test]
    fn env_int_parser_matches_node_parse_int_prefixes() {
        assert_eq!(
            crate::node_compat::parse_i64_prefix_trim_start("60s"),
            Some(60)
        );
        assert_eq!(
            crate::node_compat::parse_i64_prefix_trim_start("  +3.9"),
            Some(3)
        );
        assert_eq!(
            crate::node_compat::parse_i64_prefix_trim_start("-1x"),
            Some(-1)
        );
        assert_eq!(
            crate::node_compat::parse_i64_prefix_trim_start("0x10"),
            Some(0)
        );
        assert_eq!(
            crate::node_compat::parse_i64_prefix_trim_start("nope"),
            None
        );
        assert_eq!(crate::node_compat::parse_i64_prefix_trim_start("+"), None);
    }
}
