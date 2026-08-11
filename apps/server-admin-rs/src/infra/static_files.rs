use std::path::{Component, Path, PathBuf};

use crate::{i18n::Translator, response, state::AppState};
use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderValue, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

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
}

async fn admin_index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    serve_index(&state.settings.admin_static_path, Some(&headers)).await
}

async fn auth_index(State(state): State<AppState>, headers: HeaderMap) -> Response {
    serve_index(&state.settings.auth_static_path, Some(&headers)).await
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
        serve_file(asset_path, Some(req.headers())).await
    } else if is_known_auth_view_path(&path) {
        serve_index(&state.settings.auth_static_path, Some(req.headers())).await
    } else {
        auth_not_found_html()
    }
}

pub async fn admin_fallback(
    State(state): State<AppState>,
    OriginalUri(original_uri): OriginalUri,
    req: Request<Body>,
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
        serve_file(asset_path, Some(req.headers())).await
    } else {
        serve_index(&state.settings.admin_static_path, Some(req.headers())).await
    }
}

async fn serve_index(root: &Path, request_headers: Option<&HeaderMap>) -> Response {
    serve_file_with_kind(
        root.join("index.html"),
        request_headers,
        StaticFileKind::Index,
    )
    .await
}

async fn serve_file(path: PathBuf, request_headers: Option<&HeaderMap>) -> Response {
    serve_file_with_kind(path, request_headers, StaticFileKind::Asset).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticFileKind {
    Index,
    Asset,
}

async fn serve_file_with_kind(
    path: PathBuf,
    request_headers: Option<&HeaderMap>,
    kind: StaticFileKind,
) -> Response {
    let (served_path, content_encoding) = select_precompressed_path(&path, request_headers).await;
    let Ok(file) = tokio::fs::File::open(&served_path).await else {
        return not_found();
    };
    let content_length = file.metadata().await.ok().map(|metadata| metadata.len());
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let stream = tokio_util::io::ReaderStream::new(file);
    let mut response = Response::new(Body::from_stream(stream));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(mime.as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control_for_file(&path, kind)),
    );
    if let Some(content_length) = content_length
        && let Ok(value) = HeaderValue::from_str(&content_length.to_string())
    {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    if let Some(content_encoding) = content_encoding {
        response.headers_mut().insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static(content_encoding),
        );
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    if kind == StaticFileKind::Asset {
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
    }
    response
}

async fn select_precompressed_path(
    path: &Path,
    request_headers: Option<&HeaderMap>,
) -> (PathBuf, Option<&'static str>) {
    let accepted = request_headers
        .and_then(|headers| headers.get(header::ACCEPT_ENCODING))
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    for (encoding, suffix) in [("br", ".br"), ("gzip", ".gz")] {
        if !accepts_encoding(accepted, encoding) {
            continue;
        }
        let candidate = PathBuf::from(format!("{}{suffix}", path.display()));
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return (candidate, Some(encoding));
        }
    }
    (path.to_path_buf(), None)
}

fn accepts_encoding(header_value: &str, target: &str) -> bool {
    let mut exact_quality = None;
    let mut wildcard_quality = None;
    for entry in header_value.split(',') {
        let mut parts = entry.trim().split(';');
        let encoding = parts.next().unwrap_or_default().trim();
        let mut quality = 1.0;
        for parameter in parts {
            let Some((name, value)) = parameter.trim().split_once('=') else {
                continue;
            };
            if name.trim().eq_ignore_ascii_case("q") {
                quality = value
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .filter(|quality| (0.0..=1.0).contains(quality))
                    .unwrap_or(0.0);
            }
        }
        if encoding.eq_ignore_ascii_case(target) {
            exact_quality = Some(quality);
        } else if encoding == "*" {
            wildcard_quality = Some(quality);
        }
    }
    exact_quality.or(wildcard_quality).unwrap_or(0.0) > 0.0
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
        "/" | "/index.html"
            | "/login"
            | "/login/"
            | "/oidc/bind"
            | "/oidc/bind/"
            | "/ldap/bind"
            | "/ldap/bind/"
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
        StaticFileKind, accepts_encoding, auth_not_found_html, cache_control_for_file,
        has_fingerprinted_file_name, is_known_auth_view_path, normalize_auth_path, serve_file,
        serve_index,
    };
    use axum::http::{StatusCode, header};
    use std::path::Path;

    #[test]
    fn auth_view_fallback_paths_match_node() {
        assert_eq!(normalize_auth_path("/auth/login"), "/login");
        assert_eq!(normalize_auth_path("/__auth__/oidc/bind/"), "/oidc/bind/");
        assert!(is_known_auth_view_path("/auth/login"));
        assert!(is_known_auth_view_path("/__auth__/oidc/bind/"));
        assert!(is_known_auth_view_path("/auth/ldap/bind"));
        assert!(is_known_auth_view_path("/__auth__/ldap/bind/"));
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

    #[test]
    fn accept_encoding_honors_quality_zero() {
        assert!(accepts_encoding("gzip, br", "br"));
        assert!(accepts_encoding("*;q=0.5", "br"));
        assert!(!accepts_encoding("br;q=0, gzip", "br"));
        assert!(!accepts_encoding("*;q=1, br;q=0", "br"));
        assert!(!accepts_encoding("br;q=invalid", "br"));
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
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok()),
            Some("18")
        );
    }

    #[tokio::test]
    async fn serve_static_asset_prefers_precompressed_brotli() {
        let temp_dir = tempfile::tempdir().unwrap();
        let asset_path = temp_dir.path().join("app-ABCDEFG.js");
        std::fs::write(&asset_path, "uncompressed").unwrap();
        std::fs::write(format!("{}.br", asset_path.display()), "brotli").unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, "gzip, br".parse().unwrap());

        let response = serve_file(asset_path, Some(&headers)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CONTENT_ENCODING)
                .and_then(|value| value.to_str().ok()),
            Some("br")
        );
        assert_eq!(
            response
                .headers()
                .get(header::VARY)
                .and_then(|value| value.to_str().ok()),
            Some("Accept-Encoding")
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
}
