mod discovery;
mod dispatch;
mod integrations;
mod probe;
mod relay;
mod routes;
mod secrets;
pub(crate) mod service;
mod status;
mod store;

pub(crate) use routes::wol_routes;
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
    Ok(feature_enabled(&state.store.get_config().await?))
}

pub(crate) async fn clear_secrets_after_backup_restore(
    state: &crate::state::AppState,
) -> Result<(), String> {
    let _guard = state.wol_config_lock.lock().await;
    secrets::secret_store(state).clear_all()?;
    state.wol_integration_status.write().await.clear();
    state.wol_relay_reload.notify_one();
    notify_runtime_reload(state);
    Ok(())
}

pub(crate) fn notify_runtime_reload(state: &crate::state::AppState) {
    state
        .wol_runtime_reload
        .send_modify(|generation| *generation = generation.wrapping_add(1));
}
