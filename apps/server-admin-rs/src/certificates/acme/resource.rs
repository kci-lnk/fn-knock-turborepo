use super::*;
use utoipa_axum::{router::OpenApiRouter, routes};

pub(super) fn openapi_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(resource_status))
        .routes(routes!(initialize_resource))
        .routes(routes!(cancel_resource_initialization))
        .routes(routes!(delete_resource))
}

const RUST_ACMESH_EXECUTABLE: &str = "rust-acmesh.exe";

pub(super) fn windows_acme_provider_ids() -> &'static [&'static str] {
    &[
        "dns_ali",
        "dns_baiducloud",
        "dns_cf",
        "dns_dp",
        "dns_tencent",
        "dns_duckdns",
        "dns_dynu",
        "dns_dynv6",
        "dns_gd",
        "dns_huaweicloud",
        "dns_porkbun",
    ]
}

pub(super) fn rust_acmesh_executable_path() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os("FN_KNOCK_RUST_ACMESH_EXE") {
        candidates.push(PathBuf::from(path));
    }
    if let Ok(executable) = env::current_exe()
        && let Some(directory) = executable.parent()
    {
        candidates.push(directory.join(RUST_ACMESH_EXECUTABLE));
    }
    if let Ok(directory) = env::current_dir() {
        candidates.extend([
            directory.join(RUST_ACMESH_EXECUTABLE),
            directory.join("resources").join(RUST_ACMESH_EXECUTABLE),
            directory
                .join("apps/server-admin-rs/resources")
                .join(RUST_ACMESH_EXECUTABLE),
        ]);
    }
    candidates.into_iter().find(|path| path.is_file())
}

#[utoipa::path(get, path = "/api/admin/acme/resource/status", tag = "acme", responses((status = 200, description = "ACME resource status")))]
pub(super) async fn resource_status(State(state): State<AppState>) -> Response {
    if crate::runtime_profile::deployment_target(&state) != "windows" {
        return response::ok(json!({
            "supported": false,
            "initialized": false,
            "platform": "native-acme-sh",
            "installedVersion": Value::Null,
            "availableVersion": Value::Null,
            "progress": { "status": "idle", "percent": 0, "error": Value::Null },
            "providerIds": [],
        }))
        .into_response();
    }
    let executable = rust_acmesh_executable_path();
    response::ok(json!({
        "supported": cfg!(windows),
        "initialized": executable.is_some(),
        "platform": if cfg!(windows) { "windows-x86_64" } else { "native-acme-sh" },
        "installedVersion": Value::Null,
        "availableVersion": Value::Null,
        "progress": {
            "status": if executable.is_some() { "completed" } else { "error" },
            "percent": if executable.is_some() { 100 } else { 0 },
            "error": if executable.is_some() {
                Value::Null
            } else {
                json!("bundled rust-acmesh.exe is missing")
            },
        },
        "providerIds": windows_acme_provider_ids(),
    }))
    .into_response()
}

#[utoipa::path(post, path = "/api/admin/acme/resource/initialize", tag = "acme", responses((status = 200, description = "Initialized ACME resource")))]
pub(super) async fn initialize_resource(State(state): State<AppState>) -> Response {
    if crate::runtime_profile::deployment_target(&state) != "windows" {
        return response::error(
            StatusCode::BAD_REQUEST,
            "the bundled ACME resource is only used on Windows",
        );
    }
    if rust_acmesh_executable_path().is_some() {
        response::ok(json!({ "started": false, "bundled": true })).into_response()
    } else {
        response::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "bundled rust-acmesh.exe is missing",
        )
    }
}

#[utoipa::path(post, path = "/api/admin/acme/resource/cancel", tag = "acme", responses((status = 200, description = "Cancelled ACME resource initialization")))]
pub(super) async fn cancel_resource_initialization() -> Response {
    response::ok(json!({ "cancelRequested": false, "bundled": true })).into_response()
}

#[utoipa::path(delete, path = "/api/admin/acme/resource", tag = "acme", responses((status = 200, description = "Deleted ACME resource")))]
pub(super) async fn delete_resource(State(_state): State<AppState>) -> Response {
    response::error(
        StatusCode::BAD_REQUEST,
        "the bundled Windows ACME client cannot be deleted",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_provider_list_matches_native_client() {
        assert_eq!(windows_acme_provider_ids().len(), 11);
        assert!(windows_acme_provider_ids().contains(&"dns_cf"));
        assert!(windows_acme_provider_ids().contains(&"dns_huaweicloud"));
        assert!(!windows_acme_provider_ids().contains(&"dns_dgon"));

        let catalog = windows_acme_dns_providers(&Translator::new("en"))
            .into_iter()
            .filter_map(|provider| {
                provider
                    .get("dnsType")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .collect::<BTreeSet<_>>();
        let supported = windows_acme_provider_ids()
            .iter()
            .map(|value| (*value).to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(catalog, supported);
    }
}
