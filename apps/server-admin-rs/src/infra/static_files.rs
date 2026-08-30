use std::{
    collections::HashMap,
    io::Read,
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime},
};

use crate::{i18n::Translator, response, state::AppState};
use axum::{
    Router,
    body::Body,
    extract::{OriginalUri, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use sha2::{Digest, Sha256};

const AUTH_PUBLIC_PREFIX: &str = "/auth";
const AUTH_LOCAL_PREFIX: &str = "/__auth__";
const INDEX_CACHE_CONTROL: &str = "private, no-store, no-cache, max-age=0, must-revalidate";
const FINGERPRINTED_ASSET_CACHE_CONTROL: &str = "public, max-age=31536000, immutable";
const STATIC_ASSET_CACHE_CONTROL: &str = "public, max-age=300";
const HTTP_DATE_MAX_OFFSET: Duration = Duration::from_secs(253_402_300_800);

#[derive(Clone)]
struct StaticFileVariant {
    path: PathBuf,
    content_length: u64,
    etag: HeaderValue,
}

#[derive(Clone)]
struct StaticFileEntry {
    raw: StaticFileVariant,
    brotli: Option<StaticFileVariant>,
    gzip: Option<StaticFileVariant>,
    content_type: HeaderValue,
    last_modified: Option<HeaderValue>,
}

#[derive(Clone, Default)]
pub(crate) struct StaticFileCatalog {
    entries: HashMap<PathBuf, StaticFileEntry>,
}

#[derive(Clone, Default)]
pub(crate) struct StaticFileCatalogs {
    pub(crate) admin: StaticFileCatalog,
    pub(crate) auth: StaticFileCatalog,
}

impl StaticFileCatalogs {
    pub(crate) fn build(admin_root: &Path, auth_root: &Path) -> Self {
        Self {
            admin: StaticFileCatalog::build(admin_root),
            auth: StaticFileCatalog::build(auth_root),
        }
    }
}

impl StaticFileCatalog {
    fn build(root: &Path) -> Self {
        let mut entries = HashMap::new();
        collect_static_files(root, &mut entries);
        Self { entries }
    }

    fn get(&self, path: &Path) -> Option<&StaticFileEntry> {
        self.entries.get(path)
    }
}

fn collect_static_files(path: &Path, entries: &mut HashMap<PathBuf, StaticFileEntry>) {
    let Ok(children) = std::fs::read_dir(path) else {
        return;
    };
    for child in children.flatten() {
        let path = child.path();
        let Ok(file_type) = child.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_static_files(&path, entries);
            continue;
        }
        if !file_type.is_file()
            || matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("br" | "gz")
            )
        {
            continue;
        }
        if let Some(entry) = static_file_entry(&path) {
            entries.insert(path, entry);
        }
    }
}

fn static_file_variant(path: PathBuf) -> Option<StaticFileVariant> {
    let metadata = std::fs::metadata(&path).ok()?;
    let mut file = std::fs::File::open(&path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    // Release packaging normalizes mtimes for reproducibility, so a
    // size+mtime validator can collide when index.html changes without a size
    // change. A content digest remains stable and representation-specific.
    let etag = HeaderValue::from_str(&format!("\"sha256-{:x}\"", hasher.finalize())).ok()?;
    Some(StaticFileVariant {
        path,
        content_length: metadata.len(),
        etag,
    })
}

fn static_file_entry(path: &Path) -> Option<StaticFileEntry> {
    let raw = static_file_variant(path.to_path_buf())?;
    let metadata = std::fs::metadata(path).ok()?;
    let last_modified = metadata.modified().ok().and_then(last_modified_header);
    let content_type =
        HeaderValue::from_str(mime_guess::from_path(path).first_or_octet_stream().as_ref())
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    StaticFileEntry {
        raw,
        brotli: static_file_variant(PathBuf::from(format!("{}.br", path.display()))),
        gzip: static_file_variant(PathBuf::from(format!("{}.gz", path.display()))),
        content_type,
        last_modified,
    }
    .into()
}

fn last_modified_header(value: SystemTime) -> Option<HeaderValue> {
    let since_epoch = value.duration_since(SystemTime::UNIX_EPOCH).ok()?;
    // httpdate panics outside its supported 1970..9999 range. Filesystem
    // metadata is external input, so omit the optional validator instead of
    // allowing a malformed package timestamp to abort application startup.
    if since_epoch >= HTTP_DATE_MAX_OFFSET {
        return None;
    }
    HeaderValue::from_str(&httpdate::fmt_http_date(value)).ok()
}

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

async fn admin_index(State(state): State<AppState>, request: Request<Body>) -> Response {
    serve_index(
        &state,
        &state.settings.admin_static_path,
        &state.static_files.admin,
        request.headers(),
        request.method(),
    )
    .await
}

async fn auth_index(State(state): State<AppState>, request: Request<Body>) -> Response {
    serve_index(
        &state,
        &state.settings.auth_static_path,
        &state.static_files.auth,
        request.headers(),
        request.method(),
    )
    .await
}

pub async fn auth_fallback(State(state): State<AppState>, req: Request<Body>) -> Response {
    let path = req.uri().path().to_string();
    let normalized_path = normalize_auth_path(&path);
    if is_api_path(&normalized_path) {
        let translator = Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.authRoutes.pathNotFound"),
        );
    }
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return method_not_allowed();
    }

    let Some(asset_path) = auth_asset_path(&state.settings.auth_static_path, &path) else {
        return not_found();
    };
    if let Some(entry) = state.static_files.auth.get(&asset_path) {
        serve_catalog_file(
            &asset_path,
            entry,
            req.headers(),
            req.method(),
            StaticFileKind::Asset,
        )
        .await
    } else if is_asset_request_path(&normalized_path) {
        static_asset_not_found()
    } else if is_known_auth_view_path(&path) {
        serve_index(
            &state,
            &state.settings.auth_static_path,
            &state.static_files.auth,
            req.headers(),
            req.method(),
        )
        .await
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
    if is_api_path(path) {
        let translator = Translator::from_state(&state).await;
        return response::error(
            StatusCode::NOT_FOUND,
            translator.t("server.apiPathNotFound"),
        );
    }
    if req.method() != Method::GET && req.method() != Method::HEAD {
        return method_not_allowed();
    }

    let Some(asset_path) = safe_join(
        &state.settings.admin_static_path,
        path.trim_start_matches('/'),
    ) else {
        return not_found();
    };
    if let Some(entry) = state.static_files.admin.get(&asset_path) {
        serve_catalog_file(
            &asset_path,
            entry,
            req.headers(),
            req.method(),
            StaticFileKind::Asset,
        )
        .await
    } else if is_asset_request_path(path) {
        if is_recoverable_missing_javascript(path) {
            stale_asset_recovery(req.method())
        } else {
            static_asset_not_found()
        }
    } else {
        serve_index(
            &state,
            &state.settings.admin_static_path,
            &state.static_files.admin,
            req.headers(),
            req.method(),
        )
        .await
    }
}

async fn serve_index(
    state: &AppState,
    root: &Path,
    catalog: &StaticFileCatalog,
    request_headers: &HeaderMap,
    method: &Method,
) -> Response {
    let path = root.join("index.html");
    let mut response = match catalog.get(&path) {
        Some(entry) => {
            serve_catalog_file(&path, entry, request_headers, method, StaticFileKind::Index).await
        }
        None => not_found(),
    };
    if let Some(cookie) = locale_cookie(state).await {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

#[cfg(test)]
async fn serve_file(path: PathBuf, request_headers: Option<&HeaderMap>) -> Response {
    let Some(entry) = static_file_entry(&path) else {
        return not_found();
    };
    let empty_headers = HeaderMap::new();
    serve_catalog_file(
        &path,
        &entry,
        request_headers.unwrap_or(&empty_headers),
        &Method::GET,
        StaticFileKind::Asset,
    )
    .await
}

#[cfg(test)]
async fn serve_index_file(root: &Path, request_headers: Option<&HeaderMap>) -> Response {
    let path = root.join("index.html");
    let Some(entry) = static_file_entry(&path) else {
        return not_found();
    };
    let empty_headers = HeaderMap::new();
    serve_catalog_file(
        &path,
        &entry,
        request_headers.unwrap_or(&empty_headers),
        &Method::GET,
        StaticFileKind::Index,
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StaticFileKind {
    Index,
    Asset,
}

async fn serve_catalog_file(
    path: &Path,
    entry: &StaticFileEntry,
    request_headers: &HeaderMap,
    method: &Method,
    kind: StaticFileKind,
) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return method_not_allowed();
    }
    let (variant, content_encoding) = select_precompressed_variant(entry, request_headers);
    if if_none_match_matches(request_headers, &variant.etag) {
        let mut response = StatusCode::NOT_MODIFIED.into_response();
        apply_static_headers(
            &mut response,
            path,
            entry,
            variant,
            content_encoding,
            kind,
            false,
        );
        return response;
    }
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        let Ok(file) = tokio::fs::File::open(&variant.path).await else {
            return not_found();
        };
        Body::from_stream(tokio_util::io::ReaderStream::new(file))
    };
    let mut response = Response::new(body);
    apply_static_headers(
        &mut response,
        path,
        entry,
        variant,
        content_encoding,
        kind,
        true,
    );
    response
}

fn if_none_match_matches(headers: &HeaderMap, etag: &HeaderValue) -> bool {
    let Ok(expected) = etag.to_str() else {
        return false;
    };
    let expected = expected.strip_prefix("W/").unwrap_or(expected);
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(str::trim)
        .any(|candidate| {
            candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == expected
        })
}

fn apply_static_headers(
    response: &mut Response,
    path: &Path,
    entry: &StaticFileEntry,
    variant: &StaticFileVariant,
    content_encoding: Option<&'static str>,
    kind: StaticFileKind,
    include_length: bool,
) {
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, entry.content_type.clone());
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static(cache_control_for_file(path, kind)),
    );
    if kind == StaticFileKind::Index {
        response
            .headers_mut()
            .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
        response
            .headers_mut()
            .insert(header::EXPIRES, HeaderValue::from_static("0"));
        response
            .headers_mut()
            .insert("surrogate-control", HeaderValue::from_static("no-store"));
        response
            .headers_mut()
            .insert("cdn-cache-control", HeaderValue::from_static("no-store"));
    }
    response
        .headers_mut()
        .insert(header::ETAG, variant.etag.clone());
    if let Some(value) = &entry.last_modified {
        response
            .headers_mut()
            .insert(header::LAST_MODIFIED, value.clone());
    }
    if include_length && let Ok(value) = HeaderValue::from_str(&variant.content_length.to_string())
    {
        response.headers_mut().insert(header::CONTENT_LENGTH, value);
    }
    if entry.brotli.is_some() || entry.gzip.is_some() {
        response
            .headers_mut()
            .insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    if let Some(content_encoding) = content_encoding {
        response.headers_mut().insert(
            header::CONTENT_ENCODING,
            HeaderValue::from_static(content_encoding),
        );
    }
    if kind == StaticFileKind::Asset {
        response.headers_mut().insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
    }
}

fn select_precompressed_variant<'a>(
    entry: &'a StaticFileEntry,
    request_headers: &HeaderMap,
) -> (&'a StaticFileVariant, Option<&'static str>) {
    let brotli_quality = entry
        .brotli
        .as_ref()
        .map(|_| accepted_encoding_quality(request_headers, "br"))
        .unwrap_or(0.0);
    let gzip_quality = entry
        .gzip
        .as_ref()
        .map(|_| accepted_encoding_quality(request_headers, "gzip"))
        .unwrap_or(0.0);
    if brotli_quality > 0.0
        && brotli_quality >= gzip_quality
        && let Some(variant) = &entry.brotli
    {
        return (variant, Some("br"));
    }
    if gzip_quality > 0.0
        && let Some(variant) = &entry.gzip
    {
        return (variant, Some("gzip"));
    }
    (&entry.raw, None)
}

async fn locale_cookie(state: &AppState) -> Option<HeaderValue> {
    let locale = state.browser_locale.read().await;
    HeaderValue::from_str(&format!(
        "fn_knock_locale={locale}; Path=/; Max-Age=31536000; SameSite=Lax"
    ))
    .ok()
}

#[cfg(test)]
fn accepts_encoding(header_value: &str, target: &str) -> bool {
    encoding_quality(header_value, target) > 0.0
}

fn accepted_encoding_quality(headers: &HeaderMap, target: &str) -> f32 {
    let mut exact_quality = None;
    let mut wildcard_quality = None;
    for header_value in headers
        .get_all(header::ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
    {
        let (exact, wildcard) = encoding_qualities(header_value, target);
        if exact.is_some() {
            exact_quality = exact;
        }
        if wildcard.is_some() {
            wildcard_quality = wildcard;
        }
    }
    exact_quality.or(wildcard_quality).unwrap_or(0.0)
}

#[cfg(test)]
fn encoding_quality(header_value: &str, target: &str) -> f32 {
    let (exact_quality, wildcard_quality) = encoding_qualities(header_value, target);
    exact_quality.or(wildcard_quality).unwrap_or(0.0)
}

fn encoding_qualities(header_value: &str, target: &str) -> (Option<f32>, Option<f32>) {
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
    (exact_quality, wildcard_quality)
}

fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn is_asset_request_path(path: &str) -> bool {
    path == "/assets" || path.starts_with("/assets/")
}

fn static_asset_not_found() -> Response {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::NOT_FOUND.into_response())
}

fn versioned_asset_from_request(path: &str) -> Option<(&str, &str)> {
    let relative = path.strip_prefix("/assets/")?;
    let (generation, asset) = relative.split_once('/')?;
    if !generation.starts_with('v')
        || generation.len() <= 1
        || !generation
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return None;
    }
    Some((generation, asset))
}

fn is_recoverable_missing_javascript(path: &str) -> bool {
    let Some((_, asset)) = versioned_asset_from_request(path) else {
        return false;
    };
    let asset_path = Path::new(asset);
    matches!(
        asset_path
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("js" | "mjs")
    ) && asset_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(has_fingerprinted_file_name)
}

fn stale_asset_recovery(method: &Method) -> Response {
    const SCRIPT: &str = r#"const url=new URL(window.location.href);const reason=url.searchParams.get("_fn_knock_reload_reason");if(reason==="stale-asset"||reason==="bootstrap"){throw new Error("fn-knock asset recovery was already attempted");}url.searchParams.set("_fn_knock_reload",String(Date.now()));url.searchParams.set("_fn_knock_reload_reason","stale-asset");window.location.replace(url.toString());export {};"#;
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(SCRIPT)
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/javascript; charset=utf-8")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header("x-fn-knock-asset-recovery", "reload")
        .header(header::CONTENT_LENGTH, SCRIPT.len().to_string())
        .body(body)
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
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

fn method_not_allowed() -> Response {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .header(header::ALLOW, "GET, HEAD")
        .body(Body::empty())
        .unwrap_or_else(|_| StatusCode::METHOD_NOT_ALLOWED.into_response())
}

#[cfg(test)]
mod tests {
    use super::{
        StaticFileKind, accepts_encoding, auth_not_found_html, cache_control_for_file,
        has_fingerprinted_file_name, if_none_match_matches, is_api_path, is_asset_request_path,
        is_known_auth_view_path, is_recoverable_missing_javascript, last_modified_header,
        normalize_auth_path, serve_catalog_file, serve_file, serve_index_file,
        stale_asset_recovery, static_asset_not_found, static_file_entry, static_file_variant,
        versioned_asset_from_request,
    };
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
    use std::{
        path::Path,
        time::{Duration, SystemTime},
    };

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
            "private, no-store, no-cache, max-age=0, must-revalidate"
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
    fn asset_request_path_matching_respects_segment_boundaries() {
        assert!(is_asset_request_path("/assets"));
        assert!(is_asset_request_path("/assets/app-ABCDEFG.js"));
        assert!(!is_asset_request_path("/assets-old/app.js"));
        assert!(!is_asset_request_path("/settings/assets"));
    }

    #[test]
    fn missing_static_assets_are_not_replaced_with_the_spa_document() {
        let response = static_asset_not_found();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .unwrap(),
            "nosniff"
        );
    }

    #[test]
    fn missing_versioned_javascript_is_recoverable() {
        assert_eq!(
            versioned_asset_from_request("/assets/v2.4.1/index-OLDHASH.js"),
            Some(("v2.4.1", "index-OLDHASH.js"))
        );
        assert!(is_recoverable_missing_javascript(
            "/assets/v2.4.1/index-OLDHASH.js"
        ));
        assert!(is_recoverable_missing_javascript(
            "/assets/v2.4.2/index-MISSING.js"
        ));
        assert!(!is_recoverable_missing_javascript(
            "/assets/v2.4.1/index-OLDHASH.css"
        ));
        assert!(!is_recoverable_missing_javascript(
            "/assets/index-OLDHASH.js"
        ));
        assert!(!is_recoverable_missing_javascript(
            "/assets/v2.4.1/index.js"
        ));
    }

    #[tokio::test]
    async fn stale_javascript_asset_returns_a_one_shot_reload_module() {
        let response = stale_asset_recovery(&Method::GET);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_TYPE).unwrap(),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
        assert_eq!(
            response.headers().get("x-fn-knock-asset-recovery").unwrap(),
            "reload"
        );
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let script = std::str::from_utf8(&body).unwrap();
        assert!(script.contains("_fn_knock_reload_reason"));
        assert!(script.contains("stale-asset"));
        assert!(script.contains("reason===\"bootstrap\""));
        assert!(script.contains("window.location.replace"));
    }

    #[test]
    fn accept_encoding_honors_quality_zero() {
        assert!(accepts_encoding("gzip, br", "br"));
        assert!(accepts_encoding("*;q=0.5", "br"));
        assert!(!accepts_encoding("br;q=0, gzip", "br"));
        assert!(!accepts_encoding("*;q=1, br;q=0", "br"));
        assert!(!accepts_encoding("br;q=invalid", "br"));
    }

    #[test]
    fn api_path_matching_respects_segment_boundaries() {
        assert!(is_api_path("/api"));
        assert!(is_api_path("/api/status"));
        assert!(!is_api_path("/apix"));
        assert!(!is_api_path("/api-client"));
    }

    #[test]
    fn last_modified_header_rejects_filesystem_times_outside_http_date_range() {
        let synology_epoch_in_utc8 = SystemTime::UNIX_EPOCH - Duration::from_secs(8 * 60 * 60);
        assert!(last_modified_header(synology_epoch_in_utc8).is_none());
        assert_eq!(
            last_modified_header(SystemTime::UNIX_EPOCH)
                .and_then(|value| value.to_str().ok().map(str::to_string))
                .as_deref(),
            Some("Thu, 01 Jan 1970 00:00:00 GMT")
        );
        assert!(
            last_modified_header(SystemTime::UNIX_EPOCH + Duration::from_secs(253_402_300_800))
                .is_none()
        );
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
        assert!(response.headers().contains_key(header::ETAG));
        assert!(response.headers().contains_key(header::LAST_MODIFIED));
    }

    #[tokio::test]
    async fn serve_static_asset_prefers_precompressed_brotli() {
        let temp_dir = tempfile::tempdir().unwrap();
        let asset_path = temp_dir.path().join("app-ABCDEFG.js");
        std::fs::write(&asset_path, "uncompressed").unwrap();
        std::fs::write(format!("{}.br", asset_path.display()), "brotli").unwrap();
        let mut headers = axum::http::HeaderMap::new();
        headers.insert(header::ACCEPT_ENCODING, "gzip, br".parse().unwrap());

        let response = serve_file(asset_path.clone(), Some(&headers)).await;

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

        let identity = serve_file(asset_path, None).await;
        assert!(identity.headers().get(header::CONTENT_ENCODING).is_none());
        assert_eq!(
            identity.headers().get(header::VARY).unwrap(),
            "Accept-Encoding"
        );
    }

    #[tokio::test]
    async fn serve_static_asset_honors_compression_quality_and_method() {
        let temp_dir = tempfile::tempdir().unwrap();
        let asset_path = temp_dir.path().join("app-ABCDEFG.js");
        std::fs::write(&asset_path, "uncompressed").unwrap();
        std::fs::write(format!("{}.br", asset_path.display()), "brotli").unwrap();
        std::fs::write(format!("{}.gz", asset_path.display()), "gzip").unwrap();
        let entry = static_file_entry(&asset_path).unwrap();
        let mut headers = HeaderMap::new();
        headers.append(header::ACCEPT_ENCODING, "br;q=0.2".parse().unwrap());
        headers.append(header::ACCEPT_ENCODING, "gzip;q=0.9".parse().unwrap());

        let response = serve_catalog_file(
            &asset_path,
            &entry,
            &headers,
            &Method::GET,
            StaticFileKind::Asset,
        )
        .await;
        assert_eq!(
            response.headers().get(header::CONTENT_ENCODING).unwrap(),
            "gzip"
        );

        let response = serve_catalog_file(
            &asset_path,
            &entry,
            &headers,
            &Method::POST,
            StaticFileKind::Asset,
        )
        .await;
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn serve_index_sets_node_headers() {
        let temp_dir = tempfile::tempdir().unwrap();
        std::fs::write(temp_dir.path().join("index.html"), "<!doctype html>").unwrap();

        let response = serve_index_file(temp_dir.path(), None).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|value| value.to_str().ok()),
            Some("private, no-store, no-cache, max-age=0, must-revalidate")
        );
        assert_eq!(
            response
                .headers()
                .get(header::PRAGMA)
                .and_then(|value| value.to_str().ok()),
            Some("no-cache")
        );
        assert_eq!(
            response
                .headers()
                .get("surrogate-control")
                .and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
        assert!(
            response
                .headers()
                .get(header::X_CONTENT_TYPE_OPTIONS)
                .is_none()
        );
    }

    #[tokio::test]
    async fn static_catalog_handles_conditional_and_head_requests_without_opening_body() {
        let temp_dir = tempfile::tempdir().unwrap();
        let asset_path = temp_dir.path().join("app-ABCDEFG.js");
        std::fs::write(&asset_path, "console.log('ok');").unwrap();
        let entry = static_file_entry(&asset_path).unwrap();
        let mut conditional = HeaderMap::new();
        conditional.insert(header::IF_NONE_MATCH, entry.raw.etag.clone());
        let response = serve_catalog_file(
            &asset_path,
            &entry,
            &conditional,
            &Method::GET,
            StaticFileKind::Asset,
        )
        .await;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
        assert!(!response.headers().contains_key(header::CONTENT_LENGTH));
        let strong_etag = entry.raw.etag.to_str().unwrap().trim_start_matches("W/");
        conditional.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&format!("\"unrelated\", {strong_etag}")).unwrap(),
        );
        assert!(if_none_match_matches(&conditional, &entry.raw.etag));

        let response = serve_catalog_file(
            &asset_path,
            &entry,
            &HeaderMap::new(),
            &Method::HEAD,
            StaticFileKind::Asset,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CONTENT_LENGTH).unwrap(),
            "18"
        );
    }

    #[test]
    fn static_etag_changes_for_equal_length_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let asset_path = temp_dir.path().join("app.js");
        std::fs::write(&asset_path, "aaaa").unwrap();
        let first = static_file_variant(asset_path.clone()).unwrap();
        std::fs::write(&asset_path, "bbbb").unwrap();
        let second = static_file_variant(asset_path).unwrap();
        assert_ne!(first.etag, second.etag);
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
