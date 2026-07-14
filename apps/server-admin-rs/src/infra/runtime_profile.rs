use std::{fs, path::Path};

use serde::Serialize;

use crate::{i18n::Translator, state::AppState};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeProfile {
    pub deployment_target: String,
    pub is_docker: bool,
    pub is_linux: bool,
    pub is_windows: bool,
    pub is_root_process: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuntimeCapabilities {
    pub direct_mode_available: bool,
    pub host_firewall_available: bool,
    pub smart_connect_available: bool,
    pub fnos_certificate_sync_available: bool,
    pub system_clock_sync_available: bool,
    pub self_update_available: bool,
    pub terminal_available: bool,
    pub shared_root_available: bool,
    pub acme_available: bool,
    pub acme_resource_required: bool,
    pub cloudflared_available: bool,
    pub frpc_available: bool,
    pub ssh_security_available: bool,
    pub system_resource_monitor_available: bool,
    pub desktop_update_managed: bool,
}

pub fn get_runtime_profile(state: &AppState) -> RuntimeProfile {
    let deployment_target = detect_deployment_target(Some(&state.settings.runtime_target));
    let is_windows = std::env::consts::OS == "windows" || deployment_target == "windows";
    RuntimeProfile {
        is_docker: deployment_target == "docker",
        is_linux: std::env::consts::OS == "linux" && !is_windows,
        is_windows,
        is_root_process: is_root_process(),
        deployment_target,
    }
}

pub fn get_runtime_capabilities(profile: &RuntimeProfile) -> RuntimeCapabilities {
    if profile.is_windows || profile.deployment_target == "windows" {
        return RuntimeCapabilities {
            direct_mode_available: false,
            host_firewall_available: false,
            smart_connect_available: false,
            fnos_certificate_sync_available: false,
            system_clock_sync_available: false,
            self_update_available: false,
            terminal_available: false,
            shared_root_available: false,
            acme_available: true,
            acme_resource_required: false,
            cloudflared_available: false,
            frpc_available: false,
            ssh_security_available: false,
            system_resource_monitor_available: false,
            desktop_update_managed: true,
        };
    }
    let host_runtime_available = profile.deployment_target != "docker"
        && profile.deployment_target != "linux"
        && profile.deployment_target != "synology"
        && profile.is_linux
        && profile.is_root_process;

    RuntimeCapabilities {
        direct_mode_available: host_runtime_available,
        host_firewall_available: host_runtime_available,
        smart_connect_available: host_runtime_available,
        fnos_certificate_sync_available: profile.deployment_target == "fpk"
            && profile.is_linux
            && profile.is_root_process,
        system_clock_sync_available: host_runtime_available,
        self_update_available: profile.deployment_target == "fpk",
        terminal_available: profile.deployment_target != "docker"
            && profile.deployment_target != "openwrt"
            && profile.deployment_target != "synology",
        shared_root_available: has_shared_root(),
        acme_available: true,
        acme_resource_required: false,
        cloudflared_available: true,
        frpc_available: true,
        ssh_security_available: profile.deployment_target != "synology",
        system_resource_monitor_available: true,
        desktop_update_managed: false,
    }
}

pub fn admin_panel_protected_runtime(state: &AppState) -> bool {
    matches!(
        get_runtime_profile(state).deployment_target.as_str(),
        "docker" | "openwrt" | "linux" | "windows"
    )
}

pub fn deployment_target(state: &AppState) -> String {
    get_runtime_profile(state).deployment_target
}

pub fn host_runtime_available(state: &AppState) -> bool {
    let profile = get_runtime_profile(state);
    let capabilities = get_runtime_capabilities(&profile);
    capabilities.direct_mode_available
}

pub fn host_firewall_available(state: &AppState) -> bool {
    let profile = get_runtime_profile(state);
    let capabilities = get_runtime_capabilities(&profile);
    capabilities.host_firewall_available
}

pub fn terminal_available(state: &AppState) -> bool {
    let profile = get_runtime_profile(state);
    let capabilities = get_runtime_capabilities(&profile);
    capabilities.terminal_available
}

pub fn capability_unavailable_message(
    capability: &str,
    profile: &RuntimeProfile,
    translator: &Translator,
) -> String {
    let reason = match capability {
        "direct_mode_available"
        | "host_firewall_available"
        | "smart_connect_available"
        | "system_clock_sync_available" => {
            if profile.is_docker {
                "docker"
            } else if !profile.is_linux {
                "platform"
            } else {
                "permission"
            }
        }
        "fnos_certificate_sync_available" => {
            if profile.is_docker {
                "docker"
            } else if profile.deployment_target != "fpk" || !profile.is_linux {
                "platform"
            } else {
                "permission"
            }
        }
        "self_update_available" => {
            if profile.is_docker {
                "docker"
            } else if profile.deployment_target == "openwrt" {
                "openwrt"
            } else {
                "deployment"
            }
        }
        "terminal_available" => {
            if profile.is_docker {
                "docker"
            } else if profile.deployment_target == "openwrt" {
                "openwrt"
            } else {
                "platform"
            }
        }
        "shared_root_available" => "missing",
        _ => "default",
    };

    translator.t_with_fallback(
        &format!("server.runtimeProfile.capabilities.{capability}.{reason}"),
        &translator.t("server.runtimeProfile.capabilities.default"),
    )
}

pub(crate) fn detect_deployment_target(explicit: Option<&str>) -> String {
    if let Some(target) = explicit.and_then(normalize_deployment_target) {
        return target.to_string();
    }
    if detect_strong_fpk_environment() {
        return "fpk".to_string();
    }
    if Path::new("/.dockerenv").exists() || detect_docker_by_cgroup() {
        return "docker".to_string();
    }
    if detect_fpk_environment() {
        return "fpk".to_string();
    }
    if cfg!(target_os = "windows") {
        return "windows".to_string();
    }
    "dev".to_string()
}

fn detect_strong_fpk_environment() -> bool {
    env_present("TRIM_APPDEST")
        || env_present("TRIM_PKGVAR")
        || env_present("TRIM_SERVICE_PORT")
        || env_present("TRIM_DATA_SHARE_PATHS")
}

fn detect_fpk_environment() -> bool {
    detect_strong_fpk_environment()
        || env_present("FN_KNOCK_ROOT_SHARE_DIR")
        || env_present("FN_KNOCK_CERT_SHARE_DIR")
}

fn normalize_deployment_target(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "docker" => Some("docker"),
        "fpk" => Some("fpk"),
        "openwrt" => Some("openwrt"),
        "linux" => Some("linux"),
        "synology" | "dsm" => Some("synology"),
        "windows" => Some("windows"),
        "dev" | "development" => Some("dev"),
        _ => None,
    }
}

fn detect_docker_by_cgroup() -> bool {
    fs::read_to_string("/proc/1/cgroup")
        .map(|content| {
            let lower = content.to_ascii_lowercase();
            lower.contains("docker")
                || lower.contains("containerd")
                || lower.contains("kubepods")
                || lower.contains("podman")
        })
        .unwrap_or(false)
}

fn env_present(key: &str) -> bool {
    std::env::var(key)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn has_shared_root() -> bool {
    configured_share_directory().is_some_and(|path| path.exists())
}

pub fn configured_share_directory() -> Option<std::path::PathBuf> {
    ["FN_KNOCK_ROOT_SHARE_DIR", "FN_KNOCK_CERT_SHARE_DIR"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .next()
        .or_else(trim_data_share_paths)
}

pub fn configured_share_directory_with_legacy_env_precedence() -> Option<std::path::PathBuf> {
    std::env::var("FN_KNOCK_ROOT_SHARE_DIR")
        .or_else(|_| std::env::var("FN_KNOCK_CERT_SHARE_DIR"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(trim_data_share_paths)
}

fn trim_data_share_paths() -> Option<std::path::PathBuf> {
    let raw = std::env::var_os("TRIM_DATA_SHARE_PATHS")?;
    std::env::split_paths(&raw)
        .map(|value| value.to_string_lossy().trim().to_string())
        .filter(|value| !value.is_empty())
        .min_by_key(|value| value.len())
        .map(std::path::PathBuf::from)
}

fn is_root_process() -> bool {
    crate::unix::is_root_process()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::EnvGuard;

    fn profile(target: &str, linux: bool, root: bool) -> RuntimeProfile {
        RuntimeProfile {
            deployment_target: target.to_string(),
            is_docker: target == "docker",
            is_linux: linux,
            is_windows: target == "windows",
            is_root_process: root,
        }
    }

    #[test]
    fn local_runtime_target_does_not_mask_environment_detection() {
        assert_eq!(normalize_deployment_target("development"), Some("dev"));
        assert_eq!(normalize_deployment_target("dev"), Some("dev"));
        assert_eq!(normalize_deployment_target("local"), None);
        assert_eq!(normalize_deployment_target(""), None);
    }

    #[test]
    fn detects_fpk_from_shared_root_environment_when_target_is_missing() {
        let env = EnvGuard::new(&["FN_KNOCK_ROOT_SHARE_DIR"]);
        env.set("FN_KNOCK_ROOT_SHARE_DIR", "/vol1/@appdata/fn-knock");

        let is_fpk = detect_fpk_environment();

        assert!(is_fpk);
    }

    #[test]
    fn detects_strong_fpk_environment_before_container_markers() {
        let env = EnvGuard::new(&["TRIM_APPDEST"]);
        env.set("TRIM_APPDEST", "/trim/app/fn-knock");

        let target = detect_deployment_target(None);

        assert_eq!(target, "fpk");
    }

    #[test]
    fn fpk_root_linux_exposes_host_capabilities() {
        let capabilities = get_runtime_capabilities(&profile("fpk", true, true));
        assert!(capabilities.direct_mode_available);
        assert!(capabilities.host_firewall_available);
        assert!(capabilities.smart_connect_available);
        assert!(capabilities.fnos_certificate_sync_available);
        assert!(capabilities.system_clock_sync_available);
        assert!(capabilities.self_update_available);
        assert!(capabilities.terminal_available);
    }

    #[test]
    fn generic_linux_disables_host_mutation_capabilities_without_web_self_update() {
        let capabilities = get_runtime_capabilities(&profile("linux", true, true));
        assert!(!capabilities.direct_mode_available);
        assert!(!capabilities.host_firewall_available);
        assert!(!capabilities.smart_connect_available);
        assert!(!capabilities.fnos_certificate_sync_available);
        assert!(!capabilities.system_clock_sync_available);
        assert!(capabilities.terminal_available);
        assert!(!capabilities.self_update_available);
        assert_eq!(normalize_deployment_target("linux"), Some("linux"));
    }

    #[test]
    fn synology_runtime_disables_unsupported_host_capabilities() {
        let capabilities = get_runtime_capabilities(&profile("synology", true, true));
        assert!(!capabilities.direct_mode_available);
        assert!(!capabilities.host_firewall_available);
        assert!(!capabilities.smart_connect_available);
        assert!(!capabilities.fnos_certificate_sync_available);
        assert!(!capabilities.system_clock_sync_available);
        assert!(!capabilities.self_update_available);
        assert!(!capabilities.terminal_available);
        assert!(!capabilities.ssh_security_available);
        assert!(capabilities.cloudflared_available);
        assert!(capabilities.frpc_available);
        assert!(capabilities.system_resource_monitor_available);
        assert_eq!(normalize_deployment_target("DSM"), Some("synology"));
    }

    #[test]
    fn windows_uses_desktop_managed_capabilities_only() {
        let capabilities = get_runtime_capabilities(&profile("windows", false, false));
        assert!(!capabilities.direct_mode_available);
        assert!(!capabilities.host_firewall_available);
        assert!(!capabilities.smart_connect_available);
        assert!(!capabilities.fnos_certificate_sync_available);
        assert!(!capabilities.system_clock_sync_available);
        assert!(!capabilities.self_update_available);
        assert!(!capabilities.terminal_available);
        assert!(!capabilities.shared_root_available);
        assert!(capabilities.acme_available);
        assert!(!capabilities.acme_resource_required);
        assert!(!capabilities.cloudflared_available);
        assert!(!capabilities.frpc_available);
        assert!(!capabilities.ssh_security_available);
        assert!(!capabilities.system_resource_monitor_available);
        assert!(capabilities.desktop_update_managed);
        assert_eq!(normalize_deployment_target("WINDOWS"), Some("windows"));
    }

    #[test]
    fn shared_root_available_uses_trim_data_share_paths_when_root_env_is_empty() {
        let env = EnvGuard::new(&[
            "FN_KNOCK_ROOT_SHARE_DIR",
            "FN_KNOCK_CERT_SHARE_DIR",
            "TRIM_DATA_SHARE_PATHS",
        ]);
        let directory = tempfile::tempdir().unwrap();
        env.set("FN_KNOCK_ROOT_SHARE_DIR", "");
        env.remove("FN_KNOCK_CERT_SHARE_DIR");
        env.set("TRIM_DATA_SHARE_PATHS", directory.path().as_os_str());

        let capabilities = get_runtime_capabilities(&profile("fpk", true, true));

        assert!(capabilities.shared_root_available);
    }

    #[test]
    fn configured_share_directory_preserves_environment_priority_and_fallback() {
        let env = EnvGuard::new(&[
            "FN_KNOCK_ROOT_SHARE_DIR",
            "FN_KNOCK_CERT_SHARE_DIR",
            "TRIM_DATA_SHARE_PATHS",
        ]);
        env.set("FN_KNOCK_ROOT_SHARE_DIR", " /root-share ");
        env.set("FN_KNOCK_CERT_SHARE_DIR", " /cert-share ");
        let trim_paths = std::env::join_paths(["/very/long/share", "/short"]).unwrap();
        env.set("TRIM_DATA_SHARE_PATHS", &trim_paths);

        assert_eq!(
            configured_share_directory(),
            Some(std::path::PathBuf::from("/root-share"))
        );

        env.set("FN_KNOCK_ROOT_SHARE_DIR", " ");
        assert_eq!(
            configured_share_directory(),
            Some(std::path::PathBuf::from("/cert-share"))
        );

        env.set("FN_KNOCK_CERT_SHARE_DIR", "");
        assert_eq!(
            configured_share_directory(),
            Some(std::path::PathBuf::from("/short"))
        );
    }

    #[test]
    fn legacy_share_directory_preserves_empty_root_shadowing_cert_env() {
        let env = EnvGuard::new(&[
            "FN_KNOCK_ROOT_SHARE_DIR",
            "FN_KNOCK_CERT_SHARE_DIR",
            "TRIM_DATA_SHARE_PATHS",
        ]);
        env.set("FN_KNOCK_ROOT_SHARE_DIR", " ");
        env.set("FN_KNOCK_CERT_SHARE_DIR", " /cert-share ");
        let trim_paths = std::env::join_paths(["/very/long/share", "/short"]).unwrap();
        env.set("TRIM_DATA_SHARE_PATHS", &trim_paths);

        assert_eq!(
            configured_share_directory(),
            Some(std::path::PathBuf::from("/cert-share"))
        );
        assert_eq!(
            configured_share_directory_with_legacy_env_precedence(),
            Some(std::path::PathBuf::from("/short"))
        );
    }

    #[test]
    fn docker_blocks_host_and_terminal_capabilities() {
        let capabilities = get_runtime_capabilities(&profile("docker", true, true));
        assert!(!capabilities.direct_mode_available);
        assert!(!capabilities.host_firewall_available);
        assert!(!capabilities.smart_connect_available);
        assert!(!capabilities.fnos_certificate_sync_available);
        assert!(!capabilities.system_clock_sync_available);
        assert!(!capabilities.self_update_available);
        assert!(!capabilities.terminal_available);
    }

    #[test]
    fn openwrt_keeps_host_capabilities_but_blocks_terminal() {
        let capabilities = get_runtime_capabilities(&profile("openwrt", true, true));
        assert!(capabilities.direct_mode_available);
        assert!(capabilities.host_firewall_available);
        assert!(capabilities.smart_connect_available);
        assert!(!capabilities.fnos_certificate_sync_available);
        assert!(capabilities.system_clock_sync_available);
        assert!(!capabilities.self_update_available);
        assert!(!capabilities.terminal_available);
    }
}
