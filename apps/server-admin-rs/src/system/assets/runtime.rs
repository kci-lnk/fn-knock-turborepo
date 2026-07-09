use std::fs;

use crate::{i18n::Translator, runtime_profile, state::AppState};

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

pub(super) fn system_clock_sync_available(state: &AppState) -> bool {
    let profile = runtime_profile::get_runtime_profile(state);
    system_clock_sync_available_for_profile(&profile)
}

pub(super) fn system_clock_sync_available_for_profile(
    profile: &runtime_profile::RuntimeProfile,
) -> bool {
    runtime_profile::get_runtime_capabilities(profile).system_clock_sync_available
}

pub(super) fn smart_connect_available(state: &AppState) -> bool {
    let profile = runtime_profile::get_runtime_profile(state);
    smart_connect_available_for_profile(&profile)
}

pub(super) fn smart_connect_available_for_profile(
    profile: &runtime_profile::RuntimeProfile,
) -> bool {
    runtime_profile::get_runtime_capabilities(profile).smart_connect_available
}

pub(super) fn system_clock_unavailable_message(
    state: &AppState,
    translator: &Translator,
) -> String {
    let profile = runtime_profile::get_runtime_profile(state);
    runtime_profile::capability_unavailable_message(
        "system_clock_sync_available",
        &profile,
        translator,
    )
}

pub(super) fn smart_connect_unavailable_message(
    state: &AppState,
    translator: &Translator,
) -> String {
    let profile = runtime_profile::get_runtime_profile(state);
    runtime_profile::capability_unavailable_message("smart_connect_available", &profile, translator)
}
