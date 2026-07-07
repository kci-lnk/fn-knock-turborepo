use std::{fs, path::Path, process::Command, sync::Mutex};

use serde_json::{Value, json};

use crate::{i18n::Translator, time_utils};

use super::{
    DNSMASQ_INSTALL, DnsmasqInstallState, SMART_CONNECT_LOCAL_TTL_SECONDS,
    SMART_CONNECT_MANAGED_CONF_PATH,
    process::run_process_success,
    text::{dnsmasq_text, dnsmasq_text_params},
};

pub(crate) fn build_dnsmasq_status_with_translator(translator: &Translator) -> Value {
    let current = dnsmasq_install_state();
    let executable = detect_dnsmasq_executable();
    let raw_service_active = dnsmasq_service_active();
    let service_active = if executable.is_none() && current.status != "installing" {
        false
    } else {
        raw_service_active
    };
    let initialized = current.status != "installing"
        && executable
            .as_ref()
            .is_some_and(|(path, _)| dnsmasq_can_initialize(path));
    let has_service_definition =
        current.status != "installing" && executable.is_some() && has_service_definition();
    let version = executable.as_ref().map(|(_, version)| version.as_str());
    let install_state = resolve_dnsmasq_install_state(
        translator,
        version,
        service_active,
        initialized,
        has_service_definition,
        current,
    );
    json!({
        "installed": executable.is_some(),
        "service_active": service_active,
        "initialized": initialized,
        "version": executable.map(|(_, version)| version).unwrap_or_default(),
        "install_state": dnsmasq_install_state_to_json(&install_state, translator)
    })
}

pub(super) fn dnsmasq_state(status: &str, progress: i64, message: String) -> DnsmasqInstallState {
    DnsmasqInstallState {
        status: status.to_string(),
        progress,
        message,
    }
}

pub(super) fn dnsmasq_ready_message(translator: &Translator, version: &str) -> String {
    if version.trim().is_empty() {
        dnsmasq_text(translator, "ready")
    } else {
        dnsmasq_text_params(
            translator,
            "readyWithVersion",
            &[("version", version.to_string())],
        )
    }
}

pub(super) fn dnsmasq_detected_message(
    translator: &Translator,
    version: &str,
    has_service_definition: bool,
) -> String {
    if !has_service_definition {
        dnsmasq_text(translator, "missingServiceAutoComplete")
    } else if version.trim().is_empty() {
        dnsmasq_text(translator, "detected")
    } else {
        dnsmasq_text_params(
            translator,
            "detectedWithVersion",
            &[("version", version.to_string())],
        )
    }
}

pub(super) fn resolve_dnsmasq_install_state(
    translator: &Translator,
    executable_version: Option<&str>,
    service_active: bool,
    initialized: bool,
    has_service_definition: bool,
    current: DnsmasqInstallState,
) -> DnsmasqInstallState {
    if current.status == "installing" {
        return current;
    }
    let Some(version) = executable_version else {
        return if current.status == "error" {
            current
        } else {
            dnsmasq_state(
                "uninstalled",
                0,
                dnsmasq_text(translator, "notDetectedInstallFirst"),
            )
        };
    };
    if service_active && initialized {
        return dnsmasq_state("installed", 100, dnsmasq_ready_message(translator, version));
    }
    if current.status == "error" {
        return current;
    }
    dnsmasq_state(
        "installed",
        100,
        dnsmasq_detected_message(translator, version, has_service_definition),
    )
}

pub(super) fn detect_dnsmasq_executable() -> Option<(String, String)> {
    for candidate in ["dnsmasq", "/usr/sbin/dnsmasq", "/usr/bin/dnsmasq"] {
        let Ok(output) = Command::new(candidate).arg("--version").output() else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let version = stdout
            .lines()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("dnsmasq")
            .to_string();
        return Some((candidate.to_string(), version));
    }
    None
}

pub(super) fn dnsmasq_service_active() -> bool {
    if has_systemd_unit()
        && run_process_success("systemctl", &["is-active", "--quiet", "dnsmasq"]).is_ok()
    {
        return true;
    }
    has_init_script() && run_process_success("service", &["dnsmasq", "status"]).is_ok()
}

pub(super) fn dnsmasq_can_initialize(executable_path: &str) -> bool {
    if fs::create_dir_all(
        Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
            .parent()
            .unwrap_or_else(|| Path::new("/etc/dnsmasq.d")),
    )
    .is_err()
    {
        return false;
    }
    let test_path = Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
        .parent()
        .unwrap_or_else(|| Path::new("/etc/dnsmasq.d"))
        .join(format!(".fn-knock-write-test-{}", time_utils::now_ms()));
    if fs::write(&test_path, "").is_err() {
        return false;
    }
    let _ = fs::remove_file(test_path);
    validate_dnsmasq_config(executable_path, &dnsmasq_bootstrap_config()).is_ok()
}

pub(super) fn install_dnsmasq_background(already_installed: bool, translator: Translator) {
    let result = if already_installed {
        initialize_dnsmasq(&translator)
    } else {
        install_dnsmasq_package(&translator)
    };
    if let Err(error) = result {
        set_dnsmasq_install_state("error", 0, error);
    }
}

pub(super) fn install_dnsmasq_package(translator: &Translator) -> Result<(), String> {
    set_dnsmasq_install_state("installing", 15, dnsmasq_text(translator, "refreshingApt"));
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["update"],
        "aptUpdateFailed",
    )?;

    set_dnsmasq_install_state("installing", 55, dnsmasq_text(translator, "installing"));
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["install", "-y", "dnsmasq"],
        "aptInstallFailed",
    )?;

    initialize_dnsmasq(translator)
}

pub(super) fn initialize_dnsmasq(translator: &Translator) -> Result<(), String> {
    set_dnsmasq_install_state(
        "installing",
        20,
        dnsmasq_text(translator, "checkingEnvironment"),
    );
    let executable = detect_dnsmasq_executable()
        .ok_or_else(|| dnsmasq_text(translator, "notDetectedInstallFirst"))?;

    set_dnsmasq_install_state(
        "installing",
        45,
        dnsmasq_text(translator, "validatingConfig"),
    );
    ensure_dnsmasq_service_package_installed(translator)?;
    fs::create_dir_all(
        Path::new(SMART_CONNECT_MANAGED_CONF_PATH)
            .parent()
            .unwrap_or_else(|| Path::new("/etc/dnsmasq.d")),
    )
    .map_err(|error| error.to_string())?;
    validate_dnsmasq_config(&executable.0, &dnsmasq_bootstrap_config())
        .map_err(|error| normalize_dnsmasq_error(translator, &error, "configTestFailed"))?;

    set_dnsmasq_install_state(
        "installing",
        72,
        dnsmasq_text(translator, "enablingService"),
    );
    enable_dnsmasq_on_boot();

    set_dnsmasq_install_state(
        "installing",
        90,
        dnsmasq_text(translator, "startingService"),
    );
    restart_dnsmasq_service(translator)?;

    set_dnsmasq_install_state(
        "installed",
        100,
        dnsmasq_ready_message(translator, &executable.1),
    );
    Ok(())
}

pub(super) fn ensure_dnsmasq_service_package_installed(
    translator: &Translator,
) -> Result<(), String> {
    if has_service_definition() {
        return Ok(());
    }
    if !Path::new("/usr/bin/apt-get").exists() {
        return Err(dnsmasq_text(translator, "servicePackageMissing"));
    }
    set_dnsmasq_install_state(
        "installing",
        58,
        dnsmasq_text(translator, "completingService"),
    );
    run_dnsmasq_process_success(
        translator,
        "/usr/bin/apt-get",
        &["install", "-y", "dnsmasq"],
        "completeServiceFailed",
    )?;
    if !has_service_definition() {
        return Err(dnsmasq_text(
            translator,
            "serviceDefinitionMissingAfterInstall",
        ));
    }
    Ok(())
}

pub(super) fn dnsmasq_bootstrap_config() -> String {
    [
        format!("local-ttl={SMART_CONNECT_LOCAL_TTL_SECONDS}"),
        "listen-address=127.0.0.1".to_string(),
        "bind-interfaces".to_string(),
        String::new(),
    ]
    .join("\n")
}

pub(super) fn validate_dnsmasq_config(executable_path: &str, content: &str) -> Result<(), String> {
    let temp_dir = std::env::temp_dir().join(format!("fn-knock-dnsmasq-{}", time_utils::now_ms()));
    fs::create_dir_all(&temp_dir).map_err(|error| error.to_string())?;
    let temp_conf_path = temp_dir.join("dnsmasq.conf");
    let result = (|| {
        fs::write(&temp_conf_path, content).map_err(|error| error.to_string())?;
        run_process_success(
            executable_path,
            &[
                "--test",
                &format!("--conf-file={}", temp_conf_path.display()),
            ],
        )
    })();
    let _ = fs::remove_dir_all(temp_dir);
    result.map(|_| ())
}

pub(super) fn restart_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    let mut errors = Vec::new();
    if has_systemd_unit() {
        match run_dnsmasq_process_success(
            translator,
            "systemctl",
            &["restart", "dnsmasq"],
            "restartFailed",
        ) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    if has_init_script() {
        match run_dnsmasq_process_success(
            translator,
            "service",
            &["dnsmasq", "restart"],
            "restartFailed",
        ) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Err(dnsmasq_text(translator, "serviceDefinitionMissing"))
    } else {
        Err(errors.join(" | "))
    }
}

pub(super) fn enable_dnsmasq_on_boot() {
    if has_systemd_unit() {
        let _ = run_process_success("systemctl", &["enable", "dnsmasq"]);
        return;
    }
    if has_init_script() {
        let _ = run_process_success("update-rc.d", &["dnsmasq", "defaults"]);
    }
}

pub(super) fn has_service_definition() -> bool {
    has_systemd_unit() || has_init_script()
}

pub(super) fn has_systemd_unit() -> bool {
    [
        "/etc/systemd/system/dnsmasq.service",
        "/lib/systemd/system/dnsmasq.service",
        "/usr/lib/systemd/system/dnsmasq.service",
    ]
    .iter()
    .any(|path| Path::new(path).exists())
}

pub(super) fn has_init_script() -> bool {
    Path::new("/etc/init.d/dnsmasq").exists()
}

pub(super) fn dnsmasq_install_state() -> DnsmasqInstallState {
    dnsmasq_install_state_lock()
        .lock()
        .expect("dnsmasq install mutex poisoned")
        .clone()
}

pub(super) fn dnsmasq_install_state_json(translator: &Translator) -> Value {
    dnsmasq_install_state_to_json(&dnsmasq_install_state(), translator)
}

pub(super) fn dnsmasq_install_state_to_json(
    state: &DnsmasqInstallState,
    translator: &Translator,
) -> Value {
    json!({
        "status": state.status,
        "progress": state.progress,
        "message": localize_dnsmasq_install_message(state, translator)
    })
}

pub(super) fn localize_dnsmasq_install_message(
    state: &DnsmasqInstallState,
    translator: &Translator,
) -> String {
    let message = state.message.trim();
    if state.status == "uninstalled"
        && (message.is_empty()
            || message == "dnsmasq is not detected"
            || message == "dnsmasq was not detected. Install it first.")
    {
        return dnsmasq_text(translator, "notDetectedInstallFirst");
    }
    state.message.clone()
}

pub(super) fn set_dnsmasq_install_state(
    status: impl Into<String>,
    progress: i64,
    message: impl Into<String>,
) {
    let mut guard = dnsmasq_install_state_lock()
        .lock()
        .expect("dnsmasq install mutex poisoned");
    guard.status = status.into();
    guard.progress = progress.clamp(0, 100);
    guard.message = message.into();
}

pub(super) fn dnsmasq_install_state_lock() -> &'static Mutex<DnsmasqInstallState> {
    DNSMASQ_INSTALL.get_or_init(|| Mutex::new(DnsmasqInstallState::default()))
}

pub(super) fn run_dnsmasq_process_success(
    translator: &Translator,
    command: &str,
    args: &[&str],
    fallback_key: &str,
) -> Result<(), String> {
    run_process_success(command, args)
        .map_err(|error| normalize_dnsmasq_error(translator, &error, fallback_key))
}

pub(super) fn normalize_dnsmasq_error(
    translator: &Translator,
    message: &str,
    fallback_key: &str,
) -> String {
    let detail = message.trim();
    let lower = detail.to_ascii_lowercase();
    if lower.contains("address already in use")
        || lower.contains("failed to create listening socket")
        || lower.contains("failed to bind listening socket")
        || lower.contains("permission denied")
    {
        return if detail.is_empty() {
            dnsmasq_text(translator, "dnsPortUnavailable")
        } else {
            dnsmasq_text_params(
                translator,
                "dnsPortUnavailableWithDetail",
                &[("detail", detail.to_string())],
            )
        };
    }
    if detail.is_empty() {
        dnsmasq_text(translator, fallback_key)
    } else {
        detail.to_string()
    }
}
