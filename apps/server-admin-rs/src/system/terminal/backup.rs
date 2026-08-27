use std::collections::{HashMap, HashSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

use super::{
    domain::AuthMethod,
    repository::TargetRepository,
    runtime::MAX_TARGETS,
    secrets::{CredentialBundle, TerminalSecretStore},
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TerminalCredentialBackup {
    credentials: Vec<TerminalCredentialBackupEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct TerminalCredentialBackupEntry {
    target_id: String,
    target_revision: u64,
    auth_method: AuthMethod,
    password: Option<String>,
    private_key: Option<String>,
    passphrase: Option<String>,
}

pub(crate) async fn export_credentials_for_backup(
    state: &AppState,
) -> Result<TerminalCredentialBackup, String> {
    let targets = TargetRepository::new(state)
        .list()
        .await
        .map_err(|error| error.to_string())?;
    let store = TerminalSecretStore::from_state(state);
    let mut credentials = Vec::new();
    for target in targets {
        let bundle = store
            .read_bundle(&target.id)
            .map_err(|error| error.to_string())?;
        if bundle.is_empty() {
            continue;
        }
        if bundle.auth_method != Some(target.auth_method)
            || bundle.target_revision != target.revision
        {
            return Err(format!(
                "terminal credential metadata does not match target {}",
                target.id
            ));
        }
        credentials.push(TerminalCredentialBackupEntry {
            target_id: target.id,
            target_revision: target.revision,
            auth_method: target.auth_method,
            password: bundle.password.map(|value| STANDARD.encode(value)),
            private_key: bundle.private_key.map(|value| STANDARD.encode(value)),
            passphrase: bundle.passphrase.map(|value| STANDARD.encode(value)),
        });
    }
    Ok(TerminalCredentialBackup { credentials })
}

pub(crate) async fn restore_credentials_after_backup(
    state: &AppState,
    backup: &TerminalCredentialBackup,
) -> Result<(), String> {
    if backup.credentials.len() > MAX_TARGETS {
        return Err("terminal credential backup contains too many targets".to_string());
    }
    let targets = TargetRepository::new(state)
        .list()
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|target| (target.id.clone(), target))
        .collect::<HashMap<_, _>>();
    let mut seen = HashSet::new();
    let mut bundles = Vec::with_capacity(backup.credentials.len());
    for entry in &backup.credentials {
        let target = targets
            .get(&entry.target_id)
            .ok_or_else(|| "terminal credential references an unknown target".to_string())?;
        if !seen.insert(entry.target_id.as_str())
            || entry.target_revision != target.revision
            || entry.auth_method != target.auth_method
        {
            return Err("terminal credential metadata is inconsistent".to_string());
        }
        let decode = |value: &Option<String>| {
            value
                .as_deref()
                .map(|value| STANDARD.decode(value))
                .transpose()
                .map_err(|_| "terminal credential backup is invalid".to_string())
        };
        let bundle = CredentialBundle {
            auth_method: Some(entry.auth_method),
            target_revision: entry.target_revision,
            password: decode(&entry.password)?,
            private_key: decode(&entry.private_key)?,
            passphrase: decode(&entry.passphrase)?,
        };
        validate_bundle_shape(entry.auth_method, &bundle)?;
        bundles.push((entry.target_id.clone(), bundle));
    }

    let store = TerminalSecretStore::from_state(state);
    store.clear_all()?;
    for (target_id, bundle) in bundles {
        store
            .write_bundle(&target_id, &bundle)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn validate_bundle_shape(auth_method: AuthMethod, bundle: &CredentialBundle) -> Result<(), String> {
    if bundle
        .password
        .as_ref()
        .is_some_and(|value| value.len() > 64 * 1024)
        || bundle
            .private_key
            .as_ref()
            .is_some_and(|value| value.len() > 4 * 1024 * 1024)
        || bundle
            .passphrase
            .as_ref()
            .is_some_and(|value| value.len() > 64 * 1024)
    {
        return Err("terminal credential backup has an invalid size".to_string());
    }
    let valid = match auth_method {
        AuthMethod::Password => {
            bundle.password.is_some() && bundle.private_key.is_none() && bundle.passphrase.is_none()
        }
        AuthMethod::PrivateKey => bundle.password.is_none() && bundle.private_key.is_some(),
    };
    valid
        .then_some(())
        .ok_or_else(|| "terminal credential backup has an invalid shape".to_string())
}

#[cfg(test)]
pub(crate) fn write_backup_test_credential(
    state: &AppState,
    target_id: &str,
    auth_method: AuthMethod,
    target_revision: u64,
    password: Option<&[u8]>,
    private_key: Option<&[u8]>,
    passphrase: Option<&[u8]>,
) {
    TerminalSecretStore::from_state(state)
        .write_bundle(
            target_id,
            &CredentialBundle {
                auth_method: Some(auth_method),
                target_revision,
                password: password.map(<[u8]>::to_vec),
                private_key: private_key.map(<[u8]>::to_vec),
                passphrase: passphrase.map(<[u8]>::to_vec),
            },
        )
        .unwrap();
}

#[cfg(test)]
pub(crate) fn read_backup_test_credential(
    state: &AppState,
    target_id: &str,
) -> [Option<Vec<u8>>; 3] {
    let bundle = TerminalSecretStore::from_state(state)
        .read_bundle(target_id)
        .unwrap();
    [bundle.password, bundle.private_key, bundle.passphrase]
}
