use super::*;

pub(super) async fn probe_basic_auth_target(input_url: &str, translator: &Translator) -> Value {
    let Some(normalized_url) = normalize_http_probe_url(input_url) else {
        return json!({
            "requiresBasicAuth": false,
            "httpStatus": Value::Null,
            "error": admin_config_text(translator, "hostMappings.onlyHttpTargetsSupported"),
        });
    };
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return json!({
                "requiresBasicAuth": false,
                "httpStatus": Value::Null,
                "error": error.to_string(),
            });
        }
    };

    match client
        .get(normalized_url)
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,*/*;q=0.8",
        )
        .header(reqwest::header::USER_AGENT, BASIC_AUTH_PROBE_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .send()
        .await
    {
        Ok(response) => json!({
            "requiresBasicAuth": has_basic_auth_challenge(
                response
                    .headers()
                    .get(reqwest::header::WWW_AUTHENTICATE)
                    .and_then(|value| value.to_str().ok())
            ),
            "httpStatus": i64::from(response.status().as_u16()),
        }),
        Err(error) => json!({
            "requiresBasicAuth": false,
            "httpStatus": Value::Null,
            "error": error.to_string(),
        }),
    }
}

pub(super) fn has_basic_auth_challenge(www_authenticate: Option<&str>) -> bool {
    www_authenticate
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .any(|value| {
            value.eq_ignore_ascii_case("basic") || value.to_ascii_lowercase().starts_with("basic ")
        })
}

pub(super) fn normalize_http_probe_url(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    let mut parsed = Url::parse(trimmed).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return None;
    }
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

pub(super) async fn fetch_host_mapping_metadata(
    target: &str,
    basic_auth: Option<&Value>,
) -> Result<Value, String> {
    let normalized_url = normalize_http_probe_url(target)
        .ok_or_else(|| "Only http/https targets are supported".to_string())?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
        .map_err(|error| error.to_string())?;
    let basic_auth_context = create_basic_auth_context(basic_auth, &normalized_url);
    let response = send_metadata_get(
        &client,
        &normalized_url,
        "text/html,application/xhtml+xml,*/*;q=0.8",
        basic_auth_context.as_ref(),
    )
    .await
    .map_err(|error| error.to_string())?;
    let is_luci_login_required = is_openwrt_luci_login_required_response(&response);
    if !response.status().is_success() && !is_luci_login_required {
        return Err(format!(
            "Upstream responded with {}",
            response.status().as_u16()
        ));
    }

    let initial_document = read_metadata_document(response).await?;
    let document =
        fetch_openwrt_luci_document(&client, initial_document, basic_auth_context.as_ref())
            .await
            .unwrap_or_else(|document| document);
    let title = extract_html_title(&document.html);

    let one_panel_favicon = if is_one_panel_loading_title(&title) {
        if let Some(favicon_url) =
            resolve_origin_path_url(&document.final_url, ONE_PANEL_FAVICON_PATH)
        {
            fetch_favicon_as_data_url(&client, &favicon_url, basic_auth_context.as_ref())
                .await
                .unwrap_or_default()
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    if !one_panel_favicon.is_empty() {
        return Ok(json!({
            "title": ONE_PANEL_TITLE,
            "favicon": one_panel_favicon,
            "finalUrl": document.final_url,
        }));
    }

    let html_base_url = extract_html_base_url(&document.html, &document.final_url);
    let explicit_favicon_urls =
        extract_explicit_favicon_urls_from_html(&document.html, &html_base_url);
    let strong_heuristic_favicon_urls = extract_heuristic_favicon_urls_from_html(
        &document.html,
        &html_base_url,
        STRONG_HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    let weak_heuristic_favicon_urls = extract_heuristic_favicon_urls_from_html(
        &document.html,
        &html_base_url,
        HEURISTIC_FAVICON_MIN_PRIORITY,
    );
    let manifest_url = extract_manifest_from_html(&document.html, &html_base_url);
    let mut favicon_budget = FaviconFetchBudget {
        remaining: MAX_FAVICON_FETCH_ATTEMPTS,
        seen: HashSet::new(),
    };
    let mut favicon = fetch_first_favicon_as_data_url(
        &client,
        &explicit_favicon_urls,
        basic_auth_context.as_ref(),
        &mut favicon_budget,
        FALLBACK_FAVICON_FETCH_RESERVE,
    )
    .await;
    if favicon.is_empty() {
        if let Some(manifest_url) = manifest_url {
            let manifest_icons =
                fetch_manifest_icon_urls(&client, &manifest_url, basic_auth_context.as_ref()).await;
            favicon = fetch_first_favicon_as_data_url(
                &client,
                &manifest_icons,
                basic_auth_context.as_ref(),
                &mut favicon_budget,
                FALLBACK_FAVICON_FETCH_RESERVE,
            )
            .await;
        }
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &strong_heuristic_favicon_urls,
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            FALLBACK_FAVICON_FETCH_RESERVE,
        )
        .await;
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &resolve_fallback_favicon_urls(&document.final_url),
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            0,
        )
        .await;
    }
    if favicon.is_empty() {
        favicon = fetch_first_favicon_as_data_url(
            &client,
            &weak_heuristic_favicon_urls,
            basic_auth_context.as_ref(),
            &mut favicon_budget,
            0,
        )
        .await;
    }

    Ok(json!({
        "title": title,
        "favicon": favicon,
        "finalUrl": document.final_url,
    }))
}

pub(super) fn usable_basic_auth(value: Option<&Value>) -> Option<(String, String)> {
    let object = value?.as_object()?;
    if object.get("enabled").and_then(Value::as_bool) != Some(true) {
        return None;
    }
    let username = object
        .get("username")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let password = object.get("password").and_then(Value::as_str).unwrap_or("");
    if username.is_empty() || password.is_empty() || username.contains(':') {
        return None;
    }
    Some((username.to_string(), password.to_string()))
}

pub(super) fn create_basic_auth_context(
    value: Option<&Value>,
    target_url: &str,
) -> Option<MetadataBasicAuthContext> {
    let (username, password) = usable_basic_auth(value)?;
    Some(MetadataBasicAuthContext {
        origin: Url::parse(target_url).ok()?.origin().ascii_serialization(),
        username,
        password,
    })
}

pub(super) fn has_same_origin(value: &str, origin: &str) -> bool {
    Url::parse(value)
        .map(|url| url.origin().ascii_serialization() == origin)
        .unwrap_or(false)
}

pub(super) fn apply_basic_auth_context(
    request: reqwest::RequestBuilder,
    url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> reqwest::RequestBuilder {
    if let Some(context) = basic_auth
        && has_same_origin(url, &context.origin)
    {
        return request.basic_auth(context.username.clone(), Some(context.password.clone()));
    }
    request
}

pub(super) async fn send_metadata_get(
    client: &reqwest::Client,
    url: &str,
    accept: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> reqwest::Result<reqwest::Response> {
    let request = client
        .get(url)
        .header(reqwest::header::ACCEPT, accept)
        .header(reqwest::header::USER_AGENT, METADATA_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close");
    apply_basic_auth_context(request, url, basic_auth)
        .send()
        .await
}

pub(super) async fn read_metadata_document(
    response: reqwest::Response,
) -> Result<MetadataHtmlDocument, String> {
    let final_url = response.url().to_string();
    let html = read_response_text_limited(response, MAX_METADATA_HTML_BYTES).await?;
    Ok(MetadataHtmlDocument { html, final_url })
}

pub(super) async fn read_response_text_limited(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<String, String> {
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    let byte_len = bytes.len().min(max_bytes);
    Ok(String::from_utf8_lossy(&bytes[..byte_len])
        .trim_start_matches('\u{feff}')
        .to_string())
}

pub(super) async fn fetch_favicon_as_data_url(
    client: &reqwest::Client,
    favicon_url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Option<String> {
    let trimmed_url = favicon_url.trim();
    if trimmed_url.to_ascii_lowercase().starts_with("data:image/") {
        return (trimmed_url.len() <= MAX_FAVICON_BYTES * 2).then(|| trimmed_url.to_string());
    }

    let normalized_url = normalize_http_probe_url(trimmed_url)?;
    let response = send_metadata_get(client, &normalized_url, "image/*,*/*;q=0.8", basic_auth)
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    if response_content_length_exceeds(response.headers(), MAX_FAVICON_BYTES) {
        return None;
    }
    let media_type = resolve_image_content_type(&normalized_url, response.headers())?;
    let bytes = response.bytes().await.ok()?;
    if bytes.is_empty() || bytes.len() > MAX_FAVICON_BYTES {
        return None;
    }
    Some(format!(
        "data:{media_type};base64,{}",
        BASE64_STANDARD.encode(bytes)
    ))
}

pub(super) fn response_content_length_exceeds(
    headers: &reqwest::header::HeaderMap,
    max_bytes: usize,
) -> bool {
    headers
        .get(reqwest::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<usize>().ok())
        .is_some_and(|length| length > max_bytes)
}

pub(super) fn resolve_image_content_type(
    value: &str,
    headers: &reqwest::header::HeaderMap,
) -> Option<String> {
    let header_value = headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    match header_value.as_deref() {
        Some(
            "application/ico"
            | "application/x-ico"
            | "application/x-icon"
            | "application/vnd.microsoft.icon",
        ) => return Some("image/x-icon".to_string()),
        Some(value) if value.starts_with("image/") => return Some(value.to_string()),
        Some("application/octet-stream" | "binary/octet-stream") | None => {}
        Some(_) => return None,
    }

    let path = Url::parse(value).ok()?.path().to_ascii_lowercase();
    if path.ends_with(".svg") {
        Some("image/svg+xml".to_string())
    } else if path.ends_with(".png") {
        Some("image/png".to_string())
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        Some("image/jpeg".to_string())
    } else if path.ends_with(".gif") {
        Some("image/gif".to_string())
    } else if path.ends_with(".webp") {
        Some("image/webp".to_string())
    } else if path.ends_with(".ico") {
        Some("image/x-icon".to_string())
    } else {
        None
    }
}
