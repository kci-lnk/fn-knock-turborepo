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
pub(crate) const MIN_GATEWAY_MEMORY_LIMIT_MIB: u64 = 64;
pub(crate) const MAX_GATEWAY_MEMORY_LIMIT_MIB: u64 = 4096;
const DEFAULT_AUTO_MEMORY_LIMIT_MIB: u64 = 256;
const MIN_AUTO_MEMORY_LIMIT_MIB: u64 = 128;
const MAX_AUTO_MEMORY_LIMIT_MIB: u64 = 512;
const MIB: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GatewayMemorySettings {
    pub(crate) gc_percent: i32,
    pub(crate) memory_limit_mib: Option<u64>,
}

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

    if let Err(error) = sync_gateway_memory_on_boot(&state).await {
        tracing::warn!(%error, "failed to refresh gateway memory runtime during boot sync");
    }

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

pub(crate) fn gateway_memory_settings(config: &Value) -> GatewayMemorySettings {
    GatewayMemorySettings {
        gc_percent: gateway_memory_gc_percent(config),
        memory_limit_mib: config
            .pointer("/gateway_memory/memory_limit_mib")
            .and_then(Value::as_u64)
            .filter(|value| {
                (MIN_GATEWAY_MEMORY_LIMIT_MIB..=MAX_GATEWAY_MEMORY_LIMIT_MIB).contains(value)
            }),
    }
}

pub(crate) fn effective_host_memory_bytes() -> Option<u64> {
    crate::infra::system_resources::effective_memory_bytes().0
}

pub(crate) fn resolve_gateway_memory_limit_bytes(settings: GatewayMemorySettings) -> u64 {
    settings.memory_limit_mib.map_or_else(
        || auto_gateway_memory_limit_mib(effective_host_memory_bytes()),
        |value| value.clamp(MIN_GATEWAY_MEMORY_LIMIT_MIB, MAX_GATEWAY_MEMORY_LIMIT_MIB),
    ) * MIB
}

fn auto_gateway_memory_limit_mib(effective_memory_bytes: Option<u64>) -> u64 {
    effective_memory_bytes.map_or(DEFAULT_AUTO_MEMORY_LIMIT_MIB, |bytes| {
        (bytes / MIB / 4).clamp(MIN_AUTO_MEMORY_LIMIT_MIB, MAX_AUTO_MEMORY_LIMIT_MIB)
    })
}

pub(crate) fn validate_gateway_memory_settings(
    settings: GatewayMemorySettings,
) -> anyhow::Result<()> {
    if !(MIN_GATEWAY_GC_PERCENT..=MAX_GATEWAY_GC_PERCENT).contains(&settings.gc_percent) {
        anyhow::bail!(
            "GC percent must be between {MIN_GATEWAY_GC_PERCENT} and {MAX_GATEWAY_GC_PERCENT}"
        );
    }
    if let Some(memory_limit_mib) = settings.memory_limit_mib {
        if !(MIN_GATEWAY_MEMORY_LIMIT_MIB..=MAX_GATEWAY_MEMORY_LIMIT_MIB)
            .contains(&memory_limit_mib)
        {
            anyhow::bail!(
                "Memory limit must be between {MIN_GATEWAY_MEMORY_LIMIT_MIB} and {MAX_GATEWAY_MEMORY_LIMIT_MIB} MiB"
            );
        }
        if let Some(host_memory_bytes) = effective_host_memory_bytes()
            && memory_limit_mib.saturating_mul(MIB) > host_memory_bytes / 2
        {
            anyhow::bail!("Memory limit must not exceed 50% of effective system memory");
        }
    }
    Ok(())
}

pub(crate) async fn sync_gateway_memory_runtime(
    state: &AppState,
    config: &Value,
) -> anyhow::Result<GatewayMemorySettings> {
    let expected = gateway_memory_settings(config);
    apply_gateway_memory_settings(state, expected).await
}

pub(crate) async fn apply_gateway_memory_settings(
    state: &AppState,
    expected: GatewayMemorySettings,
) -> anyhow::Result<GatewayMemorySettings> {
    apply_gateway_memory_settings_with_client(&state.gateway.client, expected).await
}

async fn apply_gateway_memory_settings_with_client(
    client: &crate::go_backend::GoBackendClient,
    expected: GatewayMemorySettings,
) -> anyhow::Result<GatewayMemorySettings> {
    validate_gateway_memory_settings(expected)?;
    let memory_limit_bytes = resolve_gateway_memory_limit_bytes(expected);
    let (applied_gc_percent, applied_memory_limit_bytes) = client
        .set_gateway_memory_config(expected.gc_percent, i64::try_from(memory_limit_bytes)?)
        .await?;
    if applied_gc_percent != expected.gc_percent
        || u64::try_from(applied_memory_limit_bytes).ok() != Some(memory_limit_bytes)
    {
        anyhow::bail!(
            "Go gateway reported unexpected memory settings: expected_gc={}, applied_gc={}, expected_limit={}, applied_limit={}",
            expected.gc_percent,
            applied_gc_percent,
            memory_limit_bytes,
            applied_memory_limit_bytes,
        );
    }
    Ok(expected)
}

pub(crate) async fn sync_gateway_memory_on_boot(
    state: &AppState,
) -> anyhow::Result<GatewayMemorySettings> {
    let _memory_update_guard = state.gateway.memory_update_lock.lock().await;
    let config = state.storage.store.get_config().await?;
    sync_gateway_memory_runtime(state, &config).await
}

pub(crate) async fn sync_gateway_memory_on_boot_with_timeout(
    state: &AppState,
    timeout: std::time::Duration,
) -> anyhow::Result<GatewayMemorySettings> {
    let _memory_update_guard = state.gateway.memory_update_lock.lock().await;
    let config = state.storage.store.get_config().await?;
    let expected = gateway_memory_settings(&config);
    let client = state.gateway.client.with_timeout(timeout)?;
    apply_gateway_memory_settings_with_client(&client, expected).await
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
mod proxy_protocol;
mod rollback;
mod runtime;

pub(crate) use compile::compile_host_visibility_config;
use compile::*;
use details::*;
use hosts::*;
pub(crate) use migrate::{migrate_visibility_policies_locked, migrate_visibility_policies_on_boot};
use normalize::*;
use patch::*;
use proxy_protocol::*;
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
