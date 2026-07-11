use std::{fs, path::PathBuf, process::Command, time::Duration};

use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const ENDPOINT: &str = "https://cdn.fnknock.cn/windows/stable/latest.json";

#[derive(Clone, Debug, Deserialize)]
pub struct UpdatePackage {
    pub url: String,
    pub signature: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateOffer {
    pub version: String,
    pub notes: String,
    platforms: std::collections::HashMap<String, UpdatePackage>,
}

fn version_tuple(value: &str) -> Vec<u64> {
    value
        .trim_start_matches('v')
        .split('.')
        .map(|part| part.parse().unwrap_or(0))
        .collect()
}

pub fn check() -> Result<Option<UpdateOffer>, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let offer = client
        .get(format!(
            "{ENDPOINT}?t={}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ))
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|e| format!("检查更新失败：{e}"))?
        .error_for_status()
        .map_err(|e| format!("检查更新失败：{e}"))?
        .json::<UpdateOffer>()
        .map_err(|e| format!("更新清单无效：{e}"))?;
    if version_tuple(&offer.version) > version_tuple(env!("CARGO_PKG_VERSION")) {
        Ok(Some(offer))
    } else {
        Ok(None)
    }
}

fn public_key() -> Result<PublicKey, String> {
    let raw = option_env!("FN_KNOCK_UPDATER_PUBLIC_KEY")
        .filter(|value| !value.trim().is_empty())
        .ok_or("此构建未嵌入 Windows 更新公钥")?;
    PublicKey::decode(raw)
        .or_else(|_| PublicKey::from_base64(raw))
        .map_err(|e| e.to_string())
}

pub fn install(offer: &UpdateOffer) -> Result<(), String> {
    let package = offer
        .platforms
        .get("windows-x86_64")
        .ok_or("更新清单缺少 windows-x86_64")?;
    if !package.url.starts_with("https://cdn.fnknock.cn/") {
        return Err("更新下载地址不受信任".to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let bytes = client
        .get(&package.url)
        .send()
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != package.size {
        return Err("更新安装包大小不匹配".to_string());
    }
    let digest = hex::encode(Sha256::digest(&bytes));
    if !digest.eq_ignore_ascii_case(&package.sha256) {
        return Err("更新安装包 SHA-256 不匹配".to_string());
    }
    let signature =
        Signature::decode(&package.signature).map_err(|e| format!("更新签名无效：{e}"))?;
    public_key()?
        .verify(&bytes, &signature, false)
        .map_err(|e| format!("更新签名校验失败：{e}"))?;
    let directory = crate::platform::program_data_dir()?.join("updates");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let path: PathBuf = directory.join(format!("fn-knock-{}-setup.exe", offer.version));
    let temp = path.with_extension("exe.tmp");
    fs::write(&temp, &bytes).map_err(|e| e.to_string())?;
    fs::rename(&temp, &path).map_err(|e| e.to_string())?;
    Command::new(path)
        .arg("/passive")
        .spawn()
        .map_err(|e| format!("启动更新安装器失败：{e}"))?;
    Ok(())
}
