//! SSH-backed Web Terminal.
//!
//! Persistent target metadata and installation-bound credentials live outside
//! the runtime. Shells, attachments and their output buffers intentionally
//! exist only for the lifetime of this process.

mod backup;
pub(crate) mod domain;
mod http;
mod legacy_cleanup;
mod repository;
mod runtime;
mod secrets;
mod service;
mod ssh;

use std::time::Duration;

use crate::state::AppState;
use axum::Router;

pub(crate) use backup::{
    TerminalCredentialBackup, export_credentials_for_backup, restore_credentials_after_backup,
};
#[cfg(test)]
pub(crate) use backup::{read_backup_test_credential, write_backup_test_credential};
pub use http::routes as terminal_runtime_routes;
pub use runtime::TerminalRuntime;

pub fn terminal_routes() -> Router<AppState> {
    let routes: Router<AppState> = terminal_runtime_routes().into();
    routes
}

/// Starts the only terminal background worker. It expires abandoned browser
/// attachments, but never expires a live SSH shell merely because no browser
/// is attached.
pub fn start_terminal_tasks(state: AppState) {
    let task_state = state.clone();
    state.spawn_background("terminal-runtime-maintenance", async move {
        if let Err(error) = legacy_cleanup::cleanup(&task_state).await {
            tracing::warn!(%error, "failed to clean legacy local terminal runtime");
        }
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = task_state.shutdown.cancelled() => {
                    task_state.terminal.shutdown_all().await;
                    break;
                }
                _ = interval.tick() => task_state.terminal.expire_attachments().await,
            }
        }
    });
}

pub fn clear_all_credentials(state: &AppState) -> Result<(), String> {
    secrets::TerminalSecretStore::from_state(state).clear_all()
}

#[cfg(test)]
mod tests;
