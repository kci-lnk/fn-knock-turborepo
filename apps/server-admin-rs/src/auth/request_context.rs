//! Read-only inputs shared within one authorization request. Sessions and
//! authorization decisions are deliberately not cached: mutation paths still
//! reload authority before publishing mobility or whitelist state.
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    sync::{Arc, Mutex},
};

use serde_json::Value;
use tokio::sync::OnceCell;

use crate::{
    state::AppState,
    storage::StorageResult,
    store::{AuthAccount, Store, TotpCredential},
};

mod json_scan;

tokio::task_local! { static CURRENT: Arc<RequestContext>; }

struct RequestContext {
    config: Arc<Value>,
    accounts: OnceCell<CredentialSnapshot<AuthAccount>>,
    totps: OnceCell<CredentialSnapshot<TotpCredential>>,
}

/// Keep the authoritative bytes, not every normalized credential's object tree.
/// Later IDs use these same bytes even when another request changes storage.
struct CredentialSnapshot<T> {
    normalized_at: String,
    cache: Mutex<CredentialCache<T>>,
}

enum CredentialCache<T> {
    Selective {
        raw: Option<String>,
        first: Option<(String, Option<T>)>,
    },
    Indexed(HashMap<String, T>),
}

trait Credential: Clone {
    fn id(&self) -> &str;
}

impl Credential for AuthAccount {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Credential for TotpCredential {
    fn id(&self) -> &str {
        &self.id
    }
}

impl<T: Credential> CredentialSnapshot<T> {
    fn new(raw: Option<String>) -> Self {
        Self {
            normalized_at: crate::time_utils::now_iso(),
            cache: Mutex::new(CredentialCache::Selective { raw, first: None }),
        }
    }

    fn get(
        &self,
        id: &str,
        corrupted_is_empty: bool,
        normalize: impl Fn(Value, Option<&str>, &str) -> Option<T>,
    ) -> StorageResult<Option<T>> {
        let mut cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        let (raw, first) = match &mut *cache {
            CredentialCache::Indexed(by_id) => return Ok(by_id.get(id).cloned()),
            CredentialCache::Selective { raw, first } => (raw, first),
        };
        if let Some((first_id, value)) = first
            && first_id == id
        {
            return Ok(value.clone());
        }
        // A multi-owner request scans at most twice: normalize the complete
        // immutable snapshot on the second distinct ID, then release its bytes.
        if first.is_some() {
            let mut by_id = HashMap::new();
            if let Some(raw) = raw.as_deref() {
                let parsed = json_scan::array(raw, |value| {
                    if let Some(value) = normalize(value, None, &self.normalized_at) {
                        by_id.entry(value.id().to_string()).or_insert(value);
                    }
                    true
                });
                if let Err(error) = parsed {
                    if !corrupted_is_empty {
                        return Err(error.into());
                    }
                    by_id.clear();
                }
            }
            let found = by_id.get(id).cloned();
            *cache = CredentialCache::Indexed(by_id);
            return Ok(found);
        }
        let mut found = None;
        if let Some(raw) = raw.as_deref() {
            let parsed = json_scan::array(raw, |value| {
                found = normalize(value, Some(id), &self.normalized_at);
                found.is_none()
            });
            if let Err(error) = parsed {
                if !corrupted_is_empty {
                    return Err(error.into());
                }
                found = None;
            }
        }
        *first = Some((id.to_string(), found.clone()));
        Ok(found)
    }
}

pub(crate) fn scope<T>(
    state: &AppState,
    future: impl Future<Output = T>,
) -> impl Future<Output = T> {
    let context = CURRENT.try_with(Arc::clone).unwrap_or_else(|_| {
        Arc::new(RequestContext {
            config: state.storage.store.config_snapshot(),
            accounts: OnceCell::new(),
            totps: OnceCell::new(),
        })
    });
    // Auth route futures are large; box rather than adding another inline copy.
    CURRENT.scope(context, Box::pin(future))
}

pub(crate) fn config(state: &AppState) -> Arc<Value> {
    CURRENT
        .try_with(|context| context.config.clone())
        .unwrap_or_else(|_| state.storage.store.config_snapshot())
}

pub(crate) async fn account(state: &AppState, id: &str) -> StorageResult<Option<AuthAccount>> {
    let load = || async {
        state
            .storage
            .store
            .get_auth_accounts_raw_for_authorization()
            .await
            .map(CredentialSnapshot::new)
    };
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return load()
            .await?
            .get(id, false, Store::authorization_account_from_value_at);
    };
    context.accounts.get_or_try_init(load).await?.get(
        id,
        false,
        Store::authorization_account_from_value_at,
    )
}

pub(crate) async fn totp(state: &AppState, id: &str) -> StorageResult<Option<TotpCredential>> {
    let load = || async {
        state
            .storage
            .store
            .get_totps_raw_for_authorization()
            .await
            .map(CredentialSnapshot::new)
    };
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return load()
            .await?
            .get(id, true, Store::authorization_totp_from_value_at);
    };
    context.totps.get_or_try_init(load).await?.get(
        id,
        true,
        Store::authorization_totp_from_value_at,
    )
}

/// Passkey presentation needs membership only, never a cloned credential list.
/// Loading still performs the historical legacy migration even for no passkeys.
pub(crate) async fn matching_totp_ids(
    state: &AppState,
    requested: &HashSet<String>,
) -> StorageResult<HashSet<String>> {
    let load = || async {
        state
            .storage
            .store
            .get_totps_raw_for_authorization()
            .await
            .map(CredentialSnapshot::<TotpCredential>::new)
    };
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return Ok(load().await?.matching_totp_ids(requested));
    };
    Ok(context
        .totps
        .get_or_try_init(load)
        .await?
        .matching_totp_ids(requested))
}

impl CredentialSnapshot<TotpCredential> {
    fn matching_totp_ids(&self, requested: &HashSet<String>) -> HashSet<String> {
        let mut found = HashSet::new();
        if requested.is_empty() {
            return found;
        }
        let cache = self.cache.lock().unwrap_or_else(|error| error.into_inner());
        let raw = match &*cache {
            CredentialCache::Indexed(by_id) => {
                return requested
                    .iter()
                    .filter(|id| by_id.contains_key(*id))
                    .cloned()
                    .collect();
            }
            CredentialCache::Selective { raw, .. } => raw,
        };
        if let Some(raw) = raw.as_deref()
            && json_scan::array(raw, |value| {
                if let Some(id) = Store::authorization_totp_id(&value)
                    && requested.contains(&id)
                {
                    found.insert(id);
                }
                found.len() < requested.len()
            })
            .is_err()
        {
            found.clear();
        }
        found
    }
}

#[cfg(test)]
mod tests;
