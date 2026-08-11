use serde_json::Value;

use crate::{
    proxy_config::{self, build_gateway_auth_config},
    state::AppState,
};

pub(super) async fn refresh_gateway_auth_runtime(state: &AppState) -> anyhow::Result<()> {
    proxy_config::with_host_mappings_runtime_transaction(state, |state| async move {
        let config = state
            .storage
            .store
            .get_config()
            .await
            .map_err(|error| error.to_string())?;
        let auth_config = build_gateway_auth_config(&config);
        ensure_go_success(
            state
                .gateway
                .client
                .set_auth_config(&auth_config)
                .await
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())
    })
    .await
    .map_err(anyhow::Error::msg)
}

pub(super) fn ensure_go_success(value: Value) -> anyhow::Result<()> {
    crate::go_backend::ensure_response_success(
        &value,
        "Go backend returned an unsuccessful response",
    )
    .map_err(anyhow::Error::msg)
}
