use std::{collections::BTreeSet, net::IpAddr};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ipnet::IpNet;
use serde_json::{Map, Value, json};
use url::Url;

use crate::{
    cidr::{CidrOperator, CidrRegionQuery, CompiledIpSet, compile_ip_set},
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
pub(crate) const DEFAULT_GATEWAY_GC_PERCENT: i32 = 100;
pub(crate) const MIN_GATEWAY_GC_PERCENT: i32 = 25;
pub(crate) const MAX_GATEWAY_GC_PERCENT: i32 = 500;

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

pub fn gateway_settings_routes() -> utoipa_axum::router::OpenApiRouter<AppState> {
    handlers::routes()
}

pub(crate) async fn sync_gateway_settings_on_boot(state: AppState) {
    let config = match state.storage.store.get_config().await {
        Ok(config) => config,
        Err(error) => {
            tracing::warn!(%error, "failed to load config for gateway settings boot sync");
            return;
        }
    };

    if let Err(error) = sync_gateway_runtime(&state, &config).await {
        tracing::warn!(%error, "failed to sync gateway base runtime on boot");
    }

    let _memory_update_guard = state.gateway.memory_update_lock.lock().await;
    match state.storage.store.get_config().await {
        Ok(current_config) => {
            if let Err(error) = sync_gateway_memory_runtime(&state, &current_config).await {
                tracing::warn!(%error, "failed to sync gateway memory runtime on boot");
            }
        }
        Err(error) => {
            tracing::warn!(%error, "failed to refresh config for gateway memory boot sync");
        }
    }
    drop(_memory_update_guard);

    let visibility_runtime = match state
        .storage
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
        .storage
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
        .storage
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

pub(crate) fn gateway_memory_gc_percent(config: &Value) -> i32 {
    config
        .pointer("/gateway_memory/gc_percent")
        .and_then(Value::as_i64)
        .and_then(|value| i32::try_from(value).ok())
        .filter(|value| (MIN_GATEWAY_GC_PERCENT..=MAX_GATEWAY_GC_PERCENT).contains(value))
        .unwrap_or(DEFAULT_GATEWAY_GC_PERCENT)
}

pub(crate) async fn sync_gateway_memory_runtime(
    state: &AppState,
    config: &Value,
) -> anyhow::Result<i32> {
    let expected = gateway_memory_gc_percent(config);
    let applied = state
        .gateway
        .client
        .set_gateway_memory_config(expected)
        .await?;
    if applied != expected {
        anyhow::bail!(
            "Go gateway reported an unexpected GC percent: expected={expected}, applied={applied}"
        );
    }
    Ok(applied)
}

struct CompiledGatewayVisibility {
    config: Value,
    runtime: Value,
    policy: Option<crate::cidr::CompiledIpSet>,
}

pub(crate) struct CompiledHostVisibility {
    pub(crate) config: Value,
    pub(crate) policy: crate::cidr::CompiledIpSet,
}

struct CompiledGatewayTargetRuntime {
    config: Value,
    runtime: Value,
}

mod compile;
mod details;
mod handlers;
mod hosts;
mod migrate;
mod normalize;
mod patch;
mod rollback;
mod runtime;

pub(crate) use compile::compile_host_visibility_config;
use compile::*;
use details::*;
use hosts::*;
pub(crate) use migrate::{migrate_visibility_policies_locked, migrate_visibility_policies_on_boot};
use normalize::*;
use patch::*;
use rollback::*;
use runtime::{
    apply_gateway_portal_host_rules_patches_if_needed, sync_gateway_host_response_runtime,
    sync_gateway_proxy_headers_runtime, sync_gateway_visibility_runtime,
};
pub(crate) use runtime::{
    sync_gateway_runtime, sync_gateway_target_runtime_for_config,
    sync_gateway_visibility_runtime_from_store,
};

#[cfg(test)]
mod tests;
