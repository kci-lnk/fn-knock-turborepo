use super::*;

pub(super) async fn fetch_first_favicon_as_data_url(
    client: &reqwest::Client,
    favicon_urls: &[String],
    basic_auth: Option<&MetadataBasicAuthContext>,
    budget: &mut FaviconFetchBudget,
    reserve_attempts: i32,
) -> String {
    for favicon_url in favicon_urls {
        let normalized = favicon_url.trim();
        if normalized.is_empty() || budget.seen.contains(normalized) {
            continue;
        }

        let is_inline_image = normalized.to_ascii_lowercase().starts_with("data:image/");
        if !is_inline_image {
            if budget.remaining <= reserve_attempts {
                break;
            }
            budget.remaining -= 1;
        }
        budget.seen.insert(normalized.to_string());
        if let Some(favicon) = fetch_favicon_as_data_url(client, normalized, basic_auth).await {
            return favicon;
        }
    }
    String::new()
}

pub(super) fn is_openwrt_luci_url(value: &str) -> bool {
    Url::parse(value)
        .map(|url| {
            let pathname = url.path().to_ascii_lowercase();
            pathname == "/cgi-bin/luci" || pathname.starts_with(OPENWRT_LUCI_PATH)
        })
        .unwrap_or(false)
}

pub(super) fn is_same_origin_url(value: &str, base_url: &str) -> bool {
    let Ok(value) = Url::parse(value) else {
        return false;
    };
    let Ok(base) = Url::parse(base_url) else {
        return false;
    };
    value.origin() == base.origin()
}

pub(super) fn strip_refresh_url_quotes(value: &str) -> String {
    value.trim().trim_matches(['"', '\'']).to_string()
}

pub(super) fn extract_openwrt_luci_url_from_html(html: &str, base_url: &str) -> Option<String> {
    for tag in collect_html_tags(html, "meta") {
        let attributes = parse_html_attributes(tag);
        if attributes
            .get("http-equiv")
            .map(|value| value.trim().eq_ignore_ascii_case("refresh"))
            != Some(true)
        {
            continue;
        }
        let content =
            decode_html_entities(attributes.get("content").map(String::as_str).unwrap_or(""));
        let Some(refresh_url) = find_refresh_url(&content) else {
            continue;
        };
        let Some(resolved) =
            normalize_manifest_url(&strip_refresh_url_quotes(refresh_url), base_url)
        else {
            continue;
        };
        if is_openwrt_luci_url(&resolved) && is_same_origin_url(&resolved, base_url) {
            return Some(resolved);
        }
    }

    for tag in collect_html_tags(html, "a") {
        let attributes = parse_html_attributes(tag);
        let Some(resolved) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        else {
            continue;
        };
        if is_openwrt_luci_url(&resolved) && is_same_origin_url(&resolved, base_url) {
            return Some(resolved);
        }
    }

    Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(OPENWRT_LUCI_PATH).ok())
        .map(|url| url.to_string())
}

pub(super) fn find_refresh_url(content: &str) -> Option<&str> {
    let lower = content.to_ascii_lowercase();
    let lower_bytes = lower.as_bytes();
    let content_bytes = content.as_bytes();
    let mut cursor = 0;
    while let Some(relative_pos) = lower[cursor..].find("url") {
        let pos = cursor + relative_pos;
        let before_ok = pos == 0 || !lower_bytes[pos - 1].is_ascii_alphanumeric();
        if !before_ok {
            cursor = pos + 3;
            continue;
        }

        let mut idx = pos + 3;
        while idx < content_bytes.len() && content_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if content_bytes.get(idx) != Some(&b'=') {
            cursor = idx;
            continue;
        }
        idx += 1;
        while idx < content_bytes.len() && content_bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        let value = &content[idx..];
        return Some(value.split(';').next().unwrap_or(value).trim());
    }
    None
}

pub(super) fn has_openwrt_luci_entrypoint_html(html: &str) -> bool {
    let normalized = html.to_ascii_lowercase();
    normalized.contains("cgi-bin/luci")
        && (normalized.contains("luci - lua configuration interface")
            || normalized.contains("http-equiv=\"refresh\"")
            || normalized.contains("http-equiv='refresh'")
            || normalized.contains("http-equiv=refresh"))
}

pub(super) fn has_openwrt_luci_document_html(html: &str) -> bool {
    let title = extract_html_title(html).to_ascii_lowercase();
    let normalized = html.to_ascii_lowercase();
    title_has_luci_word(&title)
        && (normalized.contains("/luci-static/")
            || normalized.contains("application-name")
            || normalized.contains("apple-mobile-web-app-title"))
}

pub(super) fn title_has_luci_word(title: &str) -> bool {
    let bytes = title.as_bytes();
    for (index, _) in title.match_indices("luci") {
        let before_ok = index == 0 || !bytes[index - 1].is_ascii_alphanumeric();
        let after = index + "luci".len();
        let after_ok = after >= bytes.len() || !bytes[after].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

pub(super) fn is_openwrt_luci_login_required_response(response: &reqwest::Response) -> bool {
    response.status() == reqwest::StatusCode::FORBIDDEN
        && response
            .headers()
            .get(OPENWRT_LUCI_LOGIN_REQUIRED_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(|value| value.trim().eq_ignore_ascii_case("yes"))
            == Some(true)
}

pub(super) async fn fetch_openwrt_luci_document(
    client: &reqwest::Client,
    document: MetadataHtmlDocument,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Result<MetadataHtmlDocument, MetadataHtmlDocument> {
    if is_openwrt_luci_url(&document.final_url) || has_openwrt_luci_document_html(&document.html) {
        return Ok(document);
    }
    if !has_openwrt_luci_entrypoint_html(&document.html) {
        return Err(document);
    }

    let Some(luci_url) = extract_openwrt_luci_url_from_html(&document.html, &document.final_url)
    else {
        return Err(document);
    };
    let Ok(response) = send_metadata_get(
        client,
        &luci_url,
        "text/html,application/xhtml+xml,*/*;q=0.8",
        basic_auth,
    )
    .await
    else {
        return Err(document);
    };
    let is_luci_login_required = is_openwrt_luci_login_required_response(&response);
    if !response.status().is_success() && !is_luci_login_required {
        return Err(document);
    }
    let final_url = response.url().to_string();
    let Ok(html) = read_response_text_limited(response, MAX_METADATA_HTML_BYTES).await else {
        return Err(document);
    };
    if !has_openwrt_luci_document_html(&html) && !is_luci_login_required {
        return Err(document);
    }
    Ok(MetadataHtmlDocument { html, final_url })
}

pub(super) fn is_one_panel_loading_title(value: &str) -> bool {
    value.trim().eq_ignore_ascii_case(ONE_PANEL_LOADING_TITLE)
}

pub(super) fn decode_html_entities(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '&' {
            output.push(ch);
            continue;
        }
        let mut token = String::new();
        while let Some(next) = chars.peek().copied() {
            chars.next();
            if next == ';' {
                break;
            }
            token.push(next);
            if token.len() > 16 {
                output.push('&');
                output.push_str(&token);
                token.clear();
                break;
            }
        }
        if token.is_empty() {
            continue;
        }
        let replacement = match token.to_ascii_lowercase().as_str() {
            "amp" => Some("&".to_string()),
            "lt" => Some("<".to_string()),
            "gt" => Some(">".to_string()),
            "quot" => Some("\"".to_string()),
            "apos" => Some("'".to_string()),
            "nbsp" => Some(" ".to_string()),
            token if token.starts_with("#x") => u32::from_str_radix(&token[2..], 16)
                .ok()
                .and_then(char::from_u32)
                .map(|ch| ch.to_string()),
            token if token.starts_with('#') => token[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(|ch| ch.to_string()),
            _ => None,
        };
        if let Some(replacement) = replacement {
            output.push_str(&replacement);
        } else {
            output.push('&');
            output.push_str(&token);
            output.push(';');
        }
    }
    output
}

pub(super) fn collapse_html_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(super) fn contains_word(value: &str, word: &str) -> bool {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .any(|token| token == word)
}
