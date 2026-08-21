use std::{fs, process::Command};

use serde_json::{Map, Value, json};
use tokio::time::{self, MissedTickBehavior};

use crate::{state::AppState, system_events, time_utils};

const CPU_STATE_KEY: &str = "fn_knock:events:state:system-resource-monitor:cpu";
const MEMORY_STATE_KEY: &str = "fn_knock:events:state:system-resource-monitor:memory";
const RESOURCE_ALERT_DEDUPE_TTL_SECONDS: i64 = 60;

#[derive(Clone, Copy)]
struct CpuSnapshot {
    idle: u64,
    total: u64,
}

struct MonitorRuntimeState {
    config: Value,
    config_loaded: bool,
    cpu: MetricRuntimeState,
    memory: MetricRuntimeState,
}

struct MetricRuntimeState {
    value: Value,
    dirty: bool,
}

impl MonitorRuntimeState {
    async fn load(state: &AppState) -> Self {
        let config = state.storage.store.get_config().await.ok();
        Self {
            // Configuration changes explicitly wake this task. Keeping the
            // snapshot in memory prevents the five-second sampler from
            // reading SQLite while the appliance is otherwise idle.
            config_loaded: config.is_some(),
            config: config.unwrap_or(Value::Null),
            cpu: MetricRuntimeState {
                value: state
                    .storage
                    .store
                    .get_json_value(CPU_STATE_KEY)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(Value::Null),
                dirty: false,
            },
            memory: MetricRuntimeState {
                value: state
                    .storage
                    .store
                    .get_json_value(MEMORY_STATE_KEY)
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or(Value::Null),
                dirty: false,
            },
        }
    }
}

pub fn start_system_monitor_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("system-resource-monitor", async move {
        let mut runtime = MonitorRuntimeState::load(&task_state).await;
        let mut ticker = time::interval(system_monitor_interval());
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            let reload = tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                _ = ticker.tick() => false,
                _ = task_state.system_monitor_reload_notify.notified() => true,
            };
            if reload {
                runtime = MonitorRuntimeState::load(&task_state).await;
            }
            tokio::select! {
                _ = task_state.shutdown.cancelled() => break,
                result = tick_system_monitor(&task_state, &mut runtime) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "system resource monitor tick failed");
                    }
                }
            }
        }
    });
}

async fn tick_system_monitor(
    state: &AppState,
    runtime: &mut MonitorRuntimeState,
) -> anyhow::Result<()> {
    if !runtime.config_loaded {
        runtime.config = state.storage.store.get_config().await?;
        runtime.config_loaded = true;
    }
    let config = &runtime.config;
    if !event_system_monitor_enabled(config) {
        clear_metric_state(state, CPU_STATE_KEY, &mut runtime.cpu).await?;
        clear_metric_state(state, MEMORY_STATE_KEY, &mut runtime.memory).await?;
        return Ok(());
    }
    process_metric(
        state,
        "cpu",
        event_system_rule_value(config, "cpu_alert"),
        &mut runtime.cpu,
    )
    .await?;
    process_metric(
        state,
        "memory",
        event_system_rule_value(config, "memory_alert"),
        &mut runtime.memory,
    )
    .await?;
    Ok(())
}

async fn process_metric(
    state: &AppState,
    metric: &str,
    rule: Option<&Value>,
    current: &mut MetricRuntimeState,
) -> anyhow::Result<()> {
    let rule = normalize_rule(rule);
    let state_key = metric_state_key(metric);
    if !rule.enabled {
        clear_metric_state(state, state_key, current).await?;
        return Ok(());
    }

    let now = time_utils::now_ms();
    let sample_interval_ms = rule.sample_interval_seconds.max(1) * 1000;
    if current
        .value
        .get("lastSampleAt")
        .and_then(Value::as_i64)
        .is_some_and(|last| now - last < sample_interval_ms)
    {
        return Ok(());
    }

    let (usage_percent, cpu_snapshot) = match metric {
        "cpu" => read_cpu_usage_percent(current.value.get("cpuSnapshot")),
        _ => (read_memory_usage_percent(), None),
    };

    let mut next = current
        .value
        .as_object()
        .cloned()
        .unwrap_or_else(|| Map::from_iter([("status".to_string(), json!("normal"))]));
    next.insert("lastSampleAt".to_string(), json!(now));
    if let Some(snapshot) = cpu_snapshot {
        next.insert(
            "cpuSnapshot".to_string(),
            json!({ "idle": snapshot.idle, "total": snapshot.total }),
        );
    }

    let Some(usage_percent) = usage_percent else {
        update_metric_state(state, state_key, current, Value::Object(next)).await?;
        return Ok(());
    };
    next.insert("lastUsagePercent".to_string(), json!(usage_percent));
    let status = next
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("normal")
        .to_string();
    let sustain_ms = rule.sustain_seconds.max(1) * 1000;

    if usage_percent >= rule.threshold_percent {
        let above_since = next
            .get("aboveThresholdSince")
            .and_then(Value::as_i64)
            .unwrap_or(now);
        next.insert("aboveThresholdSince".to_string(), json!(above_since));
        next.insert("belowRecoverSince".to_string(), Value::Null);
        if status != "alert" && now - above_since >= sustain_ms {
            let published =
                publish_resource_event(state, metric, false, &rule, usage_percent, above_since)
                    .await?;
            if published {
                next.insert("status".to_string(), json!("alert"));
            } else {
                return Ok(());
            }
        }
        update_metric_state(state, state_key, current, Value::Object(next)).await?;
        return Ok(());
    }

    if usage_percent <= rule.recover_percent {
        if status == "alert" {
            let below_since = next
                .get("belowRecoverSince")
                .and_then(Value::as_i64)
                .unwrap_or(now);
            next.insert("belowRecoverSince".to_string(), json!(below_since));
            next.insert("aboveThresholdSince".to_string(), Value::Null);
            if now - below_since >= sustain_ms {
                let published =
                    publish_resource_event(state, metric, true, &rule, usage_percent, below_since)
                        .await?;
                if published {
                    next.insert("status".to_string(), json!("normal"));
                    next.insert("belowRecoverSince".to_string(), Value::Null);
                } else {
                    return Ok(());
                }
            }
        } else {
            next.insert("aboveThresholdSince".to_string(), Value::Null);
            next.insert("belowRecoverSince".to_string(), Value::Null);
        }
        update_metric_state(state, state_key, current, Value::Object(next)).await?;
        return Ok(());
    }

    if status == "alert" {
        next.insert("belowRecoverSince".to_string(), Value::Null);
    } else {
        next.insert("aboveThresholdSince".to_string(), Value::Null);
    }
    update_metric_state(state, state_key, current, Value::Object(next)).await?;
    Ok(())
}

async fn update_metric_state(
    state: &AppState,
    state_key: &str,
    current: &mut MetricRuntimeState,
    next: Value,
) -> anyhow::Result<()> {
    let previous_status = current
        .value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("normal");
    let next_status = next
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("normal");
    let persist = current.dirty || previous_status != next_status;
    current.value = next;
    if persist {
        let result = state
            .storage
            .store
            .set_json_value(state_key, &current.value)
            .await;
        current.dirty = result.is_err();
        result?;
    }
    Ok(())
}

async fn clear_metric_state(
    state: &AppState,
    state_key: &str,
    current: &mut MetricRuntimeState,
) -> anyhow::Result<()> {
    if !current.value.is_null() {
        state.storage.store.delete_key(state_key).await?;
        current.value = Value::Null;
        current.dirty = false;
    }
    Ok(())
}

async fn publish_resource_event(
    state: &AppState,
    metric: &str,
    recovered: bool,
    rule: &MonitorRule,
    usage_percent: f64,
    transition_since: i64,
) -> anyhow::Result<bool> {
    let hostname = hostname();
    system_events::publish_resource_alert_event(
        state,
        metric,
        &hostname,
        recovered,
        format!(
            "resource-alert:{hostname}:{metric}:{}:{transition_since}",
            if recovered { "recovered" } else { "alert" }
        ),
        RESOURCE_ALERT_DEDUPE_TTL_SECONDS,
        json!({
            "hostname": hostname,
            "usage_percent": usage_percent,
            "threshold_percent": rule.threshold_percent,
            "recover_percent": rule.recover_percent,
            "sample_interval_seconds": rule.sample_interval_seconds,
            "sustain_seconds": rule.sustain_seconds,
        }),
    )
    .await
}

pub(crate) async fn reset_states(state: &AppState) {
    let _ = state.storage.store.delete_key(CPU_STATE_KEY).await;
    let _ = state.storage.store.delete_key(MEMORY_STATE_KEY).await;
    state.request_system_monitor_reload();
}

fn read_cpu_usage_percent(previous: Option<&Value>) -> (Option<f64>, Option<CpuSnapshot>) {
    let Some(snapshot) = read_cpu_snapshot() else {
        return (None, None);
    };
    let previous = previous.and_then(parse_cpu_snapshot);
    let usage = previous.and_then(|old| {
        let total_delta = snapshot.total.saturating_sub(old.total);
        let idle_delta = snapshot.idle.saturating_sub(old.idle);
        if total_delta == 0 {
            None
        } else {
            Some(clamp_percent(
                (1.0 - idle_delta as f64 / total_delta as f64) * 100.0,
            ))
        }
    });
    (usage, Some(snapshot))
}

fn read_cpu_snapshot() -> Option<CpuSnapshot> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().find(|line| line.starts_with("cpu "))?;
    let values = line
        .split_whitespace()
        .skip(1)
        .filter_map(|value| value.parse::<u64>().ok())
        .collect::<Vec<_>>();
    if values.len() < 4 {
        return None;
    }
    let idle = values.get(3).copied().unwrap_or(0) + values.get(4).copied().unwrap_or(0);
    let total = values.iter().sum();
    Some(CpuSnapshot { idle, total })
}

fn parse_cpu_snapshot(value: &Value) -> Option<CpuSnapshot> {
    Some(CpuSnapshot {
        idle: value.get("idle")?.as_u64()?,
        total: value.get("total")?.as_u64()?,
    })
}

fn read_memory_usage_percent() -> Option<f64> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let mut total_kb = None;
    let mut available_kb = None;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            total_kb = parse_meminfo_kb(line);
        } else if line.starts_with("MemAvailable:") {
            available_kb = parse_meminfo_kb(line);
        }
        if total_kb.is_some() && available_kb.is_some() {
            break;
        }
    }
    let total = total_kb?;
    let available = available_kb?.min(total);
    if total == 0 {
        return Some(0.0);
    }
    Some(clamp_percent(
        ((total - available) as f64 / total as f64) * 100.0,
    ))
}

fn parse_meminfo_kb(line: &str) -> Option<u64> {
    line.split_whitespace().nth(1)?.parse::<u64>().ok()
}

fn clamp_percent(value: f64) -> f64 {
    ((value.clamp(0.0, 100.0) * 10.0).round()) / 10.0
}

fn metric_state_key(metric: &str) -> &'static str {
    if metric == "cpu" {
        CPU_STATE_KEY
    } else {
        MEMORY_STATE_KEY
    }
}

#[derive(Clone)]
struct MonitorRule {
    enabled: bool,
    threshold_percent: f64,
    recover_percent: f64,
    sample_interval_seconds: i64,
    sustain_seconds: i64,
}

fn event_system_monitor_enabled(config: &Value) -> bool {
    config
        .get("event_system")
        .and_then(Value::as_object)
        .and_then(|value| value.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn event_system_rule_value<'a>(config: &'a Value, key: &str) -> Option<&'a Value> {
    config
        .get("event_system")
        .and_then(Value::as_object)
        .and_then(|value| value.get("rules"))
        .and_then(Value::as_object)
        .and_then(|value| value.get(key))
}

fn normalize_rule(value: Option<&Value>) -> MonitorRule {
    let threshold_percent = bounded_int_field(value, "threshold_percent", 80, 1, 100);
    MonitorRule {
        enabled: value
            .and_then(|value| value.get("enabled"))
            .and_then(Value::as_bool)
            .unwrap_or(true),
        threshold_percent: threshold_percent as f64,
        recover_percent: bounded_int_field(value, "recover_percent", 60, 0, threshold_percent)
            as f64,
        sample_interval_seconds: bounded_int_field(value, "sample_interval_seconds", 5, 5, 3600),
        sustain_seconds: bounded_int_field(value, "sustain_seconds", 30, 10, 24 * 3600),
    }
}

fn bounded_int_field(value: Option<&Value>, key: &str, fallback: i64, min: i64, max: i64) -> i64 {
    int_field(value, key, fallback).clamp(min, max)
}

fn int_field(value: Option<&Value>, key: &str, fallback: i64) -> i64 {
    value
        .and_then(|value| value.get(key))
        .and_then(parse_js_int)
        .unwrap_or(fallback)
}

fn parse_js_int(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return Some(value);
    }
    if let Some(value) = value.as_u64() {
        return i64::try_from(value).ok();
    }
    if let Some(value) = value.as_f64() {
        return value.is_finite().then_some(value.trunc() as i64);
    }
    crate::node_compat::parse_i64_prefix_trim_start(value.as_str()?)
}

fn system_monitor_interval() -> std::time::Duration {
    let cron = std::env::var("SYSTEM_MONITOR_CRON").ok();
    let interval_seconds = std::env::var("SYSTEM_MONITOR_INTERVAL_SECONDS").ok();
    system_monitor_interval_from_values(cron.as_deref(), interval_seconds.as_deref())
}

fn system_monitor_interval_from_values(
    cron: Option<&str>,
    interval_seconds: Option<&str>,
) -> std::time::Duration {
    let seconds = cron
        .filter(|value| !value.trim().is_empty())
        .map(|value| crate::settings::parse_cron_interval_seconds(value, 5) as i64)
        .unwrap_or_else(|| parse_env_int_like_node(interval_seconds, 5))
        .clamp(1, 300) as u64;
    std::time::Duration::from_secs(seconds)
}

fn parse_env_int_like_node(value: Option<&str>, fallback: i64) -> i64 {
    crate::node_compat::parse_i64_or(value, fallback)
}

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            Command::new("hostname")
                .output()
                .ok()
                .and_then(|output| String::from_utf8(output.stdout).ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .unwrap_or_else(|| "unknown-host".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_system_defaults_to_enabled_for_legacy_configs() {
        assert!(event_system_monitor_enabled(&json!({})));
        assert!(event_system_monitor_enabled(&json!({
            "event_system": {
                "rules": {}
            }
        })));
        assert!(event_system_monitor_enabled(&json!({
            "event_system": null
        })));
        assert!(!event_system_monitor_enabled(&json!({
            "event_system": {
                "enabled": false
            }
        })));
    }

    #[test]
    fn event_system_rule_value_reads_nested_resource_rules() {
        let config = json!({
            "event_system": {
                "rules": {
                    "cpu_alert": {
                        "threshold_percent": 70
                    }
                }
            }
        });

        assert_eq!(
            event_system_rule_value(&config, "cpu_alert")
                .and_then(|value| value.get("threshold_percent"))
                .and_then(Value::as_i64),
            Some(70)
        );
        assert!(event_system_rule_value(&config, "memory_alert").is_none());
    }

    #[test]
    fn normalize_rule_defaults_match_node_event_system_defaults() {
        let rule = normalize_rule(None);

        assert!(rule.enabled);
        assert_eq!(rule.threshold_percent, 80.0);
        assert_eq!(rule.recover_percent, 60.0);
        assert_eq!(rule.sample_interval_seconds, 5);
        assert_eq!(rule.sustain_seconds, 30);
    }

    #[test]
    fn normalize_rule_uses_node_bounds_and_recover_cannot_exceed_threshold() {
        let value = json!({
            "threshold_percent": 50,
            "recover_percent": 75,
            "sample_interval_seconds": 1,
            "sustain_seconds": 5
        });
        let rule = normalize_rule(Some(&value));

        assert!(rule.enabled);
        assert_eq!(rule.threshold_percent, 50.0);
        assert_eq!(rule.recover_percent, 50.0);
        assert_eq!(rule.sample_interval_seconds, 5);
        assert_eq!(rule.sustain_seconds, 10);
    }

    #[test]
    fn normalize_rule_accepts_node_parse_int_style_values() {
        let value = json!({
            "enabled": false,
            "threshold_percent": "95%",
            "recover_percent": " 70 seconds",
            "sample_interval_seconds": 12.9,
            "sustain_seconds": "+45s"
        });
        let rule = normalize_rule(Some(&value));

        assert!(!rule.enabled);
        assert_eq!(rule.threshold_percent, 95.0);
        assert_eq!(rule.recover_percent, 70.0);
        assert_eq!(rule.sample_interval_seconds, 12);
        assert_eq!(rule.sustain_seconds, 45);
    }

    #[test]
    fn env_int_parser_matches_node_parse_int_edges() {
        assert_eq!(parse_env_int_like_node(None, 30), 30);
        assert_eq!(parse_env_int_like_node(Some("60s"), 30), 60);
        assert_eq!(parse_env_int_like_node(Some("  +3.9"), 30), 3);
        assert_eq!(parse_env_int_like_node(Some("0x10"), 30), 0);
        assert_eq!(parse_env_int_like_node(Some("nope"), 30), 30);
    }

    #[test]
    fn system_monitor_interval_prefers_node_cron_env() {
        assert_eq!(
            system_monitor_interval_from_values(Some("*/5 * * * * *"), Some("60")).as_secs(),
            5
        );
        assert_eq!(
            system_monitor_interval_from_values(Some("*/2 * * * *"), None).as_secs(),
            120
        );
        assert_eq!(
            system_monitor_interval_from_values(None, Some("60s")).as_secs(),
            60
        );
    }
}
