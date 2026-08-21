use std::{
    collections::BTreeSet,
    io::{Cursor, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use ::time::UtcOffset;
use anyhow::anyhow;
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path as AxumPath, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;
use x509_parser::{
    extensions::GeneralName, parse_x509_certificate, pem::parse_x509_pem, time::ASN1Time,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    response,
    state::AppState,
    time_utils,
};

mod ca;
mod deployment;
mod external;
mod handlers;
mod lan;
mod library;
mod normalize;
mod recommendation;
mod shared_files;
mod status;

use ca::*;
pub(crate) use deployment::*;
use external::{
    __path_create_external_certificate_binding, __path_delete_external_certificate_binding,
    __path_list_external_certificate_bindings, __path_rotate_external_certificate_binding_token,
    __path_update_external_certificate_binding, create_external_certificate_binding,
    delete_external_certificate_binding, list_external_certificate_bindings,
    rotate_external_certificate_binding_token, update_external_certificate_binding,
};
pub(crate) use external::{
    external_certificate_openapi_routes, external_certificate_routes,
    public_external_certificate_openapi_routes, public_external_certificate_routes,
};
use handlers::*;
use handlers::{
    __path_activate_certificate, __path_active_cert_pem, __path_active_cert_zip,
    __path_add_ca_host, __path_ca_cert_pem, __path_ca_clear, __path_ca_hosts, __path_ca_init,
    __path_ca_issue, __path_ca_server_cert_zip, __path_ca_status, __path_clear_library,
    __path_clear_ssl, __path_delete_ca_host, __path_delete_certificate, __path_save_certificate,
    __path_set_deployment_mode, __path_shared_file_content, __path_shared_files, __path_status,
};
use lan::*;
pub(crate) use library::*;
pub(crate) use normalize::*;
use recommendation::*;
pub(crate) use shared_files::*;
pub(crate) use status::*;

#[cfg(test)]
mod tests;

const MAX_SHARED_FILES: usize = 500;
const MAX_SHARED_FILE_SIZE: u64 = 512 * 1024;
const MAX_SHARED_SCAN_DEPTH: usize = 3;
const SSL_CERT_SHARE_NAME: &str = "fn-knock";
const CA_HOSTS_KEY: &str = "fn_knock:ca:hosts";
const CA_CERT_FILENAME: &str = "rootCA.pem";
const CA_KEY_FILENAME: &str = "rootCA.key.pem";

fn ssl_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.sslRoutes.{key}"))
}

fn ssl_text_params(translator: &Translator, key: &str, params: &[(&str, String)]) -> String {
    translator.t_params(&format!("server.store.ssl.{key}"), params)
}

fn fnos_data_share_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.fnosDataShare.{key}"))
}

fn ssl_gateway_error(translator: &Translator, detail: &str) -> String {
    let base = translator.t("server.sslGateway.syncFailed");
    let detail = detail.trim();
    if detail.is_empty() {
        base
    } else {
        format!("{base}: {detail}")
    }
}

pub(crate) fn validate_ssl_cert_for_response(
    cert: &str,
    key: &str,
    translator: &Translator,
) -> Result<(), String> {
    if cert.trim().is_empty() || key.trim().is_empty() {
        return Err(translator.t("server.store.ssl.certContentRequired"));
    }
    validate_ssl_cert_pair(cert, key)
        .map_err(|error| ssl_validation_error_message(translator, &error))
}

#[derive(Debug)]
enum SslValidationError {
    CertFormatInvalid(String),
    KeyFormatInvalid(String),
    CertKeyMismatch,
    CertKeyCheckFailed(String),
}

fn ssl_validation_error_message(translator: &Translator, error: &SslValidationError) -> String {
    match error {
        SslValidationError::CertFormatInvalid(message) => ssl_text_params(
            translator,
            "certFormatInvalid",
            &[("message", message.clone())],
        ),
        SslValidationError::KeyFormatInvalid(message) => ssl_text_params(
            translator,
            "keyFormatInvalid",
            &[("message", message.clone())],
        ),
        SslValidationError::CertKeyMismatch => translator.t("server.store.ssl.certKeyMismatch"),
        SslValidationError::CertKeyCheckFailed(message) => ssl_text_params(
            translator,
            "certKeyCheckFailed",
            &[("message", message.clone())],
        ),
    }
}

fn ssl_validation_error_plain(error: SslValidationError) -> String {
    match error {
        SslValidationError::CertFormatInvalid(_) => "Certificate format is invalid".to_string(),
        SslValidationError::KeyFormatInvalid(_) => "Private key format is invalid".to_string(),
        SslValidationError::CertKeyMismatch => {
            "Certificate and private key do not match".to_string()
        }
        SslValidationError::CertKeyCheckFailed(message) => {
            if message.trim().is_empty() {
                "Certificate and private key check failed".to_string()
            } else {
                format!("Certificate and private key check failed: {message}")
            }
        }
    }
}

fn localize_ssl_error(translator: &Translator, error: &dyn std::fmt::Display) -> String {
    let message = error.to_string();
    if message.starts_with("Certificate and private key check failed") {
        return ssl_route_text(translator, "certOrKeyInvalid");
    }
    match message.as_str() {
        "Root CA not initialized" => ssl_route_text(translator, "rootCaNotInitialized"),
        "No hosts configured" => ssl_route_text(translator, "emptyDomains"),
        "Certificate format is invalid" | "Private key format is invalid" => {
            ssl_route_text(translator, "certOrKeyInvalid")
        }
        "Certificate and private key do not match" => {
            ssl_route_text(translator, "certOrKeyInvalid")
        }
        _ => message,
    }
}

fn ssl_error_or_route_text(
    translator: &Translator,
    fallback_key: &str,
    error: &dyn std::fmt::Display,
) -> String {
    let raw = error.to_string();
    let localized = localize_ssl_error(translator, error);
    if localized == raw {
        ssl_route_text(translator, fallback_key)
    } else {
        localized
    }
}

fn shared_file_error_status_and_message(
    translator: &Translator,
    error: &anyhow::Error,
) -> (StatusCode, String) {
    if error.downcast_ref::<SharedFileNotFound>().is_some() {
        return (
            StatusCode::NOT_FOUND,
            ssl_route_text(translator, "readSharedFileFailed"),
        );
    }
    if error.downcast_ref::<SharedFileForbidden>().is_some() {
        return (
            StatusCode::FORBIDDEN,
            ssl_route_text(translator, "readSharedFileFailed"),
        );
    }
    let message = error.to_string();
    match message.as_str() {
        "Shared directory is not configured" => (
            StatusCode::NOT_FOUND,
            fnos_data_share_text(translator, "shareMissing"),
        ),
        "Invalid shared file path" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "invalidPath"),
        ),
        "Shared path must be a file" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "fileOnly"),
        ),
        "Shared file is too large" => (
            StatusCode::BAD_REQUEST,
            fnos_data_share_text(translator, "fileTooLarge"),
        ),
        _ => (StatusCode::BAD_REQUEST, message),
    }
}

fn shared_file_error_response(translator: &Translator, error: anyhow::Error) -> Response {
    let (status, message) = shared_file_error_status_and_message(translator, &error);
    response::error(status, message)
}

#[derive(Deserialize)]
struct SharedContentQuery {
    path: String,
}

#[derive(Deserialize)]
struct SaveCertificateBody {
    id: Option<String>,
    label: Option<String>,
    source: Option<String>,
    source_provider: Option<String>,
    primary_domain: Option<String>,
    source_ref_id: Option<String>,
    cert: String,
    key: String,
    activate: Option<bool>,
}

#[derive(Deserialize)]
struct ActivateBody {
    id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CertificateCoverageInput {
    id: String,
    certificate_domains: Vec<String>,
}

#[derive(Deserialize)]
struct DeploymentModeBody {
    deployment_mode: String,
}

#[derive(Deserialize)]
struct AddCaHostBody {
    value: String,
}

pub fn ssl_routes() -> Router<AppState> {
    ssl_openapi_routes().into()
}

pub(crate) fn ssl_openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status))
        .routes(routes!(shared_files))
        .routes(routes!(shared_file_content))
        .routes(routes!(active_cert_pem))
        .routes(routes!(active_cert_zip))
        .routes(routes!(ca_status))
        .routes(routes!(ca_init))
        .routes(routes!(ca_clear))
        .routes(routes!(ca_cert_pem))
        .routes(routes!(ca_server_cert_zip))
        .routes(routes!(ca_hosts))
        .routes(routes!(add_ca_host))
        .routes(routes!(delete_ca_host))
        .routes(routes!(ca_issue))
        .routes(routes!(save_certificate))
        .routes(routes!(clear_library))
        .routes(routes!(delete_certificate))
        .routes(routes!(activate_certificate))
        .routes(routes!(set_deployment_mode))
        .routes(routes!(get_lan_certificate_deployment))
        .routes(routes!(update_lan_certificate_deployment))
        .routes(routes!(list_external_certificate_bindings))
        .routes(routes!(create_external_certificate_binding))
        .routes(routes!(update_external_certificate_binding))
        .routes(routes!(rotate_external_certificate_binding_token))
        .routes(routes!(delete_external_certificate_binding))
        .routes(routes!(clear_ssl))
}
