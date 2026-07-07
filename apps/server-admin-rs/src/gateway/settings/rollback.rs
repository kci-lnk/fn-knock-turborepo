use super::*;

pub(super) async fn rollback_gateway_visibility(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway visibility config");
        return Some(translator.t("server.admin.rollback.restoreVisibilityConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway visibility runtime");
        return Some(translator.t("server.admin.rollback.restoreVisibilityRuntimeFailed"));
    }
    if let Err(error) = sync_gateway_visibility_runtime(state, previous_runtime).await {
        tracing::warn!(%error, "failed to rollback gateway visibility runtime on go gateway");
        return Some(translator.t("server.admin.rollback.restoreGatewayVisibilityFailed"));
    }
    None
}

pub(super) async fn rollback_gateway_proxy_headers(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway proxy headers config");
        return Some(translator.t("server.admin.rollback.restoreProxyHeadersConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway proxy headers runtime");
        return Some(translator.t("server.admin.rollback.restoreProxyHeadersRuntimeFailed"));
    }
    if let Err(error) = sync_gateway_proxy_headers_runtime(state, previous_runtime).await {
        tracing::warn!(%error, "failed to rollback gateway proxy headers runtime on go gateway");
        return Some(translator.t("server.admin.rollback.restoreGatewayProxyHeadersRuntimeFailed"));
    }
    None
}

pub(super) async fn rollback_gateway_host_response(
    state: &AppState,
    previous_config: &Value,
    previous_runtime: &Value,
    translator: &Translator,
) -> Option<String> {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway host response config");
        return Some(translator.t("server.gatewayHostResponse.restoreConfigFailed"));
    }
    if let Err(error) = state
        .redis
        .set_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY, previous_runtime)
        .await
    {
        tracing::warn!(%error, "failed to rollback gateway host response runtime");
        return Some(translator.t("server.gatewayHostResponse.restoreRuntimeFailed"));
    }
    if let Err(error) =
        sync_gateway_host_response_runtime(state, previous_config, previous_runtime).await
    {
        tracing::warn!(%error, "failed to rollback gateway host response runtime on go gateway");
        return Some(translator.t("server.gatewayHostResponse.restoreGatewayRuntimeFailed"));
    }
    None
}

pub(super) fn rollback_message(
    translator: &Translator,
    message: &str,
    rollback_error: Option<&str>,
    rolled_back_key: &str,
) -> String {
    if let Some(rollback_error) = rollback_error {
        return translator.t_params(
            "server.admin.rollback.failed",
            &[
                (
                    "message",
                    non_empty_message(message, rolled_back_key, translator),
                ),
                ("rollbackError", rollback_error.to_string()),
            ],
        );
    }
    if message.trim().is_empty() {
        translator.t(rolled_back_key)
    } else {
        localize_gateway_route_message(translator, message)
    }
}

pub(super) fn non_empty_message(
    message: &str,
    fallback_key: &str,
    translator: &Translator,
) -> String {
    if message.trim().is_empty() {
        translator.t(fallback_key)
    } else {
        localize_gateway_route_message(translator, message)
    }
}

pub(super) async fn rollback_gateway_settings(state: &AppState, previous_config: &Value) {
    if let Err(error) = state.redis.save_config(previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway settings config");
        return;
    }
    if let Err(error) = sync_gateway_runtime(state, previous_config).await {
        tracing::warn!(%error, "failed to rollback gateway settings runtime");
    }
}
