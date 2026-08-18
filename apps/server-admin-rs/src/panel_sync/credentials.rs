use std::{
    fs,
    fs::OpenOptions,
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
}

impl CredentialStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("panel-sync"),
        }
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
        let credential_path = self.credential_path(id)?;
        let bytes = match fs::read(&credential_path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("读取加密凭据失败: {error}")),
        };
        secure_file(&credential_path)?;
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
                    aad: aad(id).as_bytes(),
                },
            )
            .map_err(|_| "无法解密面板凭据".to_string())?;
        String::from_utf8(plaintext)
            .map(Some)
            .map_err(|_| "面板凭据编码无效".to_string())
    }
    pub fn write(&self, id: &str, value: &str) -> Result<(), String> {
        fs::create_dir_all(self.dir.join("credentials")).map_err(|error| error.to_string())?;
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
                    aad: aad(id).as_bytes(),
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
    fn load_or_create_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let path = self.key_path();
        if let Ok(bytes) = fs::read(&path) {
            secure_file(&path)?;
            return bytes.try_into().map_err(|_| "安装密钥长度无效".to_string());
        }
        let key = random_bytes::<32>();
        atomic_private_write(&path, &key)?;
        Ok(key)
    }
    fn load_key(&self) -> Result<[u8; 32], String> {
        let _guard = KEY_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let path = self.key_path();
        let bytes = fs::read(&path).map_err(|error| format!("读取安装密钥失败: {error}"))?;
        secure_file(&path)?;
        bytes.try_into().map_err(|_| "安装密钥长度无效".to_string())
    }
}

fn aad(id: &str) -> String {
    format!("fn-knock:panel-sync:{id}:v1")
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
        secure_file(path)
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
#[cfg(not(unix))]
fn secure_file(_path: &Path) -> Result<(), String> {
    Ok(())
}
#[cfg(unix)]
fn secure_dir(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|error| error.to_string())
}
#[cfg(not(unix))]
fn secure_dir(_path: &Path) -> Result<(), String> {
    Ok(())
}
