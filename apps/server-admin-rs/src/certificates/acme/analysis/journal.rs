//! The UI log is a bounded buffer. Keep rate-limit decisions separately so
//! stopping a job, noisy cleanup and process restarts cannot discard a 429.
use super::*;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tokio::sync::Mutex;

#[derive(Clone)]
pub(in crate::certificates::acme) struct JobRates {
    state: AppState,
    id: String,
    scopes: Arc<Vec<String>>,
    tracker: Arc<Mutex<Tracker>>,
}

#[derive(Default)]
struct Tracker {
    recent: VecDeque<String>,
    completed: BTreeMap<String, Cooldown>,
    current: Option<(i64, Vec<String>, Cooldown)>,
}

fn merge(map: &mut BTreeMap<String, Cooldown>, keys: &[String], limit: &Cooldown) {
    for key in keys {
        if map.get(key).is_none_or(|old| old.retry_at < limit.retry_at) {
            map.insert(key.clone(), limit.clone());
        }
    }
}

impl Tracker {
    fn observe(&mut self, line: &str, scopes: &[String], now: i64) -> bool {
        let lower = line.to_ascii_lowercase();
        if lower.contains("url='http") || lower.trim_start().starts_with("http/") {
            if let Some((_, keys, limit)) = self.current.take() {
                merge(&mut self.completed, &keys, &limit);
            }
            let endpoint = self
                .recent
                .iter()
                .rev()
                .find(|l| l.to_ascii_lowercase().contains("url='http"))
                .cloned();
            self.recent.clear();
            if !lower.contains("url='http")
                && let Some(endpoint) = endpoint
            {
                self.recent.push_back(endpoint);
            }
        }
        self.recent.push_back(line.into());
        if self.recent.len() > 24 {
            self.recent.pop_front();
        }
        if limit_signal(line)
            || (parse_retry_after_seconds(line).is_some() && contains_ci(line, "will not retry"))
        {
            let start = self.current.as_ref().map_or(now, |e| e.0);
            let Some((mut limit, proof)) = detect_limit(self.recent.make_contiguous(), start)
            else {
                return false;
            };
            if let Some((_, _, prior)) = &self.current
                && !prior.estimated
                && limit.estimated
            {
                limit.retry_at = prior.retry_at;
                limit.estimated = false;
            }
            if let Some((_, prior_keys, prior)) = &self.current {
                if limit.source == "unknown" {
                    limit.source.clone_from(&prior.source);
                } else if prior.source != "unknown" && prior.source != limit.source {
                    merge(&mut self.completed, prior_keys, prior);
                }
            }
            let mut keys = limit_keys(scopes, &limit, &proof);
            if let Some((_, prior_keys, prior)) = &self.current
                && prior.source == limit.source
            {
                if !prior.estimated && !limit.estimated {
                    limit.retry_at = limit.retry_at.max(prior.retry_at);
                }
                keys.extend_from_slice(prior_keys);
                keys.sort();
                keys.dedup();
            }
            self.current = Some((start, keys, limit));
            return true;
        }
        if let Some((_, keys, limit)) = &mut self.current {
            // ACME problem documents often put "type" and "detail" on
            // separate lines. A later detail can identify an account or zone.
            let detail = quota_detail(line);
            if detail {
                keys.extend(limit_keys(scopes, limit, &lower));
                keys.sort();
                keys.dedup();
            }
            if let Some(until) = retry_time(line, now) {
                limit.retry_at = if limit.estimated {
                    until
                } else {
                    limit.retry_at.max(until)
                };
                limit.estimated = false;
                return true;
            }
            return detail;
        }
        false
    }

    fn snapshot(&self) -> BTreeMap<String, Cooldown> {
        let mut map = self.completed.clone();
        if let Some((_, keys, limit)) = &self.current {
            merge(&mut map, keys, limit);
        }
        map
    }
}
impl JobRates {
    pub(in crate::certificates::acme) fn new(
        state: &AppState,
        id: &str,
        app: &Value,
        ca: &str,
    ) -> Self {
        Self {
            state: state.clone(),
            id: id.into(),
            scopes: Arc::new(scope_keys(app, ca)),
            tracker: Arc::default(),
        }
    }
    pub(in crate::certificates::acme) async fn observe(&self, line: &str) -> anyhow::Result<()> {
        let mut tracker = self.tracker.lock().await;
        if tracker.observe(line, &self.scopes, time_utils::now_ms() / 1000) {
            let observations = tracker.snapshot();
            let deadline = observations.values().map(|v| v.retry_at).max().unwrap_or(0);
            let ttl = deadline
                .saturating_sub(time_utils::now_ms() / 1000)
                .max(ACME_JOB_TTL_SECONDS as i64);
            self.state
                .storage
                .store
                .set_acme_rate_evidence(
                    ACME_RUNTIME_LOCK_KEY,
                    PENDING_RATES,
                    &self.id,
                    &json!({"lockId":self.id,"observations":observations}),
                    usize::try_from(ttl).unwrap_or(usize::MAX),
                )
                .await?;
        }
        Ok(())
    }
}
// Only one native issuance owns the runtime lease. Expose its provisional
// decisions immediately, independently of the application and log lifetimes.
pub(super) const PENDING_RATES: &str = "fn_knock:acme:pending-rate-limits";
pub(in crate::certificates::acme) async fn apply_job_rates(
    state: &AppState,
    id: &str,
) -> anyhow::Result<bool> {
    let Some(raw) = state.storage.store.get_json_value(PENDING_RATES).await? else {
        return Ok(false);
    };
    if raw["lockId"] != id {
        return Ok(false);
    }
    let observations: BTreeMap<String, Cooldown> =
        serde_json::from_value(raw["observations"].clone())?;
    for (key, limit) in &observations {
        store_limit(state, std::slice::from_ref(key), limit).await?;
    }
    if let Some(max) = observations.values().max_by_key(|limit| limit.retry_at) {
        mark_limited_job(state, id, max).await?;
    }
    state
        .storage
        .store
        .delete_lock_if_owned(PENDING_RATES, id)
        .await?;
    Ok(!observations.is_empty())
}
// Called only while holding the runtime lease. Promote even if a user has
// already removed the original application.
pub(in crate::certificates::acme) async fn flush_interrupted_rates(
    state: &AppState,
) -> anyhow::Result<()> {
    if let Some(raw) = state.storage.store.get_json_value(PENDING_RATES).await?
        && let Some(id) = raw["lockId"].as_str()
    {
        apply_job_rates(state, id).await?;
    }
    Ok(())
}

pub(in crate::certificates::acme) async fn restore_interrupted_limit(
    state: &AppState,
    app: &Value,
    id: &str,
    baseline: &Value,
) -> anyhow::Result<()> {
    if state
        .storage
        .store
        .get_json_value(&format!("fn_knock:acme:limited-job:{id}"))
        .await?
        .is_some()
        || baseline["configSignature"] != preflight::config_signature(app)
    {
        return Ok(());
    }
    // Older jobs have no structured journal. Recover evidence before admitting
    // another request, using the issuing CA whenever the baseline identifies it.
    let Some(ca) = baseline["issuanceCa"].as_str() else {
        return Ok(());
    };
    let logs = get_acme_logs(state, id, MAX_ACME_LOG_LIMIT, "asc").await?;
    if let Some(limit) = record_cooldown_for_ca(state, app, ca, &logs).await? {
        mark_limited_job(state, id, &limit).await?
    }
    Ok(())
}
