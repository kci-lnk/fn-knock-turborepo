use std::{
    io::Read,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::fs_utils::replace_file;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CloudflaredAssetSpec {
    pub platform: &'static str,
    pub file_name: &'static str,
    pub version: &'static str,
    pub sha256: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CloudflaredInstallationStatus {
    Missing,
    Outdated,
    Current,
}

impl CloudflaredInstallationStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Outdated => "outdated",
            Self::Current => "current",
        }
    }
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
    cloudflared_install_is_current_for_asset(data_dir, &binary, asset)
}

pub(crate) fn cloudflared_installation_status(
    data_dir: &Path,
    platform: &str,
) -> CloudflaredInstallationStatus {
    let Some(binary) = cloudflared_binary_path(data_dir, platform) else {
        return CloudflaredInstallationStatus::Missing;
    };
    let binary_exists = binary.is_file();
    classify_cloudflared_installation_status(
        binary_exists,
        binary_exists && cloudflared_install_is_current(data_dir, platform),
    )
}

fn classify_cloudflared_installation_status(
    binary_exists: bool,
    is_current: bool,
) -> CloudflaredInstallationStatus {
    match (binary_exists, is_current) {
        (false, _) => CloudflaredInstallationStatus::Missing,
        (true, true) => CloudflaredInstallationStatus::Current,
        (true, false) => CloudflaredInstallationStatus::Outdated,
    }
}

fn cloudflared_install_is_current_for_asset(
    data_dir: &Path,
    binary: &Path,
    asset: CloudflaredAssetSpec,
) -> bool {
    if !binary.is_file() || !file_checksum_matches(binary, asset.sha256) {
        return false;
    }
    let metadata_path = cloudflared_install_metadata_path(data_dir);
    match std::fs::read_to_string(&metadata_path) {
        Ok(raw) => {
            return serde_json::from_str::<Value>(&raw)
                .ok()
                .is_some_and(|metadata| {
                    metadata.get("version").and_then(Value::as_str) == Some(asset.version)
                        && metadata.get("platform").and_then(Value::as_str) == Some(asset.platform)
                        && metadata.get("asset").and_then(Value::as_str) == Some(asset.file_name)
                        && metadata.get("sha256").and_then(Value::as_str) == Some(asset.sha256)
                });
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return false,
    }
    let metadata = serde_json::json!({
        "version": asset.version,
        "platform": asset.platform,
        "asset": asset.file_name,
        "sha256": asset.sha256,
    });
    repair_cloudflared_install_metadata(
        data_dir,
        &serde_json::to_vec_pretty(&metadata).unwrap_or_default(),
    );

    // A checksum-verified binary without metadata is the one repairable legacy
    // state. If the repair cannot be persisted yet, do not misclassify the
    // known binary as outdated; a later status check will retry the repair.
    true
}

fn repair_cloudflared_install_metadata(data_dir: &Path, content: &[u8]) {
    let metadata = cloudflared_install_metadata_path(data_dir);
    let Some(directory) = metadata.parent() else {
        return;
    };
    let temporary = directory.join(format!(
        "install.repair.{}.tmp",
        uuid::Uuid::new_v4().simple()
    ));
    if std::fs::write(&temporary, content).is_ok() {
        let _ = replace_file(&temporary, &metadata);
    }
    let _ = std::fs::remove_file(temporary);
}

fn file_checksum_matches(path: &Path, expected_sha256: &str) -> bool {
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
    hex::encode(hasher.finalize()) == expected_sha256
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
    fn classifies_missing_outdated_and_current_installations() {
        assert_eq!(
            classify_cloudflared_installation_status(false, false),
            CloudflaredInstallationStatus::Missing
        );
        assert_eq!(
            classify_cloudflared_installation_status(true, false),
            CloudflaredInstallationStatus::Outdated
        );
        assert_eq!(
            classify_cloudflared_installation_status(true, true),
            CloudflaredInstallationStatus::Current
        );
    }

    #[test]
    fn repairs_missing_metadata_for_a_checksum_verified_binary() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let asset = CloudflaredAssetSpec {
            platform: "test-platform",
            file_name: "cloudflared-test",
            version: "2099.1.0",
            sha256: "97b0560280ed60a5a1eaa1bc45492543c8a986ad5a25b468c427eb83c3e88191",
        };
        let binary = data_dir.join("cloudflared").join("cloudflared");
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"current").unwrap();

        assert!(cloudflared_install_is_current_for_asset(
            data_dir, &binary, asset
        ));
        let metadata: Value = serde_json::from_slice(
            &std::fs::read(cloudflared_install_metadata_path(data_dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["version"], asset.version);
        assert_eq!(metadata["sha256"], asset.sha256);
        assert!(
            std::fs::read_dir(binary.parent().unwrap())
                .unwrap()
                .flatten()
                .all(|entry| !entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("install.repair."))
        );
    }

    #[test]
    fn rejects_a_damaged_binary_even_when_metadata_matches() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let asset = CloudflaredAssetSpec {
            platform: "test-platform",
            file_name: "cloudflared-test",
            version: "2099.1.0",
            sha256: "97b0560280ed60a5a1eaa1bc45492543c8a986ad5a25b468c427eb83c3e88191",
        };
        let binary = data_dir.join("cloudflared").join("cloudflared");
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"current").unwrap();
        assert!(cloudflared_install_is_current_for_asset(
            data_dir, &binary, asset
        ));

        std::fs::write(&binary, b"damaged").unwrap();
        assert!(!cloudflared_install_is_current_for_asset(
            data_dir, &binary, asset
        ));
    }

    #[test]
    fn rejects_existing_metadata_that_does_not_match_the_pinned_asset() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let asset = CloudflaredAssetSpec {
            platform: "test-platform",
            file_name: "cloudflared-test",
            version: "2099.1.0",
            sha256: "97b0560280ed60a5a1eaa1bc45492543c8a986ad5a25b468c427eb83c3e88191",
        };
        let binary = data_dir.join("cloudflared").join("cloudflared");
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"current").unwrap();
        std::fs::write(
            cloudflared_install_metadata_path(data_dir),
            serde_json::to_vec(&serde_json::json!({
                "version": "2098.12.0",
                "platform": asset.platform,
                "asset": asset.file_name,
                "sha256": asset.sha256,
            }))
            .unwrap(),
        )
        .unwrap();

        assert!(!cloudflared_install_is_current_for_asset(
            data_dir, &binary, asset
        ));
        let metadata: Value = serde_json::from_slice(
            &std::fs::read(cloudflared_install_metadata_path(data_dir)).unwrap(),
        )
        .unwrap();
        assert_eq!(metadata["version"], "2098.12.0");
    }
}
