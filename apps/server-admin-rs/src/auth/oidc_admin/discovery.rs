use std::time::Duration;

use serde_json::{Value, json};

use crate::i18n::Translator;

use super::{
    OIDC_HTTP_USER_AGENT,
    provider::{missing_required_provider_fields, normalize_string},
    text::{oidc_text, oidc_text_params},
};

const MAX_OIDC_DISCOVERY_RESPONSE_BYTES: usize = 1024 * 1024;

pub(super) async fn run_provider_test(
    provider: &Value,
    translator: &Translator,
) -> Result<(), String> {
    let missing = missing_required_provider_fields(provider);
    if !missing.is_empty() {
        return Err(oidc_text_params(
            translator,
            "providerMissingRequiredFields",
            &[("fields", missing.join(", "))],
        ));
    }
    if provider.get("protocol").and_then(Value::as_str) == Some("oidc") {
        resolve_discovery_with_translator(provider, translator).await?;
        return Ok(());
    }
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))?;
    for key in ["authorization_endpoint", "token_endpoint"] {
        if normalize_string(config.get(key)).is_none() {
            return Err(oidc_text_params(
                translator,
                "oauthEndpointIncompleteWithField",
                &[("field", key.to_string())],
            ));
        }
    }
    Ok(())
}

pub(crate) async fn resolve_discovery_with_translator(
    provider: &Value,
    translator: &Translator,
) -> Result<Value, String> {
    let config = provider
        .get("connection_config")
        .and_then(Value::as_object)
        .ok_or_else(|| oidc_text(translator, "connectionConfigInvalid"))?;
    let direct = [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
    ]
    .into_iter()
    .all(|key| normalize_string(config.get(key)).is_some());
    if direct {
        return Ok(json!({
            "issuer": normalize_string(config.get("issuer")).unwrap_or_default(),
            "authorization_endpoint": normalize_string(config.get("authorization_endpoint")).unwrap_or_default(),
            "token_endpoint": normalize_string(config.get("token_endpoint")).unwrap_or_default(),
            "userinfo_endpoint": normalize_string(config.get("userinfo_endpoint")),
            "jwks_uri": normalize_string(config.get("jwks_uri")).unwrap_or_default(),
        }));
    }

    let issuer = normalize_string(config.get("issuer"))
        .ok_or_else(|| oidc_text(translator, "issuerMissing"))?;
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        issuer.trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(7))
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(&discovery_url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::USER_AGENT, OIDC_HTTP_USER_AGENT)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    let text =
        crate::http_body::read_response_text_limited(response, MAX_OIDC_DISCOVERY_RESPONSE_BYTES)
            .await
            .map_err(|error| error.to_string())?;
    if !status.is_success() {
        return Err(oidc_text_params(
            translator,
            "discoveryHttpFailed",
            &[
                ("status", status.as_u16().to_string()),
                ("detail", text.chars().take(160).collect::<String>()),
            ],
        ));
    }
    let payload = serde_json::from_str::<Value>(&text).map_err(|error| error.to_string())?;
    let Some(object) = payload.as_object() else {
        return Err(oidc_text(translator, "discoveryInvalid"));
    };
    let missing = [
        "issuer",
        "authorization_endpoint",
        "token_endpoint",
        "jwks_uri",
    ]
    .into_iter()
    .filter(|key| normalize_string(object.get(*key)).is_none())
    .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(oidc_text_params(
            translator,
            "discoveryMissingFieldsWithList",
            &[("fields", missing.join(", "))],
        ));
    }
    Ok(payload)
}
