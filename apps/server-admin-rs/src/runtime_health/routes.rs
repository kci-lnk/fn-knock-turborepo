use std::{
    io::{Cursor, Write},
    net::IpAddr,
    path::Path,
    sync::OnceLock,
};

use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, Query, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use regex::Regex;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{app_version::APP_LOCAL_VERSION, response, state::AppState, time_utils};

const MAX_ARCHIVE_BYTES: usize = 4 * 1024 * 1024;
const MAX_LOG_EXPORT_BYTES: usize = 512 * 1024;
const MAX_CRASH_EXPORT_BYTES: usize = 256 * 1024;
const LOG_CUTOFF_MS: i64 = 24 * 60 * 60 * 1000;
const DEFAULT_LOG_VIEW_LIMIT: usize = 200;
const MAX_LOG_VIEW_LIMIT: usize = 500;

#[derive(Debug, Deserialize)]
struct RuntimeLogQuery {
    limit: Option<usize>,
}

pub(crate) fn runtime_health_routes() -> Router<AppState> {
    Router::new()
        .route("/api/admin/runtime-health", get(runtime_health))
        .route(
            "/api/admin/runtime-health/logs/{component}",
            get(runtime_logs).delete(clear_runtime_logs),
        )
        .route("/api/admin/runtime-health/diagnostics", get(diagnostics))
        .route(
            "/api/admin/runtime-health/diagnostics/archive",
            get(diagnostics_archive),
        )
}

async fn runtime_health(State(state): State<AppState>) -> Response {
    response::ok(state.runtime_health.snapshot().await).into_response()
}

async fn runtime_logs(
    State(state): State<AppState>,
    AxumPath(component): AxumPath<String>,
    Query(query): Query<RuntimeLogQuery>,
) -> Response {
    let Some(file_names) = runtime_log_files(&component) else {
        return response::error(StatusCode::BAD_REQUEST, "Unsupported runtime log component");
    };
    let limit = query
        .limit
        .unwrap_or(DEFAULT_LOG_VIEW_LIMIT)
        .clamp(1, MAX_LOG_VIEW_LIMIT);
    match read_runtime_log_entries(&state.runtime_health.logs_dir(), &file_names, limit) {
        Ok(entries) => response::ok(json!({
            "schema_version": 1,
            "component": component,
            "generated_at": time_utils::now_iso(),
            "entries": entries,
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, component, "failed to read runtime operational log");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to read runtime operational log",
            )
        }
    }
}

async fn clear_runtime_logs(
    State(state): State<AppState>,
    AxumPath(component): AxumPath<String>,
) -> Response {
    if runtime_log_files(&component).is_none() {
        return response::error(StatusCode::BAD_REQUEST, "Unsupported runtime log component");
    }
    match state.runtime_health.clear_operational_log(&component).await {
        Ok(()) => response::ok(json!({
            "component": component,
            "cleared_at": time_utils::now_iso(),
        }))
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, component, "failed to clear runtime operational log");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to clear runtime operational log",
            )
        }
    }
}

async fn diagnostics(State(state): State<AppState>) -> Response {
    match build_diagnostics(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to build runtime diagnostics");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build runtime diagnostics",
            )
        }
    }
}

async fn diagnostics_archive(State(state): State<AppState>) -> Response {
    match build_archive(&state).await {
        Ok(bytes) if bytes.len() <= MAX_ARCHIVE_BYTES => {
            let filename = format!("fn-knock-diagnostics-{}.zip", time_utils::now_ms());
            let mut response = Response::new(Body::from(bytes));
            response.headers_mut().insert(
                header::CONTENT_TYPE,
                header::HeaderValue::from_static("application/zip"),
            );
            if let Ok(value) =
                header::HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            {
                response
                    .headers_mut()
                    .insert(header::CONTENT_DISPOSITION, value);
            }
            response
        }
        Ok(_) => response::error(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Diagnostics archive exceeds the 4 MiB limit",
        ),
        Err(error) => {
            tracing::warn!(%error, "failed to build runtime diagnostics archive");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to export runtime diagnostics",
            )
        }
    }
}

async fn build_diagnostics(state: &AppState) -> anyhow::Result<Value> {
    let snapshot = state.runtime_health.snapshot().await;
    let first = state
        .store
        .list_system_events(1, 100, "", None, None, Some("RUNTIME_MONITOR"))
        .await?;
    let second = state
        .store
        .list_system_events(2, 100, "", None, None, Some("RUNTIME_MONITOR"))
        .await?;
    let mut events = first
        .get("events")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    events.extend(
        second
            .get("events")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .cloned(),
    );
    events.truncate(200);
    let value = json!({
        "schema_version": 1,
        "generated_at": time_utils::now_iso(),
        "version": APP_LOCAL_VERSION,
        "commit": option_env!("FN_KNOCK_GIT_COMMIT").unwrap_or(""),
        "platform": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "runtime_target": state.settings.runtime_target,
        },
        "runtime": snapshot,
        "recent_runtime_events": events,
        "collection": {
            "includes": ["component health", "runtime lifecycle events", "bounded operational logs"],
            "excludes": ["requests", "WAF details", "authentication records", "configuration", "environment", "certificates", "database"],
        },
    });
    Ok(sanitize_value(value))
}

async fn build_archive(state: &AppState) -> anyhow::Result<Vec<u8>> {
    let diagnostics = serde_json::to_vec_pretty(&build_diagnostics(state).await?)?;
    let logs_dir = state.runtime_health.logs_dir();
    let mut files: Vec<(String, Vec<u8>)> = vec![("diagnostics.json".to_string(), diagnostics)];

    for (archive_name, file_names) in [
        (
            "logs/management.jsonl",
            ["management.jsonl.1", "management.jsonl"],
        ),
        ("logs/gateway.jsonl", ["gateway.jsonl.1", "gateway.jsonl"]),
        (
            "logs/supervisor.jsonl",
            ["supervisor.jsonl.1", "supervisor.jsonl"],
        ),
    ] {
        let data = export_jsonl(&logs_dir, &file_names)?;
        if !data.is_empty() {
            files.push((archive_name.to_string(), data));
        }
    }
    for (archive_name, file_name) in [
        ("crash/management.log", "management-crash.log"),
        ("crash/gateway.log", "gateway-crash.log"),
    ] {
        if let Some(data) = export_crash(&logs_dir.join(file_name))? {
            files.push((archive_name.to_string(), data));
        }
    }
    files.push((
        "README.txt".to_string(),
        format!(
            "fn-knock diagnostics\nGenerated: {}\n\nContains only component health, runtime lifecycle events, and bounded fn-knock operational logs.\nRequest logs, WAF details, authentication/session records, raw configuration, environment variables, certificates, databases, and complete platform logs are excluded.\nAll structured records are defensively redacted during export. Log coverage is limited to the latest 24 hours and may be shorter after rotation. Exit-code availability depends on the platform supervisor.\n",
            time_utils::now_iso()
        )
        .into_bytes(),
    ));

    let manifest_files = files
        .iter()
        .map(|(name, data)| {
            json!({
                "name": name,
                "size": data.len(),
                "sha256": hex::encode(Sha256::digest(data)),
            })
        })
        .collect::<Vec<_>>();
    let manifest = serde_json::to_vec_pretty(&json!({
        "schema_version": 1,
        "generated_at": time_utils::now_iso(),
        "files": manifest_files,
    }))?;

    let cursor = Cursor::new(Vec::with_capacity(2 * 1024 * 1024));
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o600);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&manifest)?;
    for (name, data) in files {
        // Names are compile-time constants. No request or filesystem path is
        // ever used as an archive member name.
        zip.start_file(name, options)?;
        zip.write_all(&data)?;
    }
    Ok(zip.finish()?.into_inner())
}

fn export_jsonl(directory: &Path, files: &[&str; 2]) -> anyhow::Result<Vec<u8>> {
    let cutoff = time_utils::now_ms() - LOG_CUTOFF_MS;
    let mut output = Vec::new();
    let mut omitted = 0_u64;
    for name in files {
        let path = directory.join(name);
        let Ok(raw) = std::fs::read(path) else {
            continue;
        };
        let raw = String::from_utf8_lossy(&raw);
        for line in raw.lines() {
            let Ok(mut value) = serde_json::from_str::<Value>(line) else {
                omitted += 1;
                continue;
            };
            let timestamp = value
                .get("time")
                .and_then(Value::as_str)
                .and_then(time_utils::parse_iso_ms);
            if timestamp.is_none() {
                omitted += 1;
                continue;
            }
            if timestamp.is_some_and(|timestamp| timestamp < cutoff) {
                continue;
            }
            value = sanitize_value(value);
            let mut encoded = serde_json::to_vec(&value)?;
            if encoded.len() > 8 * 1024 {
                omitted += 1;
                continue;
            }
            encoded.push(b'\n');
            output.extend(encoded);
        }
    }
    if omitted > 0 {
        let notice = json!({
            "time": time_utils::now_iso(),
            "level": "WARN",
            "component": "diagnostics_export",
            "event": "invalid_log_lines_omitted",
            "reason_code": "invalid_or_oversized_jsonl",
            "fields": { "count": omitted },
        });
        output.extend(serde_json::to_vec(&notice)?);
        output.push(b'\n');
    }
    Ok(tail_at_line_boundary(output, MAX_LOG_EXPORT_BYTES))
}

fn runtime_log_files(component: &str) -> Option<[&'static str; 2]> {
    match component {
        "management" => Some(["management.jsonl.1", "management.jsonl"]),
        "gateway_process" => Some(["gateway.jsonl.1", "gateway.jsonl"]),
        _ => None,
    }
}

fn read_runtime_log_entries(
    directory: &Path,
    files: &[&str; 2],
    limit: usize,
) -> anyhow::Result<Vec<Value>> {
    let exported = export_jsonl(directory, files)?;
    let mut entries = String::from_utf8_lossy(&exported)
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .rev()
        .take(limit.clamp(1, MAX_LOG_VIEW_LIMIT))
        .collect::<Vec<_>>();
    entries.reverse();
    Ok(entries)
}

fn export_crash(path: &Path) -> anyhow::Result<Option<Vec<u8>>> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Ok(None);
    };
    if raw.trim().is_empty() {
        return Ok(None);
    }
    let sanitized = raw
        .lines()
        .map(redact_string)
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes();
    Ok(Some(tail_at_line_boundary(
        sanitized,
        MAX_CRASH_EXPORT_BYTES,
    )))
}

fn tail_at_line_boundary(data: Vec<u8>, max: usize) -> Vec<u8> {
    if data.len() <= max {
        return data;
    }
    let start = data.len() - max;
    let boundary = data[start..]
        .iter()
        .position(|byte| *byte == b'\n')
        .map(|offset| start + offset + 1)
        .unwrap_or(start);
    data[boundary..].to_vec()
}

fn sanitize_value(value: Value) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .into_iter()
                .filter_map(|(key, value)| {
                    if sensitive_key(&key) {
                        None
                    } else {
                        Some((key, sanitize_value(value)))
                    }
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(values) => Value::Array(values.into_iter().map(sanitize_value).collect()),
        Value::String(value) => Value::String(redact_string(&value)),
        other => other,
    }
}

fn sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    [
        "password",
        "passwd",
        "secret",
        "token",
        "cookie",
        "authorization",
        "private_key",
        "certificate",
        "session",
    ]
    .iter()
    .any(|needle| key.contains(needle))
}

pub(super) fn redact_string(value: &str) -> String {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    if [
        "authorization:",
        "authorization=",
        "cookie:",
        "cookie=",
        "set-cookie:",
        "bearer ",
        "password=",
        "secret=",
        "token=",
        "private_key",
        "begin private key",
        "begin certificate",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return "[redacted]".to_string();
    }
    if let Ok(parsed) = url::Url::parse(trimmed) {
        return truncate_string(
            format!("{}://[host]{}", parsed.scheme(), parsed.path()),
            512,
        );
    }
    if let Ok(ip) = trimmed.parse::<IpAddr>()
        && !ip.is_loopback()
    {
        return "[ip]".to_string();
    }
    if Path::new(trimmed).is_absolute() {
        return Path::new(trimmed)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("[path]")
            .to_string();
    }
    if domain_regex().is_match(trimmed) {
        return "[host]".to_string();
    }
    let with_urls = url_regex().replace_all(trimmed, "https://[host]");
    let with_ips = ipv4_regex().replace_all(&with_urls, |captures: &regex::Captures<'_>| {
        captures
            .get(0)
            .and_then(|value| value.as_str().parse::<IpAddr>().ok())
            .filter(|ip| !ip.is_loopback())
            .map(|_| "[ip]".to_string())
            .unwrap_or_else(|| captures[0].to_string())
    });
    let without_paths =
        absolute_path_regex().replace_all(&with_ips, |captures: &regex::Captures<'_>| {
            captures
                .get(1)
                .or_else(|| captures.get(2))
                .map(|value| value.as_str().to_string())
                .unwrap_or_else(|| "[path]".to_string())
        });
    truncate_string(without_paths.into_owned(), 512)
}

fn truncate_string(mut value: String, max: usize) -> String {
    if value.len() <= max {
        return value;
    }
    let mut boundary = max;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
}

fn domain_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?i)^(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}(?::\d+)?$")
            .expect("valid domain redaction regex")
    })
}

fn url_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r#"(?i)https?://[^\s\"']+"#).expect("valid URL regex"))
}

fn ipv4_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").expect("valid IP regex"))
}

fn absolute_path_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r"(?:[A-Za-z]:\\(?:[^\\\s:]+\\)*([^\\\s:]+)|/(?:[^/\s:]+/)+([^/\s:]+))")
            .expect("valid absolute path regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    async fn diagnostics_test_state() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().unwrap();
        let mut settings = crate::settings::Settings::from_env();
        settings.data_dir = directory.path().join("data");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.legacy_redis_url.clear();
        settings.go_backend_grpc_addr = "127.0.0.1:1".to_string();
        settings.internal_rpc_token = "diagnostics-test-token".to_string();
        settings.altcha_hmac_key = Some("diagnostics-altcha-key".to_string());
        let state = AppState::new(settings).await.unwrap();
        (directory, state)
    }

    #[test]
    fn defensive_redaction_covers_canaries() {
        let value = sanitize_value(json!({
            "token": "canary-token",
            "url": "https://user:pass@example.com/private?q=canary#fragment",
            "ip": "203.0.113.7",
            "domain": "private.example.com",
            "path": "/Users/example/fn-knock/config.json",
            "cookie_line": "Cookie: session=canary",
        }));
        let encoded = serde_json::to_string(&value).unwrap();
        for canary in [
            "canary-token",
            "user:pass",
            "example.com",
            "203.0.113.7",
            "/Users/example",
            "session=canary",
        ] {
            assert!(
                !encoded.contains(canary),
                "canary leaked: {canary}: {encoded}"
            );
        }
        assert!(encoded.contains("[host]") && encoded.contains("[ip]"));
    }

    #[test]
    fn malformed_jsonl_is_replaced_with_notice() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("management.jsonl"),
            "not-json\n{\"level\":\"INFO\"}\n",
        )
        .unwrap();
        let exported = export_jsonl(
            directory.path(),
            &["management.jsonl.1", "management.jsonl"],
        )
        .unwrap();
        let exported = String::from_utf8(exported).unwrap();
        assert!(exported.contains("invalid_log_lines_omitted"));
        assert!(!exported.contains("not-json"));
        assert!(!exported.contains("\"level\":\"INFO\""));
    }

    #[test]
    fn runtime_log_view_only_accepts_fixed_process_components() {
        assert_eq!(
            runtime_log_files("management"),
            Some(["management.jsonl.1", "management.jsonl"])
        );
        assert_eq!(
            runtime_log_files("gateway_process"),
            Some(["gateway.jsonl.1", "gateway.jsonl"])
        );
        assert_eq!(runtime_log_files("../../config"), None);
        assert_eq!(runtime_log_files("gateway_dataplane"), None);
    }

    #[tokio::test]
    async fn component_log_clear_preserves_crash_and_other_component_logs() {
        let (_directory, state) = diagnostics_test_state().await;
        let logs = state.runtime_health.logs_dir();
        state.runtime_health.flush_operational_log().await;
        std::fs::write(logs.join("management.jsonl.1"), b"old management\n").unwrap();
        std::fs::write(logs.join("gateway.jsonl"), b"gateway\n").unwrap();
        std::fs::write(logs.join("management-crash.log"), b"crash\n").unwrap();

        state
            .runtime_health
            .clear_operational_log("management")
            .await
            .unwrap();

        assert_eq!(
            std::fs::metadata(logs.join("management.jsonl"))
                .unwrap()
                .len(),
            0
        );
        assert!(!logs.join("management.jsonl.1").exists());
        assert_eq!(
            std::fs::read(logs.join("gateway.jsonl")).unwrap(),
            b"gateway\n"
        );
        assert_eq!(
            std::fs::read(logs.join("management-crash.log")).unwrap(),
            b"crash\n"
        );
        assert!(
            state
                .runtime_health
                .clear_operational_log("../../config")
                .await
                .is_err()
        );
    }

    #[test]
    fn runtime_log_view_returns_latest_sanitized_entries() {
        let directory = tempfile::tempdir().unwrap();
        let lines = (0..4)
            .map(|index| {
                json!({
                    "time": time_utils::now_iso(),
                    "level": "INFO",
                    "component": "management",
                    "event": format!("event-{index}"),
                    "reason_code": "ready",
                    "fields": {
                        "url": "https://user:pass@example.com/private?token=canary"
                    }
                })
                .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(directory.path().join("management.jsonl"), lines).unwrap();

        let entries = read_runtime_log_entries(
            directory.path(),
            &["management.jsonl.1", "management.jsonl"],
            2,
        )
        .unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0]["event"], "event-2");
        assert_eq!(entries[1]["event"], "event-3");
        let encoded = serde_json::to_string(&entries).unwrap();
        assert!(!encoded.contains("example.com"));
        assert!(!encoded.contains("user:pass"));
        assert!(!encoded.contains("canary"));
    }

    #[test]
    fn crash_lines_redact_sensitive_markers_and_inline_paths() {
        assert_eq!(redact_string("panic: token=canary"), "[redacted]");
        let redacted = redact_string("at /Users/example/project/src/main.rs:42");
        assert!(!redacted.contains("/Users/example"));
        assert!(redacted.contains("main.rs:42"));
    }

    #[tokio::test]
    async fn archive_has_fixed_members_hashes_limits_and_redaction() {
        let (_directory, state) = diagnostics_test_state().await;
        let logs = state.runtime_health.logs_dir();
        std::fs::write(
            logs.join("gateway.jsonl"),
            format!(
                "{}\nnot-json\n",
                json!({
                    "time": time_utils::now_iso(),
                    "level": "INFO",
                    "component": "gateway_process",
                    "event": "ready",
                    "reason_code": "serving",
                    "fields": {
                        "listener": "https://user:pass@example.com/private?q=canary#fragment"
                    }
                })
            ),
        )
        .unwrap();
        std::fs::write(
            logs.join("gateway-crash.log"),
            "panic token=crash-canary at /Users/example/project/main.go:42\n",
        )
        .unwrap();

        let bytes = build_archive(&state).await.unwrap();
        assert!(bytes.len() <= MAX_ARCHIVE_BYTES);
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let names = (0..archive.len())
            .map(|index| archive.by_index(index).unwrap().name().to_string())
            .collect::<Vec<_>>();
        assert!(names.contains(&"manifest.json".to_string()));
        assert!(names.contains(&"diagnostics.json".to_string()));
        assert!(names.contains(&"logs/gateway.jsonl".to_string()));
        assert!(names.contains(&"crash/gateway.log".to_string()));
        assert!(names.contains(&"README.txt".to_string()));
        assert!(
            names
                .iter()
                .all(|name| !name.contains("..") && !name.starts_with('/'))
        );

        let manifest: Value = {
            let mut raw = String::new();
            archive
                .by_name("manifest.json")
                .unwrap()
                .read_to_string(&mut raw)
                .unwrap();
            serde_json::from_str(&raw).unwrap()
        };
        for entry in manifest["files"].as_array().unwrap() {
            let name = entry["name"].as_str().unwrap();
            let mut data = Vec::new();
            archive
                .by_name(name)
                .unwrap()
                .read_to_end(&mut data)
                .unwrap();
            assert_eq!(entry["size"].as_u64(), Some(data.len() as u64));
            let hash = hex::encode(Sha256::digest(&data));
            assert_eq!(entry["sha256"].as_str(), Some(hash.as_str()));
            let text = String::from_utf8_lossy(&data);
            assert!(!text.contains("example.com"));
            assert!(!text.contains("crash-canary"));
            assert!(!text.contains("/Users/example"));
        }
        let mut gateway_log = String::new();
        archive
            .by_name("logs/gateway.jsonl")
            .unwrap()
            .read_to_string(&mut gateway_log)
            .unwrap();
        assert!(gateway_log.contains("invalid_log_lines_omitted"));
    }
}
