use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrpInstallationStatus {
    Missing,
    Outdated,
    Current,
}

impl FrpInstallationStatus {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Outdated => "outdated",
            Self::Current => "current",
        }
    }
}

pub(crate) const FRP_VERSION: &str = "0.71.0";

pub(crate) fn detect_frp_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("macos", "x86_64") => "darwin-amd64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

pub(crate) fn frp_archive_name(platform: &str) -> Option<String> {
    match platform {
        "linux-amd64" => Some(format!("frp_{FRP_VERSION}_linux_amd64")),
        "linux-arm64" => Some(format!("frp_{FRP_VERSION}_linux_arm64")),
        "linux-arm" => Some(format!("frp_{FRP_VERSION}_linux_arm")),
        "darwin-arm64" => Some(format!("frp_{FRP_VERSION}_darwin_arm64")),
        "darwin-amd64" => Some(format!("frp_{FRP_VERSION}_darwin_amd64")),
        _ => None,
    }
}

pub(crate) fn frp_archive_sha256(platform: &str) -> Option<&'static str> {
    match platform {
        "linux-amd64" => Some("84f27e39f11169f7adcef8e8b70c9329de17747b1f14dad9fb95eef5682ea716"),
        "linux-arm" => Some("f40a984f83e8d34a9241b0be4a9d5fbcfe513a4a5c022b84a02637ff6d36833b"),
        "linux-arm64" => Some("f33c293c275d8fc68c654b6fba8f10b2551d6463d09a9fc9cffb7227eae82266"),
        "darwin-amd64" => Some("1b1b4e2f1836e21e8733f1dddaacd4ed9ae67d7dbee39046b9d7b7eda6253637"),
        "darwin-arm64" => Some("45be02b186860d375ed49a8941ae9569628a54bf14e67fc36b29c98c99dabcc6"),
        _ => None,
    }
}

pub(crate) fn frp_binary_path(data_dir: &Path, platform: &str, binary: &str) -> Option<PathBuf> {
    frp_extracted_dir(data_dir, platform).map(|archive| archive.join(binary))
}

pub(crate) fn frp_extracted_dir(data_dir: &Path, platform: &str) -> Option<PathBuf> {
    frp_archive_name(platform).map(|archive_name| data_dir.join("frp").join(archive_name))
}

pub(crate) fn frp_installation_status(data_dir: &Path, platform: &str) -> FrpInstallationStatus {
    let Some(current_archive) = frp_archive_name(platform) else {
        return FrpInstallationStatus::Missing;
    };
    let frp_dir = data_dir.join("frp");
    let current_root = frp_dir.join(&current_archive);
    if frp_root_has_binaries(&current_root) {
        return FrpInstallationStatus::Current;
    }

    // Successful managed installs keep the downloaded archive. Requiring it
    // prevents a deliberately deleted resource from being rediscovered only
    // because an older extracted directory was left behind.
    if !frp_dir.join("frp.tar.gz").is_file() {
        return FrpInstallationStatus::Missing;
    }
    let Some(platform_suffix) = frp_platform_archive_suffix(platform) else {
        return FrpInstallationStatus::Missing;
    };
    let Ok(entries) = fs::read_dir(&frp_dir) else {
        return FrpInstallationStatus::Missing;
    };
    let has_outdated_install = entries.filter_map(Result::ok).any(|entry| {
        let Ok(file_type) = entry.file_type() else {
            return false;
        };
        if !file_type.is_dir() {
            return false;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return false;
        };
        name != current_archive
            && name
                .strip_prefix("frp_")
                .and_then(|value| value.strip_suffix(platform_suffix))
                .is_some_and(|version| !version.is_empty())
            && frp_root_has_binaries(&entry.path())
    });
    if has_outdated_install {
        FrpInstallationStatus::Outdated
    } else {
        FrpInstallationStatus::Missing
    }
}

fn frp_root_has_binaries(root: &Path) -> bool {
    root.join("frpc").is_file() && root.join("frps").is_file()
}

fn frp_platform_archive_suffix(platform: &str) -> Option<&'static str> {
    match platform {
        "linux-amd64" => Some("_linux_amd64"),
        "linux-arm64" => Some("_linux_arm64"),
        "linux-arm" => Some("_linux_arm"),
        "darwin-arm64" => Some("_darwin_arm64"),
        "darwin-amd64" => Some("_darwin_amd64"),
        _ => None,
    }
}

pub(crate) fn frp_github_archive_url(archive: &str) -> String {
    format!("https://github.com/fatedier/frp/releases/download/v{FRP_VERSION}/{archive}.tar.gz")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_frp_archive_paths_from_single_version() {
        assert_eq!(
            frp_archive_name("linux-amd64"),
            Some("frp_0.71.0_linux_amd64".to_string())
        );
        assert_eq!(
            frp_binary_path(Path::new("/tmp/data"), "linux-amd64", "frpc").unwrap(),
            Path::new("/tmp/data/frp/frp_0.71.0_linux_amd64/frpc")
        );
        assert_eq!(
            frp_github_archive_url("frp_0.71.0_linux_amd64"),
            "https://github.com/fatedier/frp/releases/download/v0.71.0/frp_0.71.0_linux_amd64.tar.gz"
        );
        assert_eq!(
            frp_archive_name("darwin-amd64"),
            Some("frp_0.71.0_darwin_amd64".to_string())
        );
        assert_eq!(
            frp_archive_sha256("linux-amd64"),
            Some("84f27e39f11169f7adcef8e8b70c9329de17747b1f14dad9fb95eef5682ea716")
        );
        assert!(frp_archive_name("unsupported").is_none());
        assert!(frp_archive_sha256("unsupported").is_none());
    }

    #[test]
    fn pins_every_supported_archive_to_the_official_checksum() {
        for (platform, expected) in [
            (
                "linux-amd64",
                "84f27e39f11169f7adcef8e8b70c9329de17747b1f14dad9fb95eef5682ea716",
            ),
            (
                "linux-arm",
                "f40a984f83e8d34a9241b0be4a9d5fbcfe513a4a5c022b84a02637ff6d36833b",
            ),
            (
                "linux-arm64",
                "f33c293c275d8fc68c654b6fba8f10b2551d6463d09a9fc9cffb7227eae82266",
            ),
            (
                "darwin-amd64",
                "1b1b4e2f1836e21e8733f1dddaacd4ed9ae67d7dbee39046b9d7b7eda6253637",
            ),
            (
                "darwin-arm64",
                "45be02b186860d375ed49a8941ae9569628a54bf14e67fc36b29c98c99dabcc6",
            ),
        ] {
            assert_eq!(frp_archive_sha256(platform), Some(expected), "{platform}");
        }
    }

    #[test]
    fn distinguishes_missing_outdated_and_current_installations() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path();
        assert_eq!(
            frp_installation_status(data_dir, "linux-amd64"),
            FrpInstallationStatus::Missing
        );

        let frp_dir = data_dir.join("frp");
        let old_root = frp_dir.join("frp_0.70.0_linux_amd64");
        fs::create_dir_all(&old_root).unwrap();
        fs::write(old_root.join("frpc"), b"old").unwrap();
        fs::write(old_root.join("frps"), b"old").unwrap();
        fs::write(frp_dir.join("frp.tar.gz"), b"archive").unwrap();
        assert_eq!(
            frp_installation_status(data_dir, "linux-amd64"),
            FrpInstallationStatus::Outdated
        );
        assert_eq!(
            frp_installation_status(data_dir, "darwin-amd64"),
            FrpInstallationStatus::Missing
        );

        let current_root = frp_extracted_dir(data_dir, "linux-amd64").unwrap();
        fs::create_dir_all(&current_root).unwrap();
        fs::write(current_root.join("frpc"), b"current").unwrap();
        assert_eq!(
            frp_installation_status(data_dir, "linux-amd64"),
            FrpInstallationStatus::Outdated
        );
        fs::write(current_root.join("frps"), b"current").unwrap();
        assert_eq!(
            frp_installation_status(data_dir, "linux-amd64"),
            FrpInstallationStatus::Current
        );

        fs::remove_file(current_root.join("frpc")).unwrap();
        fs::remove_file(frp_dir.join("frp.tar.gz")).unwrap();
        assert_eq!(
            frp_installation_status(data_dir, "linux-amd64"),
            FrpInstallationStatus::Missing
        );
    }
}
