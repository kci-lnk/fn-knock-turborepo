use super::*;

pub(super) async fn load_fnos_network_tuning_status(state: &AppState) -> anyhow::Result<Value> {
    let config = state.redis.get_config().await?;
    let tuning = normalize_fnos_network_tuning(config.get("fnos_network_tuning"));
    Ok(build_fnos_network_tuning_status(state, tuning))
}

pub(super) async fn update_fnos_network_tuning_config(
    state: &AppState,
    patch: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let patch = normalize_fnos_network_tuning_patch(patch, translator)?;
    let previous_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let previous = normalize_fnos_network_tuning(previous_config.get("fnos_network_tuning"));
    let before = read_fnos_kernel_state();
    let mut next = build_next_fnos_network_tuning_config(&previous, &patch, &before);
    let result = (|| {
        let transition_targets =
            apply_fnos_network_tuning_transition(&previous, &next, &patch, &before, translator)?;
        let verified_state = read_fnos_kernel_state();
        verify_fnos_network_tuning_state(
            &next,
            &patch,
            &verified_state,
            &transition_targets,
            translator,
        )?;
        Ok::<Value, String>(verified_state)
    })();

    let verified_state = match result {
        Ok(state) => state,
        Err(error) => {
            mark_fnos_network_tuning_failure(
                state,
                &previous_config,
                &previous,
                &before,
                &error,
                translator,
            )
            .await;
            return Err(error);
        }
    };

    clear_fnos_network_tuning_last_error(&mut next);
    let mut config = previous_config.clone();
    ensure_config_object(&mut config).insert("fnos_network_tuning".to_string(), next.clone());
    if let Err(error) = state.redis.save_config(&config).await {
        let message = error.to_string();
        mark_fnos_network_tuning_failure(
            state,
            &previous_config,
            &previous,
            &before,
            &message,
            translator,
        )
        .await;
        return Err(message);
    }
    if let Err(error) = write_fnos_network_tuning_sysctl_config(&next) {
        mark_fnos_network_tuning_failure(
            state,
            &previous_config,
            &previous,
            &before,
            &error,
            translator,
        )
        .await;
        return Err(error);
    }
    let saved_config = state
        .redis
        .get_config()
        .await
        .map_err(|error| error.to_string())?;
    let saved = normalize_fnos_network_tuning(saved_config.get("fnos_network_tuning"));
    Ok(build_fnos_network_tuning_status_with_state(
        state,
        saved,
        verified_state,
    ))
}

pub(super) fn normalize_fnos_network_tuning_patch(
    patch: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let mut normalized = serde_json::Map::new();
    if let Some(value) = bool_patch_alias(patch, "bbr_enabled", "bbrEnabled") {
        normalized.insert("bbr_enabled".to_string(), Value::Bool(value));
    }
    if let Some(value) = bool_patch_alias(patch, "mtu_probing_enabled", "mtuProbingEnabled") {
        normalized.insert("mtu_probing_enabled".to_string(), Value::Bool(value));
    }
    if normalized.is_empty() {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.emptyPatch",
        ));
    }
    Ok(Value::Object(normalized))
}

pub(super) fn bool_patch_alias(patch: &Value, snake_key: &str, camel_key: &str) -> Option<bool> {
    patch
        .get(snake_key)
        .and_then(Value::as_bool)
        .or_else(|| patch.get(camel_key).and_then(Value::as_bool))
}

pub(super) fn build_next_fnos_network_tuning_config(
    previous: &Value,
    patch: &Value,
    before: &Value,
) -> Value {
    let mut next = previous.clone();
    if let Some(object) = next.as_object_mut() {
        if let Some(value) = patch.get("bbr_enabled").and_then(Value::as_bool) {
            if value && previous.get("bbr_enabled").and_then(Value::as_bool) != Some(true) {
                object.insert(
                    "previous_tcp_congestion_control".to_string(),
                    before
                        .get("tcp_congestion_control")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                object.insert(
                    "previous_default_qdisc".to_string(),
                    before.get("default_qdisc").cloned().unwrap_or(Value::Null),
                );
            }
            object.insert("bbr_enabled".to_string(), Value::Bool(value));
        }
        if let Some(value) = patch.get("mtu_probing_enabled").and_then(Value::as_bool) {
            if value && previous.get("mtu_probing_enabled").and_then(Value::as_bool) != Some(true) {
                object.insert(
                    "previous_tcp_mtu_probing".to_string(),
                    before
                        .get("tcp_mtu_probing")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
            }
            object.insert("mtu_probing_enabled".to_string(), Value::Bool(value));
        }
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
        object.insert("last_error".to_string(), Value::Null);
    }
    normalize_fnos_network_tuning(Some(&next))
}

pub(super) fn clear_fnos_network_tuning_last_error(config: &mut Value) {
    if let Some(object) = config.as_object_mut() {
        object.insert("last_error".to_string(), Value::Null);
    }
}

pub(super) fn build_fnos_network_tuning_status(state: &AppState, config: Value) -> Value {
    let kernel_state = read_fnos_kernel_state();
    build_fnos_network_tuning_status_with_state(state, config, kernel_state)
}

pub(super) fn build_fnos_network_tuning_status_with_state(
    state: &AppState,
    config: Value,
    kernel_state: Value,
) -> Value {
    let blocked_reason_code = fnos_network_tuning_blocked_reason_code(state);
    let available = fnos_network_tuning_available(blocked_reason_code.as_deref());
    let blocked_reason = blocked_reason_code
        .as_deref()
        .map(fnos_network_tuning_blocked_reason_fallback);
    json!({
        "available": available && blocked_reason_code.is_none(),
        "blocked_reason_code": blocked_reason_code.map(Value::String).unwrap_or(Value::Null),
        "blocked_reason": blocked_reason.map(Value::String).unwrap_or(Value::Null),
        "managed_config_path": fnos_network_tuning_sysctl_path().to_string_lossy(),
        "config": config.clone(),
        "state": kernel_state,
        "bbr": {
            "desired_enabled": config.get("bbr_enabled").and_then(Value::as_bool).unwrap_or(false),
            "active": kernel_state.get("bbr_active").and_then(Value::as_bool).unwrap_or(false),
            "supported": kernel_state.get("bbr_supported").and_then(Value::as_bool).unwrap_or(false),
            "module_loaded": kernel_state.get("bbr_module_loaded").and_then(Value::as_bool).unwrap_or(false),
            "current_congestion_control": kernel_state.get("tcp_congestion_control").cloned().unwrap_or(Value::Null),
            "current_default_qdisc": kernel_state.get("default_qdisc").cloned().unwrap_or(Value::Null),
            "available_congestion_control": kernel_state.get("tcp_available_congestion_control").cloned().unwrap_or_else(|| json!([])),
        },
        "mtu_probing": {
            "desired_enabled": config.get("mtu_probing_enabled").and_then(Value::as_bool).unwrap_or(false),
            "active": kernel_state.get("mtu_probing_active").and_then(Value::as_bool).unwrap_or(false),
            "current_value": kernel_state.get("tcp_mtu_probing").cloned().unwrap_or(Value::Null),
        },
        "last_error": config.get("last_error").cloned().unwrap_or(Value::Null),
    })
}

pub(super) fn localize_fnos_network_tuning_status(
    mut status: Value,
    translator: &Translator,
) -> Value {
    let reason_code = status
        .get("blocked_reason_code")
        .and_then(Value::as_str)
        .map(str::to_string);
    if let (Some(object), Some(reason_code)) = (status.as_object_mut(), reason_code) {
        object.insert(
            "blocked_reason".to_string(),
            Value::String(fnos_network_tuning_blocked_reason(&reason_code, translator)),
        );
    }
    status
}

#[derive(Default)]
pub(super) struct FnosNetworkTuningTransitionTargets {
    disabled_bbr_congestion_control: Option<String>,
    disabled_bbr_default_qdisc: Option<String>,
    disabled_tcp_mtu_probing: Option<String>,
}

pub(super) fn verify_fnos_network_tuning_state(
    config: &Value,
    patch: &Value,
    state: &Value,
    targets: &FnosNetworkTuningTransitionTargets,
    translator: &Translator,
) -> Result<(), String> {
    if config.get("bbr_enabled").and_then(Value::as_bool) == Some(true)
        && state.get("bbr_active").and_then(Value::as_bool) != Some(true)
    {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.bbrEnableVerificationFailed",
        ));
    }
    if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(false) {
        let expected_congestion = targets
            .disabled_bbr_congestion_control
            .clone()
            .or_else(|| config_string(config, "previous_tcp_congestion_control"));
        let expected_qdisc = targets
            .disabled_bbr_default_qdisc
            .clone()
            .or_else(|| config_string(config, "previous_default_qdisc"));
        let current_congestion = state
            .get("tcp_congestion_control")
            .and_then(Value::as_str)
            .unwrap_or("");
        let current_qdisc = state
            .get("default_qdisc")
            .and_then(Value::as_str)
            .unwrap_or("");
        if let Some(expected) = expected_congestion.as_deref()
            && expected != current_congestion
        {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackCongestionFailed",
            ));
        }
        if let Some(expected) = expected_qdisc.as_deref()
            && expected != current_qdisc
        {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackQdiscFailed",
            ));
        }
        if expected_congestion.is_none() && current_congestion == "bbr" {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.bbrRollbackStillBbrFailed",
            ));
        }
    }
    if config.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true)
        && state.get("tcp_mtu_probing").and_then(Value::as_str) != Some("1")
    {
        return Err(admin_text(
            translator,
            "fnosNetworkTuning.errors.mtuEnableVerificationFailed",
        ));
    }
    if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(false) {
        let expected_mtu = targets
            .disabled_tcp_mtu_probing
            .clone()
            .unwrap_or_else(|| "0".to_string());
        if state.get("tcp_mtu_probing").and_then(Value::as_str) != Some(expected_mtu.as_str()) {
            return Err(admin_text(
                translator,
                "fnosNetworkTuning.errors.mtuRollbackFailed",
            ));
        }
    }
    Ok(())
}

pub(super) fn read_fnos_kernel_state() -> Value {
    let congestion = read_sysctl("net.ipv4.tcp_congestion_control");
    let available = read_sysctl("net.ipv4.tcp_available_congestion_control")
        .map(|value| {
            value
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let qdisc = read_sysctl("net.core.default_qdisc");
    let mtu = read_sysctl("net.ipv4.tcp_mtu_probing");
    let bbr_module_loaded = read_bbr_module_loaded();
    let bbr_supported =
        available.iter().any(|value| value == "bbr") || congestion.as_deref() == Some("bbr");
    let bbr_active = congestion.as_deref() == Some("bbr") && qdisc.as_deref() == Some("fq");
    json!({
        "tcp_congestion_control": congestion.map(Value::String).unwrap_or(Value::Null),
        "tcp_available_congestion_control": available,
        "default_qdisc": qdisc.map(Value::String).unwrap_or(Value::Null),
        "tcp_mtu_probing": mtu.clone().map(Value::String).unwrap_or(Value::Null),
        "bbr_module_loaded": bbr_module_loaded,
        "bbr_supported": bbr_supported,
        "bbr_active": bbr_active,
        "mtu_probing_active": fnos_mtu_probing_active(mtu.as_deref()),
    })
}

pub(super) fn fnos_mtu_probing_active(value: Option<&str>) -> bool {
    value == Some("1")
}

pub(super) fn read_bbr_module_loaded() -> bool {
    fs::read_to_string("/proc/modules")
        .is_ok_and(|modules| bbr_module_loaded_from_proc_modules(&modules))
}

pub(super) fn bbr_module_loaded_from_proc_modules(modules: &str) -> bool {
    modules
        .lines()
        .any(|line| line.split_whitespace().next() == Some("tcp_bbr"))
}

pub(super) fn read_sysctl(key: &str) -> Option<String> {
    Command::new("sysctl")
        .args(["-n", key])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(super) fn apply_fnos_network_tuning_transition(
    previous: &Value,
    next: &Value,
    patch: &Value,
    before_state: &Value,
    translator: &Translator,
) -> Result<FnosNetworkTuningTransitionTargets, String> {
    let mut targets = FnosNetworkTuningTransitionTargets::default();
    if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    } else if patch.get("bbr_enabled").and_then(Value::as_bool) == Some(false) {
        let fallback = fnos_congestion_fallback(before_state);
        let previous_congestion =
            config_string(next, "previous_tcp_congestion_control").filter(|value| value != "bbr");
        targets.disabled_bbr_congestion_control = Some(write_sysctl_from_candidates(
            "net.ipv4.tcp_congestion_control",
            unique_fnos_network_candidates(vec![previous_congestion, Some(fallback)]),
            translator,
        )?);
        targets.disabled_bbr_default_qdisc = Some(write_sysctl_from_candidates(
            "net.core.default_qdisc",
            unique_fnos_network_candidates(vec![
                config_string(next, "previous_default_qdisc"),
                Some("pfifo_fast".to_string()),
            ]),
            translator,
        )?);
    } else if next.get("bbr_enabled").and_then(Value::as_bool) == Some(true)
        && previous.get("bbr_enabled").and_then(Value::as_bool) != Some(true)
    {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    }

    if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
        write_sysctl("net.ipv4.tcp_mtu_probing", "1")?;
    } else if patch.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(false) {
        write_sysctl("net.ipv4.tcp_mtu_probing", "0")?;
        targets.disabled_tcp_mtu_probing = Some("0".to_string());
    }

    Ok(targets)
}

pub(super) fn ensure_bbr_supported(translator: &Translator) -> Result<(), String> {
    let _ = Command::new("modprobe").arg("tcp_bbr").output();
    let state = read_fnos_kernel_state();
    if state.get("bbr_supported").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }
    Err(admin_text(
        translator,
        "fnosNetworkTuning.errors.bbrNotSupported",
    ))
}

pub(super) fn unique_fnos_network_candidates(values: Vec<Option<String>>) -> Vec<String> {
    let mut candidates = Vec::new();
    for value in values {
        let Some(value) = value else {
            continue;
        };
        let trimmed = value.trim();
        if trimmed.is_empty() || candidates.iter().any(|candidate| candidate == trimmed) {
            continue;
        }
        candidates.push(trimmed.to_string());
    }
    candidates
}

pub(super) fn write_sysctl_from_candidates(
    key: &str,
    candidates: Vec<String>,
    translator: &Translator,
) -> Result<String, String> {
    let mut last_error = None;
    for candidate in candidates {
        match write_sysctl(key, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        admin_text_params(
            translator,
            "fnosNetworkTuning.errors.setSysctlFailed",
            &[("key", key.to_string())],
        )
    }))
}

pub(super) fn config_string(config: &Value, key: &str) -> Option<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn kernel_state_string(state: &Value, key: &str) -> Option<String> {
    state
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(super) fn restore_fnos_network_tuning_runtime(
    previous: &Value,
    before_state: &Value,
    translator: &Translator,
) -> Result<(), String> {
    if previous.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        ensure_bbr_supported(translator)?;
        write_sysctl("net.core.default_qdisc", "fq")?;
        write_sysctl("net.ipv4.tcp_congestion_control", "bbr")?;
    } else {
        if let Some(congestion) = kernel_state_string(before_state, "tcp_congestion_control") {
            write_sysctl("net.ipv4.tcp_congestion_control", &congestion)?;
        }
        if let Some(qdisc) = kernel_state_string(before_state, "default_qdisc") {
            write_sysctl("net.core.default_qdisc", &qdisc)?;
        }
    }

    if previous.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
        write_sysctl("net.ipv4.tcp_mtu_probing", "1")?;
    } else {
        write_sysctl("net.ipv4.tcp_mtu_probing", "0")?;
    }

    Ok(())
}

pub(super) async fn mark_fnos_network_tuning_failure(
    state: &AppState,
    previous_config: &Value,
    previous: &Value,
    before_state: &Value,
    message: &str,
    translator: &Translator,
) {
    let mut message = message.to_string();
    if let Err(error) = write_fnos_network_tuning_sysctl_config(previous)
        .and_then(|_| restore_fnos_network_tuning_runtime(previous, before_state, translator))
    {
        message = admin_text_params(
            translator,
            "fnosNetworkTuning.errors.rollbackFailed",
            &[("message", message), ("error", error)],
        );
    }

    let mut failed = previous.clone();
    if let Some(object) = failed.as_object_mut() {
        object.insert("last_error".to_string(), Value::String(message));
        object.insert(
            "updated_at".to_string(),
            Value::String(time_utils::now_iso()),
        );
    }
    let mut config = previous_config.clone();
    ensure_config_object(&mut config).insert("fnos_network_tuning".to_string(), failed);
    let _ = state.redis.save_config(&config).await;
}

pub(super) fn fnos_congestion_fallback(state: &Value) -> String {
    state
        .get("tcp_available_congestion_control")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .find(|value| *value == "cubic")
                .or_else(|| {
                    items
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|value| !value.trim().is_empty() && *value != "bbr")
                })
        })
        .unwrap_or("cubic")
        .to_string()
}

pub(super) fn write_sysctl(key: &str, value: &str) -> Result<(), String> {
    let output = Command::new("sysctl")
        .arg("-w")
        .arg(format!("{key}={value}"))
        .output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub(super) fn write_fnos_network_tuning_sysctl_config(config: &Value) -> Result<(), String> {
    let path = fnos_network_tuning_sysctl_path();
    let lines = render_fnos_network_tuning_sysctl_config(config);
    if lines.is_empty() {
        let _ = fs::remove_file(&path);
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|value| value.to_str())
            .unwrap_or("conf")
    ));
    fs::write(&tmp, lines.join("\n")).map_err(|error| error.to_string())?;
    fs::rename(&tmp, &path).map_err(|error| error.to_string())
}

pub(super) fn render_fnos_network_tuning_sysctl_config(config: &Value) -> Vec<String> {
    let mut lines = vec![
        "# Managed by fn-knock. Do not edit this file manually.".to_string(),
        "# Source: System settings -> FNOS network tuning.".to_string(),
    ];
    if config.get("bbr_enabled").and_then(Value::as_bool) == Some(true) {
        lines.push("net.core.default_qdisc=fq".to_string());
        lines.push("net.ipv4.tcp_congestion_control=bbr".to_string());
    }
    lines.push(format!(
        "net.ipv4.tcp_mtu_probing={}",
        if config.get("mtu_probing_enabled").and_then(Value::as_bool) == Some(true) {
            "1"
        } else {
            "0"
        }
    ));
    lines.push(String::new());
    lines
}

pub(super) fn fnos_network_tuning_sysctl_path() -> std::path::PathBuf {
    std::env::var("FN_KNOCK_NETWORK_SYSCTL_PATH")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(FNOS_NETWORK_TUNING_SYSCTL_PATH))
}

pub(super) fn fnos_network_tuning_blocked_reason_code(state: &AppState) -> Option<String> {
    let profile = runtime_profile::get_runtime_profile(state);
    if profile.deployment_target != "fpk" {
        return Some("deployment".to_string());
    }
    if !profile.is_linux {
        return Some("platform".to_string());
    }
    if !profile.is_root_process {
        return Some("permission".to_string());
    }
    None
}

pub(super) fn fnos_network_tuning_available(blocked_reason_code: Option<&str>) -> bool {
    blocked_reason_code.is_none()
}

pub(super) fn fnos_network_tuning_blocked_reason(
    reason_code: &str,
    translator: &Translator,
) -> String {
    translator.t_with_fallback(
        &format!("server.admin.fnosNetworkTuning.blocked.{reason_code}"),
        &fnos_network_tuning_blocked_reason_fallback(reason_code),
    )
}

pub(super) fn fnos_network_tuning_blocked_reason_fallback(reason_code: &str) -> String {
    match reason_code {
        "deployment" => "飞牛 FPK 网络优化仅支持 FPK 部署。",
        "platform" => "飞牛 FPK 网络优化需要 Linux 宿主环境。",
        "permission" => "飞牛 FPK 网络优化需要 root 权限。",
        _ => "飞牛 FPK 网络优化不可用。",
    }
    .to_string()
}
