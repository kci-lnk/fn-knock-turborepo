use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::Mutex,
};

use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, Payload},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};

use crate::crypto_utils::random_bytes;

const KEY_FILE: &str = "secret.key";
const API_TOKEN_FILE: &str = "cloudflare-api-token.enc";
const TUNNEL_TOKEN_FILE: &str = "tunnel-token.enc";
const ENVELOPE_VERSION: u8 = 1;
static KEY_CREATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy)]
pub(super) enum SecretKind {
    ApiToken,
    TunnelToken,
}

impl SecretKind {
    fn file_name(self) -> &'static str {
        match self {
            Self::ApiToken => API_TOKEN_FILE,
            Self::TunnelToken => TUNNEL_TOKEN_FILE,
        }
    }

    fn aad(self) -> &'static [u8] {
        match self {
            Self::ApiToken => b"fn-knock:cloudflared:api-token:v1",
            Self::TunnelToken => b"fn-knock:cloudflared:tunnel-token:v1",
        }
    }
}

#[derive(Serialize, Deserialize)]
struct SecretEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone)]
pub(super) struct CloudflaredSecretStore {
    dir: PathBuf,
}

impl CloudflaredSecretStore {
    pub(super) fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub(super) fn configured(&self, kind: SecretKind) -> bool {
        self.dir.join(kind.file_name()).is_file()
    }

    pub(super) fn read(&self, kind: SecretKind) -> Result<Option<String>, String> {
        let path = self.dir.join(kind.file_name());
        if path.exists() {
            secure_file(&path)?;
        }
        let raw = match fs::read(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("failed to read encrypted credential: {error}")),
        };
        let envelope = serde_json::from_slice::<SecretEnvelope>(&raw)
            .map_err(|error| format!("encrypted credential is invalid: {error}"))?;
        if envelope.version != ENVELOPE_VERSION {
            return Err("encrypted credential version is unsupported".to_string());
        }
        let nonce = STANDARD
            .decode(envelope.nonce)
            .map_err(|_| "encrypted credential nonce is invalid".to_string())?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|_| "encrypted credential payload is invalid".to_string())?;
        if nonce.len() != 12 {
            return Err("encrypted credential nonce length is invalid".to_string());
        }
        let key = self.read_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "cloudflared credential key is invalid".to_string())?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: kind.aad(),
                },
            )
            .map_err(|_| "encrypted credential cannot be decrypted".to_string())?;
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| "encrypted credential is not valid UTF-8".to_string())
    }

    pub(super) fn write(&self, kind: SecretKind, value: &str) -> Result<(), String> {
        self.ensure_dir()?;
        let key = self.load_or_create_key()?;
        let nonce = random_bytes::<12>();
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| "cloudflared credential key is invalid".to_string())?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: value.as_bytes(),
                    aad: kind.aad(),
                },
            )
            .map_err(|_| "failed to encrypt cloudflared credential".to_string())?;
        let envelope = SecretEnvelope {
            version: ENVELOPE_VERSION,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        };
        let bytes = serde_json::to_vec(&envelope)
            .map_err(|error| format!("failed to encode cloudflared credential: {error}"))?;
        atomic_private_write(&self.dir.join(kind.file_name()), &bytes)
    }

    pub(super) fn delete(&self, kind: SecretKind) -> Result<(), String> {
        match fs::remove_file(self.dir.join(kind.file_name())) {
            Ok(()) => sync_parent_directory(&self.dir),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("failed to remove cloudflared credential: {error}")),
        }
    }

    fn ensure_dir(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dir).map_err(|error| {
            format!("failed to create cloudflared credential directory: {error}")
        })?;
        secure_directory(&self.dir)
    }

    fn read_key(&self) -> Result<[u8; 32], String> {
        let path = self.dir.join(KEY_FILE);
        if path.exists() {
            secure_file(&path)?;
        }
        let raw = fs::read(&path)
            .map_err(|error| format!("failed to read cloudflared credential key: {error}"))?;
        raw.try_into()
            .map_err(|_| "cloudflared credential key has an invalid length".to_string())
    }

    fn load_or_create_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_CREATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        match self.read_key() {
            Ok(key) => {
                secure_file(&self.dir.join(KEY_FILE))?;
                Ok(key)
            }
            Err(_) if !self.dir.join(KEY_FILE).exists() => {
                let key = random_bytes::<32>();
                atomic_private_write(&self.dir.join(KEY_FILE), &key)?;
                Ok(key)
            }
            Err(error) => Err(error),
        }
    }
}

pub(super) fn atomic_private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "cloudflared credential path has no parent".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    secure_directory(parent)?;
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("credential");
    let temporary = parent.join(format!(
        ".{name}.{}.{}.tmp",
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
        sync_parent_directory(parent)
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
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Storage::FileSystem::{
        MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    };

    let source = temporary
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    // SAFETY: both buffers are valid, NUL-terminated UTF-16 paths and remain
    // alive for the duration of this same-volume atomic replacement call.
    let moved = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<(), String> {
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<(), String> {
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
    fn encrypts_credentials_and_binds_ciphertext_to_kind() {
        let directory = tempfile::tempdir().unwrap();
        let store = CloudflaredSecretStore::new(directory.path());
        store.write(SecretKind::ApiToken, "api-secret").unwrap();
        store
            .write(SecretKind::TunnelToken, "tunnel-secret")
            .unwrap();

        assert_eq!(
            store.read(SecretKind::ApiToken).unwrap().as_deref(),
            Some("api-secret")
        );
        assert_eq!(
            store.read(SecretKind::TunnelToken).unwrap().as_deref(),
            Some("tunnel-secret")
        );
        let encrypted = fs::read_to_string(directory.path().join(API_TOKEN_FILE)).unwrap();
        assert!(!encrypted.contains("api-secret"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(directory.path().join(API_TOKEN_FILE))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn delete_is_idempotent() {
        let directory = tempfile::tempdir().unwrap();
        let store = CloudflaredSecretStore::new(directory.path());
        store.delete(SecretKind::ApiToken).unwrap();
        store.write(SecretKind::ApiToken, "secret").unwrap();
        store.delete(SecretKind::ApiToken).unwrap();
        store.delete(SecretKind::ApiToken).unwrap();
        assert!(!store.configured(SecretKind::ApiToken));
    }

    #[cfg(unix)]
    #[test]
    fn read_repairs_credential_and_key_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let store = CloudflaredSecretStore::new(directory.path());
        store.write(SecretKind::ApiToken, "secret").unwrap();

        let credential_path = directory.path().join(API_TOKEN_FILE);
        let key_path = directory.path().join(KEY_FILE);
        fs::set_permissions(&credential_path, fs::Permissions::from_mode(0o700)).unwrap();
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o700)).unwrap();

        assert_eq!(
            store.read(SecretKind::ApiToken).unwrap().as_deref(),
            Some("secret")
        );
        for path in [credential_path, key_path] {
            let mode = fs::metadata(path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }
}
