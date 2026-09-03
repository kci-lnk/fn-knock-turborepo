use super::*;

pub(in crate::notifications::routes) const WEBHOOK_MAX_BODY_TEMPLATE_BYTES: usize = 64 * 1024;
pub(in crate::notifications::routes) const WEBHOOK_MAX_BODY_SAMPLE_BYTES: usize = 64 * 1024;
pub(in crate::notifications::routes) const WEBHOOK_MAX_BODY_PLACEHOLDERS: usize = 256;
pub(in crate::notifications::routes) const WEBHOOK_MAX_RENDERED_BODY_BYTES: usize = 256 * 1024;
pub(in crate::notifications::routes) const WEBHOOK_MAX_CONTENT_TYPE_BYTES: usize = 256;
const WEBHOOK_MAX_VARIABLE_PATH_BYTES: usize = 256;

pub(in crate::notifications::routes) const WEBHOOK_BODY_VARIABLE_ROOTS: &[&str] = &[
    "message", "event", "context", "rule", "target", "provider", "legacy",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::notifications::routes) enum WebhookBodyScope {
    Provider,
    Target,
}

impl WebhookBodyScope {
    fn default_mode(self) -> &'static str {
        match self {
            Self::Provider => "standard",
            Self::Target => "inherit",
        }
    }

    fn accepts_mode(self, mode: &str) -> bool {
        match self {
            Self::Provider => matches!(mode, "standard" | "custom"),
            Self::Target => matches!(mode, "inherit" | "custom"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::notifications::routes) enum WebhookBodyFormat {
    Json,
    Text,
}

impl WebhookBodyFormat {
    pub(in crate::notifications::routes) fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Text => "text",
        }
    }

    fn default_content_type(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Text => "text/plain; charset=utf-8",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::notifications::routes) struct WebhookBodyConfig {
    pub(in crate::notifications::routes) mode: String,
    pub(in crate::notifications::routes) format: WebhookBodyFormat,
    pub(in crate::notifications::routes) content_type: String,
    pub(in crate::notifications::routes) template: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::notifications::routes) struct WebhookBodyValidationError {
    key: &'static str,
    params: Vec<(&'static str, String)>,
}

impl WebhookBodyValidationError {
    pub(in crate::notifications::routes) fn text(&self, translator: &Translator) -> String {
        notification_provider_error_text(translator, "webhook", self.key, &self.params)
    }

    pub(in crate::notifications::routes) fn default_text(&self) -> String {
        notification_provider_error_default("webhook", self.key, &self.params)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::notifications::routes) struct RenderedWebhookBody {
    pub(in crate::notifications::routes) format: WebhookBodyFormat,
    pub(in crate::notifications::routes) content_type: String,
    pub(in crate::notifications::routes) bytes: Vec<u8>,
    pub(in crate::notifications::routes) missing_variables: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TemplateSegment {
    Literal(String),
    Variable(String),
}

fn body_error(key: &'static str, params: &[(&'static str, String)]) -> WebhookBodyValidationError {
    WebhookBodyValidationError {
        key,
        params: params.to_vec(),
    }
}

fn body_mode(value: &Value, scope: WebhookBodyScope) -> Result<String, WebhookBodyValidationError> {
    let object = value
        .as_object()
        .ok_or_else(|| body_error("invalidBodyConfig", &[]))?;
    let raw_mode = match object.get("mode") {
        None => None,
        Some(Value::String(mode)) => Some(mode.as_str()),
        Some(_) => return Err(body_error("invalidBodyConfig", &[])),
    };
    let mode = raw_mode
        .map(str::trim)
        .filter(|mode| !mode.is_empty())
        .unwrap_or_else(|| scope.default_mode())
        .to_ascii_lowercase();
    if !scope.accepts_mode(&mode) {
        return Err(body_error("invalidBodyMode", &[("mode", mode)]));
    }
    Ok(mode)
}

fn parse_body_format(
    value: Option<&Value>,
) -> Result<WebhookBodyFormat, WebhookBodyValidationError> {
    let raw_format = match value {
        None => None,
        Some(Value::String(format)) => Some(format.as_str()),
        Some(_) => {
            return Err(body_error(
                "invalidBodyFormat",
                &[("format", String::new())],
            ));
        }
    };
    match raw_format
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("json")
        .to_ascii_lowercase()
        .as_str()
    {
        "json" => Ok(WebhookBodyFormat::Json),
        "text" => Ok(WebhookBodyFormat::Text),
        format => Err(body_error(
            "invalidBodyFormat",
            &[("format", format.to_string())],
        )),
    }
}

fn parse_content_type(
    value: Option<&Value>,
    format: WebhookBodyFormat,
) -> Result<String, WebhookBodyValidationError> {
    let raw = match value {
        None => "",
        Some(Value::String(content_type)) => content_type.as_str(),
        Some(_) => return Err(body_error("invalidBodyContentType", &[])),
    };
    if raw.chars().any(char::is_control) {
        return Err(body_error("invalidBodyContentType", &[]));
    }
    let content_type = raw.trim();
    let content_type = if content_type.is_empty() {
        format.default_content_type()
    } else {
        content_type
    };
    if content_type.len() > WEBHOOK_MAX_CONTENT_TYPE_BYTES {
        return Err(body_error(
            "bodyContentTypeTooLong",
            &[("max", WEBHOOK_MAX_CONTENT_TYPE_BYTES.to_string())],
        ));
    }
    content_type
        .parse::<mime::Mime>()
        .map_err(|_| body_error("invalidBodyContentType", &[]))?;
    reqwest::header::HeaderValue::from_str(content_type)
        .map_err(|_| body_error("invalidBodyContentType", &[]))?;
    Ok(content_type.to_string())
}

fn valid_variable_path(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= WEBHOOK_MAX_VARIABLE_PATH_BYTES
        && path
            .split('.')
            .next()
            .is_some_and(|root| WEBHOOK_BODY_VARIABLE_ROOTS.contains(&root))
        && path.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        })
}

fn parse_template_segments(
    input: &str,
) -> Result<(Vec<TemplateSegment>, usize), WebhookBodyValidationError> {
    let mut segments = Vec::new();
    let mut literal = String::new();
    let mut cursor = 0_usize;
    let mut placeholders = 0_usize;

    while let Some(offset) = input[cursor..].find("{{") {
        let start = cursor + offset;
        let escaping_backslashes = input[..start]
            .bytes()
            .rev()
            .take_while(|byte| *byte == b'\\')
            .count();
        if escaping_backslashes % 2 == 1 {
            literal.push_str(&input[cursor..start - 1]);
            literal.push_str("{{");
            cursor = start + 2;
            continue;
        }

        literal.push_str(&input[cursor..start]);
        if !literal.is_empty() {
            segments.push(TemplateSegment::Literal(std::mem::take(&mut literal)));
        }
        let variable_start = start + 2;
        let Some(end_offset) = input[variable_start..].find("}}") else {
            return Err(body_error("unclosedBodyVariable", &[]));
        };
        let end = variable_start + end_offset;
        let path = input[variable_start..end].trim();
        if !valid_variable_path(path) {
            return Err(body_error(
                "invalidBodyVariable",
                &[("path", path.to_string())],
            ));
        }
        placeholders = placeholders.saturating_add(1);
        if placeholders > WEBHOOK_MAX_BODY_PLACEHOLDERS {
            return Err(body_error(
                "tooManyBodyVariables",
                &[("max", WEBHOOK_MAX_BODY_PLACEHOLDERS.to_string())],
            ));
        }
        segments.push(TemplateSegment::Variable(path.to_string()));
        cursor = end + 2;
    }
    literal.push_str(&input[cursor..]);
    if !literal.is_empty() || segments.is_empty() {
        segments.push(TemplateSegment::Literal(literal));
    }
    Ok((segments, placeholders))
}

fn validate_json_template_value(
    value: &Value,
    placeholders: &mut usize,
) -> Result<(), WebhookBodyValidationError> {
    match value {
        Value::Object(object) => {
            for (key, value) in object {
                let (_, key_placeholders) = parse_template_segments(key)?;
                *placeholders = placeholders.saturating_add(key_placeholders);
                if *placeholders > WEBHOOK_MAX_BODY_PLACEHOLDERS {
                    return Err(body_error(
                        "tooManyBodyVariables",
                        &[("max", WEBHOOK_MAX_BODY_PLACEHOLDERS.to_string())],
                    ));
                }
                validate_json_template_value(value, placeholders)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                validate_json_template_value(value, placeholders)?;
            }
        }
        Value::String(value) => {
            let (_, count) = parse_template_segments(value)?;
            *placeholders = placeholders.saturating_add(count);
            if *placeholders > WEBHOOK_MAX_BODY_PLACEHOLDERS {
                return Err(body_error(
                    "tooManyBodyVariables",
                    &[("max", WEBHOOK_MAX_BODY_PLACEHOLDERS.to_string())],
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_body_template(
    template: &str,
    format: WebhookBodyFormat,
) -> Result<(), WebhookBodyValidationError> {
    if template.len() > WEBHOOK_MAX_BODY_TEMPLATE_BYTES {
        return Err(body_error(
            "bodyTemplateTooLarge",
            &[("max", WEBHOOK_MAX_BODY_TEMPLATE_BYTES.to_string())],
        ));
    }
    match format {
        WebhookBodyFormat::Json => {
            let parsed = serde_json::from_str::<Value>(template)
                .map_err(|_| body_error("invalidBodyTemplateJson", &[]))?;
            let mut placeholders = 0;
            validate_json_template_value(&parsed, &mut placeholders)
        }
        WebhookBodyFormat::Text => {
            parse_template_segments(template)?;
            Ok(())
        }
    }
}

pub(in crate::notifications::routes) fn parse_webhook_body_config(
    value: &Value,
    scope: WebhookBodyScope,
) -> Result<WebhookBodyConfig, WebhookBodyValidationError> {
    let mode = body_mode(value, scope)?;
    if mode != "custom" {
        return Ok(WebhookBodyConfig {
            mode,
            format: WebhookBodyFormat::Json,
            content_type: "application/json".to_string(),
            template: String::new(),
        });
    }
    let object = value
        .as_object()
        .ok_or_else(|| body_error("invalidBodyConfig", &[]))?;
    let format = parse_body_format(object.get("format"))?;
    let template = object
        .get("template")
        .and_then(Value::as_str)
        .ok_or_else(|| body_error("bodyTemplateRequired", &[]))?
        .to_string();
    if format == WebhookBodyFormat::Json && template.trim().is_empty() {
        return Err(body_error("bodyTemplateRequired", &[]));
    }
    validate_body_template(&template, format)?;
    let content_type = parse_content_type(object.get("content_type"), format)?;
    Ok(WebhookBodyConfig {
        mode,
        format,
        content_type,
        template,
    })
}

pub(in crate::notifications::routes) fn normalize_webhook_body_config(
    value: &Value,
    scope: WebhookBodyScope,
) -> NotifyResult<Value> {
    parse_webhook_body_config(value, scope)
        .map(|config| {
            if config.mode != "custom" {
                return json!({ "mode": config.mode });
            }
            json!({
                "mode": config.mode,
                "format": config.format.as_str(),
                "content_type": config.content_type,
                "template": config.template
            })
        })
        .map_err(|error| NotifyError::BadRequest(error.default_text()))
}

pub(in crate::notifications::routes) fn resolve_webhook_body_config(
    provider_config: &Map<String, Value>,
    target_config: Option<&Map<String, Value>>,
) -> Result<Option<WebhookBodyConfig>, WebhookBodyValidationError> {
    if let Some(target_body) = target_config.and_then(|config| config.get("body_override")) {
        let config = parse_webhook_body_config(target_body, WebhookBodyScope::Target)?;
        if config.mode == "custom" {
            return Ok(Some(config));
        }
    }
    if let Some(provider_body) = provider_config.get("body_config") {
        let config = parse_webhook_body_config(provider_body, WebhookBodyScope::Provider)?;
        if config.mode == "custom" {
            return Ok(Some(config));
        }
    }
    Ok(None)
}

fn resolve_variable<'a>(context: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = context;
    for segment in path.split('.') {
        current = match current {
            Value::Object(object) => object.get(segment)?,
            Value::Array(values) => values.get(segment.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn variable_to_text(value: Option<&Value>) -> String {
    match value {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(value)) => value.clone(),
        Some(Value::Bool(value)) => value.to_string(),
        Some(Value::Number(value)) => value.to_string(),
        Some(value @ (Value::Array(_) | Value::Object(_))) => {
            serde_json::to_string(value).unwrap_or_default()
        }
    }
}

fn note_missing(path: &str, missing: &mut Vec<String>, seen: &mut HashSet<String>) {
    if seen.insert(path.to_string()) {
        missing.push(path.to_string());
    }
}

fn render_template_string(
    template: &str,
    context: &Value,
    preserve_exact_type: bool,
    missing: &mut Vec<String>,
    seen_missing: &mut HashSet<String>,
) -> Result<Value, WebhookBodyValidationError> {
    let (segments, _) = parse_template_segments(template)?;
    if preserve_exact_type
        && segments.len() == 1
        && let TemplateSegment::Variable(path) = &segments[0]
    {
        if let Some(value) = resolve_variable(context, path) {
            return Ok(value.clone());
        }
        note_missing(path, missing, seen_missing);
        return Ok(Value::Null);
    }

    let mut output = String::new();
    for segment in segments {
        match segment {
            TemplateSegment::Literal(value) => output.push_str(&value),
            TemplateSegment::Variable(path) => {
                let value = resolve_variable(context, &path);
                if value.is_none() {
                    note_missing(&path, missing, seen_missing);
                }
                output.push_str(&variable_to_text(value));
            }
        }
    }
    Ok(Value::String(output))
}

fn render_json_template_value(
    template: Value,
    context: &Value,
    missing: &mut Vec<String>,
    seen_missing: &mut HashSet<String>,
) -> Result<Value, WebhookBodyValidationError> {
    match template {
        Value::Object(object) => {
            let mut rendered = Map::new();
            for (key, value) in object {
                let rendered_key =
                    render_template_string(&key, context, false, missing, seen_missing)?
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                if rendered.contains_key(&rendered_key) {
                    return Err(body_error("duplicateRenderedBodyKey", &[]));
                }
                rendered.insert(
                    rendered_key,
                    render_json_template_value(value, context, missing, seen_missing)?,
                );
            }
            Ok(Value::Object(rendered))
        }
        Value::Array(values) => values
            .into_iter()
            .map(|value| render_json_template_value(value, context, missing, seen_missing))
            .collect::<Result<Vec<_>, _>>()
            .map(Value::Array),
        Value::String(value) => {
            render_template_string(&value, context, true, missing, seen_missing)
        }
        value => Ok(value),
    }
}

pub(in crate::notifications::routes) fn render_webhook_body(
    config: &WebhookBodyConfig,
    context: &Value,
) -> Result<RenderedWebhookBody, WebhookBodyValidationError> {
    validate_body_template(&config.template, config.format)?;
    let mut missing_variables = Vec::new();
    let mut seen_missing = HashSet::new();
    let bytes = match config.format {
        WebhookBodyFormat::Json => {
            let template = serde_json::from_str::<Value>(&config.template)
                .map_err(|_| body_error("invalidBodyTemplateJson", &[]))?;
            let rendered = render_json_template_value(
                template,
                context,
                &mut missing_variables,
                &mut seen_missing,
            )?;
            serde_json::to_vec(&rendered).map_err(|_| body_error("invalidBodyTemplateJson", &[]))?
        }
        WebhookBodyFormat::Text => render_template_string(
            &config.template,
            context,
            false,
            &mut missing_variables,
            &mut seen_missing,
        )?
        .as_str()
        .unwrap_or_default()
        .as_bytes()
        .to_vec(),
    };
    if bytes.len() > WEBHOOK_MAX_RENDERED_BODY_BYTES {
        return Err(body_error(
            "renderedBodyTooLarge",
            &[("max", WEBHOOK_MAX_RENDERED_BODY_BYTES.to_string())],
        ));
    }
    Ok(RenderedWebhookBody {
        format: config.format,
        content_type: config.content_type.clone(),
        bytes,
        missing_variables,
    })
}

pub(in crate::notifications::routes) fn render_standard_webhook_body(
    body: &Value,
) -> Result<RenderedWebhookBody, WebhookBodyValidationError> {
    let bytes = serde_json::to_vec(body).map_err(|_| body_error("invalidBodyTemplateJson", &[]))?;
    if bytes.len() > WEBHOOK_MAX_RENDERED_BODY_BYTES {
        return Err(body_error(
            "renderedBodyTooLarge",
            &[("max", WEBHOOK_MAX_RENDERED_BODY_BYTES.to_string())],
        ));
    }
    Ok(RenderedWebhookBody {
        format: WebhookBodyFormat::Json,
        content_type: "application/json".to_string(),
        bytes,
        missing_variables: Vec::new(),
    })
}

fn project_object(value: &Value, keys: &[&str]) -> Value {
    let mut projected = Map::new();
    if let Some(object) = value.as_object() {
        for key in keys {
            if let Some(value) = object.get(*key) {
                projected.insert((*key).to_string(), value.clone());
            }
        }
    }
    Value::Object(projected)
}

pub(in crate::notifications::routes) fn sanitize_webhook_event_snapshot(event: &Value) -> Value {
    fn strip_trace_fields(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.remove("trace_id");
                object.remove("waf_trace_id");
                object.values_mut().for_each(strip_trace_fields);
            }
            Value::Array(values) => values.iter_mut().for_each(strip_trace_fields),
            _ => {}
        }
    }

    let mut sanitized = project_object(
        event,
        &[
            "id",
            "type",
            "source",
            "level",
            "happened_at",
            "dedupe_key",
            "subject",
            "tags",
            "payload",
        ],
    );
    strip_trace_fields(&mut sanitized);
    sanitized
}

#[allow(clippy::too_many_arguments)]
pub(in crate::notifications::routes) fn build_webhook_template_context(
    message: &Value,
    event: &Value,
    context: Value,
    rule: &Value,
    target: &Value,
    provider: &Value,
    extra_body: Value,
) -> Value {
    json!({
        "message": sanitize_notification_message(message),
        "event": sanitize_webhook_event_snapshot(event),
        "context": context,
        "rule": project_object(rule, &["id", "name", "event_type", "group_by", "window_seconds", "threshold_count", "cooldown_seconds"]),
        "target": project_object(target, &["id", "provider_id"]),
        "provider": project_object(provider, &["id", "name", "type"]),
        "legacy": { "extra_body": extra_body }
    })
}

pub(in crate::notifications::routes) fn apply_webhook_sample_context(
    mut context: Value,
    sample: Option<&Value>,
    mode: &str,
    provider: &Value,
) -> Result<Value, WebhookBodyValidationError> {
    if let Some(sample) = sample {
        let sample_bytes =
            serde_json::to_vec(sample).map_err(|_| body_error("invalidBodySample", &[]))?;
        if sample_bytes.len() > WEBHOOK_MAX_BODY_SAMPLE_BYTES {
            return Err(body_error(
                "bodySampleTooLarge",
                &[("max", WEBHOOK_MAX_BODY_SAMPLE_BYTES.to_string())],
            ));
        }
        let sample = sample
            .as_object()
            .ok_or_else(|| body_error("invalidBodySample", &[]))?;
        if let Some(target) = context.as_object_mut() {
            for root in WEBHOOK_BODY_VARIABLE_ROOTS {
                if *root == "provider" {
                    continue;
                }
                if let Some(value) = sample.get(*root) {
                    target.insert((*root).to_string(), value.clone());
                }
            }
        }
    }
    if let Some(object) = context.as_object_mut() {
        if let Some(message) = object.get_mut("message") {
            *message = sanitize_notification_message(message);
        }
        if let Some(rule) = object.get_mut("rule") {
            *rule = project_object(
                rule,
                &[
                    "id",
                    "name",
                    "event_type",
                    "group_by",
                    "window_seconds",
                    "threshold_count",
                    "cooldown_seconds",
                ],
            );
        }
        if let Some(target) = object.get_mut("target") {
            *target = project_object(target, &["id", "provider_id"]);
        }
        if let Some(legacy) = object.get_mut("legacy") {
            *legacy = json!({
                "extra_body": legacy
                    .get("extra_body")
                    .cloned()
                    .unwrap_or_else(|| json!({}))
            });
        }
        object.insert(
            "provider".to_string(),
            project_object(provider, &["id", "name", "type"]),
        );
        let context_value = object
            .entry("context".to_string())
            .or_insert_with(|| json!({}));
        *context_value = project_object(
            context_value,
            &[
                "mode",
                "trigger_id",
                "delivery_id",
                "event_id",
                "rule_id",
                "target_id",
                "provider_id",
            ],
        );
        context_value
            .as_object_mut()
            .expect("project_object always returns an object")
            .insert("mode".to_string(), Value::String(mode.to_string()));
        let event = object.entry("event".to_string()).or_insert(Value::Null);
        *event = sanitize_webhook_event_snapshot(event);
    }
    Ok(context)
}
