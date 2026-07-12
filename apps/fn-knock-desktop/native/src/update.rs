use std::{fs, path::PathBuf, process::Command, time::Duration};

use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use sha2::{Digest, Sha256};

const ENDPOINT: &str = "https://cor.fnknock.cn/latest.json";

#[derive(Clone, Debug, Deserialize)]
pub struct UpdatePackage {
    #[serde(rename = "download_url")]
    pub url: String,
    pub signature: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateOffer {
    pub version: String,
    #[serde(default, rename = "release_notes")]
    pub notes: String,
    #[serde(flatten)]
    pub package: UpdatePackage,
}

#[derive(Clone, Debug, Deserialize)]
struct SharedLatestManifest {
    packages: SharedPackages,
}

#[derive(Clone, Debug, Deserialize)]
struct SharedPackages {
    windows: std::collections::HashMap<String, UpdateOffer>,
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
    let mut offer = client
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
        .json::<SharedLatestManifest>()
        .map_err(|e| format!("更新清单无效：{e}"))?;
    let offer = offer
        .packages
        .windows
        .remove("x86_64")
        .ok_or("更新清单缺少 packages.windows.x86_64")?;
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
    let package = &offer.package;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_offer_from_shared_latest_document() {
        let manifest: SharedLatestManifest = serde_json::from_value(serde_json::json!({
            "version": "2.0.0",
            "packages": {
                "fpk": { "amd64": { "sha256": "core" } },
                "windows": {
                    "x86_64": {
                        "version": "2.0.2",
                        "download_url": "https://cdn.fnknock.cn/files/2.0.2/windows/x86_64/setup.exe",
                        "signature": "signature",
                        "sha256": "abc123",
                        "size": 42,
                        "release_notes": "Windows notes"
                    }
                }
            }
        }))
        .unwrap();
        let offer = manifest.packages.windows.get("x86_64").unwrap();
        assert_eq!(offer.version, "2.0.2");
        assert_eq!(offer.notes, "Windows notes");
        assert_eq!(
            offer.package.url,
            "https://cdn.fnknock.cn/files/2.0.2/windows/x86_64/setup.exe"
        );
    }
}
