use super::*;

// A read-only HTTPS endpoint exercises proxy DNS, CONNECT and TLS without DNS writes.
const PROXY_TEST_URL: &str = "https://www.cloudflare.com/cdn-cgi/trace";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct UpdateProxyTestBody {
    update_proxy: DDNSUpdateProxyUpdate,
    http_transport: Option<String>,
}

#[utoipa::path(post, path = "/api/admin/ddns/update-proxy/test", tag = "ddns", operation_id = "post_api_admin_ddns_update_proxy_test", responses((status = 200, description = "Read-only DDNS proxy connectivity test")))]
pub(super) async fn test_update_proxy(
    State(state): State<AppState>,
    Json(body): Json<UpdateProxyTestBody>,
) -> Response {
    run_update_proxy_test(&state, body, PROXY_TEST_URL).await
}

async fn run_update_proxy_test(state: &AppState, body: UpdateProxyTestBody, url: &str) -> Response {
    let translator = Translator::from_state(state).await;
    let current = match state.storage.store.get_string_value(DDNS_SETTINGS).await {
        Ok(raw) => parse_settings(raw.as_deref()),
        Err(_) => {
            return response::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ddns_text(&translator, "settingsLoadFailed", &[]),
            );
        }
    };
    let proxy = DDNSUpdateProxy::from_settings(&current)
        .merge(body.update_proxy)
        .and_then(|mut proxy| {
            proxy.validate(true)?;
            Ok(proxy)
        });
    let proxy = match proxy {
        Ok(proxy) => proxy,
        Err(error) => {
            return response::error(
                StatusCode::BAD_REQUEST,
                localize_ddns_error(&translator, &error.to_string()),
            );
        }
    };
    // Explicit proxy selection is independent of the saved/default-enabled switch.
    let options = DDNSHttpClientOptions {
        transport: merge_http_transport_update(body.http_transport.as_deref(), &current),
        network_interface: String::new(),
        update_proxy: Some(proxy),
    };
    let started = std::time::Instant::now();
    let result = tokio_time::timeout(Duration::from_secs(12), async {
        let client = ddns_http_client_no_redirects(&translator, &options)?;
        client.get(url).send().await
    })
    .await;
    let (success, status, error) = match result {
        Ok(Ok(response)) => {
            let status = response.status();
            (
                status.is_success(),
                Some(status.as_u16()),
                (!status.is_success()).then(|| {
                    ddns_text(
                        &translator,
                        "updateProxyTestHttpFailed",
                        &[("status", status.as_u16().to_string())],
                    )
                }),
            )
        }
        Ok(Err(error)) => (false, None, Some(error.to_string())),
        Err(_) => (
            false,
            None,
            Some(ddns_text(&translator, "updateProxyTimeout", &[])),
        ),
    };
    response::ok(
        json!({ "success": success, "status": status, "error": error,
        "elapsedMs": started.elapsed().as_millis() as u64, "url": url }),
    )
    .into_response()
}

#[cfg(test)]
#[path = "update_proxy_test_tests.rs"]
mod tests;
