use std::{fs, path::Path};

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

fn validate_windows_bundle_identity() {
    if !std::env::var("TARGET").is_ok_and(|target| target.contains("windows")) {
        return;
    }
    let path = Path::new("../bundle/windows/runtime/bundle.json");
    println!("cargo:rerun-if-changed={}", path.display());
    let bytes = fs::read(path).unwrap_or_else(|error| {
        panic!(
            "Windows runtime bundle identity is missing at {}: {error}; run npm run fn-knock:windows:prepare from the repository root",
            path.display()
        )
    });
    let document: serde_json::Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|error| panic!("invalid Windows runtime bundle identity: {error}"));
    for field in ["commit", "gateway_commit"] {
        let value = document[field].as_str().unwrap_or_default();
        assert!(
            value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "Windows runtime bundle identity has an invalid or missing {field}; run npm run fn-knock:windows:prepare from the repository root"
        );
    }
    assert_eq!(
        document["control_api_version"].as_u64(),
        Some(1),
        "Windows runtime bundle identity has an invalid or missing control_api_version; run npm run fn-knock:windows:prepare from the repository root"
    );
}

fn main() {
    validate_windows_bundle_identity();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(STATUS_COMMANDS)),
    )
    .expect("failed to build FnKnock desktop metadata");
}
