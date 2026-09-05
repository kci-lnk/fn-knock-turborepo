use std::time::Duration;

use axum::{
    Json,
    extract::{FromRequest, FromRequestParts, Path, Query, State},
    http::{HeaderMap, StatusCode, Uri, header, request::Parts},
    response::{IntoResponse, Response},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Serialize, de::DeserializeOwned};
use utoipa::ToSchema;
use utoipa_axum::{router::OpenApiRouter, routes};
use uuid::Uuid;

use crate::{response, state::AppState};

use super::{
    access::{
        self, TerminalAccess, WebTerminalAccessStatus, WebTerminalSettings,
        WebTerminalSettingsInput, WebTerminalVerifyInput,
    },
    domain::*,
    service,
};

const MAX_INPUT_BASE64_BYTES: usize = 87_384;

struct TerminalJson<T>(T);

impl<S, T> FromRequest<S> for TerminalJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|_| terminal_error(TerminalError::invalid("invalid terminal JSON body")))
    }
}

struct TerminalQuery<T>(T);

impl<S, T> FromRequestParts<S> for TerminalQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(value)| Self(value))
            .map_err(|_| terminal_error(TerminalError::invalid("invalid terminal query")))
    }
}

struct TerminalId(String);

impl<S> FromRequestParts<S> for TerminalId
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Path(id) = Path::<String>::from_request_parts(parts, state)
            .await
            .map_err(|_| terminal_error(TerminalError::invalid("invalid terminal resource id")))?;
        if Uuid::parse_str(&id).is_err() {
            return Err(terminal_error(TerminalError::invalid(
                "invalid terminal resource id",
            )));
        }
        Ok(Self(id))
    }
}

pub fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(get_feature_settings, update_feature_settings))
        .routes(routes!(get_access_status))
        .routes(routes!(verify_access))
        .routes(routes!(get_local_terminal, update_local_terminal))
        .routes(routes!(create_local_session))
        .routes(routes!(list_targets, create_target))
        .routes(routes!(get_target, update_target, delete_target))
        .routes(routes!(probe_host_key))
        .routes(routes!(test_connection))
        .routes(routes!(list_sessions))
        .routes(routes!(create_session))
        .routes(routes!(rename_session, delete_session))
        .routes(routes!(create_attachment))
        .routes(routes!(attachment_events))
        .routes(routes!(send_input))
        .routes(routes!(resize))
        .routes(routes!(claim_control))
        .routes(routes!(delete_attachment))
}

#[utoipa::path(get, path = "/api/admin/terminal/local", tag = "terminal", responses((status = 200, body = LocalTerminalStatus)))]
async fn get_local_terminal(_access: TerminalAccess, State(state): State<AppState>) -> Response {
    result(service::local_terminal_status(&state).await)
}

#[utoipa::path(patch, path = "/api/admin/terminal/local", tag = "terminal", params(ForceQuery), request_body = LocalTerminalSettingsInput, responses((status = 200, body = LocalTerminalStatus), (status = 400, body = TerminalErrorEnvelope), (status = 409, body = TerminalErrorEnvelope), (status = 503, body = TerminalErrorEnvelope)))]
async fn update_local_terminal(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalQuery(query): TerminalQuery<ForceQuery>,
    TerminalJson(input): TerminalJson<LocalTerminalSettingsInput>,
) -> Response {
    result(
        service::update_local_terminal(
            &state,
            input,
            query.force,
            query.confirmation_token.as_deref(),
        )
        .await,
    )
}

#[utoipa::path(post, path = "/api/admin/terminal/local/sessions", tag = "terminal", request_body = CreateSessionInput, responses((status = 200, body = TerminalSession), (status = 409, body = TerminalErrorEnvelope), (status = 502, body = TerminalErrorEnvelope), (status = 503, body = TerminalErrorEnvelope)))]
async fn create_local_session(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalJson(input): TerminalJson<CreateSessionInput>,
) -> Response {
    result(service::create_local_session(&state, input).await)
}

#[utoipa::path(get, path = "/api/admin/terminal/targets", tag = "terminal", responses((status = 200, body = [TerminalTarget])))]
async fn list_targets(_access: TerminalAccess, State(state): State<AppState>) -> Response {
    result(service::targets(&state).await)
}

#[utoipa::path(post, path = "/api/admin/terminal/targets", tag = "terminal", request_body = TargetCreateInput, responses((status = 200, body = TerminalTarget), (status = 400, body = TerminalErrorEnvelope)))]
async fn create_target(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalJson(input): TerminalJson<TargetCreateInput>,
) -> Response {
    result(service::create_target(&state, input).await)
}

#[utoipa::path(get, path = "/api/admin/terminal/targets/{id}", tag = "terminal", params(("id" = String, Path)), responses((status = 200, body = TerminalTarget), (status = 404, body = TerminalErrorEnvelope)))]
async fn get_target(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
) -> Response {
    result(service::target(&state, &id).await)
}

#[utoipa::path(patch, path = "/api/admin/terminal/targets/{id}", tag = "terminal", params(("id" = String, Path), ForceQuery), request_body = TargetUpdateInput, responses((status = 200, body = TerminalTarget), (status = 409, body = TerminalErrorEnvelope)))]
async fn update_target(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalQuery(query): TerminalQuery<ForceQuery>,
    TerminalJson(input): TerminalJson<TargetUpdateInput>,
) -> Response {
    result(
        service::update_target(
            &state,
            &id,
            input,
            query.force,
            query.confirmation_token.as_deref(),
        )
        .await,
    )
}

#[utoipa::path(delete, path = "/api/admin/terminal/targets/{id}", tag = "terminal", params(("id" = String, Path), TargetDeleteQuery), responses((status = 200), (status = 409, body = TerminalErrorEnvelope)))]
async fn delete_target(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalQuery(query): TerminalQuery<TargetDeleteQuery>,
) -> Response {
    empty(
        service::delete_target(
            &state,
            &id,
            query.revision,
            query.force,
            query.confirmation_token.as_deref(),
        )
        .await,
    )
}

#[utoipa::path(post, path = "/api/admin/terminal/targets/probe-host-key", tag = "terminal", request_body = ProbeHostKeyInput, responses((status = 200, body = HostKeyProbeResult), (status = 400, body = TerminalErrorEnvelope)))]
async fn probe_host_key(
    _access: TerminalAccess,
    TerminalJson(input): TerminalJson<ProbeHostKeyInput>,
) -> Response {
    result(service::probe_host_key(input).await)
}

#[utoipa::path(post, path = "/api/admin/terminal/targets/test-connection", tag = "terminal", request_body = TerminalTestConnectionInput, responses((status = 200, body = ConnectionTestResult), (status = 400, body = TerminalErrorEnvelope)))]
async fn test_connection(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalJson(input): TerminalJson<TerminalTestConnectionInput>,
) -> Response {
    result(service::test_connection(&state, input).await)
}

#[utoipa::path(get, path = "/api/admin/terminal/sessions", tag = "terminal", responses((status = 200, body = SessionListResult)))]
async fn list_sessions(_access: TerminalAccess, State(state): State<AppState>) -> Response {
    response::ok(service::sessions(&state).await).into_response()
}

#[utoipa::path(post, path = "/api/admin/terminal/targets/{id}/sessions", tag = "terminal", params(("id" = String, Path)), request_body = CreateSessionInput, responses((status = 200, body = TerminalSession), (status = 502, body = TerminalErrorEnvelope)))]
async fn create_session(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<CreateSessionInput>,
) -> Response {
    result(service::create_session(&state, &id, input).await)
}

#[utoipa::path(patch, path = "/api/admin/terminal/sessions/{id}", tag = "terminal", params(("id" = String, Path)), request_body = RenameSessionInput, responses((status = 200, body = TerminalSession), (status = 404, body = TerminalErrorEnvelope)))]
async fn rename_session(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<RenameSessionInput>,
) -> Response {
    result(service::rename_session(&state, &id, input).await)
}

#[utoipa::path(delete, path = "/api/admin/terminal/sessions/{id}", tag = "terminal", params(("id" = String, Path)), responses((status = 200), (status = 404, body = TerminalErrorEnvelope)))]
async fn delete_session(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
) -> Response {
    empty(service::terminate_session(&state, &id).await)
}

#[utoipa::path(post, path = "/api/admin/terminal/sessions/{id}/attachments", tag = "terminal", params(("id" = String, Path)), request_body = CreateAttachmentInput, responses((status = 200, body = TerminalAttachment), (status = 404, body = TerminalErrorEnvelope)))]
async fn create_attachment(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<CreateAttachmentInput>,
) -> Response {
    result(
        state
            .terminal
            .create_attachment(&id, input.cols, input.rows)
            .await,
    )
}

#[utoipa::path(get, path = "/api/admin/terminal/attachments/{id}/events", tag = "terminal", params(("id" = String, Path), EventsQuery), responses((status = 200, body = EventsResult), (status = 410, body = TerminalErrorEnvelope)))]
async fn attachment_events(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalQuery(query): TerminalQuery<EventsQuery>,
) -> Response {
    result(
        state
            .terminal
            .events(
                &id,
                query.after.unwrap_or_default(),
                Duration::from_millis(query.timeout_ms.unwrap_or(4_500)),
            )
            .await,
    )
}

#[utoipa::path(post, path = "/api/admin/terminal/attachments/{id}/input", tag = "terminal", params(("id" = String, Path)), request_body = InputRequest, responses((status = 200), (status = 409, body = TerminalErrorEnvelope)))]
async fn send_input(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<InputRequest>,
) -> Response {
    let decoded = if input.data_base64.len() > MAX_INPUT_BASE64_BYTES {
        Err(TerminalError::invalid("terminal input exceeds 64 KiB"))
    } else {
        STANDARD
            .decode(input.data_base64.as_bytes())
            .map_err(|_| TerminalError::invalid("terminal input is not valid base64"))
    };
    let result = match decoded {
        Ok(data) => {
            state
                .terminal
                .send_input(&id, input.generation, input.sequence, data)
                .await
        }
        Err(error) => Err(error),
    };
    empty(result)
}

#[utoipa::path(post, path = "/api/admin/terminal/attachments/{id}/resize", tag = "terminal", params(("id" = String, Path)), request_body = ResizeRequest, responses((status = 200, body = TerminalSession), (status = 409, body = TerminalErrorEnvelope)))]
async fn resize(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<ResizeRequest>,
) -> Response {
    result(
        state
            .terminal
            .resize(
                &id,
                input.generation,
                input.revision,
                input.cols,
                input.rows,
            )
            .await,
    )
}

#[utoipa::path(post, path = "/api/admin/terminal/attachments/{id}/control", tag = "terminal", params(("id" = String, Path)), request_body = ClaimControlRequest, responses((status = 200, body = TerminalAttachment), (status = 409, body = TerminalErrorEnvelope)))]
async fn claim_control(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
    TerminalJson(input): TerminalJson<ClaimControlRequest>,
) -> Response {
    result(state.terminal.claim_control(&id, input.generation).await)
}

#[utoipa::path(delete, path = "/api/admin/terminal/attachments/{id}", tag = "terminal", params(("id" = String, Path)), responses((status = 200), (status = 410, body = TerminalErrorEnvelope)))]
async fn delete_attachment(
    _access: TerminalAccess,
    State(state): State<AppState>,
    TerminalId(id): TerminalId,
) -> Response {
    empty(state.terminal.detach(&id).await)
}

fn result<T: Serialize>(result: TerminalResult<T>) -> Response {
    match result {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => terminal_error(error),
    }
}

fn empty(result: TerminalResult<()>) -> Response {
    match result {
        Ok(()) => response::success_empty().into_response(),
        Err(error) => terminal_error(error),
    }
}

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TerminalErrorEnvelope {
    pub success: bool,
    pub error_code: TerminalErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_session_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation_token: Option<String>,
}

pub(super) fn terminal_error(error: TerminalError) -> Response {
    let status = match error.code {
        TerminalErrorCode::FeatureDisabled | TerminalErrorCode::AccessPasswordRequired => {
            StatusCode::FORBIDDEN
        }
        TerminalErrorCode::AccessRateLimited => StatusCode::TOO_MANY_REQUESTS,
        TerminalErrorCode::InvalidRequest
        | TerminalErrorCode::HostKeyRequired
        | TerminalErrorCode::LocalTerminalUnsupported
        | TerminalErrorCode::LocalTerminalRiskAcknowledgementRequired => StatusCode::BAD_REQUEST,
        TerminalErrorCode::TargetNotFound | TerminalErrorCode::SessionNotFound => {
            StatusCode::NOT_FOUND
        }
        TerminalErrorCode::AttachmentExpired => StatusCode::GONE,
        TerminalErrorCode::HostKeyMismatch
        | TerminalErrorCode::ControllerConflict
        | TerminalErrorCode::TargetRevisionConflict
        | TerminalErrorCode::LocalTerminalDisabled
        | TerminalErrorCode::LocalTerminalRevisionConflict
        | TerminalErrorCode::SessionLimitReached
        | TerminalErrorCode::SessionLost
        | TerminalErrorCode::Conflict => StatusCode::CONFLICT,
        TerminalErrorCode::AuthenticationFailed => StatusCode::UNAUTHORIZED,
        TerminalErrorCode::PtyRejected
        | TerminalErrorCode::LocalPtyStartFailed
        | TerminalErrorCode::UpstreamUnavailable => StatusCode::BAD_GATEWAY,
        TerminalErrorCode::LocalShellUnavailable | TerminalErrorCode::ResourceBusy => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        TerminalErrorCode::ConnectTimeout => StatusCode::GATEWAY_TIMEOUT,
        TerminalErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
    };
    let busy = error.code == TerminalErrorCode::ResourceBusy;
    let mut response = (
        status,
        Json(TerminalErrorEnvelope {
            success: false,
            error_code: error.code,
            message: error.message,
            active_session_count: error.active_session_count,
            confirmation_token: error.confirmation_token,
        }),
    )
        .into_response();
    if busy {
        response.headers_mut().insert(
            header::RETRY_AFTER,
            axum::http::HeaderValue::from_static("3"),
        );
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, header::CONTENT_TYPE},
    };

    #[tokio::test]
    async fn malformed_json_uses_stable_terminal_error_envelope() {
        let request = Request::builder()
            .method("POST")
            .uri("/api/admin/terminal/targets/probe-host-key")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"host":"localhost","port":70000}"#))
            .unwrap();
        let response = match TerminalJson::<ProbeHostKeyInput>::from_request(request, &()).await {
            Ok(_) => panic!("out-of-range port should be rejected"),
            Err(response) => response,
        };
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["success"], false);
        assert_eq!(body["errorCode"], "invalid_request");
    }

    #[test]
    fn local_terminal_failures_use_stable_http_statuses() {
        for (code, expected) in [
            (
                TerminalErrorCode::LocalTerminalUnsupported,
                StatusCode::BAD_REQUEST,
            ),
            (
                TerminalErrorCode::LocalTerminalRiskAcknowledgementRequired,
                StatusCode::BAD_REQUEST,
            ),
            (
                TerminalErrorCode::LocalTerminalDisabled,
                StatusCode::CONFLICT,
            ),
            (
                TerminalErrorCode::LocalTerminalRevisionConflict,
                StatusCode::CONFLICT,
            ),
            (
                TerminalErrorCode::LocalShellUnavailable,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (
                TerminalErrorCode::LocalPtyStartFailed,
                StatusCode::BAD_GATEWAY,
            ),
        ] {
            assert_eq!(
                terminal_error(TerminalError::new(code, "test")).status(),
                expected
            );
        }
    }
}

#[utoipa::path(get, path = "/api/admin/terminal/settings", tag = "terminal", responses((status = 200, body = WebTerminalSettings)))]
async fn get_feature_settings(State(state): State<AppState>) -> Response {
    result(access::settings(&state).await)
}

#[utoipa::path(patch, path = "/api/admin/terminal/settings", tag = "terminal", request_body = WebTerminalSettingsInput, responses((status = 200, body = WebTerminalSettings), (status = 409, body = TerminalErrorEnvelope), (status = 503, body = TerminalErrorEnvelope)))]
async fn update_feature_settings(
    State(state): State<AppState>,
    TerminalJson(input): TerminalJson<WebTerminalSettingsInput>,
) -> Response {
    result(access::update(&state, input).await)
}

#[utoipa::path(get, path = "/api/admin/terminal/access", tag = "terminal", responses((status = 200, body = WebTerminalAccessStatus)))]
async fn get_access_status(State(state): State<AppState>, headers: HeaderMap) -> Response {
    result(access::status(&state, &headers).await)
}

#[utoipa::path(post, path = "/api/admin/terminal/access/verify", tag = "terminal", request_body = WebTerminalVerifyInput, responses((status = 200), (status = 403, body = TerminalErrorEnvelope), (status = 429, body = TerminalErrorEnvelope), (status = 503, body = TerminalErrorEnvelope)))]
async fn verify_access(
    State(state): State<AppState>,
    headers: HeaderMap,
    uri: Uri,
    TerminalJson(input): TerminalJson<WebTerminalVerifyInput>,
) -> Response {
    match access::verify(&state, &headers, input).await {
        Err(error) => terminal_error(error),
        Ok(token) => {
            let mut response = response::success_empty().into_response();
            if let Some(token) = token {
                let cookie = access::browser_cookie(
                    &token,
                    crate::http_utils::is_secure_request(&headers, &uri),
                );
                if let Ok(value) = cookie.parse() {
                    response.headers_mut().append(header::SET_COOKIE, value);
                }
            }
            response
        }
    }
}
