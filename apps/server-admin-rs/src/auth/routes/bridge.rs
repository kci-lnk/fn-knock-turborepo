use std::{str::FromStr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, Uri, header},
};
use tokio::sync::{Semaphore, mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, metadata::MetadataValue, transport::Endpoint};
use uuid::Uuid;

use super::*;
use crate::grpc_proto::{
    AuthBridgeEnvelope, AuthBridgeReady, AuthContext, Header, PreflightAuthRequest,
    PreflightAuthResponse, VerifyAuthRequest, VerifyAuthResponse, VerifyStreamAuthRequest,
    VerifyStreamAuthResponse, auth_bridge_envelope,
    auth_bridge_service_client::AuthBridgeServiceClient,
};

const INTERNAL_TOKEN_METADATA_KEY: &str = "x-fn-knock-internal-rpc-token";
const INTERNAL_GRPC_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
const AUTH_BRIDGE_MAX_IN_FLIGHT: usize = 128;

pub(crate) fn start_auth_bridge(state: AppState) {
    tokio::spawn(async move {
        loop {
            if let Err(error) = run_auth_bridge_once(state.clone()).await {
                tracing::warn!(%error, "auth bridge disconnected");
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
}

async fn run_auth_bridge_once(state: AppState) -> anyhow::Result<()> {
    let endpoint = format!(
        "http://{}",
        normalize_grpc_addr(&state.settings.go_backend_grpc_addr)
    );
    let channel = Endpoint::from_shared(endpoint.clone())?
        .timeout(state.settings.request_timeout)
        .connect_timeout(state.settings.request_timeout)
        .connect()
        .await
        .with_context(|| format!("connect Go gRPC backend at {endpoint}"))?;
    let mut client = AuthBridgeServiceClient::new(channel)
        .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
        .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE);
    let (tx, rx) = mpsc::channel::<AuthBridgeEnvelope>(128);
    let limiter = Arc::new(Semaphore::new(AUTH_BRIDGE_MAX_IN_FLIGHT));
    let mut request = Request::new(ReceiverStream::new(rx));
    request.metadata_mut().insert(
        INTERNAL_TOKEN_METADATA_KEY,
        metadata_token(&state.settings.internal_rpc_token)?,
    );

    let mut stream = client
        .connect_auth_bridge(request)
        .await
        .context("connect auth bridge stream")?
        .into_inner();
    tx.send(AuthBridgeEnvelope {
        request_id: String::new(),
        payload: Some(auth_bridge_envelope::Payload::Ready(AuthBridgeReady {
            instance_id: Uuid::new_v4().to_string(),
        })),
    })
    .await
    .context("send auth bridge ready")?;

    while let Some(message) = stream.message().await.context("read auth bridge message")? {
        let permit = limiter
            .clone()
            .acquire_owned()
            .await
            .context("auth bridge concurrency limiter closed")?;
        let tx = tx.clone();
        let state = state.clone();
        tokio::spawn(async move {
            let _permit = permit;
            if let Some(response) = handle_bridge_message(state, message).await {
                if let Err(error) = tx.send(response).await {
                    tracing::debug!(%error, "failed to send auth bridge response");
                }
            }
        });
    }
    Ok(())
}

async fn handle_bridge_message(
    state: AppState,
    message: AuthBridgeEnvelope,
) -> Option<AuthBridgeEnvelope> {
    let request_id = message.request_id;
    let payload = match message.payload? {
        auth_bridge_envelope::Payload::VerifyAuthRequest(request) => {
            auth_bridge_envelope::Payload::VerifyAuthResponse(
                handle_verify_auth(state, request).await,
            )
        }
        auth_bridge_envelope::Payload::PreflightAuthRequest(request) => {
            auth_bridge_envelope::Payload::PreflightAuthResponse(
                handle_preflight_auth(state, request).await,
            )
        }
        auth_bridge_envelope::Payload::VerifyStreamAuthRequest(request) => {
            auth_bridge_envelope::Payload::VerifyStreamAuthResponse(
                handle_verify_stream_auth(state, request).await,
            )
        }
        _ => return None,
    };
    Some(AuthBridgeEnvelope {
        request_id,
        payload: Some(payload),
    })
}

async fn handle_verify_auth(state: AppState, request: VerifyAuthRequest) -> VerifyAuthResponse {
    let headers = headers_from_auth_context(request.context.as_ref());
    let uri = uri_from_auth_context(request.context.as_ref());
    let translator = Translator::from_state(&state).await;
    match resolve_auth_access(&state, &headers, &uri, &translator).await {
        Ok(access) => {
            let status = if access.authenticated {
                StatusCode::OK
            } else {
                auth_verify_denied_status(&access)
            };
            VerifyAuthResponse {
                success: access.authenticated,
                message: access.message,
                status: status.as_u16() as i32,
                set_cookies: access.set_cookies,
                suppress_toolbar: access.grant_type.as_deref() == Some("fnos_share"),
                redirect_location: String::new(),
                access_denied_reason: access.deny_reason.unwrap_or_default(),
                response_headers: headers_from_pairs(&access.response_headers),
            }
        }
        Err(error) => {
            tracing::warn!(%error, "auth bridge verify failed");
            VerifyAuthResponse {
                success: false,
                message: auth_route_text(&translator, "verifyFailed"),
                status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
                set_cookies: Vec::new(),
                suppress_toolbar: false,
                redirect_location: String::new(),
                access_denied_reason: String::new(),
                response_headers: Vec::new(),
            }
        }
    }
}

async fn handle_preflight_auth(
    state: AppState,
    request: PreflightAuthRequest,
) -> PreflightAuthResponse {
    let headers = headers_from_auth_context(request.context.as_ref());
    let uri = uri_from_auth_context(request.context.as_ref());
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()));
    apply_no_store_headers(response.headers_mut());

    if let Err(error) = apply_preflight_behavior(&state, &headers, &uri, &mut response).await {
        tracing::warn!(%error, "auth bridge preflight failed");
    }
    let response_headers = response.headers();
    PreflightAuthResponse {
        deny: response_headers
            .get("X-Option")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("deny")),
        redirect_location: response_headers
            .get("X-Reauth-Redirect-Location")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        access_denied_reason: response_headers
            .get(REAUTH_ACCESS_DENIED_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        response_headers: headers_from_header_map(response_headers),
    }
}

async fn handle_verify_stream_auth(
    state: AppState,
    request: VerifyStreamAuthRequest,
) -> VerifyStreamAuthResponse {
    let mut headers = HeaderMap::new();
    insert_header(&mut headers, "X-Real-IP", &request.client_ip);
    insert_header(&mut headers, "X-Forwarded-For", &request.client_ip);
    insert_header(&mut headers, "X-Reauth-Protocol", &request.protocol);
    insert_header(
        &mut headers,
        "X-Reauth-Listen-Port",
        &request.listen_port.to_string(),
    );
    insert_header(&mut headers, "X-Reauth-Target", &request.target);
    let uri = Uri::from_static("/");
    let translator = Translator::from_state(&state).await;

    match resolve_auth_access(&state, &headers, &uri, &translator).await {
        Ok(access) if access.authenticated => VerifyStreamAuthResponse {
            allowed: true,
            status: StatusCode::OK.as_u16() as i32,
            decision: "passed".to_string(),
            message: access.message,
        },
        Ok(access) => VerifyStreamAuthResponse {
            allowed: false,
            status: auth_verify_denied_status(&access).as_u16() as i32,
            decision: if access.deny_reason.as_deref() == Some(REAUTH_SCOPE_DENIED) {
                "access_denied".to_string()
            } else {
                "denied".to_string()
            },
            message: access.message,
        },
        Err(error) => {
            tracing::warn!(%error, "auth bridge stream verify failed");
            VerifyStreamAuthResponse {
                allowed: false,
                status: StatusCode::BAD_GATEWAY.as_u16() as i32,
                decision: "auth_error".to_string(),
                message: "Authentication Service Unavailable".to_string(),
            }
        }
    }
}

fn headers_from_auth_context(context: Option<&AuthContext>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Some(context) = context else {
        return headers;
    };
    for header in &context.extra_headers {
        for value in &header.values {
            append_header(&mut headers, &header.name, value);
        }
    }
    insert_header(&mut headers, "X-Real-IP", &context.client_ip);
    insert_header(&mut headers, "X-Forwarded-For", &context.forwarded_for);
    insert_header(&mut headers, "X-Forwarded-Host", &context.forwarded_host);
    insert_header(&mut headers, "X-Forwarded-Proto", &context.forwarded_proto);
    insert_header(&mut headers, "X-Forwarded-Path", &context.forwarded_path);
    insert_header(&mut headers, "X-Reauth-Access-Mode", &context.access_mode);
    insert_header(&mut headers, header::COOKIE.as_str(), &context.cookie);
    insert_header(
        &mut headers,
        header::AUTHORIZATION.as_str(),
        &context.authorization,
    );
    insert_header(
        &mut headers,
        header::USER_AGENT.as_str(),
        &context.user_agent,
    );
    headers
}

fn uri_from_auth_context(context: Option<&AuthContext>) -> Uri {
    let Some(context) = context else {
        return Uri::from_static("/");
    };
    let request_uri = if context.request_uri.trim().is_empty() {
        let path = if context.path.trim().is_empty() {
            "/"
        } else {
            context.path.trim()
        };
        if context.raw_query.trim().is_empty() {
            path.to_string()
        } else {
            format!("{path}?{}", context.raw_query.trim())
        }
    } else {
        context.request_uri.trim().to_string()
    };
    Uri::from_str(&request_uri).unwrap_or_else(|_| Uri::from_static("/"))
}

fn headers_from_pairs(values: &[(String, String)]) -> Vec<Header> {
    values
        .iter()
        .map(|(name, value)| Header {
            name: name.clone(),
            values: vec![value.clone()],
        })
        .collect()
}

fn headers_from_header_map(headers: &HeaderMap) -> Vec<Header> {
    headers
        .iter()
        .filter_map(|(name, value)| {
            value.to_str().ok().map(|value| Header {
                name: name.as_str().to_string(),
                values: vec![value.to_string()],
            })
        })
        .collect()
}

fn insert_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };
    headers.insert(name, value);
}

fn append_header(headers: &mut HeaderMap, name: &str, value: &str) {
    if value.trim().is_empty() {
        return;
    }
    let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
        return;
    };
    let Ok(value) = HeaderValue::from_str(value) else {
        return;
    };
    headers.append(name, value);
}

fn normalize_grpc_addr(addr: &str) -> String {
    addr.trim()
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/')
        .to_string()
}

fn metadata_token(token: &str) -> anyhow::Result<MetadataValue<tonic::metadata::Ascii>> {
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("FN_KNOCK_INTERNAL_RPC_TOKEN must be set for auth bridge");
    }
    MetadataValue::try_from(token).context("encode FN_KNOCK_INTERNAL_RPC_TOKEN metadata")
}
