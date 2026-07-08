use super::*;

pub(super) use crate::frp_utils::detect_frp_platform;

pub(super) fn frp_executable(state: &AppState) -> Option<PathBuf> {
    let path =
        crate::frp_utils::frp_binary_path(&state.settings.data_dir, detect_frp_platform(), "frpc")?;
    path.exists().then_some(path)
}
