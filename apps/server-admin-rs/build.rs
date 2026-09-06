// Build-time validation intentionally aborts when repository metadata is
// invalid. This script is not linked into the production runtime guarded by
// scripts/check-rust-prod-panics.sh.
#![allow(clippy::panic, clippy::todo, clippy::unimplemented)]

use std::{
    env, fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use serde_json::Value;

struct AppMetadata {
    version: String,
    github_url: String,
    gateway_commit: String,
    backup_schema_version: i64,
    backup_import_min_version: String,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let version_file = manifest_dir.join("../../version.json");
    let proto_file =
        manifest_dir.join("../../packages/grpc-contracts/proto/fnknock/v1/gateway.proto");
    let proto_root = manifest_dir.join("../../packages/grpc-contracts/proto");
    println!("cargo:rerun-if-changed={}", version_file.display());
    println!("cargo:rerun-if-changed={}", proto_file.display());
    println!("cargo:rerun-if-env-changed=FN_KNOCK_GATEWAY_COMMIT");

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("resolve vendored protoc");
    // SAFETY: build scripts run single-threaded here before tonic_build reads
    // PROTOC, so no concurrent environment access is introduced.
    unsafe {
        env::set_var("PROTOC", protoc);
    }
    tonic_build::configure()
        .build_server(true)
        .server_mod_attribute(".", "#[cfg(test)]")
        .compile_protos(&[proto_file], &[proto_root])
        .expect("compile fn-knock grpc proto");

    let mut metadata = load_app_metadata(&version_file);
    if let Ok(commit) = env::var("FN_KNOCK_GATEWAY_COMMIT")
        && !commit.trim().is_empty()
    {
        metadata.gateway_commit = validate_gateway_commit(commit.trim());
    }
    println!(
        "cargo:rustc-env=FN_KNOCK_GATEWAY_COMMIT={}",
        metadata.gateway_commit
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(
        out_dir.join("app_version.rs"),
        app_version_source(&metadata),
    )
    .expect("write generated app version");
    write_swagger_ui_assets(&out_dir);
}

fn write_swagger_ui_assets(out_dir: &Path) {
    const ASSETS: [(&str, &str); 6] = [
        ("/dist/swagger-ui.css", "swagger-ui.css"),
        ("/dist/index.css", "index.css"),
        ("/dist/swagger-ui-bundle.js", "swagger-ui-bundle.js"),
        (
            "/dist/swagger-ui-standalone-preset.js",
            "swagger-ui-standalone-preset.js",
        ),
        ("/dist/favicon-16x16.png", "favicon-16x16.png"),
        ("/dist/favicon-32x32.png", "favicon-32x32.png"),
    ];

    let reader = Cursor::new(utoipa_swagger_ui_vendored::SWAGGER_UI_VENDORED);
    let mut archive = zip::ZipArchive::new(reader).expect("open vendored Swagger UI archive");

    for (source_suffix, output_name) in ASSETS {
        let index = (0..archive.len())
            .find(|index| {
                archive
                    .by_index(*index)
                    .map(|file| file.name().ends_with(source_suffix))
                    .unwrap_or(false)
            })
            .unwrap_or_else(|| panic!("vendored Swagger UI is missing {source_suffix}"));
        let mut source = archive
            .by_index(index)
            .unwrap_or_else(|_| panic!("read vendored Swagger UI asset {source_suffix}"));
        let mut bytes = Vec::with_capacity(source.size() as usize);
        source
            .read_to_end(&mut bytes)
            .unwrap_or_else(|_| panic!("extract vendored Swagger UI asset {source_suffix}"));
        fs::write(out_dir.join(output_name), bytes)
            .unwrap_or_else(|_| panic!("write vendored Swagger UI asset {output_name}"));
    }
}

fn load_app_metadata(path: &Path) -> AppMetadata {
    let content = fs::read_to_string(path).expect("read version.json");
    let value = serde_json::from_str::<Value>(&content).expect("parse version.json");
    let object = value.as_object().expect("version.json must be an object");
    let gateway_commit = required_gateway_commit(object.get("gatewayCommit"));
    AppMetadata {
        version: required_string(object.get("version"), "version"),
        github_url: required_string(object.get("githubUrl"), "githubUrl"),
        gateway_commit,
        backup_schema_version: object
            .get("backupSchemaVersion")
            .and_then(Value::as_i64)
            .expect("version.json backupSchemaVersion must be an integer"),
        backup_import_min_version: required_string(
            object.get("backupImportMinVersion"),
            "backupImportMinVersion",
        ),
    }
}

fn required_gateway_commit(value: Option<&Value>) -> String {
    validate_gateway_commit(&required_string(value, "gatewayCommit"))
}

fn validate_gateway_commit(commit: &str) -> String {
    if commit.len() != 40
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        panic!("gateway commit must be a 40-character lowercase Git commit");
    }
    commit.to_string()
}

fn required_string(value: Option<&Value>, key: &str) -> String {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| panic!("version.json {key} must be a non-empty string"))
}

fn app_version_source(metadata: &AppMetadata) -> String {
    format!(
        "pub const APP_LOCAL_VERSION: &str = {version:?};\n\
         pub const APP_GITHUB_URL: &str = {github_url:?};\n\
         pub const APP_BACKUP_SCHEMA_VERSION: i64 = {backup_schema_version};\n\
         pub const APP_BACKUP_IMPORT_MIN_VERSION: &str = {backup_import_min_version:?};\n",
        version = metadata.version,
        github_url = metadata.github_url,
        backup_schema_version = metadata.backup_schema_version,
        backup_import_min_version = metadata.backup_import_min_version,
    )
}
