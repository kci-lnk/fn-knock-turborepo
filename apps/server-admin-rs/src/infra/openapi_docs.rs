use axum::{
    Json, Router,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::{app_version::APP_LOCAL_VERSION, state::AppState};

const GENERATED_ROUTES_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/openapi_routes.json"));

#[derive(Debug, Clone, Deserialize)]
struct GeneratedRoute {
    method: String,
    path: String,
}

pub fn openapi_docs_routes() -> Router<AppState> {
    Router::new()
        .route("/docs", get(docs_html))
        .route("/docs/", get(docs_html))
        .route("/docs/json", get(docs_json))
}

async fn docs_json() -> Response {
    Json(build_openapi_document()).into_response()
}

async fn docs_html() -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(axum::body::Body::from(swagger_html()))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

fn build_openapi_document() -> Value {
    let routes = generated_routes();
    let mut paths = Map::new();
    for route in routes {
        let path_entry = paths
            .entry(route.path.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        let Some(methods) = path_entry.as_object_mut() else {
            continue;
        };
        methods.insert(
            route.method.to_ascii_lowercase(),
            json!({
                "summary": format!("{} {}", route.method, route.path),
                "tags": [route_tag(&route.path)],
                "responses": {
                    "200": {
                        "description": "Successful response"
                    }
                }
            }),
        );
    }

    json!({
        "openapi": "3.0.3",
        "info": {
            "title": "fn-knock server-admin API",
            "version": APP_LOCAL_VERSION,
            "description": "server-admin 7998 端口提供的管理端接口文档。"
        },
        "servers": [
            {
                "url": "/",
                "description": "server-admin (port 7998)"
            }
        ],
        "paths": Value::Object(paths),
    })
}

fn generated_routes() -> Vec<GeneratedRoute> {
    serde_json::from_str(GENERATED_ROUTES_JSON).unwrap_or_default()
}

fn route_tag(path: &str) -> String {
    path.strip_prefix("/api/admin/")
        .or_else(|| path.strip_prefix("/api/internal/"))
        .and_then(|rest| rest.split('/').next())
        .filter(|value| !value.is_empty())
        .unwrap_or("admin")
        .to_string()
}

fn swagger_html() -> &'static str {
    r##"<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>fn-knock server-admin API</title>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui.css" />
    <style>body{margin:0;background:#fff}</style>
  </head>
  <body>
    <div id="swagger-ui"></div>
    <script src="https://cdn.jsdelivr.net/npm/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
    <script>
      window.ui = SwaggerUIBundle({
        url: "/docs/json",
        dom_id: "#swagger-ui",
        docExpansion: "list",
        deepLinking: true,
        persistAuthorization: true,
        displayRequestDuration: true
      });
    </script>
  </body>
</html>"#
    "##
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_openapi_document_contains_admin_routes() {
        let document = build_openapi_document();
        assert_eq!(document["openapi"], json!("3.0.3"));
        assert_eq!(
            document.pointer("/paths/~1api~1admin~1config/get"),
            Some(&json!({
                "summary": "GET /api/admin/config",
                "tags": ["config"],
                "responses": {
                    "200": {
                        "description": "Successful response"
                    }
                }
            }))
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1maintenance~1backup~1export~1fnos/post")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1maintenance~1backup~1automatic/get")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1maintenance~1backup~1automatic/put")
                .is_some()
        );
        assert!(
            document
                .pointer("/paths/~1api~1admin~1maintenance~1backup~1import~1automatic/post")
                .is_some()
        );
        assert!(document.pointer("/paths/~1api~1auth~1login").is_none());
    }

    #[test]
    fn route_tags_follow_first_admin_segment() {
        assert_eq!(route_tag("/api/admin/ddns/status"), "ddns");
        assert_eq!(route_tag("/api/internal/system-events"), "system-events");
    }
}
