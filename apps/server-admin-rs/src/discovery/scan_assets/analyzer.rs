use super::*;

pub(super) async fn analyze_discovered_http_service(
    client: &reqwest::Client,
    result: DiscoveryHttpResult,
    translator: &Translator,
) -> Option<Value> {
    if is_plain_http_to_https_response(result.status, &result.body) {
        return None;
    }

    let rule = match_discovery_analyzer_rule(client, &result, translator).await;
    Some(build_discovered_service_value(&result, rule))
}

#[cfg(test)]
pub(super) fn build_discovered_http_service(
    host: &str,
    port: u16,
    status: u16,
    www_authenticate: Option<&str>,
    body: &str,
) -> Option<Value> {
    if is_plain_http_to_https_response(status, body) {
        return None;
    }
    let mut headers = HashMap::new();
    if let Some(www_authenticate) = www_authenticate {
        headers.insert("www-authenticate".to_string(), www_authenticate.to_string());
    }
    let result = DiscoveryHttpResult {
        host: host.to_string(),
        port,
        status,
        headers,
        body: body.to_string(),
    };
    Some(build_discovered_service_value(
        &result,
        build_generic_http_rule(&result),
    ))
}

pub(super) fn build_discovered_service_value(
    result: &DiscoveryHttpResult,
    rule: DiscoveryAnalyzerRule,
) -> Value {
    let service_name = rule.name.to_string();
    let mut service = json!({
        "serviceKey": format!("{}::{service_name}", result.host),
        "host": result.host,
        "port": result.port,
        "httpStatus": result.status,
        "detail": {
            "name": service_name,
            "label": rule.label,
            "rule": {
                "path": rule.proxy.path,
                "rewrite_html": rule.proxy.rewrite_html,
                "use_auth": true,
                "use_root_mode": rule.proxy.use_root_mode,
                "strip_path": true,
                "target": "",
            },
            "isDefault": rule.is_default,
        },
    });
    if has_basic_auth_challenge(result.headers.get("www-authenticate").map(String::as_str))
        && let Some(object) = service.as_object_mut()
    {
        object.insert("requiresBasicAuth".to_string(), json!(true));
    }
    service
}

pub(super) fn build_generic_http_rule(result: &DiscoveryHttpResult) -> DiscoveryAnalyzerRule {
    DiscoveryAnalyzerRule {
        name: format!("http-{}", result.port),
        label: extract_html_title(&result.body).unwrap_or_else(|| format!("HTTP {}", result.port)),
        proxy: DiscoveryProxyRule {
            path: format!("/app-{}", result.port),
            rewrite_html: true,
            use_root_mode: false,
        },
        is_default: false,
    }
}

pub(super) async fn match_discovery_analyzer_rule(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
    translator: &Translator,
) -> DiscoveryAnalyzerRule {
    if header_contains(result, "set-cookie", "mongo-express=") {
        return discovery_rule(
            "mongoexpress",
            "Mongo Express",
            "/mongoe",
            true,
            false,
            false,
        );
    }
    if body_contains(result, "<title>Redis Insight</title>") {
        return discovery_rule(
            "redisinsight",
            "Redis Insight",
            "/redisi",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "<title>go2rtc</title>") {
        return discovery_rule("go2rtc", "Go2RTC", "/go2rtc", true, false, false);
    }
    if is_openwrt_luci_result(result) {
        return discovery_rule("openwrt", "OpenWrt LuCI", "/openwrt", false, true, false);
    }
    if body_contains(result, "<title>飞牛 fnOS</title>") {
        return discovery_rule(
            "fnos",
            &scanner_service_label(translator, "fnos"),
            "/fnos",
            false,
            true,
            true,
        );
    }
    if body_contains(result, "<title>Lucky</title>") {
        return discovery_rule("lucky", "Lucky", "/lucky", true, false, false);
    }

    if let Some(site_title) = fetch_list_public_site_title(client, result).await {
        if site_title == "小雅的分类 Alist" {
            return discovery_rule(
                "xiaoya",
                &scanner_service_label(translator, "xiaoyaAlist"),
                "/xy",
                false,
                true,
                false,
            );
        }
        if site_title == "Alist" {
            return discovery_rule("alist", "AList", "/alist", false, true, false);
        }
        if site_title == "OpenList" {
            return discovery_rule("openlist", "OpenList", "/op", false, true, false);
        }
    }

    if body_contains(result, "<title>Home Assistant</title>") {
        return discovery_rule("homeassistant", "Home Assistant", "/ha", true, false, false);
    }
    if body_contains(result, "<title>Sun-Panel</title>") {
        return discovery_rule("sun-panel", "Sun-Panel", "/sp", true, true, false);
    }
    if result.port == 5005
        && header_contains(result, "www-authenticate", "Basic realm=\"Restricted\"")
    {
        return discovery_rule("webdav", "WebDAV", "/webdav", true, false, false);
    }
    if body_contains(result, "<title>迅雷下载</title>") {
        return discovery_rule(
            "xunlei",
            &scanner_service_label(translator, "xunlei"),
            "/xunlei",
            true,
            false,
            false,
        );
    }
    if body_contains(result, "<TITLE>MiniDLNA") {
        return discovery_rule("miniDLNA", "miniDLNA", "/dlna", true, false, false);
    }
    if body_contains(result, "<title>Digital Zen Garden</title>") {
        return discovery_rule(
            "nowen",
            &scanner_service_label(translator, "nowen"),
            "/nowen",
            false,
            true,
            true,
        );
    }
    if body_contains(result, "<title>飞牛影视</title>") {
        return discovery_rule(
            "fnys",
            &scanner_service_label(translator, "fnys"),
            "/v",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "dpanel/ui") {
        return discovery_rule("DPanel", "DPanel", "/dp", false, true, false);
    }
    if body_contains(result, "<title>彩票助手</title>") {
        return discovery_rule(
            "cpzs",
            &scanner_service_label(translator, "lottery"),
            "/cpzs",
            false,
            true,
            false,
        );
    }
    if result.port == 5005 && body_contains(result, "<title>登录</title>") {
        return discovery_rule(
            "Kuake",
            &scanner_service_label(translator, "kuake"),
            "/kuake",
            false,
            true,
            false,
        );
    }
    if body_contains(result, "<title>Jellyfin</title>") {
        return discovery_rule("Jellyfin", "Jellyfin", "/jellyfin", false, true, false);
    }
    if body_contains(result, "<title>WebUI 登录 | ME Frp</title>") {
        return discovery_rule("ME Frp", "ME Frp", "/mefrp", false, true, false);
    }
    if body_contains(result, "<title>MoonTV</title>") {
        return discovery_rule("MoonTV", "MoonTV", "/moontv", false, true, false);
    }
    if body_contains(result, "<title>fnOS Apps</title>") {
        return discovery_rule("fnOS Apps", "fnOS Apps", "/fnosapps", false, true, false);
    }
    if body_contains(result, "emby-elements/emby-collapse/emby-collapse") {
        return discovery_rule("Emby", "Emby", "/emby", false, true, false);
    }
    if body_contains(result, "<title>道理鱼音乐管理</title>") {
        return discovery_rule(
            "DLYMusic",
            &scanner_service_label(translator, "dlymusic"),
            "/music",
            false,
            true,
            false,
        );
    }
    if has_one_panel_loading_title(&result.body)
        && has_one_panel_public_favicon(client, result).await
    {
        return discovery_rule("1Panel", "1Panel", "/1panel", false, true, false);
    }

    build_generic_http_rule(result)
}

pub(super) fn discovery_rule(
    name: &str,
    label: &str,
    path: &str,
    rewrite_html: bool,
    use_root_mode: bool,
    is_default: bool,
) -> DiscoveryAnalyzerRule {
    DiscoveryAnalyzerRule {
        name: name.to_string(),
        label: label.to_string(),
        proxy: DiscoveryProxyRule {
            path: path.to_string(),
            rewrite_html,
            use_root_mode,
        },
        is_default,
    }
}

pub(super) fn scanner_service_label(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.scanDiscovery.serviceLabels.{key}"))
}

pub(super) fn body_contains(result: &DiscoveryHttpResult, needle: &str) -> bool {
    result.body.contains(needle)
}

pub(super) fn header_contains(result: &DiscoveryHttpResult, header: &str, needle: &str) -> bool {
    result
        .headers
        .get(header)
        .is_some_and(|value| value.contains(needle))
}

pub(super) fn extract_html_title_text(body: &str) -> String {
    extract_html_title(body).unwrap_or_default()
}

pub(super) fn has_list_title(body: &str) -> bool {
    extract_html_title_text(body)
        .trim()
        .to_ascii_lowercase()
        .contains("list")
}

pub(super) async fn fetch_list_public_site_title(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
) -> Option<String> {
    if !has_list_title(&result.body) {
        return None;
    }
    let url = format!("http://{}:{}/api/public/settings", result.host, result.port);
    let response = client
        .get(url)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let payload = crate::http_body::read_response_json_limited::<Value>(
        response,
        MAX_DISCOVERY_API_RESPONSE_BYTES,
    )
    .await
    .ok()?;
    if payload.get("code").and_then(Value::as_i64) != Some(200) {
        return None;
    }
    payload
        .pointer("/data/site_title")
        .and_then(Value::as_str)
        .map(str::to_string)
}

pub(super) fn is_openwrt_luci_result(result: &DiscoveryHttpResult) -> bool {
    result
        .headers
        .get("x-luci-login-required")
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("yes"))
        || has_luci_entrypoint(&result.body)
        || has_luci_login_page(&result.body)
}

pub(super) fn has_luci_entrypoint(body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    normalized.contains("cgi-bin/luci")
        && (normalized.contains("luci - lua configuration interface")
            || normalized.contains("http-equiv=\"refresh\"")
            || normalized.contains("http-equiv='refresh'")
            || normalized.contains("http-equiv=refresh"))
}

pub(super) fn has_luci_login_page(body: &str) -> bool {
    let title = extract_html_title_text(body)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let normalized = body.to_ascii_lowercase();
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

pub(super) fn has_one_panel_loading_title(body: &str) -> bool {
    extract_html_title_text(body)
        .trim()
        .eq_ignore_ascii_case("loading...")
}

pub(super) async fn has_one_panel_public_favicon(
    client: &reqwest::Client,
    result: &DiscoveryHttpResult,
) -> bool {
    let url = format!("http://{}:{}/public/favicon.png", result.host, result.port);
    let Ok(response) = client
        .get(url)
        .header(reqwest::header::USER_AGENT, DISCOVERY_HTTP_USER_AGENT)
        .header(reqwest::header::CONNECTION, "close")
        .header(reqwest::header::ACCEPT, "image/*,*/*;q=0.8")
        .send()
        .await
    else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(|value| value.trim().to_ascii_lowercase());
    content_type.is_none_or(|value| {
        value.starts_with("image/")
            || value == "application/octet-stream"
            || value == "binary/octet-stream"
    })
}

pub(super) fn is_plain_http_to_https_response(status: u16, body: &str) -> bool {
    if status != 400 {
        return false;
    }
    let normalized = body.to_ascii_lowercase();
    normalized.contains("plain http request was sent to https port")
        || normalized.contains("client sent an http request to an https server")
}

pub(super) fn extract_html_title(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let title_start = lower.find("<title")?;
    let content_start = title_start + lower[title_start..].find('>')? + 1;
    let content_end = content_start + lower[content_start..].find("</title>")?;
    let title = body[content_start..content_end].trim().to_string();
    (!title.is_empty()).then_some(title)
}

pub(super) fn has_basic_auth_challenge(www_authenticate: Option<&str>) -> bool {
    let Some(www_authenticate) = www_authenticate else {
        return false;
    };
    www_authenticate
        .split(',')
        .map(str::trim_start)
        .any(|part| {
            let lower = part.to_ascii_lowercase();
            lower == "basic" || lower.starts_with("basic ") || lower.starts_with("basic\t")
        })
}
