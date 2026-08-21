use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    net::Ipv4Addr,
    str::FromStr,
    time::Duration,
};

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use ipnet::Ipv4Net;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::{task::JoinSet, time};
use utoipa_axum::{router::OpenApiRouter, routes};

use crate::{response, state::AppState, time_utils};

use super::{
    cloudflare_api::{CloudflareApi, CloudflareApiError},
    managed::{
        ManagedDnsRequest, acquire_http_manage_lock, api_for_background, configured_hosts,
        dns_record_owned_for_update, load_managed_config, load_managed_state, managed_instance_id,
        managed_root_domain, save_managed_config, save_managed_state, upsert_managed_dns,
    },
};

mod api;
mod cleanup_snapshot;
mod coordination;
mod fallback_origin;
mod model;
mod preview;
mod probes;
mod public_state;
mod recovery;
mod resolvers;
mod resource_cleanup;
mod resource_reconcile;
mod runtime;
mod scan;
mod scheduler;
mod settings;
mod state_helpers;
mod warnings;

pub(super) use api::openapi_routes;
pub(super) use cleanup_snapshot::append_cleanup_remote_snapshot;
pub(super) use coordination::{
    configured_optimization_hosts, schedule_after_host_mappings_change, start_tasks,
};
use fallback_origin::*;
use model::*;
pub(super) use preview::append_preview;
use probes::*;
pub(super) use public_state::public_state;
use recovery::*;
use resolvers::*;
pub(super) use resource_cleanup::{cleanup_resources, fallback_to_wildcard};
use resource_cleanup::{
    forget_optimization_host_state, host_has_tracked_remote_resources,
    reconcile_optimization_host_membership, relinquish_optimization_host,
    tracked_exact_dns_snapshot,
};
pub(super) use resource_reconcile::reconcile_resources;
use resource_reconcile::{active_probe_hostname, record_preferred_edge_probe_failure};
#[cfg(test)]
use resource_reconcile::{
    active_probe_hostnames, custom_hostname_needs_activation_dns,
    custom_hostname_ownership_conflict, set_exact_dns_route, update_custom_hostname_activation,
};
pub(super) use runtime::is_capability_unsupported_api_error;
use runtime::{
    api_error_response, delete_dns_if_owned, ignore_not_found, is_job_cancelled, load_runtime,
    local_error, local_error_display, optimization_is_enabled, optimization_scan_error_code,
    save_runtime, update_job, weekly_jitter_ms,
};
use scan::*;
#[cfg(test)]
use scheduler::apply_automatic_scan_result;
use scheduler::scheduled_tick;
use state_helpers::*;
pub(super) use warnings::{plan_warning_codes, plan_warnings};

#[cfg(test)]
use settings::normalize_domain_settings;
use settings::{
    default_builtin_source_ids, default_true, load_domain_settings, load_source_settings,
    normalize_candidate_hostname, normalize_source_settings, partition_optimization_hosts,
    public_source_settings, source_settings_fingerprint,
};

#[cfg(test)]
mod tests {
    use super::*;

    mod cancellation;
    mod candidates;
    mod ownership;
    mod reconcile;
    mod runtime;
}
