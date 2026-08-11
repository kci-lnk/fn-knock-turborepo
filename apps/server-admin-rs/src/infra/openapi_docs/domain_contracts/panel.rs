use serde::Serialize;
use utoipa::ToSchema;

use super::LocaleConfigData;

#[derive(Serialize, ToSchema)]
pub(super) struct PanelAppearanceData {
    theme_color_preset: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct PanelBootstrapData {
    deployment_target: String,
    enabled: bool,
    password_configured: bool,
    authenticated: bool,
    #[schema(required = true)]
    auth_source: Option<String>,
    #[schema(required = true)]
    session_expires_at: Option<String>,
    locale: LocaleConfigData,
    appearance: PanelAppearanceData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct PanelPasswordBodyData {
    password: String,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PanelLoginBodyData {
    password: String,
    #[schema(nullable = false)]
    remember_me: Option<bool>,
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct PanelLoginRateLimitErrorData {
    success: bool,
    message: String,
    retry_after: i64,
    blocked_until: i64,
}
