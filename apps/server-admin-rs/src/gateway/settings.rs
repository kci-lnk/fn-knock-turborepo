use std::{collections::BTreeSet, net::IpAddr};

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
};
use ipnet::IpNet;
use serde_json::{Map, Value, json};
use url::Url;

use crate::{
    cidr::{CidrOperator, CidrRegionQuery},
    i18n::Translator,
    proxy_config::{self, build_gateway_auth_config},
    response,
    state::AppState,
    time_utils, whitelist,
};

const GATEWAY_VISIBILITY_RUNTIME_KEY: &str = "fn_knock:gateway:visibility:runtime";
const GATEWAY_PROXY_HEADERS_RUNTIME_KEY: &str = "fn_knock:gateway:proxy-headers:runtime";
const GATEWAY_HOST_RESPONSE_RUNTIME_KEY: &str = "fn_knock:gateway:host-response:runtime";
const GATEWAY_PORTAL_TITLE_HOST_RULES_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:gateway-portal-title-host-rules:v1";
const GATEWAY_PORTAL_ICON_HOST_RULES_PATCH_FLAG_KEY: &str =
    "fn_knock:patch:gateway-portal-icon-host-rules:v1";
const GO_BACKEND_UNSUCCESSFUL_RESPONSE: &str = "Go backend returned an unsuccessful response";

fn gateway_route_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.admin.gatewaySettingsRoutes.{key}"))
}

fn gateway_route_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.admin.gatewaySettingsRoutes.{key}"), params)
}

fn localize_gateway_route_message(translator: &Translator, message: &str) -> String {
    match message.trim() {
        "Gateway payload must be an object" | "Gateway visibility payload must be an object" => {
            gateway_route_text(translator, "payloadObjectRequired")
        }
        "Failed to load config" => gateway_route_text(translator, "loadConfigFailed"),
        "Failed to load runtime" => gateway_route_text(translator, "loadRuntimeFailed"),
        GO_BACKEND_UNSUCCESSFUL_RESPONSE => {
            translator.t("server.admin.runtimeConfigRoutes.upstreamUnavailable")
        }
        _ => crate::cidr::localize_error(translator, message),
    }
}

pub fn gateway_settings_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/config/gateway",
            get(get_gateway).post(update_gateway),
        )
        .route(
            "/api/admin/config/gateway/visibility",
            get(get_gateway_visibility).post(update_gateway_visibility),
        )
        .route(
            "/api/admin/config/gateway/proxy-headers",
            get(get_gateway_proxy_headers).post(update_gateway_proxy_headers),
        )
        .route(
            "/api/admin/config/gateway/host-response",
            get(get_gateway_host_response).post(update_gateway_host_response),
        )
}

pub(crate) async fn sync_gateway_settings_on_boot(state: AppState) {
    let config = match state.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for gateway settings boot sync");
            return;
        }
    };

    if let Err(error) = sync_gateway_runtime(&state, &config).await {
        tracing::warn!(%error, "failed to sync gateway base runtime on boot");
    }

    let visibility_runtime = match state
        .store
        .get_json_value(GATEWAY_VISIBILITY_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_visibility_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway visibility runtime for boot sync");
            default_gateway_visibility_runtime()
        }
    };
    if let Err(error) = sync_gateway_visibility_runtime(&state, &visibility_runtime).await {
        tracing::warn!(%error, "failed to sync gateway visibility runtime on boot");
    }

    let proxy_headers_runtime = match state
        .store
        .get_json_value(GATEWAY_PROXY_HEADERS_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_proxy_headers_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway proxy headers runtime for boot sync");
            default_gateway_proxy_headers_runtime()
        }
    };
    if let Err(error) = sync_gateway_proxy_headers_runtime(&state, &proxy_headers_runtime).await {
        tracing::warn!(%error, "failed to sync gateway proxy headers runtime on boot");
    }

    let host_response_runtime = match state
        .store
        .get_json_value(GATEWAY_HOST_RESPONSE_RUNTIME_KEY)
        .await
    {
        Ok(Some(value)) => value,
        Ok(None) => default_gateway_host_response_runtime(),
        Err(error) => {
            tracing::warn!(%error, "failed to load gateway host response runtime for boot sync");
            default_gateway_host_response_runtime()
        }
    };
    if let Err(error) =
        sync_gateway_host_response_runtime(&state, &config, &host_response_runtime).await
    {
        tracing::warn!(%error, "failed to sync gateway host response runtime on boot");
    }
}

struct CompiledGatewayVisibility {
    config: Value,
    runtime: Value,
}

struct CompiledGatewayTargetRuntime {
    config: Value,
    runtime: Value,
}

mod compile;
mod details;
mod handlers;
mod hosts;
mod normalize;
mod patch;
mod rollback;
mod runtime;

pub(crate) use compile::compile_host_visibility_config;
use compile::*;
use details::*;
use handlers::*;
use hosts::*;
use normalize::*;
use patch::*;
use rollback::*;
use runtime::{
    apply_gateway_portal_host_rules_patches_if_needed, sync_gateway_host_response_runtime,
    sync_gateway_proxy_headers_runtime, sync_gateway_runtime, sync_gateway_visibility_runtime,
};
pub(crate) use runtime::{
    sync_gateway_target_runtime_for_config, sync_gateway_visibility_runtime_from_store,
};

#[cfg(test)]
mod tests;
