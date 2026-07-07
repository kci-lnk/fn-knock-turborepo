use super::*;

pub(super) fn frpc_text(translator: &Translator, key: &str) -> String {
    translator.t(&format!("server.frpc.{key}"))
}

pub(super) fn frpc_text_params(
    translator: &Translator,
    key: &str,
    params: &[(&str, String)],
) -> String {
    translator.t_params(&format!("server.frpc.{key}"), params)
}

pub(super) fn default_frpc_text(key: &str) -> String {
    frpc_text(&Translator::new(DEFAULT_LOCALE), key)
}

pub(super) fn default_frpc_primary_name() -> String {
    default_frpc_text("primaryName")
}

pub(super) fn default_frpc_instance_name() -> String {
    default_frpc_text("instanceName")
}

pub(super) fn localize_frpc_error(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    if let Some(id) = message.strip_prefix("FRPC instance not found: ") {
        return frpc_text_params(translator, "instanceNotFound", &[("id", id.to_string())]);
    }
    if let Some(limit) = message
        .strip_prefix("FRPC instance limit exceeded (")
        .and_then(|value| value.strip_suffix(')'))
    {
        return frpc_text_params(
            translator,
            "instanceLimitExceeded",
            &[("limit", limit.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("Failed to verify frpc config: ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }
    if let Some(code) = message.strip_prefix("frpc config verify failed with code ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithCode",
            &[("code", code.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("frpc config verify failed: ") {
        return frpc_text_params(
            translator,
            "verifyFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("Failed to start frpc: ") {
        return frpc_text_params(
            translator,
            "startFailedWithDetail",
            &[("detail", detail.to_string())],
        );
    }

    match message {
        "Primary FRPC instance cannot be deleted" => frpc_text(translator, "primaryDeleteDenied"),
        "FRP is not initialized" => frpc_text(translator, "notInitialized"),
        "Failed to read frpc pid" => frpc_text(translator, "pidReadFailed"),
        _ => message.to_string(),
    }
}

pub(super) fn localize_frpc_response_value(mut value: Value, translator: &Translator) -> Value {
    localize_frpc_value_in_place(&mut value, translator);
    value
}

pub(super) fn localize_frpc_value_in_place(value: &mut Value, translator: &Translator) {
    match value {
        Value::Object(object) => {
            for key in ["lastMessage", "last_message"] {
                if let Some(message) = object.get(key).and_then(Value::as_str) {
                    object.insert(
                        key.to_string(),
                        Value::String(localize_frpc_runtime_message(translator, message)),
                    );
                }
            }
            for child in object.values_mut() {
                localize_frpc_value_in_place(child, translator);
            }
        }
        Value::Array(items) => {
            for item in items {
                localize_frpc_value_in_place(item, translator);
            }
        }
        _ => {}
    }
}

pub(super) fn localize_frpc_runtime_message(translator: &Translator, message: &str) -> String {
    let message = message.trim();
    if let Some(pid) = message.strip_prefix("frpc started pid=") {
        return frpc_text_params(translator, "startedWithPid", &[("pid", pid.to_string())]);
    }
    if let Some(pid) = message.strip_prefix("frpc stopped pid=") {
        return frpc_text_params(translator, "stoppedWithPid", &[("pid", pid.to_string())]);
    }
    if let Some(code) = message.strip_prefix("frpc exited with code ") {
        return frpc_text_params(
            translator,
            "processExitedWithCode",
            &[("code", code.to_string())],
        );
    }
    if let Some(detail) = message.strip_prefix("frpc process error: ") {
        return frpc_text_params(
            translator,
            "processCrashed",
            &[("message", detail.to_string())],
        );
    }
    match message {
        "frpc pid is no longer running" => frpc_text(translator, "pidInvalidForInstance"),
        "frpc already stopped" => frpc_text(translator, "alreadyStopped"),
        _ => message.to_string(),
    }
}
