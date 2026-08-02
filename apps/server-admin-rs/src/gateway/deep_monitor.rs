use std::{convert::Infallible, io};

use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_stream::{StreamExt, wrappers::ReceiverStream};

use crate::{grpc_proto::DeepMonitorQuery, response, state::AppState};

const DEFAULT_DURATION_SECONDS: i32 = 30 * 60;

#[derive(Deserialize)]
struct StartBody {
    host: String,
    #[serde(default = "default_duration")]
    duration_seconds: i32,
}

#[derive(Deserialize)]
struct ExtendBody {
    duration_seconds: i32,
}

#[derive(Deserialize, Default)]
struct ListQuery {
    #[serde(default)]
    include_expired: bool,
}

#[derive(Deserialize, Default)]
struct EventsQuery {
    cursor: Option<String>,
    limit: Option<i32>,
    #[serde(rename = "type")]
    event_type: Option<String>,
    search: Option<String>,
    direction: Option<String>,
    method: Option<String>,
    status: Option<i32>,
    client_ip: Option<String>,
    identity: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct PayloadQuery {
    part: String,
    #[serde(default)]
    offset: u64,
    limit: Option<usize>,
}

#[derive(Deserialize, Default)]
struct LiveQuery {
    #[serde(default)]
    after_sequence: u64,
}

fn default_duration() -> i32 {
    DEFAULT_DURATION_SECONDS
}

pub fn deep_monitor_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/deep-monitor/sessions",
            get(list_sessions).post(start_session),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/extend",
            axum::routing::post(extend_session),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/stop",
            axum::routing::post(stop_session),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/events",
            get(list_events),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}",
            get(get_event),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload",
            get(payload),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/live",
            get(live),
        )
        .route(
            "/api/admin/deep-monitor/sessions/{session_id}/download",
            get(download_session),
        )
        .layer(middleware::from_fn(no_store))
}

async fn no_store(request: axum::extract::Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, max-age=0"),
    );
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

async fn start_session(State(state): State<AppState>, Json(body): Json<StartBody>) -> Response {
    go_json(
        state
            .go_backend
            .start_deep_monitor(body.host, body.duration_seconds)
            .await,
    )
}

async fn list_sessions(State(state): State<AppState>, Query(query): Query<ListQuery>) -> Response {
    go_json(
        state
            .go_backend
            .list_deep_monitors(query.include_expired)
            .await,
    )
}

async fn get_session(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    match state
        .go_backend
        .list_deep_monitors(true)
        .await
        .and_then(data)
    {
        Ok(value) => {
            let found = value
                .get("items")
                .and_then(Value::as_array)
                .and_then(|items| {
                    items
                        .iter()
                        .find(|item| item.get("id").and_then(Value::as_str) == Some(&session_id))
                })
                .cloned();
            match found {
                Some(item) => response::ok(item).into_response(),
                None => response::error(StatusCode::NOT_FOUND, "deep monitor session not found"),
            }
        }
        Err(error) => backend_error(error),
    }
}

async fn extend_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(body): Json<ExtendBody>,
) -> Response {
    go_json(
        state
            .go_backend
            .extend_deep_monitor(session_id, body.duration_seconds)
            .await,
    )
}

async fn stop_session(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    go_json(state.go_backend.stop_deep_monitor(session_id).await)
}

async fn delete_session(State(state): State<AppState>, Path(session_id): Path<String>) -> Response {
    go_json(state.go_backend.delete_deep_monitor(session_id).await)
}

async fn list_events(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventsQuery>,
) -> Response {
    go_json(
        state
            .go_backend
            .query_deep_monitor_events(DeepMonitorQuery {
                session_id,
                cursor: query.cursor.unwrap_or_default(),
                limit: query.limit.unwrap_or(100).clamp(1, 200),
                r#type: query.event_type.unwrap_or_default(),
                search: query.search.unwrap_or_default(),
                direction: query.direction.unwrap_or_default(),
                method: query.method.unwrap_or_default(),
                status: query.status.unwrap_or_default(),
                client_ip: query.client_ip.unwrap_or_default(),
                identity: query.identity.unwrap_or_default(),
                path: query.path.unwrap_or_default(),
            })
            .await,
    )
}

async fn get_event(
    State(state): State<AppState>,
    Path((session_id, event_id)): Path<(String, String)>,
) -> Response {
    go_json(
        state
            .go_backend
            .get_deep_monitor_event(session_id, event_id)
            .await,
    )
}

async fn payload(
    State(state): State<AppState>,
    Path((session_id, event_id)): Path<(String, String)>,
    Query(query): Query<PayloadQuery>,
) -> Response {
    let mut stream = match state
        .go_backend
        .stream_deep_monitor_payload(session_id, event_id, query.part, query.offset)
        .await
    {
        Ok(stream) => stream,
        Err(error) => return backend_error(error),
    };
    let first = match stream.message().await {
        Ok(Some(chunk)) => chunk,
        Ok(None) => return StatusCode::NO_CONTENT.into_response(),
        Err(error) => return backend_error(error.into()),
    };
    let content_type = HeaderValue::from_str(&first.content_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    if let Some(limit) = query.limit.map(|value| value.clamp(1, 256 * 1024)) {
        let mut data = first.data;
        while data.len() < limit {
            match stream.message().await {
                Ok(Some(chunk)) => data.extend_from_slice(&chunk.data),
                Ok(None) => break,
                Err(error) => return backend_error(error.into()),
            }
        }
        data.truncate(limit);
        let mut response = Body::from(data).into_response();
        response
            .headers_mut()
            .insert(header::CONTENT_TYPE, content_type);
        return response;
    }
    let first_bytes = Bytes::from(first.data);
    let output =
        tokio_stream::once(Ok::<Bytes, io::Error>(first_bytes)).chain(stream.map(|item| {
            item.map(|chunk| Bytes::from(chunk.data))
                .map_err(io::Error::other)
        }));
    let mut response = Body::from_stream(output).into_response();
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, content_type);
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_static("attachment"),
    );
    response
}

async fn live(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Query(query): Query<LiveQuery>,
    headers: HeaderMap,
) -> Response {
    let header_sequence = headers
        .get("last-event-id")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or_default();
    let after = query.after_sequence.max(header_sequence);
    let mut grpc = match state
        .go_backend
        .watch_deep_monitor_events(session_id, after)
        .await
    {
        Ok(stream) => stream,
        Err(error) => return backend_error(error),
    };
    let (tx, rx) = mpsc::channel::<Result<Event, Infallible>>(128);
    tokio::spawn(async move {
        loop {
            match grpc.message().await {
                Ok(Some(item)) => {
                    let sequence = item.sequence;
                    let data = crate::go_backend::deep_monitor::summary_json(item).to_string();
                    let event = Event::default()
                        .event("traffic")
                        .id(sequence.to_string())
                        .data(data);
                    if tx.send(Ok(event)).await.is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let event = Event::default()
                        .event("monitor_error")
                        .data(json!({"message": error.message()}).to_string());
                    let _ = tx.send(Ok(event)).await;
                    break;
                }
            }
        }
    });
    Sse::new(ReceiverStream::new(rx))
        .keep_alive(KeepAlive::default())
        .into_response()
}

async fn download_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Response {
    let mut stream = match state
        .go_backend
        .stream_deep_monitor_archive(session_id.clone())
        .await
    {
        Ok(stream) => stream,
        Err(error) => return backend_error(error),
    };
    let first = match stream.message().await {
        Ok(Some(chunk)) => chunk,
        Ok(None) => return StatusCode::NO_CONTENT.into_response(),
        Err(error) => return backend_error(error.into()),
    };
    let first_bytes = Bytes::from(first.data);
    let output =
        tokio_stream::once(Ok::<Bytes, io::Error>(first_bytes)).chain(stream.map(|item| {
            item.map(|chunk| Bytes::from(chunk.data))
                .map_err(io::Error::other)
        }));
    let safe_id: String = session_id
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
        .take(64)
        .collect();
    let filename = if safe_id.is_empty() {
        "deep-monitor.zip".to_string()
    } else {
        format!("deep-monitor-{safe_id}.zip")
    };
    let mut response = Body::from_stream(output).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")) {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, value);
    }
    response
}

fn go_json(result: anyhow::Result<Value>) -> Response {
    match result.and_then(data) {
        Ok(value) => response::ok(value).into_response(),
        Err(error) => backend_error(error),
    }
}

fn data(value: Value) -> anyhow::Result<Value> {
    if value.get("success").and_then(Value::as_bool) == Some(true) {
        Ok(value.get("data").cloned().unwrap_or(Value::Null))
    } else {
        anyhow::bail!(
            "{}",
            value
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("gateway request failed")
        )
    }
}

fn backend_error(error: anyhow::Error) -> Response {
    tracing::warn!(%error, "deep monitor backend request failed");
    response::error(StatusCode::BAD_GATEWAY, error.to_string())
}
