use super::*;

pub(super) async fn resolve_backup_archive_path(
    relative_path: &str,
) -> Result<PathBuf, BackupImportError> {
    let directory = ensure_backup_directory().await?;
    resolve_backup_archive_path_like_node(&directory, relative_path)
}

pub(super) fn resolve_backup_archive_path_like_node(
    directory: &Path,
    relative_path: &str,
) -> Result<PathBuf, BackupImportError> {
    let sanitized = relative_path.replace('\\', "/").trim().to_string();
    if sanitized.is_empty() || sanitized.starts_with('/') {
        return Err(BackupImportError::bad_request("Invalid backup path"));
    }
    let normalized_root = normalize_path_like_node(directory);
    let resolved = normalize_path_like_node(&normalized_root.join(&sanitized));
    if !resolved.starts_with(&normalized_root) || resolved == normalized_root {
        return Err(BackupImportError::bad_request("Invalid backup path"));
    }
    Ok(resolved)
}

pub(super) fn normalize_path_like_node(path: &Path) -> PathBuf {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
}

pub(super) fn is_backup_archive_file(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            format!(".{}", extension.to_ascii_lowercase()) == KNOCK_BACKUP_EXTENSION
        })
}

pub(super) fn is_node_base64(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() % 4 != 0 {
        return false;
    }

    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return false;
    }

    let content_len = bytes.len() - padding;
    let expected_remainder = match padding {
        0 => 0,
        1 => 3,
        2 => 2,
        _ => unreachable!(),
    };
    if content_len % 4 != expected_remainder {
        return false;
    }

    bytes[..content_len]
        .iter()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'+' | b'/'))
        && bytes[content_len..].iter().all(|byte| *byte == b'=')
}

pub(super) fn js_number_from_json(value: &Value) -> Option<f64> {
    let number = match value {
        Value::Number(number) => number.as_f64()?,
        Value::String(value) => js_number_from_string(value)?,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Array(values) => js_number_from_string(&js_array_to_string(values))?,
        Value::Object(_) => return None,
    };
    number.is_finite().then_some(number)
}

pub(super) fn js_array_to_string(values: &[Value]) -> String {
    values
        .iter()
        .map(js_value_to_array_string)
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn js_value_to_array_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Array(values) => js_array_to_string(values),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub(super) fn js_number_from_string(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };

    match radix_value {
        Some(value) => Some(value),
        None => trimmed.parse::<f64>().ok(),
    }
}

pub(super) fn backup_app_version_supported(version: &str) -> bool {
    compare_version(version, APP_BACKUP_IMPORT_MIN_VERSION) >= 0
        && compare_version(version, APP_LOCAL_VERSION) <= 0
}

pub(super) fn compare_version(left: &str, right: &str) -> i8 {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    let max_len = left_parts.len().max(right_parts.len()).max(3);
    for index in 0..max_len {
        let left = *left_parts.get(index).unwrap_or(&0);
        let right = *right_parts.get(index).unwrap_or(&0);
        if left > right {
            return 1;
        }
        if left < right {
            return -1;
        }
    }
    0
}

pub(super) fn version_parts(value: &str) -> Vec<i64> {
    value
        .trim()
        .split('.')
        .map(|part| {
            let digits = part
                .chars()
                .skip_while(|ch| !ch.is_ascii_digit())
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<i64>().unwrap_or(0)
        })
        .collect()
}

pub(super) fn summarize_command_failure(stdout: &[u8], stderr: &[u8]) -> Option<String> {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    let detail = if stderr.is_empty() {
        stdout.as_ref()
    } else {
        stderr.as_ref()
    };
    let summary = detail
        .trim()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .rev()
        .take(3)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" | ");
    (!summary.is_empty()).then_some(summary)
}
