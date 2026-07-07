use super::*;

pub(super) fn frp_executable(state: &AppState) -> Option<PathBuf> {
    let path = frp_binary_path(&state.settings.data_dir, detect_frp_platform(), "frpc")?;
    path.exists().then_some(path)
}

pub(super) fn detect_frp_platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "darwin-arm64",
        ("linux", "x86_64") => "linux-amd64",
        ("linux", "aarch64") => "linux-arm64",
        ("linux", "arm") | ("linux", "armv7") => "linux-arm",
        _ => "unsupported",
    }
}

pub(super) fn frp_archive_name(platform: &str) -> Option<String> {
    match platform {
        "linux-amd64" => Some(format!("frp_{FRPC_VERSION}_linux_amd64")),
        "linux-arm64" => Some(format!("frp_{FRPC_VERSION}_linux_arm64")),
        "linux-arm" => Some(format!("frp_{FRPC_VERSION}_linux_arm")),
        "darwin-arm64" => Some(format!("frp_{FRPC_VERSION}_darwin_arm64")),
        _ => None,
    }
}

pub(super) fn frp_binary_path(data_dir: &Path, platform: &str, binary: &str) -> Option<PathBuf> {
    frp_archive_name(platform).map(|archive| data_dir.join("frp").join(archive).join(binary))
}
