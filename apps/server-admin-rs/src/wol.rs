mod discovery;
mod dispatch;
mod integrations;
mod probe;
mod relay;
mod routes;
mod secrets;
pub(crate) mod service;
mod ssh;
mod status;
mod store;

pub(crate) use routes::{
    shutdown_target_for_portal, wol_discovery_openapi_routes, wol_local_relay_openapi_routes,
    wol_relay_openapi_routes, wol_routes, wol_target_openapi_routes,
};
pub(crate) use status::start_wol_tasks;

pub(crate) fn feature_enabled(config: &serde_json::Value) -> bool {
    crate::runtime_config::normalize_wol_feature(config.get("wol_feature"))
        .get("enabled")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

pub(crate) async fn feature_enabled_for_state(
    state: &crate::state::AppState,
) -> crate::storage::StorageResult<bool> {
    Ok(feature_enabled(&state.storage.store.get_config().await?))
}

pub(crate) async fn clear_secrets_after_backup_restore(
    state: &crate::state::AppState,
) -> Result<(), String> {
    let _guard = state.wol.config_lock.lock().await;
    secrets::secret_store(state).clear_all()?;
    state.wol.integration_status.write().await.clear();
    state.wol.relay_reload.notify_one();
    notify_runtime_reload(state);
    Ok(())
}

pub(crate) fn notify_runtime_reload(state: &crate::state::AppState) {
    state
        .wol
        .runtime_reload
        .send_modify(|generation| *generation = generation.wrapping_add(1));
}
