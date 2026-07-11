use std::{str::FromStr, sync::Arc, time::Duration};

use anyhow::Context;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, Uri, header},
};
use tokio::{
    sync::{Semaphore, mpsc},
    task::{JoinHandle, JoinSet},
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, metadata::MetadataValue, transport::Endpoint};
use uuid::Uuid;

use super::*;
use crate::grpc_proto::{
    AuthBridgeEnvelope, AuthBridgeReady, AuthCacheScope, AuthContext, AuthorizeHttpRequest,
    AuthorizeHttpResponse, Header, HttpAuthMode, PreflightAuthRequest, PreflightAuthResponse,
    VerifyAuthRequest, VerifyAuthResponse, VerifyStreamAuthRequest, VerifyStreamAuthResponse,
    auth_bridge_envelope, auth_bridge_service_client::AuthBridgeServiceClient,
};

const INTERNAL_TOKEN_METADATA_KEY: &str = "x-fn-knock-internal-rpc-token";
const INTERNAL_GRPC_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
const AUTH_BRIDGE_MAX_IN_FLIGHT: usize = 128;
const AUTHORIZE_HTTP_V1_CAPABILITY: &str = "authorize_http_v1";

pub(crate) fn start_auth_bridge(state: AppState) -> JoinHandle<()> {
    let shutdown = state.shutdown.clone();
    tokio::spawn(async move {
        loop {
            if shutdown.is_cancelled() {
                break;
            }
            if let Err(error) = run_auth_bridge_once(state.clone(), &shutdown).await {
                if shutdown.is_cancelled() {
                    break;
                }
                tracing::warn!(%error, "auth bridge disconnected");
            }
            tokio::select! {
                _ = shutdown.cancelled() => break,
                _ = tokio::time::sleep(Duration::from_secs(1)) => {}
            }
        }
    })
}

async fn run_auth_bridge_once(state: AppState, shutdown: &CancellationToken) -> anyhow::Result<()> {
    let endpoint = format!(
        "http://{}",
        normalize_grpc_addr(&state.settings.go_backend_grpc_addr)
    );
    let endpoint_config = Endpoint::from_shared(endpoint.clone())?
        .timeout(state.settings.request_timeout)
        .connect_timeout(state.settings.request_timeout);
    let connect = endpoint_config.connect();
    let channel = tokio::select! {
        _ = shutdown.cancelled() => return Ok(()),
        result = connect => result
            .with_context(|| format!("connect Go gRPC backend at {endpoint}"))?,
    };
    let mut client = AuthBridgeServiceClient::new(channel)
        .max_decoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE)
        .max_encoding_message_size(INTERNAL_GRPC_MAX_MESSAGE_SIZE);
    let (tx, rx) = mpsc::channel::<AuthBridgeEnvelope>(128);
    let limiter = Arc::new(Semaphore::new(AUTH_BRIDGE_MAX_IN_FLIGHT));

    // Queue the handshake before opening the stream. The Go server sends its
    // initial headers eagerly, but having Ready available immediately also
    // avoids coupling the client handshake to response-stream scheduling.
    tx.send(AuthBridgeEnvelope {
        request_id: String::new(),
        payload: Some(auth_bridge_envelope::Payload::Ready(AuthBridgeReady {
            instance_id: Uuid::new_v4().to_string(),
            capabilities: vec![AUTHORIZE_HTTP_V1_CAPABILITY.to_string()],
        })),
    })
    .await
    .context("queue auth bridge ready")?;

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

    let mut workers = JoinSet::new();
    loop {
        let message = tokio::select! {
            _ = shutdown.cancelled() => break,
            result = stream.message() => result.context("read auth bridge message")?,
            Some(result) = workers.join_next(), if !workers.is_empty() => {
                if let Err(error) = result {
                    tracing::debug!(%error, "auth bridge worker stopped unexpectedly");
                }
                continue;
            }
        };
        let Some(message) = message else {
            break;
        };
        let permit = tokio::select! {
            _ = shutdown.cancelled() => break,
            result = limiter.clone().acquire_owned() => {
                result.context("auth bridge concurrency limiter closed")?
            }
        };
        let tx = tx.clone();
        let state = state.clone();
        let worker_shutdown = shutdown.clone();
        workers.spawn(async move {
            let _permit = permit;
            let response = tokio::select! {
                _ = worker_shutdown.cancelled() => None,
                response = handle_bridge_message(state, message) => response,
            };
            if let Some(response) = response
                && let Err(error) = tx.send(response).await
            {
                tracing::debug!(%error, "failed to send auth bridge response");
            }
        });
    }
    workers.abort_all();
    while workers.join_next().await.is_some() {}
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
        auth_bridge_envelope::Payload::AuthorizeHttpRequest(request) => {
            auth_bridge_envelope::Payload::AuthorizeHttpResponse(
                handle_authorize_http(state, request).await,
            )
        }
        _ => return None,
    };
    Some(AuthBridgeEnvelope {
        request_id,
        payload: Some(payload),
    })
}

async fn handle_authorize_http(
    state: AppState,
    request: AuthorizeHttpRequest,
) -> AuthorizeHttpResponse {
    let (run_preflight, run_verify) = http_auth_stages(request.mode);
    let headers = headers_from_auth_context(request.context.as_ref());
    let uri = uri_from_auth_context(request.context.as_ref());
    let client_ip = client_ip_for_auth(&headers);
    let access_mode = requested_access_mode(&headers);
    let mut response = empty_authorize_http_response();

    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "auth bridge authorize HTTP config resolution failed");
            return authorize_http_preparation_error(&state, run_preflight, run_verify).await;
        }
    };
    let translator = translator_from_config(&config);
    let normal_access = match resolve_preflight_normal_access(
        &state,
        &headers,
        &uri,
        &config,
        &client_ip,
        access_mode,
    )
    .await
    {
        Ok(access) => access,
        Err(error) => {
            tracing::warn!(%error, "auth bridge authorize HTTP normal access resolution failed");
            return authorize_http_preparation_error(&state, run_preflight, run_verify).await;
        }
    };

    let mut preflight_rejected = false;
    if run_preflight {
        let mut preflight = new_preflight_response();
        match apply_preflight_behavior_with_normal_access(
            &state,
            &headers,
            &uri,
            &mut preflight,
            &config,
            &client_ip,
            access_mode,
            &normal_access,
        )
        .await
        {
            Ok(()) => {
                response.preflight_cache_scope = AuthCacheScope::ExactRequest as i32;
            }
            Err(error) => {
                tracing::warn!(%error, "auth bridge authorize HTTP preflight failed");
            }
        }
        let preflight = preflight_auth_response_from_http(&preflight);
        preflight_rejected = preflight_rejects_request(&preflight);
        response.preflight = Some(preflight);
    }

    if run_verify && !preflight_rejected {
        match resolve_auth_access_with_normal_access(
            &state,
            &headers,
            &uri,
            &translator,
            &config,
            &client_ip,
            &normal_access,
        )
        .await
        {
            Ok(access) => {
                response.verify_cache_scope = verify_cache_scope(&access) as i32;
                response.verify = Some(verify_auth_response_from_access(access));
            }
            Err(error) => {
                tracing::warn!(%error, "auth bridge authorize HTTP verify failed");
                response.verify = Some(verify_auth_error_response(&translator));
            }
        }
    }

    response
}

fn empty_authorize_http_response() -> AuthorizeHttpResponse {
    AuthorizeHttpResponse {
        preflight: None,
        verify: None,
        preflight_cache_scope: AuthCacheScope::None as i32,
        verify_cache_scope: AuthCacheScope::None as i32,
    }
}

async fn authorize_http_preparation_error(
    state: &AppState,
    run_preflight: bool,
    run_verify: bool,
) -> AuthorizeHttpResponse {
    let mut response = empty_authorize_http_response();
    if run_preflight {
        response.preflight = Some(preflight_auth_response_from_http(&new_preflight_response()));
    }
    if run_verify {
        let translator = Translator::from_state(state).await;
        response.verify = Some(verify_auth_error_response(&translator));
    }
    response
}

fn http_auth_stages(mode: i32) -> (bool, bool) {
    match HttpAuthMode::try_from(mode).unwrap_or(HttpAuthMode::Unspecified) {
        HttpAuthMode::PreflightOnly => (true, false),
        HttpAuthMode::VerifyOnly => (false, true),
        HttpAuthMode::PreflightAndVerify | HttpAuthMode::Unspecified => (true, true),
    }
}

async fn handle_verify_auth(state: AppState, request: VerifyAuthRequest) -> VerifyAuthResponse {
    let headers = headers_from_auth_context(request.context.as_ref());
    let uri = uri_from_auth_context(request.context.as_ref());
    let translator = Translator::from_state(&state).await;
    match resolve_auth_access(&state, &headers, &uri, &translator).await {
        Ok(access) => verify_auth_response_from_access(access),
        Err(error) => {
            tracing::warn!(%error, "auth bridge verify failed");
            verify_auth_error_response(&translator)
        }
    }
}

fn verify_auth_response_from_access(access: AuthAccess) -> VerifyAuthResponse {
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

fn verify_auth_error_response(translator: &Translator) -> VerifyAuthResponse {
    VerifyAuthResponse {
        success: false,
        message: auth_route_text(translator, "verifyFailed"),
        status: StatusCode::INTERNAL_SERVER_ERROR.as_u16() as i32,
        set_cookies: Vec::new(),
        suppress_toolbar: false,
        redirect_location: String::new(),
        access_denied_reason: String::new(),
        response_headers: Vec::new(),
    }
}

fn verify_cache_scope(access: &AuthAccess) -> AuthCacheScope {
    if !access.set_cookies.is_empty()
        || access
            .response_headers
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(header::SET_COOKIE.as_str()))
    {
        return AuthCacheScope::None;
    }
    if access.authenticated
        && matches!(
            access.grant_type.as_deref(),
            Some("local_exempt" | "manual_whitelist" | "browser_session" | "login_ip_grant")
        )
    {
        AuthCacheScope::Host
    } else {
        AuthCacheScope::ExactRequest
    }
}

async fn handle_preflight_auth(
    state: AppState,
    request: PreflightAuthRequest,
) -> PreflightAuthResponse {
    let headers = headers_from_auth_context(request.context.as_ref());
    let uri = uri_from_auth_context(request.context.as_ref());
    let mut response = new_preflight_response();

    if let Err(error) = apply_preflight_behavior(&state, &headers, &uri, &mut response).await {
        tracing::warn!(%error, "auth bridge preflight failed");
    }
    preflight_auth_response_from_http(&response)
}

fn new_preflight_response() -> Response<Body> {
    let mut response = Response::builder()
        .status(StatusCode::NO_CONTENT)
        .body(Body::empty())
        .unwrap_or_else(|_| Response::new(Body::empty()));
    apply_no_store_headers(response.headers_mut());
    response
}

fn preflight_auth_response_from_http(response: &Response<Body>) -> PreflightAuthResponse {
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

fn preflight_rejects_request(response: &PreflightAuthResponse) -> bool {
    response.deny
        || !response.redirect_location.trim().is_empty()
        || !response.access_denied_reason.trim().is_empty()
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
    insert_header(&mut headers, "accesstoken", &context.access_token);
    insert_header(
        &mut headers,
        "access-token",
        &context.access_token_hyphenated,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn access(grant_type: Option<&str>) -> AuthAccess {
        AuthAccess {
            authenticated: grant_type.is_some(),
            message: String::new(),
            grant_type: grant_type.map(ToString::to_string),
            deny_reason: None,
            set_cookies: Vec::new(),
            response_headers: Vec::new(),
        }
    }

    #[test]
    fn authorize_http_capability_and_modes_are_stable() {
        assert_eq!(AUTHORIZE_HTTP_V1_CAPABILITY, "authorize_http_v1");
        assert_eq!(
            http_auth_stages(HttpAuthMode::PreflightOnly as i32),
            (true, false)
        );
        assert_eq!(
            http_auth_stages(HttpAuthMode::VerifyOnly as i32),
            (false, true)
        );
        assert_eq!(
            http_auth_stages(HttpAuthMode::PreflightAndVerify as i32),
            (true, true)
        );
        assert_eq!(http_auth_stages(i32::MAX), (true, true));
    }

    #[test]
    fn dedicated_access_token_fields_replace_legacy_header_copies() {
        let context = AuthContext {
            access_token: "compact-token".to_string(),
            access_token_hyphenated: "hyphenated-token".to_string(),
            extra_headers: vec![
                Header {
                    name: "AccessToken".to_string(),
                    values: vec!["legacy-compact".to_string()],
                },
                Header {
                    name: "Access-Token".to_string(),
                    values: vec!["legacy-hyphenated".to_string()],
                },
            ],
            ..Default::default()
        };

        let headers = headers_from_auth_context(Some(&context));
        assert_eq!(headers["accesstoken"], "compact-token");
        assert_eq!(headers["access-token"], "hyphenated-token");
    }

    #[test]
    fn preflight_rejection_stops_combined_verification() {
        assert!(!preflight_rejects_request(&PreflightAuthResponse::default()));
        assert!(preflight_rejects_request(&PreflightAuthResponse {
            redirect_location: "/login".to_string(),
            ..Default::default()
        }));
        assert!(preflight_rejects_request(&PreflightAuthResponse {
            access_denied_reason: REAUTH_SCOPE_DENIED.to_string(),
            ..Default::default()
        }));
    }

    #[test]
    fn verify_cache_scope_is_host_only_for_stable_normal_access() {
        for grant_type in [
            "local_exempt",
            "manual_whitelist",
            "browser_session",
            "login_ip_grant",
        ] {
            assert_eq!(
                verify_cache_scope(&access(Some(grant_type))),
                AuthCacheScope::Host
            );
        }

        assert_eq!(
            verify_cache_scope(&access(Some("fnos_share"))),
            AuthCacheScope::ExactRequest
        );
        assert_eq!(
            verify_cache_scope(&access(Some("fnos_fingerprint_session"))),
            AuthCacheScope::ExactRequest
        );
        assert_eq!(
            verify_cache_scope(&access(None)),
            AuthCacheScope::ExactRequest
        );

        let mut cookie_access = access(Some("browser_session"));
        cookie_access.set_cookies.push("sid=rotated".to_string());
        assert_eq!(verify_cache_scope(&cookie_access), AuthCacheScope::None);

        let mut header_cookie_access = access(Some("browser_session"));
        header_cookie_access
            .response_headers
            .push(("Set-Cookie".to_string(), "sid=rotated".to_string()));
        assert_eq!(
            verify_cache_scope(&header_cookie_access),
            AuthCacheScope::None
        );
    }
}
