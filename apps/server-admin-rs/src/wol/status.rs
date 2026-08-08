use fn_knock_wol_protocol::MacAddress;
use serde::Serialize;
use std::{net::Ipv4Addr, time::Duration};
use tokio::{task::JoinSet, time};

use crate::{state::AppState, time_utils};

use super::{
    dispatch::{DispatchError, dispatch_status},
    probe::{DeviceProbeResult, DeviceProbeState},
    secrets::secret_store,
    store::{
        TargetRecord, TargetStatusRecord, list_targets, load_relay, load_target,
        load_target_status, save_target_status,
    },
};

const CHECK_INTERVAL: Duration = Duration::from_secs(60);
const STATUS_STALE_AFTER_MS: i64 = 120_000;
const CHECK_CONCURRENCY: usize = 16;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TargetStatusView {
    pub state: String,
    pub checked_at: Option<String>,
    pub last_online_at: Option<String>,
    pub observed_ip: Option<String>,
    pub last_error: Option<String>,
}

pub(crate) fn start_wol_tasks(state: AppState) {
    super::relay::start_wol_relay_tasks(state.clone());
    tokio::spawn(async move {
        status_supervisor(state).await;
    });
}

async fn status_supervisor(state: AppState) {
    let mut runtime_reload = state.wol_runtime_reload.subscribe();
    loop {
        if state.shutdown.is_cancelled() {
            return;
        }
        match super::feature_enabled_for_state(&state).await {
            Ok(true) => {
                if run_status_cycle(&state, &mut runtime_reload).await {
                    continue;
                }
            }
            Ok(false) => {
                tokio::select! {
                    _ = state.shutdown.cancelled() => return,
                    _ = runtime_reload.changed() => continue,
                }
            }
            Err(error) => tracing::warn!(%error, "failed to load WoL feature for status worker"),
        }
        tokio::select! {
            _ = state.shutdown.cancelled() => return,
            _ = runtime_reload.changed() => continue,
            _ = time::sleep(CHECK_INTERVAL) => {}
        }
    }
}

/// Returns `true` when a runtime reload or shutdown interrupted the cycle.
async fn run_status_cycle(
    state: &AppState,
    runtime_reload: &mut tokio::sync::watch::Receiver<u64>,
) -> bool {
    let targets = match list_targets(state).await {
        Ok(targets) => targets,
        Err(error) => {
            tracing::warn!(%error, "failed to load WoL targets for online checks");
            return false;
        }
    };
    let mut targets = targets.into_iter().filter(|target| target.enabled);
    let mut tasks = JoinSet::new();
    for _ in 0..CHECK_CONCURRENCY {
        let Some(target) = targets.next() else {
            break;
        };
        spawn_target_check(&mut tasks, state, target);
    }
    while !tasks.is_empty() {
        tokio::select! {
            _ = state.shutdown.cancelled() => {
                tasks.abort_all();
                return true;
            }
            _ = runtime_reload.changed() => {
                tasks.abort_all();
                return true;
            }
            result = tasks.join_next() => {
                if let Some(Err(error)) = result {
                    tracing::warn!(%error, "WoL online check task failed");
                }
                if let Some(target) = targets.next() {
                    spawn_target_check(&mut tasks, state, target);
                }
            }
        }
    }
    false
}

fn spawn_target_check(tasks: &mut JoinSet<()>, state: &AppState, target: TargetRecord) {
    let state = state.clone();
    tasks.spawn(async move {
        if let Err(error) = check_target(&state, target).await {
            tracing::warn!(%error, "failed to persist WoL target status");
        }
    });
}

pub(super) fn schedule_target_rechecks(state: AppState, target_id: String) {
    tokio::spawn(async move {
        time::sleep(Duration::from_secs(5)).await;
        let _ = check_target_by_id(&state, &target_id).await;
        time::sleep(Duration::from_secs(15)).await;
        let _ = check_target_by_id(&state, &target_id).await;
    });
}

pub(super) async fn check_target_by_id(state: &AppState, id: &str) -> anyhow::Result<()> {
    if !super::feature_enabled_for_state(state).await? {
        return Ok(());
    }
    let Some(target) = load_target(state, id).await? else {
        return Ok(());
    };
    if !target.enabled {
        return Ok(());
    }
    check_target(state, target).await
}

async fn check_target(state: &AppState, target: TargetRecord) -> anyhow::Result<()> {
    let previous = load_target_status(state, &target.id).await?;
    let configured_ip = target
        .ip_address
        .as_deref()
        .and_then(|value| value.parse::<Ipv4Addr>().ok());
    let observed_ip = previous
        .observed_ip
        .as_deref()
        .and_then(|value| value.parse::<Ipv4Addr>().ok());
    let preferred_ip = configured_ip.or(observed_ip);
    let mac = match target.mac.parse::<MacAddress>() {
        Ok(mac) => mac,
        Err(_) => {
            return persist_result_if_current(
                state,
                &target,
                previous,
                DeviceProbeResult {
                    state: DeviceProbeState::Unknown,
                    observed_ip: preferred_ip,
                    error: Some("Target MAC address is invalid".to_string()),
                },
            )
            .await;
        }
    };

    let result = match target.relay_id.as_deref() {
        Some(relay_id) => remote_probe(state, relay_id, mac, preferred_ip).await,
        None => {
            super::probe::probe_device_candidates_bounded(
                mac,
                [configured_ip, observed_ip].into_iter().flatten(),
            )
            .await
        }
    };
    persist_result_if_current(state, &target, previous, result).await
}

async fn remote_probe(
    state: &AppState,
    relay_id: &str,
    mac: MacAddress,
    preferred_ip: Option<Ipv4Addr>,
) -> DeviceProbeResult {
    let relay = match load_relay(state, relay_id).await {
        Ok(Some(relay)) if relay.enabled => relay,
        Ok(_) => return unknown(preferred_ip, "Relay is unavailable"),
        Err(error) => return unknown(preferred_ip, error.to_string()),
    };
    let psk = match secret_store(state).read(&relay.id, relay.key_version) {
        Ok(Some(psk)) => psk,
        Ok(None) => return unknown(preferred_ip, "Relay PSK is not configured"),
        Err(error) => return unknown(preferred_ip, error),
    };
    match dispatch_status(&relay, &psk, mac, preferred_ip).await {
        Ok(result) => DeviceProbeResult {
            state: result.state,
            observed_ip: result.observed_ip.or(preferred_ip),
            error: None,
        },
        Err(error) => unknown(preferred_ip, dispatch_error_text(&error)),
    }
}

fn unknown(preferred_ip: Option<Ipv4Addr>, error: impl Into<String>) -> DeviceProbeResult {
    DeviceProbeResult {
        state: DeviceProbeState::Unknown,
        observed_ip: preferred_ip,
        error: Some(error.into()),
    }
}

fn dispatch_error_text(error: &DispatchError) -> String {
    match error {
        DispatchError::Network { message, .. } => message.clone(),
        DispatchError::Timeout { .. } => "Relay status request timed out".to_string(),
        DispatchError::Relay { status, .. } => format!("Relay returned {status:?}"),
    }
}

async fn persist_result(
    state: &AppState,
    id: &str,
    previous: TargetStatusRecord,
    result: DeviceProbeResult,
) -> anyhow::Result<()> {
    let now = time_utils::now_iso();
    let online = result.state == DeviceProbeState::Online;
    save_target_status(
        state,
        id,
        &TargetStatusRecord {
            state: state_name(result.state).to_string(),
            checked_at: Some(now.clone()),
            last_online_at: if online {
                Some(now)
            } else {
                previous.last_online_at
            },
            observed_ip: result
                .observed_ip
                .map(|value| value.to_string())
                .or(previous.observed_ip),
            last_error: result.error,
        },
    )
    .await
}

async fn persist_result_if_current(
    state: &AppState,
    checked_target: &TargetRecord,
    previous: TargetStatusRecord,
    result: DeviceProbeResult,
) -> anyhow::Result<()> {
    // CRUD uses the same lock. Re-read only while holding it so an old probe
    // cannot recreate status after deletion or overwrite the reset performed
    // by an address/MAC/Relay edit.
    let _guard = state.wol_config_lock.lock().await;
    let Some(current) = load_target(state, &checked_target.id).await? else {
        return Ok(());
    };
    if !same_probe_identity(&current, checked_target) {
        return Ok(());
    }
    persist_result(state, &checked_target.id, previous, result).await
}

fn same_probe_identity(current: &TargetRecord, checked: &TargetRecord) -> bool {
    current.enabled
        && current.mac == checked.mac
        && current.ip_address == checked.ip_address
        && current.relay_id == checked.relay_id
}

pub(super) async fn status_view(state: &AppState, id: &str) -> anyhow::Result<TargetStatusView> {
    let record = load_target_status(state, id).await?;
    Ok(status_view_from_record(record, time_utils::now_ms()))
}

fn status_view_from_record(record: TargetStatusRecord, now_ms: i64) -> TargetStatusView {
    let fresh = record
        .checked_at
        .as_deref()
        .and_then(time_utils::parse_iso_ms)
        .is_some_and(|checked_at| {
            let age = now_ms.saturating_sub(checked_at);
            (0..=STATUS_STALE_AFTER_MS).contains(&age)
        });
    let normalized_state = match record.state.as_str() {
        "online" | "offline" | "unknown" => record.state.as_str(),
        _ => "unknown",
    };
    TargetStatusView {
        state: if fresh { normalized_state } else { "unknown" }.to_string(),
        checked_at: record.checked_at,
        last_online_at: record.last_online_at,
        observed_ip: record.observed_ip,
        last_error: record.last_error,
    }
}

fn state_name(state: DeviceProbeState) -> &'static str {
    match state {
        DeviceProbeState::Online => "online",
        DeviceProbeState::Offline => "offline",
        DeviceProbeState::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_status_degrades_to_unknown_without_losing_history() {
        let record = TargetStatusRecord {
            state: "online".to_string(),
            checked_at: Some("2024-01-01T00:00:00Z".to_string()),
            last_online_at: Some("2024-01-01T00:00:00Z".to_string()),
            observed_ip: Some("192.0.2.10".to_string()),
            last_error: None,
        };
        let view = status_view_from_record(record, 1_704_067_321_000);
        assert_eq!(view.state, "unknown");
        assert_eq!(view.last_online_at.as_deref(), Some("2024-01-01T00:00:00Z"));
        assert_eq!(view.observed_ip.as_deref(), Some("192.0.2.10"));
    }

    #[test]
    fn future_or_invalid_status_never_reports_online() {
        for record in [
            TargetStatusRecord {
                state: "online".to_string(),
                checked_at: Some("2099-01-01T00:00:00Z".to_string()),
                ..TargetStatusRecord::default()
            },
            TargetStatusRecord {
                state: "unexpected".to_string(),
                checked_at: Some("2024-01-01T00:00:00Z".to_string()),
                ..TargetStatusRecord::default()
            },
        ] {
            assert_eq!(
                status_view_from_record(record, 1_704_067_200_000).state,
                "unknown"
            );
        }
    }

    #[test]
    fn probe_identity_changes_invalidate_in_flight_results() {
        let checked = TargetRecord {
            id: "target".to_string(),
            name: "Target".to_string(),
            mac: "02:11:22:33:44:55".to_string(),
            note: String::new(),
            relay_id: None,
            broadcast_address: None,
            ip_address: Some("192.0.2.10".to_string()),
            enabled: true,
            created_at: String::new(),
            updated_at: String::new(),
        };
        assert!(same_probe_identity(&checked, &checked));
        assert!(!same_probe_identity(
            &TargetRecord {
                ip_address: Some("192.0.2.11".to_string()),
                ..checked.clone()
            },
            &checked
        ));
        assert!(!same_probe_identity(
            &TargetRecord {
                enabled: false,
                ..checked.clone()
            },
            &checked
        ));
    }
}
