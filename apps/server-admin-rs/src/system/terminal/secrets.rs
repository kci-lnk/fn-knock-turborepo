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

use crate::{crypto_utils::random_bytes, fs_utils, state::AppState};

use super::domain::{AuthMethod, TerminalError, TerminalResult};

const ENVELOPE_VERSION: u8 = 1;
const BUNDLE_VERSION: u8 = 1;
const KEY_FILE: &str = "secret.key";
const MAX_ENVELOPE_BYTES: u64 = 4 * 1024 * 1024;
static SECRET_OPERATION_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CredentialKind {
    Password,
    PrivateKey,
    #[cfg(test)]
    Passphrase,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct CredentialBundle {
    pub auth_method: Option<AuthMethod>,
    pub target_revision: u64,
    pub password: Option<Vec<u8>>,
    pub private_key: Option<Vec<u8>>,
    pub passphrase: Option<Vec<u8>>,
}

impl CredentialBundle {
    #[cfg(test)]
    fn get(&self, kind: CredentialKind) -> Option<&[u8]> {
        match kind {
            CredentialKind::Password => self.password.as_deref(),
            CredentialKind::PrivateKey => self.private_key.as_deref(),
            #[cfg(test)]
            CredentialKind::Passphrase => self.passphrase.as_deref(),
        }
    }

    #[cfg(test)]
    fn set(&mut self, kind: CredentialKind, value: Option<Vec<u8>>) {
        match kind {
            CredentialKind::Password => self.password = value,
            CredentialKind::PrivateKey => self.private_key = value,
            #[cfg(test)]
            CredentialKind::Passphrase => self.passphrase = value,
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.password.is_none() && self.private_key.is_none() && self.passphrase.is_none()
    }
}

#[derive(Serialize, Deserialize)]
struct SecretEnvelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EncodedCredentialBundle {
    version: u8,
    auth_method: Option<AuthMethod>,
    target_revision: u64,
    password: Option<String>,
    private_key: Option<String>,
    passphrase: Option<String>,
}

impl From<&CredentialBundle> for EncodedCredentialBundle {
    fn from(bundle: &CredentialBundle) -> Self {
        Self {
            version: BUNDLE_VERSION,
            auth_method: bundle.auth_method,
            target_revision: bundle.target_revision,
            password: bundle.password.as_ref().map(|value| STANDARD.encode(value)),
            private_key: bundle
                .private_key
                .as_ref()
                .map(|value| STANDARD.encode(value)),
            passphrase: bundle
                .passphrase
                .as_ref()
                .map(|value| STANDARD.encode(value)),
        }
    }
}

impl TryFrom<EncodedCredentialBundle> for CredentialBundle {
    type Error = TerminalError;

    fn try_from(bundle: EncodedCredentialBundle) -> Result<Self, Self::Error> {
        if bundle.version != BUNDLE_VERSION {
            return Err(TerminalError::internal(
                "terminal credential bundle version is unsupported",
            ));
        }
        let decode = |value: Option<String>| {
            value
                .map(|value| STANDARD.decode(value))
                .transpose()
                .map_err(|_| TerminalError::internal("terminal credential bundle is invalid"))
        };
        Ok(Self {
            auth_method: bundle.auth_method,
            target_revision: bundle.target_revision,
            password: decode(bundle.password)?,
            private_key: decode(bundle.private_key)?,
            passphrase: decode(bundle.passphrase)?,
        })
    }
}

#[derive(Clone)]
pub struct TerminalSecretStore {
    dir: PathBuf,
}

impl TerminalSecretStore {
    pub fn from_state(state: &AppState) -> Self {
        Self::new(state.settings.data_dir.join("terminal"))
    }

    pub(crate) fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    #[cfg(test)]
    pub fn read(&self, target_id: &str, kind: CredentialKind) -> TerminalResult<Option<Vec<u8>>> {
        Ok(self.read_bundle(target_id)?.get(kind).map(<[u8]>::to_vec))
    }

    #[cfg(test)]
    pub fn write(&self, target_id: &str, kind: CredentialKind, value: &[u8]) -> TerminalResult<()> {
        if value.is_empty() {
            return Err(TerminalError::invalid(
                "terminal credential cannot be empty",
            ));
        }
        self.update_bundle(target_id, |bundle| {
            bundle.set(kind, Some(value.to_vec()));
            Ok(())
        })
    }

    #[cfg(test)]
    pub fn delete(&self, target_id: &str, kind: CredentialKind) -> TerminalResult<()> {
        self.update_bundle(target_id, |bundle| {
            bundle.set(kind, None);
            Ok(())
        })
    }

    pub(super) fn read_bundle(&self, target_id: &str) -> TerminalResult<CredentialBundle> {
        validate_target_id(target_id)?;
        let _guard = SECRET_OPERATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.read_bundle_locked(target_id)
    }

    pub(super) fn write_bundle(
        &self,
        target_id: &str,
        bundle: &CredentialBundle,
    ) -> TerminalResult<()> {
        validate_target_id(target_id)?;
        let _guard = SECRET_OPERATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.write_bundle_locked(target_id, bundle)
    }

    pub(super) fn update_bundle<F>(&self, target_id: &str, update: F) -> TerminalResult<()>
    where
        F: FnOnce(&mut CredentialBundle) -> TerminalResult<()>,
    {
        validate_target_id(target_id)?;
        let _guard = SECRET_OPERATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut bundle = self.read_bundle_locked(target_id)?;
        update(&mut bundle)?;
        self.write_bundle_locked(target_id, &bundle)
    }

    pub fn delete_target(&self, target_id: &str) -> TerminalResult<()> {
        validate_target_id(target_id)?;
        let _guard = SECRET_OPERATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.delete_bundle_locked(target_id)
    }

    pub fn clear_all(&self) -> Result<(), String> {
        let _guard = SECRET_OPERATION_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let path = self.secrets_dir();
        if self.dir.exists() {
            ensure_regular_directory(&self.dir)?;
        }
        if path.exists() {
            ensure_regular_directory(&path)?;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => {
                fs::create_dir_all(&path).map_err(|error| error.to_string())?;
                secure_directory(&self.dir)?;
                secure_directory(&path)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn read_bundle_locked(&self, target_id: &str) -> TerminalResult<CredentialBundle> {
        if !self
            .validate_existing_layout()
            .map_err(TerminalError::internal)?
        {
            return Ok(CredentialBundle::default());
        }
        let path = self.bundle_path(target_id);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(CredentialBundle::default());
            }
            Err(error) => return Err(TerminalError::internal(error.to_string())),
        };
        if metadata.len() > MAX_ENVELOPE_BYTES {
            return Err(TerminalError::internal(
                "terminal credential envelope exceeds the size limit",
            ));
        }
        ensure_regular_file(&path, &metadata).map_err(TerminalError::internal)?;
        let raw = fs::read(&path).map_err(|error| TerminalError::internal(error.to_string()))?;
        let envelope: SecretEnvelope = serde_json::from_slice(&raw)
            .map_err(|_| TerminalError::internal("terminal credential envelope is invalid"))?;
        if envelope.version != ENVELOPE_VERSION {
            return Err(TerminalError::internal(
                "terminal credential envelope version is unsupported",
            ));
        }
        let nonce = STANDARD
            .decode(envelope.nonce)
            .map_err(|_| TerminalError::internal("terminal credential nonce is invalid"))?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|_| TerminalError::internal("terminal credential payload is invalid"))?;
        if nonce.len() != 12 {
            return Err(TerminalError::internal(
                "terminal credential nonce length is invalid",
            ));
        }
        let key = self.read_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| TerminalError::internal("terminal credential key is invalid"))?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: &bundle_aad(target_id),
                },
            )
            .map_err(|_| TerminalError::internal("terminal credential cannot be decrypted"))?;
        let bundle: EncodedCredentialBundle = serde_json::from_slice(&plaintext)
            .map_err(|_| TerminalError::internal("terminal credential bundle is invalid"))?;
        bundle.try_into()
    }

    fn write_bundle_locked(
        &self,
        target_id: &str,
        bundle: &CredentialBundle,
    ) -> TerminalResult<()> {
        if bundle.is_empty() {
            return self.delete_bundle_locked(target_id);
        }
        self.ensure_layout().map_err(TerminalError::internal)?;
        let plaintext = serde_json::to_vec(&EncodedCredentialBundle::from(bundle))
            .map_err(|error| TerminalError::internal(error.to_string()))?;
        let key = self.load_or_create_key()?;
        let nonce = random_bytes::<12>();
        let cipher = Aes256Gcm::new_from_slice(&key)
            .map_err(|_| TerminalError::internal("terminal credential key is invalid"))?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &plaintext,
                    aad: &bundle_aad(target_id),
                },
            )
            .map_err(|_| TerminalError::internal("failed to encrypt terminal credential"))?;
        let encoded = serde_json::to_vec(&SecretEnvelope {
            version: ENVELOPE_VERSION,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        })
        .map_err(|error| TerminalError::internal(error.to_string()))?;
        atomic_private_write(&self.bundle_path(target_id), &encoded)
            .map_err(TerminalError::internal)
    }

    fn delete_bundle_locked(&self, target_id: &str) -> TerminalResult<()> {
        match fs::remove_file(self.bundle_path(target_id)) {
            Ok(()) => sync_directory(&self.secrets_dir()).map_err(TerminalError::internal),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(TerminalError::internal(error.to_string())),
        }
    }

    fn ensure_layout(&self) -> Result<(), String> {
        fs::create_dir_all(self.secrets_dir()).map_err(|error| error.to_string())?;
        ensure_regular_directory(&self.dir)?;
        ensure_regular_directory(&self.secrets_dir())?;
        secure_directory(&self.dir)?;
        secure_directory(&self.secrets_dir())
    }

    fn validate_existing_layout(&self) -> Result<bool, String> {
        match fs::symlink_metadata(&self.dir) {
            Ok(_) => ensure_regular_directory(&self.dir)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        }
        match fs::symlink_metadata(self.secrets_dir()) {
            Ok(_) => ensure_regular_directory(&self.secrets_dir())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error.to_string()),
        }
        Ok(true)
    }

    fn secrets_dir(&self) -> PathBuf {
        self.dir.join("secrets")
    }

    fn bundle_path(&self, target_id: &str) -> PathBuf {
        self.secrets_dir().join(format!("target-{target_id}.enc"))
    }

    fn read_key(&self) -> TerminalResult<[u8; 32]> {
        let path = self.dir.join(KEY_FILE);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| TerminalError::internal(error.to_string()))?;
        ensure_regular_file(&path, &metadata).map_err(TerminalError::internal)?;
        fs::read(path)
            .map_err(|error| TerminalError::internal(error.to_string()))?
            .try_into()
            .map_err(|_| TerminalError::internal("terminal credential key has invalid length"))
    }

    fn load_or_create_key(&self) -> TerminalResult<[u8; 32]> {
        match self.read_key() {
            Ok(key) => Ok(key),
            Err(_) if !self.dir.join(KEY_FILE).exists() => {
                let key = random_bytes::<32>();
                atomic_private_write(&self.dir.join(KEY_FILE), &key)
                    .map_err(TerminalError::internal)?;
                Ok(key)
            }
            Err(error) => Err(error),
        }
    }
}

fn validate_target_id(value: &str) -> TerminalResult<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(TerminalError::invalid(
            "terminal credential target identifier is invalid",
        ));
    }
    Ok(())
}

fn bundle_aad(target_id: &str) -> Vec<u8> {
    format!("fn-knock:terminal:target:{target_id}:credential-bundle:v1").into_bytes()
}

fn atomic_private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "invalid terminal secret path".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    ensure_regular_directory(parent)?;
    secure_directory(parent)?;
    let temporary = parent.join(format!(
        ".terminal-secret.{}.{}.tmp",
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
        secure_file(&temporary)?;
        fs_utils::replace_file(&temporary, path).map_err(|error| error.to_string())?;
        secure_file(path)?;
        sync_directory(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

fn ensure_regular_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !metadata.file_type().is_dir() || metadata_is_reparse_point(&metadata) {
        return Err(format!(
            "terminal secret directory is not a regular directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn ensure_regular_file(path: &Path, metadata: &fs::Metadata) -> Result<(), String> {
    if !metadata.file_type().is_file() || metadata_is_reparse_point(metadata) {
        return Err(format!(
            "terminal secret path is not a regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(windows)]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;

    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
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

#[cfg(windows)]
fn secure_directory(path: &Path) -> Result<(), String> {
    secure_windows_path(path, true)
}

#[cfg(all(not(unix), not(windows)))]
fn secure_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn secure_file(path: &Path) -> Result<(), String> {
    secure_windows_path(path, false)
}

#[cfg(all(not(unix), not(windows)))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn secure_windows_path(path: &Path, directory: bool) -> Result<(), String> {
    use std::process::{Command, Stdio};

    let system_root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    let icacls = PathBuf::from(system_root)
        .join("System32")
        .join("icacls.exe");
    if !icacls.is_file() {
        return Err(format!(
            "required Windows ACL tool is missing: {}",
            icacls.display()
        ));
    }
    let grants: &[&str] = if directory {
        &[
            "*S-1-5-18:F",
            "*S-1-5-18:(OI)(CI)F",
            "*S-1-5-32-544:F",
            "*S-1-5-32-544:(OI)(CI)F",
            r"NT SERVICE\FnKnock:M",
            r"NT SERVICE\FnKnock:(OI)(CI)M",
        ]
    } else {
        &["*S-1-5-18:F", "*S-1-5-32-544:F", r"NT SERVICE\FnKnock:M"]
    };
    let status = Command::new(icacls)
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .args(grants)
        .args(["/L", "/Q"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("icacls.exe failed with {status}"));
    }
    Ok(())
}
