use super::*;

pub(in crate::notifications::routes) use crate::http_utils::html_escape as escape_html;

pub(in crate::notifications::routes) fn build_text_body(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    let body_text = message_text(message, "body_text");
    if body_text.is_empty() {
        push_if_non_empty(&mut sections, message_text(message, "body_markdown"));
    } else {
        push_if_non_empty(&mut sections, body_text);
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(in crate::notifications::routes) fn build_markdown_body(message: &Value, tail: &str) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(
        &mut sections,
        escape_notification_markdown_text(&message_summary(message)),
    );
    let body_markdown = message_text(message, "body_markdown");
    if body_markdown.is_empty() {
        push_if_non_empty(
            &mut sections,
            normalize_multiline_trimmed(&message_text(message, "body_text"), true),
        );
    } else {
        push_if_non_empty(&mut sections, body_markdown);
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_markdown_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_markdown_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    push_if_non_empty(&mut sections, tail.to_string());
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(in crate::notifications::routes) fn build_pushplus_text_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        push_if_non_empty(&mut sections, normalize_multiline_trimmed(&body_text, true));
    } else {
        push_if_non_empty(&mut sections, message_text(message, "body_markdown"));
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_fullwidth_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    default_string(
        sections
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        &message_title(message),
    )
}

pub(in crate::notifications::routes) fn build_pushplus_markdown_content(message: &Value) -> String {
    let body = build_markdown_body(message, "");
    if body.trim().is_empty() {
        build_pushplus_text_content(message)
    } else {
        body
    }
}

pub(in crate::notifications::routes) fn build_pushplus_html_content(message: &Value) -> String {
    let mut sections = Vec::new();
    let summary = message_summary(message);
    if !summary.is_empty() {
        sections.push(format!("<p>{}</p>", escape_html(&summary)));
    }
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let body_html = normalize_multiline_trimmed(&body_text, true)
            .lines()
            .map(escape_html)
            .collect::<Vec<_>>()
            .join("<br />");
        if !body_html.is_empty() {
            sections.push(format!("<p>{body_html}</p>"));
        }
    } else {
        let body_markdown = message_text(message, "body_markdown");
        if !body_markdown.is_empty() {
            sections.push(format!("<pre>{}</pre>", escape_html(&body_markdown)));
        }
    }
    push_html_facts(&mut sections, message);
    push_html_action_list(&mut sections, message);
    default_string(
        sections
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(""),
        &format!("<p>{}</p>", escape_html(&message_title(message))),
    )
}

pub(in crate::notifications::routes) fn build_wxpusher_html_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(
        &mut sections,
        format!("<h2>{}</h2>", escape_html(&message_title(message))),
    );
    let summary = message_summary(message);
    if !summary.is_empty() {
        sections.push(format!("<p>{}</p>", escape_html(&summary)));
    }
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let paragraphs = normalize_multiline_trimmed(&body_text, true)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(|line| format!("<p>{}</p>", escape_html(line)))
            .collect::<String>();
        push_if_non_empty(&mut sections, paragraphs);
    }
    push_html_facts(&mut sections, message);
    push_html_actions_as_paragraphs(&mut sections, message);
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("")
}

pub(in crate::notifications::routes) fn push_html_facts(
    sections: &mut Vec<String>,
    message: &Value,
) {
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        let items = facts
            .iter()
            .filter_map(|fact| {
                let label = fact.get("label").map(value_to_trimmed_string)?;
                let value = fact.get("value").map(value_to_trimmed_string)?;
                Some(format!(
                    "<li><strong>{}</strong>：{}</li>",
                    escape_html(&label),
                    escape_html(&value)
                ))
            })
            .collect::<String>();
        push_if_non_empty(sections, format!("<ul>{items}</ul>"));
    }
}

pub(in crate::notifications::routes) fn push_html_action_list(
    sections: &mut Vec<String>,
    message: &Value,
) {
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        let items = actions
            .iter()
            .filter_map(|action| {
                let label = action.get("label").map(value_to_trimmed_string)?;
                let url = action.get("url").map(value_to_trimmed_string)?;
                if label.is_empty() || url.is_empty() {
                    None
                } else {
                    Some(format!(
                        "<li><a href=\"{}\">{}</a></li>",
                        escape_html(&url),
                        escape_html(&label)
                    ))
                }
            })
            .collect::<String>();
        push_if_non_empty(sections, format!("<ul>{items}</ul>"));
    }
}

pub(in crate::notifications::routes) fn push_html_actions_as_paragraphs(
    sections: &mut Vec<String>,
    message: &Value,
) {
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        let items = actions
            .iter()
            .filter_map(|action| {
                let label = action.get("label").map(value_to_trimmed_string)?;
                let url = action.get("url").map(value_to_trimmed_string)?;
                if label.is_empty() || url.is_empty() {
                    None
                } else {
                    Some(format!(
                        "<p><a href=\"{}\">{}</a></p>",
                        escape_html(&url),
                        escape_html(&label)
                    ))
                }
            })
            .collect::<String>();
        push_if_non_empty(sections, items);
    }
}

pub(in crate::notifications::routes) fn build_pushplus_json_content(message: &Value) -> String {
    serde_json::to_string_pretty(&json!({
        "summary": message.get("summary").cloned().unwrap_or(Value::Null),
        "body_text": message.get("body_text").cloned().unwrap_or(Value::Null),
        "body_markdown": message.get("body_markdown").cloned().unwrap_or(Value::Null),
        "severity": message.get("severity").cloned().unwrap_or(Value::Null),
        "facts": message.get("facts").cloned().unwrap_or_else(|| json!([])),
        "actions": message.get("actions").cloned().unwrap_or_else(|| json!([])),
        "occurred_at": message.get("occurred_at").cloned().unwrap_or(Value::Null),
        "event_id": message.get("event_id").cloned().unwrap_or(Value::Null),
        "metadata": message.get("metadata").cloned().unwrap_or_else(|| json!({})),
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

pub(in crate::notifications::routes) fn build_wecom_markdown_content(
    message: &Value,
    mentioned_list: &[String],
) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(
        &mut sections,
        format!("# {}", sanitize_wecom_text(&message_title(message))),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&message_summary(message)),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&normalize_multiline_trimmed(
            &message_text(message, "body_text"),
            true,
        )),
    );
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        push_if_non_empty(
            &mut sections,
            facts
                .iter()
                .filter_map(|fact| {
                    let label =
                        sanitize_wecom_text(&fact.get("label").map(value_to_trimmed_string)?);
                    let value =
                        sanitize_wecom_text(&fact.get("value").map(value_to_trimmed_string)?);
                    Some(format!("> {label}：{value}"))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array) {
        push_if_non_empty(
            &mut sections,
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .map(|line| format!("> {}", sanitize_wecom_text(&line)))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if !mentioned_list.is_empty() {
        sections.push(
            mentioned_list
                .iter()
                .map(|value| {
                    if value.starts_with('@') {
                        format!("<{value}>")
                    } else {
                        format!("<@{value}>")
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(in crate::notifications::routes) fn build_wecom_text_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, sanitize_wecom_text(&message_title(message)));
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&message_summary(message)),
    );
    push_if_non_empty(
        &mut sections,
        sanitize_wecom_text(&normalize_multiline_trimmed(
            &message_text(message, "body_text"),
            true,
        )),
    );
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_fullwidth_plain_line)
                .map(|line| sanitize_wecom_text(&line))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_fullwidth_plain_line)
                .map(|line| sanitize_wecom_text(&line))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(in crate::notifications::routes) fn build_dingtalk_mention_text(
    at_mobiles: &[String],
    at_user_ids: &[String],
    is_at_all: bool,
) -> String {
    let mut tokens = Vec::new();
    if is_at_all {
        tokens.push("@all".to_string());
    }
    tokens.extend(at_mobiles.iter().map(|value| format!("@{value}")));
    tokens.extend(at_user_ids.iter().map(|value| format!("@{value}")));
    tokens.join(" ")
}

pub(in crate::notifications::routes) fn build_feishu_post_content(
    message: &Value,
    mention_user_ids: &[String],
) -> Value {
    let mut paragraphs: Vec<Value> = Vec::new();
    let body_source =
        message_text(message, "body_text").if_empty(message_text(message, "body_markdown"));
    for section in [message_summary(message), body_source] {
        for line in section
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            paragraphs.push(json!([{ "tag": "text", "text": line }]));
        }
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        for fact in facts {
            if let Some(line) = fact_fullwidth_plain_line(fact) {
                paragraphs.push(json!([{ "tag": "text", "text": line }]));
            }
        }
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array) {
        for action in actions {
            let label = action
                .get("label")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            let url = action
                .get("url")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            if !label.is_empty() && !url.is_empty() {
                paragraphs.push(json!([{ "tag": "a", "text": label, "href": url }]));
            }
        }
    }
    if !mention_user_ids.is_empty() {
        paragraphs.push(Value::Array(
            mention_user_ids
                .iter()
                .map(|user_id| {
                    if user_id == "all" {
                        json!({ "tag": "at", "user_id": "all", "user_name": "所有人" })
                    } else {
                        json!({ "tag": "at", "user_id": user_id })
                    }
                })
                .collect(),
        ));
    }
    if paragraphs.is_empty() {
        paragraphs.push(json!([{ "tag": "text", "text": message_title(message) }]));
    }
    Value::Array(paragraphs)
}

pub(in crate::notifications::routes) fn build_magicpush_content(message: &Value) -> String {
    let mut sections = Vec::new();
    push_if_non_empty(&mut sections, message_summary(message));
    push_if_non_empty(&mut sections, message_text(message, "body_text"));
    if let Some(facts) = message.get("facts").and_then(Value::as_array)
        && !facts.is_empty()
    {
        sections.push(
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(actions) = message.get("actions").and_then(Value::as_array)
        && !actions.is_empty()
    {
        sections.push(
            actions
                .iter()
                .filter_map(action_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(in crate::notifications::routes) fn magicpush_facts_object(message: &Value) -> Value {
    let mut facts = Map::new();
    if let Some(values) = message.get("facts").and_then(Value::as_array) {
        for fact in values {
            let label = fact
                .get("label")
                .map(value_to_trimmed_string)
                .unwrap_or_default();
            if label.is_empty() {
                continue;
            }
            let value = fact
                .get("value")
                .map(js_string_like_node)
                .unwrap_or_default();
            facts.insert(label, Value::String(value));
        }
    }
    Value::Object(facts)
}

pub(in crate::notifications::routes) fn build_bark_payload(
    message: &Value,
    target: &Value,
) -> Value {
    let target_config = target_config(target);
    let summary = message_summary(message);
    let body_text = message_text(message, "body_text");
    let has_standalone_body = !body_text.is_empty() && body_text != summary;
    let mut payload = json!({
        "title": message_title(message),
        "body": if has_standalone_body { body_text.clone() } else { default_string(summary.clone(), &message_title(message)) },
        "level": default_string(config_text(&target_config, "level"), "active")
    });
    if has_standalone_body && !summary.is_empty() {
        insert_string(&mut payload, "subtitle", summary);
    }
    for key in ["group", "sound", "url", "icon"] {
        insert_non_empty(&mut payload, key, config_text(&target_config, key));
    }
    if let Some(action_url) = payload
        .get("url")
        .is_none()
        .then(|| primary_action_url(message))
        && !action_url.is_empty()
    {
        insert_string(&mut payload, "url", action_url);
    }
    if let Some(badge) = optional_nonnegative_i64(target_config.get("badge")) {
        insert_i64(&mut payload, "badge", badge);
    }
    if target_config
        .get("call")
        .map(value_to_bool)
        .unwrap_or(false)
    {
        insert_string(&mut payload, "call", "1".to_string());
    }
    payload
}

pub(in crate::notifications::routes) fn build_telegram_text(message: &Value) -> String {
    let mut plain_sections = Vec::new();
    let mut rich_sections = Vec::new();
    let title = message_title(message);
    push_if_non_empty(&mut plain_sections, title.clone());
    push_if_non_empty(
        &mut rich_sections,
        format!("<b>{}</b>", escape_html(&title)),
    );
    let summary = message_summary(message);
    push_if_non_empty(&mut plain_sections, summary.clone());
    push_if_non_empty(&mut rich_sections, escape_html(&summary));
    let body_text = message_text(message, "body_text");
    if !body_text.is_empty() {
        let normalized = normalize_multiline_trimmed(&body_text, false);
        plain_sections.push(normalized.clone());
        rich_sections.push(
            normalized
                .lines()
                .map(escape_html)
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    if let Some(facts) = message.get("facts").and_then(Value::as_array) {
        push_if_non_empty(
            &mut plain_sections,
            facts
                .iter()
                .filter_map(fact_plain_line)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        push_if_non_empty(
            &mut rich_sections,
            facts
                .iter()
                .filter_map(|fact| {
                    let label = escape_html(&fact.get("label").map(value_to_trimmed_string)?);
                    let value = escape_html(&fact.get("value").map(value_to_trimmed_string)?);
                    Some(format!("<b>{label}:</b> {value}"))
                })
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }
    let rich_text = rich_sections
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
    if rich_text.encode_utf16().count() <= 4096 {
        rich_text
    } else {
        escape_html(&truncate_text(
            &plain_sections
                .into_iter()
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n"),
            4096,
        ))
    }
}

pub(in crate::notifications::routes) fn build_telegram_reply_markup(
    message: &Value,
) -> Option<Value> {
    let buttons = message
        .get("actions")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|action| {
            let label = action.get("label").map(value_to_trimmed_string)?;
            let url = action.get("url").map(value_to_trimmed_string)?;
            if label.is_empty() || url.is_empty() {
                None
            } else {
                Some(json!([{ "text": label, "url": url }]))
            }
        })
        .collect::<Vec<_>>();
    (!buttons.is_empty()).then(|| json!({ "inline_keyboard": buttons }))
}

pub(in crate::notifications::routes) fn fact_plain_line(fact: &Value) -> Option<String> {
    let label = fact.get("label").map(value_to_trimmed_string)?;
    let value = fact.get("value").map(value_to_trimmed_string)?;
    if label.is_empty() && value.is_empty() {
        None
    } else if label.is_empty() {
        Some(value)
    } else if value.is_empty() {
        Some(label)
    } else {
        Some(format!("{label}: {value}"))
    }
}

pub(in crate::notifications::routes) fn fact_fullwidth_plain_line(fact: &Value) -> Option<String> {
    let label = fact.get("label").map(value_to_trimmed_string)?;
    let value = fact.get("value").map(value_to_trimmed_string)?;
    if label.is_empty() && value.is_empty() {
        None
    } else if label.is_empty() {
        Some(value)
    } else if value.is_empty() {
        Some(label)
    } else {
        Some(format!("{label}：{value}"))
    }
}

pub(in crate::notifications::routes) fn fact_markdown_line(fact: &Value) -> Option<String> {
    let label = fact
        .get("label")
        .map(value_to_trimmed_string)
        .map(|value| escape_notification_markdown_text(&value))?;
    let value = fact
        .get("value")
        .map(value_to_trimmed_string)
        .map(|value| escape_notification_markdown_text(&value))?;
    if label.is_empty() && value.is_empty() {
        None
    } else {
        Some(format!("- **{label}**：{value}"))
    }
}

pub(in crate::notifications::routes) fn action_plain_line(action: &Value) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("{label}: {url}"))
    }
}

pub(in crate::notifications::routes) fn action_fullwidth_plain_line(
    action: &Value,
) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("{label}：{url}"))
    }
}

pub(in crate::notifications::routes) fn action_markdown_line(action: &Value) -> Option<String> {
    let label = action.get("label").map(value_to_trimmed_string)?;
    let url = action.get("url").map(value_to_trimmed_string)?;
    if label.is_empty() || url.is_empty() {
        None
    } else {
        Some(format!("- [{label}]({url})"))
    }
}

pub(in crate::notifications::routes) fn primary_action_url(message: &Value) -> String {
    message
        .get("actions")
        .and_then(Value::as_array)
        .and_then(|actions| {
            actions.iter().find_map(|action| {
                let url = action.get("url").map(value_to_trimmed_string)?;
                (!url.is_empty()).then_some(url)
            })
        })
        .unwrap_or_default()
}

pub(in crate::notifications::routes) fn push_if_non_empty(values: &mut Vec<String>, value: String) {
    if !value.trim().is_empty() {
        values.push(value);
    }
}

pub(in crate::notifications::routes) fn normalize_multiline_trimmed(
    value: &str,
    drop_empty_lines: bool,
) -> String {
    let lines = value.lines().map(str::trim);
    if drop_empty_lines {
        lines
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        lines.collect::<Vec<_>>().join("\n")
    }
}

pub(in crate::notifications::routes) fn push_form_if(
    form: &mut Vec<(String, String)>,
    key: &str,
    value: String,
) {
    if !value.trim().is_empty() {
        form.push((key.to_string(), value));
    }
}

pub(in crate::notifications::routes) fn insert_non_empty(
    object: &mut Value,
    key: &str,
    value: String,
) {
    if !value.trim().is_empty() {
        insert_string(object, key, value);
    }
}

pub(in crate::notifications::routes) fn insert_string(
    object: &mut Value,
    key: &str,
    value: String,
) {
    insert_value(object, key, Value::String(value));
}

pub(in crate::notifications::routes) fn insert_i64(object: &mut Value, key: &str, value: i64) {
    insert_value(object, key, json!(value));
}

pub(in crate::notifications::routes) fn insert_value(object: &mut Value, key: &str, value: Value) {
    if let Some(map) = object.as_object_mut() {
        map.insert(key.to_string(), value);
    }
}

pub(in crate::notifications::routes) fn truncate_utf8_bytes(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }
    let mut bytes = 0;
    let mut output = String::new();
    for ch in value.chars() {
        let len = ch.len_utf8();
        if bytes + len > limit {
            break;
        }
        bytes += len;
        output.push(ch);
    }
    output
}

pub(in crate::notifications::routes) fn sanitize_wecom_text(value: &str) -> String {
    value.replace('<', "＜").replace('>', "＞")
}
