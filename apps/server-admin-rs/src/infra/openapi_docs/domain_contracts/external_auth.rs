use std::collections::BTreeMap;

use serde::Serialize;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProviderCatalogItemData {
    #[serde(rename = "type")]
    provider_type: String,
    protocol: String,
    label: String,
    description: String,
    default_name: String,
    default_scopes: Vec<String>,
    required_fields: Vec<String>,
    optional_fields: Vec<String>,
    supports_pkce: bool,
    supports_discovery: bool,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProviderCatalogData {
    providers: Vec<OidcProviderCatalogItemData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcConnectionConfigInputData {
    client_id: Option<String>,
    client_secret: Option<String>,
    issuer: Option<String>,
    tenant: Option<String>,
    authorization_endpoint: Option<String>,
    token_endpoint: Option<String>,
    userinfo_endpoint: Option<String>,
    jwks_uri: Option<String>,
    emails_endpoint: Option<String>,
    scopes: Option<Vec<String>>,
    extra_auth_params: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcConnectionConfigMaskedData {
    client_id: Option<String>,
    client_secret: Option<String>,
    issuer: Option<String>,
    tenant: Option<String>,
    authorization_endpoint: Option<String>,
    token_endpoint: Option<String>,
    userinfo_endpoint: Option<String>,
    jwks_uri: Option<String>,
    emails_endpoint: Option<String>,
    scopes: Option<Vec<String>>,
    extra_auth_params: Option<BTreeMap<String, String>>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProviderCreateData {
    #[serde(rename = "type")]
    provider_type: String,
    name: Option<String>,
    enabled: Option<bool>,
    connection_config: Option<OidcConnectionConfigInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProviderUpdateData {
    name: Option<String>,
    enabled: Option<bool>,
    connection_config: Option<OidcConnectionConfigInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProviderData {
    id: String,
    #[serde(rename = "type")]
    provider_type: String,
    protocol: String,
    name: String,
    enabled: bool,
    connection_config_masked: OidcConnectionConfigMaskedData,
    callback_url: Option<String>,
    created_at: String,
    updated_at: String,
    last_test_at: Option<String>,
    last_test_status: Option<String>,
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcProvidersData {
    providers: Vec<OidcProviderData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcBindingData {
    id: String,
    provider_id: String,
    #[serde(rename = "provider_type")]
    provider_type: String,
    provider_name: Option<String>,
    totp_id: String,
    totp_name: Option<String>,
    issuer: String,
    subject: String,
    display_name: Option<String>,
    email: Option<String>,
    email_verified: Option<bool>,
    avatar_url: Option<String>,
    created_at: String,
    updated_at: String,
    last_used_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct OidcBindingsData {
    bindings: Vec<OidcBindingData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalAuthConnectionTestData {
    success: bool,
    message: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalAuthInvitationBodyData {
    totp_id: String,
    provider_id: String,
    note: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct ExternalAuthInvitationData {
    invite_url: String,
    expires_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderCatalogDefaultsData {
    transport: String,
    bind_mode: String,
    user_filter: String,
    subject_attribute: String,
    username_attribute: String,
    display_name_attribute: String,
    email_attribute: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderCatalogItemData {
    #[serde(rename = "type")]
    provider_type: String,
    label: String,
    defaults: LdapProviderCatalogDefaultsData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderCatalogData {
    providers: Vec<LdapProviderCatalogItemData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapConnectionConfigInputData {
    servers: Option<Vec<String>>,
    transport: Option<String>,
    bind_mode: Option<String>,
    base_dn: Option<String>,
    user_filter: Option<String>,
    service_bind_dn: Option<String>,
    service_bind_password: Option<String>,
    direct_bind_template: Option<String>,
    subject_attribute: Option<String>,
    username_attribute: Option<String>,
    display_name_attribute: Option<String>,
    email_attribute: Option<String>,
    ca_pem: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapConnectionConfigMaskedData {
    servers: Vec<String>,
    transport: String,
    bind_mode: String,
    base_dn: String,
    user_filter: String,
    service_bind_dn: String,
    service_bind_password: String,
    direct_bind_template: String,
    subject_attribute: String,
    username_attribute: String,
    display_name_attribute: String,
    email_attribute: String,
    ca_pem: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderCreateData {
    #[serde(rename = "type")]
    provider_type: String,
    name: Option<String>,
    enabled: Option<bool>,
    connection_config: Option<LdapConnectionConfigInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderUpdateData {
    name: Option<String>,
    enabled: Option<bool>,
    connection_config: Option<LdapConnectionConfigInputData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderData {
    id: String,
    #[serde(rename = "type")]
    provider_type: String,
    protocol: String,
    name: String,
    enabled: bool,
    connection_config: LdapConnectionConfigMaskedData,
    created_at: String,
    updated_at: String,
    last_test_at: Option<String>,
    last_test_status: Option<String>,
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProvidersData {
    providers: Vec<LdapProviderData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapProviderTestBodyData {
    username: Option<String>,
    password: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapBindingData {
    id: String,
    provider_id: String,
    #[serde(rename = "provider_type")]
    provider_type: String,
    provider_name: Option<String>,
    totp_id: String,
    subject: String,
    dn: String,
    username: String,
    display_name: Option<String>,
    email: Option<String>,
    created_at: String,
    updated_at: String,
    last_used_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct LdapBindingsData {
    bindings: Vec<LdapBindingData>,
}
