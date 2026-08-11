use std::{collections::HashSet, net::IpAddr};

use crate::{crypto_utils, state::AppState};

use super::{
    BUILTIN_CANDIDATE_SOURCES, CloudflareApiError, MAX_CUSTOM_SOURCE_HOSTNAMES,
    OPTIMIZATION_DOMAIN_SETTINGS_KEY, OPTIMIZATION_SETTINGS_KEY, OptimizationDomainSettings,
    OptimizationSourceSettings, local_error, local_error_display,
};

pub(super) fn default_true() -> bool {
    true
}

pub(super) fn default_builtin_source_ids() -> Vec<String> {
    BUILTIN_CANDIDATE_SOURCES
        .iter()
        .map(|source| source.id.to_string())
        .collect()
}

pub(super) async fn load_source_settings(
    state: &AppState,
) -> Result<OptimizationSourceSettings, CloudflareApiError> {
    let stored = state
        .storage
        .store
        .get_json_value(OPTIMIZATION_SETTINGS_KEY)
        .await
        .map_err(local_error_display)?;
    let Some(value) = stored else {
        return Ok(OptimizationSourceSettings::default());
    };
    let settings = serde_json::from_value(value)
        .map_err(|error| local_error(format!("Invalid optimization source settings: {error}")))?;
    normalize_source_settings(settings).map_err(local_error)
}

pub(super) async fn load_domain_settings(
    state: &AppState,
) -> Result<OptimizationDomainSettings, CloudflareApiError> {
    let stored = state
        .storage
        .store
        .get_json_value(OPTIMIZATION_DOMAIN_SETTINGS_KEY)
        .await
        .map_err(local_error_display)?;
    let Some(value) = stored else {
        return Ok(OptimizationDomainSettings::default());
    };
    let settings = serde_json::from_value(value)
        .map_err(|error| local_error(format!("Invalid optimization domain settings: {error}")))?;
    normalize_domain_settings(settings).map_err(local_error)
}

pub(super) fn normalize_domain_settings(
    mut settings: OptimizationDomainSettings,
) -> Result<OptimizationDomainSettings, String> {
    let mut external = Vec::new();
    let mut seen = HashSet::new();
    for value in settings.external_hostnames {
        let hostname = normalize_candidate_hostname(&value)?;
        if seen.insert(hostname.clone()) {
            external.push(hostname);
        }
    }
    external.sort();
    settings.external_hostnames = external;
    Ok(settings)
}

pub(super) fn partition_optimization_hosts(
    hosts: Vec<String>,
    settings: &OptimizationDomainSettings,
) -> (Vec<String>, Vec<String>) {
    let external = settings
        .external_hostnames
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    hosts
        .into_iter()
        .partition(|hostname| !external.contains(hostname.as_str()))
}

pub(super) fn source_settings_fingerprint(settings: &OptimizationSourceSettings) -> String {
    let serialized = serde_json::to_string(settings).unwrap_or_default();
    crypto_utils::sha256_hex_str(&serialized)
}

pub(super) fn normalize_source_settings(
    mut settings: OptimizationSourceSettings,
) -> Result<OptimizationSourceSettings, String> {
    let available_ids = BUILTIN_CANDIDATE_SOURCES
        .iter()
        .map(|source| source.id)
        .collect::<HashSet<_>>();
    let mut seen_ids = HashSet::new();
    settings
        .builtin_ids
        .retain(|id| available_ids.contains(id.as_str()) && seen_ids.insert(id.clone()));

    let mut custom = Vec::new();
    let mut seen_hosts = HashSet::new();
    for value in settings.custom_hostnames {
        let hostname = normalize_candidate_hostname(&value)?;
        if seen_hosts.insert(hostname.clone()) {
            custom.push(hostname);
        }
    }
    if custom.len() > MAX_CUSTOM_SOURCE_HOSTNAMES {
        return Err(format!(
            "At most {MAX_CUSTOM_SOURCE_HOSTNAMES} custom candidate hostnames are allowed"
        ));
    }
    settings.custom_hostnames = custom;
    if !settings.official_ranges
        && settings.builtin_ids.is_empty()
        && settings.custom_hostnames.is_empty()
    {
        return Err(
            "Enable the official ranges or configure at least one candidate hostname".to_string(),
        );
    }
    Ok(settings)
}

pub(super) fn normalize_candidate_hostname(value: &str) -> Result<String, String> {
    let value = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 253
        || value.contains("://")
        || value.contains('/')
        || value.contains('*')
    {
        return Err(format!("Invalid candidate hostname: {value}"));
    }
    if value.parse::<IpAddr>().is_ok() {
        return Err(format!(
            "Candidate source must be a hostname, not an IP address: {value}"
        ));
    }
    let ascii = idna::domain_to_ascii(&value)
        .map_err(|_| format!("Invalid candidate hostname: {value}"))?
        .to_ascii_lowercase();
    if ascii.len() > 253
        || !ascii.contains('.')
        || ascii.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || label.starts_with('-')
                || label.ends_with('-')
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
    {
        return Err(format!("Invalid candidate hostname: {value}"));
    }
    Ok(ascii)
}
