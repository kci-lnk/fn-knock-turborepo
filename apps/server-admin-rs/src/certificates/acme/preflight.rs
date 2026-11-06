use super::*;
use crate::ddns::acme_txt::{TxtProvider, TxtRecord};
use serde::Serialize;
use utoipa::ToSchema;

mod dns;
mod routes;
#[cfg(test)]
mod tests;
pub(crate) use routes::{DeleteSelection, SelectedValue};
#[derive(Deserialize, ToSchema)]
pub(crate) struct ConfirmRequest {
    pub(super) snapshot: Option<String>,
}
pub(super) use routes::routes as preflight_routes;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChallengeGroup {
    name: String,
    domains: Vec<String>,
    values: Vec<String>,
    delegation: Vec<String>,
    status: String,
    selectable: Vec<String>,
    zone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PreflightReport {
    application_id: String,
    snapshot: String,
    checked_at: String,
    groups: Vec<ChallengeGroup>,
    pub(super) status: String,
    #[serde(default)]
    renewal_pending: bool,
    pub(super) cooldown: Option<Cooldown>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Cooldown {
    pub(super) source: String,
    pub(super) retry_at: i64,
    pub(super) estimated: bool,
}

#[derive(Debug, thiserror::Error)]
#[error("ACME DNS preflight requires attention")]
pub(super) struct NeedsReview(pub PreflightReport);

pub(super) fn gate_error(error: anyhow::Error) -> Response {
    if let Some(NeedsReview(report)) = error.downcast_ref::<NeedsReview>() {
        let (status, code) = if report.status == "cooldown" {
            (StatusCode::TOO_MANY_REQUESTS, "rate_limited")
        } else {
            (StatusCode::CONFLICT, "preflight_required")
        };
        let mut response = (
            status,
            Json(json!({"success":false,"errorCode":code,"message":code,"data":report})),
        )
            .into_response();
        if let Some(limit) = &report.cooldown
            && let Ok(value) = (limit.retry_at - time_utils::now_ms() / 1000)
                .max(0)
                .to_string()
                .parse()
        {
            response
                .headers_mut()
                .insert(axum::http::header::RETRY_AFTER, value);
        }
        response
    } else {
        response::error(StatusCode::CONFLICT, error.to_string())
    }
}

pub(super) fn key(id: &str) -> String {
    format!("fn_knock:acme:preflight:{id}")
}
pub(super) async fn last_report(
    state: &AppState,
    id: &str,
) -> crate::storage::StorageResult<Option<Value>> {
    state.storage.store.get_json_value(&key(id)).await
}

use super::hook::audited_hook;

async fn inspect_group(
    name: String,
    domains: Vec<String>,
    safe: bool,
    observation: anyhow::Result<dns::Observation>,
    provider: Option<&TxtProvider<'_>>,
) -> (
    ChallengeGroup,
    Vec<TxtRecord>,
    Option<crate::ddns::acme_txt::ProviderLimit>,
) {
    let mut group = ChallengeGroup {
        name,
        domains,
        zone: String::new(),
        status: "unknown".into(),
        values: vec![],
        delegation: vec![],
        selectable: vec![],
    };
    let Ok(observed) = observation else {
        return (group, vec![], None);
    };
    group.zone = observed.zone;
    group.values = observed.values.into_iter().collect();
    group.delegation = observed.delegation.into_iter().collect();
    // The pinned Cloudflare hook writes the original FQDN into its selected
    // zone. Accept NS delegation only when that zone is the challenge apex,
    // its assigned NS set matches the public referral, and the child agrees.
    let mut routed = false;
    if safe
        && !group.delegation.is_empty()
        && let Some(provider) = provider
        && let Some(servers) = group
            .delegation
            .iter()
            .map(|target| {
                target
                    .strip_prefix("NS ")
                    .map(|s| s.trim_end_matches('.').to_ascii_lowercase())
            })
            .collect::<Option<BTreeSet<_>>>()
    {
        match provider.owns_delegation(&group.name, &servers).await {
            Ok(true) => {
                if let Ok(child) = dns::delegated_txt(&group.name, servers).await
                    && child.delegation.is_empty()
                {
                    group.zone = child.zone;
                    group.values = child.values.into_iter().collect();
                    routed = true;
                }
            }
            Err(error) if error.is::<crate::ddns::acme_txt::ProviderLimit>() => {
                group.status = "delegated".into();
                return (group, vec![], error.downcast().ok());
            }
            _ => {}
        }
    }
    group.status = if !routed && !group.delegation.is_empty() {
        "delegated"
    } else if group.values.is_empty() {
        "clear"
    } else if safe {
        "existing"
    } else {
        "unsafe"
    }
    .into();
    let mut records = Vec::new();
    if matches!(group.status.as_str(), "clear" | "existing" | "unsafe")
        && let Some(provider) = provider
    {
        let result = tokio_time::timeout(
            std::time::Duration::from_secs(5),
            provider.list(&group.zone, &group.name),
        )
        .await;
        match result {
            Ok(Ok(list)) => records = merge_provider_records(&mut group, list, safe),
            failure => {
                if !safe && group.status == "clear" {
                    group.status = "unknown".into();
                }
                let limit = failure
                    .ok()
                    .and_then(Result::err)
                    .and_then(|err| err.downcast::<crate::ddns::acme_txt::ProviderLimit>().ok());
                return (group, records, limit);
            }
        }
    }
    (group, records, None)
}

// A provider may already have a TXT that no authority serves yet. Include it
// in the decision and residual check, but only allow deletion when both agree.
fn merge_provider_records(
    group: &mut ChallengeGroup,
    list: Vec<TxtRecord>,
    safe: bool,
) -> Vec<TxtRecord> {
    let authoritative: BTreeSet<_> = group.values.iter().cloned().collect();
    if !list.is_empty() {
        group.values = authoritative
            .iter()
            .cloned()
            .chain(list.iter().map(|r| r.value.clone()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        group.status = if safe { "existing" } else { "unsafe" }.into();
    }
    let mut records = vec![];
    for value in &authoritative {
        let mut matching = list.iter().filter(|r| &r.value == value);
        if let Some(record) = matching.next()
            && matching.next().is_none()
        {
            group.selectable.push(value.clone());
            records.push(record.clone());
        }
    }
    records
}

struct Inspection {
    report: PreflightReport,
    records: Vec<TxtRecord>,
    signature: String,
    fingerprint: String,
}

async fn inspect(
    state: &AppState,
    application: &Value,
    t: &Translator,
) -> anyhow::Result<Inspection> {
    let id = application["id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing application id"))?;
    let domains = application["domains"]
        .as_array()
        .map(|v| normalize_domain_list(v.iter()))
        .unwrap_or_default();
    anyhow::ensure!(!domains.is_empty(), "missing domains");
    let kind = application["dnsType"].as_str().unwrap_or_default();
    let safe = audited_hook(state, kind).await;

    let query_provider = !dns_cooling(state, application).await?;
    let mut groups = Vec::new();
    let mut records = Vec::new();
    let mut tasks = tokio::task::JoinSet::new();
    let mut names = dns::challenge_names(&domains).into_iter();
    loop {
        while tasks.len() < 8 {
            let Some((name, domains)) = names.next() else {
                break;
            };
            let application = application.clone();
            let t = t.clone();
            let lookup = dns::observe(name.clone());
            tasks.spawn(async move {
                let provider = query_provider
                    .then(|| {
                        TxtProvider::new(
                            &t,
                            application["dnsType"].as_str().unwrap_or(""),
                            &application["credentials"],
                        )
                        .ok()
                    })
                    .flatten();
                inspect_group(name, domains, safe, lookup.await, provider.as_ref()).await
            });
        }
        let Some(result) = tasks.join_next().await else {
            break;
        };
        let (group, found, limit) = result?;
        if let Some(limit) = limit {
            persist_provider_limit(state, application, &limit).await?;
        }
        groups.push(group);
        records.extend(found);
    }
    groups.sort_by(|a, b| a.name.cmp(&b.name));
    records.sort();
    let signature = config_signature(application);
    if let Some(pending) = state
        .storage
        .store
        .get_json_value(&propagation_key(&signature))
        .await?
    {
        for group in &mut groups {
            if pending
                .as_array()
                .into_iter()
                .flatten()
                .any(|r| r["name"] == group.name && group.values.iter().any(|v| r["value"] == *v))
            {
                group.status = "propagating".into();
                group.selectable.retain(|value| {
                    !pending
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|r| r["name"] == group.name && r["value"] == *value)
                });
            }
        }
    }
    let fingerprint = hex::encode(Sha256::digest(serde_json::to_vec(&(
        &signature, &groups, &records,
    ))?));
    let cooldown = super::analysis::active_cooldown(state, application).await?;
    let status = if cooldown.is_some() {
        "cooldown"
    } else if groups
        .iter()
        .any(|g| !matches!(g.status.as_str(), "clear" | "existing"))
    {
        "blocked"
    } else if groups.iter().any(|g| g.status == "existing") {
        "review"
    } else {
        "ready"
    };
    Ok(Inspection {
        report: PreflightReport {
            application_id: id.into(),
            snapshot: uuid::Uuid::new_v4().simple().to_string(),
            checked_at: now_node_iso(),
            groups,
            status: status.into(),
            renewal_pending: false,
            cooldown,
        },
        records,
        signature,
        fingerprint,
    })
}

pub(super) fn config_signature(application: &Value) -> String {
    hex::encode(Sha256::digest(
        json!([
            application["id"],
            application["domains"],
            application["dnsType"],
            application["credentials"]
        ])
        .to_string(),
    ))
}

async fn persist_provider_limit(
    state: &AppState,
    application: &Value,
    limit: &crate::ddns::acme_txt::ProviderLimit,
) -> anyhow::Result<()> {
    let signal = if limit.credential_scoped {
        "DNS API HTTP 429 credential rate limit exceeded"
    } else {
        "DNS API HTTP 429 Too Many Requests"
    };
    let mut evidence = vec![json!(signal)];
    if let Some(header) = &limit.retry_after {
        evidence.push(json!(format!("Retry-After: {header}")));
    }
    record_cooldown(state, application, &evidence).await
}

fn propagation_key(signature: &str) -> String {
    format!("fn_knock:acme:dns-propagation:{signature}")
}

async fn record_deletion(
    state: &AppState,
    signature: &str,
    name: &str,
    value: &str,
) -> anyhow::Result<()> {
    let key = propagation_key(signature);
    let mut entries = state
        .storage
        .store
        .get_json_value(&key)
        .await?
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    let selected = json!({"name":name,"value":value});
    if !entries.contains(&selected) {
        entries.push(selected);
    }
    state
        .storage
        .store
        .set_json_value_ex(&key, &json!(entries), 7 * 24 * 3600)
        .await?;
    Ok(())
}

async fn remember(state: &AppState, check: &Inspection) -> anyhow::Result<()> {
    state
        .storage
        .store
        .set_json_value(
            &key(&check.report.application_id),
            &serde_json::to_value(&check.report)?,
        )
        .await?;
    remember_snapshot(state, check).await
}
async fn remember_snapshot(state: &AppState, check: &Inspection) -> anyhow::Result<()> {
    state.storage.store.set_json_value_ex(&format!("fn_knock:acme:dns-snapshot:{}",check.report.snapshot),
        &json!({"report":check.report,"records":check.records,"signature":check.signature,"fingerprint":check.fingerprint}),900).await?;
    Ok(())
}

pub(super) async fn guard(
    state: &AppState,
    application: &Value,
    trigger: &str,
    confirmed: Option<&str>,
    t: &Translator,
) -> anyhow::Result<PreflightReport> {
    if let Some(limit) = active_cooldown(state, application).await? {
        let id = application["id"].as_str().unwrap_or_default();
        let mut report = last_report(state, id)
            .await?
            .and_then(|v| serde_json::from_value::<PreflightReport>(v).ok())
            .unwrap_or_else(|| PreflightReport {
                application_id: id.into(),
                snapshot: String::new(),
                checked_at: now_node_iso(),
                groups: vec![],
                status: String::new(),
                renewal_pending: false,
                cooldown: None,
            });
        report.cooldown = Some(limit);
        report.status = "cooldown".into();
        report.renewal_pending = trigger == "auto_renew";
        state
            .storage
            .store
            .set_json_value(&key(id), &serde_json::to_value(&report)?)
            .await?;
        return Err(NeedsReview(report).into());
    }
    let mut check = inspect(state, application, t).await?;
    if check
        .report
        .groups
        .iter()
        .all(|g| matches!(g.status.as_str(), "clear" | "existing" | "unsafe"))
    {
        state
            .storage
            .store
            .delete_key(&propagation_key(&check.signature))
            .await?;
    }
    let allowed = match confirmed {
        Some(snapshot)
            if snapshot.len() == 32
                && matches!(check.report.status.as_str(), "review" | "ready") =>
        {
            let key = format!("fn_knock:acme:dns-snapshot:{snapshot}");
            let saved = state.storage.store.get_json_value(&key).await?;
            let matches = saved.is_some_and(|s| s["fingerprint"] == check.fingerprint);
            if matches {
                state.storage.store.delete_key(&key).await?;
            }
            matches
        }
        Some(_) => false,
        None => {
            check.report.status == "ready"
                || (trigger == "auto_renew" && check.report.status == "review")
        }
    };
    check.report.renewal_pending = trigger == "auto_renew" && !allowed;
    remember(state, &check).await?;
    if !allowed {
        return Err(NeedsReview(check.report).into());
    }
    Ok(check.report)
}

/// Preserve the baseline so unchanged third-party TXT values are never called
/// leftovers. An observed difference alone cannot establish write ownership.
pub(super) async fn finish(
    state: &AppState,
    application: &Value,
    job_id: &str,
    t: &Translator,
) -> anyhow::Result<()> {
    let id = application["id"].as_str().unwrap_or_default();
    let Some(baseline) = state
        .storage
        .store
        .get_json_value(&format!("fn_knock:acme:dns-baseline:{job_id}"))
        .await?
    else {
        return Ok(());
    };
    if baseline["configSignature"] != config_signature(application) {
        update_acme_job(state, job_id, json!({"cleanupPending":true})).await?;
        state
            .storage
            .store
            .delete_key(&format!("fn_knock:acme:dns-baseline:{job_id}"))
            .await?;
        return Ok(());
    }
    // A DNS delta or text in client stdout is not proof of a successful
    // provider create. The client performs its native cleanup; the server
    // reports residuals for explicit review, including on restart.
    let mut check = inspect(state, application, t).await?;
    let pending = check.report.groups.iter().any(|group| {
        matches!(group.status.as_str(), "unknown" | "delegated")
            || group.values.iter().any(|v| {
                !baseline["groups"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|g| {
                        g["name"] == group.name
                            && g["values"]
                                .as_array()
                                .is_some_and(|values| values.iter().any(|val| val == v))
                    })
            })
    });
    if check.report.cooldown.is_none() {
        check.report.status = if pending { "cleanup" } else { "ready" }.into();
    }
    state
        .storage
        .store
        .set_json_value(&key(id), &serde_json::to_value(&check.report)?)
        .await?;
    update_acme_job(state, job_id, json!({"cleanupPending":pending})).await?;
    state
        .storage
        .store
        .delete_key(&format!("fn_knock:acme:dns-baseline:{job_id}"))
        .await?;
    Ok(())
}
