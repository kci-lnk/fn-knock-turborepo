use super::*;

pub(super) const HOST_TARGET_TYPE_PROXY: &str = "proxy";
pub(super) const HOST_TARGET_TYPE_FILE: &str = "file";
pub(super) const HOST_TARGET_TYPE_DIRECTORY: &str = "directory";

const DEFAULT_INDEX_FILES: [&str; 2] = ["index.html", "index.htm"];
const MAX_INDEX_FILES: usize = 16;
const MAX_INDEX_FILE_BYTES: usize = 255;
const STATIC_PATH_PROBE_ERROR_CODES: [&str; 7] = [
    "invalid_path",
    "protected_path",
    "not_found",
    "permission_denied",
    "type_mismatch",
    "unsupported_type",
    "unavailable",
];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct StaticPathSpec {
    pub(super) target_type: String,
    pub(super) path: String,
}

pub(super) fn host_target_type(value: Option<&Value>) -> Result<&'static str, String> {
    match value {
        None => Ok(HOST_TARGET_TYPE_PROXY),
        Some(Value::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            HOST_TARGET_TYPE_PROXY => Ok(HOST_TARGET_TYPE_PROXY),
            HOST_TARGET_TYPE_FILE => Ok(HOST_TARGET_TYPE_FILE),
            HOST_TARGET_TYPE_DIRECTORY => Ok(HOST_TARGET_TYPE_DIRECTORY),
            _ => Err("target type must be proxy, file or directory".to_string()),
        },
        Some(_) => Err("target type must be proxy, file or directory".to_string()),
    }
}

pub(super) fn normalized_host_target_type(mapping: &Value) -> &'static str {
    host_target_type(mapping.get("target_type")).unwrap_or(HOST_TARGET_TYPE_PROXY)
}

pub(super) fn normalize_static_serve_config(
    host: &str,
    target_type: &str,
    value: Option<&Value>,
) -> Result<Option<Value>, String> {
    if target_type == HOST_TARGET_TYPE_PROXY {
        return Ok(None);
    }

    let object = value
        .and_then(Value::as_object)
        .ok_or_else(|| format!("Host mapping {host} static serve configuration is required"))?;
    let path = object
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .ok_or_else(|| format!("Host mapping {host} static path is required"))?;
    let path = normalize_static_path(host, path)?;

    if target_type == HOST_TARGET_TYPE_FILE {
        return Ok(Some(json!({
            "path": path,
            "index_files": [],
            "directory_listing": {
                "enabled": false,
                "render_readme": false,
            },
        })));
    }

    let index_files = normalize_index_files(host, object.get("index_files"))?;
    let (listing_enabled, render_readme) =
        normalize_directory_listing(host, object.get("directory_listing"))?;
    Ok(Some(json!({
        "path": path,
        "index_files": index_files,
        "directory_listing": {
            "enabled": listing_enabled,
            "render_readme": render_readme,
        },
    })))
}

fn normalize_static_path(host: &str, path: &str) -> Result<String, String> {
    if path.is_empty() {
        return Err(format!("Host mapping {host} static path is required"));
    }
    if path.contains('\0') {
        return Err(format!("Host mapping {host} static path contains NUL"));
    }
    if path.chars().any(is_unsafe_filesystem_char) {
        return Err(format!(
            "Host mapping {host} static path contains an unsafe control character"
        ));
    }
    if has_windows_unc_prefix(path) {
        return Err(format!(
            "Host mapping {host} static path cannot use a UNC or device namespace"
        ));
    }
    #[cfg(not(windows))]
    if path.contains('\\') {
        return Err(format!(
            "Host mapping {host} static path cannot contain a backslash"
        ));
    }
    let path = std::path::Path::new(path);
    if !path.is_absolute() {
        return Err(format!("Host mapping {host} static path must be absolute"));
    }
    #[cfg(windows)]
    if path.components().next().is_some_and(|component| {
        matches!(
            component,
            std::path::Component::Prefix(prefix)
                if !matches!(
                    prefix.kind(),
                    std::path::Prefix::Disk(_)
                )
        )
    }) {
        return Err(format!(
            "Host mapping {host} static path cannot use a Windows device namespace"
        ));
    }
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "Host mapping {host} static path cannot contain parent traversal"
        ));
    }

    // Rebuild from components instead of canonicalizing the filesystem. This
    // only removes redundant separators and `.` components; it deliberately
    // never follows symlinks or exposes their resolved target to the control
    // plane. The Go gateway performs the authoritative real-path checks.
    let normalized = path.components().collect::<std::path::PathBuf>();
    if normalized.parent().is_none() {
        return Err(format!(
            "Host mapping {host} static path cannot be a filesystem root"
        ));
    }
    let target_name = normalized
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .ok_or_else(|| format!("Host mapping {host} static path target is invalid"))?;
    if !is_safe_visible_name(target_name) {
        return Err(format!(
            "Host mapping {host} static path target must be a visible regular name"
        ));
    }
    normalized
        .into_os_string()
        .into_string()
        .map_err(|_| format!("Host mapping {host} static path target is invalid"))
}

fn has_windows_unc_prefix(path: &str) -> bool {
    let mut characters = path.chars();
    let is_separator = |character| matches!(character, '/' | '\\');
    characters.next().is_some_and(is_separator) && characters.next().is_some_and(is_separator)
}

fn is_safe_visible_name(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && !value.starts_with('.')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.chars().any(is_unsafe_filesystem_char)
        && platform_visible_name_is_safe(value)
}

#[cfg(not(windows))]
fn platform_visible_name_is_safe(_value: &str) -> bool {
    true
}

#[cfg(windows)]
fn platform_visible_name_is_safe(value: &str) -> bool {
    is_safe_windows_visible_name(value)
}

#[cfg(any(windows, test))]
fn is_safe_windows_visible_name(value: &str) -> bool {
    if value
        .chars()
        .any(|character| matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
        || value.ends_with(' ')
        || value.ends_with('.')
    {
        return false;
    }
    let stem = value.split('.').next().unwrap_or("");
    let upper = stem.to_ascii_uppercase();
    if matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$" | "CONIN$" | "CONOUT$"
    ) {
        return false;
    }
    if let Some(suffix) = upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"))
    {
        return !matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        );
    }
    true
}

fn is_unsafe_filesystem_char(value: char) -> bool {
    value.is_control()
        || matches!(
            value,
            '\u{00ad}'
                | '\u{0600}'..='\u{0605}'
                | '\u{061c}'
                | '\u{06dd}'
                | '\u{070f}'
                | '\u{0890}'..='\u{0891}'
                | '\u{08e2}'
                | '\u{180e}'
                | '\u{200b}'..='\u{200f}'
                | '\u{202a}'..='\u{202e}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{206f}'
                | '\u{feff}'
                | '\u{fff9}'..='\u{fffb}'
                | '\u{110bd}'
                | '\u{110cd}'
                | '\u{13430}'..='\u{1343f}'
                | '\u{1bca0}'..='\u{1bca3}'
                | '\u{1d173}'..='\u{1d17a}'
                | '\u{e0001}'
                | '\u{e0020}'..='\u{e007f}'
        )
}

fn normalize_index_files(host: &str, value: Option<&Value>) -> Result<Vec<String>, String> {
    let values = match value {
        None => DEFAULT_INDEX_FILES
            .into_iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        Some(Value::Array(values)) => {
            if values.len() > MAX_INDEX_FILES {
                return Err(format!(
                    "Host mapping {host} static index files cannot contain more than {MAX_INDEX_FILES} entries"
                ));
            }
            values
                .iter()
                .map(|value| {
                    value.as_str().map(ToString::to_string).ok_or_else(|| {
                        format!("Host mapping {host} static index file name is invalid")
                    })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
        Some(_) => {
            return Err(format!(
                "Host mapping {host} static index files must be an array"
            ));
        }
    };

    let mut seen = HashSet::with_capacity(values.len());
    let mut normalized = Vec::with_capacity(values.len());
    for value in values {
        let value = value.trim();
        if !is_safe_visible_name(value)
            || value.len() > MAX_INDEX_FILE_BYTES
            || value.chars().count() > MAX_INDEX_FILE_BYTES
        {
            return Err(format!(
                "Host mapping {host} static index file name is invalid"
            ));
        }
        if seen.insert(value.to_string()) {
            normalized.push(value.to_string());
        }
    }
    Ok(normalized)
}

fn normalize_directory_listing(host: &str, value: Option<&Value>) -> Result<(bool, bool), String> {
    let Some(value) = value else {
        return Ok((false, false));
    };
    let object = value
        .as_object()
        .ok_or_else(|| format!("Host mapping {host} static directory listing is invalid"))?;
    let enabled = optional_bool(host, object, "enabled")?.unwrap_or(false);
    let render_readme = enabled && optional_bool(host, object, "render_readme")?.unwrap_or(false);
    Ok((enabled, render_readme))
}

fn optional_bool(
    host: &str,
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<bool>, String> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!(
            "Host mapping {host} static directory listing {key} is invalid"
        )),
    }
}

pub(super) fn static_path_spec(mapping: &Value) -> Option<StaticPathSpec> {
    let target_type = normalized_host_target_type(mapping);
    if !matches!(
        target_type,
        HOST_TARGET_TYPE_FILE | HOST_TARGET_TYPE_DIRECTORY
    ) {
        return None;
    }
    let path = mapping
        .pointer("/static_serve/path")
        .and_then(Value::as_str)?;
    Some(StaticPathSpec {
        target_type: target_type.to_string(),
        path: path.to_string(),
    })
}

pub(super) fn changed_static_path_specs(
    previous_mappings: &[Value],
    next_mappings: &[Value],
) -> Vec<(String, StaticPathSpec)> {
    let previous_by_sync_id = previous_mappings
        .iter()
        .filter_map(|mapping| {
            mapping
                .get("sync_id")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|sync_id| (sync_id, mapping))
        })
        .collect::<HashMap<_, _>>();
    let previous_by_host = previous_mappings
        .iter()
        .filter_map(|mapping| {
            mapping
                .get("host")
                .and_then(Value::as_str)
                .map(|host| (normalize_host_value(host), mapping))
        })
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    next_mappings
        .iter()
        .filter_map(|mapping| {
            let next = static_path_spec(mapping)?;
            let host =
                normalize_host_value(mapping.get("host").and_then(Value::as_str).unwrap_or(""));
            let previous = mapping
                .get("sync_id")
                .and_then(Value::as_str)
                .and_then(|sync_id| previous_by_sync_id.get(sync_id).copied())
                .or_else(|| previous_by_host.get(&host).copied());
            if previous.and_then(static_path_spec).as_ref() == Some(&next)
                || !seen.insert(next.clone())
            {
                return None;
            }
            Some((host, next))
        })
        .collect()
}

pub(super) fn static_path_probe_spec(
    target_type: &str,
    path: &str,
) -> Result<StaticPathSpec, String> {
    let target_type = host_target_type(Some(&Value::String(target_type.to_string())))?;
    if target_type == HOST_TARGET_TYPE_PROXY {
        return Err("Static path target type must be file or directory".to_string());
    }
    let path = normalize_static_path("probe", path.trim())?;
    Ok(StaticPathSpec {
        target_type: target_type.to_string(),
        path,
    })
}

pub(super) fn rejected_static_path_probe_result(target_type: &str) -> Value {
    let target_type = host_target_type(Some(&Value::String(target_type.to_string())))
        .ok()
        .filter(|target_type| *target_type != HOST_TARGET_TYPE_PROXY);
    let error_code = if target_type.is_some() {
        "invalid_path"
    } else {
        "unsupported_type"
    };

    // Do not echo an invalid path or an unsupported discriminator. Besides
    // keeping this endpoint's result shape stable, this prevents control
    // characters and filesystem-looking attacker input from reaching an API
    // response or client-side diagnostics.
    json!({
        "target_type": target_type,
        "normalized_path": "",
        "exists": false,
        "readable": false,
        "actual_type": "other",
        "error_code": error_code,
    })
}

pub(super) async fn probe_static_path_with_gateway(
    state: &AppState,
    spec: &StaticPathSpec,
) -> Result<Value, String> {
    let result = state
        .gateway
        .client
        .probe_static_path(&spec.target_type, &spec.path)
        .await
        .map_err(|_| "Static path probe request failed".to_string())?;
    sanitize_static_path_probe_result(spec, &result)
}

fn sanitize_static_path_probe_result(
    spec: &StaticPathSpec,
    result: &Value,
) -> Result<Value, String> {
    let target_type = result
        .get("target_type")
        .and_then(Value::as_str)
        .unwrap_or("");
    if target_type != spec.target_type {
        return Err("Static path probe returned an inconsistent target type".to_string());
    }

    // A lexical probe must echo the exact canonical request path. Rejecting a
    // different path prevents a buggy or downgraded gateway from returning a
    // symlink-resolved real path and keeps it out of APIs, logs and storage.
    let echoed_path = result
        .get("normalized_path")
        .and_then(Value::as_str)
        .unwrap_or("");
    let normalized_echo = normalize_static_path("probe response", echoed_path)
        .map_err(|_| "Static path probe returned an invalid normalized path".to_string())?;
    if normalized_echo != spec.path {
        return Err("Static path probe returned an inconsistent normalized path".to_string());
    }

    let actual_type = match result.get("actual_type").and_then(Value::as_str) {
        Some(HOST_TARGET_TYPE_FILE) => HOST_TARGET_TYPE_FILE,
        Some(HOST_TARGET_TYPE_DIRECTORY) => HOST_TARGET_TYPE_DIRECTORY,
        _ => "other",
    };
    let raw_error_code = result
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("unavailable");
    let mut error_code =
        if raw_error_code.is_empty() || STATIC_PATH_PROBE_ERROR_CODES.contains(&raw_error_code) {
            raw_error_code
        } else {
            "unavailable"
        };
    let exists = result
        .get("exists")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let readable = result
        .get("readable")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if error_code.is_empty() && actual_type != spec.target_type {
        error_code = "type_mismatch";
    } else if error_code.is_empty() && (!exists || !readable) {
        error_code = "unavailable";
    }

    Ok(json!({
        "target_type": spec.target_type,
        "normalized_path": spec.path,
        "exists": exists,
        "readable": readable,
        "actual_type": actual_type,
        "error_code": error_code,
    }))
}

fn static_probe_failure_code(spec: &StaticPathSpec, result: &Value) -> Option<String> {
    let error_code = result
        .get("error_code")
        .and_then(Value::as_str)
        .unwrap_or("unavailable");
    if !error_code.is_empty() {
        return Some(error_code.to_string());
    }
    let actual_type = result
        .get("actual_type")
        .and_then(Value::as_str)
        .unwrap_or("other");
    if actual_type != spec.target_type {
        return Some("type_mismatch".to_string());
    }
    if result.get("exists").and_then(Value::as_bool) != Some(true)
        || result.get("readable").and_then(Value::as_bool) != Some(true)
    {
        return Some("unavailable".to_string());
    }
    None
}

pub(super) async fn probe_changed_static_paths(
    state: &AppState,
    previous_mappings: &[Value],
    next_mappings: &mut [Value],
) -> Result<(), String> {
    // This save-time probe is an admission check, not a filesystem lease.
    // Unchanged roots stay editable if a mount is temporarily absent, while
    // the gateway revalidates protected paths and object types when applying
    // rules and again for every request to close the unavoidable TOCTOU gap.
    for (host, spec) in changed_static_path_specs(previous_mappings, next_mappings) {
        let result = match probe_static_path_with_gateway(state, &spec).await {
            Ok(result) => result,
            Err(_) => {
                tracing::warn!(host, "failed to validate static mapping path");
                return Err("Static path probe unavailable".to_string());
            }
        };
        if let Some(code) = static_probe_failure_code(&spec, &result) {
            return Err(format!(
                "Host mapping {host} static path is unavailable ({code})"
            ));
        }
        let normalized_path = result
            .get("normalized_path")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("");
        let normalized_path = normalize_static_path(&host, normalized_path)?;
        if normalized_path != spec.path {
            return Err(format!(
                "Host mapping {host} static path probe returned an inconsistent path"
            ));
        }

        // Multiple hosts may intentionally share one static root. Probe once,
        // then persist the gateway's platform-native lexical normalization for
        // every mapping that submitted the same path/type pair.
        for mapping in next_mappings.iter_mut() {
            if static_path_spec(mapping).as_ref() != Some(&spec) {
                continue;
            }
            if let Some(static_serve) = mapping
                .get_mut("static_serve")
                .and_then(Value::as_object_mut)
            {
                static_serve.insert("path".to_string(), Value::String(normalized_path.clone()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn absolute_test_path(name: &str) -> String {
        std::env::temp_dir()
            .join(name)
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn defaults_directory_index_files_and_disables_listing() {
        let path = absolute_test_path("fn-knock-docs");
        let value = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({ "path": path })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(value["index_files"], json!(["index.html", "index.htm"]));
        assert_eq!(value["directory_listing"]["enabled"], json!(false));
        assert_eq!(value["directory_listing"]["render_readme"], json!(false));
    }

    #[test]
    fn preserves_explicit_empty_index_files_and_deduplicates_in_order() {
        let path = absolute_test_path("fn-knock-docs");
        let empty = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({ "path": path.clone(), "index_files": [] })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(empty["index_files"], json!([]));

        let ordered = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({
                "path": path,
                "index_files": ["home.html", "index.html", "home.html"]
            })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(ordered["index_files"], json!(["home.html", "index.html"]));
    }

    #[test]
    fn file_config_clears_directory_only_options() {
        let path = absolute_test_path("fn-knock-download.zip");
        let value = normalize_static_serve_config(
            "download.example.test",
            HOST_TARGET_TYPE_FILE,
            Some(&json!({
                "path": path,
                "index_files": ["ignored"],
                "directory_listing": { "enabled": true, "render_readme": true }
            })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(value["index_files"], json!([]));
        assert_eq!(value["directory_listing"]["enabled"], json!(false));
    }

    #[test]
    fn changed_specs_ignore_unrelated_edits_and_follow_sync_id_rename() {
        let docs_path = absolute_test_path("fn-knock-docs");
        let next_path = absolute_test_path("fn-knock-next");
        let previous = vec![json!({
            "host": "old.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": { "path": docs_path.clone() }
        })];
        let unchanged = vec![json!({
            "host": "new.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": { "path": docs_path },
            "title_override": "Docs"
        })];
        assert!(changed_static_path_specs(&previous, &unchanged).is_empty());

        let changed = vec![json!({
            "host": "new.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": { "path": next_path }
        })];
        assert_eq!(changed_static_path_specs(&previous, &changed).len(), 1);

        let options_only = vec![json!({
            "host": "new.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": {
                "path": absolute_test_path("fn-knock-docs"),
                "index_files": ["home.html"],
                "directory_listing": { "enabled": true, "render_readme": true }
            }
        })];
        assert!(changed_static_path_specs(&previous, &options_only).is_empty());

        let changed_type = vec![json!({
            "host": "new.example.test",
            "sync_id": "stable",
            "target_type": "file",
            "static_serve": { "path": absolute_test_path("fn-knock-docs") }
        })];
        assert_eq!(changed_static_path_specs(&previous, &changed_type).len(), 1);

        let shared_new = vec![
            json!({
                "host": "a.example.test",
                "target_type": "directory",
                "static_serve": { "path": absolute_test_path("shared-docs") }
            }),
            json!({
                "host": "b.example.test",
                "target_type": "directory",
                "static_serve": { "path": absolute_test_path("shared-docs") }
            }),
        ];
        assert_eq!(changed_static_path_specs(&[], &shared_new).len(), 1);
    }

    #[test]
    fn rejects_traversal_roots_hidden_targets_and_unsafe_characters() {
        let separator = std::path::MAIN_SEPARATOR;
        let base = std::env::temp_dir().to_string_lossy().to_string();
        let cases = [
            format!("relative{separator}docs"),
            format!("{base}{separator}..{separator}private"),
            separator.to_string(),
            format!("{base}{separator}.secret"),
            format!("{base}{separator}line\nbreak"),
            format!("{base}{separator}bidirectional-\u{202e}txt"),
            format!("{base}{separator}format-\u{0600}txt"),
            format!("{base}{separator}nul\0byte"),
        ];

        for path in cases {
            let error = normalize_static_serve_config(
                "docs.example.test",
                HOST_TARGET_TYPE_DIRECTORY,
                Some(&json!({ "path": path })),
            )
            .unwrap_err();
            assert!(
                !error.contains(&path),
                "validation error must not reflect the submitted filesystem path"
            );
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn rejects_backslashes_in_every_posix_path_component() {
        let separator = std::path::MAIN_SEPARATOR;
        let base = std::env::temp_dir().to_string_lossy().to_string();
        for path in [
            format!("{base}{separator}foo\\bar{separator}docs"),
            format!("{base}{separator}docs\\archive"),
        ] {
            let error = normalize_static_serve_config(
                "docs.example.test",
                HOST_TARGET_TYPE_DIRECTORY,
                Some(&json!({ "path": path })),
            )
            .unwrap_err();
            assert!(!error.contains(&path));
        }
    }

    #[test]
    fn canonicalizes_paths_lexically_without_following_filesystem_links() {
        let separator = std::path::MAIN_SEPARATOR;
        let base = std::env::temp_dir();
        let raw = format!(
            "{}{separator}.{separator}fn-knock-static-docs{separator}",
            base.to_string_lossy()
        );
        let expected = base
            .join("fn-knock-static-docs")
            .to_string_lossy()
            .to_string();
        let value = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({ "path": raw })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(value["path"], json!(expected));
    }

    #[test]
    fn rejects_hidden_or_format_control_index_files_and_trims_valid_names() {
        let path = absolute_test_path("fn-knock-docs");
        for name in [
            ".index.html",
            "line\nbreak.html",
            "safe-\u{202e}lmth",
            "safe-\u{0600}html",
        ] {
            let error = normalize_static_serve_config(
                "docs.example.test",
                HOST_TARGET_TYPE_DIRECTORY,
                Some(&json!({ "path": path, "index_files": [name] })),
            )
            .unwrap_err();
            assert!(!error.contains(name));
        }

        let value = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({
                "path": path,
                "index_files": [" index.html ", "index.html", " home.htm "]
            })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(value["index_files"], json!(["index.html", "home.htm"]));
    }

    #[test]
    fn recognizes_windows_device_and_ambiguous_names() {
        for name in [
            "CON",
            "con.txt",
            "PRN",
            "AUX.json",
            "NUL",
            "CLOCK$",
            "CONIN$",
            "CONOUT$",
            "COM1",
            "com9.log",
            "LPT1",
            "LPT³.txt",
            "report.txt:secret",
            "report?.txt",
            "report*.txt",
            "report<draft>.txt",
            "report|draft.txt",
            "report\"draft.txt",
            "trailing.",
            "trailing ",
        ] {
            assert!(!is_safe_windows_visible_name(name), "accepted {name:?}");
        }
        for name in ["CONSOLE", "COM0", "COM10", "LPT0", "report.txt"] {
            assert!(is_safe_windows_visible_name(name), "rejected {name:?}");
        }
    }

    #[test]
    fn rejects_unc_admin_shares_pipes_and_mailslots_with_any_separator_case() {
        for path in [
            r"\\server\share\docs",
            r"//SERVER/share/docs",
            r"\\server\C$\Windows",
            r"//SERVER/c$/Windows",
            r"\\.\pipe\fn-knock",
            r"//./PiPe/fn-knock",
            r"\\.\MAILSLOT\fn-knock",
            r"//./mailslot/fn-knock",
            r"\\?\UNC\server\share\docs",
            r"//?/uNc/server/share/docs",
            r"\/server/share/docs",
            r"/\server\share\docs",
        ] {
            let error = normalize_static_path("docs.example.test", path).unwrap_err();
            assert!(error.contains("UNC or device namespace"), "{path:?}");
            assert!(!error.contains("server"));
            assert!(!error.contains("Windows"));
        }
    }

    #[test]
    fn rejected_probe_results_use_stable_codes_without_echoing_input() {
        let invalid_path = rejected_static_path_probe_result(" DIRECTORY ");
        assert_eq!(invalid_path["target_type"], json!("directory"));
        assert_eq!(invalid_path["normalized_path"], json!(""));
        assert_eq!(invalid_path["exists"], json!(false));
        assert_eq!(invalid_path["readable"], json!(false));
        assert_eq!(invalid_path["actual_type"], json!("other"));
        assert_eq!(invalid_path["error_code"], json!("invalid_path"));

        let unsupported = rejected_static_path_probe_result("/secret/unsupported\nkind");
        assert_eq!(unsupported["target_type"], Value::Null);
        assert_eq!(unsupported["normalized_path"], json!(""));
        assert_eq!(unsupported["error_code"], json!("unsupported_type"));
        assert!(!unsupported.to_string().contains("secret"));

        let proxy = rejected_static_path_probe_result(HOST_TARGET_TYPE_PROXY);
        assert_eq!(proxy["target_type"], Value::Null);
        assert_eq!(proxy["error_code"], json!("unsupported_type"));
    }

    #[test]
    fn probe_projection_rejects_real_path_substitution_and_drops_extra_fields() {
        let spec = static_path_probe_spec(
            HOST_TARGET_TYPE_DIRECTORY,
            &absolute_test_path("fn-knock-docs"),
        )
        .unwrap();
        let raw = json!({
            "target_type": "directory",
            "normalized_path": spec.path,
            "exists": true,
            "readable": true,
            "actual_type": "directory",
            "error_code": "",
            "resolved_path": "/secret/real/root",
            "diagnostic": "/secret/real/root is readable",
        });
        let projected = sanitize_static_path_probe_result(&spec, &raw).unwrap();
        assert_eq!(projected.as_object().unwrap().len(), 6);
        assert!(projected.get("resolved_path").is_none());
        assert!(projected.get("diagnostic").is_none());

        let mut substituted = raw.clone();
        substituted["normalized_path"] = json!(absolute_test_path("resolved-secret"));
        let error = sanitize_static_path_probe_result(&spec, &substituted).unwrap_err();
        assert!(!error.contains("resolved-secret"));

        let mut wrong_type = raw.clone();
        wrong_type["target_type"] = json!("file");
        assert!(sanitize_static_path_probe_result(&spec, &wrong_type).is_err());
    }

    #[test]
    fn probe_projection_allowlists_stable_error_codes() {
        let spec = static_path_probe_spec(
            HOST_TARGET_TYPE_FILE,
            &absolute_test_path("fn-knock-manual.pdf"),
        )
        .unwrap();
        for code in STATIC_PATH_PROBE_ERROR_CODES {
            let projected = sanitize_static_path_probe_result(
                &spec,
                &json!({
                    "target_type": "file",
                    "normalized_path": spec.path,
                    "exists": false,
                    "readable": false,
                    "actual_type": "other",
                    "error_code": code,
                }),
            )
            .unwrap();
            assert_eq!(projected["error_code"], json!(code));
        }

        let projected = sanitize_static_path_probe_result(
            &spec,
            &json!({
                "target_type": "file",
                "normalized_path": spec.path,
                "exists": false,
                "readable": false,
                "actual_type": "other",
                "error_code": "internal: /secret/real/root",
            }),
        )
        .unwrap();
        assert_eq!(projected["error_code"], json!("unavailable"));
        assert!(!projected.to_string().contains("/secret/real/root"));
    }

    #[test]
    fn successful_flags_cannot_bypass_probe_type_validation() {
        let spec = static_path_probe_spec(
            HOST_TARGET_TYPE_DIRECTORY,
            &absolute_test_path("fn-knock-docs"),
        )
        .unwrap();
        let malformed_success = json!({
            "target_type": "directory",
            "normalized_path": spec.path,
            "exists": true,
            "readable": true,
            "actual_type": "other",
            "error_code": "",
        });
        let projected = sanitize_static_path_probe_result(&spec, &malformed_success).unwrap();
        assert_eq!(projected["error_code"], json!("type_mismatch"));
        assert_eq!(
            static_probe_failure_code(&spec, &projected).as_deref(),
            Some("type_mismatch")
        );

        let mut valid = projected;
        valid["actual_type"] = json!("directory");
        valid["error_code"] = json!("");
        assert_eq!(static_probe_failure_code(&spec, &valid), None);
    }
}
