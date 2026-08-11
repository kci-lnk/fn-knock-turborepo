use std::{collections::BTreeMap, future::Future, time::Duration};

use serde_json::{Value, json};

use crate::{ip_location, json_utils::ensure_object, state::AppState};

const GEO_BUCKET_LIMIT: usize = 8;
const GEO_REFRESH_LOCK_KEY: &str = "fn_knock:gateway_logs:analytics:geo_refresh";
const GEO_REFRESH_LOCK_TTL_SECONDS: usize = 60;
const GEO_REFRESH_WAIT_SECONDS: u64 = 30 * 60;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct GeoRegionKey {
    country_code: String,
    province: String,
    city: String,
}

pub(super) async fn hydrate_analytics_response(state: &AppState, mut data: Value) -> Value {
    let requests = data
        .get("summary")
        .and_then(|value| value.get("requests"))
        .and_then(Value::as_i64)
        .unwrap_or(0);
    hydrate_dimension_shares(&mut data, requests);

    let clients = take_internal_clients(&mut data);
    let total_clients = data
        .get("summary")
        .and_then(|value| value.get("unique_clients"))
        .and_then(Value::as_i64)
        .unwrap_or(0)
        .max(0);

    let mut countries = BTreeMap::<String, i64>::new();
    let mut regions = BTreeMap::<GeoRegionKey, i64>::new();
    let mut resolved_clients = 0_i64;
    let mut resolved_region_clients = 0_i64;
    let mut pending_clients = 0_i64;

    for client in clients {
        let Some(ip) = client.get("ip").and_then(Value::as_str) else {
            continue;
        };
        match ip_location::get_ip_location_snapshot(state, ip).await {
            Ok(snapshot) => {
                let status = snapshot.get("status").and_then(Value::as_str).unwrap_or("");
                if matches!(status, "queued" | "processing") {
                    pending_clients += 1;
                    continue;
                }
                if status != "success" {
                    continue;
                }
                let country_code = snapshot
                    .get("result")
                    .and_then(|value| value.get("countryCode"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_ascii_uppercase();
                if country_code.len() != 2
                    || !country_code
                        .bytes()
                        .all(|value| value.is_ascii_alphabetic())
                {
                    continue;
                }
                resolved_clients += 1;
                *countries.entry(country_code.clone()).or_default() += 1;

                let province = snapshot
                    .get("result")
                    .and_then(|value| value.get("province"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                let city = snapshot
                    .get("result")
                    .and_then(|value| value.get("city"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !province.is_empty() || !city.is_empty() {
                    resolved_region_clients += 1;
                    *regions
                        .entry(GeoRegionKey {
                            country_code,
                            province,
                            city,
                        })
                        .or_default() += 1;
                }
            }
            Err(error) => {
                tracing::warn!(%error, "failed to read gateway analytics IP location cache");
            }
        }
    }

    let refreshing = is_geo_refresh_active(state).await.unwrap_or(false);
    let mut geo = build_geo_summary(
        total_clients,
        resolved_clients,
        resolved_region_clients,
        pending_clients,
        countries,
        regions,
    );
    ensure_object(&mut geo).insert("refreshing".to_string(), json!(refreshing));
    ensure_object(&mut data).insert("geo".to_string(), geo);
    data
}

pub(super) async fn try_acquire_geo_refresh(
    state: &AppState,
) -> crate::storage::StorageResult<Option<String>> {
    let lock_id = uuid::Uuid::new_v4().to_string();
    let acquired = state
        .storage
        .store
        .set_json_value_nx_ex(
            GEO_REFRESH_LOCK_KEY,
            &json!({
                "lockId": lock_id,
                "startedAt": crate::time_utils::now_ms(),
            }),
            GEO_REFRESH_LOCK_TTL_SECONDS,
        )
        .await?;
    Ok(acquired.then_some(lock_id))
}

pub(super) async fn release_geo_refresh(state: &AppState, lock_id: &str) {
    if let Err(error) = state
        .storage
        .store
        .delete_lock_if_owned(GEO_REFRESH_LOCK_KEY, lock_id)
        .await
    {
        tracing::warn!(%error, "failed to release gateway analytics geo refresh lock");
    }
}

async fn renew_geo_refresh(state: &AppState, lock_id: &str) -> crate::storage::StorageResult<bool> {
    state
        .storage
        .store
        .set_json_lock_if_owned_ex(
            GEO_REFRESH_LOCK_KEY,
            lock_id,
            &json!({
                "lockId": lock_id,
                "refreshedAt": crate::time_utils::now_ms(),
            }),
            GEO_REFRESH_LOCK_TTL_SECONDS,
        )
        .await
}

pub(super) async fn with_geo_refresh_lease<T>(
    state: &AppState,
    lock_id: &str,
    work: impl Future<Output = anyhow::Result<T>>,
) -> anyhow::Result<T> {
    tokio::pin!(work);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(15));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            result = &mut work => return result,
            _ = heartbeat.tick() => {
                if !renew_geo_refresh(state, lock_id).await? {
                    anyhow::bail!("gateway analytics geo refresh lock was lost");
                }
            }
        }
    }
}

pub(super) fn spawn_geo_refresh(state: &AppState, mut data: Value, lock_id: String) {
    let ips = take_internal_clients(&mut data)
        .into_iter()
        .filter_map(|client| {
            client
                .get("ip")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|ip| !ip.is_empty())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();
    let task_state = state.clone();
    state.spawn_background("gateway-analytics-geo-refresh", async move {
        if let Err(error) = run_geo_refresh(&task_state, ips, &lock_id).await {
            tracing::warn!(%error, "gateway analytics geo refresh failed");
        }
        release_geo_refresh(&task_state, &lock_id).await;
    });
}

async fn is_geo_refresh_active(state: &AppState) -> crate::storage::StorageResult<bool> {
    Ok(state
        .storage
        .store
        .get_json_value(GEO_REFRESH_LOCK_KEY)
        .await?
        .is_some())
}

async fn run_geo_refresh(state: &AppState, ips: Vec<String>, lock_id: &str) -> anyhow::Result<()> {
    with_geo_refresh_lease(state, lock_id, run_geo_refresh_work(state, ips)).await
}

async fn run_geo_refresh_work(state: &AppState, ips: Vec<String>) -> anyhow::Result<()> {
    ip_location::ensure_ip_locations_enqueued(state, ips.clone()).await?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(GEO_REFRESH_WAIT_SECONDS);
    loop {
        let mut active = false;
        for ip in &ips {
            let snapshot = ip_location::get_ip_location_snapshot(state, ip).await?;
            let status = snapshot.get("status").and_then(Value::as_str).unwrap_or("");
            if matches!(status, "queued" | "processing") {
                active = true;
                break;
            }
        }
        if !active || tokio::time::Instant::now() >= deadline {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

fn take_internal_clients(data: &mut Value) -> Vec<Value> {
    ensure_object(data)
        .remove("clients")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
}

fn build_geo_summary(
    total_clients: i64,
    resolved_clients: i64,
    resolved_region_clients: i64,
    pending_clients: i64,
    countries: BTreeMap<String, i64>,
    regions: BTreeMap<GeoRegionKey, i64>,
) -> Value {
    let items = build_country_items(total_clients, resolved_clients, countries);
    let region_items = build_region_items(total_clients, resolved_region_clients, regions);
    let status = geo_status(total_clients, resolved_clients, pending_clients);
    let region_status = geo_status(total_clients, resolved_region_clients, pending_clients);
    let coverage = if total_clients > 0 {
        resolved_clients as f64 / total_clients as f64
    } else {
        1.0
    };
    let region_coverage = if total_clients > 0 {
        resolved_region_clients as f64 / total_clients as f64
    } else {
        1.0
    };
    json!({
        "status": status,
        "region_status": region_status,
        "resolved_clients": resolved_clients,
        "resolved_region_clients": resolved_region_clients,
        "pending_clients": pending_clients,
        "total_clients": total_clients,
        "coverage": coverage,
        "region_coverage": region_coverage,
        "items": items,
        "regions": region_items,
    })
}

fn geo_status(total_clients: i64, resolved_clients: i64, pending_clients: i64) -> &'static str {
    if total_clients == 0 || resolved_clients >= total_clients {
        "complete"
    } else if pending_clients > 0 {
        "resolving"
    } else {
        "partial"
    }
}

fn visible_geo_limit(item_count: usize, unresolved_clients: i64) -> usize {
    if unresolved_clients > 0 || item_count > GEO_BUCKET_LIMIT {
        GEO_BUCKET_LIMIT.saturating_sub(1)
    } else {
        GEO_BUCKET_LIMIT
    }
}

fn build_country_items(
    total_clients: i64,
    resolved_clients: i64,
    countries: BTreeMap<String, i64>,
) -> Vec<Value> {
    let mut items = countries.into_iter().collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let unresolved_clients = (total_clients - resolved_clients).max(0);
    let visible_limit = visible_geo_limit(items.len(), unresolved_clients);
    let overflow_clients = if items.len() > visible_limit {
        items
            .drain(visible_limit..)
            .map(|(_, count)| count)
            .sum::<i64>()
    } else {
        0
    };
    let unknown_clients = unresolved_clients + overflow_clients;
    if unknown_clients > 0 {
        items.push(("unknown".to_string(), unknown_clients));
    }
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items
        .into_iter()
        .map(|(key, count)| analytics_bucket(key, count, total_clients))
        .collect()
}

fn build_region_items(
    total_clients: i64,
    resolved_clients: i64,
    regions: BTreeMap<GeoRegionKey, i64>,
) -> Vec<Value> {
    let mut items = regions.into_iter().collect::<Vec<_>>();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let unresolved_clients = (total_clients - resolved_clients).max(0);
    let visible_limit = visible_geo_limit(items.len(), unresolved_clients);
    let overflow_clients = if items.len() > visible_limit {
        items
            .drain(visible_limit..)
            .map(|(_, count)| count)
            .sum::<i64>()
    } else {
        0
    };

    let mut result = items
        .into_iter()
        .map(|(region, count)| {
            let key = format!(
                "{}|{}|{}",
                region.country_code, region.province, region.city
            );
            let mut bucket = analytics_bucket(key, count, total_clients);
            let object = ensure_object(&mut bucket);
            object.insert("country_code".to_string(), json!(region.country_code));
            object.insert("province".to_string(), json!(region.province));
            object.insert("city".to_string(), json!(region.city));
            bucket
        })
        .collect::<Vec<_>>();
    let unknown_clients = unresolved_clients + overflow_clients;
    if unknown_clients > 0 {
        result.push(analytics_bucket(
            "unknown".to_string(),
            unknown_clients,
            total_clients,
        ));
    }
    result.sort_by(|left, right| {
        let left_count = left.get("count").and_then(Value::as_i64).unwrap_or(0);
        let right_count = right.get("count").and_then(Value::as_i64).unwrap_or(0);
        let left_key = left.get("key").and_then(Value::as_str).unwrap_or("");
        let right_key = right.get("key").and_then(Value::as_str).unwrap_or("");
        right_count
            .cmp(&left_count)
            .then_with(|| left_key.cmp(right_key))
    });
    result
}

fn hydrate_dimension_shares(data: &mut Value, requests: i64) {
    let Some(dimensions) = data.get_mut("dimensions").and_then(Value::as_object_mut) else {
        return;
    };
    for items in dimensions.values_mut() {
        let Some(items) = items.as_array_mut() else {
            continue;
        };
        for item in items {
            let count = item.get("count").and_then(Value::as_i64).unwrap_or(0);
            ensure_object(item).insert(
                "share".to_string(),
                json!(if requests > 0 {
                    count as f64 / requests as f64
                } else {
                    0.0
                }),
            );
        }
    }
}

fn analytics_bucket(key: String, count: i64, total: i64) -> Value {
    json!({
        "key": key,
        "count": count,
        "share": if total > 0 { count as f64 / total as f64 } else { 0.0 },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimension_shares_use_total_requests() {
        let mut data = json!({
            "dimensions": {
                "methods": [{ "key": "GET", "count": 3 }]
            }
        });
        hydrate_dimension_shares(&mut data, 4);
        assert_eq!(data["dimensions"]["methods"][0]["share"], 0.75);
    }

    #[test]
    fn internal_client_ips_are_removed_before_the_http_response() {
        let mut data = json!({
            "summary": { "unique_clients": 1 },
            "clients": [{ "ip": "203.0.113.7", "count": 3 }]
        });

        let clients = take_internal_clients(&mut data);

        assert_eq!(clients.len(), 1);
        assert!(data.get("clients").is_none());
        assert!(!data.to_string().contains("203.0.113.7"));
    }

    #[test]
    fn geo_summary_limits_rows_and_reserves_unknown_bucket() {
        let countries = (0..9)
            .map(|index| (format!("C{index}"), 1_i64))
            .collect::<BTreeMap<_, _>>();

        let geo = build_geo_summary(10, 9, 0, 1, countries, BTreeMap::new());
        let items = geo["items"].as_array().expect("geo items");

        assert_eq!(geo["status"], "resolving");
        assert_eq!(geo["coverage"], 0.9);
        assert_eq!(items.len(), GEO_BUCKET_LIMIT);
        assert_eq!(
            items.first().and_then(|item| item["key"].as_str()),
            Some("unknown")
        );
        assert_eq!(
            items.first().and_then(|item| item["count"].as_i64()),
            Some(3)
        );
    }

    #[test]
    fn geo_summary_exposes_region_buckets_without_client_ips() {
        let regions = BTreeMap::from([
            (
                GeoRegionKey {
                    country_code: "CN".to_string(),
                    province: "广东省".to_string(),
                    city: "深圳市".to_string(),
                },
                3,
            ),
            (
                GeoRegionKey {
                    country_code: "US".to_string(),
                    province: "California".to_string(),
                    city: "Los Angeles".to_string(),
                },
                1,
            ),
        ]);

        let geo = build_geo_summary(5, 5, 4, 0, BTreeMap::new(), regions);
        let items = geo["regions"].as_array().expect("region items");

        assert_eq!(geo["region_status"], "partial");
        assert_eq!(geo["region_coverage"], 0.8);
        assert_eq!(items[0]["province"], "广东省");
        assert_eq!(items[0]["city"], "深圳市");
        assert!(items.iter().any(|item| item["key"] == "unknown"));
        assert!(!geo.to_string().contains("203.0.113"));
    }
}
