use std::path::{Component, Path, PathBuf};

use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;

use crate::{i18n::Translator, response, state::AppState};

const AUTH_PUBLIC_PREFIX: &str = "/auth";
const AUTH_LOCAL_PREFIX: &str = "/__auth__";
const INDEX_CACHE_CONTROL: &str = "no-cache";
const FINGERPRINTED_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const STATIC_ASSET_CACHE_CONTROL: &str = "public, max-age=300";

pub fn admin_static_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(admin_index))
        .route("/index.html", get(admin_index))
}

pub fn auth_static_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(auth_index))
        .route("/index.html", get(auth_index))
        .route("/auth", get(auth_index))
        .route("/auth/", get(auth_index))
        .route("/auth/index.html", get(auth_index))
        .route("/__auth__", get(auth_index))
        .route("/__auth__/", get(auth_index))
        .route("/__auth__/index.html", get(auth_index))
        .route("/__fn-knock/runtime-hmac-secret", get(runtime_hmac_secret))
        .route(
            "/auth/__fn-knock/runtime-hmac-secret",
            get(runtime_hmac_secret),
        )
        .route(
            "/__auth__/__fn-knock/runtime-hmac-secret",
            get(runtime_hmac_secret),
        )
        .route("/oidc/bind", get(redirect_legacy_oidc_bind_route))
        .route("/oidc/bind/", get(redirect_legacy_oidc_bind_route))
        .route("/auth/oidc/bind", get(redirect_legacy_oidc_bind_route))
        .route("/auth/oidc/bind/", get(redirect_legacy_oidc_bind_route))
        .route("/__auth__/oidc/bind", get(redirect_legacy_oidc_bind_route))
        .route("/__auth__/oidc/bind/", get(redirect_legacy_oidc_bind_route))
}

async fn admin_index(State(state): State<AppState>) -> Response {
    serve_index(&state.settings.admin_static_path, None).await
}

async fn auth_index(State(state): State<AppState>) -> Response {
    serve_index(
        &state.settings.auth_static_path,
        Some(index_injection_script(&state)),
    )
    .await
}

async fn runtime_hmac_secret(State(state): State<AppState>) -> Response {
    if !state.settings.expose_runtime_hmac_secret {
        return runtime_hmac_secret_not_found_response();
    }

    (
        [(header::CACHE_CONTROL, HeaderValue::from_static("no-store"))],
        axum::Json(json!({
            "success": true,
            "data": {
                "hmacSecret": state.settings.hmac_secret,
                "secret": state.settings.hmac_secret
            }
        })),
    )
        .into_response()
}

async fn redirect_legacy_oidc_bind_route(OriginalUri(original_uri): OriginalUri) -> Response {
    let path = original_uri.path();
    let base_prefix = if path.starts_with("/__auth__/") {
        "/__auth__"
    } else if path.starts_with("/auth/") {
        "/auth"
    } else {
        ""
    };
    let query = original_uri
        .query()
        .map(|query| format!("?{query}"))
        .unwrap_or_default();
    let location = format!("{base_prefix}/api/auth/oidc/bind{query}");
    (
        StatusCode::FOUND,
        [
            (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            (
                header::LOCATION,
                HeaderValue::from_str(&location)
                    .unwrap_or_else(|_| HeaderValue::from_static("/api/auth/oidc/bind")),
            ),
        ],
    )
        .into_response()
}

pub async fn auth_fallback(State(state): State<AppState>, req: Request<Body>) -> Response {
    let path = req.uri().path().to_string();
    let normalized_path = normalize_auth_path(&path);
    if normalized_path.starts_with("/api") {
        let translator = Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.authRoutes.pathNotFound"),
        );
    }

    let Some(asset_path) = auth_asset_path(&state.settings.auth_static_path, &path) else {
        return not_found();
    };
    if asset_path.is_file() {
        serve_file(asset_path, None).await
    } else if is_known_auth_view_path(&path) {
        serve_index(
            &state.settings.auth_static_path,
            Some(index_injection_script(&state)),
        )
        .await
    } else {
        auth_not_found_html()
    }
}

pub async fn admin_fallback(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    _req: Request<Body>,
) -> Response {
    let path = original_uri.path();
    if path.starts_with("/api/") {
        let translator = Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.apiPathNotFound"),
        );
    }

    let Some(asset_path) = safe_join(
        &state.settings.admin_static_path,
        path.trim_start_matches('/'),
    ) else {
        return not_found();
    };
    if asset_path.is_file() {
        serve_file(asset_path, None).await
    } else {
        serve_index(&state.settings.admin_static_path, None).await
    }
}

async fn serve_index(root: &Path, injection: Option<String>) -> Response {
    serve_file_with_kind(root.join("index.html"), injection, StaticFileKind::Index).await
}

async fn serve_file(path: PathBuf, injection: Option<String>) -> Response {
    serve_file_with_kind(path, injection, StaticFileKind::Asset).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticFileKind {
    Index,
    Asset,
}

async fn serve_file_with_kind(
    path: PathBuf,
    injection: Option<String>,
    kind: StaticFileKind,
) -> Response {
    let Ok(bytes) = tokio::fs::read(&path).await else {
        return not_found();
    };

    let mut response_bytes = bytes;
    if let Some(script) = injection {
        if path.file_name().and_then(|name| name.to_str()) == Some("index.html") {
            let mut html = String::from_utf8_lossy(&response_bytes).into_owned();
            if html.contains("</head>") {
                html = html.replacen("</head>", &format!("{script}</head>"), 1);
            } else {
                html.push_str(&script);
            }
            response_bytes = html.into_bytes();
        }
    }

    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let mut response = Response::new(Body::from(response_bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control_for_file(&path, kind)),
    );
    if kind == StaticFileKind::Asset {
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
    }
    response
}

fn cache_control_for_file(path: &Path, kind: StaticFileKind) -> &'static str {
    match kind {
        StaticFileKind::Index => INDEX_CACHE_CONTROL,
        StaticFileKind::Asset if is_fingerprinted_asset_path(path) => {
            FINGERPRINTED_ASSET_CACHE_CONTROL
        }
        StaticFileKind::Asset => STATIC_ASSET_CACHE_CONTROL,
    }
}

fn is_fingerprinted_asset_path(path: &Path) -> bool {
    if !path.components().any(
        |component| matches!(component, Component::Normal(part) if part.to_str() == Some("assets")),
    ) {
        return false;
    }
    let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    has_fingerprinted_file_name(file_name)
}

fn has_fingerprinted_file_name(file_name: &str) -> bool {
    let Some(dot_index) = file_name.rfind('.') else {
        return false;
    };
    if dot_index == 0 || dot_index + 1 >= file_name.len() {
        return false;
    }
    let stem = &file_name[..dot_index];
    stem.match_indices('-').any(|(dash_index, _)| {
        let fingerprint = &stem[dash_index + 1..];
        fingerprint.len() >= 7
            && fingerprint
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    })
}

fn auth_asset_path(root: &Path, path: &str) -> Option<PathBuf> {
    let normalized = path
        .strip_prefix(&format!("{AUTH_LOCAL_PREFIX}/"))
        .or_else(|| path.strip_prefix(&format!("{AUTH_PUBLIC_PREFIX}/")))
        .unwrap_or_else(|| path.trim_start_matches('/'));
    safe_join(root, normalized)
}

fn normalize_auth_path(path: &str) -> String {
    if matches!(
        path,
        AUTH_PUBLIC_PREFIX | "/auth/" | AUTH_LOCAL_PREFIX | "/__auth__/"
    ) {
        return "/".to_string();
    }
    path.strip_prefix(&format!("{AUTH_PUBLIC_PREFIX}/"))
        .or_else(|| path.strip_prefix(&format!("{AUTH_LOCAL_PREFIX}/")))
        .map(|value| format!("/{value}"))
        .unwrap_or_else(|| path.to_string())
}

fn is_known_auth_view_path(path: &str) -> bool {
    matches!(
        normalize_auth_path(path).as_str(),
        "/" | "/index.html" | "/login" | "/login/" | "/oidc/bind" | "/oidc/bind/"
    )
}

fn safe_join(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() {
        return Some(root.join("index.html"));
    }
    let mut path = PathBuf::from(root);
    for component in Path::new(relative).components() {
        match component {
            Component::Normal(part) => path.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(path)
}

fn index_injection_script(state: &AppState) -> String {
    hmac_secret_injection_script(&state.settings.hmac_secret)
}

fn hmac_secret_injection_script(hmac_secret: &str) -> String {
    format!(
        "<script>window.__FN_KNOCK_HMAC_SECRET__={};</script>",
        serde_json::to_string(hmac_secret).unwrap_or_else(|_| "\"\"".to_string())
    )
}

fn runtime_hmac_secret_not_found_response() -> Response {
    let mut response = axum::Json(json!({
        "success": false,
        "message": "Not found"
    }))
    .into_response();
    *response.status_mut() = StatusCode::NOT_FOUND;
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn auth_not_found_html() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from(
            r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>404</title><style>body{margin:0;min-height:100vh;display:grid;place-items:center;font-family:ui-sans-serif,system-ui,sans-serif;color:#111;background:#fff}.wrap{text-align:center}h1{margin:0;font-size:3rem;line-height:1;font-weight:600}p{margin:.75rem 0 0;color:#666;font-size:.875rem}</style></head><body><main class="wrap"><h1>404</h1><p>Page not found</p></main></body></html>"#,
        ))
        .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response())
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        StaticFileKind, auth_not_found_html, cache_control_for_file, has_fingerprinted_file_name,
        hmac_secret_injection_script, is_known_auth_view_path, normalize_auth_path,
        runtime_hmac_secret_not_found_response, serve_file, serve_index,
    };
    use axum::http::{StatusCode, header};
    use std::path::Path;

    #[test]
    fn hmac_secret_injection_script_serializes_secret_for_html() {
        assert_eq!(
            hmac_secret_injection_script("secret-with-\"quote\""),
            "<script>window.__FN_KNOCK_HMAC_SECRET__=\"secret-with-\\\"quote\\\"\";</script>"
        );
    }

    #[test]
    fn auth_view_fallback_paths_match_node() {
        assert_eq!(normalize_auth_path("/auth/login"), "/login");
        assert_eq!(normalize_auth_path("/__auth__/oidc/bind/"), "/oidc/bind/");
        assert!(is_known_auth_view_path("/auth/login"));
        assert!(is_known_auth_view_path("/__auth__/oidc/bind/"));
        assert!(!is_known_auth_view_path("/auth/unknown"));
        assert!(!is_known_auth_view_path("/assets/missing.js"));
    }

    #[test]
    fn static_asset_cache_control_matches_node() {
        assert_eq!(
            cache_control_for_file(
                Path::new("/tmp/dist/assets/framework-E0douY43.js"),
                StaticFileKind::Asset
            ),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_control_for_file(
                Path::new("/tmp/dist/assets/useAsyncAction-PIbIqKz-.js"),
                StaticFileKind::Asset
            ),
            "public, max-age=31536000, immutable"
        );
        assert_eq!(
            cache_control_for_file(Path::new("/tmp/dist/favicon.ico"), StaticFileKind::Asset),
            "public, max-age=300"
        );
        assert_eq!(
            cache_control_for_file(Path::new("/tmp/dist/index.html"), StaticFileKind::Index),
            "no-cache"
        );
        assert_eq!(
            cache_control_for_file(
                Path::new("/tmp/dist/assets/index.html"),
                StaticFileKind::Asset
            ),
            "public, max-age=300"
        );
    }

    #[test]
    fn fingerprinted_file_name_rule_matches_node() {
        assert!(has_fingerprinted_file_name("framework-E0douY43.js"));
        assert!(has_fingerprinted_file_name("useAsyncAction-PIbIqKz-.js"));
        assert!(has_fingerprinted_file_name(
            "__vite-browser-external-2447137e-B7sh2Mfd.js"
        ));
        assert!(!has_fingerprinted_file_name("favicon.ico"));
        assert!(!has_fingerprinted_file_name("app-short.js"));
    }

    #[tokio::test]
    async fn serve_static_asset_sets_node_headers() {
        let temp_dir = tempfile::tempdir().unwrap();
        let assets_dir = temp_dir.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let asset_path = assets_dir.join("app-ABCDEFG.js");
        std::fs::write(&asset_path, "console.log('ok');").unwrap();

        let response = serve_file(asset_path, None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("public, max-age=31536000, immutable")
        );
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
    }

    #[tokio::test]
    async fn serve_index_sets_node_headers() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("index.html"), "<!doctype html>").unwrap();

        let response = serve_index(temp_dir.path(), None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        assert!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .is_none()
        );
    }

    #[test]
    fn auth_not_found_html_matches_node_boundary() {
        let response = auth_not_found_html();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
    }

    #[test]
    fn runtime_hmac_secret_disabled_response_matches_node() {
        let response = runtime_hmac_secret_not_found_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
    }
}
