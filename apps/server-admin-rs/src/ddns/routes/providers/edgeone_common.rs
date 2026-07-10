use super::*;

const EDGEONE_OVERSEAS_ACCESS_STATE_KEY_PREFIX: &str = "fn_knock:ddns:edgeone:overseas_access:";
const EDGEONE_OVERSEAS_ACCESS_RULE_NAME_PREFIX: &str = "fn_knock_block_overseas_";
const EDGEONE_OVERSEAS_ACCESS_LEGACY_RULE_NAME: &str = "fn_knock_block_overseas";
const EDGEONE_OVERSEAS_ACCESS_SYNC_VERSION: &str = "edgeone-overseas-console-v1";
const EDGEONE_ALLOWED_MAINLAND_REGION_CODES: &[&str] = &[
    "CN-Other", "CN-SH", "CN-YN", "CN-NM", "CN-BJ", "CN-JL", "CN-SC", "CN-TJ", "CN-NX", "CN-AH",
    "CN-SD", "CN-GD", "CN-GX", "CN-XJ", "CN-JS", "CN-JX", "CN-HE", "CN-HA", "CN-ZJ", "CN-HI",
    "CN-HB", "CN-HN", "CN-GS", "CN-FJ", "CN-XZ", "CN-GZ", "CN-LN", "CN-CQ", "CN-SN", "CN-QH",
    "CN-HL", "CN-SX", "CN-MO", "CN-TW", "CN-HK",
];

#[derive(Clone)]
pub(in crate::ddns::routes) struct EdgeOneOverseasAccessSyncResult {
    pub(in crate::ddns::routes) changed: bool,
    pub(in crate::ddns::routes) message: Option<String>,
}

#[derive(Clone)]
struct EdgeOneDomainTarget {
    domain: String,
    endpoint_host: String,
    region: Option<String>,
    zone_id: String,
}

pub(in crate::ddns::routes) async fn ensure_edgeone_overseas_access_synced(
    state: &AppState,
    translator: &Translator,
    provider_name: &str,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
) -> anyhow::Result<EdgeOneOverseasAccessSyncResult> {
    if !is_edgeone_provider(provider_name) {
        return Ok(EdgeOneOverseasAccessSyncResult {
            changed: false,
            message: None,
        });
    }

    let desired_mode = normalize_edgeone_overseas_access_mode(
        config
            .get(DDNS_EDGEONE_OVERSEAS_ACCESS_FIELD)
            .map(String::as_str),
    );
    let previous_state = read_edgeone_overseas_access_state(state, provider_name).await?;
    if desired_mode == "off"
        && previous_state
            .as_ref()
            .and_then(|state| state.get("mode").and_then(Value::as_str))
            != Some("block_overseas")
    {
        return Ok(EdgeOneOverseasAccessSyncResult {
            changed: false,
            message: None,
        });
    }

    let target = edgeone_domain_target(translator, config)?;
    let config_signature = edgeone_overseas_access_config_signature(provider_name, &target);
    if previous_state
        .as_ref()
        .and_then(|state| state.get("mode").and_then(Value::as_str))
        == Some(desired_mode)
        && previous_state
            .as_ref()
            .and_then(|state| state.get("configSignature").and_then(Value::as_str))
            == Some(config_signature.as_str())
    {
        return Ok(EdgeOneOverseasAccessSyncResult {
            changed: false,
            message: None,
        });
    }

    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    if secret_id.is_empty() || secret_key.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(translator, "providers.edgeone.configIncomplete", &[])
        );
    }

    let client = ddns_http_client(translator, http_options)?;
    let mut changed = false;
    let mut success_count = 0;
    let mut attempt_errors = Vec::new();
    for scope in ["zone_level_domain", "zone_default_policy"] {
        match sync_edgeone_overseas_access_scope(
            translator,
            &client,
            config,
            provider_name,
            &secret_id,
            &secret_key,
            &target,
            desired_mode,
            scope,
        )
        .await
        {
            Ok(scope_changed) => {
                changed |= scope_changed;
                success_count += 1;
                if desired_mode == "block_overseas" {
                    break;
                }
            }
            Err(error) => attempt_errors.push(error.to_string()),
        }
    }

    if success_count == 0 && !attempt_errors.is_empty() {
        let mut lines = vec![ddns_text(
            translator,
            if desired_mode == "block_overseas" {
                "providers.edgeone.overseasAccess.syncAllScopesFailed"
            } else {
                "providers.edgeone.overseasAccess.cleanupAllScopesFailed"
            },
            &[],
        )];
        lines.extend(
            attempt_errors
                .into_iter()
                .enumerate()
                .map(|(index, message)| format!("{}. {message}", index + 1)),
        );
        anyhow::bail!("{}", lines.join("\n"));
    }

    write_edgeone_overseas_access_state(
        state,
        provider_name,
        json!({
            "appliedAt": time_utils::now_iso(),
            "configSignature": config_signature,
            "mode": desired_mode,
        }),
    )
    .await?;

    Ok(EdgeOneOverseasAccessSyncResult {
        changed,
        message: Some(ddns_text(
            translator,
            if desired_mode == "block_overseas" {
                "providers.edgeone.overseasAccess.syncSuccess"
            } else {
                "providers.edgeone.overseasAccess.cleanupSuccess"
            },
            &[],
        )),
    })
}

fn normalize_edgeone_overseas_access_mode(value: Option<&str>) -> &'static str {
    if value == Some("block_overseas") {
        "block_overseas"
    } else {
        "off"
    }
}

async fn read_edgeone_overseas_access_state(
    state: &AppState,
    provider_name: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    Ok(state
        .store
        .get_json_value(&edgeone_overseas_access_state_key(provider_name))
        .await?
        .filter(|value| {
            matches!(
                value.get("mode").and_then(Value::as_str),
                Some("off" | "block_overseas")
            ) && value
                .get("configSignature")
                .and_then(Value::as_str)
                .is_some()
                && value.get("appliedAt").and_then(Value::as_str).is_some()
        }))
}

async fn write_edgeone_overseas_access_state(
    state: &AppState,
    provider_name: &str,
    value: Value,
) -> crate::storage::StorageResult<()> {
    state
        .store
        .set_json_value(&edgeone_overseas_access_state_key(provider_name), &value)
        .await
}

fn edgeone_overseas_access_state_key(provider_name: &str) -> String {
    format!("{EDGEONE_OVERSEAS_ACCESS_STATE_KEY_PREFIX}{provider_name}")
}

fn edgeone_domain_target(
    translator: &Translator,
    config: &HashMap<String, String>,
) -> anyhow::Result<EdgeOneDomainTarget> {
    let zone_id = config_value(config, "zone_id");
    let raw_domain = config_value(config, "domain");
    if zone_id.is_empty() || raw_domain.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(translator, "providers.edgeone.configTargetIncomplete", &[])
        );
    }
    let targets = parse_ddns_domain_targets(&raw_domain)?;
    let domain = targets
        .pair_root()
        .map(str::to_string)
        .unwrap_or_else(|| targets.canonical());
    let region = config
        .get("region")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    Ok(EdgeOneDomainTarget {
        domain,
        endpoint_host: edgeone_api_host(config.get("endpoint").map(String::as_str)),
        region,
        zone_id,
    })
}

fn edgeone_overseas_access_config_signature(
    provider_name: &str,
    target: &EdgeOneDomainTarget,
) -> String {
    sha256_hex(
        &[
            EDGEONE_OVERSEAS_ACCESS_SYNC_VERSION,
            provider_name,
            target.zone_id.as_str(),
            target.domain.as_str(),
            target.endpoint_host.as_str(),
            target.region.as_deref().unwrap_or(""),
        ]
        .join("\n"),
    )
}

#[allow(clippy::too_many_arguments)]
async fn sync_edgeone_overseas_access_scope(
    translator: &Translator,
    client: &DDNSHttpClient,
    config: &HashMap<String, String>,
    provider_name: &str,
    secret_id: &str,
    secret_key: &str,
    target: &EdgeOneDomainTarget,
    mode: &str,
    scope: &str,
) -> anyhow::Result<bool> {
    let existing_rules = describe_edgeone_custom_rules(
        translator, client, config, secret_id, secret_key, target, scope,
    )
    .await?;
    let managed_rules = existing_rules
        .iter()
        .filter(|rule| is_edgeone_managed_rule(provider_name, rule))
        .cloned()
        .collect::<Vec<_>>();
    let remaining_rules = existing_rules
        .into_iter()
        .filter(|rule| !is_edgeone_managed_rule(provider_name, rule))
        .collect::<Vec<_>>();
    let existing_managed_rule = managed_rules.first().cloned();

    if mode != "block_overseas" {
        if managed_rules.is_empty() {
            return Ok(false);
        }
        let tracked_rule = existing_managed_rule
            .clone()
            .unwrap_or_else(|| build_edgeone_managed_rule(provider_name));
        modify_edgeone_custom_rules(
            translator,
            client,
            config,
            provider_name,
            secret_id,
            secret_key,
            target,
            scope,
            remaining_rules,
            &tracked_rule,
        )
        .await?;
        return Ok(true);
    }

    let desired_rule = build_edgeone_managed_rule(provider_name);
    if managed_rules.len() == 1
        && existing_managed_rule
            .as_ref()
            .is_some_and(|rule| edgeone_managed_rule_same(rule, &desired_rule))
    {
        return Ok(false);
    }
    let mut rules = remaining_rules;
    rules.push(desired_rule.clone());
    modify_edgeone_custom_rules(
        translator,
        client,
        config,
        provider_name,
        secret_id,
        secret_key,
        target,
        scope,
        rules,
        &desired_rule,
    )
    .await?;
    Ok(true)
}

async fn describe_edgeone_custom_rules(
    translator: &Translator,
    client: &DDNSHttpClient,
    config: &HashMap<String, String>,
    secret_id: &str,
    secret_key: &str,
    target: &EdgeOneDomainTarget,
    scope: &str,
) -> anyhow::Result<Vec<Value>> {
    let response = edgeone_request(
        translator,
        client,
        config,
        secret_id,
        secret_key,
        "DescribeSecurityPolicy",
        json!({
            "ZoneId": target.zone_id,
            "Entity": edgeone_scope_entity(scope),
        }),
    )
    .await
    .map_err(|error| {
        anyhow::anyhow!(
            "{}",
            ddns_text(
                translator,
                "providers.edgeone.overseasAccess.describeRulesFailed",
                &[
                    ("target", target.domain.clone()),
                    ("zoneId", target.zone_id.clone()),
                    ("endpointHost", target.endpoint_host.clone()),
                    (
                        "region",
                        target.region.clone().unwrap_or_else(|| "empty".to_string()),
                    ),
                    ("entity", edgeone_scope_entity(scope).to_string()),
                    ("scope", scope.to_string()),
                    ("message", error.to_string()),
                ],
            )
        )
    })?;
    Ok(response
        .pointer("/SecurityPolicy/CustomRules/Rules")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

#[allow(clippy::too_many_arguments)]
async fn modify_edgeone_custom_rules(
    translator: &Translator,
    client: &DDNSHttpClient,
    config: &HashMap<String, String>,
    provider_name: &str,
    secret_id: &str,
    secret_key: &str,
    target: &EdgeOneDomainTarget,
    scope: &str,
    rules: Vec<Value>,
    tracked_rule: &Value,
) -> anyhow::Result<()> {
    let count = rules.len();
    edgeone_request(
        translator,
        client,
        config,
        secret_id,
        secret_key,
        "ModifySecurityPolicy",
        json!({
            "ZoneId": target.zone_id,
            "Entity": edgeone_scope_entity(scope),
            "SecurityPolicy": {
                "CustomRules": {
                    "Rules": rules,
                },
            },
            "SecurityConfig": {},
        }),
    )
    .await
    .map(|_| ())
    .map_err(|error| {
        anyhow::anyhow!(
            "{}",
            ddns_text(
                translator,
                "providers.edgeone.overseasAccess.syncFailedWithAttempt",
                &[
                    (
                        "attempt",
                        edgeone_overseas_access_attempt_label(
                            provider_name,
                            tracked_rule,
                            scope,
                            target,
                        ),
                    ),
                    ("count", count.to_string()),
                    ("message", error.to_string()),
                ],
            )
        )
    })
}

fn edgeone_scope_entity(scope: &str) -> &'static str {
    if scope == "zone_level_domain" {
        "@ZoneLevel@domain"
    } else {
        "ZoneDefaultPolicy"
    }
}

fn build_edgeone_managed_rule(provider_name: &str) -> Value {
    json!({
        "Name": edgeone_managed_rule_name(provider_name),
        "Condition": edgeone_console_country_condition(),
        "Action": { "Name": "Deny" },
        "Enabled": "on",
        "RuleType": "BasicAccessRule",
    })
}

fn edgeone_managed_rule_name(provider_name: &str) -> String {
    format!("fk_eo_ovs_{}", &sha256_hex(provider_name)[..12])
}

fn edgeone_legacy_managed_rule_names(provider_name: &str) -> Vec<String> {
    vec![
        format!("{EDGEONE_OVERSEAS_ACCESS_RULE_NAME_PREFIX}{provider_name}"),
        EDGEONE_OVERSEAS_ACCESS_LEGACY_RULE_NAME.to_string(),
    ]
}

fn is_edgeone_managed_rule(provider_name: &str, rule: &Value) -> bool {
    let name = rule.get("Name").and_then(Value::as_str).unwrap_or("");
    name == edgeone_managed_rule_name(provider_name)
        || edgeone_legacy_managed_rule_names(provider_name)
            .iter()
            .any(|legacy| legacy == name)
}

fn edgeone_managed_rule_same(left: &Value, right: &Value) -> bool {
    left.get("Name").and_then(Value::as_str) == right.get("Name").and_then(Value::as_str)
        && left.get("Condition").and_then(Value::as_str)
            == right.get("Condition").and_then(Value::as_str)
        && left.get("Enabled").and_then(Value::as_str)
            == right.get("Enabled").and_then(Value::as_str)
        && left.get("RuleType").and_then(Value::as_str)
            == right.get("RuleType").and_then(Value::as_str)
        && left.pointer("/Action/Name").and_then(Value::as_str)
            == right.pointer("/Action/Name").and_then(Value::as_str)
}

fn edgeone_console_country_condition() -> String {
    let codes = EDGEONE_ALLOWED_MAINLAND_REGION_CODES
        .iter()
        .map(|code| format!("'{}'", edgeone_escape_condition_value(code)))
        .collect::<Vec<_>>()
        .join(",");
    format!("not ${{http.request.ip.country}} in [{codes}]")
}

fn edgeone_escape_condition_value(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "\\'")
}

fn edgeone_overseas_access_attempt_label(
    provider_name: &str,
    rule: &Value,
    scope: &str,
    target: &EdgeOneDomainTarget,
) -> String {
    [
        format!("provider_name={provider_name}"),
        format!("provider_target={}", target.domain),
        format!("zone_id={}", target.zone_id),
        format!("endpoint_host={}", target.endpoint_host),
        format!("region={}", target.region.as_deref().unwrap_or("empty")),
        format!("entity={}", edgeone_scope_entity(scope)),
        format!("scope={scope}"),
        "module=SecurityPolicy.CustomRules".to_string(),
        format!(
            "rule_name={}",
            rule.get("Name").and_then(Value::as_str).unwrap_or("")
        ),
        format!(
            "rule_type={}",
            rule.get("RuleType")
                .and_then(Value::as_str)
                .unwrap_or("BasicAccessRule")
        ),
        format!(
            "allowed_regions={}",
            EDGEONE_ALLOWED_MAINLAND_REGION_CODES.join(",")
        ),
        format!(
            "condition={}",
            rule.get("Condition").and_then(Value::as_str).unwrap_or("")
        ),
    ]
    .join(", ")
}

pub(in crate::ddns::routes) async fn edgeone_request(
    translator: &Translator,
    client: &DDNSHttpClient,
    config: &HashMap<String, String>,
    secret_id: &str,
    secret_key: &str,
    action: &str,
    payload: Value,
) -> anyhow::Result<Value> {
    let host = edgeone_api_host(config.get("endpoint").map(String::as_str));
    let region = config_value(config, "region");
    tencentcloud_tc3_request(
        translator,
        client,
        secret_id,
        secret_key,
        action,
        payload,
        &host,
        "teo",
        "2022-09-01",
        if region.is_empty() {
            None
        } else {
            Some(region.as_str())
        },
    )
    .await
}

pub(in crate::ddns::routes) async fn validate_edgeone_pair_root_in_zone(
    translator: &Translator,
    config: &HashMap<String, String>,
    http_options: &DDNSHttpClientOptions,
    pair_root: &str,
) -> anyhow::Result<()> {
    let secret_id = config_value(config, "secret_id");
    let secret_key = config_value(config, "secret_key");
    let zone_id = config_value(config, "zone_id");
    if secret_id.is_empty() || secret_key.is_empty() || zone_id.is_empty() {
        anyhow::bail!(
            "{}",
            ddns_text(translator, "providers.edgeone.configIncomplete", &[])
        );
    }
    let client = ddns_http_client(translator, http_options).map_err(|error| {
        anyhow::anyhow!(ddns_text(
            translator,
            "providers.edgeone.zoneLookupFailed",
            &[("detail", error.to_string())],
        ))
    })?;
    let response = edgeone_request(
        translator,
        &client,
        config,
        &secret_id,
        &secret_key,
        "DescribeZones",
        json!({
            "Offset": 0,
            "Limit": 20,
            "Filters": [{
                "Name": "zone-id",
                "Values": [zone_id],
            }],
        }),
    )
    .await
    .map_err(|error| {
        anyhow::anyhow!(ddns_text(
            translator,
            "providers.edgeone.zoneLookupFailed",
            &[("detail", error.to_string())],
        ))
    })?;
    let remote_root = edgeone_zone_name_from_response(&response, &zone_id).unwrap_or_default();
    if !ddns_domain_is_same_or_subdomain(pair_root, &remote_root) {
        anyhow::bail!(
            "{}",
            ddns_text(
                translator,
                "providers.edgeone.zoneMismatch",
                &[
                    (
                        "expected",
                        if remote_root.is_empty() {
                            "<unknown>".to_string()
                        } else {
                            remote_root
                        },
                    ),
                    ("actual", pair_root.to_string()),
                ],
            )
        );
    }
    Ok(())
}

pub(in crate::ddns::routes) fn edgeone_zone_name_from_response(
    data: &Value,
    zone_id: &str,
) -> Option<String> {
    data.get("Zones")
        .and_then(Value::as_array)?
        .iter()
        .find(|zone| zone.get("ZoneId").and_then(Value::as_str) == Some(zone_id))?
        .get("ZoneName")
        .and_then(Value::as_str)
        .map(|name| normalize_domain(name).to_ascii_lowercase())
        .filter(|name| !name.is_empty())
}

pub(in crate::ddns::routes) fn normalize_edgeone_location(value: Option<&str>) -> String {
    let trimmed = value.unwrap_or_default().trim();
    if trimmed.is_empty() {
        "default".to_string()
    } else {
        trimmed.to_ascii_lowercase()
    }
}

pub(in crate::ddns::routes) fn edgeone_api_host(endpoint: Option<&str>) -> String {
    let value = endpoint.unwrap_or_default().trim();
    if value.is_empty() {
        return "teo.tencentcloudapi.com".to_string();
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return url::Url::parse(value)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string))
            .filter(|host| !host.is_empty())
            .unwrap_or_else(|| "teo.tencentcloudapi.com".to_string());
    }
    value.trim_end_matches('/').to_string()
}

pub(in crate::ddns::routes) fn is_valid_edgeone_host_header(value: &str) -> bool {
    let host = normalize_domain(value);
    if host.is_empty()
        || host.contains('/')
        || host.contains(':')
        || host.contains('[')
        || host.contains(']')
        || host.contains('*')
        || host.len() > 253
        || value.split_whitespace().count() > 1
        || value.starts_with("http://")
        || value.starts_with("https://")
    {
        return false;
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    })
}
