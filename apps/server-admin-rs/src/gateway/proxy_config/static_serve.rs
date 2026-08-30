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
const STATIC_PATH_BROWSE_ERROR_CODES: [&str; 9] = [
    "invalid_path",
    "invalid_cursor",
    "protected_path",
    "not_found",
    "permission_denied",
    "not_directory",
    "directory_too_large",
    "unsupported_type",
    "unavailable",
];
const MAX_STATIC_PATH_BROWSE_PATH_BYTES: usize = 4096;
const MAX_STATIC_PATH_BROWSE_CURSOR_BYTES: usize = 512;
const MAX_STATIC_PATH_BROWSE_ENTRIES: usize = 100;
const MAX_STATIC_PATH_BROWSE_BREADCRUMBS: usize = 256;
const MAX_STATIC_PATH_BROWSE_NAME_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct StaticPathSpec {
    pub(super) target_type: String,
    pub(super) path: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StaticPathBrowseSpec {
    pub(super) target_type: String,
    pub(super) path: String,
    pub(super) cursor: String,
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

pub(super) fn static_path_browse_target_type(value: &str) -> Option<&'static str> {
    match value {
        HOST_TARGET_TYPE_FILE => Some(HOST_TARGET_TYPE_FILE),
        HOST_TARGET_TYPE_DIRECTORY => Some(HOST_TARGET_TYPE_DIRECTORY),
        _ => None,
    }
}

pub(super) fn static_path_browse_spec(
    target_type: &str,
    path: Option<&str>,
    cursor: Option<&str>,
) -> Result<StaticPathBrowseSpec, &'static str> {
    let path = normalize_static_path_browse_path(path.unwrap_or("")).ok_or("invalid_path")?;
    let cursor = cursor.unwrap_or("");
    if !is_valid_static_path_browse_cursor(cursor) {
        return Err("invalid_cursor");
    }
    Ok(StaticPathBrowseSpec {
        target_type: target_type.to_string(),
        path,
        cursor: cursor.to_string(),
    })
}

pub(super) fn rejected_static_path_browse_result(target_type: &str, error_code: &str) -> Value {
    json!({
        "target_type": target_type,
        "platform": static_path_browse_platform(),
        "current_path": null,
        "parent_path": null,
        "current_selectable": false,
        "selected_path": null,
        "breadcrumbs": [],
        "entries": [],
        "previous_cursor": null,
        "next_cursor": null,
        "error_code": error_code,
    })
}

pub(super) async fn browse_static_path_with_gateway(
    state: &AppState,
    spec: &StaticPathBrowseSpec,
) -> Result<Value, &'static str> {
    let result = state
        .gateway
        .client
        .browse_static_path(&spec.target_type, &spec.path, &spec.cursor)
        .await
        .map_err(|_| "Static path browse request failed")?;
    sanitize_static_path_browse_result(spec, &result)
}

fn static_path_browse_platform() -> &'static str {
    if cfg!(windows) { "windows" } else { "posix" }
}

fn normalize_static_path_browse_path(path: &str) -> Option<String> {
    if path.len() > MAX_STATIC_PATH_BROWSE_PATH_BYTES
        || path.contains('\0')
        || path.chars().any(is_unsafe_filesystem_char)
    {
        return None;
    }
    if path.is_empty() {
        return Some(String::new());
    }
    if cfg!(windows) {
        normalize_windows_static_path_browse_path(path)
    } else {
        normalize_posix_static_path_browse_path(path)
    }
}

fn normalize_posix_static_path_browse_path(path: &str) -> Option<String> {
    if !path.starts_with('/') || has_windows_unc_prefix(path) || path.contains('\\') {
        return None;
    }
    let mut components = Vec::new();
    for component in path.split('/').skip(1) {
        if component.is_empty() {
            continue;
        }
        if matches!(component, "." | "..") || !is_safe_browse_name(component, "posix") {
            return None;
        }
        components.push(component);
    }
    if components.is_empty() {
        Some("/".to_string())
    } else {
        Some(format!("/{}", components.join("/")))
    }
}

fn normalize_windows_static_path_browse_path(path: &str) -> Option<String> {
    if has_windows_unc_prefix(path) || path.len() < 3 {
        return None;
    }
    let bytes = path.as_bytes();
    if !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' || !matches!(bytes[2], b'/' | b'\\') {
        return None;
    }
    let drive = (bytes[0] as char).to_ascii_uppercase();
    let mut components = Vec::new();
    for component in path[3..].split(['/', '\\']) {
        if component.is_empty() {
            continue;
        }
        if matches!(component, "." | "..") || !is_safe_browse_name(component, "windows") {
            return None;
        }
        components.push(component);
    }
    let mut normalized = format!("{drive}:\\");
    normalized.push_str(&components.join("\\"));
    Some(normalized)
}

fn is_valid_static_path_browse_cursor(cursor: &str) -> bool {
    cursor.len() <= MAX_STATIC_PATH_BROWSE_CURSOR_BYTES
        && cursor
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn is_safe_browse_name(value: &str, platform: &str) -> bool {
    value.len() <= MAX_STATIC_PATH_BROWSE_NAME_BYTES
        && value.chars().count() <= MAX_STATIC_PATH_BROWSE_NAME_BYTES
        && !value.starts_with("__")
        && !value.is_empty()
        && value != "."
        && value != ".."
        && !value.starts_with('.')
        && !value.contains('/')
        && !value.contains('\\')
        && !value.contains('\0')
        && !value.chars().any(is_unsafe_filesystem_char)
        && (platform != "windows" || is_safe_windows_visible_name(value))
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
    // Filesystem path whitespace is significant on POSIX. Use trimming only
    // to recognize an all-whitespace missing value; never feed the trimmed
    // spelling into normalization, probing, or durable configuration.
    if path.trim().is_empty() {
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

fn sanitize_static_path_browse_result(
    spec: &StaticPathBrowseSpec,
    result: &Value,
) -> Result<Value, &'static str> {
    const INVALID: &str = "Static path browse returned an invalid response";
    const FIELDS: [&str; 11] = [
        "target_type",
        "platform",
        "current_path",
        "parent_path",
        "current_selectable",
        "selected_path",
        "breadcrumbs",
        "entries",
        "previous_cursor",
        "next_cursor",
        "error_code",
    ];
    let object = result.as_object().ok_or(INVALID)?;
    if object.len() != FIELDS.len() || object.keys().any(|key| !FIELDS.contains(&key.as_str())) {
        return Err(INVALID);
    }
    if required_string_field(object, "target_type")? != spec.target_type {
        return Err(INVALID);
    }
    let platform = required_string_field(object, "platform")?;
    if !matches!(platform, "posix" | "windows") || platform != static_path_browse_platform() {
        return Err(INVALID);
    }
    let requested_path = if platform == "posix" && spec.path.is_empty() {
        "/"
    } else {
        &spec.path
    };
    let current_path = nullable_non_empty_string_field(object, "current_path")?;
    let parent_path = nullable_string_field(object, "parent_path")?;
    let selected_path = nullable_non_empty_string_field(object, "selected_path")?;
    let previous_cursor = nullable_non_empty_string_field(object, "previous_cursor")?;
    let next_cursor = nullable_non_empty_string_field(object, "next_cursor")?;
    let error_code = nullable_non_empty_string_field(object, "error_code")?;
    let current_selectable = object
        .get("current_selectable")
        .and_then(Value::as_bool)
        .ok_or(INVALID)?;
    let breadcrumbs = object
        .get("breadcrumbs")
        .and_then(Value::as_array)
        .ok_or(INVALID)?;
    let entries = object
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(INVALID)?;

    if let Some(error_code) = error_code {
        if !STATIC_PATH_BROWSE_ERROR_CODES.contains(&error_code)
            || current_path.is_some()
            || parent_path.is_some()
            || selected_path.is_some()
            || previous_cursor.is_some()
            || next_cursor.is_some()
            || current_selectable
            || !breadcrumbs.is_empty()
            || !entries.is_empty()
        {
            return Err(INVALID);
        }
        return Ok(rejected_static_path_browse_result(
            &spec.target_type,
            error_code,
        ));
    }

    if previous_cursor.is_some_and(|value| !is_valid_static_path_browse_cursor(value))
        || next_cursor.is_some_and(|value| !is_valid_static_path_browse_cursor(value))
        || previous_cursor.is_some() && previous_cursor == next_cursor
    {
        return Err(INVALID);
    }

    let virtual_windows_root = platform == "windows" && current_path.is_none();
    if virtual_windows_root {
        if !spec.path.is_empty()
            || parent_path.is_some()
            || selected_path.is_some()
            || current_selectable
            || !breadcrumbs.is_empty()
            || previous_cursor.is_some()
            || next_cursor.is_some()
        {
            return Err(INVALID);
        }
    } else {
        let current_path = current_path.ok_or(INVALID)?;
        if !is_canonical_static_path_browse_path(platform, current_path) {
            return Err(INVALID);
        }
        let expected_parent = static_path_browse_parent(platform, current_path);
        if !optional_paths_equal(platform, parent_path, expected_parent.as_deref()) {
            return Err(INVALID);
        }
        if !breadcrumbs_match_current(platform, breadcrumbs, current_path) {
            return Err(INVALID);
        }
        if spec.target_type == HOST_TARGET_TYPE_DIRECTORY {
            if selected_path.is_some()
                || !paths_equal(platform, current_path, requested_path)
                || current_selectable && is_static_path_browse_root(platform, current_path)
            {
                return Err(INVALID);
            }
        } else if let Some(selected_path) = selected_path {
            if !is_canonical_static_path_browse_path(platform, selected_path)
                || !paths_equal(platform, selected_path, requested_path)
                || !is_immediate_static_path_child(platform, current_path, selected_path)
                || current_selectable
            {
                return Err(INVALID);
            }
        } else if !paths_equal(platform, current_path, requested_path) || current_selectable {
            return Err(INVALID);
        }
    }

    let sanitized_breadcrumbs = sanitize_static_path_breadcrumbs(platform, breadcrumbs)?;
    let sanitized_entries = sanitize_static_path_browse_entries(
        platform,
        &spec.target_type,
        current_path,
        virtual_windows_root,
        entries,
    )?;

    Ok(json!({
        "target_type": spec.target_type,
        "platform": platform,
        "current_path": current_path,
        "parent_path": parent_path,
        "current_selectable": current_selectable,
        "selected_path": selected_path,
        "breadcrumbs": sanitized_breadcrumbs,
        "entries": sanitized_entries,
        "previous_cursor": previous_cursor,
        "next_cursor": next_cursor,
        "error_code": null,
    }))
}

fn required_string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<&'a str, &'static str> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or("Static path browse returned an invalid response")
}

fn nullable_non_empty_string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, &'static str> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err("Static path browse returned an invalid response"),
    }
}

fn nullable_string_field<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Result<Option<&'a str>, &'static str> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err("Static path browse returned an invalid response"),
    }
}

fn is_canonical_static_path_browse_path(platform: &str, path: &str) -> bool {
    if path.is_empty() || path.len() > MAX_STATIC_PATH_BROWSE_PATH_BYTES {
        return false;
    }
    let normalized = match platform {
        "posix" => normalize_posix_static_path_browse_path(path),
        "windows" => normalize_windows_static_path_browse_path(path),
        _ => None,
    };
    normalized.is_some_and(|normalized| paths_equal(platform, &normalized, path))
}

fn paths_equal(platform: &str, left: &str, right: &str) -> bool {
    if platform == "windows" {
        left.eq_ignore_ascii_case(right)
    } else {
        left == right
    }
}

fn optional_paths_equal(platform: &str, left: Option<&str>, right: Option<&str>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => paths_equal(platform, left, right),
        (None, None) => true,
        _ => false,
    }
}

fn static_path_browse_parent(platform: &str, path: &str) -> Option<String> {
    if platform == "posix" {
        if path == "/" {
            return None;
        }
        let separator = path.rfind('/')?;
        return Some(if separator == 0 {
            "/".to_string()
        } else {
            path[..separator].to_string()
        });
    }
    if path.len() == 3 && path.as_bytes().get(1) == Some(&b':') && path.ends_with('\\') {
        return Some(String::new());
    }
    let separator = path.rfind('\\')?;
    Some(if separator == 2 {
        path[..=separator].to_string()
    } else {
        path[..separator].to_string()
    })
}

fn is_static_path_browse_root(platform: &str, path: &str) -> bool {
    path == "/" || platform == "windows" && is_windows_drive_root(path)
}

fn is_immediate_static_path_child(platform: &str, parent: &str, child: &str) -> bool {
    static_path_browse_parent(platform, child)
        .as_deref()
        .is_some_and(|candidate| paths_equal(platform, candidate, parent))
}

fn breadcrumbs_match_current(platform: &str, breadcrumbs: &[Value], current_path: &str) -> bool {
    if breadcrumbs.is_empty() || breadcrumbs.len() > MAX_STATIC_PATH_BROWSE_BREADCRUMBS {
        return false;
    }
    let expected = expected_static_path_breadcrumbs(platform, current_path);
    if breadcrumbs.len() != expected.len() {
        return false;
    }
    breadcrumbs
        .iter()
        .zip(expected)
        .all(|(value, (expected_name, expected_path))| {
            value.as_object().is_some_and(|object| {
                object.len() == 2
                    && object
                        .get("name")
                        .and_then(Value::as_str)
                        .is_some_and(|name| name == expected_name)
                    && object
                        .get("path")
                        .and_then(Value::as_str)
                        .is_some_and(|path| paths_equal(platform, path, &expected_path))
            })
        })
}

fn expected_static_path_breadcrumbs(platform: &str, current_path: &str) -> Vec<(String, String)> {
    if platform == "posix" {
        let mut result = vec![("/".to_string(), "/".to_string())];
        let mut path = String::new();
        for name in current_path.split('/').filter(|value| !value.is_empty()) {
            path.push('/');
            path.push_str(name);
            result.push((name.to_string(), path.clone()));
        }
        return result;
    }
    let drive_name = current_path[..2].to_string();
    let drive_path = current_path[..3].to_string();
    let mut result = vec![(drive_name, drive_path.clone())];
    let mut path = drive_path;
    for name in current_path[3..]
        .split('\\')
        .filter(|value| !value.is_empty())
    {
        if !path.ends_with('\\') {
            path.push('\\');
        }
        path.push_str(name);
        result.push((name.to_string(), path.clone()));
    }
    result
}

fn sanitize_static_path_breadcrumbs(
    platform: &str,
    breadcrumbs: &[Value],
) -> Result<Vec<Value>, &'static str> {
    breadcrumbs
        .iter()
        .map(|value| {
            let object = value
                .as_object()
                .ok_or("Static path browse returned an invalid response")?;
            if object.len() != 2 {
                return Err("Static path browse returned an invalid response");
            }
            let name = required_string_field(object, "name")?;
            let path = required_string_field(object, "path")?;
            if name.len() > MAX_STATIC_PATH_BROWSE_NAME_BYTES
                || name.chars().count() > MAX_STATIC_PATH_BROWSE_NAME_BYTES
                || !is_canonical_static_path_browse_path(platform, path)
            {
                return Err("Static path browse returned an invalid response");
            }
            Ok(json!({ "name": name, "path": path }))
        })
        .collect()
}

fn sanitize_static_path_browse_entries(
    platform: &str,
    target_type: &str,
    current_path: Option<&str>,
    virtual_windows_root: bool,
    entries: &[Value],
) -> Result<Vec<Value>, &'static str> {
    const INVALID: &str = "Static path browse returned an invalid response";
    if entries.len() > MAX_STATIC_PATH_BROWSE_ENTRIES {
        return Err(INVALID);
    }
    let mut seen_paths = HashSet::with_capacity(entries.len());
    let mut saw_file = false;
    let mut sanitized = Vec::with_capacity(entries.len());
    for value in entries {
        let object = value.as_object().ok_or(INVALID)?;
        let allowed_fields = [
            "name",
            "path",
            "entry_type",
            "navigable",
            "selectable",
            "size_bytes",
            "modified_at",
        ];
        if object.len() != allowed_fields.len()
            || object
                .keys()
                .any(|key| !allowed_fields.contains(&key.as_str()))
        {
            return Err(INVALID);
        }
        let name = required_string_field(object, "name")?;
        let path = required_string_field(object, "path")?;
        let entry_type = required_string_field(object, "entry_type")?;
        if !matches!(
            entry_type,
            HOST_TARGET_TYPE_FILE | HOST_TARGET_TYPE_DIRECTORY
        ) {
            return Err(INVALID);
        }
        let navigable = object
            .get("navigable")
            .and_then(Value::as_bool)
            .ok_or(INVALID)?;
        let selectable = object
            .get("selectable")
            .and_then(Value::as_bool)
            .ok_or(INVALID)?;
        let size_bytes = match object.get("size_bytes") {
            Some(Value::Null) => None,
            Some(value) => Some(value.as_u64().ok_or(INVALID)?),
            None => return Err(INVALID),
        };
        let modified_at = nullable_non_empty_string_field(object, "modified_at")?;
        if !is_valid_static_path_modified_at(modified_at)
            || navigable != (entry_type == HOST_TARGET_TYPE_DIRECTORY)
            || selectable && entry_type != target_type
            || entry_type == HOST_TARGET_TYPE_DIRECTORY && size_bytes.is_some()
        {
            return Err(INVALID);
        }

        if virtual_windows_root {
            if entry_type != HOST_TARGET_TYPE_DIRECTORY
                || selectable
                || size_bytes.is_some()
                || modified_at.is_some()
                || !is_windows_drive_root(path)
                || name != &path[..2]
            {
                return Err(INVALID);
            }
        } else {
            let current_path = current_path.ok_or(INVALID)?;
            if !is_safe_browse_name(name, platform)
                || !is_canonical_static_path_browse_path(platform, path)
                || !is_immediate_static_path_child(platform, current_path, path)
                || static_path_browse_leaf(platform, path)
                    .is_none_or(|leaf| !paths_equal(platform, leaf, name))
            {
                return Err(INVALID);
            }
        }

        let unique_path = if platform == "windows" {
            path.to_lowercase()
        } else {
            path.to_string()
        };
        if !seen_paths.insert(unique_path) {
            return Err(INVALID);
        }
        if entry_type == HOST_TARGET_TYPE_FILE {
            saw_file = true;
        } else if saw_file {
            return Err(INVALID);
        }
        sanitized.push(json!({
            "name": name,
            "path": path,
            "entry_type": entry_type,
            "navigable": navigable,
            "selectable": selectable,
            "size_bytes": size_bytes,
            "modified_at": modified_at,
        }));
    }
    Ok(sanitized)
}

fn is_windows_drive_root(path: &str) -> bool {
    path.len() == 3
        && path.as_bytes()[0].is_ascii_alphabetic()
        && path.as_bytes()[1] == b':'
        && path.as_bytes()[2] == b'\\'
}

fn static_path_browse_leaf<'a>(platform: &str, path: &'a str) -> Option<&'a str> {
    let separator = if platform == "windows" { '\\' } else { '/' };
    path.rsplit(separator).find(|value| !value.is_empty())
}

fn is_valid_static_path_modified_at(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return true;
    };
    value.len() <= 64
        && time::OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339)
            .is_ok_and(|timestamp| timestamp.offset().is_utc())
}

pub(super) fn static_path_probe_spec(
    target_type: &str,
    path: &str,
) -> Result<StaticPathSpec, String> {
    let target_type = host_target_type(Some(&Value::String(target_type.to_string())))?;
    if target_type == HOST_TARGET_TYPE_PROXY {
        return Err("Static path target type must be file or directory".to_string());
    }
    let path = normalize_static_path("probe", path)?;
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
    if echoed_path != spec.path {
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
            .unwrap_or("");
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
                static_serve.insert("path".to_string(), Value::String(spec.path.clone()));
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

    #[cfg(not(windows))]
    #[test]
    fn exact_static_path_preserves_posix_trailing_whitespace_across_specs() {
        let spaced = "/srv/public/ ";
        let adjacent = "/srv/public";

        let browse =
            static_path_browse_spec(HOST_TARGET_TYPE_DIRECTORY, Some(spaced), None).unwrap();
        assert_eq!(browse.path, spaced);

        let config = normalize_static_serve_config(
            "docs.example.test",
            HOST_TARGET_TYPE_DIRECTORY,
            Some(&json!({ "path": spaced })),
        )
        .unwrap()
        .unwrap();
        assert_eq!(config["path"], json!(spaced));

        let spaced_probe = static_path_probe_spec(HOST_TARGET_TYPE_DIRECTORY, spaced).unwrap();
        let adjacent_probe = static_path_probe_spec(HOST_TARGET_TYPE_DIRECTORY, adjacent).unwrap();
        assert_eq!(spaced_probe.path, spaced);
        assert_eq!(adjacent_probe.path, adjacent);
        assert_ne!(spaced_probe, adjacent_probe);

        let previous = vec![json!({
            "host": "docs.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": { "path": adjacent },
        })];
        let next = vec![json!({
            "host": "docs.example.test",
            "sync_id": "stable",
            "target_type": "directory",
            "static_serve": { "path": spaced },
        })];
        let changed = changed_static_path_specs(&previous, &next);
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].1.path, spaced);
    }

    #[test]
    fn exact_static_path_rejects_all_whitespace_inputs() {
        for path in ["", " ", "   ", "\u{00a0}"] {
            assert!(static_path_probe_spec(HOST_TARGET_TYPE_DIRECTORY, path).is_err());
            assert!(
                normalize_static_serve_config(
                    "docs.example.test",
                    HOST_TARGET_TYPE_DIRECTORY,
                    Some(&json!({ "path": path })),
                )
                .is_err()
            );
        }
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

    #[cfg(not(windows))]
    #[test]
    fn exact_static_path_probe_projection_rejects_adjacent_substitution() {
        let spaced = "/srv/public/ ";
        let spec = static_path_probe_spec(HOST_TARGET_TYPE_DIRECTORY, spaced).unwrap();
        let exact = json!({
            "target_type": "directory",
            "normalized_path": spaced,
            "exists": true,
            "readable": true,
            "actual_type": "directory",
            "error_code": "",
        });

        let projected = sanitize_static_path_probe_result(&spec, &exact).unwrap();
        assert_eq!(projected["normalized_path"], json!(spaced));

        let mut adjacent = exact;
        adjacent["normalized_path"] = json!("/srv/public");
        assert!(sanitize_static_path_probe_result(&spec, &adjacent).is_err());

        let mut rewritten = adjacent;
        rewritten["normalized_path"] = json!("/srv//public/ ");
        assert!(sanitize_static_path_probe_result(&spec, &rewritten).is_err());
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

    #[cfg(not(windows))]
    fn valid_directory_browse_response() -> (StaticPathBrowseSpec, Value) {
        let spec = static_path_browse_spec(HOST_TARGET_TYPE_DIRECTORY, Some("/srv/docs"), None)
            .expect("browse spec");
        let result = json!({
            "target_type": "directory",
            "platform": "posix",
            "current_path": "/srv/docs",
            "parent_path": "/srv",
            "current_selectable": true,
            "selected_path": null,
            "breadcrumbs": [
                { "name": "/", "path": "/" },
                { "name": "srv", "path": "/srv" },
                { "name": "docs", "path": "/srv/docs" },
            ],
            "entries": [
                {
                    "name": "assets",
                    "path": "/srv/docs/assets",
                    "entry_type": "directory",
                    "navigable": true,
                    "selectable": true,
                    "size_bytes": null,
                    "modified_at": "2026-08-31T01:02:03.123456789Z",
                },
                {
                    "name": "readme.txt",
                    "path": "/srv/docs/readme.txt",
                    "entry_type": "file",
                    "navigable": false,
                    "selectable": false,
                    "size_bytes": 42,
                    "modified_at": "2026-08-31T01:02:03Z",
                },
            ],
            "previous_cursor": null,
            "next_cursor": "bmV4dA",
            "error_code": null,
        });
        (spec, result)
    }

    #[cfg(not(windows))]
    #[test]
    fn browse_projection_accepts_a_bounded_canonical_directory_page() {
        let (spec, raw) = valid_directory_browse_response();
        let projected = sanitize_static_path_browse_result(&spec, &raw).unwrap();
        assert_eq!(projected["current_path"], json!("/srv/docs"));
        assert_eq!(projected["parent_path"], json!("/srv"));
        assert_eq!(projected["entries"].as_array().unwrap().len(), 2);
        assert_eq!(projected["next_cursor"], json!("bmV4dA"));
        assert!(projected["error_code"].is_null());
    }

    #[cfg(not(windows))]
    #[test]
    fn browse_projection_keeps_file_selection_across_parent_pagination() {
        let spec = static_path_browse_spec(
            HOST_TARGET_TYPE_FILE,
            Some("/srv/docs/readme.txt"),
            Some("bmV4dA"),
        )
        .expect("browse spec");
        let (_, mut raw) = valid_directory_browse_response();
        raw["target_type"] = json!("file");
        raw["current_selectable"] = json!(false);
        raw["selected_path"] = json!("/srv/docs/readme.txt");
        raw["entries"][0]["selectable"] = json!(false);
        raw["entries"][1]["selectable"] = json!(true);

        let projected = sanitize_static_path_browse_result(&spec, &raw).unwrap();
        assert_eq!(projected["selected_path"], json!("/srv/docs/readme.txt"));
        assert_eq!(projected["next_cursor"], json!("bmV4dA"));
    }

    #[cfg(not(windows))]
    #[test]
    fn browse_projection_rejects_unknown_fields_types_and_replaced_paths() {
        let (spec, raw) = valid_directory_browse_response();

        let mut diagnostic = raw.clone();
        diagnostic["diagnostic"] = json!("resolved to /secret/real/root");
        let error = sanitize_static_path_browse_result(&spec, &diagnostic).unwrap_err();
        assert!(!error.contains("secret"));

        let mut unknown_type = raw.clone();
        unknown_type["entries"][0]["entry_type"] = json!("symlink");
        assert!(sanitize_static_path_browse_result(&spec, &unknown_type).is_err());

        let mut replacement = raw;
        replacement["current_path"] = json!("/secret/real/root");
        let error = sanitize_static_path_browse_result(&spec, &replacement).unwrap_err();
        assert!(!error.contains("secret"));

        let (_, mut wrong_parent) = valid_directory_browse_response();
        wrong_parent["entries"][1]["path"] = json!("/srv/other/readme.txt");
        assert!(sanitize_static_path_browse_result(&spec, &wrong_parent).is_err());

        let (_, mut oversized_page) = valid_directory_browse_response();
        let repeated_entry = oversized_page["entries"][0].clone();
        oversized_page["entries"] = Value::Array(vec![repeated_entry; 101]);
        assert!(sanitize_static_path_browse_result(&spec, &oversized_page).is_err());

        let (_, mut oversized_cursor) = valid_directory_browse_response();
        oversized_cursor["next_cursor"] = json!("a".repeat(513));
        assert!(sanitize_static_path_browse_result(&spec, &oversized_cursor).is_err());
    }

    #[cfg(not(windows))]
    #[test]
    fn browse_projection_allows_only_empty_stable_failure_results() {
        let spec = static_path_browse_spec(HOST_TARGET_TYPE_FILE, Some("/srv/manual.pdf"), None)
            .expect("browse spec");
        for error_code in STATIC_PATH_BROWSE_ERROR_CODES {
            let raw = json!({
                "target_type": "file",
                "platform": "posix",
                "current_path": null,
                "parent_path": null,
                "current_selectable": false,
                "selected_path": null,
                "breadcrumbs": [],
                "entries": [],
                "previous_cursor": null,
                "next_cursor": null,
                "error_code": error_code,
            });
            let projected = sanitize_static_path_browse_result(&spec, &raw).unwrap();
            assert_eq!(projected["error_code"], json!(error_code));
        }

        let mut leaking = rejected_static_path_browse_result("file", "not_found");
        leaking["current_path"] = json!("/secret/real/root");
        assert!(sanitize_static_path_browse_result(&spec, &leaking).is_err());

        let unknown = rejected_static_path_browse_result("file", "internal_failure");
        assert!(sanitize_static_path_browse_result(&spec, &unknown).is_err());
    }

    #[test]
    fn browse_request_limits_paths_and_base64url_cursors() {
        assert!(is_valid_static_path_browse_cursor("Abc_123-xYz"));
        assert!(!is_valid_static_path_browse_cursor("padded="));
        assert!(!is_valid_static_path_browse_cursor("contains/slash"));
        assert!(!is_valid_static_path_browse_cursor(&"a".repeat(513)));

        assert!(normalize_posix_static_path_browse_path("/srv/docs/").is_some());
        assert!(normalize_posix_static_path_browse_path("/srv/./docs").is_none());
        assert!(normalize_posix_static_path_browse_path("/srv/../secret").is_none());
        assert!(normalize_posix_static_path_browse_path("/srv/.secret").is_none());
        assert!(normalize_posix_static_path_browse_path("/srv/__internal").is_none());
        assert_eq!(
            normalize_posix_static_path_browse_path("/srv/ docs ").as_deref(),
            Some("/srv/ docs ")
        );
    }

    #[test]
    fn windows_browse_helpers_use_drive_roots_and_server_breadcrumbs() {
        let path = normalize_windows_static_path_browse_path(r"c:/Users/Public/")
            .expect("Windows browse path");
        assert_eq!(path, r"C:\Users\Public");
        assert_eq!(
            static_path_browse_parent("windows", &path).as_deref(),
            Some(r"C:\Users")
        );
        assert_eq!(
            expected_static_path_breadcrumbs("windows", &path),
            vec![
                ("C:".to_string(), "C:\\".to_string()),
                ("Users".to_string(), r"C:\Users".to_string()),
                ("Public".to_string(), r"C:\Users\Public".to_string()),
            ]
        );
        assert!(is_windows_drive_root(r"C:\"));
        assert_eq!(
            static_path_browse_parent("windows", r"C:\").as_deref(),
            Some("")
        );
        assert!(normalize_windows_static_path_browse_path(r"\\server\share").is_none());
        assert!(normalize_windows_static_path_browse_path(r"C:\Users\.\Public").is_none());
        assert!(normalize_windows_static_path_browse_path(r"C:\Users\..\secret").is_none());
    }
}
