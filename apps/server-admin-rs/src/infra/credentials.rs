use std::{
    fs,
    fs::OpenOptions,
    io::{Read, Write},
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

const VERSION: u8 = 1;
static KEY_LOCK: Mutex<()> = Mutex::new(());

#[derive(Serialize, Deserialize)]
struct Envelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}

#[derive(Clone)]
pub struct CredentialStore {
    dir: PathBuf,
    namespace: &'static str,
}

impl CredentialStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("panel-sync"),
            namespace: "panel-sync",
        }
    }
    pub(crate) fn with_directory(dir: PathBuf, namespace: &'static str) -> Self {
        Self { dir, namespace }
    }

    fn credential_path(&self, id: &str) -> Result<PathBuf, String> {
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|value| value.is_ascii_alphanumeric() || matches!(value, b'-' | b'_'))
        {
            return Err("面板连接 ID 无效".to_string());
        }
        Ok(self.dir.join("credentials").join(format!("{id}.enc")))
    }
    fn key_path(&self) -> PathBuf {
        self.dir.join("secret.key")
    }

    pub fn configured(&self, id: &str) -> bool {
        self.credential_path(id).is_ok_and(|path| path.is_file())
    }
    pub fn read(&self, id: &str) -> Result<Option<String>, String> {
        if !self.validate_layout()? {
            return Ok(None);
        }
        let credential_path = self.credential_path(id)?;
        let Some(bytes) = read_private_file(&credential_path, 4 * 1024 * 1024)? else {
            return Ok(None);
        };
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| "加密凭据格式无效".to_string())?;
        if envelope.version != VERSION {
            return Err("加密凭据版本不受支持".to_string());
        }
        let nonce = STANDARD
            .decode(envelope.nonce)
            .map_err(|_| "加密凭据随机数无效".to_string())?;
        let ciphertext = STANDARD
            .decode(envelope.ciphertext)
            .map_err(|_| "加密凭据载荷无效".to_string())?;
        if nonce.len() != 12 {
            return Err("加密凭据随机数长度无效".to_string());
        }
        let key = self.load_key()?;
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "安装密钥无效".to_string())?;
        let plaintext = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: format!("fn-knock:{}:{id}:v1", self.namespace).as_bytes(),
                },
            )
            .map_err(|_| "无法解密面板凭据".to_string())?;
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| "面板凭据编码无效".to_string())
    }
    pub fn write(&self, id: &str, value: &str) -> Result<(), String> {
        self.validate_layout()?;
        fs::create_dir_all(self.dir.join("credentials")).map_err(|error| error.to_string())?;
        self.validate_layout()?;
        secure_dir(&self.dir)?;
        secure_dir(&self.dir.join("credentials"))?;
        let key = self.load_or_create_key()?;
        let nonce = random_bytes::<12>();
        let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "安装密钥无效".to_string())?;
        let ciphertext = cipher
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: value.as_bytes(),
                    aad: format!("fn-knock:{}:{id}:v1", self.namespace).as_bytes(),
                },
            )
            .map_err(|_| "加密面板凭据失败".to_string())?;
        let bytes = serde_json::to_vec(&Envelope {
            version: VERSION,
            nonce: STANDARD.encode(nonce),
            ciphertext: STANDARD.encode(ciphertext),
        })
        .map_err(|error| error.to_string())?;
        atomic_private_write(&self.credential_path(id)?, &bytes)
    }
    pub fn delete(&self, id: &str) -> Result<(), String> {
        if !self.validate_layout()? {
            return Ok(());
        }
        match fs::remove_file(self.credential_path(id)?) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    pub fn clear_all(&self) -> Result<(), String> {
        match fs::remove_dir_all(&self.dir) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    }
    fn validate_layout(&self) -> Result<bool, String> {
        for path in [&self.dir, &self.dir.join("credentials")] {
            match fs::symlink_metadata(path) {
                Ok(metadata) if metadata.is_dir() && !is_link(&metadata) => {}
                Ok(_) => return Err("credential directory must be a regular directory".into()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(true)
    }
    fn load_or_create_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let path = self.key_path();
        if let Some(bytes) = read_private_file(&path, 32)? {
            return bytes.try_into().map_err(|_| "安装密钥长度无效".to_string());
        }
        for entry in
            fs::read_dir(self.dir.join("credentials")).map_err(|error| error.to_string())?
        {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "enc")
            {
                return Err("credential key is missing; refusing to replace it".into());
            }
        }
        let key = random_bytes::<32>();
        atomic_private_write(&path, &key)?;
        Ok(key)
    }
    fn load_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let path = self.key_path();
        let bytes =
            read_private_file(&path, 32)?.ok_or_else(|| "credential key is missing".to_string())?;
        bytes.try_into().map_err(|_| "安装密钥长度无效".to_string())
    }
}

fn atomic_private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "凭据路径无父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let temporary = parent.join(format!(
        ".credential.{}.{}.tmp",
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
        crate::fs_utils::replace_file(&temporary, path).map_err(|error| error.to_string())?;
        secure_file(path)?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| error.to_string())?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}

#[cfg(unix)]
fn secure_file(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| error.to_string())
}
#[cfg(all(not(unix), not(windows)))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}
#[cfg(unix)]
fn secure_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}
#[cfg(all(not(unix), not(windows)))]
fn secure_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn read_private_file(path: &Path, limit: u64) -> Result<Option<Vec<u8>>, String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !is_link(&metadata) => {}
        Ok(_) => return Err("credential must be a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path).map_err(|error| error.to_string())?;
    let metadata = file.metadata().map_err(|error| error.to_string())?;
    if !metadata.is_file() || is_link(&metadata) || metadata.len() > limit {
        return Err("credential file is invalid or too large".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(windows)]
    secure_file(path)?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("credential file is too large".into());
    }
    Ok(Some(bytes))
}
fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return metadata.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0;
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

#[cfg(windows)]
fn secure_file(path: &Path) -> Result<(), String> {
    crate::infra::private_permissions::secure_windows_path(path, false)
}
#[cfg(windows)]
fn secure_dir(path: &Path) -> Result<(), String> {
    crate::infra::private_permissions::secure_windows_path(path, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_key_is_not_regenerated_over_existing_credentials() {
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::with_directory(dir.path().join("email"), "backup-email");
        store.write("first", "sensitive").unwrap();
        fs::remove_file(store.key_path()).unwrap();
        assert!(store.write("second", "other").is_err());
        assert!(!store.key_path().exists());
    }

    #[cfg(unix)]
    #[test]
    fn credential_reads_reject_file_and_directory_symlinks() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let store = CredentialStore::with_directory(dir.path().join("email"), "backup-email");
        store.write("first", "sensitive").unwrap();
        let original = store.credential_path("first").unwrap();
        let displaced = dir.path().join("outside.enc");
        fs::rename(&original, &displaced).unwrap();
        symlink(&displaced, &original).unwrap();
        assert!(store.read("first").is_err());
        fs::remove_file(&original).unwrap();
        fs::rename(&displaced, &original).unwrap();
        let actual = dir.path().join("actual");
        fs::rename(&store.dir, &actual).unwrap();
        symlink(&actual, &store.dir).unwrap();
        assert!(store.read("first").is_err());
        assert!(store.write("second", "other").is_err());
        assert!(!actual.join("credentials/second.enc").exists());
    }
}
