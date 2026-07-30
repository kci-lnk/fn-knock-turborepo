use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    net::IpAddr,
    str::FromStr,
};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post},
};
use ipnet::IpNet;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::{
    net::lookup_host,
    time::{self, MissedTickBehavior},
};
use uuid::Uuid;

use crate::{
    auth_mobility,
    cidr::{CidrOperator, CidrRegionQuery, CompiledIpSet, compile_ip_set, union_ip_sets},
    http_utils::normalize_ip,
    i18n::Translator,
    ip_location, response, runtime_profile,
    state::AppState,
    store::{
        LoginSession, WhitelistConcreteTarget, WhitelistRecord, WhitelistRegionGroupRecord,
        WhitelistRegionInput,
    },
    time_utils,
};

const DEFAULT_CNAME_CHECK_INTERVAL_MINUTES: i64 = 5;
const MIN_CNAME_CHECK_INTERVAL_MINUTES: i64 = 1;
const MAX_CNAME_CHECK_INTERVAL_MINUTES: i64 = 24 * 60;
const AUTO_WHITELIST_OWNER_LOCK_TTL_SECONDS: usize = 60;
const AUTO_WHITELIST_OWNER_LOCK_WAIT_SECONDS: u64 = 10;
const WHITELIST_ALL_IPSET_KEY: &str = "whitelist_all";
const WHITELIST_MANUAL_IPSET_KEY: &str = "whitelist_manual";
const WHITELIST_AUTO_IPSET_KEY: &str = "whitelist_auto";

pub(crate) struct DeferredSessionAutoWhitelist {
    pub(crate) record: WhitelistRecord,
    previous_targets: Vec<WhitelistConcreteTarget>,
}

fn whitelist_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.whitelist.{key}"))
}

fn whitelist_manager_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.whitelistManager.{key}"))
}

fn whitelist_manager_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.whitelistManager.{key}"), params)
}

fn localize_whitelist_error(translator: &Translator, message: &str) -> String {
    match message {
        "Invalid whitelist target format" => {
            whitelist_manager_text(translator, "targetFormatInvalid")
        }
        "Automatic whitelist grants only support IP targets" => {
            whitelist_manager_text(translator, "autoGrantIpOnly")
        }
        "Invalid whitelist CIDR" => whitelist_manager_text(translator, "cidrInvalid"),
        "Invalid whitelist domain" => whitelist_manager_text(translator, "domainInvalid"),
        "Invalid whitelist IP" | "Invalid whitelist target" => {
            whitelist_manager_text(translator, "ipInvalid")
        }
        "Auto whitelist owner is missing" => whitelist_manager_text(translator, "autoOwnerMissing"),
        _ => message.to_string(),
    }
}

fn normalize_auto_whitelist_comment(
    value: Option<&str>,
    translator: &Translator,
) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if is_auto_ip_grant_comment(trimmed) {
        Some(translator.t("auth.autoIpGrantComment"))
    } else {
        Some(trimmed.to_string())
    }
}

fn is_auto_ip_grant_comment(value: &str) -> bool {
    matches!(
        value.trim(),
        "登录后自动授权"
            | "登入後自動授權"
            | "Automatically authorized after sign-in"
            | "로그인 후 자동 승인됨"
            | "ログイン後自動認証"
            | "server.auth.autoIpGrantComment"
    )
}

fn whitelist_record_for_response(
    mut record: WhitelistRecord,
    translator: &Translator,
) -> WhitelistRecord {
    record.comment = normalize_auto_whitelist_comment(record.comment.as_deref(), translator);
    record
}

#[derive(Deserialize)]
struct AddWhitelistBody {
    ip: String,
    #[serde(rename = "targetType")]
    target_type: Option<String>,
    #[serde(rename = "expireAt")]
    expire_at: Option<i64>,
    source: Option<String>,
    comment: Option<String>,
    #[serde(rename = "checkIntervalMinutes")]
    check_interval_minutes: Option<i64>,
}

#[derive(Deserialize)]
struct CommentBody {
    comment: String,
}

#[derive(Deserialize)]
struct AddWhitelistRegionsBody {
    #[serde(default)]
    regions: Vec<Value>,
    #[serde(rename = "expireAt")]
    expire_at: Option<i64>,
    comment: Option<String>,
}

pub fn whitelist_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/admin/whitelist",
            get(list_whitelist).post(add_whitelist),
        )
        .route(
            "/api/admin/whitelist/regions",
            get(list_whitelist_regions).post(add_whitelist_regions),
        )
        .route(
            "/api/admin/whitelist/regions/{id}",
            delete(delete_whitelist_region),
        )
        .route("/api/admin/whitelist/{id}", delete(delete_whitelist))
        .route(
            "/api/admin/whitelist/{id}/comment",
            patch(update_whitelist_comment),
        )
        .route(
            "/api/admin/whitelist/{id}/refresh",
            post(refresh_whitelist_cname),
        )
}

pub fn start_whitelist_tasks(state: AppState) {
    let maintenance_state = state.clone();
    tokio::spawn(async move {
        let mut ticker = time::interval(std::time::Duration::from_secs(60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = maintenance_state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = maintenance_state.shutdown.cancelled() => break,
                result = run_whitelist_maintenance_once(&maintenance_state) => {
                    if let Err(error) = result {
                        tracing::warn!(%error, "whitelist maintenance task failed");
                    }
                }
            }
        }
    });
    tokio::spawn(async move {
        let mut ticker = time::interval(std::time::Duration::from_secs(2 * 60));
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await;
        loop {
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                _ = state.shutdown.cancelled() => break,
                _ = sync_reverse_proxy_trusted_ips(&state) => {}
            }
        }
    });
}

pub async fn ensure_session_auto_whitelist(
    state: &AppState,
    owner_key: &str,
    ip: &str,
    expire_at: Option<i64>,
    comment: Option<String>,
    existing_record_id: Option<&str>,
) -> anyhow::Result<WhitelistRecord> {
    let deferred = ensure_session_auto_whitelist_deferred(
        state,
        owner_key,
        ip,
        expire_at,
        comment,
        existing_record_id,
        None,
        "active",
    )
    .await?;
    publish_deferred_session_auto_whitelist(state, deferred).await
}

pub(crate) async fn ensure_pending_session_auto_whitelist(
    state: &AppState,
    owner_key: &str,
    ip: &str,
    expire_at: Option<i64>,
    comment: Option<String>,
    existing_record_id: Option<&str>,
    candidate_record_id: &str,
) -> anyhow::Result<DeferredSessionAutoWhitelist> {
    ensure_session_auto_whitelist_deferred(
        state,
        owner_key,
        ip,
        expire_at,
        comment,
        existing_record_id,
        Some(candidate_record_id),
        "pending",
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn ensure_session_auto_whitelist_deferred(
    state: &AppState,
    owner_key: &str,
    ip: &str,
    expire_at: Option<i64>,
    comment: Option<String>,
    existing_record_id: Option<&str>,
    candidate_record_id: Option<&str>,
    new_record_status: &str,
) -> anyhow::Result<DeferredSessionAutoWhitelist> {
    let owner_key = owner_key.trim();
    if owner_key.is_empty() {
        anyhow::bail!("Auto whitelist owner is missing");
    }
    let owner_record_key = whitelist_auto_owner_record_key(owner_key);
    let lock_key = format!("{owner_record_key}:lock");
    let lock_id = Uuid::new_v4().to_string();
    let deadline = time::Instant::now()
        + std::time::Duration::from_secs(AUTO_WHITELIST_OWNER_LOCK_WAIT_SECONDS);
    loop {
        if state
            .store
            .set_json_value_nx_ex(
                &lock_key,
                &json!({ "lockId": lock_id, "createdAt": time_utils::now_iso() }),
                AUTO_WHITELIST_OWNER_LOCK_TTL_SECONDS,
            )
            .await?
        {
            break;
        }
        if time::Instant::now() >= deadline {
            anyhow::bail!("Timed out waiting for auto whitelist owner lock");
        }
        time::sleep(std::time::Duration::from_millis(10)).await;
    }

    let result = ensure_session_auto_whitelist_locked(
        state,
        owner_key,
        ip,
        expire_at,
        comment,
        existing_record_id,
        candidate_record_id,
        new_record_status,
    )
    .await;
    let release_result = state.store.delete_lock_if_owned(&lock_key, &lock_id).await;
    if let Err(error) = release_result {
        // The lock is leased and expires on its own. Do not turn an already
        // persisted owner result into an error that the caller cannot clean up.
        tracing::warn!(%error, %owner_key, "failed to release auto whitelist owner lock");
    }
    result
}

#[allow(clippy::too_many_arguments)]
async fn ensure_session_auto_whitelist_locked(
    state: &AppState,
    owner_key: &str,
    ip: &str,
    expire_at: Option<i64>,
    comment: Option<String>,
    existing_record_id: Option<&str>,
    candidate_record_id: Option<&str>,
    new_record_status: &str,
) -> anyhow::Result<DeferredSessionAutoWhitelist> {
    let (target, target_type) =
        normalize_target(ip, "auto", Some("ip")).map_err(|message| anyhow::anyhow!(message))?;
    let owner_record_key = whitelist_auto_owner_record_key(owner_key);
    let owner_record_id = state.store.get_string_value(&owner_record_key).await?;
    let mut candidate_ids = Vec::new();
    for id in [existing_record_id, owner_record_id.as_deref()]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        if !candidate_ids
            .iter()
            .any(|candidate: &String| candidate == id)
        {
            candidate_ids.push(id.to_string());
        }
    }

    for candidate_id in candidate_ids {
        let Some(existing) = state.store.get_whitelist_record(&candidate_id).await? else {
            continue;
        };
        if existing.source != "auto" || existing.target_type() != "ip" {
            continue;
        }
        let reusable_pending = new_record_status == "pending" && existing.status == "pending";
        if (!existing.is_active() && !reusable_pending)
            || existing
                .expire_at
                .is_some_and(|value| value <= now_seconds())
        {
            remove_whitelist_record_by_id(state, &candidate_id).await?;
            continue;
        }

        let previous_targets = existing.concrete_targets();
        let mut next = existing.clone();
        next.ip = target.clone();
        next.target_type = target_type.clone();
        next.expire_at = expire_at;
        next.comment = comment.clone();
        if !existing.is_active() {
            next.status = new_record_status.to_string();
        }
        next.ip_location = cached_ip_location(state, &target).await;
        state
            .store
            .replace_whitelist_record(&existing, &next)
            .await?;
        state
            .store
            .set_string_value_with_optional_ttl(
                &owner_record_key,
                &next.id,
                expire_at.map(|value| value - now_seconds()),
            )
            .await?;
        return Ok(DeferredSessionAutoWhitelist {
            record: next,
            previous_targets,
        });
    }

    let record = WhitelistRecord {
        id: candidate_record_id
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("whitelist:{}", Uuid::new_v4())),
        ip: target.clone(),
        target_type,
        expire_at,
        source: "auto".to_string(),
        created_at: now_seconds(),
        status: new_record_status.to_string(),
        comment,
        ip_location: cached_ip_location(state, &target).await,
        resolved_targets: None,
        check_interval_minutes: None,
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: None,
        resolve_message: None,
    };
    state.store.insert_whitelist_record(&record).await?;
    let _ =
        ip_location::register_usage(state, &target, vec![format!("whitelist|{}", record.id)]).await;
    if let Err(error) = state
        .store
        .set_string_value_with_optional_ttl(
            &owner_record_key,
            &record.id,
            expire_at.map(|value| value - now_seconds()),
        )
        .await
    {
        let _ = state.store.delete_whitelist_record(&record.id).await;
        return Err(error.into());
    }
    Ok(DeferredSessionAutoWhitelist {
        record,
        previous_targets: Vec::new(),
    })
}

pub(crate) async fn publish_deferred_session_auto_whitelist(
    state: &AppState,
    mut deferred: DeferredSessionAutoWhitelist,
) -> anyhow::Result<WhitelistRecord> {
    if !deferred.record.is_active() {
        let previous = deferred.record.clone();
        deferred.record.status = "active".to_string();
        state
            .store
            .replace_whitelist_record(&previous, &deferred.record)
            .await?;
    }
    cleanup_removed_targets(state, &deferred.previous_targets).await;
    sync_reverse_proxy_trusted_ips_required(state).await?;
    Ok(deferred.record)
}

pub async fn rollback_session_auto_whitelist(
    state: &AppState,
    owner_key: &str,
    whitelist_record_id: &str,
) -> anyhow::Result<()> {
    let owner_record_key = whitelist_auto_owner_record_key(owner_key);
    state
        .store
        .delete_key_if_value(&owner_record_key, whitelist_record_id)
        .await?;
    remove_whitelist_record_by_id(state, whitelist_record_id).await?;
    Ok(())
}

pub async fn remove_whitelist_record_by_id(state: &AppState, id: &str) -> anyhow::Result<bool> {
    match state.store.delete_whitelist_record(id).await? {
        Some(record) => {
            let targets = record.concrete_targets();
            cleanup_removed_targets(state, &targets).await;
            sync_reverse_proxy_trusted_ips_required(state).await?;
            Ok(true)
        }
        None => Ok(false),
    }
}

pub async fn remove_whitelist_records_by_source(
    state: &AppState,
    source: &str,
) -> anyhow::Result<usize> {
    let records = state.store.list_whitelist_records().await?;
    let ids = whitelist_record_ids_by_source(&records, source);
    let mut removed = 0usize;
    for id in ids {
        if remove_whitelist_record_by_id(state, &id).await? {
            removed += 1;
        }
    }
    Ok(removed)
}

pub async fn remove_whitelist_records_by_ip(
    state: &AppState,
    ip: &str,
    source: Option<&str>,
) -> anyhow::Result<bool> {
    let normalized_ip = normalize_ip(ip);
    let target = if normalized_ip.is_empty() {
        ip.trim().to_string()
    } else {
        normalized_ip
    };
    if target.is_empty() {
        return Ok(false);
    }

    let records = state
        .store
        .find_whitelist_records_by_target(&target, "ip", source)
        .await?;
    let mut removed = false;
    for record in records {
        if remove_whitelist_record_by_id(state, &record.id).await? {
            removed = true;
        }
    }
    Ok(removed)
}

pub async fn move_record_to_ip(
    state: &AppState,
    id: &str,
    new_ip: &str,
) -> anyhow::Result<Option<WhitelistRecord>> {
    let Some(record) = state.store.get_whitelist_record(id).await? else {
        return Ok(None);
    };
    if !record.is_active() || record.target_type() != "ip" {
        return Ok(None);
    }
    if record
        .expire_at
        .is_some_and(|expire_at| expire_at <= now_seconds())
    {
        return Ok(None);
    }

    let old_ip = normalize_ip(&record.ip);
    let old_ip = if old_ip.is_empty() {
        record.ip.trim().to_string()
    } else {
        old_ip
    };
    let target = normalize_ip(new_ip);
    let target = if target.is_empty() {
        new_ip.trim().to_string()
    } else {
        target
    };
    if target.is_empty() {
        return Ok(None);
    }
    if old_ip == target {
        return Ok(Some(record));
    }

    let previous_targets = record.concrete_targets();
    let mut next = record.clone();
    next.ip = target.clone();
    next.target_type = "ip".to_string();
    if let Some(ip_location) = cached_ip_location(state, &target).await {
        next.ip_location = Some(ip_location);
    }
    state.store.replace_whitelist_record(&record, &next).await?;
    let _ =
        ip_location::register_usage(state, &target, vec![format!("whitelist|{}", next.id)]).await;
    cleanup_removed_targets(state, &previous_targets).await;
    sync_reverse_proxy_trusted_ips_required(state).await?;
    Ok(Some(next))
}

fn whitelist_record_ids_by_source(records: &[WhitelistRecord], source: &str) -> Vec<String> {
    records
        .iter()
        .filter(|record| record.source == source)
        .map(|record| record.id.clone())
        .collect()
}

async fn run_whitelist_maintenance_once(state: &AppState) -> anyhow::Result<()> {
    let now = now_seconds();
    let mut changed = false;

    for record in state.store.list_whitelist_records().await? {
        if record.expire_at.is_some_and(|expire_at| expire_at <= now) {
            let targets = record.concrete_targets();
            if state
                .store
                .expire_whitelist_record(&record.id)
                .await?
                .is_some()
            {
                cleanup_removed_targets(state, &targets).await;
                changed = true;
            }
            continue;
        }

        if cname_refresh_due(&record, now) {
            match refresh_cname_record(state, &record.id).await {
                Ok(Some(result)) => {
                    changed = changed
                        || result
                            .get("changed")
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(%error, record_id = %record.id, "failed to refresh due whitelist CNAME record");
                }
            }
        }
    }

    for group in state.store.list_whitelist_region_groups().await? {
        if group.expire_at.is_some_and(|expire_at| expire_at <= now) {
            let targets = group.concrete_targets();
            if state
                .store
                .expire_whitelist_region_group(&group.id)
                .await?
                .is_some()
            {
                cleanup_removed_targets(state, &targets).await;
                changed = true;
            }
        }
    }

    if changed {
        sync_reverse_proxy_trusted_ips_required(state).await?;
    }
    Ok(())
}

fn cname_refresh_due(record: &WhitelistRecord, now: i64) -> bool {
    if record.target_type() != "cname" || !record.is_active() {
        return false;
    }
    if record.expire_at.is_some_and(|expire_at| expire_at <= now) {
        return false;
    }
    let interval = normalize_cname_check_interval(record.check_interval_minutes) * 60;
    record
        .last_checked_at
        .is_none_or(|last_checked_at| now - last_checked_at >= interval)
}

async fn list_whitelist_regions(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.list_whitelist_region_groups().await {
        Ok(groups) => response::ok(
            groups
                .into_iter()
                .map(|group| group.summary())
                .collect::<Vec<_>>(),
        )
        .into_response(),
        Err(error) => {
            tracing::warn!(%error, "failed to list whitelist region groups");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "regionListFailed"),
            )
        }
    }
}

async fn add_whitelist_regions(
    State(state): State<AppState>,
    Json(body): Json<AddWhitelistRegionsBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let regions = match normalize_whitelist_region_inputs(&body.regions) {
        Ok(regions) => regions,
        Err(message) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                crate::cidr::localize_error(&translator, &message),
            );
        }
    };
    if regions.is_empty() {
        return response::error(
            StatusCode::BAD_REQUEST,
            whitelist_text(&translator, "regionRequired"),
        );
    }

    let policy = match resolve_whitelist_region_policy(&state, &regions).await {
        Ok(policy) => policy,
        Err(WhitelistRegionResolveError::Empty) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                whitelist_text(&translator, "regionEmpty"),
            );
        }
        Err(WhitelistRegionResolveError::Lookup(message)) => {
            return response::error(
                StatusCode::BAD_GATEWAY,
                crate::cidr::localize_error(&translator, &message),
            );
        }
    };

    let now = now_seconds();
    let comment = body
        .comment
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let record = WhitelistRegionGroupRecord {
        id: format!("whitelist-region:{}", Uuid::new_v4()),
        regions,
        cidrs: Vec::new(),
        policy_id: policy.id.clone(),
        policy: Some(policy.to_transport_value()),
        source_cidr_count: policy.source_cidr_count,
        range_count: policy.range_count(),
        expire_at: body.expire_at,
        source: "manual".to_string(),
        created_at: now,
        updated_at: now,
        status: "active".to_string(),
        comment,
    };

    if let Err(error) = state.store.insert_whitelist_region_group(&record).await {
        tracing::warn!(%error, "failed to insert whitelist region group");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            whitelist_text(&translator, "regionAddFailed"),
        );
    }

    if let Err(error) = sync_reverse_proxy_trusted_ips_required(&state).await {
        tracing::warn!(%error, group_id = %record.id, "failed to activate whitelist region group");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            whitelist_text(&translator, "regionAddFailed"),
        );
    }
    response::ok(json!({
        "group": record.summary(),
        "total": record.source_cidr_count,
    }))
    .into_response()
}

async fn delete_whitelist_region(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.delete_whitelist_region_group(&id).await {
        Ok(Some(group)) => {
            let targets = group.concrete_targets();
            cleanup_removed_targets(&state, &targets).await;
            if let Err(error) = sync_reverse_proxy_trusted_ips_required(&state).await {
                tracing::warn!(%error, %id, "failed to revoke whitelist region group runtime");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    whitelist_text(&translator, "regionDeleteFailed"),
                );
            }
            response::success_empty().into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            whitelist_text(&translator, "regionNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete whitelist region group");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "regionDeleteFailed"),
            )
        }
    }
}

async fn list_whitelist(State(state): State<AppState>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.list_whitelist_records().await {
        Ok(records) => {
            let mut hydrated = Vec::with_capacity(records.len());
            for record in records {
                hydrated.push(hydrate_whitelist_record_ip_location(&state, record).await);
            }
            response::ok(
                hydrated
                    .into_iter()
                    .map(|record| whitelist_record_for_response(record, &translator))
                    .collect::<Vec<_>>(),
            )
            .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "failed to list whitelist records");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "listFailed"),
            )
        }
    }
}

async fn hydrate_whitelist_record_ip_location(
    state: &AppState,
    mut record: WhitelistRecord,
) -> WhitelistRecord {
    if record.target_type() != "ip" {
        return record;
    }
    match ip_location::register_usage(state, &record.ip, vec![format!("whitelist|{}", record.id)])
        .await
    {
        Ok(location) if !location.trim().is_empty() => {
            record.ip_location = Some(location);
        }
        Ok(_) => {}
        Err(error) => {
            tracing::debug!(%error, record_id = %record.id, ip = %record.ip, "failed to hydrate whitelist IP location");
        }
    }
    record
}

async fn add_whitelist(
    State(state): State<AppState>,
    Json(body): Json<AddWhitelistBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    let source = normalize_source(body.source.as_deref());
    let (target, target_type) =
        match normalize_target(&body.ip, &source, body.target_type.as_deref()) {
            Ok(value) => value,
            Err(message) => {
                return response::error(
                    StatusCode::BAD_REQUEST,
                    localize_whitelist_error(&translator, message),
                );
            }
        };

    let existing = match state
        .store
        .find_whitelist_records_by_target(&target, &target_type, Some(&source))
        .await
    {
        Ok(records) => records,
        Err(error) => {
            tracing::warn!(%error, "failed to find existing whitelist records before add");
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "updateRecordsFailed"),
            );
        }
    };

    for record in existing {
        if let Ok(Some(deleted)) = state.store.delete_whitelist_record(&record.id).await {
            let targets = deleted.concrete_targets();
            cleanup_removed_targets(&state, &targets).await;
        }
    }

    let id = format!("whitelist:{}", Uuid::new_v4());
    let now = now_seconds();
    let ip_location = if target_type == "ip" {
        cached_ip_location(&state, &target).await
    } else {
        None
    };
    let record = WhitelistRecord {
        id: id.clone(),
        ip: target.clone(),
        target_type: target_type.clone(),
        expire_at: body.expire_at,
        source,
        created_at: now,
        status: "active".to_string(),
        comment: body.comment,
        ip_location,
        resolved_targets: (target_type == "cname").then(Vec::new),
        check_interval_minutes: (target_type == "cname")
            .then_some(normalize_cname_check_interval(body.check_interval_minutes)),
        last_checked_at: None,
        last_resolved_at: None,
        resolve_status: (target_type == "cname").then(|| "pending".to_string()),
        resolve_message: None,
    };

    if let Err(error) = state.store.insert_whitelist_record(&record).await {
        tracing::warn!(%error, "failed to insert whitelist record");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            whitelist_text(&translator, "addFailed"),
        );
    }

    if target_type == "cname" {
        let _ = refresh_cname_record(&state, &id).await;
    } else {
        if target_type == "ip" {
            let _ =
                ip_location::register_usage(&state, &target, vec![format!("whitelist|{id}")]).await;
        }
    }
    if let Err(error) = sync_reverse_proxy_trusted_ips_required(&state).await {
        tracing::warn!(%error, %id, "failed to activate whitelist runtime");
        return response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            whitelist_text(&translator, "addFailed"),
        );
    }
    response::ok(json!({ "id": id })).into_response()
}

async fn delete_whitelist(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let translator = Translator::from_state(&state).await;
    match state.store.delete_whitelist_record(&id).await {
        Ok(Some(record)) => {
            let targets = record.concrete_targets();
            cleanup_removed_targets(&state, &targets).await;
            if let Err(error) = sync_reverse_proxy_trusted_ips_required(&state).await {
                tracing::warn!(%error, %id, "failed to revoke whitelist runtime");
                return response::error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    whitelist_text(&translator, "deleteFailed"),
                );
            }
            response::success_empty().into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            whitelist_text(&translator, "recordNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to delete whitelist record");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "deleteFailed"),
            )
        }
    }
}

async fn update_whitelist_comment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<CommentBody>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match state
        .store
        .update_whitelist_comment(&id, body.comment)
        .await
    {
        Ok(Some(_)) => response::success_empty().into_response(),
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            whitelist_text(&translator, "recordNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to update whitelist comment");
            response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                whitelist_text(&translator, "commentUpdateFailed"),
            )
        }
    }
}

async fn refresh_whitelist_cname(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    let translator = Translator::from_state(&state).await;
    match refresh_cname_record(&state, &id).await {
        Ok(Some(result)) => {
            if result
                .get("record")
                .and_then(|record| record.get("resolveStatus"))
                .and_then(Value::as_str)
                == Some("error")
            {
                let message = result
                    .get("record")
                    .and_then(|record| record.get("resolveMessage"))
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .unwrap_or_else(|| whitelist_text(&translator, "domainResolveFailed"));
                return failure_with_data(StatusCode::OK, message, result);
            }
            response::ok(result).into_response()
        }
        Ok(None) => response::error(
            StatusCode::NOT_FOUND,
            whitelist_text(&translator, "recordNotFound"),
        ),
        Err(error) => {
            tracing::warn!(%error, %id, "failed to refresh whitelist CNAME record");
            response::error(
                StatusCode::BAD_REQUEST,
                whitelist_text(&translator, "refreshFailed"),
            )
        }
    }
}

async fn refresh_cname_record(state: &AppState, id: &str) -> anyhow::Result<Option<Value>> {
    let Some(record) = state.store.get_whitelist_record(id).await? else {
        return Ok(None);
    };
    if record.target_type() != "cname" || !record.is_active() {
        return Ok(None);
    }
    let translator = Translator::from_state(state).await;

    let now = now_seconds();
    let previous_targets = record.concrete_targets();
    let mut next = record.clone();
    next.check_interval_minutes = Some(normalize_cname_check_interval(
        record.check_interval_minutes,
    ));
    next.last_checked_at = Some(now);

    match resolve_cname_targets(&record.ip, &translator).await {
        Ok(resolved_targets) => {
            let previous_ips = previous_targets
                .iter()
                .map(|target| target.target.clone())
                .collect::<Vec<_>>();
            let changed = previous_ips != resolved_targets;
            next.resolved_targets = Some(resolved_targets.clone());
            next.last_resolved_at = Some(now);
            next.resolve_status = Some(if resolved_targets.is_empty() {
                "empty".to_string()
            } else {
                "resolved".to_string()
            });
            next.resolve_message = Some(if resolved_targets.is_empty() {
                whitelist_manager_text(&translator, "noAaaaRecords")
            } else {
                whitelist_manager_text_params(
                    &translator,
                    "resolvedIpCount",
                    &[("count", resolved_targets.len().to_string())],
                )
            });
            state.store.replace_whitelist_record(&record, &next).await?;

            let next_targets = next.concrete_targets();
            let removed = diff_targets(&previous_targets, &next_targets);
            cleanup_removed_targets(state, &removed).await;
            sync_reverse_proxy_trusted_ips_required(state).await?;
            Ok(Some(json!({
                "record": next,
                "changed": changed,
                "skipped": false
            })))
        }
        Err(message) => {
            next.resolved_targets = Some(Vec::new());
            next.resolve_status = Some("error".to_string());
            next.resolve_message = Some(message);
            state.store.replace_whitelist_record(&record, &next).await?;
            cleanup_removed_targets(state, &previous_targets).await;
            sync_reverse_proxy_trusted_ips_required(state).await?;
            Ok(Some(json!({
                "record": next,
                "changed": !previous_targets.is_empty(),
                "skipped": false
            })))
        }
    }
}

async fn resolve_cname_targets(
    domain: &str,
    translator: &Translator,
) -> Result<Vec<String>, String> {
    let addresses = match lookup_host((domain, 0)).await {
        Ok(addresses) => addresses,
        Err(error) if is_node_no_data_lookup_error(&error) => return Ok(Vec::new()),
        Err(error) => {
            return Err(whitelist_manager_text_params(
                translator,
                "dnsRecordQueryFailed",
                &[
                    ("label", "A / AAAA".to_string()),
                    ("message", error.to_string()),
                ],
            ));
        }
    };
    let mut targets = BTreeSet::new();
    for address in addresses {
        targets.insert(address.ip().to_string());
    }
    Ok(targets.into_iter().collect())
}

fn is_node_no_data_lookup_error(error: &io::Error) -> bool {
    if matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::AddrNotAvailable
    ) {
        return true;
    }

    let message = error.to_string().to_ascii_uppercase();
    [
        "ENODATA",
        "ENOTFOUND",
        "EAI_NODATA",
        "EAI_NONAME",
        "NAME OR SERVICE NOT KNOWN",
        "NODENAME NOR SERVNAME",
        "NO ADDRESS ASSOCIATED WITH HOSTNAME",
        "NO SUCH HOST",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

async fn cleanup_removed_targets(state: &AppState, targets: &[WhitelistConcreteTarget]) {
    match state
        .store
        .cleanup_whitelist_concrete_targets(targets)
        .await
    {
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(%error, "failed to cleanup whitelist concrete targets");
        }
    }
}

async fn should_sync_direct_firewall(state: &AppState) -> bool {
    match direct_firewall_sync_required(state).await {
        Ok(required) => required,
        Err(error) => {
            tracing::warn!(%error, "failed to read config for whitelist firewall sync");
            false
        }
    }
}

async fn direct_firewall_sync_required(state: &AppState) -> anyhow::Result<bool> {
    if !runtime_profile::host_firewall_available(state) {
        return Ok(false);
    }
    let config = state.store.get_config().await?;
    let run_type = config.get("run_type").and_then(Value::as_i64).unwrap_or(3);
    Ok(run_type == 0
        || config
            .get("auto_manage_firewall")
            .and_then(Value::as_bool)
            .unwrap_or(true))
}

pub async fn migrate_whitelist_ipsets_on_boot(state: &AppState) -> anyhow::Result<()> {
    migrate_whitelist_ipsets_in_storage(state).await?;
    sync_reverse_proxy_trusted_ips_inner(state).await
}

pub(crate) async fn migrate_whitelist_ipsets_in_storage(state: &AppState) -> anyhow::Result<()> {
    state
        .store
        .migrate_whitelist_region_groups_to_ipsets()
        .await?;
    rebuild_whitelist_ipset_snapshots(state).await
}

pub fn whitelist_snapshot_contains(
    state: &AppState,
    address: IpAddr,
    sources: Option<&[&str]>,
) -> bool {
    match sources {
        None => state
            .ipsets
            .get(WHITELIST_ALL_IPSET_KEY)
            .is_some_and(|policy| policy.contains(address)),
        Some(sources) => sources.iter().any(|source| {
            let key = match *source {
                "manual" => WHITELIST_MANUAL_IPSET_KEY,
                "auto" => WHITELIST_AUTO_IPSET_KEY,
                _ => return false,
            };
            state
                .ipsets
                .get(key)
                .is_some_and(|policy| policy.contains(address))
        }),
    }
}

pub(crate) async fn rebuild_whitelist_ipset_snapshots(state: &AppState) -> anyhow::Result<()> {
    let now = now_seconds();
    let mut manual_policies = Vec::<CompiledIpSet>::new();
    let mut auto_policies = Vec::<CompiledIpSet>::new();
    let mut manual_cidrs = Vec::new();
    let mut auto_cidrs = Vec::new();

    for record in state.store.list_whitelist_records().await? {
        if !record.is_active() || record.expire_at.is_some_and(|expire_at| expire_at <= now) {
            continue;
        }
        let output = if record.source == "auto" {
            &mut auto_cidrs
        } else {
            &mut manual_cidrs
        };
        for target in record.concrete_targets() {
            if target.target_type == "cidr" {
                output.push(target.target);
            } else if let Ok(address) = normalize_ip(&target.target).parse::<IpAddr>() {
                output.push(host_cidr(address));
            }
        }
    }
    for group in state.store.list_whitelist_region_groups().await? {
        if !group.is_active() || group.expire_at.is_some_and(|expire_at| expire_at <= now) {
            continue;
        }
        let policy = group.policy_result().map_err(|error| {
            anyhow::anyhow!(
                "whitelist region group {} policy is invalid: {error}",
                group.id
            )
        })?;
        if group.source == "auto" {
            auto_policies.push(policy);
        } else {
            manual_policies.push(policy);
        }
    }

    manual_policies.push(compile_ip_set(manual_cidrs).map_err(anyhow::Error::msg)?);
    auto_policies.push(compile_ip_set(auto_cidrs).map_err(anyhow::Error::msg)?);
    let manual = union_ip_sets(manual_policies.iter());
    let auto = union_ip_sets(auto_policies.iter());
    let all = union_ip_sets([&manual, &auto]);
    state.ipsets.publish(
        WHITELIST_MANUAL_IPSET_KEY,
        (manual.range_count() > 0).then_some(manual),
    );
    state.ipsets.publish(
        WHITELIST_AUTO_IPSET_KEY,
        (auto.range_count() > 0).then_some(auto),
    );
    state.ipsets.publish(
        WHITELIST_ALL_IPSET_KEY,
        (all.range_count() > 0).then_some(all),
    );
    Ok(())
}

fn host_cidr(address: IpAddr) -> String {
    match address {
        IpAddr::V4(address) => format!("{address}/32"),
        IpAddr::V6(address) => format!("{address}/128"),
    }
}

pub async fn sync_reverse_proxy_trusted_ips(state: &AppState) {
    if let Err(error) = sync_reverse_proxy_trusted_ips_inner(state).await {
        tracing::warn!(%error, "failed to synchronize reverse proxy trusted IP runtime");
    }
}

pub async fn sync_reverse_proxy_trusted_ips_required(state: &AppState) -> anyhow::Result<()> {
    // Only managed host-firewall deployments have an external authorization
    // boundary that must be confirmed before acknowledging a mutation.
    if !direct_firewall_sync_required(state).await? {
        sync_reverse_proxy_trusted_ips(state).await;
        return Ok(());
    }
    sync_reverse_proxy_trusted_ips_inner(state).await
}

pub(crate) async fn sync_direct_firewall_whitelist(state: &AppState) -> anyhow::Result<usize> {
    let _guard = state.whitelist_runtime_sync_lock.lock().await;
    rebuild_whitelist_ipset_snapshots(state).await?;
    sync_direct_firewall_whitelist_from_snapshot(state).await
}

async fn sync_reverse_proxy_trusted_ips_inner(state: &AppState) -> anyhow::Result<()> {
    let _guard = state.whitelist_runtime_sync_lock.lock().await;
    rebuild_whitelist_ipset_snapshots(state).await?;
    let runtime = compile_reverse_proxy_trusted_ips(state).await?;
    let ips = runtime
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("ip").and_then(Value::as_str))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    // Persist the derived snapshots before network publication. Session
    // revocation and local authorization readers must immediately observe the
    // new generation even while the gateway is reconnecting.
    state
        .store
        .save_reverse_proxy_trusted_ips_runtime(&runtime)
        .await?;
    state
        .store
        .save_gateway_trusted_client_ips_runtime(&runtime)
        .await?;
    let gateway_payload = json!({
        "enabled": runtime.get("enabled").and_then(Value::as_bool).unwrap_or(false),
        "ips": if runtime.get("enabled").and_then(Value::as_bool).unwrap_or(false) {
            ips.clone()
        } else {
            Vec::new()
        },
        "cidrs": Vec::<String>::new(),
        "policy_id": runtime.get("policy_id").cloned().unwrap_or(Value::Null),
        "policy": runtime.get("policy").cloned().unwrap_or(Value::Null),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null)
    });
    let value = state
        .go_backend
        .set_reverse_proxy_throttle_exempt_ips(&gateway_payload)
        .await?;
    if !crate::go_backend::response_success(&value) {
        anyhow::bail!(
            "{}",
            crate::go_backend::response_message(
                &value,
                "Failed to sync reverse proxy trusted IP runtime",
            )
        );
    }
    sync_direct_firewall_whitelist_from_snapshot(state).await?;
    let trusted_payload = json!({
        "ips": ips,
        "cidrs": Vec::<String>::new(),
        "policy_id": runtime.get("policy_id").cloned().unwrap_or(Value::Null),
        "policy": runtime.get("policy").cloned().unwrap_or(Value::Null),
        "updated_at": runtime.get("updated_at").cloned().unwrap_or(Value::Null)
    });
    let trusted_result = state
        .go_backend
        .set_gateway_trusted_client_ips(&trusted_payload)
        .await?;
    if !crate::go_backend::response_success(&trusted_result) {
        anyhow::bail!(
            "{}",
            crate::go_backend::response_message(
                &trusted_result,
                "Failed to sync gateway trusted client IP runtime",
            )
        );
    }
    Ok(())
}

async fn sync_direct_firewall_whitelist_from_snapshot(state: &AppState) -> anyhow::Result<usize> {
    if !should_sync_direct_firewall(state).await {
        return Ok(0);
    }
    let policy = state
        .ipsets
        .get(WHITELIST_ALL_IPSET_KEY)
        .map(|policy| (*policy).clone())
        .unwrap_or_else(|| compile_ip_set(std::iter::empty::<&str>()).expect("empty IP set"));
    let payload = json!({
        "policy_id": policy.id,
        "policy": policy.to_transport_value(),
    });
    let result = state.go_backend.sync_whitelist_firewall(&payload).await?;
    if !crate::go_backend::response_success(&result) {
        anyhow::bail!(
            "{}",
            crate::go_backend::response_message(
                &result,
                "Failed to sync direct-mode whitelist firewall",
            )
        );
    }
    Ok(policy.range_count())
}

async fn compile_reverse_proxy_trusted_ips(state: &AppState) -> anyhow::Result<Value> {
    let config = state.store.get_config().await?;
    let enabled = config
        .pointer("/reverse_proxy_throttle/enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let mobility_enabled = config
        .pointer("/auth_credential_settings/session_ip_mobility_enabled")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let sessions = state.store.list_session_values().await?;
    let whitelist_targets = state.store.list_whitelist_active_concrete_targets().await?;
    let whitelist_region_groups = state.store.list_whitelist_region_groups().await?;

    let mut source_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut session_linked_auto_whitelist_final_ip_by_record_id = BTreeMap::<String, String>::new();
    for (session_id, data) in sessions {
        let parsed_session = serde_json::from_value::<LoginSession>(data.clone()).ok();
        if parsed_session
            .as_ref()
            .is_some_and(crate::auth::routes::login_session_has_expired)
        {
            continue;
        }
        if mobility_enabled {
            if let Some(session) = parsed_session.as_ref() {
                add_ip_source(
                    &mut source_map,
                    &session.ip,
                    format!("session:{session_id}"),
                );
                for ip in auth_mobility::effective_session_ips(state, &session_id, session, &config)
                    .await?
                {
                    add_ip_source(&mut source_map, &ip, format!("session:{session_id}"));
                }
            }
            continue;
        }
        let ip = parsed_session
            .as_ref()
            .map(|session| session.ip.as_str())
            .or_else(|| data.get("ip").and_then(Value::as_str))
            .unwrap_or("");
        let normalized_ip = normalize_ip(ip);
        add_ip_source(
            &mut source_map,
            &normalized_ip,
            format!("session:{session_id}"),
        );
        if !normalized_ip.is_empty()
            && let Some(record_id) = parsed_session
                .as_ref()
                .and_then(|session| session.post_login_ip_grant_record_id.as_deref())
                .or_else(|| data.get("postLoginIpGrantRecordId").and_then(Value::as_str))
                .map(str::trim)
                .filter(|record_id| !record_id.is_empty())
        {
            session_linked_auto_whitelist_final_ip_by_record_id
                .insert(record_id.to_string(), normalized_ip);
        }
    }

    let mut cidrs = Vec::<String>::new();
    let mut cidr_keys = BTreeSet::<String>::new();
    for target in whitelist_targets {
        if target.target_type == "cidr" {
            let key = target.target.to_ascii_lowercase();
            if cidr_keys.insert(key) {
                cidrs.push(target.target);
            }
            continue;
        }
        let compiled_target = reverse_proxy_compiled_whitelist_target(
            &target,
            &session_linked_auto_whitelist_final_ip_by_record_id,
            mobility_enabled,
        );
        add_ip_source(
            &mut source_map,
            compiled_target,
            format!("whitelist:{}:{}", target.source, target.record_id),
        );
    }

    let items = source_map
        .into_iter()
        .map(
            |(ip, sources)| json!({ "ip": ip, "sources": sources.into_iter().collect::<Vec<_>>() }),
        )
        .collect::<Vec<_>>();

    let mut policies = vec![compile_ip_set(&cidrs).map_err(anyhow::Error::msg)?];
    for group in whitelist_region_groups {
        if group
            .expire_at
            .is_some_and(|expire_at| expire_at <= now_seconds())
        {
            continue;
        }
        policies.push(group.policy_result().map_err(|error| {
            anyhow::anyhow!(
                "whitelist region group {} policy is invalid: {error}",
                group.id
            )
        })?);
    }
    let policy = union_ip_sets(policies.iter());
    Ok(json!({
        "enabled": enabled,
        "items": items,
        "policy_id": policy.id.clone(),
        "source_cidr_count": policy.source_cidr_count,
        "range_count": policy.range_count(),
        "policy": policy.to_transport_value(),
        "updated_at": time_utils::now_iso()
    }))
}

fn reverse_proxy_compiled_whitelist_target<'a>(
    target: &'a WhitelistConcreteTarget,
    session_linked_auto_whitelist_final_ip_by_record_id: &'a BTreeMap<String, String>,
    mobility_enabled: bool,
) -> &'a str {
    if !mobility_enabled && target.source == "auto" && target.record_target_type == "ip" {
        session_linked_auto_whitelist_final_ip_by_record_id
            .get(&target.record_id)
            .map(String::as_str)
            .unwrap_or(&target.target)
    } else {
        &target.target
    }
}

fn add_ip_source(source_map: &mut BTreeMap<String, BTreeSet<String>>, ip: &str, source: String) {
    let normalized = normalize_ip(ip);
    if normalized.parse::<IpAddr>().is_err() {
        return;
    }
    source_map.entry(normalized).or_default().insert(source);
}

pub(crate) fn whitelist_auto_owner_record_key(owner_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(owner_key.trim());
    format!(
        "fn_knock:whitelist:auto_owner:{}",
        hex::encode(hasher.finalize())
    )
}

async fn cached_ip_location(state: &AppState, ip: &str) -> Option<String> {
    state
        .store
        .get_ip_location_cache(ip)
        .await
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("raw")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string)
        })
}

fn normalize_source(value: Option<&str>) -> String {
    if value == Some("auto") {
        "auto".to_string()
    } else {
        "manual".to_string()
    }
}

#[derive(Debug)]
enum WhitelistRegionResolveError {
    Empty,
    Lookup(String),
}

fn normalize_whitelist_region_inputs(value: &[Value]) -> Result<Vec<WhitelistRegionInput>, String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for item in value {
        let Some(object) = item.as_object() else {
            continue;
        };
        let province = js_region_string(object.get("province")).trim().to_string();
        if province.is_empty() {
            continue;
        }
        let query_city = js_region_string(object.get("query_city"))
            .trim()
            .to_string();
        let query_city = (!query_city.is_empty()).then_some(query_city);
        let operator = CidrOperator::parse_value(object.get("operator"))?;
        let key = CidrRegionQuery::new(province.clone(), query_city.clone(), operator).key();
        if seen.insert(key) {
            result.push(WhitelistRegionInput {
                province,
                query_city,
                operator,
            });
        }
    }
    Ok(result)
}

fn js_region_string(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| js_region_string(Some(item)))
            .collect::<Vec<_>>()
            .join(","),
        Some(Value::Object(_)) => "[object Object]".to_string(),
    }
}

async fn resolve_whitelist_region_policy(
    state: &AppState,
    regions: &[WhitelistRegionInput],
) -> Result<CompiledIpSet, WhitelistRegionResolveError> {
    let mut policies = Vec::new();
    for region in regions {
        let query = CidrRegionQuery::new(
            region.province.clone(),
            region.query_city.clone(),
            region.operator,
        );
        let lookup = crate::cidr::lookup_region(state, &query)
            .await
            .map_err(|error| WhitelistRegionResolveError::Lookup(error.to_string()))?;
        policies.push(lookup.policy);
    }
    let policy = crate::cidr::union_ip_sets(policies.iter());
    if policy.range_count() == 0 {
        return Err(WhitelistRegionResolveError::Empty);
    }
    Ok(policy)
}

fn normalize_target(
    value: &str,
    source: &str,
    target_type: Option<&str>,
) -> Result<(String, String), &'static str> {
    let inferred = match target_type {
        Some("ip") => Some("ip"),
        Some("cidr") => Some("cidr"),
        Some("cname") => Some("cname"),
        _ => infer_target_type(value),
    }
    .ok_or("Invalid whitelist target format")?;

    if source == "auto" && inferred != "ip" {
        return Err("Automatic whitelist grants only support IP targets");
    }

    let target = match inferred {
        "cidr" => normalize_cidr(value),
        "cname" => normalize_domain(value),
        _ => {
            let normalized = normalize_ip(value);
            (!normalized.is_empty()).then_some(normalized)
        }
    }
    .ok_or(match inferred {
        "cidr" => "Invalid whitelist CIDR",
        "cname" => "Invalid whitelist domain",
        _ => "Invalid whitelist IP",
    })?;

    Ok((target, inferred.to_string()))
}

fn infer_target_type(value: &str) -> Option<&'static str> {
    if normalize_cidr(value).is_some() {
        return Some("cidr");
    }
    if !normalize_ip(value).is_empty() {
        return Some("ip");
    }
    if normalize_domain(value).is_some() {
        return Some("cname");
    }
    None
}

fn normalize_cidr(value: &str) -> Option<String> {
    let parsed = IpNet::from_str(value.trim()).ok()?;
    Some(match parsed {
        IpNet::V4(network) => format!("{}/{}", network.network(), network.prefix_len()),
        IpNet::V6(network) => format!("{}/{}", network.network(), network.prefix_len()),
    })
}

fn normalize_domain(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('.').to_ascii_lowercase();
    if trimmed.is_empty() || trimmed.contains('/') || trimmed.contains("..") {
        return None;
    }
    let ascii = idna::domain_to_ascii(&trimmed).ok()?;
    if ascii.is_empty() || ascii.len() > 253 {
        return None;
    }
    let labels = ascii.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return None;
    }
    for label in labels {
        if label.is_empty()
            || label.len() > 63
            || label.starts_with('-')
            || label.ends_with('-')
            || !label
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
        {
            return None;
        }
    }
    Some(ascii)
}

fn normalize_cname_check_interval(value: Option<i64>) -> i64 {
    value.unwrap_or(DEFAULT_CNAME_CHECK_INTERVAL_MINUTES).clamp(
        MIN_CNAME_CHECK_INTERVAL_MINUTES,
        MAX_CNAME_CHECK_INTERVAL_MINUTES,
    )
}

fn diff_targets(
    left: &[WhitelistConcreteTarget],
    right: &[WhitelistConcreteTarget],
) -> Vec<WhitelistConcreteTarget> {
    left.iter()
        .filter(|candidate| {
            !right.iter().any(|other| {
                other.target == candidate.target && other.target_type == candidate.target_type
            })
        })
        .cloned()
        .collect()
}

use crate::time_utils::now_seconds;

fn failure_with_data(status: StatusCode, message: String, data: Value) -> Response {
    (
        status,
        Json(json!({
            "success": false,
            "message": message,
            "data": data
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
