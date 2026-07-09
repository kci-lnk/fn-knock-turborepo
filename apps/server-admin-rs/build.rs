use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Route {
    method: &'static str,
    path: String,
}

struct AppMetadata {
    version: String,
    github_url: String,
    backup_schema_version: i64,
    backup_import_min_version: String,
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let version_file = manifest_dir.join("../../version.json");
    let proto_file =
        manifest_dir.join("../../packages/grpc-contracts/proto/fnknock/v1/gateway.proto");
    let proto_root = manifest_dir.join("../../packages/grpc-contracts/proto");
    println!("cargo:rerun-if-changed={}", src_dir.display());
    println!("cargo:rerun-if-changed={}", version_file.display());
    println!("cargo:rerun-if-changed={}", proto_file.display());

    let protoc = protoc_bin_vendored::protoc_bin_path().expect("resolve vendored protoc");
    // SAFETY: build scripts run single-threaded here before tonic_build reads
    // PROTOC, so no concurrent environment access is introduced.
    unsafe {
        env::set_var("PROTOC", protoc);
    }
    tonic_build::configure()
        .build_server(false)
        .compile_protos(&[proto_file], &[proto_root])
        .expect("compile fn-knock grpc proto");

    let mut routes = BTreeSet::new();
    collect_routes(&src_dir, &mut routes);

    let metadata = load_app_metadata(&version_file);
    let json = format!(
        "[{}]",
        routes
            .iter()
            .map(|route| format!(
                "{{\"method\":\"{}\",\"path\":\"{}\"}}",
                route.method,
                escape_json(&route.path)
            ))
            .collect::<Vec<_>>()
            .join(",")
    );
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join("openapi_routes.json"), json).expect("write generated OpenAPI routes");
    fs::write(
        out_dir.join("app_version.rs"),
        app_version_source(&metadata),
    )
    .expect("write generated app version");
}

fn load_app_metadata(path: &Path) -> AppMetadata {
    let content = fs::read_to_string(path).expect("read version.json");
    let value = serde_json::from_str::<Value>(&content).expect("parse version.json");
    let object = value.as_object().expect("version.json must be an object");
    AppMetadata {
        version: required_string(object.get("version"), "version"),
        github_url: required_string(object.get("githubUrl"), "githubUrl"),
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

fn collect_routes(dir: &Path, routes: &mut BTreeSet<Route>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_routes(&path, routes);
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        extract_routes(&content, routes);
    }
}

fn extract_routes(content: &str, routes: &mut BTreeSet<Route>) {
    let mut cursor = 0;
    while let Some(relative_index) = content[cursor..].find(".route(") {
        let start = cursor + relative_index;
        let Some((path, after_path)) = parse_route_path(content, start + ".route(".len()) else {
            cursor = start + ".route(".len();
            continue;
        };
        if !(path.starts_with("/api/admin") || path.starts_with("/api/internal")) {
            cursor = after_path;
            continue;
        }
        let Some(end) = find_balanced_call_end(content, start) else {
            cursor = after_path;
            continue;
        };
        let spec = &content[start..=end];
        for (name, method) in [
            ("get", "GET"),
            ("post", "POST"),
            ("put", "PUT"),
            ("delete", "DELETE"),
            ("patch", "PATCH"),
            ("head", "HEAD"),
        ] {
            if method_is_present(spec, name) {
                routes.insert(Route {
                    method,
                    path: path.clone(),
                });
            }
        }
        cursor = end + 1;
    }
}

fn parse_route_path(content: &str, mut index: usize) -> Option<(String, usize)> {
    while content
        .as_bytes()
        .get(index)
        .is_some_and(u8::is_ascii_whitespace)
    {
        index += 1;
    }
    if content.as_bytes().get(index) != Some(&b'"') {
        return None;
    }
    index += 1;
    let mut path = String::new();
    let mut escaped = false;
    for (offset, ch) in content[index..].char_indices() {
        if escaped {
            path.push(ch);
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => return Some((path, index + offset + ch.len_utf8())),
            _ => path.push(ch),
        }
    }
    None
}

fn find_balanced_call_end(content: &str, start: usize) -> Option<usize> {
    let mut depth = 0_i64;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in content[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(start + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn method_is_present(spec: &str, name: &str) -> bool {
    let direct = format!("{name}(");
    let chained = format!(".{name}(");
    spec.contains(&direct) || spec.contains(&chained)
}

fn escape_json(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
