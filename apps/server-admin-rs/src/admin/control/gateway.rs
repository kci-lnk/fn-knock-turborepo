use serde_json::Value;

use crate::{proxy_config::build_gateway_auth_config, state::AppState};

pub(super) async fn refresh_gateway_auth_runtime(state: &AppState) -> anyhow::Result<()> {
    let config = state.store.get_config().await?;
    let auth_config = build_gateway_auth_config(&config);
    ensure_go_success(state.go_backend.set_auth_config(&auth_config).await?)
}

pub(super) fn ensure_go_success(value: Value) -> anyhow::Result<()> {
    if crate::go_backend::response_success(&value) {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        crate::go_backend::response_message(&value, "Go backend returned an unsuccessful response",)
    )
}
