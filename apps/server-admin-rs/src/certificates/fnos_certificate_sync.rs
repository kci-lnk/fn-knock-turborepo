use std::{
    collections::BTreeSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, PermissionsExt};

use anyhow::{Context, anyhow, bail};
use axum::{
    Json, Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use x509_parser::{extensions::GeneralName, pem::parse_x509_pem};

use crate::{response, runtime_profile, ssl, state::AppState, time_utils};

const CONFIG_KEY: &str = "fnos_certificate_sync";
const CERT_ROOT: &str = "/usr/trim/var/trim_connect/ssls";
const NETWORK_CERT_INDEX: &str = "/usr/trim/etc/network_cert_all.conf";
const NETWORK_GATEWAY_INDEX: &str = "/usr/trim/etc/network_gateway_cert.conf";
const AUTO_SYNC_DEBOUNCE: Duration = Duration::from_secs(3);
const BACKUP_KEEP_COUNT: usize = 10;

#[derive(Clone, Debug, Deserialize, Serialize)]
struct FnosCertRow {
    id: i64,
    domain: String,
    san: Option<String>,
    valid_from: Option<i64>,
    valid_to: Option<i64>,
    encrypt_type: Option<String>,
    issued_by: Option<String>,
    is_default: Option<i16>,
    renewal: Option<i16>,
    source: Option<String>,
    private_key: Option<String>,
    certificate: Option<String>,
    issuer_certificate: Option<String>,
    status: Option<String>,
    created_time: Option<i64>,
    updated_time: Option<i64>,
}

#[derive(Clone, Debug)]
struct ParsedCertificate {
    domains: Vec<String>,
    valid_from: i64,
    valid_to: i64,
    encrypt_type: String,
    issued_by: String,
    chain_digest: String,
    public_key_digest: String,
    fingerprint: String,
}

#[derive(Clone, Debug)]
struct LocalCandidate {
    id: String,
    label: String,
    updated_at: String,
    cert: String,
    key: String,
    parsed: Option<ParsedCertificate>,
    valid: bool,
}

#[derive(Clone, Debug)]
struct ComparedTarget {
    row: FnosCertRow,
    status: String,
    reason: Option<String>,
    target: Option<ParsedCertificate>,
    local: Option<LocalCandidate>,
}

#[derive(Debug, Deserialize)]
struct UpdateConfigBody {
    auto_sync_enabled: bool,
}

#[derive(Debug, Deserialize)]
struct SyncBody {
    #[serde(default)]
    target_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct SyncSummary {
    synced: usize,
    skipped: usize,
    failed: usize,
    rolled_back: bool,
}

#[derive(Debug)]
struct SyncExecutionError {
    source: anyhow::Error,
    target_ids: Vec<String>,
}

impl std::fmt::Display for SyncExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.source.fmt(formatter)
    }
}

impl std::error::Error for SyncExecutionError {}

impl SyncExecutionError {
    fn new(source: impl Into<anyhow::Error>, target_ids: &[String]) -> Self {
        Self {
            source: source.into(),
            target_ids: target_ids.to_vec(),
        }
    }
}

pub fn fnos_certificate_sync_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/fnos_certificate_sync/details",
            get(get_details),
        )
        .route(
            "/api/admin/config/fnos_certificate_sync",
            post(update_config),
        )
        .route(
            "/api/admin/config/fnos_certificate_sync/sync",
            post(sync_now),
        )
}

pub fn start_fnos_certificate_sync_tasks(state: AppState) {
    tokio::spawn(async move {
        if auto_sync_enabled(&state).await {
            state.fnos_certificate_sync_notify.notify_one();
        }
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = state.fnos_certificate_sync_notify.notified() => {
                    loop {
                        tokio::select! {
                            _ = state.shutdown.cancelled() => return,
                            _ = state.fnos_certificate_sync_notify.notified() => continue,
                            _ = tokio::time::sleep(AUTO_SYNC_DEBOUNCE) => break,
                        }
                    }
                    if !auto_sync_enabled(&state).await {
                        continue;
                    }
                    let _guard = state.fnos_certificate_sync_lock.lock().await;
                    let local_config = match state.store.get_config().await {
                        Ok(value) => value,
                        Err(error) => {
                            record_failure(&state, &error.to_string(), &[]).await;
                            continue;
                        }
                    };
                    record_running(&state).await;
                    let data_dir = state.settings.data_dir.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        perform_sync(&data_dir, &local_config, &[])
                    }).await;
                    match result {
                        Ok(Ok(summary)) => record_success(&state, &summary).await,
                        Ok(Err(error)) => {
                            tracing::warn!(%error, "automatic fnOS certificate sync failed");
                            record_failure(&state, &error.to_string(), &error.target_ids).await;
                        }
                        Err(error) => record_failure(&state, &error.to_string(), &[]).await,
                    }
                }
            }
        }
    });
}

pub fn notify_certificate_library_changed(state: &AppState) {
    state.fnos_certificate_sync_notify.notify_one();
}

async fn get_details(State(state): State<AppState>) -> Response {
    match build_details(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => response::error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        ),
    }
}

async fn update_config(
    State(state): State<AppState>,
    Json(body): Json<UpdateConfigBody>,
) -> Response {
    let mut config = match state.store.get_config().await {
        Ok(value) => value,
        Err(error) => {
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            );
        }
    };
    let previous = config
        .pointer("/fnos_certificate_sync/auto_sync_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    config[CONFIG_KEY] = json!({ "auto_sync_enabled": body.auto_sync_enabled });
    if let Err(error) = state.store.save_config(&config).await {
        return response::error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        );
    }
    if body.auto_sync_enabled && !previous {
        state.fnos_certificate_sync_notify.notify_one();
    }
    match build_details(&state).await {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => response::error(
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            error.to_string(),
        ),
    }
}

async fn sync_now(State(state): State<AppState>, Json(body): Json<SyncBody>) -> Response {
    let ids = match parse_target_ids(&body.target_ids) {
        Ok(ids) => ids,
        Err(error) => {
            return response::error(axum::http::StatusCode::BAD_REQUEST, error.to_string());
        }
    };
    let _guard = state.fnos_certificate_sync_lock.lock().await;
    let local_config = match state.store.get_config().await {
        Ok(value) => value,
        Err(error) => {
            return response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            );
        }
    };
    record_running(&state).await;
    let data_dir = state.settings.data_dir.clone();
    let requested_ids = ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    match tokio::task::spawn_blocking(move || perform_sync(&data_dir, &local_config, &ids)).await {
        Ok(Ok(summary)) => {
            record_success(&state, &summary).await;
            match build_details(&state).await {
                Ok(details) => {
                    response::ok(json!({ "summary": summary, "details": details })).into_response()
                }
                Err(error) => response::error(
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    error.to_string(),
                ),
            }
        }
        Ok(Err(error)) => {
            record_failure(&state, &error.to_string(), &error.target_ids).await;
            response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            )
        }
        Err(error) => {
            record_failure(&state, &error.to_string(), &requested_ids).await;
            response::error(
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                error.to_string(),
            )
        }
    }
}

async fn build_details(state: &AppState) -> anyhow::Result<Value> {
    let config = state.store.get_config().await?;
    let local_config = config.clone();
    let comparison = tokio::task::spawn_blocking(move || compare_all(&local_config)).await;
    let (environment_available, availability_reason, compared) = match comparison {
        Ok(Ok(compared)) => (true, Value::Null, compared),
        Ok(Err(error)) => (
            false,
            json!(sanitize_availability_error(&error)),
            Vec::new(),
        ),
        Err(error) => (
            false,
            json!(sanitize_availability_error(&anyhow!(error))),
            Vec::new(),
        ),
    };
    let runtime = state.fnos_certificate_sync_status.read().await.clone();
    let failed_ids = runtime
        .get("failed_target_ids")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let items = compared
        .into_iter()
        .map(|item| compared_target_json(item, &failed_ids))
        .collect::<Vec<_>>();
    let syncable_count = items
        .iter()
        .filter(|item| item["status"] == "syncable" || item["status"] == "sync_failed")
        .count();
    let up_to_date_count = items
        .iter()
        .filter(|item| item["status"] == "up_to_date")
        .count();
    Ok(json!({
        "availability": {
            "available": environment_available && runtime_profile::get_runtime_capabilities(&runtime_profile::get_runtime_profile(state)).fnos_certificate_sync_available,
            "reason": availability_reason
        },
        "config": { "auto_sync_enabled": config.pointer("/fnos_certificate_sync/auto_sync_enabled").and_then(Value::as_bool).unwrap_or(false) },
        "runtime": runtime,
        "summary": { "total": items.len(), "syncable": syncable_count, "up_to_date": up_to_date_count },
        "certificates": items
    }))
}

fn sanitize_availability_error(error: &anyhow::Error) -> String {
    let message = error.to_string();
    if message.contains("PostgreSQL") || message.contains("psql") {
        "Unable to read the fnOS certificate database".to_string()
    } else if message.contains("JSON") || message.contains("network_cert_all") {
        "Unable to read the fnOS certificate index".to_string()
    } else if message.contains("certificate") || message.contains("private key") {
        "The fnOS certificate files are unavailable or inconsistent".to_string()
    } else {
        "The fnOS certificate synchronization environment is unavailable".to_string()
    }
}

fn compared_target_json(item: ComparedTarget, failed_ids: &BTreeSet<&str>) -> Value {
    let id = item.row.id.to_string();
    let mut status = item.status;
    if failed_ids.contains(id.as_str()) && status == "syncable" {
        status = "sync_failed".to_string();
    }
    json!({
        "target_id": id,
        "domain": item.row.domain,
        "san": split_san(item.row.san.as_deref().unwrap_or("")),
        "source": item.row.source.unwrap_or_default(),
        "renewal": item.row.renewal.unwrap_or(0) == 1,
        "valid_from": item.row.valid_from,
        "valid_to": item.row.valid_to,
        "fingerprint": item.target.as_ref().map(|value| value.fingerprint.clone()),
        "status": status,
        "reason": item.reason,
        "local": item.local.map(|value| json!({
            "id": value.id,
            "label": value.label,
            "valid_from": value.parsed.as_ref().map(|parsed| parsed.valid_from),
            "valid_to": value.parsed.as_ref().map(|parsed| parsed.valid_to),
            "fingerprint": value.parsed.as_ref().map(|parsed| parsed.fingerprint.clone())
        }))
    })
}

fn compare_all(config: &Value) -> anyhow::Result<Vec<ComparedTarget>> {
    let rows = read_fnos_rows()?;
    let network_index = read_json_array(Path::new(NETWORK_CERT_INDEX))?;
    let candidates = local_candidates(config);
    Ok(rows
        .into_iter()
        .map(|row| compare_target(row, &network_index, &candidates))
        .collect())
}

fn compare_target(
    row: FnosCertRow,
    network_index: &[Value],
    candidates: &[LocalCandidate],
) -> ComparedTarget {
    if row.source.as_deref() == Some("system") {
        return ComparedTarget {
            row,
            status: "protected".into(),
            reason: None,
            target: None,
            local: None,
        };
    }
    let db_domains = normalize_domains(split_san(
        row.san
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&row.domain),
    ));
    let Some(cert_path) = row.certificate.as_deref() else {
        return invalid_target(row, "fnOS certificate path is missing");
    };
    let Some(key_path) = row.private_key.as_deref() else {
        return invalid_target(row, "fnOS private key path is missing");
    };
    if validate_target_path(Path::new(cert_path)).is_err()
        || validate_target_path(Path::new(key_path)).is_err()
    {
        return invalid_target(row, "fnOS certificate path is unsafe");
    }
    let cert_pem = match fs::read_to_string(cert_path) {
        Ok(value) => value,
        Err(_) => return invalid_target(row, "fnOS certificate file cannot be read"),
    };
    let key_pem = match fs::read_to_string(key_path) {
        Ok(value) => value,
        Err(_) => return invalid_target(row, "fnOS private key file cannot be read"),
    };
    let target = match parse_certificate(&cert_pem) {
        Ok(value) if ssl::validate_ssl_cert(&cert_pem, &key_pem).is_ok() => value,
        _ => return invalid_target(row, "fnOS certificate or private key is invalid"),
    };
    if db_domains != target.domains {
        return invalid_target(row, "fnOS database SAN does not match its certificate file");
    }
    if row.valid_from != Some(target.valid_from) || row.valid_to != Some(target.valid_to) {
        return invalid_target(
            row,
            "fnOS database validity does not match its certificate file",
        );
    }
    let index_matches = network_index
        .iter()
        .filter(|entry| {
            entry.get("certificate").and_then(Value::as_str) == Some(cert_path)
                && entry.get("privateKey").and_then(Value::as_str) == Some(key_path)
                && normalize_domains(
                    entry
                        .get("san")
                        .and_then(Value::as_array)
                        .map(|items| {
                            items
                                .iter()
                                .filter_map(Value::as_str)
                                .map(str::to_string)
                                .collect()
                        })
                        .unwrap_or_default(),
                ) == db_domains
                && entry.get("validFrom").and_then(Value::as_i64) == row.valid_from
                && entry.get("validTo").and_then(Value::as_i64) == row.valid_to
        })
        .count();
    if index_matches != 1 {
        return invalid_target(row, "fnOS certificate index is inconsistent");
    }
    let same_domain = candidates
        .iter()
        .filter(|candidate| {
            candidate
                .parsed
                .as_ref()
                .is_some_and(|parsed| parsed.domains == db_domains)
        })
        .collect::<Vec<_>>();
    if same_domain.is_empty() {
        return ComparedTarget {
            row,
            status: "unmatched".into(),
            reason: None,
            target: Some(target),
            local: None,
        };
    }
    let valid = same_domain
        .iter()
        .filter(|candidate| candidate.valid)
        .cloned()
        .cloned()
        .collect::<Vec<_>>();
    let Some(local) = select_best_local(valid) else {
        return ComparedTarget {
            row,
            status: "source_invalid".into(),
            reason: Some("Matching local certificates are invalid or expired".into()),
            target: Some(target),
            local: same_domain.first().map(|value| (*value).clone()),
        };
    };
    let local_parsed = local
        .parsed
        .as_ref()
        .expect("valid candidate has parsed certificate");
    let status = if local_parsed.chain_digest == target.chain_digest
        && local_parsed.public_key_digest == target.public_key_digest
    {
        "up_to_date"
    } else {
        "syncable"
    };
    ComparedTarget {
        row,
        status: status.into(),
        reason: None,
        target: Some(target),
        local: Some(local),
    }
}

fn select_best_local(mut candidates: Vec<LocalCandidate>) -> Option<LocalCandidate> {
    candidates.sort_by(|left, right| {
        right
            .parsed
            .as_ref()
            .map(|value| value.valid_to)
            .cmp(&left.parsed.as_ref().map(|value| value.valid_to))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| right.id.cmp(&left.id))
    });
    candidates.into_iter().next()
}

fn invalid_target(row: FnosCertRow, reason: &str) -> ComparedTarget {
    ComparedTarget {
        row,
        status: "target_invalid".into(),
        reason: Some(reason.into()),
        target: None,
        local: None,
    }
}

fn local_candidates(config: &Value) -> Vec<LocalCandidate> {
    config
        .pointer("/ssl/certificates")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| {
            let cert = item
                .get("cert")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let key = item
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let parsed = parse_certificate(&cert).ok();
            let now = time_utils::now_ms();
            let valid = parsed
                .as_ref()
                .is_some_and(|value| value.valid_from <= now && now < value.valid_to)
                && ssl::validate_ssl_cert(&cert, &key).is_ok();
            LocalCandidate {
                id: item
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                label: item
                    .get("label")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                updated_at: item
                    .get("updated_at")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                cert,
                key,
                parsed,
                valid,
            }
        })
        .collect()
}

fn parse_certificate(pem_text: &str) -> anyhow::Result<ParsedCertificate> {
    let mut remaining = pem_text.as_bytes();
    let mut chain_hasher = Sha256::new();
    let mut first = None;
    while !remaining.is_empty() {
        let Ok((rest, pem)) = parse_x509_pem(remaining) else {
            if remaining.iter().all(u8::is_ascii_whitespace) {
                break;
            }
            bail!("certificate PEM contains invalid trailing content")
        };
        let cert = pem.parse_x509().context("parse X.509 certificate")?;
        chain_hasher.update((pem.contents.len() as u64).to_be_bytes());
        chain_hasher.update(&pem.contents);
        if first.is_none() {
            let mut domains = Vec::new();
            if let Ok(Some(san)) = cert.subject_alternative_name() {
                for name in &san.value.general_names {
                    if let GeneralName::DNSName(value) = name {
                        domains.push((*value).to_string());
                    }
                }
            }
            if domains.is_empty()
                && let Some(cn) = cert
                    .subject()
                    .iter_common_name()
                    .next()
                    .and_then(|value| value.as_str().ok())
            {
                domains.push(cn.to_string());
            }
            let issued_by = cert
                .issuer()
                .iter_common_name()
                .next()
                .and_then(|value| value.as_str().ok())
                .map(str::to_string)
                .unwrap_or_else(|| cert.issuer().to_string());
            let algorithm = cert.public_key().algorithm.algorithm.to_id_string();
            let encrypt_type = if algorithm == "1.2.840.113549.1.1.1" {
                "RSA"
            } else if algorithm == "1.2.840.10045.2.1" {
                "ECDSA"
            } else {
                "UNKNOWN"
            }
            .to_string();
            let public_key_digest = hex::encode(Sha256::digest(cert.public_key().raw));
            let fingerprint = hex::encode_upper(Sha256::digest(&pem.contents))
                .as_bytes()
                .chunks(2)
                .map(|part| std::str::from_utf8(part).unwrap_or(""))
                .collect::<Vec<_>>()
                .join(":");
            first = Some((
                normalize_domains(domains),
                cert.validity().not_before.timestamp() * 1000,
                cert.validity().not_after.timestamp() * 1000,
                encrypt_type,
                issued_by,
                public_key_digest,
                fingerprint,
            ));
        }
        remaining = rest;
    }
    let Some((
        domains,
        valid_from,
        valid_to,
        encrypt_type,
        issued_by,
        public_key_digest,
        fingerprint,
    )) = first
    else {
        bail!("certificate PEM is invalid")
    };
    if domains.is_empty() {
        bail!("certificate has no DNS identity")
    }
    Ok(ParsedCertificate {
        domains,
        valid_from,
        valid_to,
        encrypt_type,
        issued_by,
        chain_digest: hex::encode(chain_hasher.finalize()),
        public_key_digest,
        fingerprint,
    })
}

fn split_san(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn normalize_domains(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter_map(|value| normalize_domain(&value))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_domain(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }
    let (wildcard, body) = value
        .strip_prefix("*.")
        .map_or((false, value.as_str()), |body| (true, body));
    let body = idna::domain_to_ascii(body).ok()?;
    Some(if wildcard { format!("*.{body}") } else { body })
}

fn read_fnos_rows() -> anyhow::Result<Vec<FnosCertRow>> {
    let sql = "select coalesce(json_agg(row_to_json(c) order by c.id),'[]'::json)::text from (select id,domain,san,valid_from,valid_to,encrypt_type,issued_by,is_default,renewal,source,private_key,certificate,issuer_certificate,status,created_time,updated_time from public.cert) c;";
    let output = psql(sql)?;
    serde_json::from_str(output.trim()).context("parse fnOS certificate rows")
}

fn psql(sql: &str) -> anyhow::Result<String> {
    let output = Command::new("sudo")
        .args([
            "-u",
            "postgres",
            "psql",
            "-d",
            "trim_connect",
            "-XqAt",
            "-v",
            "ON_ERROR_STOP=1",
            "-c",
            sql,
        ])
        .output()
        .context("run fnOS PostgreSQL CLI")?;
    if !output.status.success() {
        bail!(
            "fnOS PostgreSQL command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    String::from_utf8(output.stdout).context("decode fnOS PostgreSQL output")
}

fn read_json_array(path: &Path) -> anyhow::Result<Vec<Value>> {
    let value: Value = serde_json::from_slice(
        &fs::read(path).with_context(|| format!("read {}", path.display()))?,
    )?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| anyhow!("{} must contain a JSON array", path.display()))
}

fn validate_target_path(path: &Path) -> anyhow::Result<()> {
    validate_fixed_regular_file(path)?;
    let canonical_root = fs::canonicalize(CERT_ROOT)?;
    let canonical = fs::canonicalize(path)?;
    if !canonical.starts_with(canonical_root) {
        bail!("target path is outside fnOS certificate root")
    }
    Ok(())
}

fn validate_fixed_regular_file(path: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!("target is not a regular file")
    }
    Ok(())
}

fn parse_target_ids(values: &[String]) -> anyhow::Result<Vec<i64>> {
    values
        .iter()
        .map(|value| value.parse::<i64>().context("invalid fnOS certificate id"))
        .collect()
}

fn perform_sync(
    data_dir: &Path,
    config: &Value,
    requested_ids: &[i64],
) -> Result<SyncSummary, SyncExecutionError> {
    let compared = compare_all(config).map_err(|error| SyncExecutionError::new(error, &[]))?;
    let requested = requested_ids.iter().copied().collect::<BTreeSet<_>>();
    let selected = compared
        .into_iter()
        .filter(|item| {
            item.status == "syncable" && (requested.is_empty() || requested.contains(&item.row.id))
        })
        .collect::<Vec<_>>();
    let selected_ids = selected
        .iter()
        .map(|item| item.row.id.to_string())
        .collect::<Vec<_>>();
    let skipped = requested.len().saturating_sub(selected.len());
    if selected.is_empty() {
        return Ok(SyncSummary {
            synced: 0,
            skipped,
            failed: 0,
            rolled_back: false,
        });
    }
    let backup_dir = create_backup(data_dir, &selected)
        .map_err(|error| SyncExecutionError::new(error, &selected_ids))?;
    let mut network_index = read_json_array(Path::new(NETWORK_CERT_INDEX))
        .map_err(|error| SyncExecutionError::new(error, &selected_ids))?;
    let mut database_updated = false;
    let result = (|| -> anyhow::Result<()> {
        for item in &selected {
            let local = item
                .local
                .as_ref()
                .ok_or_else(|| anyhow!("local certificate disappeared"))?;
            let parsed = local
                .parsed
                .as_ref()
                .ok_or_else(|| anyhow!("local certificate is invalid"))?;
            let cert_path = Path::new(item.row.certificate.as_deref().unwrap_or(""));
            let key_path = Path::new(item.row.private_key.as_deref().unwrap_or(""));
            atomic_replace_preserving_metadata(cert_path, local.cert.as_bytes())?;
            atomic_replace_preserving_metadata(key_path, local.key.as_bytes())?;
            update_network_index_entry(&mut network_index, &item.row, parsed)?;
        }
        atomic_replace_preserving_metadata(
            Path::new(NETWORK_CERT_INDEX),
            serde_json::to_string(&network_index)?.as_bytes(),
        )?;
        update_database(&selected)?;
        database_updated = true;
        restart_and_verify_services()?;
        verify_sni_mappings(&selected)?;
        let verified = compare_all(config)?;
        for selected_item in &selected {
            let status = verified
                .iter()
                .find(|item| item.row.id == selected_item.row.id)
                .map(|item| item.status.as_str());
            if status != Some("up_to_date") {
                bail!(
                    "fnOS certificate verification failed for id {}",
                    selected_item.row.id
                )
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = rollback_from_backup(&backup_dir, &selected, database_updated);
        let _ = prune_backups(data_dir);
        let source = match rollback {
            Ok(()) => anyhow!("{error}; fnOS changes were rolled back"),
            Err(rollback_error) => anyhow!("{error}; rollback failed: {rollback_error}"),
        };
        return Err(SyncExecutionError::new(source, &selected_ids));
    }
    if let Err(error) = prune_backups(data_dir) {
        tracing::warn!(%error, "failed to prune old fnOS certificate sync backups");
    }
    Ok(SyncSummary {
        synced: selected.len(),
        skipped,
        failed: 0,
        rolled_back: false,
    })
}

fn update_network_index_entry(
    network_index: &mut [Value],
    row: &FnosCertRow,
    parsed: &ParsedCertificate,
) -> anyhow::Result<()> {
    let mut matched = 0;
    for entry in network_index {
        if entry.get("certificate").and_then(Value::as_str) == row.certificate.as_deref()
            && entry.get("privateKey").and_then(Value::as_str) == row.private_key.as_deref()
        {
            entry["validFrom"] = json!(parsed.valid_from);
            entry["validTo"] = json!(parsed.valid_to);
            matched += 1;
        }
    }
    if matched != 1 {
        bail!("fnOS certificate index changed during synchronization")
    }
    Ok(())
}

fn create_backup(data_dir: &Path, selected: &[ComparedTarget]) -> anyhow::Result<PathBuf> {
    let dir = data_dir.join("fnos-certificate-sync/backups").join(format!(
        "{}-{}",
        time_utils::now_ms(),
        Uuid::new_v4()
    ));
    fs::create_dir_all(&dir)?;
    set_private_directory_permissions(&dir)?;
    fs::copy(NETWORK_CERT_INDEX, dir.join("network_cert_all.conf"))?;
    fs::copy(NETWORK_GATEWAY_INDEX, dir.join("network_gateway_cert.conf"))?;
    fs::write(
        dir.join("rows.json"),
        serde_json::to_vec_pretty(&selected.iter().map(|item| &item.row).collect::<Vec<_>>())?,
    )?;
    for item in selected {
        let target_dir = dir.join(item.row.id.to_string());
        fs::create_dir_all(&target_dir)?;
        fs::copy(
            item.row.certificate.as_deref().unwrap_or(""),
            target_dir.join("certificate.pem"),
        )?;
        fs::copy(
            item.row.private_key.as_deref().unwrap_or(""),
            target_dir.join("private.key"),
        )?;
    }
    Ok(dir)
}

fn atomic_replace_preserving_metadata(path: &Path, content: &[u8]) -> anyhow::Result<()> {
    if path == Path::new(NETWORK_CERT_INDEX) {
        validate_fixed_regular_file(path)?;
    } else {
        validate_target_path(path)?;
    }
    let metadata = fs::metadata(path)?;
    let temp = path.with_file_name(format!(
        ".{}.fn-knock-sync-{}",
        path.file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file"),
        Uuid::new_v4()
    ));
    fs::write(&temp, content)?;
    preserve_file_metadata(&temp, &metadata)?;
    fs::File::open(&temp)?.sync_all()?;
    fs::rename(&temp, path)?;
    if let Some(parent) = path.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> anyhow::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_private_directory_permissions(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(unix)]
fn preserve_file_metadata(path: &Path, metadata: &fs::Metadata) -> anyhow::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(metadata.mode()))?;
    if let Err(error) = crate::unix::set_file_owner_from_metadata(path, metadata) {
        let _ = fs::remove_file(path);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(not(unix))]
fn preserve_file_metadata(_path: &Path, _metadata: &fs::Metadata) -> anyhow::Result<()> {
    Ok(())
}

fn update_database(selected: &[ComparedTarget]) -> anyhow::Result<()> {
    let mut sql = String::from("BEGIN;\n");
    for item in selected {
        let parsed = item
            .local
            .as_ref()
            .and_then(|value| value.parsed.as_ref())
            .ok_or_else(|| anyhow!("missing parsed local certificate"))?;
        let update = format!(
            "UPDATE public.cert SET valid_from={},valid_to={},encrypt_type={},issued_by={},status='suc',updated_time={} WHERE id={} AND domain={} AND COALESCE(san,'')={} AND COALESCE(certificate,'')={} AND COALESCE(private_key,'')={}",
            parsed.valid_from,
            parsed.valid_to,
            sql_text_expression(&parsed.encrypt_type),
            sql_text_expression(&parsed.issued_by),
            time_utils::now_ms(),
            item.row.id,
            sql_text_expression(&item.row.domain),
            sql_text_expression(item.row.san.as_deref().unwrap_or("")),
            sql_text_expression(item.row.certificate.as_deref().unwrap_or("")),
            sql_text_expression(item.row.private_key.as_deref().unwrap_or(""))
        );
        sql.push_str(&assert_exactly_one_update(&update));
    }
    sql.push_str("COMMIT;");
    psql(&sql)?;
    Ok(())
}

fn restore_database(selected: &[ComparedTarget]) -> anyhow::Result<()> {
    let mut sql = String::from("BEGIN;\n");
    for item in selected {
        let applied = item
            .local
            .as_ref()
            .and_then(|value| value.parsed.as_ref())
            .ok_or_else(|| anyhow!("missing applied certificate metadata"))?;
        let update = format!(
            "UPDATE public.cert SET valid_from={},valid_to={},encrypt_type={},issued_by={},status={},updated_time={} WHERE id={} AND domain={} AND COALESCE(san,'')={} AND COALESCE(certificate,'')={} AND COALESCE(private_key,'')={} AND valid_from={} AND valid_to={} AND encrypt_type IS NOT DISTINCT FROM {} AND issued_by IS NOT DISTINCT FROM {} AND status='suc'",
            sql_optional_i64(item.row.valid_from),
            sql_optional_i64(item.row.valid_to),
            sql_optional_text_expression(item.row.encrypt_type.as_deref()),
            sql_optional_text_expression(item.row.issued_by.as_deref()),
            sql_optional_text_expression(item.row.status.as_deref()),
            sql_optional_i64(item.row.updated_time),
            item.row.id,
            sql_text_expression(&item.row.domain),
            sql_text_expression(item.row.san.as_deref().unwrap_or("")),
            sql_text_expression(item.row.certificate.as_deref().unwrap_or("")),
            sql_text_expression(item.row.private_key.as_deref().unwrap_or("")),
            applied.valid_from,
            applied.valid_to,
            sql_text_expression(&applied.encrypt_type),
            sql_text_expression(&applied.issued_by),
        );
        sql.push_str(&assert_exactly_one_update(&update));
    }
    sql.push_str("COMMIT;");
    psql(&sql)?;
    Ok(())
}

fn assert_exactly_one_update(update: &str) -> String {
    format!(
        "WITH updated AS ({update} RETURNING 1) SELECT CASE WHEN count(*) = 1 THEN 1 ELSE (count(*)::text || ' rows')::integer END FROM updated;\n"
    )
}

fn sql_text_expression(value: &str) -> String {
    format!(
        "convert_from(decode('{}','base64'),'UTF8')",
        BASE64_STANDARD.encode(value.as_bytes())
    )
}

fn sql_optional_text_expression(value: Option<&str>) -> String {
    value
        .map(sql_text_expression)
        .unwrap_or_else(|| "NULL".into())
}
fn sql_optional_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "NULL".into())
}

fn restart_and_verify_services() -> anyhow::Result<()> {
    for service in ["network_service.service", "trim_nginx.service"] {
        let status = Command::new("systemctl")
            .args(["restart", service])
            .status()?;
        if !status.success() {
            bail!("failed to restart {service}")
        }
        let status = Command::new("systemctl")
            .args(["is-active", "--quiet", service])
            .status()?;
        if !status.success() {
            bail!("{service} is not active")
        }
    }
    Ok(())
}

fn verify_sni_mappings(selected: &[ComparedTarget]) -> anyhow::Result<()> {
    let mappings = read_json_array(Path::new(NETWORK_GATEWAY_INDEX))?;
    for item in selected {
        let expected = item
            .local
            .as_ref()
            .and_then(|value| value.parsed.as_ref())
            .ok_or_else(|| anyhow!("missing expected certificate fingerprint"))?;
        let hosts = mappings
            .iter()
            .filter_map(|mapping| {
                let same_path = mapping.get("cert").and_then(Value::as_str)
                    == item.row.certificate.as_deref()
                    && mapping.get("key").and_then(Value::as_str)
                        == item.row.private_key.as_deref();
                same_path
                    .then(|| mapping.get("host").and_then(Value::as_str))
                    .flatten()
            })
            .filter(|host| *host != "fallback")
            .collect::<BTreeSet<_>>();
        for host in hosts {
            let mut child = Command::new("timeout")
                .args([
                    "8",
                    "openssl",
                    "s_client",
                    "-connect",
                    "127.0.0.1:443",
                    "-servername",
                    host,
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .with_context(|| format!("probe fnOS TLS SNI {host}"))?;
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(b"Q\n")?;
            }
            let output = child.wait_with_output()?;
            if !output.status.success() {
                bail!("fnOS TLS probe failed for {host}")
            }
            let text = String::from_utf8_lossy(&output.stdout);
            let start = text
                .find("-----BEGIN CERTIFICATE-----")
                .ok_or_else(|| anyhow!("fnOS TLS probe returned no certificate for {host}"))?;
            let relative_end = text[start..]
                .find("-----END CERTIFICATE-----")
                .ok_or_else(|| anyhow!("fnOS TLS probe returned an incomplete certificate"))?;
            let end = start + relative_end + "-----END CERTIFICATE-----".len();
            let actual = parse_certificate(&text[start..end])?;
            if actual.fingerprint != expected.fingerprint {
                bail!("fnOS TLS fingerprint mismatch for {host}")
            }
        }
    }
    Ok(())
}

fn rollback_from_backup(
    dir: &Path,
    selected: &[ComparedTarget],
    database_updated: bool,
) -> anyhow::Result<()> {
    let mut network_index = read_json_array(Path::new(NETWORK_CERT_INDEX))?;
    let mut restore_files = Vec::new();
    let mut restore_network_index = false;
    for item in selected {
        let local = item
            .local
            .as_ref()
            .ok_or_else(|| anyhow!("missing applied local certificate"))?;
        let parsed = local
            .parsed
            .as_ref()
            .ok_or_else(|| anyhow!("missing applied certificate metadata"))?;
        let certificate_path = Path::new(item.row.certificate.as_deref().unwrap_or(""));
        let private_key_path = Path::new(item.row.private_key.as_deref().unwrap_or(""));
        let source = dir.join(item.row.id.to_string());
        let original_certificate = fs::read(source.join("certificate.pem"))?;
        let original_private_key = fs::read(source.join("private.key"))?;
        let certificate_changed = rollback_file_state(
            certificate_path,
            local.cert.as_bytes(),
            &original_certificate,
            dir,
        )?;
        let private_key_changed = rollback_file_state(
            private_key_path,
            local.key.as_bytes(),
            &original_private_key,
            dir,
        )?;
        restore_files.push((
            certificate_path.to_path_buf(),
            original_certificate,
            certificate_changed,
        ));
        restore_files.push((
            private_key_path.to_path_buf(),
            original_private_key,
            private_key_changed,
        ));
        restore_network_index |=
            restore_network_index_entry(&mut network_index, &item.row, parsed)?;
    }

    if database_updated {
        restore_database(selected)?;
    }

    for (path, original, changed) in restore_files {
        if changed {
            atomic_replace_preserving_metadata(&path, &original)?;
        }
    }
    if restore_network_index {
        atomic_replace_preserving_metadata(
            Path::new(NETWORK_CERT_INDEX),
            serde_json::to_string(&network_index)?.as_bytes(),
        )?;
    }
    restart_and_verify_services()
}

fn rollback_file_state(
    path: &Path,
    applied: &[u8],
    original: &[u8],
    backup_dir: &Path,
) -> anyhow::Result<bool> {
    let current = fs::read(path)?;
    if current == applied {
        return Ok(true);
    }
    if current == original {
        return Ok(false);
    }
    bail!(
        "fnOS certificate files changed externally; preserved backup at {}",
        backup_dir.display()
    )
}

fn restore_network_index_entry(
    network_index: &mut [Value],
    row: &FnosCertRow,
    applied: &ParsedCertificate,
) -> anyhow::Result<bool> {
    let mut matched = 0;
    let mut changed = false;
    for entry in network_index {
        if entry.get("certificate").and_then(Value::as_str) == row.certificate.as_deref()
            && entry.get("privateKey").and_then(Value::as_str) == row.private_key.as_deref()
        {
            let current = (
                entry.get("validFrom").and_then(Value::as_i64),
                entry.get("validTo").and_then(Value::as_i64),
            );
            let applied_values = (Some(applied.valid_from), Some(applied.valid_to));
            let original_values = (row.valid_from, row.valid_to);
            if current == applied_values {
                entry["validFrom"] = row.valid_from.map_or(Value::Null, Value::from);
                entry["validTo"] = row.valid_to.map_or(Value::Null, Value::from);
                changed = true;
            } else if current != original_values {
                bail!("fnOS certificate index changed externally; automatic rollback stopped")
            }
            matched += 1;
        }
    }
    if matched != 1 {
        bail!("fnOS certificate index changed externally; automatic rollback stopped")
    }
    Ok(changed)
}

fn prune_backups(data_dir: &Path) -> anyhow::Result<()> {
    let root = data_dir.join("fnos-certificate-sync/backups");
    let mut entries = fs::read_dir(&root)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let remove_count = entries.len().saturating_sub(BACKUP_KEEP_COUNT);
    for entry in entries.into_iter().take(remove_count) {
        fs::remove_dir_all(entry.path())?;
    }
    Ok(())
}

async fn auto_sync_enabled(state: &AppState) -> bool {
    state
        .store
        .get_config()
        .await
        .ok()
        .and_then(|value| {
            value
                .pointer("/fnos_certificate_sync/auto_sync_enabled")
                .and_then(Value::as_bool)
        })
        .unwrap_or(false)
}

async fn record_running(state: &AppState) {
    let mut status = state.fnos_certificate_sync_status.write().await;
    status["running"] = json!(true);
    status["last_error"] = Value::Null;
}

async fn record_success(state: &AppState, summary: &SyncSummary) {
    let mut status = state.fnos_certificate_sync_status.write().await;
    *status = json!({ "running": false, "last_sync_at": time_utils::now_ms(), "last_result": summary, "last_error": null, "failed_target_ids": [] });
}

async fn record_failure(state: &AppState, error: &str, ids: &[String]) {
    let mut status = state.fnos_certificate_sync_status.write().await;
    *status = json!({ "running": false, "last_sync_at": time_utils::now_ms(), "last_result": null, "last_error": error, "failed_target_ids": ids });
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::generate_simple_self_signed;

    #[test]
    fn normalizes_domain_sets_without_expanding_wildcards() {
        assert_eq!(
            normalize_domains(vec![
                "Example.COM.".into(),
                "*.Example.com".into(),
                "example.com".into()
            ]),
            vec!["*.example.com", "example.com"]
        );
        assert_ne!(
            normalize_domains(vec!["*.example.com".into()]),
            normalize_domains(vec!["example.com".into()])
        );
    }

    #[test]
    fn rejects_non_numeric_target_ids() {
        assert!(parse_target_ids(&["1 OR 1=1".into()]).is_err());
    }

    #[test]
    fn sql_text_encoding_cannot_break_statement_delimiters() {
        let malicious = "CA's $fn$; DROP TABLE public.cert; --";
        let expression = sql_text_expression(malicious);
        assert!(!expression.contains(malicious));
        assert!(!expression.contains("$fn$"));
        assert!(expression.starts_with("convert_from(decode('"));
        let assertion = assert_exactly_one_update("UPDATE public.cert SET status='suc'");
        assert!(!assertion.contains("DO $"));
    }

    #[test]
    fn parses_dns_sans_and_rejects_trailing_garbage() {
        let generated = generate_simple_self_signed(vec![
            "Example.COM".to_string(),
            "*.example.com".to_string(),
        ])
        .unwrap();
        let pem = generated.cert.pem();
        let parsed = parse_certificate(&pem).unwrap();
        assert_eq!(parsed.domains, vec!["*.example.com", "example.com"]);
        assert!(parse_certificate(&format!("{pem}\nnot-a-certificate")).is_err());
    }

    #[test]
    fn selects_latest_expiring_valid_candidate_deterministically() {
        let parsed = |valid_to| ParsedCertificate {
            domains: vec!["example.com".into()],
            valid_from: 1,
            valid_to,
            encrypt_type: "RSA".into(),
            issued_by: "test".into(),
            chain_digest: valid_to.to_string(),
            public_key_digest: valid_to.to_string(),
            fingerprint: valid_to.to_string(),
        };
        let candidate = |id: &str, valid_to, updated_at: &str| LocalCandidate {
            id: id.into(),
            label: id.into(),
            updated_at: updated_at.into(),
            cert: String::new(),
            key: String::new(),
            parsed: Some(parsed(valid_to)),
            valid: true,
        };
        let selected = select_best_local(vec![
            candidate("older", 100, "2026-01-01"),
            candidate("newer", 200, "2025-01-01"),
            candidate("newest-write", 200, "2026-01-01"),
        ])
        .unwrap();
        assert_eq!(selected.id, "newest-write");
    }

    #[test]
    fn network_index_update_preserves_unknown_and_sum_fields() {
        let row = FnosCertRow {
            id: 2,
            domain: "example.com".into(),
            san: Some("example.com".into()),
            valid_from: Some(1),
            valid_to: Some(2),
            encrypt_type: None,
            issued_by: None,
            is_default: None,
            renewal: None,
            source: Some("upload".into()),
            private_key: Some("/key".into()),
            certificate: Some("/cert".into()),
            issuer_certificate: None,
            status: None,
            created_time: None,
            updated_time: None,
        };
        let parsed = ParsedCertificate {
            domains: vec!["example.com".into()],
            valid_from: 10,
            valid_to: 20,
            encrypt_type: "RSA".into(),
            issued_by: "test".into(),
            chain_digest: String::new(),
            public_key_digest: String::new(),
            fingerprint: String::new(),
        };
        let mut index = vec![json!({
            "certificate": "/cert",
            "privateKey": "/key",
            "validFrom": 1,
            "validTo": 2,
            "sum": "keep-me",
            "futureField": { "enabled": true }
        })];
        update_network_index_entry(&mut index, &row, &parsed).unwrap();
        assert_eq!(index[0]["validFrom"], 10);
        assert_eq!(index[0]["validTo"], 20);
        assert_eq!(index[0]["sum"], "keep-me");
        assert_eq!(index[0]["futureField"]["enabled"], true);

        restore_network_index_entry(&mut index, &row, &parsed).unwrap();
        assert_eq!(index[0]["validFrom"], 1);
        assert_eq!(index[0]["validTo"], 2);
        index[0]["validTo"] = json!(999);
        assert!(restore_network_index_entry(&mut index, &row, &parsed).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn fixed_file_validation_rejects_symbolic_links() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("target.json");
        let link = directory.path().join("index.json");
        fs::write(&target, b"[]").unwrap();
        symlink(&target, &link).unwrap();
        assert!(validate_fixed_regular_file(&target).is_ok());
        assert!(validate_fixed_regular_file(&link).is_err());
    }

    #[test]
    fn execution_error_keeps_actual_target_ids() {
        let error = SyncExecutionError::new(anyhow!("failed"), &["2".into(), "7".into()]);
        assert_eq!(error.target_ids, vec!["2", "7"]);
    }

    #[test]
    fn rollback_file_state_accepts_only_original_or_applied_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("certificate.pem");
        fs::write(&path, b"original").unwrap();
        assert!(!rollback_file_state(&path, b"applied", b"original", directory.path()).unwrap());
        fs::write(&path, b"applied").unwrap();
        assert!(rollback_file_state(&path, b"applied", b"original", directory.path()).unwrap());
        fs::write(&path, b"external").unwrap();
        assert!(rollback_file_state(&path, b"applied", b"original", directory.path()).is_err());
    }

    #[test]
    fn availability_errors_are_sanitized() {
        assert_eq!(
            sanitize_availability_error(&anyhow!("fnOS PostgreSQL command failed: secret")),
            "Unable to read the fnOS certificate database"
        );
        assert!(!sanitize_availability_error(&anyhow!("private secret")).contains("secret"));
    }
}
