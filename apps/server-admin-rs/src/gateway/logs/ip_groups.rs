use std::collections::{BTreeMap, BTreeSet};

use super::*;

pub(super) fn validate_client_ip(value: Option<&str>) -> anyhow::Result<()> {
    if let Some(value) = value
        && value != "unknown"
        && value.parse::<std::net::IpAddr>().is_err()
    {
        anyhow::bail!("invalid client_ip");
    }
    Ok(())
}

fn canonical_ip(value: &str) -> String {
    normalize_ip(value)
        .parse::<std::net::IpAddr>()
        .map(|ip| ip.to_canonical().to_string())
        .unwrap_or_default()
}

pub(super) fn matches_entry(entry: &Value, ip: Option<&str>, waf: Option<&str>) -> bool {
    let matches_ip = ip.is_none_or(|ip| {
        let actual = canonical_ip(&infer_gateway_log_client_ip(entry));
        if ip == "unknown" {
            actual.is_empty()
        } else {
            actual == canonical_ip(ip)
        }
    });
    matches_ip && waf.is_none_or(|waf| gateway_log_matches_waf_status(entry, waf))
}

#[derive(Default, serde::Serialize)]
struct IpGroup {
    client_ip: String,
    requests: u64,
    hosts: BTreeSet<String>,
    client_errors: u64,
    server_errors: u64,
    waf_hits: u64,
    first_seen: String,
    last_seen: String,
    #[serde(skip)]
    first_timestamp: Option<time::OffsetDateTime>,
    #[serde(skip)]
    last_timestamp: Option<time::OffsetDateTime>,
}

fn accumulate(groups: &mut BTreeMap<String, IpGroup>, entry: &Value) {
    let ip = canonical_ip(&infer_gateway_log_client_ip(entry));
    let group = groups.entry(ip.clone()).or_insert_with(|| IpGroup {
        client_ip: ip,
        ..Default::default()
    });
    group.requests += 1;
    let host = entry.get("host").and_then(Value::as_str).unwrap_or("");
    if !host.is_empty() {
        group.hosts.insert(host.to_string());
    }
    let status = entry.get("status").and_then(Value::as_u64).unwrap_or(0);
    group.client_errors += u64::from((400..500).contains(&status));
    group.server_errors += u64::from((500..600).contains(&status));
    group.waf_hits += u64::from(gateway_log_has_waf_signal(entry));
    let timestamp = entry.get("time").and_then(Value::as_str).unwrap_or("");
    if let Ok(parsed) =
        time::OffsetDateTime::parse(timestamp, &time::format_description::well_known::Rfc3339)
    {
        if group.first_timestamp.is_none_or(|first| parsed < first) {
            group.first_timestamp = Some(parsed);
            group.first_seen = timestamp.to_string();
        }
        if group.last_timestamp.is_none_or(|last| parsed > last) {
            group.last_timestamp = Some(parsed);
            group.last_seen = timestamp.to_string();
        }
    }
}

fn group_response(groups: BTreeMap<String, IpGroup>, query: &GatewayLogQuery, date: &str) -> Value {
    let total_ips = groups.keys().filter(|ip| !ip.is_empty()).count();
    let total_requests: u64 = groups.values().map(|group| group.requests).sum();
    let total = groups.len();
    let mut items: Vec<_> = groups.into_values().collect();
    items.sort_by(|a, b| {
        let order = match query.sort.as_deref().unwrap_or("requests") {
            "last_seen" => b.last_timestamp.cmp(&a.last_timestamp),
            "client_errors" => b.client_errors.cmp(&a.client_errors),
            "server_errors" => b.server_errors.cmp(&a.server_errors),
            "waf_hits" => b.waf_hits.cmp(&a.waf_hits),
            _ => b.requests.cmp(&a.requests),
        };
        order.then_with(|| a.client_ip.cmp(&b.client_ip))
    });
    let limit = normalize_positive_integer(query.limit.as_deref(), 20, 200) as usize;
    let requested_page =
        normalize_positive_integer(query.page.as_deref(), 1, i32::MAX as i64) as usize;
    let page = requested_page.min(total.div_ceil(limit).max(1));
    let items: Vec<_> = items
        .into_iter()
        .skip((page - 1) * limit)
        .take(limit)
        .collect();
    json!({ "date": date, "page": page, "limit": limit, "total": total,
        "total_ips": total_ips, "total_requests": total_requests, "items": items })
}

async fn aggregate(state: &AppState, query: GatewayLogQuery) -> anyhow::Result<Value> {
    aggregate_with(query, |scan| async move {
        go_log_entries(state, &scan, false).await
    })
    .await
}

async fn aggregate_with<F, Fut>(query: GatewayLogQuery, mut fetch: F) -> anyhow::Result<Value>
where
    F: FnMut(GatewayLogQuery) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Value>>,
{
    let waf = normalize_waf_status_filter(query.waf_status.as_deref());
    let mut scan = query.clone();
    scan.pagination = Some("cursor".into());
    scan.cursor = None;
    scan.limit = Some("200".into());
    let mut groups = BTreeMap::new();
    let mut date = query.date.clone().unwrap_or_default();
    loop {
        let data = fetch(scan.clone()).await?;
        if let Some(value) = data.get("date").and_then(Value::as_str) {
            date = value.to_string();
            scan.date = Some(date.clone());
        }
        if let Some(items) = data.get("items").and_then(Value::as_array) {
            for entry in items {
                if matches_entry(entry, query.client_ip.as_deref(), waf) {
                    accumulate(&mut groups, entry);
                }
            }
        }
        if !data
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            break;
        }
        let cursor = data
            .get("next_cursor")
            .and_then(Value::as_str)
            .unwrap_or("");
        anyhow::ensure!(
            !cursor.is_empty() && scan.cursor.as_deref() != Some(cursor),
            "log cursor did not advance"
        );
        scan.cursor = Some(cursor.to_string());
    }
    Ok(group_response(groups, &query, &date))
}

#[utoipa::path(get, path = "/api/admin/gateway-logs/ip-groups", tag = "gateway-logs", operation_id = "get_api_admin_gateway_logs_ip_groups", responses((status = 200, description = "Complete daily request statistics grouped by client IP"), (status = 409, description = "Log cursor expired; refresh the list")))]
pub(super) async fn ip_groups(
    State(state): State<AppState>,
    Query(query): Query<GatewayLogQuery>,
) -> Response {
    if validate_client_ip(query.client_ip.as_deref()).is_err() {
        return response::error(StatusCode::BAD_REQUEST, "invalid client_ip");
    }
    if query.sort.as_deref().is_some_and(|sort| {
        ![
            "requests",
            "last_seen",
            "client_errors",
            "server_errors",
            "waf_hits",
        ]
        .contains(&sort)
    }) {
        return response::error(StatusCode::BAD_REQUEST, "invalid sort");
    }
    let translator = Translator::from_state(&state).await;
    match tokio::time::timeout(GATEWAY_LOG_ANALYTICS_TIMEOUT, aggregate(&state, query)).await {
        Ok(Ok(data)) => response::ok(data).into_response(),
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to aggregate gateway log IPs");
            gateway_log_entries_error(&translator, &error)
        }
        Err(_) => response::error(
            StatusCode::GATEWAY_TIMEOUT,
            gateway_logs_text(&translator, "readEntriesFailed"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scans_every_page_before_filtering_grouping_and_pagination() {
        let mut scanned = 0;
        let query = GatewayLogQuery {
            status: Some("4xx".into()),
            waf_status: Some("has_waf".into()),
            page: Some("2".into()),
            limit: Some("1".into()),
            ..Default::default()
        };
        let result = aggregate_with(query, |scan| {
            assert_eq!(scan.status.as_deref(), Some("4xx"));
            assert_eq!(scan.limit.as_deref(), Some("200"));
            assert_eq!(scan.cursor.as_deref().unwrap_or("0"), scanned.to_string());
            scanned += 1;
            std::future::ready(Ok(json!({"date":"2026-09-21", "has_more": scanned < 205, "next_cursor": scanned.to_string(), "items":[
                {"client_ip":"1.1.1.1", "status":404, "waf_action":"block"},
                {"client_ip":"8.8.8.8", "status":403, "waf_action":"block"},
                {"client_ip":"9.9.9.9", "status":404}
            ]})))
        }).await.unwrap();
        assert_eq!(scanned, 205);
        assert_eq!(result["total_requests"], 410);
        assert_eq!(result["total_ips"], 2);
        assert_eq!(result["items"][0]["client_ip"], "8.8.8.8");
        assert_eq!(result["items"][0]["requests"], 205);
    }

    #[tokio::test]
    async fn pins_the_resolved_day_after_the_first_page() {
        let mut calls = 0;
        let result = aggregate_with(GatewayLogQuery::default(), |scan| {
            calls += 1;
            if calls == 2 { assert_eq!(scan.date.as_deref(), Some("2026-09-21")); }
            std::future::ready(Ok(json!({"date":"2026-09-21", "has_more":calls == 1, "next_cursor":"next", "items":[]})))
        }).await.unwrap();
        assert_eq!(calls, 2);
        assert_eq!(result["date"], "2026-09-21");
    }

    #[tokio::test]
    async fn failed_or_stalled_scans_never_return_partial_totals() {
        let mut calls = 0;
        let result = aggregate_with(GatewayLogQuery::default(), |_| {
            calls += 1;
            std::future::ready(if calls == 1 {
                Ok(json!({"has_more":true,"next_cursor":"one", "items":[{"client_ip":"1.1.1.1"}]}))
            } else {
                Err(anyhow::anyhow!("log cursor expired"))
            })
        })
        .await;
        assert!(result.is_err());
        let result = aggregate_with(GatewayLogQuery::default(), |_| {
            std::future::ready(Ok(json!({"has_more":true,"next_cursor":"same","items":[]})))
        })
        .await;
        assert!(result.is_err());
    }

    #[test]
    fn grouping_counts_all_entries_and_unknowns_with_stable_paging() {
        let mut groups = BTreeMap::new();
        for n in 0..501 {
            accumulate(
                &mut groups,
                &json!({"client_ip":"36.142.108.111", "host":"example.com", "status": if n % 2 == 0 { 404 } else { 200 }, "time":"2026-09-21T10:00:00+08:00"}),
            );
        }
        accumulate(
            &mut groups,
            &json!({"client_ip":"2001:db8::1", "status":503, "waf_action":"block", "time":"2026-09-21T03:00:00Z"}),
        );
        accumulate(&mut groups, &json!({"status":200}));
        let result = group_response(
            groups,
            &GatewayLogQuery {
                limit: Some("1".into()),
                ..Default::default()
            },
            "2026-09-21",
        );
        assert_eq!(result["total_ips"], 2);
        assert_eq!(result["total"], 3);
        assert_eq!(result["total_requests"], 503);
        assert_eq!(result["items"][0]["requests"], 501);
        assert_eq!(result["items"][0]["client_errors"], 251);
    }

    #[test]
    fn exact_match_normalizes_ipv6_and_does_not_search_other_fields() {
        let entry =
            json!({"client_ip":"2001:db8::1", "path":"/36.142.108.111", "waf_action":"block"});
        assert!(matches_entry(
            &entry,
            Some("2001:0db8:0:0:0:0:0:1"),
            Some("has_waf")
        ));
        assert!(!matches_entry(&entry, Some("36.142.108.111"), None));
        assert!(!matches_entry(&entry, Some("2001:db8::1"), Some("none")));
        assert!(matches_entry(&json!({}), Some("unknown"), None));
        assert!(validate_client_ip(Some("bad-ip")).is_err());
        assert!(validate_client_ip(Some("")).is_err());
    }

    #[test]
    fn timestamps_compare_instants_and_hosts_are_distinct() {
        let mut groups = BTreeMap::new();
        for time in ["2026-09-21T10:00:00+08:00", "2026-09-21T03:00:00Z"] {
            accumulate(
                &mut groups,
                &json!({"remote_ip":"192.168.1.1", "eo_connecting_ip":"8.8.8.8", "time":time, "host":"example.com"}),
            );
        }
        let result = group_response(groups, &GatewayLogQuery::default(), "2026-09-21");
        assert_eq!(result["items"][0]["client_ip"], "8.8.8.8");
        assert_eq!(
            result["items"][0]["first_seen"],
            "2026-09-21T10:00:00+08:00"
        );
        assert_eq!(result["items"][0]["last_seen"], "2026-09-21T03:00:00Z");
        assert_eq!(result["items"][0]["hosts"], json!(["example.com"]));
    }
}
