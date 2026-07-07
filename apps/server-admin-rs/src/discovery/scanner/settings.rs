use super::*;

pub(super) async fn load_scanner_settings(
    state: &AppState,
) -> Result<ScannerSettings, ScannerError> {
    let raw = state.redis.scanner_settings_raw().await?;
    Ok(scanner_settings_from_raw(
        raw.as_ref(),
        scanner_env_defaults(),
    ))
}

pub(super) async fn save_scanner_settings(
    state: &AppState,
    body: UpdateScannerSettingsBody,
) -> Result<ScannerSettings, ScannerError> {
    let previous = load_scanner_settings(state).await?;
    let manual_cidrs = match body.cidr_exemptions {
        Some(cidrs) => validate_scanner_cidr_exemptions(cidrs)?,
        None => previous.cidr_exemptions.clone(),
    };
    let requested_regions = body
        .cidr_exemption_regions
        .map(dedupe_scanner_cidr_exemption_region_inputs);
    let previous_region_inputs = previous
        .cidr_exemption_regions
        .iter()
        .map(|item| ScannerCidrExemptionRegionInput {
            province: item.province.clone(),
            query_city: item.query_city.clone(),
        })
        .collect::<Vec<_>>();
    let reuse_region_resolution = requested_regions
        .as_ref()
        .is_none_or(|regions| scanner_cidr_region_keys_equal(regions, &previous_region_inputs));

    let (regions, region_cidrs) = if reuse_region_resolution {
        (
            previous.cidr_exemption_regions.clone(),
            previous.cidr_exemption_region_cidrs.clone(),
        )
    } else {
        let resolved =
            resolve_cidr_exemption_regions(state, requested_regions.as_deref().unwrap_or(&[]))
                .await?;
        (
            resolved
                .iter()
                .map(|item| item.selection.clone())
                .collect::<Vec<_>>(),
            normalize_scanner_cidr_exemptions_from_strings(
                resolved
                    .iter()
                    .flat_map(|item| item.cidrs.iter().cloned())
                    .collect::<Vec<_>>(),
            ),
        )
    };

    let effective_cidr_exemptions = normalize_scanner_cidr_exemptions_from_strings(
        region_cidrs
            .iter()
            .chain(manual_cidrs.iter())
            .cloned()
            .collect::<Vec<_>>(),
    );
    let stored = json!({
        "enabled": body.enabled,
        "windowMinutes": floor_to_i64(body.window_minutes).max(1),
        "threshold": floor_to_i64(body.threshold).max(1),
        "blacklistTtlSeconds": floor_to_i64(body.blacklist_ttl_seconds).max(60),
        "commonLocationExemptEnabled": body.common_location_exempt_enabled == Some(true),
        "cidrExemptions": manual_cidrs,
        "cidrExemptionRegions": regions,
        "cidrExemptionRegionCidrs": region_cidrs,
        "cidrExemptionCidrs": effective_cidr_exemptions,
    });
    state.redis.save_scanner_settings(&stored).await?;
    load_scanner_settings(state).await
}

pub(super) fn scanner_settings_from_raw(
    raw: Option<&Value>,
    defaults: ScannerEnvDefaults,
) -> ScannerSettings {
    let mut enabled = defaults.enabled;
    let mut window_minutes = defaults.window_minutes;
    let mut threshold = defaults.threshold;
    let mut blacklist_ttl_seconds = defaults.blacklist_ttl_seconds;
    let mut common_location_exempt_enabled = false;
    let mut cidr_exemptions = Vec::new();
    let mut cidr_exemption_regions = Vec::new();
    let mut cidr_exemption_region_cidrs = Vec::new();
    let mut cidr_exemption_cidrs = Vec::new();

    if let Some(raw) = raw {
        if let Some(value) = raw.get("enabled").and_then(Value::as_bool) {
            enabled = value;
        }
        if let Some(value) = positive_i64(raw.get("windowMinutes")) {
            window_minutes = value;
        }
        if let Some(value) = positive_i64(raw.get("threshold")) {
            threshold = value;
        }
        if let Some(value) = positive_i64(raw.get("blacklistTtlSeconds")) {
            blacklist_ttl_seconds = value;
        }
        if let Some(value) = raw
            .get("commonLocationExemptEnabled")
            .and_then(Value::as_bool)
        {
            common_location_exempt_enabled = value;
        }
        cidr_exemptions = normalize_scanner_cidr_exemptions(raw.get("cidrExemptions"));
        cidr_exemption_regions =
            normalize_scanner_cidr_exemption_regions(raw.get("cidrExemptionRegions"));
        cidr_exemption_region_cidrs =
            normalize_scanner_cidr_exemptions(raw.get("cidrExemptionRegionCidrs"));
        cidr_exemption_cidrs = normalize_scanner_cidr_exemptions(raw.get("cidrExemptionCidrs"));
    }

    let effective_cidr_exemptions = if cidr_exemption_cidrs.is_empty() {
        normalize_scanner_cidr_exemptions_from_strings(
            cidr_exemption_region_cidrs
                .iter()
                .chain(cidr_exemptions.iter())
                .cloned()
                .collect::<Vec<_>>(),
        )
    } else {
        cidr_exemption_cidrs.clone()
    };

    ScannerSettings {
        enabled,
        window_minutes,
        threshold,
        window_seconds: SCANNER_BASE_WINDOW_SECONDS.max(window_minutes * 60),
        blacklist_ttl_seconds,
        common_location_exempt_enabled,
        cidr_exemptions,
        cidr_exemption_regions,
        cidr_exemption_region_cidrs,
        cidr_exemption_cidrs: effective_cidr_exemptions,
    }
}

pub(super) fn scanner_env_defaults() -> ScannerEnvDefaults {
    let enabled_raw = env::var("SCANNER_ENABLED")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    ScannerEnvDefaults {
        enabled: enabled_raw == "true" || enabled_raw == "1",
        window_minutes: env_i64("SCANNER_WINDOW_MINUTES", 5),
        threshold: env_i64("SCANNER_THRESHOLD", 5),
        blacklist_ttl_seconds: env_i64("SCANNER_BLACKLIST_TTL_DAYS", 90) * 24 * 3600,
    }
}
