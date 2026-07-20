use std::{
    fs,
    io::{Cursor, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::i18n;

const ENDPOINT: &str = "https://cor.fnknock.cn/latest.json";

#[derive(Clone, Debug, Deserialize)]
pub struct UpdatePackage {
    #[serde(rename = "download_url")]
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateOffer {
    pub version: String,
    pub package: UpdatePackage,
    pub force_update: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct SharedLatestManifest {
    version: String,
    update_available: bool,
    #[serde(default)]
    force_update: bool,
    packages: SharedPackages,
}

#[derive(Clone, Debug, Deserialize)]
struct SharedPackages {
    windows: std::collections::HashMap<String, UpdatePackage>,
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
        .map_err(|e| format!("{}：{e}", i18n::tr("无法初始化更新网络客户端")))?;
    let manifest = client
        .get(format!(
            "{ENDPOINT}?t={}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ))
        .header(reqwest::header::CACHE_CONTROL, "no-cache")
        .send()
        .map_err(|e| format!("{}：{e}", i18n::tr("检查更新失败")))?
        .error_for_status()
        .map_err(|e| format!("{}：{e}", i18n::tr("检查更新失败")))?
        .json::<SharedLatestManifest>()
        .map_err(|e| format!("{}：{e}", i18n::tr("更新清单无效")))?;
    offer_from_manifest(manifest, env!("CARGO_PKG_VERSION"))
}

fn offer_from_manifest(
    mut manifest: SharedLatestManifest,
    current_version: &str,
) -> Result<Option<UpdateOffer>, String> {
    let package = manifest
        .packages
        .windows
        .remove("x86_64")
        .ok_or_else(|| i18n::tr("更新清单缺少 Windows x86_64 安装包").to_string())?;
    if manifest.update_available
        && version_tuple(&manifest.version) > version_tuple(current_version)
    {
        Ok(Some(UpdateOffer {
            version: manifest.version,
            package,
            force_update: manifest.force_update,
        }))
    } else {
        Ok(None)
    }
}

pub fn install(offer: &UpdateOffer) -> Result<(), String> {
    let package = &offer.package;
    if !package.url.starts_with("https://cdn.fnknock.cn/") {
        return Err(i18n::tr("更新下载地址不受信任").to_string());
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| format!("{}：{e}", i18n::tr("无法初始化更新网络客户端")))?;
    let bytes = client
        .get(&package.url)
        .send()
        .map_err(|e| format!("{}：{e}", i18n::tr("下载更新安装包失败")))?
        .error_for_status()
        .map_err(|e| format!("{}：{e}", i18n::tr("下载更新安装包失败")))?
        .bytes()
        .map_err(|e| format!("{}：{e}", i18n::tr("下载更新安装包失败")))?;
    if bytes.len() as u64 != package.size {
        return Err(i18n::tr("更新安装包大小不匹配").to_string());
    }
    let digest = hex::encode(Sha256::digest(&bytes));
    if !digest.eq_ignore_ascii_case(&package.sha256) {
        return Err(i18n::tr("更新安装包 SHA-256 不匹配").to_string());
    }
    verify_pe_header(&mut Cursor::new(bytes.as_ref()), bytes.len() as u64)?;
    let primary_directory = crate::platform::program_data_dir()?.join("updates");
    let fallback_directory = std::env::temp_dir().join("FnKnock").join("updates");
    let mut errors = Vec::new();

    for directory in [&primary_directory, &fallback_directory] {
        if directory == &fallback_directory && fallback_directory == primary_directory {
            continue;
        }
        match stage_and_launch(directory, offer, &bytes) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(format!("{}：{error}", directory.display())),
        }
    }

    Err(format!(
        "{}。{}；{}",
        i18n::tr("启动更新安装器失败"),
        i18n::tr("已尝试主更新目录和临时备用目录"),
        errors.join("；")
    ))
}

fn stage_and_launch(directory: &Path, offer: &UpdateOffer, bytes: &[u8]) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("{}：{error}", i18n::tr("创建更新目录失败")))?;

    // A unique name avoids reusing a damaged or locked installer left by an earlier attempt.
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_stem = format!(
        "fn-knock-{}-setup-{}-{nonce}",
        offer.version,
        std::process::id()
    );
    let path: PathBuf = directory.join(format!("{file_stem}.exe"));
    let temp = directory.join(format!("{file_stem}.tmp"));

    let write_result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&temp)
            .map_err(|error| format!("{}：{error}", i18n::tr("创建更新安装包失败")))?;
        file.write_all(bytes)
            .map_err(|error| format!("{}：{error}", i18n::tr("写入更新安装包失败")))?;
        file.sync_all()
            .map_err(|error| format!("{}：{error}", i18n::tr("同步更新安装包失败")))?;
        drop(file);
        verify_installer(&temp, &offer.package)?;
        fs::rename(&temp, &path)
            .map_err(|error| format!("{}：{error}", i18n::tr("提交更新安装包失败")))?;
        verify_installer(&path, &offer.package)
    })();
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temp);
        let _ = fs::remove_file(&path);
        return Err(error);
    }

    match Command::new(&path).arg("/passive").spawn() {
        Ok(_) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&path);
            if error.raw_os_error() == Some(1392) {
                Err(format!(
                    "{} 1392：{}（{error}）",
                    i18n::tr("Windows 错误"),
                    i18n::tr("安装包或所在目录损坏且无法读取")
                ))
            } else {
                Err(format!("{}：{error}", i18n::tr("创建安装器进程失败")))
            }
        }
    }
}

fn verify_installer(path: &Path, package: &UpdatePackage) -> Result<(), String> {
    let mut file = fs::File::open(path)
        .map_err(|error| format!("{}：{error}", i18n::tr("重新读取更新安装包失败")))?;
    let metadata = file
        .metadata()
        .map_err(|error| format!("{}：{error}", i18n::tr("读取更新安装包属性失败")))?;
    if metadata.len() != package.size {
        return Err(i18n::tr("更新安装包落盘后的大小不匹配").to_string());
    }

    verify_pe_header(&mut file, metadata.len())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("{}：{error}", i18n::tr("重新读取更新安装包失败")))?;

    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("{}：{error}", i18n::tr("重新读取更新安装包失败")))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if !hex::encode(digest.finalize()).eq_ignore_ascii_case(&package.sha256) {
        return Err(i18n::tr("更新安装包落盘后的 SHA-256 不匹配").to_string());
    }
    Ok(())
}

fn verify_pe_header<R: Read + Seek>(file: &mut R, file_size: u64) -> Result<(), String> {
    let mut dos_header = [0_u8; 64];
    file.read_exact(&mut dos_header).map_err(|error| {
        format!(
            "{}：{error}",
            i18n::tr("更新安装包不是有效的 Windows 可执行文件")
        )
    })?;
    if &dos_header[..2] != b"MZ" {
        return Err(
            i18n::tr("更新安装包不是有效的 Windows 可执行文件（缺少 MZ 文件头）").to_string(),
        );
    }

    let pe_offset = u32::from_le_bytes(
        dos_header[0x3c..0x40]
            .try_into()
            .expect("DOS header PE offset has a fixed width"),
    ) as u64;
    if pe_offset > file_size.saturating_sub(4) {
        return Err(
            i18n::tr("更新安装包不是有效的 Windows 可执行文件（PE 文件头越界）").to_string(),
        );
    }
    file.seek(SeekFrom::Start(pe_offset))
        .map_err(|error| format!("{}：{error}", i18n::tr("读取 Windows PE 文件头失败")))?;
    let mut signature = [0_u8; 4];
    file.read_exact(&mut signature)
        .map_err(|error| format!("{}：{error}", i18n::tr("读取 Windows PE 文件头失败")))?;
    if signature != *b"PE\0\0" {
        return Err(i18n::tr("更新安装包不是有效的 Windows 可执行文件（PE 签名无效）").to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_package(bytes: &[u8]) -> UpdatePackage {
        UpdatePackage {
            url: "https://cdn.fnknock.cn/test.exe".to_string(),
            sha256: hex::encode(Sha256::digest(bytes)),
            size: bytes.len() as u64,
        }
    }

    fn write_test_installer(bytes: &[u8], label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fn-knock-update-test-{label}-{}-{}.exe",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn parses_windows_offer_from_shared_latest_document() {
        let manifest: SharedLatestManifest = serde_json::from_value(serde_json::json!({
            "version": "2.0.2",
            "update_available": true,
            "force_update": true,
            "release_notes": "Shared release notes",
            "packages": {
                "fpk": { "amd64": { "sha256": "core" } },
                "windows": {
                    "x86_64": {
                        "download_url": "https://cdn.fnknock.cn/files/2.0.2/windows/x86_64/setup.exe",
                        "sha256": "abc123",
                        "size": 42,
                        "release_notes": "Windows notes"
                    }
                }
            }
        }))
        .unwrap();
        let offer = offer_from_manifest(manifest, "2.0.1").unwrap().unwrap();
        assert_eq!(offer.version, "2.0.2");
        assert!(offer.force_update);
        assert_eq!(
            offer.package.url,
            "https://cdn.fnknock.cn/files/2.0.2/windows/x86_64/setup.exe"
        );
    }

    #[test]
    fn respects_root_update_available_flag() {
        let manifest: SharedLatestManifest = serde_json::from_value(serde_json::json!({
            "version": "2.0.2",
            "update_available": false,
            "force_update": true,
            "packages": {
                "windows": {
                    "x86_64": {
                        "download_url": "https://cdn.fnknock.cn/files/2.0.2/windows/x86_64/setup.exe",
                        "sha256": "abc123",
                        "size": 12345
                    }
                }
            }
        }))
        .unwrap();

        assert!(offer_from_manifest(manifest, "2.0.1").unwrap().is_none());
    }

    #[test]
    fn accepts_installer_with_valid_pe_headers() {
        let mut bytes = vec![0_u8; 128];
        bytes[..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&64_u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        let path = write_test_installer(&bytes, "valid-pe");

        let result = verify_installer(&path, &test_package(&bytes));
        let _ = fs::remove_file(path);

        assert!(result.is_ok());
    }

    #[test]
    fn rejects_zero_filled_installer_even_when_hash_matches() {
        let bytes = vec![0_u8; 128];
        let path = write_test_installer(&bytes, "zero-filled");

        let result = verify_installer(&path, &test_package(&bytes));
        let _ = fs::remove_file(path);

        assert!(result.unwrap_err().contains("MZ"));
    }
}
