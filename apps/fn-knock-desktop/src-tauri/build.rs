const STATUS_COMMANDS: &[&str] = &[
    "get_status",
    "open_admin",
    "start_service",
    "restart_service",
    "save_runtime_config",
    "check_for_update",
    "install_update",
    "export_diagnostics",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(STATUS_COMMANDS)),
    )
    .expect("failed to build FnKnock desktop metadata");
}
