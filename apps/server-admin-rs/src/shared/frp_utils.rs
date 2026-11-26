use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;

use crate::shared::version_utils::is_release_version;

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

pub(crate) const FRP_VERSION: &str = "0.72.0";

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
        "linux-amd64" => Some("daf719b32ca2778770bb92955e231a07414d754378b9af328aa2642862479804"),
        "linux-arm" => Some("d0531b20c7fef4c7d2efb281931e62f0612bb7f0de62d200fa9020fd58d319ca"),
        "linux-arm64" => Some("02180b857480302fd934e9ddbd50fe2f73229f99062f88756abc6de9df5e95cf"),
        "darwin-amd64" => Some("60439dc6bf794c4182a1a4cac147d642f839d55201607208979eb23aecd09be9"),
        "darwin-arm64" => Some("6e8576c5b6b9914a86850f6ff4fb81c5a09e706f91f70f64d5f398558bdc9db8"),
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

pub(crate) fn frp_installed_version(
    data_dir: &Path,
    platform: &str,
    status: FrpInstallationStatus,
) -> Option<String> {
    match status {
        FrpInstallationStatus::Missing => return None,
        FrpInstallationStatus::Current => return Some(FRP_VERSION.to_owned()),
        FrpInstallationStatus::Outdated => {}
    }

    let frp_dir = data_dir.join("frp");
    let archive = fs::File::open(frp_dir.join("frp.tar.gz")).ok()?;
    // The retained official tarball identifies the installed release even
    // when extracted directories from several older releases still exist.
    let mut header = [0; 512];
    GzDecoder::new(archive).read_exact(&mut header).ok()?;
    if &header[257..262] != b"ustar" {
        return None;
    }
    let path = std::str::from_utf8(header[..100].split(|byte| *byte == 0).next()?).ok()?;
    let root = path.split('/').next()?;
    let version = root
        .strip_prefix("frp_")?
        .strip_suffix(frp_platform_archive_suffix(platform)?)?;
    (is_release_version(version) && frp_root_has_binaries(&frp_dir.join(root)))
        .then(|| version.to_owned())
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
    use std::io::Write;

    #[test]
    fn builds_frp_archive_paths_from_single_version() {
        assert_eq!(
            frp_archive_name("linux-amd64"),
            Some("frp_0.72.0_linux_amd64".to_string())
        );
        assert_eq!(
            frp_binary_path(Path::new("/tmp/data"), "linux-amd64", "frpc").unwrap(),
            Path::new("/tmp/data/frp/frp_0.72.0_linux_amd64/frpc")
        );
        assert_eq!(
            frp_github_archive_url("frp_0.72.0_linux_amd64"),
            "https://github.com/fatedier/frp/releases/download/v0.72.0/frp_0.72.0_linux_amd64.tar.gz"
        );
        assert_eq!(
            frp_archive_name("darwin-amd64"),
            Some("frp_0.72.0_darwin_amd64".to_string())
        );
        assert_eq!(
            frp_archive_sha256("linux-amd64"),
            Some("daf719b32ca2778770bb92955e231a07414d754378b9af328aa2642862479804")
        );
        assert!(frp_archive_name("unsupported").is_none());
        assert!(frp_archive_sha256("unsupported").is_none());
    }

    #[test]
    fn pins_every_supported_archive_to_the_official_checksum() {
        for (platform, expected) in [
            (
                "linux-amd64",
                "daf719b32ca2778770bb92955e231a07414d754378b9af328aa2642862479804",
            ),
            (
                "linux-arm",
                "d0531b20c7fef4c7d2efb281931e62f0612bb7f0de62d200fa9020fd58d319ca",
            ),
            (
                "linux-arm64",
                "02180b857480302fd934e9ddbd50fe2f73229f99062f88756abc6de9df5e95cf",
            ),
            (
                "darwin-amd64",
                "60439dc6bf794c4182a1a4cac147d642f839d55201607208979eb23aecd09be9",
            ),
            (
                "darwin-arm64",
                "6e8576c5b6b9914a86850f6ff4fb81c5a09e706f91f70f64d5f398558bdc9db8",
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

    #[test]
    fn reports_the_version_of_the_retained_archive_when_old_directories_coexist() {
        let temp = tempfile::tempdir().unwrap();
        let data_dir = temp.path();
        let frp_dir = data_dir.join("frp");
        for version in ["0.69.0", "0.71.0"] {
            let root = frp_dir.join(format!("frp_{version}_linux_amd64"));
            fs::create_dir_all(&root).unwrap();
            for program in ["frpc", "frps"] {
                fs::write(root.join(program), b"old").unwrap();
            }
        }

        let archive = frp_dir.join("frp.tar.gz");
        let file = fs::File::create(&archive).unwrap();
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
        let mut header = [0; 512];
        let path = b"frp_0.69.0_linux_amd64/";
        header[..path.len()].copy_from_slice(path);
        header[257..262].copy_from_slice(b"ustar");
        encoder.write_all(&header).unwrap();
        encoder.finish().unwrap();

        let platform = "linux-amd64";
        let status = frp_installation_status(data_dir, platform);
        assert_eq!(status, FrpInstallationStatus::Outdated);
        assert_eq!(
            frp_installed_version(data_dir, platform, status).as_deref(),
            Some("0.69.0"),
        );

        fs::write(archive, b"invalid").unwrap();
        assert_eq!(frp_installed_version(data_dir, platform, status), None);
    }
}
