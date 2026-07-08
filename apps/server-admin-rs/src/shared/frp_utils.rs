use std::path::{Path, PathBuf};

pub(crate) const FRP_VERSION: &str = "0.67.0";

pub(crate) fn detect_frp_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
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
        _ => None,
    }
}

pub(crate) fn frp_binary_path(data_dir: &Path, platform: &str, binary: &str) -> Option<PathBuf> {
    frp_extracted_dir(data_dir, platform).map(|archive| archive.join(binary))
}

pub(crate) fn frp_extracted_dir(data_dir: &Path, platform: &str) -> Option<PathBuf> {
    frp_archive_name(platform).map(|archive_name| data_dir.join("frp").join(archive_name))
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
            Some("frp_0.67.0_linux_amd64".to_string())
        );
        assert_eq!(
            frp_binary_path(Path::new("/tmp/data"), "linux-amd64", "frpc").unwrap(),
            Path::new("/tmp/data/frp/frp_0.67.0_linux_amd64/frpc")
        );
        assert_eq!(
            frp_github_archive_url("frp_0.67.0_linux_amd64"),
            "https://github.com/fatedier/frp/releases/download/v0.67.0/frp_0.67.0_linux_amd64.tar.gz"
        );
        assert!(frp_archive_name("unsupported").is_none());
    }
}
