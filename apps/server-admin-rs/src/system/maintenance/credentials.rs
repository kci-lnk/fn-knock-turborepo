use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use scrypt::{Params, scrypt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{crypto_utils::random_bytes, state::AppState, terminal, wol};

use super::{BackupImportError, KNOCK_BACKUP_PASSWORD};

const PROTECTED_CREDENTIALS_VERSION: u8 = 1;
const PROTECTED_CREDENTIALS_AAD: &[u8] = b"fn-knock:backup:credentials:v1";
const MAX_PROTECTED_CREDENTIALS_BYTES: usize = 32 * 1024 * 1024;
const CREDENTIAL_KDF_LOG_N: u8 = 12;
const CREDENTIAL_KDF_R: u32 = 8;
const CREDENTIAL_KDF_P: u32 = 1;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(super) struct CredentialBackupPayload {
    version: u8,
    terminal: terminal::TerminalCredentialBackup,
    wol: wol::WolCredentialBackup,
}

impl Default for CredentialBackupPayload {
    fn default() -> Self {
        Self {
            version: PROTECTED_CREDENTIALS_VERSION,
            terminal: terminal::TerminalCredentialBackup::default(),
            wol: wol::WolCredentialBackup::default(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProtectedCredentialEnvelope {
    version: u8,
    kdf_log_n: u8,
    kdf_r: u32,
    kdf_p: u32,
    salt: String,
    nonce: String,
    ciphertext: String,
}

pub(super) async fn export_protected_credentials(state: &AppState) -> anyhow::Result<Value> {
    let terminal = terminal::export_credentials_for_backup(state)
        .await
        .map_err(anyhow::Error::msg)?;
    let wol = {
        let _guard = state.wol.config_lock.lock().await;
        wol::export_credentials_for_backup(state)
            .await
            .map_err(anyhow::Error::msg)?
    };
    protect_credentials(&CredentialBackupPayload {
        version: PROTECTED_CREDENTIALS_VERSION,
        terminal,
        wol,
    })
    .map_err(anyhow::Error::msg)
}

pub(super) fn import_protected_credentials(
    value: Option<Value>,
) -> Result<CredentialBackupPayload, BackupImportError> {
    let Some(value) = value else {
        return Ok(CredentialBackupPayload::default());
    };
    unprotect_credentials(value).map_err(BackupImportError::bad_request)
}

pub(super) async fn snapshot_current_credentials(
    state: &AppState,
) -> Result<CredentialBackupPayload, BackupImportError> {
    let terminal = terminal::export_credentials_for_backup(state)
        .await
        .map_err(BackupImportError::internal)?;
    let wol = {
        let _guard = state.wol.config_lock.lock().await;
        wol::export_credentials_for_backup(state)
            .await
            .map_err(BackupImportError::internal)?
    };
    Ok(CredentialBackupPayload {
        version: PROTECTED_CREDENTIALS_VERSION,
        terminal,
        wol,
    })
}

pub(super) async fn restore_credential_snapshot(
    state: &AppState,
    backup: &CredentialBackupPayload,
) -> Result<(), String> {
    if backup.version != PROTECTED_CREDENTIALS_VERSION {
        return Err("backup credential payload version is unsupported".to_string());
    }
    terminal::restore_credentials_after_backup(state, &backup.terminal).await?;
    let _guard = state.wol.config_lock.lock().await;
    wol::restore_credentials_after_backup(state, &backup.wol).await
}

fn protect_credentials(payload: &CredentialBackupPayload) -> Result<Value, String> {
    let plaintext = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
    if plaintext.len() > MAX_PROTECTED_CREDENTIALS_BYTES {
        return Err("backup credentials are too large".to_string());
    }
    let salt = random_bytes::<16>();
    let nonce = random_bytes::<12>();
    let key = derive_backup_key(
        &salt,
        CREDENTIAL_KDF_LOG_N,
        CREDENTIAL_KDF_R,
        CREDENTIAL_KDF_P,
    )?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "backup credential encryption key is invalid".to_string())?;
    let ciphertext = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: PROTECTED_CREDENTIALS_AAD,
            },
        )
        .map_err(|_| "failed to encrypt backup credentials".to_string())?;
    serde_json::to_value(ProtectedCredentialEnvelope {
        version: PROTECTED_CREDENTIALS_VERSION,
        kdf_log_n: CREDENTIAL_KDF_LOG_N,
        kdf_r: CREDENTIAL_KDF_R,
        kdf_p: CREDENTIAL_KDF_P,
        salt: STANDARD.encode(salt),
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
    .map_err(|error| error.to_string())
}

fn unprotect_credentials(value: Value) -> Result<CredentialBackupPayload, String> {
    let envelope: ProtectedCredentialEnvelope =
        serde_json::from_value(value).map_err(|_| "backup credential envelope is invalid")?;
    if envelope.version != PROTECTED_CREDENTIALS_VERSION {
        return Err("backup credential envelope version is unsupported".to_string());
    }
    if !(10..=18).contains(&envelope.kdf_log_n)
        || !(1..=16).contains(&envelope.kdf_r)
        || !(1..=4).contains(&envelope.kdf_p)
    {
        return Err("backup credential KDF parameters are unsupported".to_string());
    }
    let salt = STANDARD
        .decode(envelope.salt)
        .map_err(|_| "backup credential salt is invalid".to_string())?;
    let nonce = STANDARD
        .decode(envelope.nonce)
        .map_err(|_| "backup credential nonce is invalid".to_string())?;
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|_| "backup credential ciphertext is invalid".to_string())?;
    if salt.len() != 16
        || nonce.len() != 12
        || ciphertext.is_empty()
        || ciphertext.len() > MAX_PROTECTED_CREDENTIALS_BYTES
    {
        return Err("backup credential envelope has invalid dimensions".to_string());
    }
    let key = derive_backup_key(&salt, envelope.kdf_log_n, envelope.kdf_r, envelope.kdf_p)?;
    let cipher = Aes256Gcm::new_from_slice(&key)
        .map_err(|_| "backup credential encryption key is invalid".to_string())?;
    let plaintext = cipher
        .decrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: PROTECTED_CREDENTIALS_AAD,
            },
        )
        .map_err(|_| "backup credentials cannot be decrypted".to_string())?;
    if plaintext.len() > MAX_PROTECTED_CREDENTIALS_BYTES {
        return Err("backup credential payload is too large".to_string());
    }
    let payload: CredentialBackupPayload = serde_json::from_slice(&plaintext)
        .map_err(|_| "backup credential payload is invalid".to_string())?;
    if payload.version != PROTECTED_CREDENTIALS_VERSION {
        return Err("backup credential payload version is unsupported".to_string());
    }
    Ok(payload)
}

fn derive_backup_key(salt: &[u8], log_n: u8, r: u32, p: u32) -> Result<[u8; 32], String> {
    let params = Params::new(log_n, r, p)
        .map_err(|_| "backup credential KDF parameters are invalid".to_string())?;
    let mut key = [0_u8; 32];
    scrypt(KNOCK_BACKUP_PASSWORD.as_bytes(), salt, &params, &mut key)
        .map_err(|_| "failed to derive backup credential encryption key".to_string())?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_credentials_are_authenticated_and_do_not_expose_plaintext() {
        let payload = CredentialBackupPayload {
            version: PROTECTED_CREDENTIALS_VERSION,
            terminal: serde_json::from_value(serde_json::json!({
                "credentials": [{
                    "targetId": "00000000-0000-4000-8000-000000000001",
                    "targetRevision": 1,
                    "authMethod": "password",
                    "password": STANDARD.encode(b"password-secret"),
                    "privateKey": null,
                    "passphrase": null
                }]
            }))
            .unwrap(),
            wol: wol::WolCredentialBackup::default(),
        };
        let mut envelope = protect_credentials(&payload).unwrap();
        let encoded = serde_json::to_string(&envelope).unwrap();
        assert!(!encoded.contains("password-secret"));
        assert_eq!(unprotect_credentials(envelope.clone()).unwrap(), payload);
        envelope["ciphertext"] = Value::String(STANDARD.encode(b"tampered"));
        assert!(unprotect_credentials(envelope).is_err());
    }
}
