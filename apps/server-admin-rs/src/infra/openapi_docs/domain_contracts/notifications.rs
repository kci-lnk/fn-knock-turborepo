use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationFieldOptionData {
    label: String,
    value: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationSchemaFieldData {
    key: String,
    label: String,
    description: Option<String>,
    placeholder: Option<String>,
    #[serde(rename = "type")]
    field_type: String,
    required: Option<bool>,
    sensitive: Option<bool>,
    default_value: Option<Value>,
    options: Option<Vec<NotificationFieldOptionData>>,
    min: Option<i64>,
    max: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderCapabilitiesData {
    supports_text: bool,
    supports_markdown: bool,
    supports_rich_blocks: bool,
    supports_actions: bool,
    supports_mentions: bool,
    supports_attachments: bool,
    supports_provider_dedupe_key: bool,
    #[schema(required = true)]
    max_body_length: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderDefinitionData {
    #[serde(rename = "type")]
    provider_type: String,
    label: String,
    description: String,
    connection_schema: Vec<NotificationSchemaFieldData>,
    target_schema: Vec<NotificationSchemaFieldData>,
    sensitive_fields: Vec<String>,
    capabilities: NotificationProviderCapabilitiesData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderCatalogData {
    providers: Vec<NotificationProviderDefinitionData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderData {
    id: String,
    name: String,
    #[serde(rename = "type")]
    provider_type: String,
    enabled: bool,
    connection_config_masked: Value,
    created_at: String,
    updated_at: String,
    #[schema(required = true)]
    last_test_at: Option<String>,
    #[schema(required = true)]
    last_test_status: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderDetailData {
    id: String,
    name: String,
    #[serde(rename = "type")]
    provider_type: String,
    enabled: bool,
    connection_config_masked: Value,
    connection_config: Value,
    created_at: String,
    updated_at: String,
    #[schema(required = true)]
    last_test_at: Option<String>,
    #[schema(required = true)]
    last_test_status: Option<String>,
    #[schema(required = true)]
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderSnapshotData {
    id: String,
    name: String,
    #[serde(rename = "type")]
    provider_type: String,
    enabled: bool,
    connection_config_masked: Value,
    created_at: String,
    updated_at: String,
    last_test_at: Option<String>,
    last_test_status: Option<String>,
    last_error: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderListData {
    providers: Vec<NotificationProviderData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderCreateBodyData {
    name: Option<String>,
    #[serde(rename = "type")]
    provider_type: String,
    enabled: Option<bool>,
    connection_config: Value,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderUpdateBodyData {
    name: Option<String>,
    enabled: Option<bool>,
    connection_config: Option<Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderTestBodyData {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    provider_type: String,
    enabled: Option<bool>,
    connection_config: Value,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderTestResultData {
    #[schema(required = true)]
    provider: Option<NotificationProviderData>,
    #[schema(required = true)]
    request_summary: Option<Value>,
    #[schema(required = true)]
    response_summary: Option<Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationProviderTestResponseData {
    success: bool,
    message: String,
    data: NotificationProviderTestResultData,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationTemplateData {
    title: Option<String>,
    body_text: Option<String>,
    body_markdown: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationDeliveryPolicyData {
    timeout_seconds: Option<i64>,
    max_attempts: Option<i64>,
    backoff_seconds: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationTargetData {
    id: String,
    provider_id: String,
    enabled: bool,
    target_config: Value,
    template_override_mode: String,
    #[schema(required = true)]
    template_override: Option<NotificationTemplateData>,
    #[schema(required = true)]
    delivery_policy: Option<NotificationDeliveryPolicyData>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationTargetInputData {
    id: Option<String>,
    provider_id: String,
    enabled: Option<bool>,
    target_config: Value,
    template_override_mode: Option<String>,
    template_override: Option<NotificationTemplateData>,
    delivery_policy: Option<NotificationDeliveryPolicyData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationRuleData {
    id: String,
    name: String,
    enabled: bool,
    event_type: String,
    event_level_filter: Option<Vec<String>>,
    event_source_filter: Option<Vec<String>>,
    window_seconds: i64,
    threshold_count: i64,
    group_by: String,
    cooldown_seconds: i64,
    targets: Vec<NotificationTargetData>,
    message_template_mode: String,
    #[schema(required = true)]
    message_template: Option<NotificationTemplateData>,
    created_at: String,
    updated_at: String,
    #[schema(required = true)]
    last_triggered_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationRuleCreateBodyData {
    enabled: Option<bool>,
    event_type: String,
    event_level_filter: Option<Vec<String>>,
    event_source_filter: Option<Vec<String>>,
    window_seconds: Option<i64>,
    threshold_count: Option<i64>,
    group_by: String,
    cooldown_seconds: Option<i64>,
    targets: Vec<NotificationTargetInputData>,
    message_template_mode: Option<String>,
    message_template: Option<NotificationTemplateData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationRuleUpdateBodyData {
    enabled: Option<bool>,
    event_type: Option<String>,
    event_level_filter: Option<Vec<String>>,
    event_source_filter: Option<Vec<String>>,
    window_seconds: Option<i64>,
    threshold_count: Option<i64>,
    group_by: Option<String>,
    cooldown_seconds: Option<i64>,
    targets: Option<Vec<NotificationTargetInputData>>,
    message_template_mode: Option<String>,
    message_template: Option<NotificationTemplateData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationRuleListData {
    rules: Vec<NotificationRuleData>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationMessageFactData {
    label: String,
    value: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationMessageActionData {
    label: String,
    url: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationMessageData {
    title: String,
    summary: String,
    body_text: String,
    body_markdown: Option<String>,
    severity: String,
    facts: Vec<NotificationMessageFactData>,
    actions: Vec<NotificationMessageActionData>,
    mentions: Vec<String>,
    dedupe_key: Option<String>,
    occurred_at: String,
    event_id: Option<String>,
    metadata: Option<Value>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationTriggerData {
    id: String,
    rule_id: String,
    event_id: String,
    group_key: String,
    matched_count: i64,
    message_snapshot: NotificationMessageData,
    rule_snapshot: NotificationRuleData,
    status: String,
    created_at: String,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationTriggerListData {
    triggers: Vec<NotificationTriggerData>,
    total: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationDeliveryData {
    id: String,
    trigger_id: String,
    rule_id: String,
    target_id: String,
    provider_id: String,
    event_id: String,
    status: String,
    #[schema(required = true)]
    reason: Option<String>,
    provider_type: String,
    message_snapshot: NotificationMessageData,
    target_snapshot: NotificationTargetData,
    provider_snapshot: NotificationProviderSnapshotData,
    #[schema(required = true)]
    request_summary: Option<Value>,
    #[schema(required = true)]
    response_summary: Option<Value>,
    attempt_count: i64,
    triggered_at: String,
    #[schema(required = true)]
    sent_at: Option<String>,
    #[schema(required = true)]
    next_retry_at: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationDeliveryListData {
    deliveries: Vec<NotificationDeliveryData>,
    total: i64,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationDeliveryClearBodyData {
    rule_id: Option<String>,
    provider_id: Option<String>,
    trigger_id: Option<String>,
    status: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub(super) struct NotificationDeliveryClearData {
    deleted_count: i64,
}
