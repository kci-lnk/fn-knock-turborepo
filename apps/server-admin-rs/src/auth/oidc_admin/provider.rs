use serde_json::{Map, Value, json};
use url::Url;

use crate::{i18n::Translator, time_utils};

use super::{
    text::{oidc_text, oidc_text_params},
    tokens::create_oidc_id,
};

pub(crate) fn oidc_provider_ready_with_translator(
    provider: &Value,
    translator: &Translator,
) -> Result<(), String> {
    let missing = missing_required_provider_fields(provider);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(oidc_text_params(
            translator,
            "providerMissingRequiredFields",
            &[("fields", missing.join(", "))],
        ))
    }
}

pub(super) fn build_new_provider(
    input: &Map<String, Value>,
    translator: &Translator,
) -> Result<Value, String> {
    let provider_type = normalize_string(input.get("type"))
        .ok_or_else(|| oidc_text(translator, "providerTypeRequired"))?;
    let definition = provider_definition(&provider_type)
        .ok_or_else(|| oidc_text(translator, "providerUnsupported"))?;
    let now = time_utils::now_iso();
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let connection_config = normalize_connection_config(
        &provider_type,
        input
            .get("connection_config")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        !enabled,
        translator,
    )?;
    Ok(json!({
        "id": create_oidc_id("oidc_provider"),
        "type": provider_type,
        "protocol": definition.protocol,
        "name": normalize_string(input.get("name")).unwrap_or_else(|| provider_default_name(&definition, translator)),
        "enabled": enabled,
        "connection_config": connection_config,
        "created_at": now,
        "updated_at": now,
        "last_test_status": "idle",
    }))
}

pub(super) fn missing_required_provider_fields(provider: &Value) -> Vec<&'static str> {
    let Some(provider_type) = provider.get("type").and_then(Value::as_str) else {
        return vec!["type"];
    };
    let Some(definition) = provider_definition(provider_type) else {
        return vec!["type"];
    };
    let config = provider.get("connection_config").and_then(Value::as_object);
    definition
        .required_fields
        .iter()
        .filter(|field| !connection_value_present(config.and_then(|config| config.get(**field))))
        .copied()
        .collect()
}

pub(super) fn build_updated_provider(
    mut provider: Value,
    input: &Map<String, Value>,
    translator: &Translator,
) -> Result<Value, String> {
    let Some(object) = provider.as_object_mut() else {
        return Err(oidc_text(translator, "storedProviderInvalid"));
    };
    let provider_type = object
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| oidc_text(translator, "storedProviderTypeInvalid"))?
        .to_string();
    let enabled = input
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or_else(|| {
            object
                .get("enabled")
                .and_then(Value::as_bool)
                .unwrap_or(true)
        });
    let mut connection = object
        .get("connection_config")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    if let Some(patch) = input.get("connection_config").and_then(Value::as_object) {
        for (key, value) in patch {
            connection.insert(key.clone(), value.clone());
        }
    }
    let normalized_connection =
        normalize_connection_config(&provider_type, connection, !enabled, translator)?;
    if let Some(name) = normalize_string(input.get("name")) {
        object.insert("name".to_string(), Value::String(name));
    }
    object.insert("enabled".to_string(), Value::Bool(enabled));
    object.insert("connection_config".to_string(), normalized_connection);
    object.insert(
        "updated_at".to_string(),
        Value::String(time_utils::now_iso()),
    );
    Ok(provider)
}

pub(super) fn normalize_connection_config(
    provider_type: &str,
    raw: Map<String, Value>,
    allow_incomplete: bool,
    translator: &Translator,
) -> Result<Value, String> {
    let definition = provider_definition(provider_type)
        .ok_or_else(|| oidc_text(translator, "providerUnsupported"))?;
    let defaults = default_connection_config(provider_type);
    let tenant = normalize_string(raw.get("tenant")).or_else(|| {
        defaults
            .get("tenant")
            .and_then(Value::as_str)
            .map(ToString::to_string)
    });
    let issuer = normalize_string(raw.get("issuer")).or_else(|| {
        if provider_type == "microsoft" {
            tenant
                .as_ref()
                .map(|tenant| format!("https://login.microsoftonline.com/{tenant}/v2.0"))
        } else {
            defaults
                .get("issuer")
                .and_then(Value::as_str)
                .map(ToString::to_string)
        }
    });
    let mut config = Map::new();
    insert_string(
        &mut config,
        "client_id",
        normalize_string(raw.get("client_id"))
            .or_else(|| {
                defaults
                    .get("client_id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or_default(),
    );
    insert_string(
        &mut config,
        "client_secret",
        normalize_string(raw.get("client_secret")).unwrap_or_default(),
    );
    insert_optional_string(&mut config, "issuer", issuer);
    insert_optional_string(&mut config, "tenant", tenant);
    for key in [
        "authorization_endpoint",
        "token_endpoint",
        "userinfo_endpoint",
        "jwks_uri",
        "emails_endpoint",
    ] {
        insert_optional_string(
            &mut config,
            key,
            normalize_string(raw.get(key)).or_else(|| {
                defaults
                    .get(key)
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            }),
        );
    }
    config.insert(
        "scopes".to_string(),
        Value::Array(
            normalize_scopes(raw.get("scopes"), definition.default_scopes)
                .into_iter()
                .map(Value::String)
                .collect(),
        ),
    );
    if let Some(extra) = normalize_extra_auth_params(raw.get("extra_auth_params"), translator)? {
        config.insert("extra_auth_params".to_string(), extra);
    }

    let missing = definition
        .required_fields
        .iter()
        .filter(|field| !connection_value_present(config.get(**field)))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() && !allow_incomplete {
        return Err(oidc_text_params(
            translator,
            "providerMissingRequiredConfig",
            &[
                ("provider", provider_label(&definition, translator)),
                ("fields", missing.join(", ")),
            ],
        ));
    }
    for key in [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "userinfo_endpoint",
        "jwks_uri",
        "emails_endpoint",
    ] {
        if let Some(value) = config
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
        {
            assert_http_url(value, key, translator)?;
        }
    }
    Ok(Value::Object(config))
}

pub(super) fn mask_provider(provider: Value, callback_origin: Option<&str>) -> Value {
    let Some(object) = provider.as_object() else {
        return provider;
    };
    let mut masked_config = Map::new();
    if let Some(config) = object.get("connection_config").and_then(Value::as_object) {
        for (key, value) in config {
            masked_config.insert(
                key.clone(),
                if key == "client_secret" {
                    Value::String(mask_sensitive_value(value))
                } else {
                    value.clone()
                },
            );
        }
    }
    let mut view = Map::new();
    for key in [
        "id",
        "type",
        "protocol",
        "name",
        "enabled",
        "created_at",
        "updated_at",
        "last_test_at",
        "last_test_status",
        "last_error",
    ] {
        if let Some(value) = object.get(key) {
            view.insert(key.to_string(), value.clone());
        }
    }
    view.insert(
        "connection_config_masked".to_string(),
        Value::Object(masked_config),
    );
    if let (Some(origin), Some(id)) = (callback_origin, object.get("id").and_then(Value::as_str)) {
        view.insert(
            "callback_url".to_string(),
            Value::String(format!(
                "{}/api/auth/oidc/callback/{}",
                origin.trim_end_matches('/'),
                crate::http_utils::url_encode_component(id)
            )),
        );
    }
    Value::Object(view)
}

pub(super) fn provider_catalog(translator: &Translator) -> Vec<Value> {
    ["fnknock_qq", "google", "microsoft", "github", "custom_oidc"]
        .into_iter()
        .filter_map(provider_definition)
        .map(|definition| {
            json!({
                "type": definition.provider_type,
                "protocol": definition.protocol,
                "label": provider_label(&definition, translator),
                "description": provider_description(&definition, translator),
                "default_name": provider_default_name(&definition, translator),
                "default_scopes": definition.default_scopes,
                "required_fields": definition.required_fields,
                "optional_fields": definition.optional_fields,
                "supports_pkce": definition.supports_pkce,
                "supports_discovery": definition.supports_discovery,
            })
        })
        .collect()
}

pub(super) fn provider_label(definition: &ProviderDefinition, translator: &Translator) -> String {
    if definition.provider_type == "custom_oidc" {
        oidc_text(translator, "catalog.customLabel")
    } else {
        definition.label.to_string()
    }
}

pub(super) fn provider_description(
    definition: &ProviderDefinition,
    translator: &Translator,
) -> String {
    let key = match definition.provider_type {
        "google" => "catalog.googleDescription",
        "microsoft" => "catalog.microsoftDescription",
        "github" => "catalog.githubDescription",
        "custom_oidc" => "catalog.customDescription",
        _ => return definition.description.to_string(),
    };
    oidc_text(translator, key)
}

pub(super) fn provider_default_name(
    definition: &ProviderDefinition,
    translator: &Translator,
) -> String {
    if definition.provider_type == "custom_oidc" {
        oidc_text(translator, "catalog.customLabel")
    } else {
        definition.default_name.to_string()
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProviderDefinition {
    provider_type: &'static str,
    protocol: &'static str,
    label: &'static str,
    description: &'static str,
    default_name: &'static str,
    default_scopes: &'static [&'static str],
    required_fields: &'static [&'static str],
    optional_fields: &'static [&'static str],
    supports_pkce: bool,
    supports_discovery: bool,
}

pub(super) fn provider_definition(provider_type: &str) -> Option<ProviderDefinition> {
    match provider_type {
        "fnknock_qq" => Some(ProviderDefinition {
            provider_type: "fnknock_qq",
            protocol: "oidc",
            label: "QQ",
            description: "Sign in with QQ through FnKnock without registering a QQ application",
            default_name: "QQ",
            default_scopes: &["openid", "profile"],
            required_fields: &["client_id", "issuer"],
            optional_fields: &["scopes"],
            supports_pkce: true,
            supports_discovery: true,
        }),
        "google" => Some(ProviderDefinition {
            provider_type: "google",
            protocol: "oidc",
            label: "Google",
            description: "Sign in with Google",
            default_name: "Google",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["issuer", "scopes", "extra_auth_params"],
            supports_pkce: true,
            supports_discovery: true,
        }),
        "microsoft" => Some(ProviderDefinition {
            provider_type: "microsoft",
            protocol: "oidc",
            label: "Microsoft",
            description: "Sign in with Microsoft",
            default_name: "Microsoft",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["tenant", "issuer", "scopes", "extra_auth_params"],
            supports_pkce: true,
            supports_discovery: true,
        }),
        "github" => Some(ProviderDefinition {
            provider_type: "github",
            protocol: "oauth2_profile",
            label: "GitHub",
            description: "Sign in with GitHub",
            default_name: "GitHub",
            default_scopes: &["read:user", "user:email"],
            required_fields: &["client_id", "client_secret"],
            optional_fields: &["scopes", "extra_auth_params"],
            supports_pkce: false,
            supports_discovery: false,
        }),
        "custom_oidc" => Some(ProviderDefinition {
            provider_type: "custom_oidc",
            protocol: "oidc",
            label: "Custom OIDC",
            description: "Sign in with a custom OpenID Connect provider",
            default_name: "Custom OIDC",
            default_scopes: &["openid", "profile", "email"],
            required_fields: &["client_id", "client_secret", "issuer"],
            optional_fields: &[
                "authorization_endpoint",
                "token_endpoint",
                "userinfo_endpoint",
                "jwks_uri",
                "scopes",
                "extra_auth_params",
            ],
            supports_pkce: true,
            supports_discovery: true,
        }),
        _ => None,
    }
}

pub(super) fn default_connection_config(provider_type: &str) -> Map<String, Value> {
    match provider_type {
        "fnknock_qq" => map_from_pairs(&[
            ("client_id", "fnknock-qq-public"),
            ("issuer", "https://api.fnknock.cn/oidc/qq"),
        ]),
        "google" => map_from_pairs(&[("issuer", "https://accounts.google.com")]),
        "microsoft" => map_from_pairs(&[
            ("tenant", "common"),
            ("issuer", "https://login.microsoftonline.com/common/v2.0"),
        ]),
        "github" => map_from_pairs(&[
            (
                "authorization_endpoint",
                "https://github.com/login/oauth/authorize",
            ),
            (
                "token_endpoint",
                "https://github.com/login/oauth/access_token",
            ),
            ("userinfo_endpoint", "https://api.github.com/user"),
            ("emails_endpoint", "https://api.github.com/user/emails"),
        ]),
        _ => Map::new(),
    }
}

pub(super) fn map_from_pairs(pairs: &[(&str, &str)]) -> Map<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), Value::String((*value).to_string())))
        .collect()
}

pub(super) fn normalize_string(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

pub(super) fn normalize_scopes(value: Option<&Value>, fallback: &[&str]) -> Vec<String> {
    let values = if let Some(items) = value.and_then(Value::as_array) {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    } else if let Some(raw) = value.and_then(Value::as_str) {
        raw.split(|ch: char| ch == ',' || ch.is_whitespace())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut seen = std::collections::HashSet::new();
    let deduped = values
        .into_iter()
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>();
    if deduped.is_empty() {
        fallback.iter().map(|value| (*value).to_string()).collect()
    } else {
        deduped
    }
}

pub(super) fn normalize_extra_auth_params(
    value: Option<&Value>,
    translator: &Translator,
) -> Result<Option<Value>, String> {
    let Some(object) = value.and_then(Value::as_object) else {
        return Ok(None);
    };
    let mut normalized = Map::new();
    for (key, value) in object {
        let key = key.trim();
        let Some(value) = normalize_string(Some(value)) else {
            continue;
        };
        if key.is_empty() {
            continue;
        }
        if reserved_extra_auth_param_key(key) {
            return Err(oidc_text_params(
                translator,
                "reservedExtraAuthParam",
                &[("key", key.to_string())],
            ));
        }
        normalized.insert(key.to_string(), Value::String(value));
    }
    if normalized.is_empty() {
        Ok(None)
    } else {
        Ok(Some(Value::Object(normalized)))
    }
}

pub(super) fn reserved_extra_auth_param_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "client_id"
            | "client_secret"
            | "response_type"
            | "redirect_uri"
            | "scope"
            | "state"
            | "nonce"
            | "code_challenge"
            | "code_challenge_method"
            | "code_verifier"
            | "grant_type"
            | "code"
    )
}

pub(super) fn connection_value_present(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(Value::Array(value)) => !value.is_empty(),
        Some(Value::Null) | None => false,
        Some(_) => true,
    }
}

pub(super) fn assert_http_url(
    value: &str,
    label: &str,
    translator: &Translator,
) -> Result<(), String> {
    let parsed = Url::parse(value)
        .map_err(|_| oidc_text_params(translator, "urlInvalid", &[("label", label.to_string())]))?;
    if parsed.scheme() != "https" && parsed.host_str() != Some("localhost") {
        return Err(oidc_text_params(
            translator,
            "urlMustUseHttps",
            &[("label", label.to_string())],
        ));
    }
    Ok(())
}

pub(super) fn insert_string(object: &mut Map<String, Value>, key: &str, value: String) {
    object.insert(key.to_string(), Value::String(value));
}

pub(super) fn insert_optional_string(
    object: &mut Map<String, Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value {
        object.insert(key.to_string(), Value::String(value));
    }
}

pub(super) fn mask_sensitive_value(value: &Value) -> String {
    let Some(value) = value.as_str() else {
        return "[configured]".to_string();
    };
    if value.is_empty() {
        String::new()
    } else if value.len() <= 8 {
        "********".to_string()
    } else {
        format!("{}******", &value[..2])
    }
}
