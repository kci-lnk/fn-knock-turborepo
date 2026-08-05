use std::{
    io::Read,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CloudflaredAssetSpec {
    pub platform: &'static str,
    pub file_name: &'static str,
    pub version: &'static str,
    pub sha256: &'static str,
}

pub(crate) const CLOUDFLARED_VERSION: &str = "2026.7.3";
pub(crate) const CLOUDFLARED_INSTALL_METADATA_FILE: &str = "install.json";

pub(crate) fn detect_cloudflared_platform() -> &'static str {
    cloudflared_platform_for_target(
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg!(all(
            target_os = "linux",
            target_arch = "arm",
            target_abi = "eabihf"
        )),
    )
}

#[cfg(test)]
pub(crate) fn cloudflared_platform(os: &str, arch: &str) -> &'static str {
    cloudflared_platform_for_target(os, arch, false)
}

pub(crate) fn cloudflared_platform_for_target(
    os: &str,
    arch: &str,
    arm_hard_float: bool,
) -> &'static str {
    match (os, arch) {
        ("macos", "x86_64") => "darwin-amd64",
        ("macos", "aarch64") => "darwin-arm64",
        ("linux", "x86_64" | "amd64") => "linux-amd64",
        ("linux", "x86" | "i386" | "i686") => "linux-386",
        ("linux", "aarch64" | "arm64") => "linux-arm64",
        ("linux", "armv7") => "linux-armhf",
        ("linux", "arm") if arm_hard_float => "linux-armhf",
        ("linux", "arm") => "linux-arm",
        ("windows", "x86_64" | "amd64") => "windows-amd64",
        ("windows", "x86" | "i386" | "i686") => "windows-386",
        _ => "unsupported",
    }
}

pub(crate) fn cloudflared_asset_name(platform: &str) -> Option<&'static str> {
    cloudflared_asset_spec(platform).map(|asset| asset.file_name)
}

pub(crate) fn cloudflared_asset_spec(platform: &str) -> Option<CloudflaredAssetSpec> {
    let (file_name, sha256) = match platform {
        "darwin-amd64" => (
            "cloudflared-darwin-amd64",
            "e88fe5874d42a94f49a7ea59cabc3722d2962d0449232b0f3b1a426a712e275c",
        ),
        "darwin-arm64" => (
            "cloudflared-darwin-arm64",
            "f35c50089cd25f77a4cb5a2152036bc26db15aa31fbe11f7995d2e42a4ed6257",
        ),
        "linux-386" => (
            "cloudflared-linux-386",
            "6c982e77e644644f5bce76781dd2b69ddc0bfa5e1dd1f55f0037850ac0946771",
        ),
        "linux-amd64" => (
            "cloudflared-linux-amd64",
            "9d71c677db00134c1bd4144b7783486b654ad281b1ea62b4972098d19f770f17",
        ),
        "linux-arm" => (
            "cloudflared-linux-arm",
            "6dadd979b8833760e9f6d840a6239a8c08c8bcf73b4231ec537f483873f37c73",
        ),
        "linux-armhf" => (
            "cloudflared-linux-armhf",
            "2aadbe6416e5c52cb7ebba99119f413a124f358516c17d4ecaacb89a363e8a35",
        ),
        "linux-arm64" => (
            "cloudflared-linux-arm64",
            "65259e652a7bea08bf5df603233ab22b8bf3116af8df9f9206209af6a1b955c0",
        ),
        "windows-386" => (
            "cloudflared-windows-386.exe",
            "d026e39d9be21c70ea652528fda2801e164d5e25688b7b0fb3b65080cbd96503",
        ),
        "windows-amd64" => (
            "cloudflared-windows-amd64.exe",
            "8635da433b6df8194746e88ed9d2589566c20e38bfc2a80e431a348b7c765841",
        ),
        _ => return None,
    };
    Some(CloudflaredAssetSpec {
        platform: match platform {
            "darwin-amd64" => "darwin-amd64",
            "darwin-arm64" => "darwin-arm64",
            "linux-386" => "linux-386",
            "linux-amd64" => "linux-amd64",
            "linux-arm" => "linux-arm",
            "linux-armhf" => "linux-armhf",
            "linux-arm64" => "linux-arm64",
            "windows-386" => "windows-386",
            "windows-amd64" => "windows-amd64",
            _ => unreachable!(),
        },
        file_name,
        version: CLOUDFLARED_VERSION,
        sha256,
    })
}

pub(crate) fn cloudflared_binary_path(data_dir: &Path, platform: &str) -> Option<PathBuf> {
    cloudflared_asset_name(platform)?;
    let binary_name = if platform.starts_with("windows-") {
        "cloudflared.exe"
    } else {
        "cloudflared"
    };
    Some(data_dir.join("cloudflared").join(binary_name))
}

pub(crate) fn cloudflared_install_metadata_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join("cloudflared")
        .join(CLOUDFLARED_INSTALL_METADATA_FILE)
}

pub(crate) fn cloudflared_install_is_current(data_dir: &Path, platform: &str) -> bool {
    let Some(asset) = cloudflared_asset_spec(platform) else {
        return false;
    };
    let Some(binary) = cloudflared_binary_path(data_dir, platform) else {
        return false;
    };
    if !binary.is_file() {
        return false;
    }
    let metadata_matches = std::fs::read_to_string(cloudflared_install_metadata_path(data_dir))
        .ok()
        .and_then(|raw| serde_json::from_str::<Value>(&raw).ok())
        .is_some_and(|metadata| {
            metadata.get("version").and_then(Value::as_str) == Some(asset.version)
                && metadata.get("platform").and_then(Value::as_str) == Some(asset.platform)
                && metadata.get("asset").and_then(Value::as_str) == Some(asset.file_name)
                && metadata.get("sha256").and_then(Value::as_str) == Some(asset.sha256)
        });
    if metadata_matches {
        return true;
    }
    if !cloudflared_binary_checksum_is_current(data_dir, platform) {
        return false;
    }
    let metadata = serde_json::json!({
        "version": asset.version,
        "platform": asset.platform,
        "asset": asset.file_name,
        "sha256": asset.sha256,
    });
    std::fs::write(
        cloudflared_install_metadata_path(data_dir),
        serde_json::to_vec_pretty(&metadata).unwrap_or_default(),
    )
    .is_ok()
}

pub(crate) fn cloudflared_binary_checksum_is_current(data_dir: &Path, platform: &str) -> bool {
    let Some(asset) = cloudflared_asset_spec(platform) else {
        return false;
    };
    let Some(path) = cloudflared_binary_path(data_dir, platform) else {
        return false;
    };
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let Ok(read) = file.read(&mut buffer) else {
            return false;
        };
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    hex::encode(hasher.finalize()) == asset.sha256
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_every_upstream_standalone_binary_platform() {
        let cases = [
            (
                "macos",
                "x86_64",
                "darwin-amd64",
                "cloudflared-darwin-amd64",
            ),
            (
                "macos",
                "aarch64",
                "darwin-arm64",
                "cloudflared-darwin-arm64",
            ),
            ("linux", "x86", "linux-386", "cloudflared-linux-386"),
            ("linux", "x86_64", "linux-amd64", "cloudflared-linux-amd64"),
            ("linux", "arm", "linux-arm", "cloudflared-linux-arm"),
            ("linux", "armv7", "linux-armhf", "cloudflared-linux-armhf"),
            ("linux", "aarch64", "linux-arm64", "cloudflared-linux-arm64"),
            (
                "windows",
                "x86",
                "windows-386",
                "cloudflared-windows-386.exe",
            ),
            (
                "windows",
                "x86_64",
                "windows-amd64",
                "cloudflared-windows-amd64.exe",
            ),
        ];

        for (os, arch, platform, asset) in cases {
            assert_eq!(cloudflared_platform(os, arch), platform);
            assert_eq!(cloudflared_asset_name(platform), Some(asset));
        }
        assert_eq!(cloudflared_platform("windows", "aarch64"), "unsupported");
        assert_eq!(
            cloudflared_platform_for_target("linux", "arm", true),
            "linux-armhf"
        );
        assert_eq!(
            cloudflared_platform_for_target("linux", "arm", false),
            "linux-arm"
        );
    }

    #[test]
    fn pins_every_asset_to_the_current_release_checksum() {
        for platform in [
            "darwin-amd64",
            "darwin-arm64",
            "linux-386",
            "linux-amd64",
            "linux-arm",
            "linux-armhf",
            "linux-arm64",
            "windows-386",
            "windows-amd64",
        ] {
            let asset = cloudflared_asset_spec(platform).unwrap();
            assert_eq!(asset.platform, platform);
            assert_eq!(asset.version, CLOUDFLARED_VERSION);
            assert_eq!(asset.sha256.len(), 64);
            assert!(asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn uses_the_native_windows_executable_suffix() {
        assert_eq!(
            cloudflared_binary_path(Path::new("C:/fn-knock/data"), "windows-amd64").unwrap(),
            Path::new("C:/fn-knock/data/cloudflared/cloudflared.exe")
        );
        assert_eq!(
            cloudflared_binary_path(Path::new("/var/lib/fn-knock"), "linux-amd64").unwrap(),
            Path::new("/var/lib/fn-knock/cloudflared/cloudflared")
        );
        assert!(cloudflared_binary_path(Path::new("/tmp"), "unsupported").is_none());
    }

    #[test]
    fn requires_matching_install_metadata_before_reporting_current() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let platform = "linux-amd64";
        let asset = cloudflared_asset_spec(platform).unwrap();
        let binary = cloudflared_binary_path(data_dir, platform).unwrap();
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"binary").unwrap();
        assert!(!cloudflared_install_is_current(data_dir, platform));

        std::fs::write(
            cloudflared_install_metadata_path(data_dir),
            serde_json::to_vec(&serde_json::json!({
                "version": asset.version,
                "platform": asset.platform,
                "asset": asset.file_name,
                "sha256": asset.sha256,
            }))
            .unwrap(),
        )
        .unwrap();
        assert!(cloudflared_install_is_current(data_dir, platform));
    }
}
