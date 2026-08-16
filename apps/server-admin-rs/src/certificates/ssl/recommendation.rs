use super::*;

pub(super) use crate::certificates::domain_utils::{
    is_requirement_covered_by_certificate_domains, normalize_domain_name, uniq_domain_strings,
};
pub(super) use crate::time_utils::node_iso_now as now_node_iso;

pub(super) fn build_subdomain_certificate_recommendation(
    auth_port: u16,
    config: &Value,
    t: &Translator,
) -> Value {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
        .unwrap_or_default();
    let auth_host = auth_host_mapping(auth_port, config)
        .or_else(|| {
            config
                .pointer("/subdomain_mode/auth_host")
                .and_then(Value::as_str)
                .map(normalize_domain_name)
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_default();
    let all_hosts = host_mapping_hosts(config);

    let mut mode = "manual";
    let mut summary = subdomain_text(t, "recommendationMissingBase");
    let mut warnings = Vec::<String>::new();
    let mut recommended_domains = Vec::<String>::new();

    if !root_domain.is_empty() {
        mode = "wildcard_parent";
        let wildcard_domain = format!("*.{root_domain}");
        recommended_domains = uniq_domain_strings([root_domain.as_str(), wildcard_domain.as_str()]);
        summary = subdomain_text_params(
            t,
            "recommendationWildcardSummary",
            &[("rootDomain", root_domain.clone())],
        );
        if !auth_host.is_empty()
            && !is_requirement_covered_by_certificate_domains(&auth_host, &recommended_domains)
        {
            recommended_domains = uniq_domain_strings(
                recommended_domains
                    .iter()
                    .map(String::as_str)
                    .chain(std::iter::once(auth_host.as_str())),
            );
            warnings.push(subdomain_text_params(
                t,
                "authOutOfRootWarning",
                &[
                    ("authHost", auth_host.clone()),
                    ("rootDomain", root_domain.clone()),
                ],
            ));
        }
    } else if !auth_host.is_empty() {
        mode = "single_host";
        recommended_domains = vec![auth_host.clone()];
        summary = subdomain_text_params(
            t,
            "recommendationSingleHostSummary",
            &[("authHost", auth_host.clone())],
        );
        warnings.push(subdomain_text(t, "wildcardSuggestion"));
    } else {
        warnings.push(subdomain_text(t, "configureRootOrAuth"));
    }

    if auth_host.is_empty() {
        warnings.push(subdomain_text(t, "authMissingWarning"));
    }

    let covered_hosts = all_hosts
        .iter()
        .filter(|host| is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_hosts = all_hosts
        .iter()
        .filter(|host| !is_requirement_covered_by_certificate_domains(host, &recommended_domains))
        .cloned()
        .collect::<Vec<_>>();

    if !uncovered_hosts.is_empty() && !recommended_domains.is_empty() {
        warnings.push(subdomain_text_params(
            t,
            "uncoveredHostMappingsWarning",
            &[("count", uncovered_hosts.len().to_string())],
        ));
    }

    json!({
        "mode": mode,
        "root_domain": if root_domain.is_empty() { Value::Null } else { json!(root_domain) },
        "auth_host": if auth_host.is_empty() { Value::Null } else { json!(auth_host) },
        "recommended_domains": recommended_domains,
        "covered_hosts": covered_hosts,
        "uncovered_hosts": uncovered_hosts,
        "warnings": warnings,
        "can_autofill": !recommended_domains.is_empty(),
        "summary": summary,
    })
}

pub(super) fn build_subdomain_certificate_coverage(
    auth_port: u16,
    config: &Value,
    certificate_domains: &[String],
    t: &Translator,
) -> Value {
    let recommendation = build_subdomain_certificate_recommendation(auth_port, config, t);
    let current_certificate_domains =
        uniq_domain_strings(certificate_domains.iter().map(String::as_str));
    let all_hosts = host_mapping_hosts(config);
    let recommended_domains = recommendation_domains(&recommendation);
    let auth_host = recommendation
        .get("auth_host")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let concrete_requirements = uniq_domain_strings(
        std::iter::once(auth_host.as_str()).chain(all_hosts.iter().map(String::as_str)),
    );
    let effective_requirements = if concrete_requirements.is_empty() {
        recommended_domains.clone()
    } else {
        concrete_requirements.clone()
    };
    let covered_recommended_domains = recommended_domains
        .iter()
        .filter(|domain| {
            is_requirement_covered_by_certificate_domains(domain, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_recommended_domains = recommended_domains
        .iter()
        .filter(|domain| {
            !is_requirement_covered_by_certificate_domains(domain, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let covered_hosts = all_hosts
        .iter()
        .filter(|host| {
            is_requirement_covered_by_certificate_domains(host, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_hosts = all_hosts
        .iter()
        .filter(|host| {
            !is_requirement_covered_by_certificate_domains(host, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let covers_auth_host = if auth_host.is_empty() {
        false
    } else {
        is_requirement_covered_by_certificate_domains(&auth_host, &current_certificate_domains)
    };
    let covered_requirements = effective_requirements
        .iter()
        .filter(|requirement| {
            is_requirement_covered_by_certificate_domains(requirement, &current_certificate_domains)
        })
        .cloned()
        .collect::<Vec<_>>();
    let uncovered_requirements = effective_requirements
        .iter()
        .filter(|requirement| {
            !is_requirement_covered_by_certificate_domains(
                requirement,
                &current_certificate_domains,
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let has_concrete_requirements = !concrete_requirements.is_empty();

    let mut status = "missing";
    let mut summary = subdomain_text(t, "coverageNoSsl");
    let mut warnings = recommendation
        .get("warnings")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if current_certificate_domains.is_empty() {
        if recommendation.get("can_autofill").and_then(Value::as_bool) != Some(true) {
            summary = recommendation
                .get("summary")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
        }
    } else if uncovered_requirements.is_empty() {
        status = "ready";
        summary = if has_concrete_requirements {
            subdomain_text(t, "coverageReadyConcrete")
        } else {
            subdomain_text(t, "coverageReadyRecommended")
        };
    } else if !covered_requirements.is_empty() {
        status = "partial";
        summary = if has_concrete_requirements {
            subdomain_text(t, "coveragePartialConcrete")
        } else {
            subdomain_text(t, "coveragePartialRecommended")
        };
    } else {
        summary = if has_concrete_requirements {
            subdomain_text(t, "coverageMismatchConcrete")
        } else {
            subdomain_text(t, "coverageMismatchRecommended")
        };
    }

    if !current_certificate_domains.is_empty()
        && has_concrete_requirements
        && !uncovered_requirements.is_empty()
    {
        warnings.push(subdomain_text_params(
            t,
            "coverageMissingRequiredWarning",
            &[("count", uncovered_requirements.len().to_string())],
        ));
    } else if !current_certificate_domains.is_empty()
        && !has_concrete_requirements
        && !uncovered_recommended_domains.is_empty()
    {
        warnings.push(subdomain_text_params(
            t,
            "coverageMissingRecommendedWarning",
            &[("count", uncovered_recommended_domains.len().to_string())],
        ));
    }

    if !current_certificate_domains.is_empty() && !auth_host.is_empty() && !covers_auth_host {
        warnings.push(subdomain_text_params(
            t,
            "coverageAuthHostMissingWarning",
            &[("authHost", auth_host.clone())],
        ));
    }

    json!({
        "status": status,
        "auth_host": if auth_host.is_empty() { Value::Null } else { json!(auth_host) },
        "certificate_domains": current_certificate_domains,
        "recommended_domains": recommended_domains,
        "covered_recommended_domains": covered_recommended_domains,
        "uncovered_recommended_domains": uncovered_recommended_domains,
        "covered_hosts": covered_hosts,
        "uncovered_hosts": uncovered_hosts,
        "covers_auth_host": covers_auth_host,
        "warnings": warnings,
        "summary": summary,
    })
}

pub(super) fn build_subdomain_certificate_inventory_coverage(
    auth_port: u16,
    config: &Value,
    certificates: &[CertificateCoverageInput],
    active_certificate_id: Option<&str>,
    deployment_mode: &str,
    t: &Translator,
) -> Value {
    let recommendation = build_subdomain_certificate_recommendation(auth_port, config, t);
    let all_hosts = host_mapping_hosts(config);
    let recommended_domains = recommendation_domains(&recommendation);
    let auth_host = recommendation
        .get("auth_host")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let concrete_requirements = uniq_domain_strings(
        std::iter::once(auth_host.as_str()).chain(all_hosts.iter().map(String::as_str)),
    );
    let requirements = if concrete_requirements.is_empty() {
        recommended_domains
    } else {
        concrete_requirements
    };

    let analyses = certificates
        .iter()
        .map(|certificate| {
            let normalized_domains =
                uniq_domain_strings(certificate.certificate_domains.iter().map(String::as_str));
            let coverage =
                build_subdomain_certificate_coverage(auth_port, config, &normalized_domains, t);
            let covered_requirements = requirements
                .iter()
                .filter(|requirement| {
                    is_requirement_covered_by_certificate_domains(requirement, &normalized_domains)
                })
                .cloned()
                .collect::<Vec<_>>();
            CertificateCoverageAnalysis {
                id: certificate.id.clone(),
                coverage,
                covered_requirements,
            }
        })
        .collect::<Vec<_>>();

    let fully_covering = analyses
        .iter()
        .filter(|item| item.coverage.get("status").and_then(Value::as_str) == Some("ready"))
        .cloned()
        .collect::<Vec<_>>();
    let partially_covering = analyses
        .iter()
        .filter(|item| {
            item.coverage.get("status").and_then(Value::as_str) != Some("ready")
                && !item.covered_requirements.is_empty()
        })
        .cloned()
        .collect::<Vec<_>>();
    let active_analysis = active_certificate_id
        .and_then(|id| analyses.iter().find(|item| item.id == id))
        .cloned();

    let mut uncovered_requirements = requirements.iter().cloned().collect::<BTreeSet<_>>();
    let mut combined_covering_certificate_ids = Vec::<String>::new();
    let mut remaining = analyses.clone();

    while !uncovered_requirements.is_empty() && !remaining.is_empty() {
        let mut best_index = None;
        let mut best_gain = 0_usize;
        for (index, item) in remaining.iter().enumerate() {
            let gain = item
                .covered_requirements
                .iter()
                .filter(|requirement| uncovered_requirements.contains(*requirement))
                .count();
            if gain > best_gain {
                best_gain = gain;
                best_index = Some(index);
            }
        }
        let Some(best_index) = best_index else {
            break;
        };
        if best_gain == 0 {
            break;
        }
        let selected = remaining.remove(best_index);
        combined_covering_certificate_ids.push(selected.id.clone());
        for requirement in selected.covered_requirements {
            uncovered_requirements.remove(&requirement);
        }
    }

    let combined_ready = !requirements.is_empty() && uncovered_requirements.is_empty();
    let active_ready = active_analysis
        .as_ref()
        .and_then(|item| item.coverage.get("status").and_then(Value::as_str))
        == Some("ready");
    let deployment_mode = normalize_deployment_mode(Some(deployment_mode));

    let mut status = "missing";
    let summary;
    let mut warnings = Vec::<String>::new();

    if active_ready {
        status = "ready";
        summary = subdomain_text(t, "inventoryActiveReady");
    } else if fully_covering.len() == 1 {
        status = "ready";
        summary = subdomain_text(t, "inventoryOneReady");
    } else if fully_covering.len() > 1 {
        status = "ready";
        summary = subdomain_text_params(
            t,
            "inventoryMultipleReady",
            &[("count", fully_covering.len().to_string())],
        );
    } else if combined_ready && deployment_mode == "multi_sni" {
        status = "ready";
        summary = if combined_covering_certificate_ids.len() > 1 {
            subdomain_text(t, "inventoryCombinedReady")
        } else {
            subdomain_text(t, "inventoryCandidateReady")
        };
    } else if combined_ready {
        status = "partial";
        summary = subdomain_text(t, "inventoryCombinedNeedsMultiSni");
    } else if !partially_covering.is_empty() {
        status = "partial";
        summary = subdomain_text(t, "inventoryPartialCandidates");
    } else if recommendation.get("can_autofill").and_then(Value::as_bool) == Some(true) {
        summary = subdomain_text(t, "inventoryNoCertificateCoversRecommendation");
    } else {
        summary = recommendation
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
    }

    if combined_ready
        && combined_covering_certificate_ids.len() > 1
        && deployment_mode != "multi_sni"
    {
        warnings.push(subdomain_text(t, "inventoryMultiCertRequiresSniWarning"));
    }
    if active_analysis.is_some() && !active_ready && fully_covering.len() == 1 {
        warnings.push(subdomain_text(t, "inventorySwitchRecommendedWarning"));
    }
    if active_analysis.is_none()
        && fully_covering.is_empty()
        && combined_covering_certificate_ids.len() > 1
    {
        warnings.push(subdomain_text(t, "inventoryBetterForSniWarning"));
    }

    let suggested_certificate_id = if active_ready || fully_covering.len() != 1 {
        Value::Null
    } else {
        json!(fully_covering[0].id)
    };

    json!({
        "status": status,
        "deployment_mode": deployment_mode,
        "active_certificate_id": active_analysis.as_ref().map(|item| item.id.as_str()),
        "fully_covering_certificate_ids": fully_covering.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "partially_covering_certificate_ids": partially_covering.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
        "combined_covering_certificate_ids": combined_covering_certificate_ids,
        "suggested_certificate_id": suggested_certificate_id,
        "can_auto_activate": !active_ready && fully_covering.len() == 1,
        "warnings": warnings,
        "summary": summary,
    })
}

#[derive(Clone, Debug)]
pub(super) struct CertificateCoverageAnalysis {
    id: String,
    coverage: Value,
    covered_requirements: Vec<String>,
}

pub(super) fn certificate_dns_names(certificate: &Value) -> Vec<String> {
    certificate
        .get("certInfo")
        .map(certificate_info_dns_names)
        .unwrap_or_default()
}

pub(super) fn certificate_info_dns_names(info: &Value) -> Vec<String> {
    info.get("dnsNames")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(super) fn recommendation_domains(recommendation: &Value) -> Vec<String> {
    recommendation
        .get("recommended_domains")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

pub(super) fn host_mapping_hosts(config: &Value) -> Vec<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)
        .map(|mappings| {
            uniq_domain_strings(
                mappings
                    .iter()
                    .filter_map(|mapping| mapping.get("host").and_then(Value::as_str)),
            )
        })
        .unwrap_or_default()
}

pub(super) fn auth_host_mapping(auth_port: u16, config: &Value) -> Option<String> {
    config
        .get("host_mappings")
        .and_then(Value::as_array)?
        .iter()
        .find(|mapping| is_auth_service_mapping(auth_port, mapping))
        .and_then(|mapping| mapping.get("host").and_then(Value::as_str))
        .map(normalize_domain_name)
        .filter(|value| !value.is_empty())
}

pub(super) fn is_auth_service_mapping(auth_port: u16, mapping: &Value) -> bool {
    if mapping.get("service_role").and_then(Value::as_str) == Some("auth") {
        return true;
    }
    let target = mapping.get("target").and_then(Value::as_str).unwrap_or("");
    parse_target_port(target) == Some(auth_port)
}

pub(super) use crate::proxy_utils::parse_url_target_port_u16 as parse_target_port;

pub(super) fn subdomain_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.subdomainMode.{key}"))
}

pub(super) fn subdomain_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.subdomainMode.{key}"), params)
}

pub(super) fn build_ssl_certificate_id(cert: &str, key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(cert.as_bytes());
    hasher.update(b"\n");
    hasher.update(key.as_bytes());
    let digest = hex::encode(hasher.finalize());
    format!("ssl_{}", &digest[..16])
}

pub(super) fn normalize_certificate_source(value: Option<&str>) -> &'static str {
    match value {
        Some("acme") => "acme",
        Some("ca") => "ca",
        Some("external") => "external",
        _ => "manual",
    }
}

pub(super) fn normalize_deployment_mode(value: Option<&str>) -> &'static str {
    if value == Some("multi_sni") {
        "multi_sni"
    } else {
        "single_active"
    }
}

pub(super) fn should_sync_ssl_deployment_after_save(activate: bool, deployment_mode: &str) -> bool {
    activate || normalize_deployment_mode(Some(deployment_mode)) == "multi_sni"
}

pub(super) fn default_certificate_label(source: &str, primary_domain: Option<&str>) -> String {
    if let Some(primary_domain) = primary_domain {
        return primary_domain.to_string();
    }
    let translator = Translator::new(DEFAULT_LOCALE);
    match source {
        "acme" => translator.t("server.store.certificateLabels.acme"),
        "ca" => translator.t("server.store.certificateLabels.ca"),
        "external" => translator.t("server.store.certificateLabels.external"),
        "current" => translator.t("server.store.certificateLabels.current"),
        _ => translator.t("server.store.certificateLabels.manual"),
    }
}

pub(super) fn normalize_timestamp(value: Option<&Value>) -> Option<String> {
    let raw = value.and_then(Value::as_str)?.trim();
    time_utils::normalize_node_iso(raw)
}
