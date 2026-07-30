use serde_json::Value;

pub(crate) fn response_success(value: &Value) -> bool {
    value
        .get("success")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

pub(crate) fn response_message(value: &Value, fallback: &str) -> String {
    value
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message| !message.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub(crate) fn ensure_response_success(value: &Value, fallback: &str) -> Result<(), String> {
    if response_success(value) {
        Ok(())
    } else {
        Err(response_message(value, fallback))
    }
}

pub(crate) fn applied_response_data<'a>(
    value: &'a Value,
    unsuccessful_fallback: &str,
    missing_data: &str,
) -> Result<&'a Value, String> {
    ensure_response_success(value, unsuccessful_fallback)?;
    value.get("data").ok_or_else(|| missing_data.to_string())
}

pub(crate) fn applied_response_object<'a>(
    value: &'a Value,
    unsuccessful_fallback: &str,
    missing_data: &str,
) -> Result<&'a Value, String> {
    let data = applied_response_data(value, unsuccessful_fallback, missing_data)?;
    data.is_object()
        .then_some(data)
        .ok_or_else(|| missing_data.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{applied_response_object, ensure_response_success};

    #[test]
    fn missing_success_field_remains_backward_compatible() {
        assert!(ensure_response_success(&json!({}), "failed").is_ok());
    }

    #[test]
    fn failed_ack_uses_trimmed_gateway_message() {
        assert_eq!(
            ensure_response_success(
                &json!({"success": false, "message": "  rejected  "}),
                "failed"
            ),
            Err("rejected".to_string())
        );
    }

    #[test]
    fn applied_object_requires_success_and_object_data() {
        assert!(
            applied_response_object(
                &json!({"success": true, "data": {"enabled": true}}),
                "failed",
                "missing"
            )
            .is_ok()
        );
        assert_eq!(
            applied_response_object(&json!({"success": true, "data": null}), "failed", "missing"),
            Err("missing".to_string())
        );
    }
}
