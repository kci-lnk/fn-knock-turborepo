use super::*;

#[derive(Clone, Debug)]
pub(crate) struct PublicHostLinkContext {
    scheme: &'static str,
    access_entry_port: String,
    omit_access_entry_port: bool,
}

pub(crate) fn public_host_link_context(config: &Value) -> PublicHostLinkContext {
    let scheme = resolve_bookmark_scheme(config);
    let raw_public_base_url = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let resolved_public_port =
        resolve_public_port_for_scheme(config, scheme, raw_public_base_url, true, false);
    PublicHostLinkContext {
        scheme,
        access_entry_port: resolved_public_port
            .map(|port| port.to_string())
            .unwrap_or_else(|| crate::system_info::resolve_access_entry_port(config)),
        omit_access_entry_port: should_omit_public_access_entry_port(config)
            && resolved_public_port.is_none(),
    }
}

pub(crate) fn public_host_url(context: &PublicHostLinkContext, host: &str) -> String {
    build_bookmark_url(
        host,
        context.scheme,
        Some(&context.access_entry_port),
        context.omit_access_entry_port,
    )
}

pub(crate) fn resolve_public_host_title(object: &Map<String, Value>, host: &str) -> String {
    resolve_bookmark_title(object, host)
}

pub(super) use crate::http_utils::html_escape as escape_html;

pub(super) fn build_bookmarks_document(
    config: &Value,
    translator: &crate::i18n::Translator,
) -> String {
    let scheme = resolve_bookmark_scheme(config);
    let raw_public_base_url = config
        .pointer("/subdomain_mode/public_auth_base_url")
        .and_then(Value::as_str)
        .unwrap_or("");
    let resolved_public_port =
        resolve_public_port_for_scheme(config, scheme, raw_public_base_url, true, false);
    let access_entry_port = resolved_public_port
        .map(|port| port.to_string())
        .unwrap_or_else(|| crate::system_info::resolve_access_entry_port(config));
    let omit_access_entry_port =
        should_omit_public_access_entry_port(config) && resolved_public_port.is_none();
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let folder_title = if root_domain.is_empty() {
        translator.t("server.admin.hostMappings.bookmarkFolderDefault")
    } else {
        translator.t_params(
            "server.admin.hostMappings.bookmarkFolderForRoot",
            &[("root", root_domain.to_string())],
        )
    };
    let add_date = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut lines = vec![
        "<!DOCTYPE NETSCAPE-Bookmark-file-1>".to_string(),
        "<!-- This is an automatically generated file.".to_string(),
        "     It will be read and overwritten.".to_string(),
        "     DO NOT EDIT! -->".to_string(),
        "<META HTTP-EQUIV=\"Content-Type\" CONTENT=\"text/html; charset=UTF-8\">".to_string(),
        "<TITLE>Bookmarks</TITLE>".to_string(),
        "<H1>Bookmarks</H1>".to_string(),
        "<DL><p>".to_string(),
        format!(
            "  <DT><H3 ADD_DATE=\"{add_date}\" LAST_MODIFIED=\"{add_date}\">{}</H3>",
            escape_html(&folder_title)
        ),
        "  <DL><p>".to_string(),
    ];
    let mappings = config
        .get("host_mappings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let groups = if host_mapping_grouped_view_from_config(config) {
        normalize_host_mapping_groups(host_mapping_groups_from_config(config)).unwrap_or_default()
    } else {
        Vec::new()
    };
    if groups.is_empty() {
        for mapping in &mappings {
            append_bookmark_mapping(
                &mut lines,
                mapping,
                "    ",
                scheme,
                &access_entry_port,
                omit_access_entry_port,
                add_date,
            );
        }
    } else {
        let valid_group_ids = groups
            .iter()
            .filter_map(|group| group.get("id").and_then(Value::as_str))
            .collect::<HashSet<_>>();
        for group in &groups {
            let Some(group_id) = group.get("id").and_then(Value::as_str) else {
                continue;
            };
            let members = mappings
                .iter()
                .filter(|mapping| {
                    !mapping
                        .get("target")
                        .and_then(Value::as_str)
                        .is_some_and(is_auth_service_target)
                        && mapping.get("group_id").and_then(Value::as_str) == Some(group_id)
                })
                .collect::<Vec<_>>();
            if members.is_empty() {
                continue;
            }
            let name = group.get("name").and_then(Value::as_str).unwrap_or("");
            append_bookmark_group_start(&mut lines, name, add_date);
            for mapping in members {
                append_bookmark_mapping(
                    &mut lines,
                    mapping,
                    "      ",
                    scheme,
                    &access_entry_port,
                    omit_access_entry_port,
                    add_date,
                );
            }
            append_bookmark_group_end(&mut lines);
        }
        let ungrouped = mappings
            .iter()
            .filter(|mapping| {
                !mapping
                    .get("target")
                    .and_then(Value::as_str)
                    .is_some_and(is_auth_service_target)
                    && mapping
                        .get("group_id")
                        .and_then(Value::as_str)
                        .is_none_or(|id| !valid_group_ids.contains(id))
            })
            .collect::<Vec<_>>();
        if !ungrouped.is_empty() {
            append_bookmark_group_start(
                &mut lines,
                &translator.t("server.admin.hostMappings.ungrouped"),
                add_date,
            );
            for mapping in ungrouped {
                append_bookmark_mapping(
                    &mut lines,
                    mapping,
                    "      ",
                    scheme,
                    &access_entry_port,
                    omit_access_entry_port,
                    add_date,
                );
            }
            append_bookmark_group_end(&mut lines);
        }
    }
    lines.push("  </DL><p>".to_string());
    lines.push("</DL><p>".to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn append_bookmark_group_start(lines: &mut Vec<String>, name: &str, add_date: i64) {
    lines.push(format!(
        "    <DT><H3 ADD_DATE=\"{add_date}\" LAST_MODIFIED=\"{add_date}\">{}</H3>",
        escape_html(name)
    ));
    lines.push("    <DL><p>".to_string());
}

fn append_bookmark_group_end(lines: &mut Vec<String>) {
    lines.push("    </DL><p>".to_string());
}

fn append_bookmark_mapping(
    lines: &mut Vec<String>,
    mapping: &Value,
    indent: &str,
    scheme: &str,
    access_entry_port: &str,
    omit_access_entry_port: bool,
    add_date: i64,
) {
    let Some(object) = mapping.as_object() else {
        return;
    };
    if object
        .get("target")
        .and_then(Value::as_str)
        .is_some_and(is_auth_service_target)
    {
        return;
    }
    let host = object
        .get("host")
        .and_then(Value::as_str)
        .map(normalize_host_value)
        .unwrap_or_default();
    if host.is_empty() {
        return;
    }
    let href = build_bookmark_url(
        &host,
        scheme,
        Some(access_entry_port),
        omit_access_entry_port,
    );
    let title = resolve_bookmark_title(object, &host);
    let icon_attribute = resolve_host_mapping_icon(object)
        .map(|icon| format!(" ICON=\"{}\"", escape_html(&icon)))
        .unwrap_or_default();
    lines.push(format!(
        "{indent}<DT><A HREF=\"{}\" ADD_DATE=\"{add_date}\"{icon_attribute}>{}</A>",
        escape_html(&href),
        escape_html(&title)
    ));
}

pub(super) fn resolve_bookmark_scheme(config: &Value) -> &'static str {
    let cert = config
        .pointer("/ssl/cert")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    let key = config
        .pointer("/ssl/key")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !cert.is_empty() && !key.is_empty() {
        "https"
    } else {
        "http"
    }
}

pub(super) fn build_bookmark_url(
    host: &str,
    scheme: &str,
    access_entry_port: Option<&str>,
    omit_access_entry_port: bool,
) -> String {
    if omit_access_entry_port {
        return format!("{scheme}://{host}/");
    }
    let port = resolve_bookmark_access_entry_port(access_entry_port);
    let parsed_port = parse_js_parse_int_radix_10(&port);
    let port_suffix = if port.is_empty()
        || parsed_port.is_some_and(|port| is_default_scheme_port(scheme, port))
    {
        String::new()
    } else {
        format!(":{port}")
    };
    format!("{scheme}://{host}{port_suffix}/")
}

pub(super) fn resolve_bookmark_access_entry_port(access_entry_port: Option<&str>) -> String {
    let normalized = access_entry_port.unwrap_or("").trim();
    if normalized.is_empty() {
        "7999".to_string()
    } else {
        normalized.to_string()
    }
}

pub(super) fn resolve_bookmark_title(object: &Map<String, Value>, host: &str) -> String {
    let title_override = object
        .get("title_override")
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or("");
    if !title_override.is_empty() {
        return title_override.to_string();
    }
    object
        .get("title")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(host)
        .to_string()
}

pub(super) fn resolve_host_mapping_icon(object: &Map<String, Value>) -> Option<String> {
    ["favicon_override", "favicon"]
        .into_iter()
        .filter_map(|key| object.get(key).and_then(Value::as_str).map(str::trim))
        .find_map(normalize_inline_favicon_data_url)
}

pub(super) fn website_icon_path(object: &Map<String, Value>, sync_id: &str) -> String {
    let extension = resolve_host_mapping_icon(object)
        .map(|value| {
            let content_type = value
                .split_once(',')
                .map(|(metadata, _)| metadata)
                .and_then(|metadata| metadata.strip_prefix("data:"))
                .and_then(|metadata| metadata.split(';').next())
                .unwrap_or("");
            match content_type {
                "image/avif" => "avif",
                "image/gif" => "gif",
                "image/jpeg" => "jpg",
                "image/svg+xml" => "svg",
                "image/vnd.microsoft.icon" | "image/x-icon" => "ico",
                "image/webp" => "webp",
                _ => "png",
            }
        })
        .unwrap_or("png");
    format!("/__assets__/website_icon.{sync_id}.{extension}")
}

pub(crate) fn public_panel_icon_url(
    context: &PublicHostLinkContext,
    object: &Map<String, Value>,
    host: &str,
    sync_id: &str,
) -> String {
    format!(
        "{}{}",
        public_host_url(context, host).trim_end_matches('/'),
        website_icon_path(object, sync_id)
    )
}

pub(super) fn build_bookmark_filename(config: &Value) -> String {
    let root_domain = config
        .pointer("/subdomain_mode/root_domain")
        .and_then(Value::as_str)
        .unwrap_or("");
    let normalized = normalize_bookmark_filename_part(root_domain);
    if normalized.is_empty() {
        "fn-knock-bookmarks.html".to_string()
    } else {
        format!("fn-knock-bookmarks-{normalized}.html")
    }
}

pub(super) fn normalize_bookmark_filename_part(value: &str) -> String {
    let mut output = String::new();
    let mut previous_dash = false;
    for ch in value.trim().to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
            output.push(ch);
            previous_dash = false;
        } else if !previous_dash {
            output.push('-');
            previous_dash = true;
        }
    }
    output.trim_matches('-').to_string()
}
