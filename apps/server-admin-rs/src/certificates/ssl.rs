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
    routing::{delete, get, post},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use x509_parser::{extensions::GeneralName, pem::parse_x509_pem, time::ASN1Time};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    i18n::{DEFAULT_LOCALE, Translator},
    response,
    state::AppState,
    time_utils,
};

mod ca;
mod deployment;
mod handlers;
mod library;
mod normalize;
mod recommendation;
mod shared_files;
mod status;

use ca::*;
pub(crate) use deployment::*;
use handlers::*;
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
    Router::new()
        .route("/api/admin/ssl/status", get(status))
        .route("/api/admin/ssl/shared-files", get(shared_files))
        .route(
            "/api/admin/ssl/shared-files/content",
            get(shared_file_content),
        )
        .route("/api/admin/ssl/cert.pem", get(active_cert_pem))
        .route("/api/admin/ssl/cert.zip", get(active_cert_zip))
        .route("/api/admin/ssl/ca/status", get(ca_status))
        .route("/api/admin/ssl/ca/init", post(ca_init))
        .route("/api/admin/ssl/ca", delete(ca_clear))
        .route("/api/admin/ssl/ca/cert.pem", get(ca_cert_pem))
        .route("/api/admin/ssl/ca/server-cert.zip", get(ca_server_cert_zip))
        .route(
            "/api/admin/ssl/ca/hosts",
            get(ca_hosts).post(add_ca_host).delete(delete_ca_host),
        )
        .route("/api/admin/ssl/ca/issue", post(ca_issue))
        .route(
            "/api/admin/ssl/certificates",
            post(save_certificate).delete(clear_library),
        )
        .route(
            "/api/admin/ssl/certificates/{id}",
            delete(delete_certificate),
        )
        .route("/api/admin/ssl/activate", post(activate_certificate))
        .route("/api/admin/ssl/deployment-mode", post(set_deployment_mode))
        .route("/api/admin/ssl", delete(clear_ssl))
}
