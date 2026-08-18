pub mod adapters;
mod credentials;
pub mod http;
pub mod model;
pub mod ownership;
pub mod projection;
pub mod repository;
pub mod runtime;
pub mod scheduler;
pub mod service;

pub use http::routes as panel_sync_routes;
pub use runtime::PanelSyncRuntime;

pub fn start_panel_sync_tasks(state: crate::state::AppState) {
    scheduler::start(state);
}

pub fn notify_source_changed(state: &crate::state::AppState) {
    state.panel_sync.source_changed.notify_one();
}

pub async fn clear_credentials_after_backup_restore(
    state: &crate::state::AppState,
) -> Result<(), String> {
    service::clear_credentials_after_backup_restore(state).await
}

pub fn clear_all_credentials(state: &crate::state::AppState) -> Result<(), String> {
    service::clear_all_credentials(state)
}

#[cfg(test)]
mod tests;
