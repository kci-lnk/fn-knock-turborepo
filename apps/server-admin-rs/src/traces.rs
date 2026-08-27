use std::collections::{BTreeMap, HashSet};

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{response, state::AppState};

pub(crate) fn trace_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(get_trace))
}

#[utoipa::path(
    get,
    path = "/api/admin/traces/{trace_id}",
    tag = "traces",
    operation_id = "get_api_admin_traces_trace_id",
    params(("trace_id" = String, Path, description = "Canonical or legacy fn-knock Trace ID")),
    responses(
        (status = 200, description = "Aggregated trace details"),
        (status = 400, description = "Invalid Trace ID")
    )
)]
async fn get_trace(State(state): State<AppState>, Path(trace_id): Path<String>) -> Response {
    let trace_id = trace_id.trim();
    if !crate::trace_id::is_valid_trace_id(trace_id) {
        return response::error(StatusCode::BAD_REQUEST, "invalid trace_id");
    }

    let gateway = state.gateway.client.find_log_entry_by_trace_id(trace_id);
    let waf = state.storage.store.get_waf_log_event(trace_id);
    let events = state.storage.store.find_system_events_by_trace(trace_id);
    let triggers = notification_history_for_trace(&state, "trigger", trace_id, &[]);
    let deliveries = notification_history_for_trace(&state, "delivery", trace_id, &[]);
    let (gateway, waf, events, direct_triggers, direct_deliveries) =
        tokio::join!(gateway, waf, events, triggers, deliveries);

    let (request, gateway_status) = match gateway {
        Ok(value) => {
            let data = value.get("data").cloned().unwrap_or(Value::Null);
            let found = data.get("found").and_then(Value::as_bool).unwrap_or(false);
            (
                found
                    .then(|| data.get("entry").cloned())
                    .flatten()
                    .filter(|entry| !entry.is_null()),
                source_status(found, false),
            )
        }
        Err(error) => {
            tracing::warn!(%error, %trace_id, "failed to query gateway trace");
            (None, source_status(false, true))
        }
    };
    let (waf_event, waf_status) = match waf {
        Ok(event) => {
            let found = event.is_some();
            (event, source_status(found, false))
        }
        Err(error) => {
            tracing::warn!(%error, %trace_id, "failed to query WAF trace");
            (None, source_status(false, true))
        }
    };
    let (mut system_events, events_status) = match events {
        Ok(mut events) => {
            let found = !events.is_empty();
            crate::events::hydrate_system_event_ip_locations(&state, &mut events).await;
            (events, source_status(found, false))
        }
        Err(error) => {
            tracing::warn!(%error, %trace_id, "failed to query system-event trace");
            (Vec::new(), source_status(false, true))
        }
    };
    system_events.sort_by(|left, right| {
        left.get("happened_at")
            .and_then(Value::as_str)
            .cmp(&right.get("happened_at").and_then(Value::as_str))
    });
    let event_ids = system_events
        .iter()
        .filter_map(|event| event.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect::<Vec<_>>();

    let (notification_triggers, trigger_unavailable) =
        merge_legacy_notification_history(&state, "trigger", trace_id, direct_triggers, &event_ids)
            .await;
    let (notification_deliveries, delivery_unavailable) = merge_legacy_notification_history(
        &state,
        "delivery",
        trace_id,
        direct_deliveries,
        &event_ids,
    )
    .await;
    let notifications_found =
        !notification_triggers.is_empty() || !notification_deliveries.is_empty();
    let notifications_status = source_status(
        notifications_found,
        trigger_unavailable || delivery_unavailable,
    );
    let found = request.is_some()
        || waf_event.is_some()
        || !system_events.is_empty()
        || notifications_found;

    response::ok(json!({
        "trace_id": trace_id,
        "found": found,
        "request": request,
        "waf_event": waf_event,
        "system_events": system_events,
        "notification_triggers": notification_triggers,
        "notification_deliveries": notification_deliveries,
        "sources": {
            "gateway_logs": gateway_status,
            "waf_logs": waf_status,
            "system_events": events_status,
            "notifications": notifications_status,
        }
    }))
    .into_response()
}

fn source_status(found: bool, unavailable: bool) -> &'static str {
    if unavailable {
        "unavailable"
    } else if found {
        "found"
    } else {
        "not_found"
    }
}

async fn notification_history_for_trace(
    state: &AppState,
    kind: &str,
    trace_id: &str,
    event_ids: &[String],
) -> crate::storage::StorageResult<Vec<Value>> {
    let mut records = state
        .storage
        .store
        .load_notification_history_by_trace(kind, trace_id)
        .await?;
    if !event_ids.is_empty() {
        let event_ids = event_ids.iter().map(String::as_str).collect::<HashSet<_>>();
        records.extend(
            state
                .storage
                .store
                .load_notification_history(kind)
                .await?
                .into_iter()
                .filter(|record| {
                    record
                        .get("event_id")
                        .and_then(Value::as_str)
                        .is_some_and(|event_id| event_ids.contains(event_id))
                }),
        );
    }
    let mut unique = BTreeMap::new();
    for record in records {
        if let Some(id) = record.get("id").and_then(Value::as_str) {
            unique.insert(id.to_string(), record);
        }
    }
    Ok(unique.into_values().collect())
}

async fn merge_legacy_notification_history(
    state: &AppState,
    kind: &str,
    trace_id: &str,
    direct: crate::storage::StorageResult<Vec<Value>>,
    event_ids: &[String],
) -> (Vec<Value>, bool) {
    let direct = match direct {
        Ok(records) => records,
        Err(error) => {
            tracing::warn!(%error, %trace_id, kind, "failed to query notification trace");
            return (Vec::new(), true);
        }
    };
    if event_ids.is_empty() {
        return (direct, false);
    }
    match notification_history_for_trace(state, kind, trace_id, event_ids).await {
        Ok(records) => (records, false),
        Err(error) => {
            tracing::warn!(%error, %trace_id, kind, "failed to join legacy notification trace");
            (direct, true)
        }
    }
}
