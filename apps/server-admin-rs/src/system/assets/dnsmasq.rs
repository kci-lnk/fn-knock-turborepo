use std::{fs, path::Path, process::Command, sync::Mutex};

use serde_json::{Value, json};

use crate::{i18n::Translator, time_utils};

use super::{
    DNSMASQ_INSTALL, DnsmasqInstallState, SMART_CONNECT_LOCAL_TTL_SECONDS,
    SMART_CONNECT_MANAGED_CONF_PATH,
    process::run_process_success,
    text::{dnsmasq_text, dnsmasq_text_params},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DnsmasqServiceKind {
    Systemd,
    SysV,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct DnsmasqServiceCommand {
    pub(super) program: &'static str,
    pub(super) args: &'static [&'static str],
    pub(super) failure_key: &'static str,
    pub(super) continue_after_failure: bool,
}

const SYSTEMD_ACTIVATE_COMMANDS: &[DnsmasqServiceCommand] = &[
    DnsmasqServiceCommand {
        program: "systemctl",
        args: &["enable", "dnsmasq"],
        failure_key: "enableServiceFailed",
        continue_after_failure: false,
    },
    DnsmasqServiceCommand {
        program: "systemctl",
        args: &["restart", "dnsmasq"],
        failure_key: "restartFailed",
        continue_after_failure: false,
    },
];
const SYSTEMD_DEACTIVATE_COMMANDS: &[DnsmasqServiceCommand] = &[DnsmasqServiceCommand {
    program: "systemctl",
    args: &["disable", "--now", "dnsmasq"],
    failure_key: "disableServiceFailed",
    continue_after_failure: false,
}];
const SYSV_ACTIVATE_COMMANDS: &[DnsmasqServiceCommand] = &[
    DnsmasqServiceCommand {
        program: "update-rc.d",
        args: &["dnsmasq", "defaults"],
        failure_key: "enableServiceFailed",
        continue_after_failure: false,
    },
    DnsmasqServiceCommand {
        program: "service",
        args: &["dnsmasq", "restart"],
        failure_key: "restartFailed",
        continue_after_failure: false,
    },
];
const SYSV_DEACTIVATE_COMMANDS: &[DnsmasqServiceCommand] = &[
    DnsmasqServiceCommand {
        program: "service",
        args: &["dnsmasq", "stop"],
        failure_key: "stopServiceFailed",
        continue_after_failure: true,
    },
    DnsmasqServiceCommand {
        program: "update-rc.d",
        args: &["-f", "dnsmasq", "remove"],
        failure_key: "disableServiceFailed",
        continue_after_failure: false,
    },
];

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
    match dnsmasq_service_kind() {
        Some(DnsmasqServiceKind::Systemd) => {
            run_process_success("systemctl", &["is-active", "--quiet", "dnsmasq"]).is_ok()
        }
        Some(DnsmasqServiceKind::SysV) => {
            run_process_success("service", &["dnsmasq", "status"]).is_ok()
        }
        None => false,
    }
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
    activate_dnsmasq_service(translator)?;

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

pub(super) fn dnsmasq_service_kind() -> Option<DnsmasqServiceKind> {
    dnsmasq_service_kind_for(
        Path::new("/run/systemd/system").is_dir(),
        has_systemd_unit(),
        has_init_script(),
    )
}

pub(super) fn dnsmasq_service_kind_for(
    systemd_running: bool,
    has_systemd_unit: bool,
    has_init_script: bool,
) -> Option<DnsmasqServiceKind> {
    if systemd_running && has_systemd_unit {
        Some(DnsmasqServiceKind::Systemd)
    } else if has_init_script {
        Some(DnsmasqServiceKind::SysV)
    } else {
        None
    }
}

pub(super) fn dnsmasq_service_commands(
    kind: DnsmasqServiceKind,
    activate: bool,
) -> &'static [DnsmasqServiceCommand] {
    match (kind, activate) {
        (DnsmasqServiceKind::Systemd, true) => SYSTEMD_ACTIVATE_COMMANDS,
        (DnsmasqServiceKind::Systemd, false) => SYSTEMD_DEACTIVATE_COMMANDS,
        (DnsmasqServiceKind::SysV, true) => SYSV_ACTIVATE_COMMANDS,
        (DnsmasqServiceKind::SysV, false) => SYSV_DEACTIVATE_COMMANDS,
    }
}

pub(super) fn run_dnsmasq_service_commands_with<F>(
    commands: &[DnsmasqServiceCommand],
    mut run: F,
) -> Result<(), String>
where
    F: FnMut(&DnsmasqServiceCommand) -> Result<(), String>,
{
    let mut errors = Vec::new();
    for command in commands {
        if let Err(error) = run(command) {
            errors.push(error);
            if !command.continue_after_failure {
                break;
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join(" | "))
    }
}

fn run_dnsmasq_service_commands(translator: &Translator, activate: bool) -> Result<(), String> {
    let Some(kind) = dnsmasq_service_kind() else {
        return if activate {
            Err(dnsmasq_text(translator, "serviceDefinitionMissing"))
        } else {
            Ok(())
        };
    };
    run_dnsmasq_service_commands_with(dnsmasq_service_commands(kind, activate), |command| {
        run_dnsmasq_process_success(
            translator,
            command.program,
            command.args,
            command.failure_key,
        )
    })
}

pub(crate) fn activate_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    run_dnsmasq_service_commands(translator, true)
}

pub(crate) fn deactivate_dnsmasq_service(translator: &Translator) -> Result<(), String> {
    run_dnsmasq_service_commands(translator, false)
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
        .unwrap_or_else(|error| error.into_inner())
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
        .unwrap_or_else(|error| error.into_inner());
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
    let service_lifecycle_error = matches!(
        fallback_key,
        "enableServiceFailed" | "stopServiceFailed" | "disableServiceFailed"
    );
    if !service_lifecycle_error
        && (lower.contains("address already in use")
            || lower.contains("failed to create listening socket")
            || lower.contains("failed to bind listening socket"))
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
    } else if service_lifecycle_error {
        format!("{}: {detail}", dnsmasq_text(translator, fallback_key))
    } else {
        detail.to_string()
    }
}
