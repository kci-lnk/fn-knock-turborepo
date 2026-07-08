use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{i18n::Translator, response, state::AppState, time_utils};

fn security_overview_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.securityOverviewRoutes.{key}"))
}

const FN_EVENT_AUTH_LOGIN_FAILURE: &str = "FN_EVENT_AUTH_LOGIN_FAILURE";
const FN_EVENT_SECURITY_SCANNER_BLOCKED: &str = "FN_EVENT_SECURITY_SCANNER_BLOCKED";

#[derive(Deserialize)]
struct OverviewQuery {
    #[serde(rename = "rangeSec")]
    range_sec: Option<String>,
}

pub fn security_overview_routes() -> Router<AppState> {
    Router::new().route("/api/admin/security/overview", get(overview))
}

async fn overview(State(state): State<AppState>, Query(query): Query<OverviewQuery>) -> Response {
    let range_sec = crate::node_compat::parse_i64_or(query.range_sec.as_deref(), 3600)
        .clamp(60, 30 * 24 * 3600);
    let now_ms = time_utils::now_ms();
    let from_ms = now_ms - range_sec * 1000;
    let bucket_count = ((range_sec as f64 / 900.0).round() as i64).clamp(12, 48);

    let events = match state
        .redis
        .list_system_events_by_range(
            from_ms,
            now_ms,
            &[
                FN_EVENT_AUTH_LOGIN_FAILURE,
                FN_EVENT_SECURITY_SCANNER_BLOCKED,
            ],
        )
        .await
    {
        Ok(events) => events,
        Err(error) => {
            let translator = Translator::from_state(&state).await;
            tracing::warn!(%error, "failed to load security overview system events");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                security_overview_route_text(&translator, "loadFailed"),
            );
        }
    };

    let failed_timestamps = event_timestamps(&events, FN_EVENT_AUTH_LOGIN_FAILURE);
    let blocked_timestamps = event_timestamps(&events, FN_EVENT_SECURITY_SCANNER_BLOCKED);
    let waf_series_base = build_bucket_series(from_ms, now_ms, bucket_count);
    let bucket_starts = waf_series_base
        .iter()
        .map(|point| point.0)
        .collect::<Vec<_>>();
    let (waf_total, waf_counts) = match state
        .redis
        .count_waf_logs_for_buckets(&bucket_starts, now_ms)
        .await
    {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(%error, "failed to load WAF overview buckets");
            (0, vec![0; bucket_starts.len()])
        }
    };
    let waf_series = bucket_starts
        .iter()
        .enumerate()
        .map(|(index, start)| json!([start, *waf_counts.get(index).unwrap_or(&0)]))
        .collect::<Vec<_>>();

    response::ok(json!({
        "rangeSec": range_sec,
        "totals": {
            "failedLogins": failed_timestamps.len(),
            "blockedScanners": blocked_timestamps.len(),
            "wafEvents": waf_total
        },
        "series": {
            "failedLogins": build_count_series(&failed_timestamps, from_ms, now_ms, bucket_count),
            "blockedScanners": build_count_series(&blocked_timestamps, from_ms, now_ms, bucket_count),
            "wafEvents": waf_series
        }
    }))
    .into_response()
}

fn event_timestamps(events: &[(Value, i64)], event_type: &str) -> Vec<i64> {
    events
        .iter()
        .filter(|(event, _)| event.get("type").and_then(Value::as_str) == Some(event_type))
        .map(|(_, timestamp)| *timestamp)
        .collect()
}

fn build_count_series(
    timestamps: &[i64],
    from_ms: i64,
    to_ms: i64,
    bucket_count: i64,
) -> Vec<Value> {
    let span = (to_ms - from_ms).max(1);
    let step = ((span + bucket_count - 1) / bucket_count).max(1);
    let mut buckets = vec![0_i64; bucket_count.max(1) as usize];
    for timestamp in timestamps {
        let index =
            ((*timestamp - from_ms).div_euclid(step)).clamp(0, buckets.len() as i64 - 1) as usize;
        buckets[index] += 1;
    }
    buckets
        .into_iter()
        .enumerate()
        .map(|(index, count)| json!([from_ms + index as i64 * step, count]))
        .collect()
}

fn build_bucket_series(from_ms: i64, to_ms: i64, bucket_count: i64) -> Vec<(i64, i64)> {
    let bucket_count = bucket_count.max(1);
    let span = (to_ms - from_ms).max(1);
    let step = ((span + bucket_count - 1) / bucket_count).max(1);
    (0..bucket_count)
        .map(|index| (from_ms + index * step, 0))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_count_series_with_clamped_buckets() {
        let series = build_count_series(&[100, 150, 1_000], 100, 1_000, 3);
        assert_eq!(
            series,
            vec![json!([100, 2]), json!([400, 0]), json!([700, 1])]
        );
    }

    #[test]
    fn filters_event_timestamps_by_type() {
        let events = vec![
            (json!({ "type": FN_EVENT_AUTH_LOGIN_FAILURE }), 10),
            (json!({ "type": FN_EVENT_SECURITY_SCANNER_BLOCKED }), 20),
        ];
        assert_eq!(
            event_timestamps(&events, FN_EVENT_AUTH_LOGIN_FAILURE),
            vec![10]
        );
    }

    #[test]
    fn range_sec_parser_matches_node_parse_int_safe() {
        assert_eq!(crate::node_compat::parse_i64_or(None, 3600), 3600);
        assert_eq!(crate::node_compat::parse_i64_or(Some("900x"), 3600), 900);
        assert_eq!(crate::node_compat::parse_i64_or(Some("  +3.9"), 3600), 3);
        assert_eq!(crate::node_compat::parse_i64_or(Some("0x10"), 3600), 0);
        assert_eq!(crate::node_compat::parse_i64_or(Some("nope"), 3600), 3600);
    }

    #[test]
    fn localizes_security_overview_route_text() {
        let zh = Translator::new("zh-CN");
        assert_eq!(
            security_overview_route_text(&zh, "loadFailed"),
            "加载安全概览失败"
        );
    }
}
