use super::*;

pub(super) struct DeliveryBuildArgs {
    pub(super) id: String,
    pub(super) trace_id: String,
    pub(super) trigger_id: String,
    pub(super) rule_id: String,
    pub(super) target_id: String,
    pub(super) provider_id: String,
    pub(super) event_id: String,
    pub(super) status: String,
    pub(super) reason: Option<String>,
    pub(super) provider_type: String,
    pub(super) message_snapshot: Value,
    pub(super) target_snapshot: Value,
    pub(super) provider_snapshot: Value,
    pub(super) attempt_count: i64,
    pub(super) triggered_at: String,
    pub(super) next_retry_at: Option<String>,
}

pub(super) fn build_delivery_value(args: DeliveryBuildArgs) -> Value {
    let mut delivery = json!({
        "id": args.id,
        "trigger_id": args.trigger_id,
        "rule_id": args.rule_id,
        "target_id": args.target_id,
        "provider_id": args.provider_id,
        "event_id": args.event_id,
        "status": args.status,
        "reason": args.reason,
        "provider_type": args.provider_type,
        "message_snapshot": args.message_snapshot,
        "target_snapshot": args.target_snapshot,
        "provider_snapshot": args.provider_snapshot,
        "request_summary": Value::Null,
        "response_summary": Value::Null,
        "attempt_count": args.attempt_count,
        "triggered_at": args.triggered_at,
        "sent_at": Value::Null,
        "next_retry_at": args.next_retry_at
    });
    if !args.trace_id.is_empty() {
        delivery["trace_id"] = Value::String(args.trace_id);
    }
    delivery
}

pub(super) fn deleted_provider_snapshot(
    provider_id: &str,
    timestamp: &str,
    translator: &Translator,
) -> Value {
    json!({
        "id": provider_id,
        "name": notification_service_text(translator, "deletedProvider", &[]),
        "type": "webhook",
        "enabled": false,
        "created_at": timestamp,
        "updated_at": timestamp,
        "connection_config_masked": {}
    })
}

pub(super) fn is_terminal_delivery_status(status: Option<&str>) -> bool {
    matches!(status, Some("success" | "gave_up" | "skipped"))
}
