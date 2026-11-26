use std::{
    io::Read,
    path::{Path, PathBuf},
};

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{fs_utils::replace_file, shared::version_utils::is_release_version};

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

pub(crate) const CLOUDFLARED_VERSION: &str = "2026.11.2";
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
            "870dc2094f98befdd993c455234562be4eee51f8fca6e2e356df4579fa7e6766",
        ),
        "darwin-arm64" => (
            "cloudflared-darwin-arm64",
            "8654fbdc9057f3e13e95f7892f417f8643113bee1b91ee2388fd0acb05210d55",
        ),
        "linux-386" => (
            "cloudflared-linux-386",
            "6bfb8ee3731194223d9a07f4a70f52c203525db5cccccbd55647a0d3c01641b1",
        ),
        "linux-amd64" => (
            "cloudflared-linux-amd64",
            "56f13335143ec0e22772963290683a84a04114a35ec64cbc794a4d39ff9bb8cf",
        ),
        "linux-arm" => (
            "cloudflared-linux-arm",
            "d219494581180764c6d2454409691bb06879d01b43cdeae12476070a7e4e4762",
        ),
        "linux-armhf" => (
            "cloudflared-linux-armhf",
            "ab8dda127892266ef25d9787ea43be2314832204e2e32272901ae36010454684",
        ),
        "linux-arm64" => (
            "cloudflared-linux-arm64",
            "04a7b7f08bce85d56b26ffe1fdbfbb21a074495a363d16f0783c40e25b5c5281",
        ),
        "windows-386" => (
            "cloudflared-windows-386.exe",
            "272154d4b8a83a2a9bd55502ab27e721838c42772396751eb5a5fc4739c9f23a",
        ),
        "windows-amd64" => (
            "cloudflared-windows-amd64.exe",
            "555b46c85582461d189cac30d14ad05480d612613801be9f110b8ce08c5b2113",
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

pub(crate) fn cloudflared_installation_info(
    data_dir: &Path,
    platform: &str,
) -> (CloudflaredInstallationStatus, Option<String>) {
    let Some(asset) = cloudflared_asset_spec(platform) else {
        return (CloudflaredInstallationStatus::Missing, None);
    };
    let Some(binary) = cloudflared_binary_path(data_dir, platform).filter(|path| path.is_file())
    else {
        return (CloudflaredInstallationStatus::Missing, None);
    };
    let version = verified_cloudflared_version(data_dir, &binary, asset);
    let status = if version.as_deref() == Some(asset.version) {
        CloudflaredInstallationStatus::Current
    } else {
        CloudflaredInstallationStatus::Outdated
    };
    (status, version)
}

fn cloudflared_install_is_current_for_asset(
    data_dir: &Path,
    binary: &Path,
    asset: CloudflaredAssetSpec,
) -> bool {
    verified_cloudflared_version(data_dir, binary, asset).as_deref() == Some(asset.version)
}

fn verified_cloudflared_version(
    data_dir: &Path,
    binary: &Path,
    asset: CloudflaredAssetSpec,
) -> Option<String> {
    let metadata_path = cloudflared_install_metadata_path(data_dir);
    match std::fs::read_to_string(&metadata_path) {
        Ok(raw) => {
            let metadata = serde_json::from_str::<Value>(&raw).ok()?;
            let version = metadata.get("version")?.as_str()?;
            let sha256 = metadata.get("sha256")?.as_str()?;
            return (is_release_version(version)
                && metadata.get("platform").and_then(Value::as_str) == Some(asset.platform)
                && metadata.get("asset").and_then(Value::as_str) == Some(asset.file_name)
                && (version == asset.version) == (sha256 == asset.sha256)
                && file_checksum_matches(binary, sha256))
            .then(|| version.to_owned());
        }
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && file_checksum_matches(binary, asset.sha256) => {}
        Err(_) => return None,
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
    Some(asset.version.to_owned())
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

pub(crate) fn file_checksum_matches(path: &Path, expected_sha256: &str) -> bool {
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
    fn reports_an_outdated_version_only_when_the_binary_matches_its_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let data_dir = directory.path();
        let platform = "linux-amd64";
        let asset = cloudflared_asset_spec(platform).unwrap();
        let binary = cloudflared_binary_path(data_dir, platform).unwrap();
        let metadata = cloudflared_install_metadata_path(data_dir);

        assert_eq!(
            cloudflared_installation_info(data_dir, platform),
            (CloudflaredInstallationStatus::Missing, None),
        );
        std::fs::create_dir_all(binary.parent().unwrap()).unwrap();
        std::fs::write(&binary, b"previous").unwrap();
        assert_eq!(
            cloudflared_installation_info(data_dir, platform),
            (CloudflaredInstallationStatus::Outdated, None),
        );
        std::fs::write(
            metadata,
            serde_json::to_vec(&serde_json::json!({
                "version": "2026.9.1",
                "platform": platform,
                "asset": asset.file_name,
                "sha256": hex::encode(Sha256::digest(b"previous")),
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            cloudflared_installation_info(data_dir, platform),
            (
                CloudflaredInstallationStatus::Outdated,
                Some("2026.9.1".into())
            ),
        );
        std::fs::write(binary, b"damaged").unwrap();
        assert_eq!(
            cloudflared_installation_info(data_dir, platform),
            (CloudflaredInstallationStatus::Outdated, None),
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
