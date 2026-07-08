use std::fs;

use crate::{i18n::Translator, state::AppState};

pub(super) fn detect_system_timezone() -> Option<String> {
    if let Ok(value) = std::env::var("TZ")
        && !value.trim().is_empty()
    {
        return Some(value.trim().to_string());
    }
    if let Ok(value) = fs::read_to_string("/etc/timezone") {
        let timezone = value.trim();
        if !timezone.is_empty() {
            return Some(timezone.to_string());
        }
    }
    if let Ok(target) = fs::read_link("/etc/localtime")
        && let Some(text) = target.to_str()
        && let Some((_, zone)) = text.split_once("zoneinfo/")
        && !zone.trim().is_empty()
    {
        return Some(zone.trim().to_string());
    }
    None
}

pub(super) fn host_runtime_available(state: &AppState) -> bool {
    deployment_target(state) != "docker" && std::env::consts::OS == "linux" && is_running_as_root()
}

pub(super) fn system_clock_unavailable_message(
    state: &AppState,
    translator: &Translator,
) -> String {
    if deployment_target(state) == "docker" {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.docker")
    } else if std::env::consts::OS != "linux" {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.platform")
    } else {
        translator.t("server.runtimeProfile.capabilities.system_clock_sync_available.permission")
    }
}

pub(super) fn smart_connect_unavailable_message(
    state: &AppState,
    translator: &Translator,
) -> String {
    if deployment_target(state) == "docker" {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.docker")
    } else if std::env::consts::OS != "linux" {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.platform")
    } else {
        translator.t("server.runtimeProfile.capabilities.smart_connect_available.permission")
    }
}

pub(super) use crate::runtime_profile::deployment_target;

#[cfg(unix)]
pub(super) fn is_running_as_root() -> bool {
    crate::unix::is_root_process()
}
