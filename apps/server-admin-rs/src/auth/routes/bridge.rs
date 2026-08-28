use std::{
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use axum::{
    body::Body,
    http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, Uri, header},
};
use tokio::{
    sync::{Semaphore, mpsc},
    task::{JoinHandle, JoinSet},
};
use tokio_stream::{StreamExt, wrappers::ReceiverStream};
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
const AUTH_BRIDGE_INSTANCE_METADATA_KEY: &str = "x-fn-knock-auth-bridge-instance-id";
const AUTH_BRIDGE_CAPABILITY_METADATA_KEY: &str = "x-fn-knock-auth-bridge-capability";
const INTERNAL_GRPC_MAX_MESSAGE_SIZE: usize = 16 * 1024 * 1024;
const AUTHORIZE_HTTP_V1_CAPABILITY: &str = "authorize_http_v1";
const AUTH_BRIDGE_RESPONSE_HEADROOM: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BridgeRequestKind {
    VerifyAuth,
    PreflightAuth,
    VerifyStreamAuth,
    AuthorizeHttp {
        run_preflight: bool,
        run_verify: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BridgeResponseSendOutcome {
    Sent,
    Closed,
    TimedOut,
    Cancelled,
}

#[derive(Clone, Copy, Debug)]
struct AuthBridgeDeadlines {
    handler: tokio::time::Instant,
    response: tokio::time::Instant,
}

impl BridgeRequestKind {
    fn from_message(message: &AuthBridgeEnvelope) -> Option<Self> {
        match message.payload.as_ref()? {
            auth_bridge_envelope::Payload::VerifyAuthRequest(_) => Some(Self::VerifyAuth),
            auth_bridge_envelope::Payload::PreflightAuthRequest(_) => Some(Self::PreflightAuth),
            auth_bridge_envelope::Payload::VerifyStreamAuthRequest(_) => {
                Some(Self::VerifyStreamAuth)
            }
            auth_bridge_envelope::Payload::AuthorizeHttpRequest(request) => {
                let (run_preflight, run_verify) = http_auth_stages(request.mode);
                Some(Self::AuthorizeHttp {
                    run_preflight,
                    run_verify,
                })
            }
            _ => None,
        }
    }

    fn operation(self) -> &'static str {
        match self {
            Self::VerifyAuth => "verify_auth",
            Self::PreflightAuth => "preflight_auth",
            Self::VerifyStreamAuth => "verify_stream_auth",
            Self::AuthorizeHttp { .. } => "authorize_http",
        }
    }

    fn unavailable_envelope(self, request_id: String) -> AuthBridgeEnvelope {
        let payload = match self {
            Self::VerifyAuth => {
                auth_bridge_envelope::Payload::VerifyAuthResponse(verify_auth_unavailable_response())
            }
            Self::PreflightAuth => auth_bridge_envelope::Payload::PreflightAuthResponse(
                preflight_auth_unavailable_response(),
            ),
            Self::VerifyStreamAuth => auth_bridge_envelope::Payload::VerifyStreamAuthResponse(
                verify_stream_auth_unavailable_response(),
            ),
            Self::AuthorizeHttp {
                run_preflight,
                run_verify,
            } => {
                let mut response = empty_authorize_http_response();
                if run_preflight {
                    response.preflight = Some(preflight_auth_unavailable_response());
                }
                if run_verify {
                    response.verify = Some(verify_auth_unavailable_response());
                }
                auth_bridge_envelope::Payload::AuthorizeHttpResponse(response)
            }
        };
        AuthBridgeEnvelope {
            request_id,
            deadline_unix_millis: 0,
            payload: Some(payload),
        }
    }
}

fn auth_bridge_response_headroom(request_timeout: Duration) -> Duration {
    if request_timeout.is_zero() {
        return Duration::ZERO;
    }
    let minimum_handler_time = request_timeout.min(Duration::from_millis(1));
    let maximum_headroom = request_timeout.saturating_sub(minimum_handler_time);
    AUTH_BRIDGE_RESPONSE_HEADROOM
        .min((request_timeout / 5).max(Duration::from_millis(1)))
        .min(maximum_headroom)
}

#[cfg(test)]
fn auth_bridge_handler_timeout(request_timeout: Duration) -> Duration {
    request_timeout.saturating_sub(auth_bridge_response_headroom(request_timeout))
}

fn auth_bridge_request_deadlines(
    message: &AuthBridgeEnvelope,
    received_at: tokio::time::Instant,
    request_timeout: Duration,
) -> AuthBridgeDeadlines {
    auth_bridge_request_deadlines_at(
        message,
        received_at,
        request_timeout,
        tokio::time::Instant::now(),
        SystemTime::now(),
    )
}

fn auth_bridge_request_deadlines_at(
    message: &AuthBridgeEnvelope,
    received_at: tokio::time::Instant,
    request_timeout: Duration,
    now: tokio::time::Instant,
    wall_now: SystemTime,
) -> AuthBridgeDeadlines {
    // Legacy gateways do not send deadline_unix_millis. Their fallback starts
    // when the request is read from gRPC, while current gateways preserve the
    // earlier deadline assigned before entering Go's send queue.
    let fallback = received_at
        .checked_add(request_timeout)
        .unwrap_or(received_at);
    let propagated = u64::try_from(message.deadline_unix_millis)
        .ok()
        .filter(|deadline| *deadline > 0)
        .and_then(|deadline| UNIX_EPOCH.checked_add(Duration::from_millis(deadline)))
        .map(|deadline| deadline.duration_since(wall_now).unwrap_or(Duration::ZERO))
        .and_then(|remaining| now.checked_add(remaining));
    let response = propagated.map_or(fallback, |deadline| deadline.min(fallback));
    let handler = response
        .checked_sub(auth_bridge_response_headroom(request_timeout))
        .unwrap_or(response);
    AuthBridgeDeadlines { handler, response }
}

async fn acquire_auth_bridge_capacity(
    limiter: Arc<Semaphore>,
    deadline: tokio::time::Instant,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    tokio::time::timeout_at(deadline, limiter.acquire_owned())
        .await
        .ok()?
        .ok()
}

async fn send_auth_bridge_response(
    tx: &mpsc::Sender<AuthBridgeEnvelope>,
    response: AuthBridgeEnvelope,
    shutdown: &CancellationToken,
    deadline: tokio::time::Instant,
) -> BridgeResponseSendOutcome {
    // Do not let an already-expired request consume response-queue capacity.
    // The Go caller has stopped waiting by this point, so enqueuing a stale
    // response can only amplify bridge backpressure for live requests.
    if tokio::time::Instant::now() >= deadline {
        return BridgeResponseSendOutcome::TimedOut;
    }
    let response = match tx.try_send(response) {
        Ok(()) => return BridgeResponseSendOutcome::Sent,
        Err(mpsc::error::TrySendError::Closed(_)) => {
            return BridgeResponseSendOutcome::Closed;
        }
        Err(mpsc::error::TrySendError::Full(response)) => response,
    };
    tokio::select! {
        _ = shutdown.cancelled() => BridgeResponseSendOutcome::Cancelled,
        result = tokio::time::timeout_at(deadline, tx.send(response)) => match result {
            Ok(Ok(())) => BridgeResponseSendOutcome::Sent,
            Ok(Err(_)) => BridgeResponseSendOutcome::Closed,
            Err(_) => BridgeResponseSendOutcome::TimedOut,
        },
    }
}

fn record_bridge_pressure(
    state: &AppState,
    event: &str,
    reason_code: &str,
    kind: BridgeRequestKind,
    duration: Duration,
    max_in_flight: usize,
) {
    state.runtime_health.operational_log(
        "WARN",
        "auth_bridge",
        event,
        reason_code,
        serde_json::Map::from_iter([
            ("operation".to_string(), json!(kind.operation())),
            ("duration_ms".to_string(), json!(duration.as_millis())),
            ("max_in_flight".to_string(), json!(max_in_flight)),
        ]),
    );
}

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
    let max_in_flight = state.settings.auth_bridge_max_in_flight;
    let (tx, rx) = mpsc::channel::<AuthBridgeEnvelope>(max_in_flight);
    let limiter = Arc::new(Semaphore::new(max_in_flight));
    tracing::info!(
        runtime_target = %crate::runtime_profile::deployment_target(&state),
        max_in_flight,
        "auth bridge concurrency configured"
    );

    let instance_id = Uuid::new_v4().to_string();
    let capabilities = vec![
        AUTHORIZE_HTTP_V1_CAPABILITY.to_string(),
        "subdomain_rule_grant_v1".to_string(),
    ];
    let ready = AuthBridgeEnvelope {
        request_id: String::new(),
        deadline_unix_millis: 0,
        payload: Some(auth_bridge_envelope::Payload::Ready(AuthBridgeReady {
            instance_id: instance_id.clone(),
            capabilities: capabilities.clone(),
        })),
    };
    // Advertise the same handshake in metadata below so readiness never
    // depends on when HTTP/2 first polls DATA. Keep Ready as the first body
    // item for protocol compatibility and capability refreshes; later
    // authorization responses continue through the bounded channel.
    let mut request = Request::new(tokio_stream::once(ready).chain(ReceiverStream::new(rx)));
    request.metadata_mut().insert(
        INTERNAL_TOKEN_METADATA_KEY,
        metadata_token(&state.settings.internal_rpc_token)?,
    );
    request.metadata_mut().insert(
        AUTH_BRIDGE_INSTANCE_METADATA_KEY,
        MetadataValue::try_from(instance_id.as_str()).context("encode auth bridge instance id")?,
    );
    for capability in &capabilities {
        request.metadata_mut().append(
            AUTH_BRIDGE_CAPABILITY_METADATA_KEY,
            MetadataValue::try_from(capability.as_str())
                .context("encode auth bridge capability")?,
        );
    }

    let mut stream = client
        .connect_auth_bridge(request)
        .await
        .context("connect auth bridge stream")?
        .into_inner();

    // Keep the response stream in this task. Capacity admission happens inside
    // a bounded worker set so a saturated handler cannot stop the stream reader.
    let dispatch_limiter = Arc::new(Semaphore::new(max_in_flight.saturating_mul(2)));
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
        let Some(kind) = BridgeRequestKind::from_message(&message) else {
            continue;
        };
        let request_id = message.request_id.clone();
        let request_started = tokio::time::Instant::now();
        let deadlines = auth_bridge_request_deadlines(
            &message,
            request_started,
            state.settings.request_timeout,
        );
        let Ok(dispatch_permit) = dispatch_limiter.clone().try_acquire_owned() else {
            record_bridge_pressure(
                &state,
                "request_rejected",
                "dispatch_queue_full",
                kind,
                request_started.elapsed(),
                max_in_flight,
            );
            let response = kind.unavailable_envelope(request_id);
            match send_auth_bridge_response(&tx, response, shutdown, deadlines.response).await {
                BridgeResponseSendOutcome::Sent => {}
                BridgeResponseSendOutcome::TimedOut => {
                    record_bridge_pressure(
                        &state,
                        "response_dropped",
                        "response_queue_timeout",
                        kind,
                        request_started.elapsed(),
                        max_in_flight,
                    );
                }
                BridgeResponseSendOutcome::Closed | BridgeResponseSendOutcome::Cancelled => break,
            }
            continue;
        };
        let tx = tx.clone();
        let state = state.clone();
        let limiter = limiter.clone();
        let worker_shutdown = shutdown.clone();
        workers.spawn(async move {
            let _dispatch_permit = dispatch_permit;
            let expired_before_capacity = tokio::time::Instant::now() >= deadlines.handler;
            let permit = tokio::select! {
                _ = worker_shutdown.cancelled() => return,
                permit = acquire_auth_bridge_capacity(limiter, deadlines.handler) => permit,
            };
            let Some(permit) = permit else {
                record_bridge_pressure(
                    &state,
                    "request_rejected",
                    if expired_before_capacity {
                        "request_deadline_expired"
                    } else {
                        "capacity_wait_timeout"
                    },
                    kind,
                    request_started.elapsed(),
                    max_in_flight,
                );
                let response = kind.unavailable_envelope(request_id);
                match send_auth_bridge_response(&tx, response, &worker_shutdown, deadlines.response)
                    .await
                {
                    BridgeResponseSendOutcome::Sent | BridgeResponseSendOutcome::Cancelled => {}
                    BridgeResponseSendOutcome::Closed => {
                        tracing::debug!("failed to send auth bridge response: stream closed");
                    }
                    BridgeResponseSendOutcome::TimedOut => record_bridge_pressure(
                        &state,
                        "response_dropped",
                        "response_queue_timeout",
                        kind,
                        request_started.elapsed(),
                        max_in_flight,
                    ),
                }
                return;
            };
            let capacity_wait = request_started.elapsed();
            if capacity_wait >= Duration::from_millis(1) {
                record_bridge_pressure(
                    &state,
                    "request_queued",
                    "capacity_waited",
                    kind,
                    capacity_wait,
                    max_in_flight,
                );
            }
            let response = {
                // Capacity protects authorization work, not response delivery.
                // Release it as soon as the handler finishes so a slow/full
                // outbound queue cannot stall otherwise independent requests.
                let _permit = permit;
                tokio::select! {
                    _ = worker_shutdown.cancelled() => None,
                    response = tokio::time::timeout_at(
                        deadlines.handler,
                        handle_bridge_message(state.clone(), message),
                    ) => match response {
                        Ok(response) => response,
                        Err(_) => {
                            record_bridge_pressure(
                                &state,
                                "request_timeout",
                                "handler_deadline_exceeded",
                                kind,
                                request_started.elapsed(),
                                max_in_flight,
                            );
                            Some(kind.unavailable_envelope(request_id))
                        }
                    },
                }
            };
            if let Some(response) = response {
                match send_auth_bridge_response(&tx, response, &worker_shutdown, deadlines.response)
                    .await
                {
                    BridgeResponseSendOutcome::Sent | BridgeResponseSendOutcome::Cancelled => {}
                    BridgeResponseSendOutcome::Closed => {
                        tracing::debug!("failed to send auth bridge response: stream closed");
                    }
                    BridgeResponseSendOutcome::TimedOut => record_bridge_pressure(
                        &state,
                        "response_dropped",
                        "response_queue_timeout",
                        kind,
                        request_started.elapsed(),
                        max_in_flight,
                    ),
                }
            }
        });
    }
    workers.abort_all();
    while workers.join_next().await.is_some() {}
    Ok(())
}

fn preflight_auth_unavailable_response() -> PreflightAuthResponse {
    PreflightAuthResponse {
        deny: true,
        redirect_location: String::new(),
        access_denied_reason: "auth_service_unavailable".to_string(),
        response_headers: auth_bridge_unavailable_headers(),
    }
}

fn auth_bridge_unavailable_headers() -> Vec<Header> {
    vec![
        Header {
            name: header::CACHE_CONTROL.as_str().to_string(),
            values: vec!["no-store".to_string()],
        },
        Header {
            name: header::RETRY_AFTER.as_str().to_string(),
            values: vec!["1".to_string()],
        },
    ]
}

fn verify_auth_unavailable_response() -> VerifyAuthResponse {
    VerifyAuthResponse {
        success: false,
        message: "Authentication Service Unavailable".to_string(),
        status: StatusCode::SERVICE_UNAVAILABLE.as_u16() as i32,
        set_cookies: Vec::new(),
        suppress_toolbar: false,
        redirect_location: String::new(),
        access_denied_reason: "auth_service_unavailable".to_string(),
        response_headers: auth_bridge_unavailable_headers(),
        grant_kind: crate::grpc_proto::AuthGrantKind::Unspecified as i32,
        decision: "auth_unavailable".to_string(),
        login_authenticated: false,
        host_authorized: false,
        cache_max_age_seconds: 0,
        auth_rule_group_id: String::new(),
        auth_grant_state: String::new(),
    }
}

fn verify_stream_auth_unavailable_response() -> VerifyStreamAuthResponse {
    VerifyStreamAuthResponse {
        allowed: false,
        status: StatusCode::SERVICE_UNAVAILABLE.as_u16() as i32,
        decision: "auth_unavailable".to_string(),
        message: "Authentication Service Unavailable".to_string(),
    }
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
        deadline_unix_millis: 0,
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
    let (routed_upstream, routed_upstream_host, routed_upstream_route_id) =
        routed_upstream_from_auth_context(request.context.as_ref());
    let client_ip = client_ip_for_auth(&headers);
    let access_mode = requested_access_mode(&headers);
    let mut response = empty_authorize_http_response();

    let config = state.storage.store.config_snapshot();
    let translator = translator_from_config(&config);
    let matched_rule_valid = request
        .subdomain_rule_match
        .as_ref()
        .is_some_and(|matched| {
            subdomain_grant::match_is_valid(
                &config,
                resolve_request_hostname_from_headers(&headers)
                    .as_deref()
                    .unwrap_or(""),
                matched,
            )
        });
    let existing_rule_access = if matched_rule_valid {
        false
    } else if subdomain_grant::has_valid_probe(&state, &headers, &config) {
        // Probe validation is stateless. Keep its exchange path available even
        // when persistent credential storage is temporarily unavailable.
        true
    } else {
        match subdomain_grant::inspect_existing(&state, &headers, &config).await {
            Ok(grant) => grant.is_some(),
            Err(error) => {
                tracing::warn!(%error, "auth bridge existing subdomain grant inspection failed");
                false
            }
        }
    };
    // A rule grant is host-scoped and must not refresh a broader FNOS/session
    // IP grant as a side effect of resolving normal access.
    let normal_access = if matched_rule_valid || existing_rule_access {
        PreflightNormalAccess::default()
    } else {
        match resolve_preflight_normal_access(
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
                return authorize_http_preparation_error(&translator, run_preflight, run_verify);
            }
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
            routed_upstream,
            routed_upstream_host,
            routed_upstream_route_id,
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
        let mut preflight = preflight_auth_response_from_http(&preflight);
        preflight_rejected = preflight_rejects_request(&preflight);
        // A validated subdomain-rule match may proceed to the verify stage so
        // Rust can issue the host-only grant. Protective preflight denials
        // (blacklist/WAF/strict whitelist) remain fail-closed.
        if preflight_rejected && matched_rule_valid && !preflight.deny {
            preflight_rejected = false;
            preflight.redirect_location.clear();
            preflight.access_denied_reason.clear();
        }
        response.preflight = Some(preflight);
    }

    if run_verify && !preflight_rejected {
        match resolve_auth_access_with_normal_access_and_rule_match(
            &state,
            &headers,
            &uri,
            &translator,
            &config,
            &client_ip,
            &normal_access,
            request.subdomain_rule_match.as_ref(),
            routed_upstream,
            routed_upstream_host,
            routed_upstream_route_id,
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

fn authorize_http_preparation_error(
    translator: &Translator,
    run_preflight: bool,
    run_verify: bool,
) -> AuthorizeHttpResponse {
    let mut response = empty_authorize_http_response();
    if run_preflight {
        response.preflight = Some(preflight_auth_response_from_http(&new_preflight_response()));
    }
    if run_verify {
        response.verify = Some(verify_auth_error_response(translator));
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
    let (routed_upstream, routed_upstream_host, routed_upstream_route_id) =
        routed_upstream_from_auth_context(request.context.as_ref());
    let config = state.storage.store.config_snapshot();
    let translator = translator_from_config(&config);
    match resolve_auth_access_with_routed_upstream_and_config(
        &state,
        &headers,
        &uri,
        &translator,
        &config,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
    {
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
    let grant_type = access.grant_type.as_deref();
    let is_rule_grant = matches!(grant_type, Some("subdomain_rule" | "subdomain_rule_login"));
    let rule_group = access.response_headers.iter().find_map(|(key, value)| {
        key.eq_ignore_ascii_case("X-Reauth-Auth-Rule-Group")
            .then_some(value.clone())
    });
    let grant_state = access.response_headers.iter().find_map(|(key, value)| {
        key.eq_ignore_ascii_case("X-Reauth-Auth-Grant-State")
            .then_some(value.clone())
    });
    let rule_cache_max_age = access.response_headers.iter().find_map(|(key, value)| {
        key.eq_ignore_ascii_case("X-Reauth-Auth-Cache-Max-Age")
            .then(|| value.trim().parse::<i32>().ok())
            .flatten()
    });
    VerifyAuthResponse {
        success: access.authenticated,
        message: access.message,
        status: status.as_u16() as i32,
        set_cookies: access.set_cookies,
        suppress_toolbar: is_rule_grant || grant_type == Some("fnos_share"),
        redirect_location: String::new(),
        access_denied_reason: access.deny_reason.unwrap_or_default(),
        response_headers: headers_from_pairs(&access.response_headers),
        grant_kind: auth_grant_kind(grant_type),
        decision: auth_decision(grant_type, access.authenticated),
        login_authenticated: grant_type != Some("subdomain_rule"),
        host_authorized: access.authenticated,
        cache_max_age_seconds: if is_rule_grant {
            rule_cache_max_age.unwrap_or(0).clamp(0, 60)
        } else {
            0
        },
        auth_rule_group_id: rule_group.unwrap_or_default(),
        auth_grant_state: grant_state.unwrap_or_default(),
    }
}

fn auth_grant_kind(grant_type: Option<&str>) -> i32 {
    match grant_type {
        Some("fnos_share") => crate::grpc_proto::AuthGrantKind::FnosShare as i32,
        Some("subdomain_rule" | "subdomain_rule_login") => {
            crate::grpc_proto::AuthGrantKind::SubdomainRule as i32
        }
        Some(_) => crate::grpc_proto::AuthGrantKind::Login as i32,
        None => crate::grpc_proto::AuthGrantKind::Unspecified as i32,
    }
}

fn auth_decision(grant_type: Option<&str>, authenticated: bool) -> String {
    match grant_type {
        Some("subdomain_rule" | "subdomain_rule_login") => "subdomain_rule_allowed".to_string(),
        Some("fnos_share") | Some(_) if authenticated => "passed".to_string(),
        _ => String::new(),
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
        grant_kind: crate::grpc_proto::AuthGrantKind::Unspecified as i32,
        decision: String::new(),
        login_authenticated: false,
        host_authorized: false,
        cache_max_age_seconds: 0,
        auth_rule_group_id: String::new(),
        auth_grant_state: String::new(),
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
    let (routed_upstream, routed_upstream_host, routed_upstream_route_id) =
        routed_upstream_from_auth_context(request.context.as_ref());
    let mut response = new_preflight_response();

    let config = state.storage.store.config_snapshot();
    if let Err(error) = apply_preflight_behavior_with_routed_upstream_and_config(
        &state,
        &headers,
        &uri,
        &mut response,
        &config,
        routed_upstream,
        routed_upstream_host,
        routed_upstream_route_id,
    )
    .await
    {
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
    let config = state.storage.store.config_snapshot();
    let translator = translator_from_config(&config);

    let session_access = match resolve_stream_session_access(
        &state,
        &request.client_ip,
        &request.protocol,
        request.listen_port,
    )
    .await
    {
        Ok(access) => access,
        Err(error) => {
            tracing::warn!(%error, "auth bridge stream session access resolution failed");
            return VerifyStreamAuthResponse {
                allowed: false,
                status: StatusCode::BAD_GATEWAY.as_u16() as i32,
                decision: "auth_error".to_string(),
                message: "Authentication Service Unavailable".to_string(),
            };
        }
    };
    if session_access.allowed {
        return VerifyStreamAuthResponse {
            allowed: true,
            status: StatusCode::OK.as_u16() as i32,
            decision: "passed".to_string(),
            message: auth_route_text(&translator, "authenticated"),
        };
    }

    match resolve_auth_access_with_routed_upstream_and_config(
        &state,
        &headers,
        &uri,
        &translator,
        &config,
        None,
        None,
        None,
    )
    .await
    {
        Ok(access)
            if access.authenticated
                && !(session_access.has_custom_owner
                    && access.grant_type.as_deref() == Some("login_ip_grant")) =>
        {
            VerifyStreamAuthResponse {
                allowed: true,
                status: StatusCode::OK.as_u16() as i32,
                decision: "passed".to_string(),
                message: access.message,
            }
        }
        Ok(access) if access.authenticated => VerifyStreamAuthResponse {
            allowed: false,
            status: StatusCode::FORBIDDEN.as_u16() as i32,
            decision: "access_denied".to_string(),
            message: "Access denied by credential scope".to_string(),
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

#[derive(Debug, Default, PartialEq, Eq)]
struct StreamSessionAccess {
    allowed: bool,
    has_custom_owner: bool,
}

async fn resolve_stream_session_access(
    state: &AppState,
    client_ip: &str,
    protocol: &str,
    listen_port: i32,
) -> anyhow::Result<StreamSessionAccess> {
    let mut result = StreamSessionAccess::default();
    for (_session_id, session) in
        auth_mobility::list_stream_access_sessions_by_ip(state, client_ip).await?
    {
        if login_session_has_expired(&session) {
            continue;
        }
        let Some(credential) = session_auth_credential(state, &session).await? else {
            continue;
        };
        result.has_custom_owner |= credential
            .subdomain_access
            .get("mode")
            .and_then(Value::as_str)
            == Some("custom");
        if !session_has_active_stream_access(&session) {
            continue;
        }
        if is_stream_allowed_by_totp_subdomain_access(
            &credential.subdomain_access,
            protocol,
            listen_port,
        ) {
            result.allowed = true;
            return Ok(result);
        }
    }
    Ok(result)
}

fn session_has_active_stream_access(session: &LoginSession) -> bool {
    match session.stream_access_expires_at.as_deref() {
        Some(expires_at) => time_utils::parse_iso_ms(expires_at)
            .is_some_and(|expires_at| expires_at > time_utils::now_ms()),
        None => {
            session.grant_type.as_deref() == Some("login_ip_grant")
                && session.post_login_ip_grant_mode.as_deref() == Some("follow_session")
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
    // The authenticated bridge's canonical client_ip is the only address
    // assertion consumed by auth. Never let copied request headers or a raw
    // forwarding chain override it.
    for name in [
        "x-forwarded-for",
        "x-real-ip",
        "eo-connecting-ip",
        "ali-real-client-ip",
    ] {
        headers.remove(name);
    }
    let client_ip = http_utils::normalize_session_client_ip(&context.client_ip);
    insert_header(&mut headers, "X-Real-IP", &client_ip);
    insert_header(&mut headers, "X-Forwarded-For", &client_ip);
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

fn routed_upstream_from_auth_context(
    context: Option<&AuthContext>,
) -> (Option<&str>, Option<&str>, Option<&str>) {
    context.map_or((None, None, None), |context| {
        (
            context.routed_upstream.as_deref(),
            context.routed_upstream_host.as_deref(),
            context.routed_upstream_route_id.as_deref(),
        )
    })
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
    fn bridge_deadline_keeps_response_headroom() {
        assert_eq!(
            auth_bridge_handler_timeout(Duration::from_secs(5)),
            Duration::from_millis(4_750)
        );
        assert_eq!(
            auth_bridge_handler_timeout(Duration::from_millis(50)),
            Duration::from_millis(40)
        );
        assert_eq!(
            auth_bridge_handler_timeout(Duration::from_millis(2)),
            Duration::from_millis(1)
        );
        assert_eq!(auth_bridge_handler_timeout(Duration::ZERO), Duration::ZERO);
    }

    #[test]
    fn propagated_bridge_deadline_includes_time_spent_before_rust_dispatch() {
        let now = tokio::time::Instant::now();
        let wall_now = UNIX_EPOCH + Duration::from_secs(1_000);
        let received_at = now - Duration::from_secs(1);
        let message = AuthBridgeEnvelope {
            deadline_unix_millis: 1_002_000,
            ..Default::default()
        };

        let deadlines = auth_bridge_request_deadlines_at(
            &message,
            received_at,
            Duration::from_secs(5),
            now,
            wall_now,
        );
        assert_eq!(deadlines.response, now + Duration::from_secs(2));
        assert_eq!(deadlines.handler, now + Duration::from_millis(1_750));
    }

    #[test]
    fn bridge_deadline_never_exceeds_local_safety_cap() {
        let now = tokio::time::Instant::now();
        let wall_now = UNIX_EPOCH + Duration::from_secs(1_000);
        let received_at = now - Duration::from_secs(1);
        let message = AuthBridgeEnvelope {
            deadline_unix_millis: 1_030_000,
            ..Default::default()
        };

        let deadlines = auth_bridge_request_deadlines_at(
            &message,
            received_at,
            Duration::from_secs(5),
            now,
            wall_now,
        );
        assert_eq!(deadlines.response, now + Duration::from_secs(4));
        assert_eq!(deadlines.handler, now + Duration::from_millis(3_750));
    }

    #[test]
    fn expired_bridge_deadline_is_not_reset_on_receipt() {
        let now = tokio::time::Instant::now();
        let wall_now = UNIX_EPOCH + Duration::from_secs(1_000);
        let message = AuthBridgeEnvelope {
            deadline_unix_millis: 999_000,
            ..Default::default()
        };

        let deadlines =
            auth_bridge_request_deadlines_at(&message, now, Duration::from_secs(5), now, wall_now);
        assert_eq!(deadlines.response, now);
        assert!(deadlines.handler <= now);
    }

    #[tokio::test]
    async fn bridge_capacity_waits_for_an_in_flight_request() {
        let limiter = Arc::new(Semaphore::new(1));
        let first = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("first bridge permit");
        let waiting_limiter = limiter.clone();
        let waiter = tokio::spawn(async move {
            acquire_auth_bridge_capacity(
                waiting_limiter,
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await
        });

        tokio::task::yield_now().await;
        assert!(!waiter.is_finished());
        drop(first);

        let second = tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("capacity waiter deadline")
            .expect("capacity waiter task")
            .expect("second bridge permit");
        drop(second);
    }

    #[tokio::test]
    async fn bridge_capacity_wait_respects_its_budget() {
        let limiter = Arc::new(Semaphore::new(1));
        let _first = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("first bridge permit");

        assert!(
            acquire_auth_bridge_capacity(
                limiter,
                tokio::time::Instant::now() + Duration::from_millis(20),
            )
            .await
            .is_none()
        );
    }

    #[tokio::test]
    async fn bridge_capacity_waiters_share_one_absolute_deadline() {
        let limiter = Arc::new(Semaphore::new(1));
        let _first = limiter
            .clone()
            .acquire_owned()
            .await
            .expect("first bridge permit");
        let shared_deadline = tokio::time::Instant::now() + Duration::from_millis(60);

        let first_waiter = tokio::spawn(acquire_auth_bridge_capacity(
            limiter.clone(),
            shared_deadline,
        ));
        tokio::time::sleep(Duration::from_millis(30)).await;
        let second_started = tokio::time::Instant::now();
        let second_waiter = tokio::spawn(acquire_auth_bridge_capacity(limiter, shared_deadline));

        assert!(first_waiter.await.expect("first waiter task").is_none());
        assert!(second_waiter.await.expect("second waiter task").is_none());
        assert!(
            second_started.elapsed() < Duration::from_millis(55),
            "later waiter received a fresh timeout instead of the shared deadline"
        );
    }

    #[tokio::test]
    async fn bridge_response_send_respects_its_headroom_budget() {
        let (tx, mut rx) = mpsc::channel(1);
        let shutdown = CancellationToken::new();
        tx.send(AuthBridgeEnvelope::default())
            .await
            .expect("fill bridge response queue");

        assert_eq!(
            send_auth_bridge_response(
                &tx,
                AuthBridgeEnvelope::default(),
                &shutdown,
                tokio::time::Instant::now() + Duration::from_millis(20),
            )
            .await,
            BridgeResponseSendOutcome::TimedOut
        );

        rx.recv().await.expect("drain bridge response queue");
        assert_eq!(
            send_auth_bridge_response(
                &tx,
                AuthBridgeEnvelope::default(),
                &shutdown,
                tokio::time::Instant::now() + Duration::from_secs(1),
            )
            .await,
            BridgeResponseSendOutcome::Sent
        );
    }

    #[tokio::test]
    async fn expired_bridge_response_does_not_consume_queue_capacity() {
        let (tx, mut rx) = mpsc::channel(1);
        let shutdown = CancellationToken::new();

        assert_eq!(
            send_auth_bridge_response(
                &tx,
                AuthBridgeEnvelope::default(),
                &shutdown,
                tokio::time::Instant::now(),
            )
            .await,
            BridgeResponseSendOutcome::TimedOut
        );
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn bridge_unavailable_responses_are_fail_closed_and_uncacheable() {
        let envelope = BridgeRequestKind::AuthorizeHttp {
            run_preflight: true,
            run_verify: true,
        }
        .unavailable_envelope("request-1".to_string());
        let Some(auth_bridge_envelope::Payload::AuthorizeHttpResponse(response)) = envelope.payload
        else {
            panic!("expected authorize response");
        };
        let preflight = response.preflight.expect("preflight response");
        let verify = response.verify.expect("verify response");
        assert!(preflight.deny);
        assert_eq!(preflight.access_denied_reason, "auth_service_unavailable");
        assert!(!verify.success);
        assert_eq!(
            verify.status,
            StatusCode::SERVICE_UNAVAILABLE.as_u16() as i32
        );
        assert_eq!(verify.decision, "auth_unavailable");
        assert_eq!(verify.cache_max_age_seconds, 0);
        assert!(preflight.response_headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("retry-after")
                && header.values.iter().any(|value| value == "1")
        }));
        assert!(verify.response_headers.iter().any(|header| {
            header.name.eq_ignore_ascii_case("retry-after")
                && header.values.iter().any(|value| value == "1")
        }));
        assert_eq!(response.preflight_cache_scope, AuthCacheScope::None as i32);
        assert_eq!(response.verify_cache_scope, AuthCacheScope::None as i32);

        let stream =
            BridgeRequestKind::VerifyStreamAuth.unavailable_envelope("request-2".to_string());
        let Some(auth_bridge_envelope::Payload::VerifyStreamAuthResponse(stream)) = stream.payload
        else {
            panic!("expected stream response");
        };
        assert!(!stream.allowed);
        assert_eq!(
            stream.status,
            StatusCode::SERVICE_UNAVAILABLE.as_u16() as i32
        );
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
    fn auth_context_client_ip_cannot_be_overridden_by_forwarding_headers() {
        let context = AuthContext {
            client_ip: "203.0.113.10".to_string(),
            forwarded_for: "127.0.0.1, 203.0.113.10".to_string(),
            extra_headers: vec![
                Header {
                    name: "X-Forwarded-For".to_string(),
                    values: vec!["192.168.1.10".to_string()],
                },
                Header {
                    name: "EO-Connecting-IP".to_string(),
                    values: vec!["::1".to_string()],
                },
            ],
            ..Default::default()
        };

        let headers = headers_from_auth_context(Some(&context));
        assert_eq!(headers["x-forwarded-for"], "203.0.113.10");
        assert_eq!(headers["x-real-ip"], "203.0.113.10");
        assert!(!headers.contains_key("eo-connecting-ip"));
        assert!(!headers.contains_key("ali-real-client-ip"));
        assert_eq!(client_ip_for_auth(&headers), "203.0.113.10");

        let invalid_context = AuthContext {
            client_ip: "::1".to_string(),
            forwarded_for: "203.0.113.10".to_string(),
            extra_headers: vec![Header {
                name: "X-Real-IP".to_string(),
                values: vec!["198.51.100.20".to_string()],
            }],
            ..Default::default()
        };
        let invalid_headers = headers_from_auth_context(Some(&invalid_context));
        assert_eq!(client_ip_for_auth(&invalid_headers), "");
        assert!(!invalid_headers.contains_key("x-forwarded-for"));
        assert!(!invalid_headers.contains_key("x-real-ip"));
    }

    #[test]
    fn routed_backend_metadata_preserves_optional_presence() {
        let context = AuthContext {
            routed_upstream: Some("http://10.0.0.8:5666/fnos".to_string()),
            routed_upstream_host: Some("nas.example.com".to_string()),
            routed_upstream_route_id: Some("route-generation-a".to_string()),
            ..Default::default()
        };
        assert_eq!(
            routed_upstream_from_auth_context(Some(&context)),
            (
                Some("http://10.0.0.8:5666/fnos"),
                Some("nas.example.com"),
                Some("route-generation-a")
            )
        );
        assert_eq!(routed_upstream_from_auth_context(None), (None, None, None));
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

    #[tokio::test]
    async fn matched_upgrade_rule_is_transient_and_does_not_refresh_session_ip_mobility() {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = "rule-mobility-isolation-test".to_string();
        let state = AppState::new(settings).await.expect("auth test state");
        state
            .storage
            .store
            .save_config(&json!({
                "run_type": 3,
                "auth_credential_settings": {
                    "post_login_ip_grant_mode": "follow_session",
                    "session_ip_mobility_enabled": true,
                    "session_ip_mobility_window_seconds": 1_200
                },
                "host_mappings": [{
                    "host": "allowed.example.com",
                    "use_auth": true,
                    "advanced_auth": {
                        "enabled": true,
                        "idle_ttl_seconds": 86_400,
                        "max_lifetime_seconds": 2_592_000,
                        "policy_version": "policy-v1",
                        "groups": [{"id": "group-v1", "conditions": []}]
                    }
                }]
            }))
            .await
            .expect("auth config");
        state
            .storage
            .store
            .add_totp(TotpCredential {
                id: "totp-1".to_string(),
                secret: "SECRET".to_string(),
                comment: "unrestricted".to_string(),
                created_at: time_utils::now_iso(),
                access_scopes: json!([]),
                subdomain_access: json!({"mode": "all", "hosts": []}),
            })
            .await
            .expect("TOTP credential");
        let session = LoginSession {
            totp_id: "totp-1".to_string(),
            method: "TOTP".to_string(),
            credential_id: "totp-1".to_string(),
            credential_name: "unrestricted".to_string(),
            linked_totp_name: None,
            access_scopes: None,
            subdomain_access: None,
            grant_type: Some("browser_session".to_string()),
            post_login_ip_grant_mode: None,
            post_login_ip_grant_record_id: None,
            stream_access_expires_at: None,
            comment: None,
            ip: "203.0.113.10".to_string(),
            user_agent: "rule-test".to_string(),
            login_time: time_utils::now_iso(),
            expires_at: Some(time_utils::iso_after_seconds(3_600)),
            ip_location: None,
        };
        state
            .storage
            .store
            .add_session("session-1", &session, 3_600)
            .await
            .expect("login session");

        let response = handle_authorize_http(
            state.clone(),
            AuthorizeHttpRequest {
                context: Some(AuthContext {
                    client_ip: "203.0.113.20".to_string(),
                    forwarded_host: "allowed.example.com".to_string(),
                    forwarded_proto: "https".to_string(),
                    path: "/websocket".to_string(),
                    cookie: format!("{}=session-1", cookies::SESSION_COOKIE_NAME),
                    extra_headers: vec![Header {
                        name: "Upgrade".to_string(),
                        values: vec!["websocket".to_string()],
                    }],
                    ..Default::default()
                }),
                matched: true,
                mode: HttpAuthMode::PreflightAndVerify as i32,
                subdomain_rule_match: Some(crate::grpc_proto::SubdomainRuleMatch {
                    host: "allowed.example.com".to_string(),
                    policy_version: "policy-v1".to_string(),
                    group_id: "group-v1".to_string(),
                }),
            },
        )
        .await;

        let verify = response.verify.expect("verify response");
        assert!(verify.success);
        assert_eq!(verify.decision, "subdomain_rule_allowed");
        assert_eq!(verify.auth_grant_state, "transient");
        assert_eq!(verify.cache_max_age_seconds, 0);
        assert!(verify.set_cookies.is_empty());
        assert!(
            state
                .storage
                .store
                .list_auth_mobility_recent_active_ip_details("session-1", 0)
                .await
                .expect("active IP details")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn returned_cookie_probe_bypasses_preflight_and_becomes_one_persistent_grant() {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = "rule-cookie-probe-round-trip".to_string();
        let state = AppState::new(settings).await.expect("auth test state");
        state
            .storage
            .store
            .save_config(&json!({
                "run_type": 3,
                "host_mappings": [{
                    "host": "allowed.example.com",
                    "use_auth": true,
                    "advanced_auth": {
                        "enabled": true,
                        "idle_ttl_seconds": 86_400,
                        "max_lifetime_seconds": 2_592_000,
                        "policy_version": "policy-v1",
                        "groups": [{"id": "group-v1", "conditions": []}]
                    }
                }]
            }))
            .await
            .expect("auth config");

        let first = handle_authorize_http(
            state.clone(),
            AuthorizeHttpRequest {
                context: Some(AuthContext {
                    client_ip: "203.0.113.20".to_string(),
                    forwarded_host: "allowed.example.com".to_string(),
                    forwarded_proto: "https".to_string(),
                    path: "/entry".to_string(),
                    ..Default::default()
                }),
                matched: true,
                mode: HttpAuthMode::PreflightAndVerify as i32,
                subdomain_rule_match: Some(crate::grpc_proto::SubdomainRuleMatch {
                    host: "allowed.example.com".to_string(),
                    policy_version: "policy-v1".to_string(),
                    group_id: "group-v1".to_string(),
                }),
            },
        )
        .await;
        let first_preflight = first.preflight.as_ref().expect("first preflight response");
        assert!(first_preflight.redirect_location.is_empty());
        let first_verify = first.verify.expect("first verify response");
        assert!(first_verify.success);
        assert_eq!(first_verify.auth_grant_state, "transient");
        assert_eq!(first_verify.set_cookies.len(), 1);
        let probe_cookie = first_verify.set_cookies[0]
            .split(';')
            .next()
            .expect("probe cookie pair")
            .to_string();
        assert!(probe_cookie.starts_with(&format!(
            "{}=p1.",
            cookies::SUBDOMAIN_RULE_GRANT_COOKIE_NAME
        )));

        // The entry-only rule no longer matches this path. A client that
        // returned the signed probe must still pass the combined preflight so
        // verify can exchange it for the persistent host-scoped credential.
        let second = handle_authorize_http(
            state.clone(),
            AuthorizeHttpRequest {
                context: Some(AuthContext {
                    client_ip: "203.0.113.20".to_string(),
                    forwarded_host: "allowed.example.com".to_string(),
                    forwarded_proto: "https".to_string(),
                    path: "/private".to_string(),
                    cookie: probe_cookie,
                    ..Default::default()
                }),
                matched: false,
                mode: HttpAuthMode::PreflightAndVerify as i32,
                subdomain_rule_match: None,
            },
        )
        .await;
        let second_preflight = second
            .preflight
            .as_ref()
            .expect("second preflight response");
        assert!(second_preflight.redirect_location.is_empty());
        assert!(second_preflight.access_denied_reason.is_empty());
        let second_verify = second.verify.expect("second verify response");
        assert!(second_verify.success);
        assert_eq!(second_verify.auth_grant_state, "issued");
        assert_eq!(second_verify.set_cookies.len(), 1);
        assert!(!second_verify.set_cookies[0].contains("=p1."));
        assert_eq!(
            state
                .storage
                .store
                .count_keys_by_prefix("fn_knock:auth:subdomain_rule_grant:")
                .await
                .expect("grant count"),
            1
        );
    }

    #[test]
    fn stream_access_expiry_supports_new_and_legacy_sessions() {
        let mut session =
            crate::store::new_login_session("totp-1", "TOTP", "203.0.113.10", "test", 3600);
        assert!(!session_has_active_stream_access(&session));

        session.stream_access_expires_at = Some(time_utils::iso_after_seconds(60));
        assert!(session_has_active_stream_access(&session));

        session.stream_access_expires_at = Some("2000-01-01T00:00:00Z".to_string());
        assert!(!session_has_active_stream_access(&session));

        session.stream_access_expires_at = None;
        session.grant_type = Some("login_ip_grant".to_string());
        session.post_login_ip_grant_mode = Some("follow_session".to_string());
        assert!(session_has_active_stream_access(&session));

        session.post_login_ip_grant_mode = Some("custom".to_string());
        assert!(!session_has_active_stream_access(&session));
    }

    #[tokio::test]
    async fn stream_scope_keeps_canonical_ip_and_expires_only_drift_ips_with_mobility() {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = "stream-scope-test".to_string();
        let state = AppState::new(settings).await.expect("auth test state");
        state
            .storage
            .store
            .save_config(&json!({
                "auth_credential_settings": {
                    "post_login_ip_grant_mode": "follow_session",
                    "session_ip_mobility_enabled": true,
                    "session_ip_mobility_window_seconds": 60
                }
            }))
            .await
            .expect("auth config");
        let credential = TotpCredential {
            id: "totp-1".to_string(),
            secret: "SECRET".to_string(),
            comment: "stream token".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: json!([]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": [],
                "streams": [{ "protocol": "tcp", "listen_port": 2222 }]
            }),
        };
        state
            .storage
            .store
            .add_totp(credential.clone())
            .await
            .expect("TOTP credential");
        let session = LoginSession {
            totp_id: credential.id.clone(),
            method: "TOTP".to_string(),
            credential_id: credential.id.clone(),
            credential_name: credential.comment.clone(),
            linked_totp_name: None,
            access_scopes: None,
            subdomain_access: None,
            grant_type: Some("browser_session".to_string()),
            post_login_ip_grant_mode: None,
            post_login_ip_grant_record_id: None,
            stream_access_expires_at: None,
            comment: None,
            ip: "203.0.113.10".to_string(),
            user_agent: "stream-test".to_string(),
            login_time: time_utils::now_iso(),
            expires_at: Some(time_utils::iso_after_seconds(3600)),
            ip_location: None,
        };
        state
            .storage
            .store
            .add_session("session-1", &session, 3600)
            .await
            .expect("login session");
        let now = time_utils::now_ms() / 1000;
        assert!(
            state
                .storage
                .store
                .save_auth_mobility_active_ip_detail(
                    "session-1",
                    "203.0.113.10",
                    now - 60,
                    &json!({
                        "version": 1,
                        "ip": "203.0.113.10",
                        "firstSeenAt": now - 60,
                        "lastSeenAt": now - 60,
                        "source": "browser-session"
                    }),
                    3_600,
                )
                .await
                .expect("expired canonical active IP")
        );

        auth_mobility::reconcile_stream_access_grants_for_totp_credential(
            &state,
            &credential.id,
            &credential.subdomain_access,
        )
        .await
        .expect("custom stream grant reconciliation");

        assert!(
            resolve_stream_session_access(&state, "203.0.113.10", "tcp", 2222)
                .await
                .expect("allowed stream lookup")
                .allowed
        );
        let denied = resolve_stream_session_access(&state, "203.0.113.10", "udp", 2222)
            .await
            .expect("denied stream lookup");
        assert!(!denied.allowed);
        assert!(denied.has_custom_owner);

        let mut updates = serde_json::Map::new();
        updates.insert("ip".to_string(), Value::String("203.0.113.11".to_string()));
        state
            .storage
            .store
            .update_session_value("session-1", updates)
            .await
            .expect("session IP update")
            .expect("live session");

        assert!(
            resolve_stream_session_access(&state, "203.0.113.11", "tcp", 2222)
                .await
                .expect("new canonical IP lookup")
                .allowed
        );
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.10", "tcp", 2222)
                .await
                .expect("unregistered drift IP lookup")
                .allowed
        );

        let drift_detail = json!({
            "version": 1,
            "ip": "203.0.113.10",
            "firstSeenAt": now,
            "lastSeenAt": now,
            "source": "browser-session"
        });
        assert!(
            state
                .storage
                .store
                .save_auth_mobility_active_ip_detail(
                    "session-1",
                    "203.0.113.10",
                    now,
                    &drift_detail,
                    3_600,
                )
                .await
                .expect("recent drift IP")
        );
        assert!(
            resolve_stream_session_access(&state, "203.0.113.10", "tcp", 2222)
                .await
                .expect("recent drift IP lookup")
                .allowed
        );

        assert!(
            state
                .storage
                .store
                .save_auth_mobility_active_ip_detail(
                    "session-1",
                    "203.0.113.10",
                    now - 60,
                    &json!({
                        "version": 1,
                        "ip": "203.0.113.10",
                        "firstSeenAt": now - 60,
                        "lastSeenAt": now - 60,
                        "source": "browser-session"
                    }),
                    3_600,
                )
                .await
                .expect("expired drift IP")
        );
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.10", "tcp", 2222)
                .await
                .expect("expired drift IP lookup")
                .allowed
        );
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.10", "tcp", 2222)
                .await
                .expect("expired drift IP repeat lookup")
                .allowed
        );

        let all_access = json!({ "mode": "all", "hosts": [], "streams": [] });
        state
            .storage
            .store
            .update_totp_subdomain_access(&credential.id, all_access.clone())
            .await
            .expect("all-scope update")
            .expect("existing TOTP");
        auth_mobility::reconcile_stream_access_grants_for_totp_credential(
            &state,
            &credential.id,
            &all_access,
        )
        .await
        .expect("all-scope stream grant reconciliation");

        assert!(
            resolve_stream_session_access(&state, "203.0.113.11", "udp", 5353)
                .await
                .expect("all-scope stream lookup")
                .allowed
        );

        let mut updates = serde_json::Map::new();
        updates.insert(
            "expiresAt".to_string(),
            Value::String("2000-01-01T00:00:00Z".to_string()),
        );
        state
            .storage
            .store
            .update_session_value("session-1", updates)
            .await
            .expect("session expiry update")
            .expect("live session key");
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.11", "udp", 5353)
                .await
                .expect("expired session lookup")
                .allowed
        );
    }

    #[tokio::test]
    async fn canonical_stream_ip_respects_grant_mode_credentials_ipv6_and_session_union() {
        let directory = tempfile::tempdir().expect("temporary auth database");
        let mut settings = {
            let _environment = crate::test_support::EnvGuard::new(&[]);
            crate::settings::Settings::from_env()
        };
        settings.data_dir = directory.path().join("data");
        settings.gateway_config_dir = directory.path().join("gateway");
        settings.sqlite_path = directory.path().join("fn-knock.sqlite3");
        settings.legacy_redis_url = String::new();
        settings.internal_rpc_token = "stream-canonical-policy-test".to_string();
        let state = AppState::new(settings).await.expect("auth test state");
        state
            .storage
            .store
            .save_config(&json!({
                "auth_credential_settings": {
                    "post_login_ip_grant_mode": "follow_session",
                    "session_ip_mobility_enabled": true,
                    "session_ip_mobility_window_seconds": 60
                },
                "host_mappings": [{
                    "host": "protected.example.com",
                    "use_auth": true
                }]
            }))
            .await
            .expect("auth config");

        let tcp_credential = TotpCredential {
            id: "totp-tcp".to_string(),
            secret: "TCP".to_string(),
            comment: "TCP only".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: json!([]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": ["protected.example.com"],
                "streams": [{ "protocol": "tcp", "listen_port": 2222 }]
            }),
        };
        let udp_credential = TotpCredential {
            id: "totp-udp".to_string(),
            secret: "UDP".to_string(),
            comment: "UDP only".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: json!([]),
            subdomain_access: json!({
                "mode": "custom",
                "hosts": [],
                "streams": [{ "protocol": "udp", "listen_port": 5353 }]
            }),
        };
        let all_credential = TotpCredential {
            id: "totp-all".to_string(),
            secret: "ALL".to_string(),
            comment: "All streams".to_string(),
            created_at: time_utils::now_iso(),
            access_scopes: json!([]),
            subdomain_access: json!({ "mode": "all", "hosts": [], "streams": [] }),
        };
        for credential in [&tcp_credential, &udp_credential, &all_credential] {
            state
                .storage
                .store
                .add_totp(credential.clone())
                .await
                .expect("TOTP credential");
        }

        let session_for = |credential: &TotpCredential, ip: &str| LoginSession {
            totp_id: credential.id.clone(),
            method: "TOTP".to_string(),
            credential_id: credential.id.clone(),
            credential_name: credential.comment.clone(),
            linked_totp_name: None,
            access_scopes: None,
            subdomain_access: None,
            grant_type: Some("login_ip_grant".to_string()),
            post_login_ip_grant_mode: Some("custom".to_string()),
            post_login_ip_grant_record_id: None,
            stream_access_expires_at: Some(time_utils::iso_after_seconds(600)),
            comment: None,
            ip: ip.to_string(),
            user_agent: "stream-test".to_string(),
            login_time: time_utils::now_iso(),
            expires_at: Some(time_utils::iso_after_seconds(3_600)),
            ip_location: None,
        };

        let ipv6 = "[2001:0db8:0:0::10]";
        state
            .storage
            .store
            .add_session("session-tcp", &session_for(&tcp_credential, ipv6), 3_600)
            .await
            .expect("TCP session");
        assert!(
            resolve_stream_session_access(&state, "2001:db8::10", "tcp", 2222)
                .await
                .expect("normalized IPv6 stream lookup")
                .allowed
        );

        let mut updates = serde_json::Map::new();
        updates.insert(
            "streamAccessExpiresAt".to_string(),
            Value::String("2000-01-01T00:00:00Z".to_string()),
        );
        state
            .storage
            .store
            .update_session_value("session-tcp", updates)
            .await
            .expect("custom grant expiry update")
            .expect("live TCP session");
        let expired_custom = resolve_stream_session_access(&state, "2001:db8::10", "tcp", 2222)
            .await
            .expect("expired custom stream lookup");
        assert!(!expired_custom.allowed);
        assert!(expired_custom.has_custom_owner);

        let protected_subdomain = handle_authorize_http(
            state.clone(),
            AuthorizeHttpRequest {
                context: Some(AuthContext {
                    client_ip: "2001:db8::10".to_string(),
                    forwarded_host: "protected.example.com".to_string(),
                    forwarded_proto: "https".to_string(),
                    path: "/".to_string(),
                    cookie: format!("{}=session-tcp", cookies::SESSION_COOKIE_NAME),
                    ..Default::default()
                }),
                matched: false,
                mode: HttpAuthMode::PreflightAndVerify as i32,
                subdomain_rule_match: None,
            },
        )
        .await;
        let protected_preflight = protected_subdomain
            .preflight
            .expect("protected subdomain preflight");
        assert!(protected_preflight.redirect_location.is_empty());
        assert!(protected_preflight.access_denied_reason.is_empty());
        assert!(
            protected_subdomain
                .verify
                .expect("protected subdomain verify")
                .success
        );
        assert!(
            !resolve_stream_session_access(&state, "2001:db8::10", "tcp", 2222)
                .await
                .expect("expired custom stream lookup after protected subdomain access")
                .allowed
        );
        let refreshed_tcp_session = state
            .storage
            .store
            .get_session("session-tcp")
            .await
            .expect("refreshed TCP session lookup")
            .expect("live TCP session");
        assert_eq!(refreshed_tcp_session.ip, "2001:db8::10");
        assert_eq!(
            refreshed_tcp_session.stream_access_expires_at.as_deref(),
            Some("2000-01-01T00:00:00Z")
        );

        state
            .storage
            .store
            .add_session("session-udp", &session_for(&udp_credential, ipv6), 3_600)
            .await
            .expect("UDP session");
        assert!(
            resolve_stream_session_access(&state, "2001:db8::10", "udp", 5353)
                .await
                .expect("same-IP session union lookup")
                .allowed
        );
        assert!(
            !resolve_stream_session_access(&state, "2001:db8::10", "tcp", 2222)
                .await
                .expect("same-IP scoped denial")
                .allowed
        );

        let mut disabled = session_for(&all_credential, "203.0.113.20");
        disabled.grant_type = Some("browser_session".to_string());
        disabled.post_login_ip_grant_mode = None;
        disabled.stream_access_expires_at = None;
        state
            .storage
            .store
            .add_session("session-disabled", &disabled, 3_600)
            .await
            .expect("disabled stream session");
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.20", "tcp", 2222)
                .await
                .expect("disabled stream lookup")
                .allowed
        );

        let mut legacy_follow = session_for(&all_credential, "203.0.113.40");
        legacy_follow.post_login_ip_grant_mode = Some("follow_session".to_string());
        legacy_follow.stream_access_expires_at = None;
        state
            .storage
            .store
            .add_session("session-legacy", &legacy_follow, 3_600)
            .await
            .expect("legacy follow-session stream session");
        assert!(
            resolve_stream_session_access(&state, "203.0.113.40", "tcp", 2222)
                .await
                .expect("legacy follow-session stream lookup")
                .allowed
        );

        let mut missing_credential = session_for(&all_credential, "203.0.113.30");
        missing_credential.totp_id = "missing-totp".to_string();
        state
            .storage
            .store
            .add_session("session-missing", &missing_credential, 3_600)
            .await
            .expect("missing-credential session");
        assert!(
            !resolve_stream_session_access(&state, "203.0.113.30", "tcp", 2222)
                .await
                .expect("missing credential stream lookup")
                .allowed
        );

        state
            .storage
            .store
            .add_session(
                "session-invalid-ip",
                &session_for(&all_credential, "unknown"),
                3_600,
            )
            .await
            .expect("invalid-IP session");
        assert!(
            !resolve_stream_session_access(&state, "unknown", "tcp", 2222)
                .await
                .expect("invalid IP stream lookup")
                .allowed
        );
    }
}
