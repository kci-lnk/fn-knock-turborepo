use super::*;
use crate::time_utils::system_time_iso;

pub(super) async fn ensure_waf_directories(state: &AppState) -> io::Result<()> {
    fs::create_dir_all(system_dir(state)).await?;
    fs::create_dir_all(custom_dir(state)).await
}

pub(super) async fn get_manifest_cache_for_details(state: &AppState) -> anyhow::Result<Value> {
    let mut cache = read_manifest_cache(state).await?;
    if cache.get("manifest").is_none_or(Value::is_null) || is_manifest_stale(&cache) {
        let _ = refresh_system_manifest_cache(state).await;
        cache = read_manifest_cache(state).await?;
    }
    Ok(cache)
}

pub(super) async fn refresh_system_manifest_cache(state: &AppState) -> anyhow::Result<Value> {
    ensure_waf_directories(state).await?;
    let checked_at = time_utils::now_iso();
    let previous = read_manifest_cache(state).await?;
    let result = async {
        let response = state
            .fallback_client
            .get(cache_busted_url(MANIFEST_URL, None)?)
            .header("cache-control", "no-cache, no-store")
            .header("pragma", "no-cache")
            .send()
            .await?;
        if !response.status().is_success() {
            anyhow::bail!("WAF manifest request failed: {}", response.status());
        }
        let manifest = validate_manifest(response.json::<Value>().await?)?;
        let cache = json!({
            "manifest": manifest,
            "cached_at": checked_at,
            "last_checked_at": checked_at,
            "last_error": Value::Null,
        });
        write_json_file(&manifest_cache_path(state), &cache).await?;
        anyhow::Ok(cache)
    }
    .await;
    match result {
        Ok(cache) => Ok(cache),
        Err(error) => {
            let cache = json!({
                "manifest": previous.get("manifest").cloned().unwrap_or(Value::Null),
                "cached_at": previous.get("cached_at").cloned().unwrap_or(Value::Null),
                "last_checked_at": checked_at,
                "last_error": error.to_string(),
            });
            write_json_file(&manifest_cache_path(state), &cache).await?;
            Err(error)
        }
    }
}

pub(super) async fn sync_system_waf_rules(state: &AppState) -> anyhow::Result<Value> {
    let cache = refresh_system_manifest_cache(state).await?;
    let manifest = cache
        .get("manifest")
        .filter(|value| !value.is_null())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("WAF manifest is empty"))?;
    sync_system_waf_rules_from_manifest(state, &manifest).await
}

pub(super) async fn sync_system_waf_rules_from_manifest(
    state: &AppState,
    manifest: &Value,
) -> anyhow::Result<Value> {
    let zip_buffer = download_system_zip(state, manifest).await?;
    let entries = unpack_system_rules_zip(&zip_buffer)?;
    if entries.rule_files.is_empty() {
        anyhow::bail!("WAF system rule bundle contains no .conf files");
    }

    let temp_dir = waf_root_dir(state).join(format!("system.tmp-{}", time_utils::now_ms()));
    let _ = fs::remove_dir_all(&temp_dir).await;
    fs::create_dir_all(&temp_dir).await?;
    for (relative_path, content) in entries.bundle_files {
        let file_path = temp_dir.join(&relative_path);
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(file_path, content).await?;
    }
    let system_dir = system_dir(state);
    let _ = fs::remove_dir_all(&system_dir).await;
    fs::rename(&temp_dir, &system_dir).await?;

    let mut rules_state = read_rules_state(state).await?;
    let previous = rules_state.system_enabled.clone();
    rules_state.system_enabled = entries
        .rule_files
        .keys()
        .map(|filename| {
            (
                filename.clone(),
                previous
                    .get(filename)
                    .copied()
                    .unwrap_or_else(|| is_system_rule_enabled_by_default(filename)),
            )
        })
        .collect();
    write_rules_state(state, &rules_state).await?;

    write_json_file(
        &system_sync_path(state),
        &json!({
            "zip_file": manifest.get("zipFile").cloned().unwrap_or(Value::Null),
            "zip_hash": manifest.get("zipHash").cloned().unwrap_or(Value::Null),
            "synced_at": time_utils::now_iso(),
            "packaging_time": manifest.get("packagingTime").cloned().unwrap_or(Value::Null),
            "commit_hash": manifest.get("commitHash").cloned().unwrap_or(Value::Null),
            "commit_date": manifest.get("commitDate").cloned().unwrap_or(Value::Null),
        }),
    )
    .await?;

    let config = load_waf_config(state).await?;
    apply_waf_config_to_gateway(
        state,
        &config,
        "Enable WAF after at least one rule is enabled",
    )
    .await?;
    get_waf_details(state).await
}

pub(super) struct UnpackedWafBundle {
    bundle_files: Vec<(String, Vec<u8>)>,
    rule_files: BTreeMap<String, String>,
}

pub(super) fn unpack_system_rules_zip(buffer: &[u8]) -> anyhow::Result<UnpackedWafBundle> {
    let mut archive = ZipArchive::new(Cursor::new(buffer))?;
    let mut bundle_files = Vec::new();
    let mut bundle_path_keys = HashSet::new();
    let mut rule_files = BTreeMap::new();
    let mut unpacked_bytes = 0_usize;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let relative_path = safe_bundle_entry_path(file.name())?;
        let path_key = relative_path.to_ascii_lowercase();
        if !bundle_path_keys.insert(path_key) {
            anyhow::bail!("Duplicate WAF bundle file: {relative_path}");
        }

        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        unpacked_bytes = unpacked_bytes.saturating_add(content.len());
        if unpacked_bytes > MAX_UNPACKED_ZIP_BYTES {
            anyhow::bail!("WAF system rule bundle is too large after unpacking");
        }

        let filename = relative_path.rsplit('/').next().unwrap_or("").to_string();
        if is_conf_filename(&filename) {
            if relative_path != filename {
                anyhow::bail!("WAF .conf files must be in the bundle root");
            }
            let text = decode_utf8_rule(&content, &filename)?;
            bundle_files.push((relative_path, text.as_bytes().to_vec()));
            rule_files.insert(filename, text);
        } else {
            bundle_files.push((relative_path, content));
        }
    }

    bundle_files.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(UnpackedWafBundle {
        bundle_files,
        rule_files,
    })
}

pub(super) async fn download_system_zip(
    state: &AppState,
    manifest: &Value,
) -> anyhow::Result<Vec<u8>> {
    let zip_file = manifest
        .get("zipFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip file"))?;
    let expected_hash = manifest
        .get("zipHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip hash"))?
        .to_ascii_lowercase();
    let response = state
        .fallback_client
        .get(cache_busted_url(zip_file, Some(MANIFEST_URL))?)
        .header("cache-control", "no-cache, no-store")
        .header("pragma", "no-cache")
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("WAF system rule download failed: {}", response.status());
    }
    let buffer = response.bytes().await?.to_vec();
    if buffer.len() > MAX_ZIP_BYTES {
        anyhow::bail!("WAF system rule zip is too large");
    }
    let actual_hash = hex::encode(Sha256::digest(&buffer));
    if actual_hash != expected_hash {
        anyhow::bail!("WAF system rule zip hash mismatch");
    }
    Ok(buffer)
}

pub(super) fn safe_bundle_entry_path(value: &str) -> anyhow::Result<String> {
    let normalized = value.replace('\\', "/");
    let segments = normalized.split('/').collect::<Vec<_>>();
    let valid_chars = normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-' | '/'));
    if normalized.is_empty()
        || normalized != normalized.trim()
        || normalized.starts_with('/')
        || normalized.contains("://")
        || segments
            .iter()
            .any(|segment| segment.is_empty() || *segment == "." || *segment == "..")
        || !valid_chars
    {
        anyhow::bail!("Invalid WAF bundle path: {value}");
    }
    Ok(segments.join("/"))
}

pub(super) fn validate_manifest(mut value: Value) -> anyhow::Result<Value> {
    let Some(object) = value.as_object_mut() else {
        anyhow::bail!("Invalid WAF manifest");
    };
    let zip_file = object
        .get("zipFile")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip info"))?
        .to_string();
    let zip_hash = object
        .get("zipHash")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("WAF manifest missing zip info"))?
        .to_string();
    object.insert("zipFile".to_string(), Value::String(zip_file));
    object.insert("zipHash".to_string(), Value::String(zip_hash));
    Ok(value)
}

pub(super) fn is_manifest_stale(cache: &Value) -> bool {
    let checked_ms = cache
        .get("last_checked_at")
        .or_else(|| cache.get("cached_at"))
        .and_then(Value::as_str)
        .and_then(time_utils::parse_iso_ms)
        .unwrap_or(0);
    checked_ms <= 0 || time_utils::now_ms() - checked_ms > MANIFEST_REFRESH_MS
}

pub(super) async fn read_manifest_cache(state: &AppState) -> anyhow::Result<Value> {
    read_json_file(
        &manifest_cache_path(state),
        json!({
            "manifest": Value::Null,
            "cached_at": Value::Null,
            "last_checked_at": Value::Null,
            "last_error": Value::Null,
        }),
    )
    .await
}

pub(super) async fn read_system_sync_state(state: &AppState) -> anyhow::Result<Option<Value>> {
    let value = read_json_file(&system_sync_path(state), Value::Null).await?;
    Ok((!value.is_null()).then_some(value))
}

pub(super) async fn read_rules_state(state: &AppState) -> anyhow::Result<WafRulesState> {
    let state = read_json_file(&rules_state_path(state), default_rules_state()).await?;
    Ok(enforce_required_rule_state(state))
}

pub(super) async fn write_rules_state(
    state: &AppState,
    rules_state: &WafRulesState,
) -> anyhow::Result<()> {
    let normalized = enforce_required_rule_state(rules_state.clone());
    write_json_file(&rules_state_path(state), &normalized).await
}

pub(super) async fn read_json_file<T>(path: &FsPath, fallback: T) -> anyhow::Result<T>
where
    T: DeserializeOwned,
{
    match fs::read_to_string(path).await {
        Ok(raw) => Ok(serde_json::from_str::<T>(&raw)?),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(fallback),
        Err(error) => Err(error.into()),
    }
}

pub(super) async fn write_json_file<T>(path: &FsPath, value: &T) -> anyhow::Result<()>
where
    T: Serialize + ?Sized,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let raw = format!("{}\n", serde_json::to_string_pretty(value)?);
    fs::write(path, raw).await?;
    Ok(())
}

pub(super) async fn list_rule_files(
    state: &AppState,
    source: &str,
    manifest_cache: &Value,
    rules_state: &WafRulesState,
) -> anyhow::Result<Vec<Value>> {
    let dir = if source == "system" {
        system_dir(state)
    } else {
        custom_dir(state)
    };
    let descriptions = manifest_descriptions(
        manifest_cache
            .get("manifest")
            .filter(|value| !value.is_null()),
    );
    let enabled_map = if source == "system" {
        &rules_state.system_enabled
    } else {
        &rules_state.custom_enabled
    };
    let mut entries = match fs::read_dir(&dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.into()),
    };
    let mut rules = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let file_type = entry.file_type().await?;
        let filename = entry.file_name().to_string_lossy().to_string();
        if !file_type.is_file() || !is_conf_filename(&filename) {
            continue;
        }
        if source == "system" && filename == INITIALIZATION_RULE_FILENAME {
            continue;
        }
        let metadata = entry.metadata().await?;
        rules.push(json!({
            "source": source,
            "filename": filename,
            "description": descriptions
                .get(&filename)
                .cloned()
                .unwrap_or_else(|| if source == "system" {
                    "System WAF rule".to_string()
                } else {
                    "Custom WAF rule".to_string()
                }),
            "enabled": enabled_map
                .get(&filename)
                .copied()
                .unwrap_or_else(|| if source == "system" {
                    is_system_rule_enabled_by_default(&filename)
                } else {
                    true
                }),
            "size_bytes": metadata.len(),
            "updated_at": system_time_iso(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)),
        }));
    }
    rules.sort_by(|left, right| {
        left.get("filename")
            .and_then(Value::as_str)
            .unwrap_or("")
            .cmp(right.get("filename").and_then(Value::as_str).unwrap_or(""))
    });
    Ok(rules)
}

pub(super) async fn has_any_enabled_rule_files(
    state: &AppState,
    rules_state: &WafRulesState,
    omit: Option<(&str, &str)>,
) -> anyhow::Result<bool> {
    let manifest_cache = get_manifest_cache_for_details(state).await?;
    let system_rules = list_rule_files(state, "system", &manifest_cache, rules_state).await?;
    let custom_rules = list_rule_files(state, "custom", &manifest_cache, rules_state).await?;
    Ok(system_rules.into_iter().chain(custom_rules).any(|rule| {
        let source = rule.get("source").and_then(Value::as_str).unwrap_or("");
        let filename = rule.get("filename").and_then(Value::as_str).unwrap_or("");
        rule.get("enabled")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && omit != Some((source, filename))
    }))
}

pub(super) fn manifest_descriptions(manifest: Option<&Value>) -> BTreeMap<String, String> {
    manifest
        .and_then(|value| value.pointer("/rulesDescription/rules"))
        .and_then(Value::as_array)
        .map(|rules| {
            rules
                .iter()
                .filter_map(|rule| {
                    let filename = rule.get("filename")?.as_str()?.trim();
                    if filename.is_empty() {
                        return None;
                    }
                    Some((
                        filename.to_string(),
                        rule.get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .trim()
                            .to_string(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn default_rules_state() -> WafRulesState {
    let mut system_enabled = BTreeMap::new();
    system_enabled.insert(INITIALIZATION_RULE_FILENAME.to_string(), true);
    WafRulesState {
        system_enabled,
        custom_enabled: BTreeMap::new(),
    }
}

pub(super) fn enforce_required_rule_state(mut state: WafRulesState) -> WafRulesState {
    state
        .system_enabled
        .insert(INITIALIZATION_RULE_FILENAME.to_string(), true);
    state
}

pub(super) fn is_system_rule_enabled_by_default(filename: &str) -> bool {
    filename == INITIALIZATION_RULE_FILENAME
        || !DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES.contains(&filename)
}

pub(super) async fn make_unique_custom_filename(
    state: &AppState,
    filename: &str,
) -> anyhow::Result<String> {
    let ext = FsPath::new(filename)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let base = filename.strip_suffix(&ext).unwrap_or(filename);
    let mut candidate = filename.to_string();
    let mut index = 1;
    loop {
        match fs::metadata(custom_dir(state).join(&candidate)).await {
            Ok(_) => {
                candidate = format!("{base}-{index}{ext}");
                index += 1;
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(candidate),
            Err(error) => return Err(error.into()),
        }
    }
}

pub(super) fn safe_rule_filename(value: &str) -> anyhow::Result<String> {
    let raw = value
        .replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .trim()
        .to_string();
    if raw.is_empty() || raw == "." || raw == ".." || !is_conf_filename(&raw) {
        anyhow::bail!("Only .conf WAF rule files are supported");
    }
    let safe = raw
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '-'
            }
        })
        .collect::<String>();
    if safe.is_empty() || !is_conf_filename(&safe) {
        anyhow::bail!("Invalid WAF rule filename");
    }
    Ok(safe)
}

pub(super) fn normalize_rule_source(source: &str) -> anyhow::Result<&'static str> {
    match source {
        "system" => Ok("system"),
        "custom" => Ok("custom"),
        _ => anyhow::bail!("Invalid WAF rule source"),
    }
}

pub(super) fn decode_utf8_rule(content: &[u8], filename: &str) -> anyhow::Result<String> {
    if content.len() > MAX_RULE_FILE_BYTES {
        anyhow::bail!("WAF rule file is too large: {filename}");
    }
    let text = String::from_utf8(content.to_vec())
        .map_err(|_| anyhow::anyhow!("WAF rule file is not valid UTF-8: {filename}"))?;
    let text = text.trim_start_matches('\u{feff}').to_string();
    if contains_blocked_directive(&text) {
        anyhow::bail!("WAF rule file contains blocked filesystem directives: {filename}");
    }
    Ok(text)
}

pub(super) fn read_utf8_rule_text(content: &[u8], filename: &str) -> anyhow::Result<String> {
    if content.len() > MAX_RULE_FILE_BYTES {
        anyhow::bail!("WAF rule file is too large: {filename}");
    }
    let text = String::from_utf8(content.to_vec())
        .map_err(|_| anyhow::anyhow!("WAF rule file is not valid UTF-8: {filename}"))?;
    Ok(text.trim_start_matches('\u{feff}').to_string())
}

pub(super) fn contains_blocked_directive(text: &str) -> bool {
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        [
            "Include",
            "SecAuditLog",
            "SecDebugLog",
            "SecDataDir",
            "SecTmpDir",
            "SecUploadDir",
        ]
        .iter()
        .any(|directive| starts_with_directive(trimmed, directive))
    })
}

pub(super) fn starts_with_directive(line: &str, directive: &str) -> bool {
    let Some(prefix) = line.get(..directive.len()) else {
        return false;
    };
    if !prefix.eq_ignore_ascii_case(directive) {
        return false;
    }
    line[directive.len()..]
        .chars()
        .next()
        .is_none_or(char::is_whitespace)
}

pub(super) fn is_conf_filename(filename: &str) -> bool {
    filename.to_ascii_lowercase().ends_with(".conf")
}

pub(super) fn normalize_i64(value: Option<&Value>, fallback: i64, min: i64, max: i64) -> i64 {
    let parsed = value
        .and_then(|value| {
            value
                .as_i64()
                .or_else(|| value.as_str().and_then(|raw| raw.parse::<i64>().ok()))
        })
        .unwrap_or(fallback);
    parsed.clamp(min, max)
}

pub(super) fn has_any_key(value: &Value, keys: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|object| keys.iter().any(|key| object.contains_key(*key)))
}

pub(super) fn cache_busted_url(input: &str, base: Option<&str>) -> anyhow::Result<String> {
    let mut url = if let Some(base) = base {
        url::Url::parse(base)?.join(input)?
    } else {
        url::Url::parse(input)?
    };
    url.query_pairs_mut().append_pair(
        "t",
        &format!("{}-{}", time_utils::now_ms(), uuid::Uuid::new_v4()),
    );
    Ok(url.to_string())
}

pub(super) fn waf_root_dir(state: &AppState) -> PathBuf {
    state.settings.gateway_config_dir.join("waf")
}

pub(super) fn system_dir(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("system")
}

pub(super) fn custom_dir(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("custom")
}

pub(super) fn manifest_cache_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("manifest.json")
}

pub(super) fn system_sync_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("system-sync.json")
}

pub(super) fn rules_state_path(state: &AppState) -> PathBuf {
    waf_root_dir(state).join("rules-state.json")
}

pub(super) fn rule_file_path(state: &AppState, source: &str, filename: &str) -> PathBuf {
    if source == "system" {
        system_dir(state).join(filename)
    } else {
        custom_dir(state).join(filename)
    }
}
