use std::collections::{HashMap, HashSet};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

use super::{
    notify_runtime_reload,
    secrets::{
        self, IntegrationCredentialKind, SshCredentialKind, local_relay_secret_id, secret_store,
    },
    store,
};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WolCredentialBackup {
    credentials: Vec<WolCredentialBackupEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum WolCredentialBackupEntry {
    Relay {
        secret_id: String,
        key_version: u32,
        secret: String,
    },
    Blinker {
        target_id: String,
        secret: String,
    },
    Bemfa {
        target_id: String,
        secret: String,
    },
    SshPassword {
        target_id: String,
        secret: String,
    },
    SshPrivateKey {
        target_id: String,
        secret: String,
    },
    SshPrivateKeyPassphrase {
        target_id: String,
        secret: String,
    },
}

pub(crate) async fn export_credentials_for_backup(
    state: &AppState,
) -> Result<WolCredentialBackup, String> {
    let secret_store = secret_store(state);
    let mut credentials = Vec::new();
    let local = store::load_local_relay_config(state)
        .await
        .map_err(|error| error.to_string())?;
    if !local.relay_id.is_empty() {
        let secret_id = local_relay_secret_id(&local.relay_id);
        if let Some(secret) = secret_store.read(&secret_id, local.key_version)? {
            credentials.push(WolCredentialBackupEntry::Relay {
                secret_id,
                key_version: local.key_version,
                secret: STANDARD.encode(secret),
            });
        }
    }
    for relay in store::list_relays(state)
        .await
        .map_err(|error| error.to_string())?
    {
        if let Some(secret) = secret_store.read(&relay.id, relay.key_version)? {
            credentials.push(WolCredentialBackupEntry::Relay {
                secret_id: relay.id,
                key_version: relay.key_version,
                secret: STANDARD.encode(secret),
            });
        }
    }
    for target in store::list_targets(state)
        .await
        .map_err(|error| error.to_string())?
    {
        append_target_credentials(&secret_store, &target.id, &mut credentials)?;
    }
    Ok(WolCredentialBackup { credentials })
}

fn append_target_credentials(
    store: &secrets::WolSecretStore,
    target_id: &str,
    credentials: &mut Vec<WolCredentialBackupEntry>,
) -> Result<(), String> {
    if let Some(secret) = store.read_integration(target_id, IntegrationCredentialKind::Blinker)? {
        credentials.push(WolCredentialBackupEntry::Blinker {
            target_id: target_id.to_string(),
            secret: STANDARD.encode(secret),
        });
    }
    if let Some(secret) = store.read_integration(target_id, IntegrationCredentialKind::Bemfa)? {
        credentials.push(WolCredentialBackupEntry::Bemfa {
            target_id: target_id.to_string(),
            secret: STANDARD.encode(secret),
        });
    }
    if let Some(secret) = store.read_ssh(target_id, SshCredentialKind::Password)? {
        credentials.push(WolCredentialBackupEntry::SshPassword {
            target_id: target_id.to_string(),
            secret: STANDARD.encode(secret),
        });
    }
    if let Some(secret) = store.read_ssh(target_id, SshCredentialKind::PrivateKey)? {
        credentials.push(WolCredentialBackupEntry::SshPrivateKey {
            target_id: target_id.to_string(),
            secret: STANDARD.encode(secret),
        });
    }
    if let Some(secret) = store.read_ssh(target_id, SshCredentialKind::PrivateKeyPassphrase)? {
        credentials.push(WolCredentialBackupEntry::SshPrivateKeyPassphrase {
            target_id: target_id.to_string(),
            secret: STANDARD.encode(secret),
        });
    }
    Ok(())
}

pub(crate) async fn restore_credentials_after_backup(
    state: &AppState,
    backup: &WolCredentialBackup,
) -> Result<(), String> {
    if backup.credentials.len() > 4096 {
        return Err("WoL credential backup contains too many credentials".to_string());
    }
    let local = store::load_local_relay_config(state)
        .await
        .map_err(|error| error.to_string())?;
    let relays = store::list_relays(state)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|relay| (relay.id, relay.key_version))
        .collect::<HashMap<_, _>>();
    let targets = store::list_targets(state)
        .await
        .map_err(|error| error.to_string())?
        .into_iter()
        .map(|target| target.id)
        .collect::<HashSet<_>>();
    let local_secret_id =
        (!local.relay_id.is_empty()).then(|| local_relay_secret_id(&local.relay_id));
    let mut seen = HashSet::new();
    let mut decoded = Vec::with_capacity(backup.credentials.len());
    for entry in &backup.credentials {
        let identity = credential_identity(entry);
        if !seen.insert(identity) {
            return Err("WoL credential backup contains duplicates".to_string());
        }
        let (secret, valid) = match entry {
            WolCredentialBackupEntry::Relay {
                secret_id,
                key_version,
                secret,
            } => {
                let valid = (local_secret_id.as_deref() == Some(secret_id.as_str())
                    && *key_version == local.key_version)
                    || relays.get(secret_id) == Some(key_version);
                (secret, valid)
            }
            WolCredentialBackupEntry::Blinker { target_id, secret }
            | WolCredentialBackupEntry::Bemfa { target_id, secret }
            | WolCredentialBackupEntry::SshPassword { target_id, secret }
            | WolCredentialBackupEntry::SshPrivateKey { target_id, secret }
            | WolCredentialBackupEntry::SshPrivateKeyPassphrase { target_id, secret } => {
                (secret, targets.contains(target_id))
            }
        };
        if !valid {
            return Err("WoL credential references unknown metadata".to_string());
        }
        let secret = STANDARD
            .decode(secret)
            .map_err(|_| "WoL credential backup is invalid".to_string())?;
        if secret.is_empty() || secret.len() > 4 * 1024 * 1024 {
            return Err("WoL credential backup has an invalid size".to_string());
        }
        decoded.push((entry.clone(), secret));
    }

    let secret_store = secret_store(state);
    secret_store.clear_all()?;
    for (entry, secret) in decoded {
        restore_entry(&secret_store, entry, &secret)?;
    }
    state.wol.integration_status.write().await.clear();
    state.wol.relay_reload.notify_one();
    notify_runtime_reload(state);
    Ok(())
}

fn credential_identity(entry: &WolCredentialBackupEntry) -> String {
    match entry {
        WolCredentialBackupEntry::Relay {
            secret_id,
            key_version,
            ..
        } => format!("relay:{secret_id}:{key_version}"),
        WolCredentialBackupEntry::Blinker { target_id, .. } => format!("blinker:{target_id}"),
        WolCredentialBackupEntry::Bemfa { target_id, .. } => format!("bemfa:{target_id}"),
        WolCredentialBackupEntry::SshPassword { target_id, .. } => {
            format!("ssh-password:{target_id}")
        }
        WolCredentialBackupEntry::SshPrivateKey { target_id, .. } => {
            format!("ssh-private-key:{target_id}")
        }
        WolCredentialBackupEntry::SshPrivateKeyPassphrase { target_id, .. } => {
            format!("ssh-private-key-passphrase:{target_id}")
        }
    }
}

fn restore_entry(
    store: &secrets::WolSecretStore,
    entry: WolCredentialBackupEntry,
    secret: &[u8],
) -> Result<(), String> {
    match entry {
        WolCredentialBackupEntry::Relay {
            secret_id,
            key_version,
            ..
        } => store.write(&secret_id, key_version, secret),
        WolCredentialBackupEntry::Blinker { target_id, .. } => {
            store.write_integration(&target_id, IntegrationCredentialKind::Blinker, secret)
        }
        WolCredentialBackupEntry::Bemfa { target_id, .. } => {
            store.write_integration(&target_id, IntegrationCredentialKind::Bemfa, secret)
        }
        WolCredentialBackupEntry::SshPassword { target_id, .. } => {
            store.write_ssh(&target_id, SshCredentialKind::Password, secret)
        }
        WolCredentialBackupEntry::SshPrivateKey { target_id, .. } => {
            store.write_ssh(&target_id, SshCredentialKind::PrivateKey, secret)
        }
        WolCredentialBackupEntry::SshPrivateKeyPassphrase { target_id, .. } => {
            store.write_ssh(&target_id, SshCredentialKind::PrivateKeyPassphrase, secret)
        }
    }
}

#[cfg(test)]
pub(crate) fn write_backup_test_relay_secret(
    state: &AppState,
    secret_id: &str,
    key_version: u32,
    secret: &[u8],
) {
    secret_store(state)
        .write(secret_id, key_version, secret)
        .unwrap();
}

#[cfg(test)]
pub(crate) fn read_backup_test_relay_secret(
    state: &AppState,
    secret_id: &str,
    key_version: u32,
) -> Option<Vec<u8>> {
    secret_store(state).read(secret_id, key_version).unwrap()
}

#[cfg(test)]
pub(crate) fn write_backup_test_target_secrets(
    state: &AppState,
    target_id: &str,
    values: [&[u8]; 5],
) {
    let store = secret_store(state);
    store
        .write_integration(target_id, IntegrationCredentialKind::Blinker, values[0])
        .unwrap();
    store
        .write_integration(target_id, IntegrationCredentialKind::Bemfa, values[1])
        .unwrap();
    store
        .write_ssh(target_id, SshCredentialKind::Password, values[2])
        .unwrap();
    store
        .write_ssh(target_id, SshCredentialKind::PrivateKey, values[3])
        .unwrap();
    store
        .write_ssh(
            target_id,
            SshCredentialKind::PrivateKeyPassphrase,
            values[4],
        )
        .unwrap();
}

#[cfg(test)]
pub(crate) fn read_backup_test_target_secrets(
    state: &AppState,
    target_id: &str,
) -> [Option<Vec<u8>>; 5] {
    let store = secret_store(state);
    [
        store
            .read_integration(target_id, IntegrationCredentialKind::Blinker)
            .unwrap(),
        store
            .read_integration(target_id, IntegrationCredentialKind::Bemfa)
            .unwrap(),
        store
            .read_ssh(target_id, SshCredentialKind::Password)
            .unwrap(),
        store
            .read_ssh(target_id, SshCredentialKind::PrivateKey)
            .unwrap(),
        store
            .read_ssh(target_id, SshCredentialKind::PrivateKeyPassphrase)
            .unwrap(),
    ]
}
