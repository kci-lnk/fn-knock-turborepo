use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum DdnsDomainTargets {
    Single(String),
    WildcardRootPair { wildcard: String, root: String },
}

impl DdnsDomainTargets {
    pub(super) fn canonical(&self) -> String {
        match self {
            Self::Single(domain) => domain.clone(),
            Self::WildcardRootPair { wildcard, root } => format!("{wildcard},{root}"),
        }
    }

    pub(super) fn domains(&self) -> Vec<&str> {
        match self {
            Self::Single(domain) => vec![domain.as_str()],
            Self::WildcardRootPair { wildcard, root } => {
                vec![wildcard.as_str(), root.as_str()]
            }
        }
    }

    pub(super) fn pair_root(&self) -> Option<&str> {
        match self {
            Self::Single(_) => None,
            Self::WildcardRootPair { root, .. } => Some(root),
        }
    }

    pub(super) fn wildcard(&self) -> Option<&str> {
        match self {
            Self::Single(_) => None,
            Self::WildcardRootPair { wildcard, .. } => Some(wildcard),
        }
    }

    pub(super) fn is_pair(&self) -> bool {
        matches!(self, Self::WildcardRootPair { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub(super) enum DDNSDomainConfigError {
    #[error("Invalid DDNS domain: {domain}")]
    InvalidDomain { domain: String },
    #[error("Too many DDNS domains")]
    TooManyDomains,
    #[error("Invalid DDNS wildcard/root domain pair")]
    InvalidPair,
    #[error("DDNS wildcard and root domains do not match")]
    MismatchedPair,
    #[error("DDNS provider does not support wildcard/root pairs: {provider}")]
    PairUnsupported { provider: String },
    #[error("DDNS pair root field is missing: {field}")]
    PairRootMissing { field: String },
    #[error("DDNS pair root is outside {field}: zone {expected}, pair root {actual}")]
    PairRootMismatch {
        field: String,
        expected: String,
        actual: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DdnsDomainTargetsMode {
    Single,
    SingleOrWildcardRootPair,
}

impl DdnsDomainTargetsMode {
    fn as_catalog_value(self) -> &'static str {
        match self {
            Self::Single => "single",
            Self::SingleOrWildcardRootPair => "single_or_wildcard_root_pair",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DdnsDomainRootPreflight {
    None,
    CloudflareZone,
    EdgeOneZone,
    EsaSite,
    DynuService,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DdnsDomainUpdateExecution {
    Single,
    FanOut,
    DynuWildcardAlias,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DdnsProviderDomainPolicy {
    pub(super) mode: DdnsDomainTargetsMode,
    pub(super) root_field: Option<&'static str>,
    pub(super) preflight: DdnsDomainRootPreflight,
    pub(super) pair_execution: DdnsDomainUpdateExecution,
}

pub(super) fn ddns_provider_domain_policy(provider: &str) -> Option<DdnsProviderDomainPolicy> {
    let config_root = |field| DdnsProviderDomainPolicy {
        mode: DdnsDomainTargetsMode::SingleOrWildcardRootPair,
        root_field: Some(field),
        preflight: DdnsDomainRootPreflight::None,
        pair_execution: DdnsDomainUpdateExecution::FanOut,
    };
    match provider {
        "alidns" | "baiducloud" | "dnshe" | "dnspod" | "godaddy" | "huaweicloud" | "porkbun"
        | "tencentcloud" => Some(config_root("root_domain")),
        "esa" => Some(DdnsProviderDomainPolicy {
            preflight: DdnsDomainRootPreflight::EsaSite,
            ..config_root("site_name")
        }),
        "cloudflare" => Some(DdnsProviderDomainPolicy {
            mode: DdnsDomainTargetsMode::SingleOrWildcardRootPair,
            root_field: None,
            preflight: DdnsDomainRootPreflight::CloudflareZone,
            pair_execution: DdnsDomainUpdateExecution::FanOut,
        }),
        "edgeone" => Some(DdnsProviderDomainPolicy {
            mode: DdnsDomainTargetsMode::SingleOrWildcardRootPair,
            root_field: None,
            preflight: DdnsDomainRootPreflight::EdgeOneZone,
            pair_execution: DdnsDomainUpdateExecution::FanOut,
        }),
        "dynu" => Some(DdnsProviderDomainPolicy {
            mode: DdnsDomainTargetsMode::SingleOrWildcardRootPair,
            root_field: None,
            preflight: DdnsDomainRootPreflight::DynuService,
            pair_execution: DdnsDomainUpdateExecution::DynuWildcardAlias,
        }),
        "edgeone_cname" => Some(DdnsProviderDomainPolicy {
            mode: DdnsDomainTargetsMode::Single,
            root_field: None,
            preflight: DdnsDomainRootPreflight::None,
            pair_execution: DdnsDomainUpdateExecution::Single,
        }),
        _ => None,
    }
}

pub(super) fn parse_ddns_domain_targets(
    value: &str,
) -> Result<DdnsDomainTargets, DDNSDomainConfigError> {
    let parts = value
        .split(|ch: char| ch == ',' || ch == '，' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(DDNSDomainConfigError::InvalidDomain {
            domain: value.to_string(),
        });
    }
    if parts.len() > 2 {
        return Err(DDNSDomainConfigError::TooManyDomains);
    }

    let domains = parts
        .into_iter()
        .map(normalize_ddns_target_domain)
        .collect::<Result<Vec<_>, _>>()?;
    if domains.len() == 1 {
        return Ok(DdnsDomainTargets::Single(domains[0].clone()));
    }

    if domains[0] == domains[1] {
        return Err(DDNSDomainConfigError::InvalidPair);
    }
    let wildcard = domains.iter().find(|domain| domain.starts_with("*."));
    let root = domains.iter().find(|domain| !domain.starts_with("*."));
    let (Some(wildcard), Some(root)) = (wildcard, root) else {
        return Err(DDNSDomainConfigError::InvalidPair);
    };
    if wildcard.trim_start_matches("*.") != root {
        return Err(DDNSDomainConfigError::MismatchedPair);
    }
    Ok(DdnsDomainTargets::WildcardRootPair {
        wildcard: wildcard.clone(),
        root: root.clone(),
    })
}

fn normalize_ddns_target_domain(value: &str) -> Result<String, DDNSDomainConfigError> {
    let normalized = value.trim().trim_end_matches('.').to_ascii_lowercase();
    let invalid = || DDNSDomainConfigError::InvalidDomain {
        domain: value.to_string(),
    };
    if normalized.is_empty() || !normalized.is_ascii() || normalized.len() > 253 {
        return Err(invalid());
    }
    let hostname = normalized.strip_prefix("*.").unwrap_or(&normalized);
    if hostname.is_empty()
        || hostname.contains('*')
        || is_ipv4_literal(hostname)
        || hostname.split('.').count() < 2
    {
        return Err(invalid());
    }
    if !hostname.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }) {
        return Err(invalid());
    }
    Ok(normalized)
}

fn is_ipv4_literal(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 3
                && part.chars().all(|ch| ch.is_ascii_digit())
                && part.parse::<u8>().is_ok()
        })
}

pub(super) fn ddns_domain_is_same_or_subdomain(domain: &str, zone: &str) -> bool {
    if domain.is_empty() || zone.is_empty() {
        return false;
    }
    domain == zone
        || (domain.len() > zone.len()
            && domain.ends_with(zone)
            && domain.as_bytes().get(domain.len() - zone.len() - 1) == Some(&b'.'))
}

pub(super) fn normalize_and_validate_ddns_domain_config(
    provider: &str,
    config: &mut HashMap<String, String>,
) -> Result<Option<DdnsDomainTargets>, DDNSDomainConfigError> {
    let Some(policy) = ddns_provider_domain_policy(provider) else {
        return Ok(None);
    };
    let raw = config.get("domain").map(String::as_str).unwrap_or("");
    if raw.trim().is_empty() {
        if config.contains_key("domain") {
            config.insert("domain".to_string(), String::new());
        }
        return Ok(None);
    }
    let targets = parse_ddns_domain_targets(raw)?;
    if targets.is_pair() && policy.mode == DdnsDomainTargetsMode::Single {
        return Err(DDNSDomainConfigError::PairUnsupported {
            provider: provider.to_string(),
        });
    }
    let normalized_explicit_root = if let Some(field) = policy.root_field {
        let raw_root = config.get(field).cloned().unwrap_or_default();
        if raw_root.trim().is_empty() {
            None
        } else {
            let normalized_root = normalize_ddns_target_domain(&raw_root)?;
            if normalized_root.starts_with("*.") {
                return Err(DDNSDomainConfigError::InvalidDomain { domain: raw_root });
            }
            Some(normalized_root)
        }
    } else {
        None
    };
    if let (Some(pair_root), Some(field)) = (targets.pair_root(), policy.root_field) {
        let Some(normalized_root) = normalized_explicit_root.as_deref() else {
            return Err(DDNSDomainConfigError::PairRootMissing {
                field: field.to_string(),
            });
        };
        if !ddns_domain_is_same_or_subdomain(pair_root, normalized_root) {
            return Err(DDNSDomainConfigError::PairRootMismatch {
                field: field.to_string(),
                expected: normalized_root.to_string(),
                actual: pair_root.to_string(),
            });
        }
    }
    if let (Some(field), Some(normalized_root)) = (policy.root_field, normalized_explicit_root) {
        config.insert(field.to_string(), normalized_root);
    }
    config.insert("domain".to_string(), targets.canonical());
    Ok(Some(targets))
}

pub(super) fn validated_ddns_domain_targets(
    provider: &str,
    config: &HashMap<String, String>,
) -> Result<Option<DdnsDomainTargets>, DDNSDomainConfigError> {
    let mut normalized = config.clone();
    normalize_and_validate_ddns_domain_config(provider, &mut normalized)
}

pub(super) fn ddns_domain_target_set(
    provider: &str,
    config: &HashMap<String, String>,
) -> Option<BTreeSet<String>> {
    validated_ddns_domain_targets(provider, config)
        .ok()
        .flatten()
        .map(|targets| targets.domains().into_iter().map(str::to_string).collect())
}

pub(super) fn apply_ddns_domain_targets_capability(provider: &mut Value) {
    let Some(name) = provider.get("name").and_then(Value::as_str) else {
        return;
    };
    let Some(policy) = ddns_provider_domain_policy(name) else {
        return;
    };
    let mut capability = json!({ "mode": policy.mode.as_catalog_value() });
    if let Some(root_field) = policy.root_field {
        insert_json_field(&mut capability, "rootField", json!(root_field));
    }
    let Some(object) = provider.as_object_mut() else {
        return;
    };
    let capabilities = object
        .entry("capabilities".to_string())
        .or_insert_with(|| json!({}));
    if !capabilities.is_object() {
        *capabilities = json!({});
    }
    insert_json_field(capabilities, "domainTargets", capability);
}

pub(super) fn localize_ddns_domain_config_error(
    translator: &Translator,
    error: &DDNSDomainConfigError,
) -> String {
    match error {
        DDNSDomainConfigError::InvalidDomain { domain } => ddns_text(
            translator,
            "domainTargets.invalidDomain",
            &[("domain", domain.clone())],
        ),
        DDNSDomainConfigError::TooManyDomains => {
            ddns_text(translator, "domainTargets.tooMany", &[])
        }
        DDNSDomainConfigError::InvalidPair => {
            ddns_text(translator, "domainTargets.invalidPair", &[])
        }
        DDNSDomainConfigError::MismatchedPair => {
            ddns_text(translator, "domainTargets.mismatchedPair", &[])
        }
        DDNSDomainConfigError::PairUnsupported { provider } => ddns_text(
            translator,
            "domainTargets.pairUnsupported",
            &[("provider", provider_label(Some(provider), translator))],
        ),
        DDNSDomainConfigError::PairRootMissing { field } => ddns_text(
            translator,
            "domainTargets.rootMissing",
            &[("field", ddns_domain_root_field_label(translator, field))],
        ),
        DDNSDomainConfigError::PairRootMismatch {
            field,
            expected,
            actual,
        } => ddns_text(
            translator,
            "domainTargets.rootMismatch",
            &[
                ("field", ddns_domain_root_field_label(translator, field)),
                ("expected", expected.clone()),
                ("actual", actual.clone()),
            ],
        ),
    }
}

fn ddns_domain_root_field_label(translator: &Translator, field: &str) -> String {
    match field {
        "root_domain" => ddns_catalog_text(
            translator,
            "providers.common.fields.root_domain.label",
            "Root Domain",
            &[],
        ),
        "site_name" => ddns_catalog_text(
            translator,
            "providers.esa.fields.site_name.label",
            "Site Name",
            &[],
        ),
        _ => field.to_string(),
    }
}
