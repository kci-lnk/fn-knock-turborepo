use std::time::Duration;

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, Scope, SearchEntry, SearchOptions};
use native_tls::TlsConnector;
use serde_json::Value;

use crate::crypto_utils;

#[cfg(test)]
use super::provider::split_pem_certificates;
use super::provider::{LdapConnectionConfig, custom_ca_certificates, provider_config};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, Debug)]
pub(super) struct LdapProfile {
    pub dn: String,
    pub subject: String,
    pub subject_key: String,
    pub username: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum LdapAuthError {
    #[error("Invalid directory credentials")]
    InvalidCredentials,
    #[error("Directory user was not found or was not unique")]
    UserNotFound,
    #[error("LDAP provider configuration error: {0}")]
    Configuration(String),
    #[error("LDAP service is unavailable: {0}")]
    Unavailable(String),
}

impl LdapAuthError {
    pub(super) fn is_authentication_failure(&self) -> bool {
        matches!(self, Self::InvalidCredentials | Self::UserNotFound)
    }
}

pub(super) async fn authenticate(
    provider: &Value,
    username: &str,
    password: &str,
) -> Result<LdapProfile, LdapAuthError> {
    if username.trim().is_empty() || password.is_empty() {
        return Err(LdapAuthError::InvalidCredentials);
    }
    let config = provider_config(provider).map_err(LdapAuthError::Configuration)?;
    let mut last_unavailable = None;
    for server in &config.servers {
        match authenticate_at(server, &config, username.trim(), password).await {
            Ok(profile) => {
                let provider_id = provider.get("id").and_then(Value::as_str).unwrap_or("");
                return Ok(LdapProfile {
                    subject_key: crypto_utils::sha256_hex_str(&format!(
                        "{provider_id}\0{}",
                        profile.subject
                    )),
                    ..profile
                });
            }
            Err(LdapAuthError::Unavailable(message)) => last_unavailable = Some(message),
            Err(error) => return Err(error),
        }
    }
    Err(LdapAuthError::Unavailable(
        last_unavailable.unwrap_or_else(|| "No LDAP server is configured".into()),
    ))
}

pub(super) async fn test_connection(provider: &Value) -> Result<String, LdapAuthError> {
    let config = provider_config(provider).map_err(LdapAuthError::Configuration)?;
    let mut last_unavailable = None;
    for server in &config.servers {
        match connect(server, &config).await {
            Ok(mut ldap) => {
                if config.bind_mode == "search" {
                    bind(
                        &mut ldap,
                        &config.service_bind_dn,
                        &config.service_bind_password,
                        false,
                    )
                    .await?;
                }
                let _ = ldap.unbind().await;
                return Ok(server.clone());
            }
            Err(LdapAuthError::Unavailable(message)) => last_unavailable = Some(message),
            Err(error) => return Err(error),
        }
    }
    Err(LdapAuthError::Unavailable(
        last_unavailable.unwrap_or_else(|| "No LDAP server is configured".into()),
    ))
}

async fn authenticate_at(
    server: &str,
    config: &LdapConnectionConfig,
    username: &str,
    password: &str,
) -> Result<LdapProfile, LdapAuthError> {
    if config.bind_mode == "search" {
        authenticate_search_bind(server, config, username, password).await
    } else {
        authenticate_direct_bind(server, config, username, password).await
    }
}

async fn authenticate_search_bind(
    server: &str,
    config: &LdapConnectionConfig,
    username: &str,
    password: &str,
) -> Result<LdapProfile, LdapAuthError> {
    let mut search_ldap = connect(server, config).await?;
    bind(
        &mut search_ldap,
        &config.service_bind_dn,
        &config.service_bind_password,
        false,
    )
    .await?;
    let profile = search_profile(&mut search_ldap, config, username).await?;
    let _ = search_ldap.unbind().await;

    let mut user_ldap = connect(server, config).await?;
    bind(&mut user_ldap, &profile.dn, password, true).await?;
    let _ = user_ldap.unbind().await;
    Ok(profile)
}

async fn authenticate_direct_bind(
    server: &str,
    config: &LdapConnectionConfig,
    username: &str,
    password: &str,
) -> Result<LdapProfile, LdapAuthError> {
    let escaped_username = if direct_template_is_dn(&config.direct_bind_template) {
        ldap3::dn_escape(username).into_owned()
    } else {
        username.to_string()
    };
    let principal = config
        .direct_bind_template
        .replace("{username}", &escaped_username);
    let mut ldap = connect(server, config).await?;
    bind(&mut ldap, &principal, password, true).await?;
    let profile = search_profile(&mut ldap, config, username).await?;
    let _ = ldap.unbind().await;
    Ok(profile)
}

async fn connect(server: &str, config: &LdapConnectionConfig) -> Result<Ldap, LdapAuthError> {
    let connector = tls_connector(&config.ca_pem)?;
    let settings = LdapConnSettings::new()
        .set_conn_timeout(CONNECT_TIMEOUT)
        .set_starttls(config.transport == "starttls")
        .set_connector(connector);
    let (connection, mut ldap) = LdapConnAsync::with_settings(settings, server)
        .await
        .map_err(classify_client_error)?;
    ldap3::drive!(connection);
    ldap.timeout = Some(OPERATION_TIMEOUT);
    Ok(ldap)
}

fn tls_connector(ca_pem: &str) -> Result<TlsConnector, LdapAuthError> {
    let mut builder = TlsConnector::builder();
    for certificate in custom_ca_certificates(ca_pem).map_err(LdapAuthError::Configuration)? {
        builder.add_root_certificate(certificate);
    }
    builder
        .build()
        .map_err(|error| LdapAuthError::Configuration(error.to_string()))
}

async fn bind(
    ldap: &mut Ldap,
    principal: &str,
    password: &str,
    user_bind: bool,
) -> Result<(), LdapAuthError> {
    let result = ldap
        .simple_bind(principal, password)
        .await
        .map_err(classify_client_error)?;
    if result.rc == 0 {
        Ok(())
    } else if user_bind && matches!(result.rc, 32 | 49) {
        Err(LdapAuthError::InvalidCredentials)
    } else {
        let bind_kind = if user_bind { "user" } else { "service" };
        Err(LdapAuthError::Configuration(format!(
            "{bind_kind} bind failed with LDAP result {}",
            result.rc
        )))
    }
}

async fn search_profile(
    ldap: &mut Ldap,
    config: &LdapConnectionConfig,
    username: &str,
) -> Result<LdapProfile, LdapAuthError> {
    let escaped = ldap3::ldap_escape(username);
    let filter = config.user_filter.replace("{username}", escaped.as_ref());
    let attributes = vec![
        config.subject_attribute.as_str(),
        config.username_attribute.as_str(),
        config.display_name_attribute.as_str(),
        config.email_attribute.as_str(),
    ];
    let ldap3::SearchResult(entries, result) = ldap
        .with_search_options(SearchOptions::new().sizelimit(2).timelimit(5))
        .search(&config.base_dn, Scope::Subtree, &filter, attributes)
        .await
        .map_err(classify_client_error)?;
    if result.rc == 4 && entries.len() >= 2 {
        return Err(LdapAuthError::UserNotFound);
    }
    if result.rc != 0 {
        return Err(LdapAuthError::Configuration(format!(
            "user search failed with LDAP result {}",
            result.rc
        )));
    }
    if entries.len() != 1 {
        return Err(LdapAuthError::UserNotFound);
    }
    profile_from_entry(
        SearchEntry::construct(entries.into_iter().next().unwrap()),
        config,
    )
}

fn classify_client_error(error: ldap3::LdapError) -> LdapAuthError {
    let message = error.to_string();
    match error {
        ldap3::LdapError::Io { .. }
        | ldap3::LdapError::OpSend { .. }
        | ldap3::LdapError::ResultRecv { .. }
        | ldap3::LdapError::IdScrubSend { .. }
        | ldap3::LdapError::MiscSend { .. }
        | ldap3::LdapError::Timeout { .. }
        | ldap3::LdapError::EndOfStream
        | ldap3::LdapError::NativeTLS { .. } => LdapAuthError::Unavailable(message),
        _ => LdapAuthError::Configuration(message),
    }
}

fn profile_from_entry(
    entry: SearchEntry,
    config: &LdapConnectionConfig,
) -> Result<LdapProfile, LdapAuthError> {
    let subject = entry
        .bin_attrs
        .get(&config.subject_attribute)
        .or_else(|| {
            entry
                .bin_attrs
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(&config.subject_attribute))
                .map(|(_, value)| value)
        })
        .and_then(|values| values.first())
        .map(|value| URL_SAFE_NO_PAD.encode(value))
        .or_else(|| first_attr(&entry, &config.subject_attribute))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            LdapAuthError::Configuration(format!(
                "LDAP subject attribute {} is missing",
                config.subject_attribute
            ))
        })?;
    let username = first_attr(&entry, &config.username_attribute).ok_or_else(|| {
        LdapAuthError::Configuration(format!(
            "LDAP username attribute {} is missing",
            config.username_attribute
        ))
    })?;
    let display_name = first_attr(&entry, &config.display_name_attribute);
    let email = first_attr(&entry, &config.email_attribute);
    Ok(LdapProfile {
        dn: entry.dn,
        subject,
        subject_key: String::new(),
        username,
        display_name,
        email,
    })
}

fn first_attr(entry: &SearchEntry, name: &str) -> Option<String> {
    entry
        .attrs
        .get(name)
        .or_else(|| {
            entry
                .attrs
                .iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, value)| value)
        })
        .and_then(|values| values.first())
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn direct_template_is_dn(template: &str) -> bool {
    template
        .split(',')
        .any(|component| component.trim().split_once('=').is_some())
}

#[cfg(test)]
pub(super) fn split_pem_certificates_for_test(value: &str) -> Vec<String> {
    split_pem_certificates(value)
}

#[cfg(test)]
pub(super) fn binary_subject_for_test(value: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(value)
}

#[cfg(test)]
pub(super) fn direct_principal_for_test(template: &str, username: &str) -> String {
    let escaped = if direct_template_is_dn(template) {
        ldap3::dn_escape(username).into_owned()
    } else {
        username.to_string()
    };
    template.replace("{username}", &escaped)
}

#[cfg(test)]
pub(super) fn client_error_is_unavailable_for_test(error: ldap3::LdapError) -> bool {
    matches!(classify_client_error(error), LdapAuthError::Unavailable(_))
}
