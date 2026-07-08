use super::*;

pub(super) fn ensure_go_success(
    value: Value,
    translator: &Translator,
    fallback_key: &str,
) -> anyhow::Result<()> {
    if crate::go_backend::response_success(&value) {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        crate::go_backend::response_message(&value, &ssh_security_text(translator, fallback_key),)
    )
}

pub(super) fn ssh_security_availability(
    state: &AppState,
    translator: &Translator,
) -> SshAvailability {
    let log_source = detect_log_source();
    let target = runtime_profile::deployment_target(state);
    if target == "openwrt" {
        return SshAvailability {
            available: false,
            reason: ssh_security_text(translator, "openWrtUnsupported"),
            log_source,
        };
    }
    if !host_firewall_available(state) {
        let profile = runtime_profile::get_runtime_profile(state);
        return SshAvailability {
            available: false,
            reason: runtime_profile::capability_unavailable_message(
                "host_firewall_available",
                &profile,
                translator,
            ),
            log_source,
        };
    }
    if log_source == "unavailable" {
        return SshAvailability {
            available: false,
            reason: ssh_security_text(translator, "logSourceUnavailable"),
            log_source,
        };
    }
    SshAvailability {
        available: true,
        reason: String::new(),
        log_source,
    }
}

pub(super) use crate::runtime_profile::host_firewall_available;
