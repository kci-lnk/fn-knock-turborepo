use axum::http::StatusCode;

use crate::i18n::Translator;

use super::TotpImportRouteError;

pub(super) fn admin_control_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.{key}"))
}

pub(super) fn admin_control_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.{key}"), params)
}

pub(super) fn totp_import_error(status: StatusCode, key: &'static str) -> TotpImportRouteError {
    TotpImportRouteError {
        status,
        key,
        max: None,
    }
}

pub(super) fn totp_import_error_with_max(
    status: StatusCode,
    key: &'static str,
    max: usize,
) -> TotpImportRouteError {
    TotpImportRouteError {
        status,
        key,
        max: Some(max),
    }
}

pub(super) fn totp_import_error_message(
    translator: &Translator,
    error: &TotpImportRouteError,
) -> String {
    let key = format!("totpImport.{}", error.key);
    if let Some(max) = error.max {
        admin_control_text_params(translator, &key, &[("max", max.to_string())])
    } else {
        admin_control_text(translator, &key)
    }
}
