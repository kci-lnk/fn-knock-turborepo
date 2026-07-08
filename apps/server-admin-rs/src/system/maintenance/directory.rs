use super::*;
use crate::{
    runtime_profile::configured_share_directory_with_legacy_env_precedence as configured_share_directory,
    time_utils::system_time_iso,
};

pub(super) async fn list_backup_directory_files() -> anyhow::Result<Value> {
    let Some(directory) = configured_share_directory().map(|path| path.join(BACKUP_DIRECTORY_NAME))
    else {
        return Ok(json!({
            "shareName": "fn-knock / backup",
            "available": false,
            "files": [],
        }));
    };
    fs::create_dir_all(&directory).await?;
    let mut files = Vec::new();
    collect_backup_directory_files(&directory, &directory, &mut files, 0).await?;
    files.sort_by(|left, right| {
        let left_time = left.get("modifiedAt").and_then(Value::as_str).unwrap_or("");
        let right_time = right
            .get("modifiedAt")
            .and_then(Value::as_str)
            .unwrap_or("");
        right_time.cmp(left_time).then_with(|| {
            left.get("relativePath")
                .and_then(Value::as_str)
                .unwrap_or("")
                .cmp(
                    right
                        .get("relativePath")
                        .and_then(Value::as_str)
                        .unwrap_or(""),
                )
        })
    });
    Ok(json!({
        "shareName": "fn-knock / backup",
        "available": true,
        "files": files,
    }))
}

pub(super) async fn collect_backup_directory_files(
    current: &Path,
    root: &Path,
    bucket: &mut Vec<Value>,
    depth: usize,
) -> io::Result<()> {
    if bucket.len() >= MAX_BACKUP_DIRECTORY_FILES {
        return Ok(());
    }
    let mut entries = fs::read_dir(current).await?;
    while let Some(entry) = entries.next_entry().await? {
        if bucket.len() >= MAX_BACKUP_DIRECTORY_FILES {
            return Ok(());
        }
        let file_type = entry.file_type().await?;
        let path = entry.path();
        if file_type.is_dir() {
            if depth < MAX_BACKUP_DIRECTORY_SCAN_DEPTH {
                Box::pin(collect_backup_directory_files(
                    &path,
                    root,
                    bucket,
                    depth + 1,
                ))
                .await?;
            }
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !file_type.is_file() || !name.to_ascii_lowercase().ends_with(KNOCK_BACKUP_EXTENSION) {
            continue;
        }
        let metadata = entry.metadata().await?;
        bucket.push(json!({
            "name": name,
            "relativePath": path.strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/"),
            "extension": KNOCK_BACKUP_EXTENSION,
            "size": metadata.len(),
            "modifiedAt": system_time_iso(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
        }));
    }
    Ok(())
}

pub(super) async fn ensure_backup_directory() -> Result<PathBuf, BackupImportError> {
    let Some(directory) = configured_share_directory().map(|path| path.join(BACKUP_DIRECTORY_NAME))
    else {
        return Err(BackupImportError::new(
            StatusCode::NOT_FOUND,
            "Backup share directory is not configured",
        ));
    };
    fs::create_dir_all(&directory)
        .await
        .map_err(|error| BackupImportError::internal(error.to_string()))?;
    Ok(directory)
}
