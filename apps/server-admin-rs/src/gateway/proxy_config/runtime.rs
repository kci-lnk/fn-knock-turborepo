use super::*;

pub(super) async fn sync_go_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) async fn sync_go_host_rules(state: &AppState, rules: &Value) -> Result<(), String> {
    ensure_go_success(
        state
            .go_backend
            .set_host_rules(rules)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) async fn sync_stream_mappings_runtime(
    state: &AppState,
    config: &Value,
) -> Result<(), String> {
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    runtime_config::apply_run_type_config(state, config, run_type).await
}

pub(super) async fn sync_go_auth_config(state: &AppState, config: &Value) -> Result<(), String> {
    let auth_config = build_gateway_auth_config(config);
    ensure_go_success(
        state
            .go_backend
            .set_auth_config(&auth_config)
            .await
            .map_err(|error| error.to_string())?,
    )
}

pub(super) async fn sync_host_mappings_runtime(
    state: &AppState,
    config: &Value,
    mappings: &[Value],
) -> Result<(), String> {
    sync_go_host_rules(state, &build_host_rules_payload(mappings)).await?;
    sync_go_auth_config(state, config).await?;
    gateway_settings::sync_gateway_target_runtime_for_config(state, config, true).await
}
