use super::*;

const REGION_POLICY_FIELD: &str = "cidrExemptionRegionPolicy";
const EFFECTIVE_POLICY_FIELD: &str = "cidrExemptionPolicy";

pub(crate) async fn migrate_scanner_cidr_ipset_on_boot(
    state: &AppState,
) -> Result<(), ScannerError> {
    let _guard = state.security.scanner_settings_update_lock.lock().await;
    let Some(raw) = state.storage.store.scanner_settings_raw().await? else {
        state
            .security
            .ipsets
            .publish(SCANNER_EXEMPT_IPSET_KEY, None);
        return Ok(());
    };
    let (stored, policy) = compact_scanner_settings(&raw)?;
    if stored != raw {
        state.storage.store.save_scanner_settings(&stored).await?;
    }
    state
        .security
        .ipsets
        .publish(SCANNER_EXEMPT_IPSET_KEY, non_empty_policy(policy));
    Ok(())
}

pub(super) async fn load_scanner_settings(
    state: &AppState,
) -> Result<ScannerSettings, ScannerError> {
    let raw = state.storage.store.scanner_settings_raw().await?;
    ensure_effective_policy_loaded(state, raw.as_ref())?;
    Ok(scanner_settings_from_raw(
        raw.as_ref(),
        scanner_env_defaults(),
    ))
}

pub(super) async fn load_scanner_preflight_inputs(
    state: &AppState,
) -> Result<(ScannerSettings, HashSet<String>), ScannerError> {
    let raw = state.storage.store.scanner_settings_raw().await?;
    ensure_effective_policy_loaded(state, raw.as_ref())?;
    Ok((
        scanner_settings_from_raw(raw.as_ref(), scanner_env_defaults()),
        scanner_path_whitelist_from_raw(raw.as_ref())?
            .into_iter()
            .collect(),
    ))
}

pub(super) async fn save_scanner_settings(
    state: &AppState,
    body: UpdateScannerSettingsBody,
) -> Result<ScannerSettings, ScannerError> {
    let _guard = state.security.scanner_settings_update_lock.lock().await;
    let previous_raw = state.storage.store.scanner_settings_raw().await?;
    let previous = scanner_settings_from_raw(previous_raw.as_ref(), scanner_env_defaults());
    let manual_cidrs = match body.cidr_exemptions.as_ref() {
        Some(cidrs) => validate_scanner_cidr_exemptions(cidrs.clone())?,
        None => previous.cidr_exemptions.clone(),
    };
    let requested_regions = body
        .cidr_exemption_regions
        .as_ref()
        .cloned()
        .map(dedupe_scanner_cidr_exemption_region_inputs)
        .transpose()?;
    let previous_region_inputs = previous
        .cidr_exemption_regions
        .iter()
        .map(|item| {
            CidrRegionQuery::new(
                item.province.clone(),
                item.query_city.clone(),
                item.operator,
            )
        })
        .collect::<Vec<_>>();
    let reuse_region_resolution = requested_regions
        .as_ref()
        .is_none_or(|regions| scanner_cidr_region_keys_equal(regions, &previous_region_inputs));

    let (regions, region_policy) = if reuse_region_resolution {
        (
            previous.cidr_exemption_regions.clone(),
            region_policy_from_raw(previous_raw.as_ref())?,
        )
    } else {
        let resolved =
            crate::cidr::lookup_regions(state, requested_regions.as_deref().unwrap_or(&[])).await?;
        let regions = resolved
            .iter()
            .map(|item| item.selection.clone())
            .collect::<Vec<_>>();
        let policy = crate::cidr::union_ip_sets(resolved.iter().map(|item| &item.policy));
        (regions, policy)
    };

    let manual_policy = crate::cidr::compile_ip_set(&manual_cidrs).map_err(ScannerError::Cidr)?;
    let effective_policy = crate::cidr::union_ip_sets([&region_policy, &manual_policy]);
    let stored = compact_scanner_settings_value(
        previous_raw.as_ref(),
        &body,
        manual_cidrs,
        regions,
        &region_policy,
        &effective_policy,
    );
    state.storage.store.save_scanner_settings(&stored).await?;
    state
        .security
        .ipsets
        .publish(SCANNER_EXEMPT_IPSET_KEY, non_empty_policy(effective_policy));
    Ok(scanner_settings_from_raw(
        Some(&stored),
        scanner_env_defaults(),
    ))
}

pub(super) fn compact_scanner_settings(
    raw: &Value,
) -> Result<(Value, crate::cidr::CompiledIpSet), ScannerError> {
    let settings = scanner_settings_from_raw(Some(raw), scanner_env_defaults());
    let region_policy = region_policy_from_raw(Some(raw))?;
    let effective_policy = effective_policy_from_raw(Some(raw), &region_policy, &settings)?;
    let mut stored = raw.clone();
    let object = stored
        .as_object_mut()
        .ok_or_else(|| ScannerError::Cidr("scanner settings must be a JSON object".to_string()))?;
    object.remove("cidrExemptionRegionCidrs");
    object.remove("cidrExemptionCidrs");
    write_policy(object, REGION_POLICY_FIELD, &region_policy);
    write_distinct_effective_policy(object, &region_policy, &effective_policy);
    object.insert(
        "cidrExemptionPolicyId".to_string(),
        Value::String(effective_policy.id.clone()),
    );
    object.insert(
        "cidrExemptionSourceCidrCount".to_string(),
        json!(effective_policy.source_cidr_count),
    );
    object.insert(
        "cidrExemptionRangeCount".to_string(),
        json!(effective_policy.range_count()),
    );
    Ok((stored, effective_policy))
}

fn compact_scanner_settings_value(
    previous: Option<&Value>,
    body: &UpdateScannerSettingsBody,
    manual_cidrs: Vec<String>,
    regions: Vec<ScannerCidrExemptionSelection>,
    region_policy: &crate::cidr::CompiledIpSet,
    effective_policy: &crate::cidr::CompiledIpSet,
) -> Value {
    let mut stored = previous
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    stored.insert("enabled".to_string(), Value::Bool(body.enabled));
    stored.insert(
        "windowMinutes".to_string(),
        json!(floor_to_i64(body.window_minutes).max(1)),
    );
    stored.insert(
        "threshold".to_string(),
        json!(floor_to_i64(body.threshold).max(1)),
    );
    stored.insert(
        "blacklistTtlSeconds".to_string(),
        json!(floor_to_i64(body.blacklist_ttl_seconds).max(60)),
    );
    stored.insert(
        "commonLocationExemptEnabled".to_string(),
        Value::Bool(body.common_location_exempt_enabled == Some(true)),
    );
    stored.insert("cidrExemptions".to_string(), json!(manual_cidrs));
    stored.insert("cidrExemptionRegions".to_string(), json!(regions));
    stored.remove("cidrExemptionRegionCidrs");
    stored.remove("cidrExemptionCidrs");
    write_policy(&mut stored, REGION_POLICY_FIELD, region_policy);
    write_distinct_effective_policy(&mut stored, region_policy, effective_policy);
    stored.insert(
        "cidrExemptionPolicyId".to_string(),
        Value::String(effective_policy.id.clone()),
    );
    stored.insert(
        "cidrExemptionSourceCidrCount".to_string(),
        json!(effective_policy.source_cidr_count),
    );
    stored.insert(
        "cidrExemptionRangeCount".to_string(),
        json!(effective_policy.range_count()),
    );
    Value::Object(stored)
}

fn write_policy(object: &mut Map<String, Value>, field: &str, policy: &crate::cidr::CompiledIpSet) {
    if policy.range_count() == 0 {
        object.remove(field);
    } else {
        object.insert(field.to_string(), policy.to_transport_value());
    }
}

fn write_distinct_effective_policy(
    object: &mut Map<String, Value>,
    region_policy: &crate::cidr::CompiledIpSet,
    effective_policy: &crate::cidr::CompiledIpSet,
) {
    if effective_policy.id == region_policy.id {
        object.remove(EFFECTIVE_POLICY_FIELD);
    } else {
        write_policy(object, EFFECTIVE_POLICY_FIELD, effective_policy);
    }
}

fn region_policy_from_raw(raw: Option<&Value>) -> Result<crate::cidr::CompiledIpSet, ScannerError> {
    if let Some(policy) = decode_embedded_policy(raw, REGION_POLICY_FIELD)? {
        return Ok(policy.into_current_format());
    }
    let cidrs = normalize_scanner_cidr_exemptions(
        raw.and_then(|value| value.get("cidrExemptionRegionCidrs")),
    );
    crate::cidr::compile_ip_set(cidrs).map_err(ScannerError::Cidr)
}

fn effective_policy_from_raw(
    raw: Option<&Value>,
    region_policy: &crate::cidr::CompiledIpSet,
    settings: &ScannerSettings,
) -> Result<crate::cidr::CompiledIpSet, ScannerError> {
    if let Some(policy) = decode_embedded_policy(raw, EFFECTIVE_POLICY_FIELD)? {
        return Ok(policy.into_current_format());
    }
    let legacy_effective =
        normalize_scanner_cidr_exemptions(raw.and_then(|value| value.get("cidrExemptionCidrs")));
    if !legacy_effective.is_empty() {
        return crate::cidr::compile_ip_set(legacy_effective).map_err(ScannerError::Cidr);
    }
    let manual_policy =
        crate::cidr::compile_ip_set(&settings.cidr_exemptions).map_err(ScannerError::Cidr)?;
    Ok(crate::cidr::union_ip_sets([region_policy, &manual_policy]))
}

fn decode_embedded_policy(
    raw: Option<&Value>,
    field: &str,
) -> Result<Option<crate::cidr::CompiledIpSet>, ScannerError> {
    raw.and_then(|value| value.get(field))
        .map(crate::cidr::CompiledIpSet::from_transport_value)
        .transpose()
        .map_err(|error| ScannerError::Cidr(format!("invalid scanner {field}: {error}")))
}

fn ensure_effective_policy_loaded(
    state: &AppState,
    raw: Option<&Value>,
) -> Result<(), ScannerError> {
    let settings = scanner_settings_from_raw(raw, scanner_env_defaults());
    if let Some(expected_id) = settings.cidr_exemption_policy_id.as_deref()
        && state
            .security
            .ipsets
            .get(SCANNER_EXEMPT_IPSET_KEY)
            .is_some_and(|policy| policy.id == expected_id)
    {
        return Ok(());
    }
    let region_policy = region_policy_from_raw(raw)?;
    let effective_policy = effective_policy_from_raw(raw, &region_policy, &settings)?;
    state
        .security
        .ipsets
        .publish(SCANNER_EXEMPT_IPSET_KEY, non_empty_policy(effective_policy));
    Ok(())
}

fn non_empty_policy(policy: crate::cidr::CompiledIpSet) -> Option<crate::cidr::CompiledIpSet> {
    (policy.range_count() > 0).then_some(policy)
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
    let mut cidr_exemption_policy_id = None;
    let mut cidr_exemption_source_cidr_count = 0usize;
    let mut cidr_exemption_range_count = 0usize;

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
        cidr_exemption_policy_id = raw
            .get("cidrExemptionPolicyId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);
        cidr_exemption_source_cidr_count = raw
            .get("cidrExemptionSourceCidrCount")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_default();
        cidr_exemption_range_count = raw
            .get("cidrExemptionRangeCount")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or_default();
    }

    ScannerSettings {
        enabled,
        window_minutes,
        threshold,
        window_seconds: SCANNER_BASE_WINDOW_SECONDS.max(window_minutes * 60),
        blacklist_ttl_seconds,
        common_location_exempt_enabled,
        cidr_exemptions,
        cidr_exemption_regions,
        cidr_exemption_region_cidrs: Vec::new(),
        cidr_exemption_cidrs: Vec::new(),
        cidr_exemption_policy_id,
        cidr_exemption_source_cidr_count,
        cidr_exemption_range_count,
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
