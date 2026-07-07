use super::*;

pub(super) fn extract_html_title(html: &str) -> String {
    let lower = html.to_ascii_lowercase();
    let Some(start) = lower.find("<title") else {
        return String::new();
    };
    let Some(open_end) = lower[start..].find('>') else {
        return String::new();
    };
    let content_start = start + open_end + 1;
    let Some(close_start) = lower[content_start..].find("</title>") else {
        return String::new();
    };
    collapse_html_whitespace(&decode_html_entities(
        &html[content_start..content_start + close_start],
    ))
}

#[cfg(test)]
pub(super) fn extract_favicon_url(html: &str, base_url: &str) -> Option<String> {
    let html_base_url = extract_html_base_url(html, base_url);
    extract_explicit_favicon_urls_from_html(html, &html_base_url)
        .into_iter()
        .next()
        .or_else(|| {
            extract_heuristic_favicon_urls_from_html(
                html,
                &html_base_url,
                HEURISTIC_FAVICON_MIN_PRIORITY,
            )
            .into_iter()
            .next()
        })
        .or_else(|| resolve_default_favicon_url(base_url))
}

pub(super) fn resolve_url(base_url: &str, href: &str) -> Option<String> {
    if href.is_empty() {
        return None;
    }
    let base = Url::parse(base_url).ok()?;
    base.join(href).ok().map(|url| url.to_string())
}

#[cfg(test)]
pub(super) fn resolve_default_favicon_url(final_url: &str) -> Option<String> {
    let mut parsed = Url::parse(final_url).ok()?;
    parsed.set_path("/favicon.ico");
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

pub(super) fn resolve_origin_path_url(value: &str, pathname: &str) -> Option<String> {
    let mut parsed = Url::parse(value).ok()?;
    parsed.set_path(pathname);
    parsed.set_query(None);
    parsed.set_fragment(None);
    Some(parsed.to_string())
}

pub(super) fn resolve_fallback_favicon_urls(value: &str) -> Vec<String> {
    FALLBACK_FAVICON_PATHS
        .iter()
        .filter_map(|pathname| resolve_origin_path_url(value, pathname))
        .collect()
}

pub(super) fn normalize_favicon_url(value: &str, base_url: &str) -> Option<String> {
    let trimmed = decode_html_entities(value).trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.to_ascii_lowercase().starts_with("data:image/") {
        return Some(trimmed);
    }
    let resolved = resolve_url(base_url, &trimmed.replace("\\/", "/"))?;
    let parsed = Url::parse(&resolved).ok()?;
    matches!(parsed.scheme(), "http" | "https" | "data").then_some(parsed.to_string())
}

pub(super) fn normalize_manifest_url(value: &str, base_url: &str) -> Option<String> {
    let trimmed = decode_html_entities(value).trim().to_string();
    if trimmed.is_empty() {
        return None;
    }
    let resolved = resolve_url(base_url, &trimmed)?;
    let parsed = Url::parse(&resolved).ok()?;
    matches!(parsed.scheme(), "http" | "https").then_some(parsed.to_string())
}

pub(super) fn extract_html_base_url(html: &str, base_url: &str) -> String {
    for tag in collect_html_tags(html, "base") {
        let attributes = parse_html_attributes(tag);
        if let Some(href) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        {
            return href;
        }
    }
    base_url.to_string()
}

pub(super) fn collect_html_tags<'a>(html: &'a str, tag_name: &str) -> Vec<&'a str> {
    let lower = html.to_ascii_lowercase();
    let needle = format!("<{}", tag_name.to_ascii_lowercase());
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(relative_start) = lower[cursor..].find(&needle) {
        let start = cursor + relative_start;
        let after_name = start + needle.len();
        if let Some(next) = lower.as_bytes().get(after_name)
            && !matches!(next, b' ' | b'\t' | b'\n' | b'\r' | b'/' | b'>')
        {
            cursor = after_name;
            continue;
        }
        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        if let Some(tag) = html.get(start..end) {
            tags.push(tag);
        }
        cursor = end;
    }
    tags
}

pub(super) fn get_html_tag_name(tag: &str) -> String {
    let trimmed = tag.trim_start();
    let Some(rest) = trimmed.strip_prefix('<') else {
        return String::new();
    };
    rest.chars()
        .take_while(|ch| !ch.is_ascii_whitespace() && *ch != '/' && *ch != '>')
        .collect::<String>()
        .to_ascii_lowercase()
}

pub(super) fn parse_html_attributes(tag: &str) -> HashMap<String, String> {
    let bytes = tag.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() && bytes[idx] != b'<' {
        idx += 1;
    }
    if idx < bytes.len() {
        idx += 1;
    }
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    while idx < bytes.len()
        && !bytes[idx].is_ascii_whitespace()
        && bytes[idx] != b'/'
        && bytes[idx] != b'>'
    {
        idx += 1;
    }

    let mut attributes = HashMap::new();
    while idx < bytes.len() {
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }
        if idx >= bytes.len() || bytes[idx] == b'>' {
            break;
        }
        if bytes[idx] == b'/' {
            idx += 1;
            continue;
        }

        let name_start = idx;
        while idx < bytes.len()
            && !bytes[idx].is_ascii_whitespace()
            && bytes[idx] != b'='
            && bytes[idx] != b'/'
            && bytes[idx] != b'>'
        {
            idx += 1;
        }
        if name_start == idx {
            idx += 1;
            continue;
        }
        let Some(raw_name) = tag.get(name_start..idx) else {
            continue;
        };
        let name = raw_name.to_ascii_lowercase();
        while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
            idx += 1;
        }

        let mut value = "";
        if idx < bytes.len() && bytes[idx] == b'=' {
            idx += 1;
            while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
                idx += 1;
            }
            if idx < bytes.len() {
                let quote = bytes[idx];
                let value_start;
                let value_end;
                if quote == b'"' || quote == b'\'' {
                    idx += 1;
                    value_start = idx;
                    while idx < bytes.len() && bytes[idx] != quote {
                        idx += 1;
                    }
                    value_end = idx;
                    if idx < bytes.len() {
                        idx += 1;
                    }
                } else {
                    value_start = idx;
                    while idx < bytes.len()
                        && !bytes[idx].is_ascii_whitespace()
                        && bytes[idx] != b'>'
                        && bytes[idx] != b'/'
                    {
                        idx += 1;
                    }
                    value_end = idx;
                }
                value = tag.get(value_start..value_end).unwrap_or("");
            }
        }
        attributes.insert(name, decode_html_entities(value).trim().to_string());
    }
    attributes
}

pub(super) fn get_favicon_priority(rel: &str) -> i32 {
    let normalized = rel
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        0
    } else if normalized == "icon" {
        500
    } else if normalized == "shortcut icon" {
        450
    } else if normalized.contains("apple-touch-icon") {
        400
    } else if normalized.contains("mask-icon") {
        300
    } else if normalized.split_whitespace().any(|token| token == "icon") {
        350
    } else {
        0
    }
}

pub(super) fn get_image_extension_priority(extension: &str) -> i32 {
    match extension {
        "ico" => 80,
        "png" => 60,
        "svg" => 50,
        "webp" => 40,
        "jpg" | "jpeg" => 30,
        "gif" => 20,
        _ => 0,
    }
}

pub(super) fn get_favicon_path_priority(value: &str) -> i32 {
    if value.to_ascii_lowercase().starts_with("data:image/") {
        return 0;
    }

    let Ok(parsed) = Url::parse(value) else {
        return 0;
    };
    let pathname = parsed.path().to_ascii_lowercase();
    let file_name = pathname.rsplit('/').next().unwrap_or("");
    let extension = file_name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");

    let mut priority = if file_name == "favicon.ico" {
        700
    } else if file_name.starts_with("favicon")
        && file_name
            .as_bytes()
            .get("favicon".len())
            .is_none_or(|ch| matches!(ch, b'-' | b'_' | b'.'))
    {
        650
    } else if file_name.starts_with("apple-touch-icon") {
        600
    } else if file_name.starts_with("android-chrome") {
        560
    } else if file_name.starts_with("mstile") {
        520
    } else if file_name.contains("favicon") {
        500
    } else if pathname.contains("/favicon") {
        450
    } else if is_icon_like_file_name(file_name) {
        220
    } else if extension == "ico" {
        180
    } else if is_logo_like_file_name(file_name) {
        80
    } else {
        return 0;
    };

    priority += get_image_extension_priority(extension);
    if pathname.contains("/img/") {
        priority += 20;
    }
    if pathname.contains("/icons/") || pathname.contains("/icon/") {
        priority += 15;
    }
    if pathname.split('/').count() <= 3 {
        priority += 10;
    }
    priority
}

pub(super) fn is_icon_like_file_name(file_name: &str) -> bool {
    let normalized = file_name.replace(['-', '_', '.'], " ");
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    tokens.iter().any(|token| {
        matches!(
            *token,
            "appicon" | "app" | "siteicon" | "site" | "touchicon" | "touch" | "icon"
        )
    }) && file_name.contains("icon")
}

pub(super) fn is_logo_like_file_name(file_name: &str) -> bool {
    file_name
        .replace(['-', '_', '.'], " ")
        .split_whitespace()
        .any(|token| token == "logo")
}

pub(super) fn get_favicon_type_priority(value: &str) -> i32 {
    let normalized = value
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "image/x-icon"
        | "image/vnd.microsoft.icon"
        | "application/x-icon"
        | "application/vnd.microsoft.icon" => 850,
        "image/svg+xml" => 260,
        value if value.starts_with("image/") => 160,
        _ => 0,
    }
}

pub(super) fn get_attribute_hint_priority(
    attribute_name: &str,
    attributes: Option<&HashMap<String, String>>,
) -> i32 {
    let mut priority = 0;
    let normalized_attribute_name = attribute_name.to_ascii_lowercase();
    if normalized_attribute_name.contains("favicon") {
        priority += 450;
    } else if normalized_attribute_name.contains("icon") {
        priority += 280;
    } else if normalized_attribute_name == "href" {
        priority += 60;
    } else if normalized_attribute_name == "src" {
        priority += 40;
    } else if normalized_attribute_name == "content" {
        priority += 30;
    }

    for key in ["name", "property", "itemprop", "id", "class"] {
        let normalized_value = attributes
            .and_then(|attributes| attributes.get(key))
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if normalized_value.is_empty() {
            continue;
        }
        if normalized_value.contains("favicon") || normalized_value.contains("shortcut icon") {
            priority += 520;
        } else if normalized_value.contains("apple-touch-icon") {
            priority += 480;
        } else if normalized_value.contains("msapplication-tileimage")
            || normalized_value.contains("tileimage")
        {
            priority += 440;
        } else if contains_word(&normalized_value, "icon") {
            priority += 260;
        }
    }
    priority
}

pub(super) fn get_tag_priority(tag_name: &str) -> i32 {
    match tag_name {
        "link" => 120,
        "meta" => 60,
        "img" => 20,
        _ => 0,
    }
}

pub(super) fn get_html_icon_size_priority(sizes: Option<&str>) -> i32 {
    let Some(sizes) = sizes else {
        return 0;
    };
    let mut best = 0_i32;
    for token in sizes.trim().to_ascii_lowercase().split_whitespace() {
        if token == "any" {
            best = best.max(1024);
            continue;
        }
        let Some((width, height)) = parse_icon_size(token) else {
            continue;
        };
        best = best.max(width.min(height));
    }

    if best >= 192 {
        160
    } else if best >= 64 {
        120
    } else if best >= 32 {
        80
    } else if best > 0 {
        30
    } else {
        0
    }
}

pub(super) fn get_surrounding_favicon_priority(value: Option<&str>) -> i32 {
    let normalized = value.map(str::to_ascii_lowercase).unwrap_or_default();
    if normalized.is_empty() {
        0
    } else if normalized.contains("favicon") {
        520
    } else if normalized.contains("shortcut icon") {
        500
    } else if normalized.contains("apple-touch-icon") {
        480
    } else if normalized.contains("msapplication-tileimage") || normalized.contains("tileimage") {
        440
    } else if normalized.contains("fav-icon")
        || normalized.contains("fav_icon")
        || normalized.contains("fav icon")
        || normalized.contains("iconurl")
        || normalized.contains("iconuri")
        || normalized.contains("iconhref")
        || normalized.contains("iconsrc")
        || normalized.contains("iconpath")
        || normalized.contains("appicon")
        || normalized.contains("siteicon")
    {
        320
    } else if contains_word(&normalized, "icon") {
        140
    } else {
        0
    }
}

pub(super) fn create_favicon_candidate(
    raw_value: &str,
    base_url: &str,
    index: usize,
    context: FaviconCandidateContext<'_>,
) -> Option<FaviconCandidate> {
    let href = normalize_favicon_url(raw_value, base_url)?;
    let attributes = context.attributes;
    let rel_priority = get_favicon_priority(
        attributes
            .and_then(|value| value.get("rel"))
            .map(String::as_str)
            .unwrap_or(""),
    );
    let path_priority = get_favicon_path_priority(&href);
    let type_priority = get_favicon_type_priority(
        attributes
            .and_then(|value| value.get("type"))
            .map(String::as_str)
            .unwrap_or(""),
    );
    let attribute_priority =
        get_attribute_hint_priority(context.attribute_name.unwrap_or(""), attributes);
    let surrounding_priority = get_surrounding_favicon_priority(context.surrounding_text);
    let size_priority = get_html_icon_size_priority(
        attributes
            .and_then(|attributes| attributes.get("sizes"))
            .map(String::as_str),
    );
    let priority = rel_priority * 1000
        + path_priority
        + type_priority
        + attribute_priority
        + surrounding_priority
        + get_tag_priority(context.tag_name.unwrap_or(""))
        + size_priority
        + context.source_priority;

    if !context.force && priority < context.min_priority {
        return None;
    }

    Some(FaviconCandidate {
        href,
        priority,
        index,
    })
}

pub(super) fn sort_favicon_candidates(mut candidates: Vec<FaviconCandidate>) -> Vec<String> {
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            if seen.insert(candidate.href.clone()) {
                Some(candidate.href)
            } else {
                None
            }
        })
        .take(MAX_HTML_FAVICON_CANDIDATES_TO_TRY)
        .collect()
}

pub(super) fn extract_explicit_favicon_urls_from_html(html: &str, base_url: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for (index, tag) in collect_html_tags(html, "link").into_iter().enumerate() {
        let attributes = parse_html_attributes(tag);
        if get_favicon_priority(attributes.get("rel").map(String::as_str).unwrap_or("")) <= 0 {
            continue;
        }
        if let Some(candidate) = create_favicon_candidate(
            attributes.get("href").map(String::as_str).unwrap_or(""),
            base_url,
            index,
            FaviconCandidateContext {
                tag_name: Some(&get_html_tag_name(tag)),
                attribute_name: Some("href"),
                attributes: Some(&attributes),
                surrounding_text: None,
                source_priority: 0,
                min_priority: HEURISTIC_FAVICON_MIN_PRIORITY,
                force: true,
            },
        ) {
            candidates.push(candidate);
        }
    }
    sort_favicon_candidates(candidates)
}

pub(super) fn extract_heuristic_favicon_urls_from_html(
    html: &str,
    base_url: &str,
    min_priority: i32,
) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut index = 0_usize;
    for tag_name in ["link", "meta", "img", "source"] {
        for tag in collect_html_tags(html, tag_name) {
            let parsed_tag_name = get_html_tag_name(tag);
            let attributes = parse_html_attributes(tag);
            for attribute_name in FAVICON_CANDIDATE_ATTRIBUTE_NAMES {
                let Some(raw_value) = attributes.get(attribute_name) else {
                    continue;
                };
                if let Some(candidate) = create_favicon_candidate(
                    raw_value,
                    base_url,
                    index,
                    FaviconCandidateContext {
                        tag_name: Some(&parsed_tag_name),
                        attribute_name: Some(attribute_name),
                        attributes: Some(&attributes),
                        surrounding_text: None,
                        source_priority: 0,
                        min_priority,
                        force: false,
                    },
                ) {
                    candidates.push(candidate);
                }
                index += 1;
            }
        }
    }

    for (raw_value, match_index) in extract_image_resource_paths(html) {
        let start = match_index.saturating_sub(80);
        let end = (match_index + raw_value.len() + 80).min(html.len());
        let surrounding_text = html.get(start..end).unwrap_or("");
        if let Some(candidate) = create_favicon_candidate(
            &raw_value,
            base_url,
            index,
            FaviconCandidateContext {
                tag_name: None,
                attribute_name: None,
                attributes: None,
                surrounding_text: Some(surrounding_text),
                source_priority: 0,
                min_priority,
                force: false,
            },
        ) {
            candidates.push(candidate);
        }
        index += 1;
    }

    sort_favicon_candidates(candidates)
}

pub(super) fn extract_image_resource_paths(html: &str) -> Vec<(String, usize)> {
    let lower = html.to_ascii_lowercase();
    let bytes = html.as_bytes();
    let mut results = Vec::new();
    let mut cursor = 0;
    while cursor < lower.len() {
        let next_match = [".ico", ".png", ".svg", ".jpg", ".jpeg", ".gif", ".webp"]
            .iter()
            .filter_map(|extension| {
                lower[cursor..]
                    .find(extension)
                    .map(|pos| (cursor + pos, *extension))
            })
            .min_by_key(|(pos, _)| *pos);
        let Some((extension_pos, extension)) = next_match else {
            break;
        };

        let mut start = extension_pos;
        while start > 0 && !is_image_resource_delimiter(bytes[start - 1]) {
            start -= 1;
        }
        let mut end = extension_pos + extension.len();
        while end < bytes.len() && !is_image_resource_delimiter(bytes[end]) {
            end += 1;
        }

        if let Some(value) = html.get(start..end) {
            let trimmed = value.trim_matches(|ch| matches!(ch, '\'' | '"' | '(' | ')' | '\\'));
            if is_plausible_image_resource_path(trimmed) {
                results.push((trimmed.to_string(), start));
            }
        }
        cursor = end.max(extension_pos + extension.len());
    }
    results
}

pub(super) fn is_image_resource_delimiter(value: u8) -> bool {
    value.is_ascii_whitespace() || matches!(value, b'"' | b'\'' | b'<' | b'>' | b'\\' | b')')
}

pub(super) fn is_plausible_image_resource_path(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("//")
        || lower.starts_with('/')
        || lower.starts_with("./")
        || lower.starts_with("../")
        || (lower.contains('/') && !lower.contains('<') && !lower.contains('>'))
}

pub(super) fn extract_manifest_from_html(html: &str, base_url: &str) -> Option<String> {
    for tag in collect_html_tags(html, "link") {
        let attributes = parse_html_attributes(tag);
        let has_manifest_rel = attributes
            .get("rel")
            .map(|rel| {
                rel.trim()
                    .to_ascii_lowercase()
                    .split_whitespace()
                    .any(|token| token == "manifest")
            })
            .unwrap_or(false);
        if !has_manifest_rel {
            continue;
        }
        if let Some(href) = attributes
            .get("href")
            .and_then(|href| normalize_manifest_url(href, base_url))
        {
            return Some(href);
        }
    }
    None
}

pub(super) async fn fetch_manifest_icon_urls(
    client: &reqwest::Client,
    manifest_url: &str,
    basic_auth: Option<&MetadataBasicAuthContext>,
) -> Vec<String> {
    let Some(normalized_url) = normalize_http_probe_url(manifest_url) else {
        return Vec::new();
    };
    let Ok(response) = send_metadata_get(
        client,
        &normalized_url,
        "application/manifest+json,application/json,*/*;q=0.8",
        basic_auth,
    )
    .await
    else {
        return Vec::new();
    };
    if !response.status().is_success()
        || response_content_length_exceeds(response.headers(), MAX_MANIFEST_BYTES)
    {
        return Vec::new();
    }
    let manifest_url = response.url().to_string();
    let Ok(text) = read_response_text_limited(response, MAX_MANIFEST_BYTES).await else {
        return Vec::new();
    };
    let Ok(manifest) = serde_json::from_str::<Value>(&text) else {
        return Vec::new();
    };
    extract_manifest_icon_urls(&manifest, &manifest_url)
}

pub(super) fn extract_manifest_icon_urls(manifest: &Value, manifest_url: &str) -> Vec<String> {
    let Some(icons) = manifest.get("icons").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for (index, raw_icon) in icons.iter().enumerate() {
        let Some(icon) = raw_icon.as_object() else {
            continue;
        };
        let Some(src) = icon.get("src").and_then(Value::as_str) else {
            continue;
        };
        let media_type = icon
            .get("type")
            .and_then(Value::as_str)
            .and_then(|value| value.split(';').next())
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        if !media_type.is_empty() && !media_type.starts_with("image/") {
            continue;
        }
        let Some(href) = normalize_favicon_url(src, manifest_url) else {
            continue;
        };
        candidates.push(FaviconCandidate {
            href,
            priority: get_manifest_icon_priority(raw_icon),
            index,
        });
    }
    sort_manifest_icon_candidates(candidates)
}

pub(super) fn sort_manifest_icon_candidates(mut candidates: Vec<FaviconCandidate>) -> Vec<String> {
    candidates.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.index.cmp(&right.index))
    });
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter_map(|candidate| {
            if seen.insert(candidate.href.clone()) {
                Some(candidate.href)
            } else {
                None
            }
        })
        .take(MAX_MANIFEST_ICONS_TO_TRY)
        .collect()
}

pub(super) fn get_manifest_icon_priority(icon: &Value) -> i32 {
    let purpose_tokens = icon
        .get("purpose")
        .and_then(Value::as_str)
        .map(|purpose| {
            purpose
                .trim()
                .to_ascii_lowercase()
                .split_whitespace()
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let media_type = icon
        .get("type")
        .and_then(Value::as_str)
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_default();

    let mut priority = icon
        .get("sizes")
        .and_then(Value::as_str)
        .map(get_manifest_icon_size_score)
        .unwrap_or(0);
    if purpose_tokens.is_empty() || purpose_tokens.iter().any(|token| token == "any") {
        priority += 2000;
    } else if purpose_tokens.iter().any(|token| token == "maskable") {
        priority += 1000;
    }
    priority += match media_type.as_str() {
        "image/png" => 80,
        "image/svg+xml" => 70,
        "image/webp" => 60,
        "image/jpeg" => 50,
        "image/x-icon" | "image/vnd.microsoft.icon" => 40,
        _ => 0,
    };
    priority
}

pub(super) fn get_manifest_icon_size_score(sizes: &str) -> i32 {
    let mut best = 0_i32;
    for token in sizes.trim().to_ascii_lowercase().split_whitespace() {
        if token == "any" {
            best = best.max(1024);
            continue;
        }
        let Some((width, height)) = parse_icon_size(token) else {
            continue;
        };
        best = best.max(width.min(height));
    }
    best
}

pub(super) fn parse_icon_size(token: &str) -> Option<(i32, i32)> {
    let (width, height) = token.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}
