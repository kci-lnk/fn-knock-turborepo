use native_tls::Certificate;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use url::Url;

use crate::{crypto_utils, i18n::Translator, time_utils};

const MASKED_SECRET: &str = "********";

#[derive(Clone, Deserialize, Serialize)]
pub(super) struct LdapConnectionConfig {
    pub servers: Vec<String>,
    pub transport: String,
    pub bind_mode: String,
    pub base_dn: String,
    pub user_filter: String,
    #[serde(default)]
    pub service_bind_dn: String,
    #[serde(default)]
    pub service_bind_password: String,
    #[serde(default)]
    pub direct_bind_template: String,
    pub subject_attribute: String,
    pub username_attribute: String,
    pub display_name_attribute: String,
    pub email_attribute: String,
    #[serde(default)]
    pub ca_pem: String,
}

#[derive(Clone, Copy)]
struct Preset {
    kind: &'static str,
    label: &'static str,
    label_key: &'static str,
    user_filter: &'static str,
    subject_attribute: &'static str,
    username_attribute: &'static str,
    display_name_attribute: &'static str,
    email_attribute: &'static str,
}

const PRESETS: [Preset; 3] = [
    Preset {
        kind: "openldap",
        label: "OpenLDAP",
        label_key: "openldapLabel",
        user_filter: "(&(objectClass=person)(uid={username}))",
        subject_attribute: "entryUUID",
        username_attribute: "uid",
        display_name_attribute: "cn",
        email_attribute: "mail",
    },
    Preset {
        kind: "active_directory",
        label: "Active Directory",
        label_key: "activeDirectoryLabel",
        user_filter: "(&(objectCategory=person)(objectClass=user)(|(userPrincipalName={username})(sAMAccountName={username})))",
        subject_attribute: "objectGUID",
        username_attribute: "userPrincipalName",
        display_name_attribute: "displayName",
        email_attribute: "mail",
    },
    Preset {
        kind: "custom",
        label: "Custom LDAP",
        label_key: "customLabel",
        user_filter: "(uid={username})",
        subject_attribute: "entryUUID",
        username_attribute: "uid",
        display_name_attribute: "cn",
        email_attribute: "mail",
    },
];

pub(super) fn catalog(translator: &Translator) -> Value {
    Value::Array(
        PRESETS
            .iter()
            .map(|preset| {
                json!({
                    "type": preset.kind,
                    "label": translator.t(&format!("server.ldap.catalog.{}", preset.label_key)),
                    "defaults": {
                        "transport": "ldaps",
                        "bind_mode": "search",
                        "user_filter": preset.user_filter,
                        "subject_attribute": preset.subject_attribute,
                        "username_attribute": preset.username_attribute,
                        "display_name_attribute": preset.display_name_attribute,
                        "email_attribute": preset.email_attribute,
                    }
                })
            })
            .collect(),
    )
}

pub(super) fn build_new_provider(input: &Map<String, Value>) -> Result<Value, String> {
    let provider_type = normalized_string(input.get("type")).unwrap_or_else(|| "custom".into());
    let preset =
        preset(&provider_type).ok_or_else(|| "Unsupported LDAP provider type".to_string())?;
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let config = normalize_connection_config(
        input
            .get("connection_config")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        preset,
        !enabled,
        None,
    )?;
    let now = time_utils::now_iso();
    Ok(json!({
        "id": format!("ldap_provider_{}", &crypto_utils::sha256_hex_str(&format!("{}:{}", now, uuid::Uuid::new_v4()))[..24]),
        "type": provider_type,
        "protocol": "ldap",
        "name": normalized_string(input.get("name")).unwrap_or_else(|| preset.label.to_string()),
        "enabled": enabled,
        "connection_config": config,
        "created_at": now,
        "updated_at": now,
        "last_test_status": "idle",
    }))
}

pub(super) fn build_updated_provider(
    mut provider: Value,
    input: &Map<String, Value>,
) -> Result<Value, String> {
    let object = provider
        .as_object_mut()
        .ok_or_else(|| "Stored LDAP provider is invalid".to_string())?;
    let provider_type = object
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("custom")
        .to_string();
    let preset =
        preset(&provider_type).ok_or_else(|| "Unsupported LDAP provider type".to_string())?;
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            object
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        });
    let existing = object
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut merged = existing.clone();
    if let Some(patch) = input.get("connection_config").and_then(Value::as_object) {
        for (key, value) in patch {
            let preserve_secret = key == "service_bind_password"
                && value
                    .as_str()
                    .is_some_and(|value| value.trim().is_empty() || value == MASKED_SECRET);
            if !preserve_secret {
                merged.insert(key.clone(), value.clone());
            }
        }
    }
    let config = normalize_connection_config(merged, preset, !enabled, Some(&existing))?;
    if let Some(name) = normalized_string(input.get("name")) {
        object.insert("name".into(), Value::String(name));
    }
    object.insert("enabled".into(), Value::Bool(enabled));
    object.insert("connection_config".into(), config);
    object.insert("updated_at".into(), Value::String(time_utils::now_iso()));
    Ok(provider)
}

fn normalize_connection_config(
    raw: Map<String, Value>,
    preset: Preset,
    allow_incomplete: bool,
    _existing: Option<&Map<String, Value>>,
) -> Result<Value, String> {
    let transport = normalized_string(raw.get("transport")).unwrap_or_else(|| "ldaps".into());
    if !matches!(transport.as_str(), "ldaps" | "starttls") {
        return Err("LDAP transport must be ldaps or starttls".into());
    }
    let bind_mode = normalized_string(raw.get("bind_mode")).unwrap_or_else(|| "search".into());
    if !matches!(bind_mode.as_str(), "search" | "direct") {
        return Err("LDAP bind mode must be search or direct".into());
    }
    let servers = normalize_servers(raw.get("servers"), &transport)?;
    let base_dn = normalized_string(raw.get("base_dn")).unwrap_or_default();
    let user_filter =
        normalized_string(raw.get("user_filter")).unwrap_or_else(|| preset.user_filter.to_string());
    if !user_filter.contains("{username}") {
        return Err("LDAP user filter must contain {username}".into());
    }
    let service_bind_dn = normalized_string(raw.get("service_bind_dn")).unwrap_or_default();
    let service_bind_password = raw
        .get("service_bind_password")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let direct_bind_template =
        normalized_string(raw.get("direct_bind_template")).unwrap_or_default();
    if bind_mode == "direct"
        && !direct_bind_template.is_empty()
        && !direct_bind_template.contains("{username}")
    {
        return Err("LDAP direct bind template must contain {username}".into());
    }
    if !allow_incomplete {
        if servers.is_empty() || base_dn.is_empty() {
            return Err("LDAP servers and Base DN are required".into());
        }
        if bind_mode == "search" && (service_bind_dn.is_empty() || service_bind_password.is_empty())
        {
            return Err("LDAP search bind credentials are required".into());
        }
        if bind_mode == "direct" && direct_bind_template.is_empty() {
            return Err("LDAP direct bind template is required".into());
        }
    }
    let ca_pem = raw
        .get("ca_pem")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    validate_ca_pem(&ca_pem)?;
    Ok(json!({
        "servers": servers,
        "transport": transport,
        "bind_mode": bind_mode,
        "base_dn": base_dn,
        "user_filter": user_filter,
        "service_bind_dn": service_bind_dn,
        "service_bind_password": service_bind_password,
        "direct_bind_template": direct_bind_template,
        "subject_attribute": normalized_string(raw.get("subject_attribute")).unwrap_or_else(|| preset.subject_attribute.into()),
        "username_attribute": normalized_string(raw.get("username_attribute")).unwrap_or_else(|| preset.username_attribute.into()),
        "display_name_attribute": normalized_string(raw.get("display_name_attribute")).unwrap_or_else(|| preset.display_name_attribute.into()),
        "email_attribute": normalized_string(raw.get("email_attribute")).unwrap_or_else(|| preset.email_attribute.into()),
        "ca_pem": ca_pem,
    }))
}

fn normalize_servers(value: Option<&Value>, transport: &str) -> Result<Vec<String>, String> {
    let entries = match value {
        Some(Value::Array(items)) => items.iter().filter_map(normalized_value_string).collect(),
        Some(Value::String(value)) => value
            .lines()
            .filter_map(|line| normalized_value_string(&Value::String(line.into())))
            .collect(),
        _ => Vec::new(),
    };
    entries
        .into_iter()
        .map(|entry| normalize_server(&entry, transport))
        .collect()
}

fn normalize_server(entry: &str, transport: &str) -> Result<String, String> {
    let scheme = if transport == "ldaps" {
        "ldaps"
    } else {
        "ldap"
    };
    let candidate = if entry.contains("://") {
        entry.to_string()
    } else {
        format!("{scheme}://{entry}")
    };
    let url = Url::parse(&candidate).map_err(|_| format!("Invalid LDAP server URL: {entry}"))?;
    if url.scheme() != scheme
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/")
    {
        return Err(format!("Invalid or insecure LDAP server URL: {entry}"));
    }
    Ok(url.to_string().trim_end_matches('/').to_string())
}

pub(super) fn provider_config(provider: &Value) -> Result<LdapConnectionConfig, String> {
    let config: LdapConnectionConfig = serde_json::from_value(
        provider
            .get("connection_config")
            .cloned()
            .ok_or_else(|| "LDAP connection config is missing".to_string())?,
    )
    .map_err(|error| format!("Invalid LDAP connection config: {error}"))?;
    validate_runtime_config(&config)?;
    Ok(config)
}

pub(super) fn provider_ready(provider: &Value) -> bool {
    provider_config(provider).is_ok()
}

fn validate_runtime_config(config: &LdapConnectionConfig) -> Result<(), String> {
    if !matches!(config.transport.as_str(), "ldaps" | "starttls") {
        return Err("LDAP transport must be ldaps or starttls".into());
    }
    if !matches!(config.bind_mode.as_str(), "search" | "direct") {
        return Err("LDAP bind mode must be search or direct".into());
    }
    if config.servers.is_empty() || config.base_dn.trim().is_empty() {
        return Err("LDAP servers and Base DN are required".into());
    }
    for server in &config.servers {
        normalize_server(server, &config.transport)?;
    }
    if !config.user_filter.contains("{username}") {
        return Err("LDAP user filter must contain {username}".into());
    }
    for (label, attribute) in [
        ("subject", &config.subject_attribute),
        ("username", &config.username_attribute),
        ("display name", &config.display_name_attribute),
        ("email", &config.email_attribute),
    ] {
        if attribute.trim().is_empty() {
            return Err(format!("LDAP {label} attribute is required"));
        }
    }
    if config.bind_mode == "search"
        && (config.service_bind_dn.trim().is_empty() || config.service_bind_password.is_empty())
    {
        return Err("LDAP search bind credentials are required".into());
    }
    if config.bind_mode == "direct"
        && (config.direct_bind_template.trim().is_empty()
            || !config.direct_bind_template.contains("{username}"))
    {
        return Err("LDAP direct bind template must contain {username}".into());
    }
    validate_ca_pem(&config.ca_pem)?;
    Ok(())
}

fn validate_ca_pem(value: &str) -> Result<(), String> {
    custom_ca_certificates(value).map(|_| ())
}

pub(super) fn custom_ca_certificates(value: &str) -> Result<Vec<Certificate>, String> {
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    let certificates = split_pem_certificates(value);
    if certificates.is_empty() {
        return Err("The custom CA PEM does not contain a certificate".into());
    }
    certificates
        .into_iter()
        .map(|pem| Certificate::from_pem(pem.as_bytes()).map_err(|error| error.to_string()))
        .collect()
}

pub(super) fn split_pem_certificates(value: &str) -> Vec<String> {
    const END: &str = "-----END CERTIFICATE-----";
    value
        .split_inclusive(END)
        .map(str::trim)
        .filter(|item| item.starts_with("-----BEGIN CERTIFICATE-----") && item.ends_with(END))
        .map(str::to_string)
        .collect()
}

pub(super) fn mask_provider(mut provider: Value) -> Value {
    if let Some(config) = provider
        .get_mut("connection_config")
        .and_then(Value::as_object_mut)
        && config
            .get("service_bind_password")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.is_empty())
    {
        config.insert(
            "service_bind_password".into(),
            Value::String(MASKED_SECRET.into()),
        );
    }
    provider
}

fn preset(kind: &str) -> Option<Preset> {
    PRESETS.iter().copied().find(|preset| preset.kind == kind)
}

fn normalized_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn normalized_value_string(value: &Value) -> Option<String> {
    normalized_string(Some(value))
}

#[cfg(test)]
pub(super) fn normalize_server_for_test(entry: &str, transport: &str) -> Result<String, String> {
    normalize_server(entry, transport)
}
