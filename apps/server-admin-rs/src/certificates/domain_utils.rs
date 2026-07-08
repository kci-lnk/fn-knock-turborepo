use std::collections::BTreeSet;

pub(crate) fn uniq_domain_strings<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for value in values {
        let normalized = normalize_domain_name(value);
        if normalized.is_empty() || !seen.insert(normalized.clone()) {
            continue;
        }
        output.push(normalized);
    }
    output
}

pub(crate) fn normalize_domain_name(value: &str) -> String {
    value.trim().trim_end_matches('.').to_ascii_lowercase()
}

pub(crate) fn is_wildcard_domain(value: &str) -> bool {
    normalize_domain_name(value).starts_with("*.")
}

pub(crate) fn strip_wildcard_prefix(value: &str) -> String {
    let normalized = normalize_domain_name(value);
    normalized
        .strip_prefix("*.")
        .unwrap_or(normalized.as_str())
        .to_string()
}

pub(crate) fn does_pattern_cover_concrete_host(concrete_host: &str, pattern: &str) -> bool {
    let normalized_host = normalize_domain_name(concrete_host);
    let normalized_pattern = normalize_domain_name(pattern);
    if normalized_host.is_empty()
        || normalized_pattern.is_empty()
        || is_wildcard_domain(&normalized_host)
    {
        return false;
    }
    if !is_wildcard_domain(&normalized_pattern) {
        return normalized_host == normalized_pattern;
    }
    let suffix = strip_wildcard_prefix(&normalized_pattern);
    if suffix.is_empty() || !normalized_host.ends_with(&format!(".{suffix}")) {
        return false;
    }
    let label = &normalized_host[..normalized_host.len() - suffix.len() - 1];
    !label.is_empty() && !label.contains('.')
}

pub(crate) fn is_requirement_covered_by_certificate_domains(
    requirement: &str,
    certificate_domains: &[String],
) -> bool {
    let requirement = normalize_domain_name(requirement);
    if requirement.is_empty() {
        return false;
    }
    if is_wildcard_domain(&requirement) {
        return certificate_domains
            .iter()
            .any(|domain| normalize_domain_name(domain) == requirement);
    }
    certificate_domains
        .iter()
        .any(|domain| does_pattern_cover_concrete_host(&requirement, domain))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcard_certificate_domain_covers_only_one_label() {
        assert!(does_pattern_cover_concrete_host(
            "app.example.com",
            "*.example.com"
        ));
        assert!(!does_pattern_cover_concrete_host(
            "deep.app.example.com",
            "*.example.com"
        ));
        assert!(!does_pattern_cover_concrete_host(
            "*.example.com",
            "*.example.com"
        ));
    }

    #[test]
    fn wildcard_requirement_requires_matching_wildcard_certificate() {
        let certificate_domains = vec!["example.com".to_string(), "*.example.com".to_string()];
        assert!(is_requirement_covered_by_certificate_domains(
            "*.example.com",
            &certificate_domains
        ));
        assert!(!is_requirement_covered_by_certificate_domains(
            "*.api.example.com",
            &certificate_domains
        ));
    }

    #[test]
    fn uniq_domain_strings_normalizes_and_preserves_order() {
        assert_eq!(
            uniq_domain_strings([" Example.COM. ", "example.com", "api.example.com"]),
            vec!["example.com".to_string(), "api.example.com".to_string()]
        );
    }
}
