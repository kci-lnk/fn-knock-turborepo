use super::preflight::{normalize_scanner_ip, normalize_scanner_path};
use super::*;

const PATH_WHITELIST_FIELD: &str = "pathWhitelist";

const DEFAULT_SCANNER_PATH_WHITELIST: &[&str] = &[
    "/",
    "/index.html",
    "/robots.txt",
    "/sitemap.xml",
    "/favicon.ico",
    "/favicon.svg",
    "/api/auth/bootstrap",
    "/api/auth/captcha/config",
    "/api/auth/challenge",
    "/api/auth/login",
    "/api/auth/ip",
    "/api/auth/ip/location",
    "/api/auth/session",
    "/api/auth/verify",
    "/api/auth/passkey/status",
    "/trimcon",
    "/.well-known/ai-plugin.json",
    "/apple-touch-icon.png",
    "/manifest.json",
    "/login",
    "/locales/zh-CN/os.json",
    "/license/v1/device/baseInfo",
    "/locales/zh-CN/apps/setting.json",
    "/app-center/v1/check-update",
    "/sac/rpcproxy/v1/new-user-guide/status",
    "/locales/zh-CN/pages/login.json",
    "/static/bg/wallpaper-1.webp",
    "/api/config",
    "/identity/connect/token",
    "/sync/event/register",
];

pub(super) fn default_scanner_path_whitelist() -> Vec<String> {
    DEFAULT_SCANNER_PATH_WHITELIST
        .iter()
        .map(|path| (*path).to_string())
        .collect()
}

pub(super) fn scanner_path_whitelist_from_raw(
    raw: Option<&Value>,
) -> Result<Vec<String>, ScannerError> {
    let Some(value) = raw.and_then(|value| value.get(PATH_WHITELIST_FIELD)) else {
        return Ok(default_scanner_path_whitelist());
    };
    let values = value
        .as_array()
        .ok_or_else(|| ScannerError::BadRequest("Invalid scanner path whitelist".to_string()))?;
    let paths = values
        .iter()
        .map(|value| {
            value.as_str().map(ToString::to_string).ok_or_else(|| {
                ScannerError::BadRequest("Invalid scanner path whitelist".to_string())
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    normalize_scanner_path_whitelist(paths)
}

pub(super) fn normalize_scanner_path_whitelist(
    paths: Vec<String>,
) -> Result<Vec<String>, ScannerError> {
    let mut normalized = Vec::with_capacity(paths.len());
    let mut seen = HashSet::with_capacity(paths.len());
    for path in paths {
        let path = normalize_scanner_whitelist_entry(&path)?;
        if seen.insert(path.clone()) {
            normalized.push(path);
        }
    }
    Ok(normalized)
}

pub(super) fn normalize_scanner_whitelist_entry(path: &str) -> Result<String, ScannerError> {
    if path.chars().any(char::is_control) {
        return Err(ScannerError::BadRequest(
            "Path contains control characters".to_string(),
        ));
    }
    let path = path.trim();
    if path.is_empty() {
        return Err(ScannerError::BadRequest(
            "Path must not be empty".to_string(),
        ));
    }
    if !path.starts_with('/') {
        return Err(ScannerError::BadRequest(
            "Path must be absolute".to_string(),
        ));
    }
    Ok(normalize_scanner_path(path))
}

pub(super) async fn load_scanner_path_whitelist(
    state: &AppState,
) -> Result<ScannerPathWhitelist, ScannerError> {
    let raw = state.storage.store.scanner_settings_raw().await?;
    Ok(scanner_path_whitelist_payload(
        scanner_path_whitelist_from_raw(raw.as_ref())?,
    ))
}

pub(super) async fn replace_scanner_path_whitelist(
    state: &AppState,
    paths: Vec<String>,
) -> Result<ScannerPathWhitelist, ScannerError> {
    let paths = normalize_scanner_path_whitelist(paths)?;
    let _guard = state.security.scanner_settings_update_lock.lock().await;
    let raw = state.storage.store.scanner_settings_raw().await?;
    let stored = with_path_whitelist(raw.as_ref(), &paths);
    state.storage.store.save_scanner_settings(&stored).await?;
    Ok(scanner_path_whitelist_payload(paths))
}

pub(super) async fn resolve_scanner_false_positive(
    state: &AppState,
    ip: &str,
    path: &str,
) -> Result<ScannerFalsePositiveResult, ScannerError> {
    let ip = normalize_scanner_ip(ip);
    if ip.is_empty() {
        return Err(ScannerError::BadRequest("IP is required".to_string()));
    }
    let path = normalize_scanner_whitelist_entry(path)?;
    let _guard = state.security.scanner_settings_update_lock.lock().await;
    let raw = state.storage.store.scanner_settings_raw().await?;
    let mut paths = scanner_path_whitelist_from_raw(raw.as_ref())?;
    let added = !paths.contains(&path);
    if added {
        paths.push(path.clone());
    }
    let stored = with_path_whitelist(raw.as_ref(), &paths);
    let unblocked = state
        .storage
        .store
        .save_scanner_settings_and_remove_blacklist(&stored, &ip)
        .await?;
    Ok(ScannerFalsePositiveResult {
        ip,
        path,
        added,
        unblocked,
    })
}

fn with_path_whitelist(raw: Option<&Value>, paths: &[String]) -> Value {
    let mut stored = raw.and_then(Value::as_object).cloned().unwrap_or_default();
    stored.insert(PATH_WHITELIST_FIELD.to_string(), json!(paths));
    Value::Object(stored)
}

fn scanner_path_whitelist_payload(paths: Vec<String>) -> ScannerPathWhitelist {
    ScannerPathWhitelist {
        paths,
        default_paths: default_scanner_path_whitelist(),
    }
}
