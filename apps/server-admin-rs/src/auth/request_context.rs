//! Read-only inputs shared within one authorization request. Sessions and
//! authorization decisions are deliberately not cached: mutation paths still
//! reload authority before publishing mobility or whitelist state.
use std::{collections::HashMap, future::Future, sync::Arc};

use serde_json::Value;
use tokio::sync::OnceCell;

use crate::{
    state::AppState,
    storage::StorageResult,
    store::{AuthAccount, TotpCredential},
};

tokio::task_local! { static CURRENT: Arc<RequestContext>; }

struct RequestContext {
    config: Arc<Value>,
    accounts: OnceCell<CredentialIndex<AuthAccount>>,
    totps: OnceCell<CredentialIndex<TotpCredential>>,
}

struct CredentialIndex<T> {
    values: Vec<T>,
    by_id: HashMap<String, usize>,
}

impl<T> CredentialIndex<T> {
    fn get(&self, id: &str) -> Option<&T> {
        self.by_id.get(id).and_then(|index| self.values.get(*index))
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
    // Auth route futures are large. Box at the boundary instead of embedding
    // another copy in an async wrapper for each nested stage.
    CURRENT.scope(context, Box::pin(future))
}

pub(crate) fn config(state: &AppState) -> Arc<Value> {
    CURRENT
        .try_with(|context| context.config.clone())
        .unwrap_or_else(|_| state.storage.store.config_snapshot())
}

fn first_by_id<T>(values: Vec<T>, id: impl Fn(&T) -> &str) -> CredentialIndex<T> {
    let mut result = HashMap::with_capacity(values.len());
    for (index, value) in values.iter().enumerate() {
        result.entry(id(value).to_string()).or_insert(index);
    }
    CredentialIndex {
        values,
        by_id: result,
    }
}

pub(crate) async fn account(state: &AppState, id: &str) -> StorageResult<Option<AuthAccount>> {
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return Ok(state
            .storage
            .store
            .get_auth_accounts_for_authorization()
            .await?
            .into_iter()
            .find(|account| account.id == id));
    };
    let accounts = context
        .accounts
        .get_or_try_init(|| async {
            state
                .storage
                .store
                .get_auth_accounts_for_authorization()
                .await
                .map(|accounts| first_by_id(accounts, |account| &account.id))
        })
        .await?;
    Ok(accounts.get(id).cloned())
}

pub(crate) async fn totp(state: &AppState, id: &str) -> StorageResult<Option<TotpCredential>> {
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return Ok(state
            .storage
            .store
            .get_totps_for_authorization()
            .await?
            .into_iter()
            .find(|credential| credential.id == id));
    };
    let credentials = context
        .totps
        .get_or_try_init(|| async {
            state
                .storage
                .store
                .get_totps_for_authorization()
                .await
                .map(|credentials| first_by_id(credentials, |credential| &credential.id))
        })
        .await?;
    Ok(credentials.get(id).cloned())
}

pub(crate) async fn totps(state: &AppState) -> StorageResult<Vec<TotpCredential>> {
    let Ok(context) = CURRENT.try_with(Arc::clone) else {
        return state.storage.store.get_totps_for_authorization().await;
    };
    let credentials = context
        .totps
        .get_or_try_init(|| async {
            state
                .storage
                .store
                .get_totps_for_authorization()
                .await
                .map(|credentials| first_by_id(credentials, |credential| &credential.id))
        })
        .await?;
    Ok(credentials.values.clone())
}

#[cfg(test)]
mod tests;
