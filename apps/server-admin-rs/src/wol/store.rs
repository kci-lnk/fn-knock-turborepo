use serde::{Deserialize, Serialize};

use crate::{state::AppState, time_utils};

const RELAY_INDEX_KEY: &str = "fn_knock:wol:relays:index";
const RELAY_PREFIX: &str = "fn_knock:wol:relay:";
const TARGET_INDEX_KEY: &str = "fn_knock:wol:targets:index";
const TARGET_PREFIX: &str = "fn_knock:wol:target:";
const TARGET_STATUS_PREFIX: &str = "fn_knock:wol:target-status:";
const LOCAL_RELAY_CONFIG_KEY: &str = "fn_knock:wol:local-relay";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct LocalRelayConfig {
    pub enabled: bool,
    pub relay_id: String,
    pub key_version: u32,
    pub listen_address: String,
    pub port: u16,
    pub broadcast_destinations: Vec<String>,
    pub allowed_sources: Vec<String>,
    pub updated_at: String,
}

impl Default for LocalRelayConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            relay_id: String::new(),
            key_version: 1,
            listen_address: "0.0.0.0".to_string(),
            port: 40009,
            broadcast_destinations: vec!["255.255.255.255:9".to_string()],
            allowed_sources: Vec::new(),
            updated_at: String::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct RelayRecord {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub enabled: bool,
    pub key_version: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TargetRecord {
    pub id: String,
    pub name: String,
    pub mac: String,
    /// `None` means that this server-admin-rs instance broadcasts the Magic
    /// Packet directly. A Relay is only selected for a different network.
    #[serde(default)]
    pub relay_id: Option<String>,
    /// Directed IPv4 broadcast learned during LAN discovery. Older/manual
    /// records may omit it and fall back to every local interface broadcast.
    #[serde(default)]
    pub broadcast_address: Option<String>,
    /// Last configured IPv4 address. Runtime checks may observe a newer DHCP address.
    #[serde(default)]
    pub ip_address: Option<String>,
    /// Non-sensitive third-party integration settings. Credentials are kept
    /// in the installation-bound encrypted WoL secret store.
    #[serde(default)]
    pub integrations: TargetIntegrations,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct TargetIntegrations {
    #[serde(default)]
    pub blinker: BlinkerIntegrationConfig,
    #[serde(default)]
    pub bemfa: BemfaIntegrationConfig,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct BlinkerIntegrationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bind_component: bool,
    #[serde(default = "default_skip_tls_verify")]
    pub skip_tls_verify: bool,
}

impl Default for BlinkerIntegrationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_component: false,
            skip_tls_verify: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct BemfaIntegrationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub topic: String,
    #[serde(default = "default_skip_tls_verify")]
    pub skip_tls_verify: bool,
}

impl Default for BemfaIntegrationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            topic: String::new(),
            skip_tls_verify: true,
        }
    }
}

fn default_skip_tls_verify() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct TargetStatusRecord {
    pub state: String,
    #[serde(default)]
    pub checked_at: Option<String>,
    #[serde(default)]
    pub last_online_at: Option<String>,
    #[serde(default)]
    pub observed_ip: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
}

impl Default for TargetStatusRecord {
    fn default() -> Self {
        Self {
            state: "unknown".to_string(),
            checked_at: None,
            last_online_at: None,
            observed_ip: None,
            last_error: None,
        }
    }
}

pub(super) async fn list_relays(state: &AppState) -> anyhow::Result<Vec<RelayRecord>> {
    load_indexed_records(state, RELAY_INDEX_KEY, relay_key).await
}

pub(super) async fn load_relay(state: &AppState, id: &str) -> anyhow::Result<Option<RelayRecord>> {
    load_record(state, &relay_key(id)).await
}

pub(super) async fn save_relay(state: &AppState, relay: &RelayRecord) -> anyhow::Result<()> {
    save_record(
        state,
        &relay_key(&relay.id),
        RELAY_INDEX_KEY,
        &relay.id,
        relay,
    )
    .await
}

pub(super) async fn delete_relay(state: &AppState, id: &str) -> anyhow::Result<()> {
    state
        .store
        .delete_string_and_zrem(&relay_key(id), RELAY_INDEX_KEY, id)
        .await?;
    Ok(())
}

pub(super) async fn list_targets(state: &AppState) -> anyhow::Result<Vec<TargetRecord>> {
    load_indexed_records(state, TARGET_INDEX_KEY, target_key).await
}

pub(super) async fn load_target(
    state: &AppState,
    id: &str,
) -> anyhow::Result<Option<TargetRecord>> {
    load_record(state, &target_key(id)).await
}

pub(super) async fn save_target(state: &AppState, target: &TargetRecord) -> anyhow::Result<()> {
    save_record(
        state,
        &target_key(&target.id),
        TARGET_INDEX_KEY,
        &target.id,
        target,
    )
    .await
}

pub(super) async fn delete_target(state: &AppState, id: &str) -> anyhow::Result<()> {
    state
        .store
        .delete_string_and_zrem(&target_key(id), TARGET_INDEX_KEY, id)
        .await?;
    Ok(())
}

pub(super) async fn load_target_status(
    state: &AppState,
    id: &str,
) -> anyhow::Result<TargetStatusRecord> {
    load_record(state, &target_status_key(id))
        .await
        .map(|value| value.unwrap_or_default())
}

pub(super) async fn save_target_status(
    state: &AppState,
    id: &str,
    status: &TargetStatusRecord,
) -> anyhow::Result<()> {
    state
        .store
        .set_json_value(&target_status_key(id), &serde_json::to_value(status)?)
        .await?;
    Ok(())
}

pub(super) async fn delete_target_status(state: &AppState, id: &str) -> anyhow::Result<()> {
    state.store.delete_key(&target_status_key(id)).await?;
    Ok(())
}

pub(super) async fn load_local_relay_config(state: &AppState) -> anyhow::Result<LocalRelayConfig> {
    state
        .store
        .get_json_value(LOCAL_RELAY_CONFIG_KEY)
        .await?
        .map(serde_json::from_value)
        .transpose()
        .map(|value| value.unwrap_or_default())
        .map_err(Into::into)
}

pub(super) async fn save_local_relay_config(
    state: &AppState,
    config: &LocalRelayConfig,
) -> anyhow::Result<()> {
    state
        .store
        .set_json_value(LOCAL_RELAY_CONFIG_KEY, &serde_json::to_value(config)?)
        .await?;
    Ok(())
}

async fn load_indexed_records<T, F>(
    state: &AppState,
    index_key: &str,
    key: F,
) -> anyhow::Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
    F: Fn(&str) -> String,
{
    let ids = state.store.zrevrange_strings(index_key).await?;
    let keys = ids.iter().map(|id| key(id)).collect::<Vec<_>>();
    let values = state.store.mget_string_values(&keys).await?;
    let mut records = Vec::with_capacity(values.len());
    for value in values.into_iter().flatten() {
        match serde_json::from_str(&value) {
            Ok(record) => records.push(record),
            Err(error) => tracing::warn!(%error, "ignored invalid WoL record"),
        }
    }
    Ok(records)
}

async fn load_record<T>(state: &AppState, key: &str) -> anyhow::Result<Option<T>>
where
    T: for<'de> Deserialize<'de>,
{
    let value = state.store.get_json_value(key).await?;
    value
        .map(serde_json::from_value::<T>)
        .transpose()
        .map_err(Into::into)
}

async fn save_record<T: Serialize>(
    state: &AppState,
    data_key: &str,
    index_key: &str,
    id: &str,
    value: &T,
) -> anyhow::Result<()> {
    let serialized = serde_json::to_string(value)?;
    state
        .store
        .set_string_and_zadd(data_key, &serialized, index_key, id, time_utils::now_ms())
        .await?;
    Ok(())
}

fn relay_key(id: &str) -> String {
    format!("{RELAY_PREFIX}{id}")
}

fn target_key(id: &str) -> String {
    format!("{TARGET_PREFIX}{id}")
}

fn target_status_key(id: &str) -> String {
    format!("{TARGET_STATUS_PREFIX}{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_keep_records_inside_backup_prefix() {
        assert_eq!(relay_key("abc"), "fn_knock:wol:relay:abc");
        assert_eq!(target_key("abc"), "fn_knock:wol:target:abc");
        assert_eq!(target_status_key("abc"), "fn_knock:wol:target-status:abc");
        assert_eq!(LOCAL_RELAY_CONFIG_KEY, "fn_knock:wol:local-relay");
    }

    #[test]
    fn target_status_persistence_uses_public_field_names() {
        let value = serde_json::to_value(TargetStatusRecord {
            state: "online".to_string(),
            checked_at: Some("2026-08-08T00:00:00Z".to_string()),
            last_online_at: Some("2026-08-08T00:00:00Z".to_string()),
            observed_ip: Some("192.0.2.10".to_string()),
            last_error: None,
        })
        .unwrap();
        assert!(value.get("checkedAt").is_some());
        assert!(value.get("lastOnlineAt").is_some());
        assert!(value.get("observedIp").is_some());
        assert!(value.get("lastError").is_some());
        assert!(value.get("checked_at").is_none());
    }

    #[test]
    fn legacy_target_records_default_integrations_to_disabled() {
        let target: TargetRecord = serde_json::from_value(serde_json::json!({
            "id": "target",
            "name": "Desktop",
            "mac": "02:11:22:33:44:55",
            "enabled": true,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        }))
        .unwrap();
        assert_eq!(target.integrations, TargetIntegrations::default());
        assert!(!target.integrations.blinker.enabled);
        assert!(!target.integrations.bemfa.enabled);
        assert!(target.integrations.blinker.skip_tls_verify);
        assert!(target.integrations.bemfa.skip_tls_verify);
    }
}
