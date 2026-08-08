use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use crate::{crypto_utils::random_bytes, state::AppState};

const KEY_FILE: &str = "secret.key";
const ENVELOPE_VERSION: u8 = 1;
const MAX_SECRET_ID_LENGTH: usize = 64;
static KEY_CREATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct SecretEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone)]
pub(super) struct WolSecretStore {
    dir: PathBuf,
}

pub(super) fn secret_store(state: &AppState) -> WolSecretStore {
    WolSecretStore::new(state.settings.data_dir.join("wol"))
}

pub(super) fn local_relay_secret_id(relay_id: &str) -> String {
    format!("local-{relay_id}")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IntegrationCredentialKind {
    Blinker,
    Bemfa,
}

impl IntegrationCredentialKind {
    fn name(self) -> &'static str {
        match self {
            Self::Blinker => "blinker",
            Self::Bemfa => "bemfa",
        }
    }
}

impl WolSecretStore {
    fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub(super) fn configured(&self, relay_id: &str) -> bool {
        validate_secret_id(relay_id).is_ok() && self.secret_path(relay_id).is_file()
    }

    pub(super) fn read(&self, relay_id: &str, key_version: u32) -> Result<Option<Vec<u8>>, String> {
        self.read_with_aad(relay_id, &secret_aad(relay_id, key_version))
    }

    pub(super) fn read_integration(
        &self,
        target_id: &str,
        kind: IntegrationCredentialKind,
    ) -> Result<Option<Vec<u8>>, String> {
        let id = integration_secret_id(target_id, kind);
        self.read_with_aad(&id, &integration_secret_aad(target_id, kind))
    }

    fn read_with_aad(&self, id: &str, aad: &[u8]) -> Result<Option<Vec<u8>>, String> {
        validate_secret_id(id)?;
        let path = self.secret_path(id);
        if path.exists() {
            secure_file(&path)?;
        }
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("failed to read encrypted WoL PSK: {error}")),
        };
        let envelope = serde_json::from_slice::<SecretEnvelope>(&raw)
            .map_err(|error| format!("encrypted WoL PSK is invalid: {error}"))?;
        if envelope.version != ENVELOPE_VERSION {
            return Err("encrypted WoL PSK version is unsupported".to_string());
        }
        let nonce = STANDARD
            .decode(envelope.nonce)
            .map_err(|_| "encrypted WoL PSK nonce is invalid".to_string())?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|_| "encrypted WoL PSK payload is invalid".to_string())?;
        if nonce.len() != 12 {
            return Err("encrypted WoL PSK nonce length is invalid".to_string());
        }
        let key = self.read_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "WoL credential key is invalid".to_string())?;
        cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad,
                },
            )
            .map(Some)
            .map_err(|_| "encrypted WoL PSK cannot be decrypted".to_string())
    }

    pub(super) fn write(
        &self,
        relay_id: &str,
        key_version: u32,
        value: &[u8],
    ) -> Result<(), String> {
        self.write_with_aad(relay_id, value, &secret_aad(relay_id, key_version))
    }

    pub(super) fn write_integration(
        &self,
        target_id: &str,
        kind: IntegrationCredentialKind,
        value: &[u8],
    ) -> Result<(), String> {
        let id = integration_secret_id(target_id, kind);
        self.write_with_aad(&id, value, &integration_secret_aad(target_id, kind))
    }

    fn write_with_aad(&self, id: &str, value: &[u8], aad: &[u8]) -> Result<(), String> {
        validate_secret_id(id)?;
        self.ensure_layout()?;
        let key = self.load_or_create_key()?;
        let nonce = random_bytes::<12>();
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "WoL credential key is invalid".to_string())?;
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce), Payload { msg: value, aad })
            .map_err(|_| "failed to encrypt WoL PSK".to_string())?;
        let bytes = serde_json::to_vec(&SecretEnvelope {
            version: ENVELOPE_VERSION,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        })
        .map_err(|error| format!("failed to encode WoL PSK: {error}"))?;
        atomic_private_write(&self.secret_path(id), &bytes)
    }

    pub(super) fn integration_configured(
        &self,
        target_id: &str,
        kind: IntegrationCredentialKind,
    ) -> bool {
        self.configured(&integration_secret_id(target_id, kind))
    }

    pub(super) fn delete_integration(
        &self,
        target_id: &str,
        kind: IntegrationCredentialKind,
    ) -> Result<(), String> {
        self.delete(&integration_secret_id(target_id, kind))
    }

    pub(super) fn delete(&self, relay_id: &str) -> Result<(), String> {
        validate_secret_id(relay_id)?;
        match fs::remove_file(self.secret_path(relay_id)) {
            Ok(()) => sync_directory(&self.secrets_dir()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("failed to remove WoL PSK: {error}")),
        }
    }

    pub(super) fn clear_all(&self) -> Result<(), String> {
        let secrets = self.secrets_dir();
        match fs::remove_dir_all(&secrets) {
            Ok(()) => {
                fs::create_dir_all(&secrets).map_err(|error| error.to_string())?;
                secure_directory(&secrets)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("failed to clear WoL PSKs: {error}")),
        }
    }

    fn ensure_layout(&self) -> Result<(), String> {
        fs::create_dir_all(self.secrets_dir()).map_err(|error| error.to_string())?;
        secure_directory(&self.dir)?;
        secure_directory(&self.secrets_dir())
    }

    fn secrets_dir(&self) -> PathBuf {
        self.dir.join("secrets")
    }

    fn secret_path(&self, relay_id: &str) -> PathBuf {
        self.secrets_dir().join(format!("{relay_id}.enc"))
    }

    fn read_key(&self) -> Result<[u8; 32], String> {
        let path = self.dir.join(KEY_FILE);
        if path.exists() {
            secure_file(&path)?;
        }
        fs::read(&path)
            .map_err(|error| format!("failed to read WoL credential key: {error}"))?
            .try_into()
            .map_err(|_| "WoL credential key has an invalid length".to_string())
    }

    fn load_or_create_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_CREATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.read_key() {
            Ok(key) => Ok(key),
            Err(_) if !self.dir.join(KEY_FILE).exists() => {
                let key = random_bytes::<32>();
                atomic_private_write(&self.dir.join(KEY_FILE), &key)?;
                Ok(key)
            }
            Err(error) => Err(error),
        }
    }
}

fn validate_secret_id(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > MAX_SECRET_ID_LENGTH
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err("WoL credential identifier is invalid".to_string());
    }
    Ok(())
}

fn secret_aad(relay_id: &str, key_version: u32) -> Vec<u8> {
    format!("fn-knock:wol:relay:{relay_id}:key:{key_version}:v1").into_bytes()
}

fn integration_secret_id(target_id: &str, kind: IntegrationCredentialKind) -> String {
    format!("{}-{target_id}", kind.name())
}

fn integration_secret_aad(target_id: &str, kind: IntegrationCredentialKind) -> Vec<u8> {
    format!(
        "fn-knock:wol:integration:{}:target:{target_id}:credential:v1",
        kind.name()
    )
    .into_bytes()
}

fn atomic_private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid WoL secret path".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    secure_directory(parent)?;
    let temporary = parent.join(format!(
        ".wol-secret.{}.{}.tmp",
        std::process::id(),
        hex::encode(random_bytes::<8>())
    ));
    let result = (|| {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| error.to_string())?;
        file.write_all(bytes).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        replace_file(&temporary, path)?;
        secure_file(path)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(temporary: &Path, path: &Path) -> Result<(), String> {
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn replace_file(temporary: &Path, path: &Path) -> Result<(), String> {
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    fs::rename(temporary, path).map_err(|error| error.to_string())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_directory(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn secure_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypts_and_binds_psk_to_relay_and_version() {
        let directory = tempfile::tempdir().unwrap();
        let store = WolSecretStore::new(directory.path());
        let psk = [7_u8; 32];
        store.write("relay-a", 1, &psk).unwrap();
        assert_eq!(store.read("relay-a", 1).unwrap().unwrap(), psk);
        assert!(store.read("relay-a", 2).is_err());
        assert!(store.read("relay-b", 1).unwrap().is_none());
        let raw = fs::read_to_string(store.secret_path("relay-a")).unwrap();
        assert!(!raw.contains(&STANDARD.encode(psk)));
        store.clear_all().unwrap();
        assert!(!store.configured("relay-a"));
        assert!(store.read("relay-a", 1).unwrap().is_none());
    }

    #[test]
    fn integration_credentials_use_independent_aad_domains() {
        let directory = tempfile::tempdir().unwrap();
        let store = WolSecretStore::new(directory.path());
        store
            .write_integration(
                "target-a",
                IntegrationCredentialKind::Blinker,
                b"device-key",
            )
            .unwrap();
        store
            .write_integration("target-a", IntegrationCredentialKind::Bemfa, b"private-key")
            .unwrap();
        assert_eq!(
            store
                .read_integration("target-a", IntegrationCredentialKind::Blinker)
                .unwrap()
                .unwrap(),
            b"device-key"
        );
        assert_eq!(
            store
                .read_integration("target-a", IntegrationCredentialKind::Bemfa)
                .unwrap()
                .unwrap(),
            b"private-key"
        );
        assert_ne!(
            fs::read(store.secret_path("blinker-target-a")).unwrap(),
            fs::read(store.secret_path("bemfa-target-a")).unwrap()
        );
    }

    #[test]
    fn rejects_secret_path_traversal_identifiers() {
        let directory = tempfile::tempdir().unwrap();
        let store = WolSecretStore::new(directory.path().join("wol"));
        for invalid in ["", "../escape", "relay/escape", "relay\\escape"] {
            assert!(store.write(invalid, 1, &[7; 32]).is_err());
            assert!(store.read(invalid, 1).is_err());
            assert!(store.delete(invalid).is_err());
            assert!(!store.configured(invalid));
        }
        assert!(!directory.path().join("escape.enc").exists());
    }
}
