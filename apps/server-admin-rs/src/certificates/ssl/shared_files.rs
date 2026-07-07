use super::*;

pub(super) fn list_ssl_shared_files() -> Value {
    let Some(directory) = configured_share_directory() else {
        return json!({ "shareName": SSL_CERT_SHARE_NAME, "available": false, "files": [] });
    };
    if !directory.is_dir() {
        return json!({ "shareName": SSL_CERT_SHARE_NAME, "available": false, "files": [] });
    }
    let mut files = Vec::new();
    walk_shared_files(&directory, &directory, &mut files, 0);
    files.sort_by(|left, right| {
        let time = right
            .get("modifiedAt")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(left.get("modifiedAt").and_then(Value::as_str).unwrap_or(""));
        if time == std::cmp::Ordering::Equal {
            left.get("relativePath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(
                    right
                        .get("relativePath")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                )
        } else {
            time
        }
    });
    json!({ "shareName": SSL_CERT_SHARE_NAME, "available": true, "files": files })
}

pub(super) fn read_ssl_shared_file(relative_path: &str) -> anyhow::Result<Value> {
    let directory = configured_share_directory()
        .ok_or_else(|| anyhow!("Shared directory is not configured"))?;
    let file_path = resolve_share_path(&directory, relative_path)?;
    let metadata = std::fs::metadata(&file_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            anyhow!(SharedFileNotFound)
        } else if error.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow!(SharedFileForbidden)
        } else {
            anyhow!(error)
        }
    })?;
    if !metadata.is_file() {
        return Err(anyhow!("Shared path must be a file"));
    }
    if metadata.len() > MAX_SHARED_FILE_SIZE {
        return Err(anyhow!("Shared file is too large"));
    }
    let content = std::fs::read_to_string(&file_path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::PermissionDenied {
            anyhow!(SharedFileForbidden)
        } else {
            anyhow!(error)
        }
    })?;
    Ok(json!({
        "file": shared_file_entry(&directory, &file_path, &metadata),
        "content": content.trim_start_matches('\u{feff}')
    }))
}

#[derive(Debug)]
pub(super) struct SharedFileNotFound;

impl std::fmt::Display for SharedFileNotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared file not found")
    }
}

impl std::error::Error for SharedFileNotFound {}

#[derive(Debug)]
pub(super) struct SharedFileForbidden;

impl std::fmt::Display for SharedFileForbidden {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "shared file cannot be read")
    }
}

impl std::error::Error for SharedFileForbidden {}

pub(super) fn configured_share_directory() -> Option<PathBuf> {
    if let Ok(value) =
        env::var("FN_KNOCK_ROOT_SHARE_DIR").or_else(|_| env::var("FN_KNOCK_CERT_SHARE_DIR"))
    {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed));
        }
    }
    let paths = env::var("TRIM_DATA_SHARE_PATHS").ok()?;
    paths
        .split(':')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .min_by_key(|value| value.len())
        .map(PathBuf::from)
}

pub(super) fn walk_shared_files(
    root: &Path,
    current: &Path,
    bucket: &mut Vec<Value>,
    depth: usize,
) {
    if bucket.len() >= MAX_SHARED_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        if bucket.len() >= MAX_SHARED_FILES {
            return;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let path = entry.path();
        if metadata.is_dir() {
            if depth < MAX_SHARED_SCAN_DEPTH {
                walk_shared_files(root, &path, bucket, depth + 1);
            }
            continue;
        }
        if metadata.is_file() {
            bucket.push(shared_file_entry(root, &path, &metadata));
        }
    }
}

pub(super) fn shared_file_entry(root: &Path, path: &Path, metadata: &std::fs::Metadata) -> Value {
    let relative = path
        .strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    json!({
        "name": path.file_name().and_then(|value| value.to_str()).unwrap_or(""),
        "relativePath": relative,
        "extension": path.extension().and_then(|value| value.to_str()).map(|value| format!(".{}", value.to_ascii_lowercase())).unwrap_or_default(),
        "size": metadata.len(),
        "modifiedAt": metadata.modified().ok().map(system_time_iso).unwrap_or_else(time_utils::now_iso)
    })
}

pub(super) fn resolve_share_path(root: &Path, relative_path: &str) -> anyhow::Result<PathBuf> {
    let sanitized = relative_path.replace('\\', "/").trim().to_string();
    if sanitized.is_empty() || sanitized.starts_with('/') {
        return Err(anyhow!("Invalid shared file path"));
    }
    let resolved = root.join(&sanitized);
    let normalized_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let normalized_parent = resolved
        .parent()
        .and_then(|parent| parent.canonicalize().ok())
        .unwrap_or_else(|| root.to_path_buf());
    if !normalized_parent.starts_with(&normalized_root) {
        return Err(anyhow!("Invalid shared file path"));
    }
    Ok(resolved)
}

pub(super) fn system_time_iso(time: SystemTime) -> String {
    let duration = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs() as i64;
    OffsetDateTime::from_unix_timestamp(seconds)
        .ok()
        .and_then(|value| value.format(&Rfc3339).ok())
        .unwrap_or_else(time_utils::now_iso)
}

pub(crate) fn zip_cert_pair(cert: &str, key: &str) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::<u8>::new());
    let mut zip = ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    zip.start_file("server-cert.pem", options)?;
    zip.write_all(cert.as_bytes())?;
    zip.start_file("server-key.pem", options)?;
    zip.write_all(key.as_bytes())?;
    Ok(zip.finish()?.into_inner())
}

pub(super) fn pem_response(content: &str, filename: &str, content_type: &'static str) -> Response {
    binary_response(content.as_bytes().to_vec(), content_type, filename)
}

pub(crate) fn binary_response(
    bytes: Vec<u8>,
    content_type: &'static str,
    filename: &str,
) -> Response {
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}
