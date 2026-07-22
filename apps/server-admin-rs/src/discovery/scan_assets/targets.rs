use super::*;

pub(super) fn build_discover_targets_payload(
    state: &AppState,
    headers: &HeaderMap,
    config: &Value,
    translator: &Translator,
) -> Value {
    let docker_candidates = if deployment_target(state) == "docker" {
        resolve_docker_discover_candidates(headers)
    } else {
        Vec::new()
    };
    let automatic_targets =
        build_automatic_discover_targets(state, config, translator, &docker_candidates);
    let scan_discovery = config.get("scan_discovery");
    let custom_targets = build_custom_discover_targets(
        scan_discovery
            .and_then(|value| value.get("custom_cidrs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string)),
        translator,
    );
    let saved_selected_cidrs = normalize_allowed_scan_cidrs(
        scan_discovery
            .and_then(|value| value.get("selected_cidrs"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string)),
    );
    let automatic_cidrs = automatic_targets
        .iter()
        .filter_map(|item| item.get("cidr").and_then(Value::as_str).map(str::to_string))
        .collect::<Vec<_>>();
    let automatic_cidr_set = automatic_cidrs.iter().cloned().collect::<BTreeSet<_>>();
    let host_candidates = docker_candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            json!({
                "address": candidate.address,
                "cidr": candidate.cidr,
                "source": candidate.source,
                "recommended": index == 0,
                "includedInAutomaticScan": automatic_cidr_set.contains(&candidate.cidr),
            })
        })
        .collect::<Vec<_>>();
    let selection_mode = if saved_selected_cidrs.is_empty() {
        "automatic"
    } else {
        "custom"
    };
    let selected_cidrs = if saved_selected_cidrs.is_empty() {
        automatic_cidrs.clone()
    } else {
        saved_selected_cidrs
    };
    let effective_cidrs = if selected_cidrs.is_empty() {
        automatic_cidrs
    } else {
        selected_cidrs
    };
    let selected_targets = build_saved_discover_targets(effective_cidrs.clone(), translator);
    json!({
        "automaticTargets": automatic_targets,
        "hostCandidates": host_candidates,
        "customTargets": custom_targets,
        "selectedTargets": selected_targets,
        "selectionMode": selection_mode,
        "selectedCidrs": effective_cidrs,
        "effectiveCidrs": effective_cidrs,
        "limits": {
            "maxCidrs": MAX_SCAN_CIDRS,
            "maxHosts": MAX_SCAN_HOSTS
        }
    })
}

pub(super) fn build_automatic_discover_targets(
    state: &AppState,
    config: &Value,
    translator: &Translator,
    docker_candidates: &[DockerDiscoverHostCandidate],
) -> Vec<Value> {
    let mut targets = Vec::new();
    if deployment_target(state) == "docker" {
        targets.extend(
            docker_candidates
                .iter()
                .map(|candidate| build_docker_discover_target(candidate, translator)),
        );
        targets.extend(build_mapping_discover_targets(config, translator));
        targets.extend(build_interface_discover_targets(translator));
    } else {
        let cidr = "127.0.0.1/32";
        targets.push(to_discover_target(
            cidr,
            &scan_discovery_target_label(translator, "loopback", &[("cidr", cidr.to_string())]),
            "loopback",
            true,
        ));
        targets.extend(build_interface_discover_targets(translator));
        targets.extend(build_mapping_discover_targets(config, translator));
    }
    limit_automatic_targets(dedupe_targets(targets))
}

pub(super) fn resolve_discover_self_hosts(state: &AppState, headers: &HeaderMap) -> Vec<String> {
    let mut hosts = vec![LOOPBACK_DISCOVERY_HOST.to_string()];
    hosts.extend(
        list_private_ipv4_candidates()
            .into_iter()
            .map(|candidate| candidate.address),
    );
    if deployment_target(state) == "docker" {
        hosts.extend(
            resolve_docker_discover_candidates(headers)
                .into_iter()
                .map(|candidate| candidate.address),
        );
    }
    normalize_discover_self_hosts(hosts)
}

pub(super) fn normalize_discover_self_hosts(
    hosts: impl IntoIterator<Item = String>,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for host in hosts {
        let host = host.trim().to_string();
        if host.parse::<Ipv4Addr>().is_err() || !is_allowed_scan_ipv4(&host) {
            continue;
        }
        if seen.insert(host.clone()) {
            output.push(host);
        }
    }
    output
}

pub(super) fn collect_excluded_ports(state: &AppState) -> Vec<u16> {
    let mut ports = Vec::new();
    if !runtime_profile::admin_panel_protected_runtime(state)
        && let Some(port) = excluded_env_port("ADMIN_VIEW_PORT", 7991)
    {
        ports.push(port);
    }
    let default_backend_port = if deployment_target(state) == "openwrt" {
        17_998
    } else {
        7_998
    };
    for (name, fallback) in [
        ("BACKEND_PORT", default_backend_port),
        ("AUTH_PORT", 7_997),
        ("GO_BACKEND_PORT", 7_996),
        ("GO_REPROXY_PORT", 7_999),
    ] {
        if let Some(port) = excluded_env_port(name, fallback) {
            ports.push(port);
        }
    }
    ports.extend([7_995, 8_000, 8_200, 30_661, 30_662]);
    ports
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn build_docker_discover_target(
    candidate: &DockerDiscoverHostCandidate,
    translator: &Translator,
) -> Option<Value> {
    let cidr = candidate.cidr.clone();
    to_discover_target(
        &cidr,
        &scan_discovery_target_label(translator, "docker", &[("cidr", cidr.clone())]),
        "docker",
        true,
    )
}

pub(super) fn limit_automatic_targets(targets: Vec<Value>) -> Vec<Value> {
    let mut output = Vec::new();
    let mut cidrs = Vec::new();
    for target in targets {
        let Some(cidr) = target.get("cidr").and_then(Value::as_str) else {
            continue;
        };
        if output.len() >= MAX_SCAN_CIDRS {
            break;
        }
        let mut candidate_cidrs = cidrs.clone();
        candidate_cidrs.push(cidr.to_string());
        if count_scan_hosts(&candidate_cidrs).is_err() {
            continue;
        }
        cidrs = candidate_cidrs;
        output.push(target);
    }
    output
}

pub(super) fn build_interface_discover_targets(translator: &Translator) -> Vec<Option<Value>> {
    list_private_ipv4_candidates()
        .into_iter()
        .filter_map(|candidate| {
            let cidr = build_interface_ipv4_cidr(&candidate.address, candidate.prefix)?;
            Some(to_discover_target(
                &cidr,
                &scan_discovery_target_label(
                    translator,
                    "interface",
                    &[("cidr", cidr.clone()), ("name", candidate.name)],
                ),
                "interface",
                true,
            ))
        })
        .collect()
}

pub(super) fn build_mapping_discover_targets(
    config: &Value,
    translator: &Translator,
) -> Vec<Option<Value>> {
    let mut targets = Vec::new();
    for key in ["proxy_mappings", "host_mappings"] {
        for mapping in config
            .get(key)
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
        {
            let Some(target) = mapping.get("target").and_then(Value::as_str) else {
                continue;
            };
            let Some(ip) = extract_ipv4_from_target(target) else {
                continue;
            };
            let cidr = build_ipv4_cidr(&ip, if ip.starts_with("127.") { 32 } else { 24 });
            if let Some(cidr) = cidr {
                targets.push(to_discover_target(
                    &cidr,
                    &scan_discovery_target_label(translator, "mapping", &[("cidr", cidr.clone())]),
                    "mapping",
                    true,
                ));
            }
        }
    }
    targets
}

pub(super) fn build_custom_discover_targets(
    cidrs: impl IntoIterator<Item = String>,
    translator: &Translator,
) -> Vec<Value> {
    normalize_allowed_scan_cidrs(cidrs)
        .into_iter()
        .filter_map(|cidr| {
            to_discover_target(
                &cidr,
                &scan_discovery_target_label(translator, "custom", &[("cidr", cidr.clone())]),
                "custom",
                false,
            )
        })
        .collect()
}

pub(super) fn build_saved_discover_targets(
    cidrs: impl IntoIterator<Item = String>,
    translator: &Translator,
) -> Vec<Value> {
    normalize_allowed_scan_cidrs(cidrs)
        .into_iter()
        .filter_map(|cidr| {
            to_discover_target(
                &cidr,
                &scan_discovery_target_label(translator, "saved", &[("cidr", cidr.clone())]),
                "saved",
                false,
            )
        })
        .collect()
}

pub(super) fn scan_discovery_target_label(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.scanDiscovery.targetLabels.{key}"), params)
}

pub(super) fn localize_scan_discovery_error(translator: &Translator, message: &str) -> String {
    let normalized = message.trim();
    if let Some(cidrs) = normalized.strip_prefix("Only local IPv4 CIDR ranges are supported: ") {
        return translator.t_params(
            "server.scanDiscovery.localIpv4CidrOnly",
            &[("cidrs", cidrs.to_string())],
        );
    }
    if normalized == format!("At most {MAX_SCAN_CIDRS} CIDR ranges can be selected") {
        return translator.t_params(
            "server.scanDiscovery.maxCidrsExceeded",
            &[("max", MAX_SCAN_CIDRS.to_string())],
        );
    }
    if let Some(current) = normalized
        .strip_prefix(&format!(
            "At most {MAX_SCAN_HOSTS} hosts can be scanned, current selection has "
        ))
        .and_then(|value| value.trim().parse::<u64>().ok())
    {
        return translator.t_params(
            "server.scanDiscovery.maxHostsExceededWithCurrent",
            &[
                ("max", MAX_SCAN_HOSTS.to_string()),
                ("current", current.to_string()),
            ],
        );
    }
    if normalized == format!("At most {MAX_SCAN_HOSTS} hosts can be scanned") {
        return translator.t_params(
            "server.scanDiscovery.maxHostsExceeded",
            &[("max", MAX_SCAN_HOSTS.to_string())],
        );
    }
    normalized.to_string()
}

pub(super) fn expand_scan_cidrs(cidrs: &[String]) -> Vec<String> {
    let mut hosts = Vec::new();
    let mut seen = BTreeSet::new();
    for cidr in cidrs {
        let Some(parsed) = parse_allowed_scan_cidr(cidr) else {
            continue;
        };
        for value in parsed.first_host..=parsed.last_host {
            let host = Ipv4Addr::from(value).to_string();
            if seen.insert(host.clone()) {
                hosts.push(host);
            }
            if hosts.len() as u64 >= MAX_SCAN_HOSTS {
                return hosts;
            }
        }
    }
    hosts
}

pub(super) fn build_scan_scope(cidrs: &[String]) -> Option<String> {
    if cidrs.is_empty() {
        None
    } else if cidrs.len() == 1 {
        cidrs.first().cloned()
    } else {
        Some(cidrs.join(", "))
    }
}

pub(super) fn to_discover_target(
    cidr: &str,
    label: &str,
    source: &str,
    is_automatic: bool,
) -> Option<Value> {
    let parsed = parse_allowed_scan_cidr(cidr)?;
    Some(json!({
        "cidr": parsed.cidr,
        "label": label,
        "source": source,
        "hostCount": parsed.host_count,
        "isAutomatic": is_automatic
    }))
}

pub(super) fn dedupe_targets(targets: Vec<Option<Value>>) -> Vec<Value> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for target in targets.into_iter().flatten() {
        let cidr = target
            .get("cidr")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !cidr.is_empty() && seen.insert(cidr) {
            output.push(target);
        }
    }
    output
}
