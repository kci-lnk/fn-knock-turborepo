export type SystemEventType =
  | "FN_EVENT_AUTH_LOGIN_SUCCESS"
  | "FN_EVENT_AUTH_LOGOUT"
  | "FN_EVENT_AUTH_LOGIN_FAILURE"
  | "FN_EVENT_AUTH_SESSION_IP_DRIFT"
  | "FN_EVENT_SECURITY_SCANNER_BLOCKED"
  | "FN_EVENT_DDNS_UPDATE_COMPLETED"
  | "FN_EVENT_GATEWAY_THROTTLE_BLOCKED"
  | "FN_EVENT_WAF_BLOCKED"
  | "FN_EVENT_SSH_LOGIN_SUCCESS"
  | "FN_EVENT_SSH_LOGIN_FAILURE"
  | "FN_EVENT_SSH_IP_BLOCKED"
  | "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"
  | "FN_EVENT_SYSTEM_CPU_ALERT"
  | "FN_EVENT_SYSTEM_CPU_RECOVERED"
  | "FN_EVENT_SYSTEM_MEMORY_ALERT"
  | "FN_EVENT_SYSTEM_MEMORY_RECOVERED"
  | "FN_EVENT_TUNNEL_FRP_CONNECTED"
  | "FN_EVENT_TUNNEL_FRP_DISCONNECTED"
  | "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED"
  | "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED";

export type SystemEventLevel = "INFO" | "WARN" | "ERROR" | "CRITICAL";

export type SystemEventSource =
  | "SERVER_ADMIN"
  | "GO_REAUTH_PROXY"
  | "SYSTEM_MONITOR";

export type SystemEventSubjectKind =
  | "IP"
  | "SESSION"
  | "DDNS"
  | "RESOURCE"
  | "APPLICATION"
  | "TUNNEL";

export interface SystemEventSubject {
  kind: SystemEventSubjectKind;
  id: string;
}

export interface SystemEventRecord {
  id: string;
  type: SystemEventType;
  source: SystemEventSource;
  level: SystemEventLevel;
  happened_at: string;
  dedupe_key?: string;
  subject?: SystemEventSubject;
  tags?: string[];
  payload: Record<string, unknown>;
}

export interface SystemEventListPayload {
  events: SystemEventRecord[];
  total: number;
}

export type NotificationProviderType =
  | "wxpusher"
  | "serverchan"
  | "pushplus"
  | "wecom"
  | "dingtalk"
  | "feishu"
  | "email"
  | "webhook"
  | "pushdeer"
  | "magicpush"
  | "bark"
  | "telegram";

export type NotificationGroupBy =
  | "GLOBAL"
  | "IP"
  | "SESSION"
  | "SUBJECT"
  | "HOSTNAME"
  | "PROVIDER";

export type NotificationTriggerStatus =
  | "created"
  | "fanout_done"
  | "partially_failed"
  | "completed";

export type NotificationDeliveryStatus =
  | "queued"
  | "sending"
  | "success"
  | "failed"
  | "gave_up"
  | "skipped";

export type NotificationTestStatus = "idle" | "success" | "failed";

export type NotificationMessageTemplateMode = "default" | "custom";

export type NotificationTemplateOverrideMode = "inherit" | "custom";

export type NotificationSeverity = "info" | "warn" | "error" | "critical";

export type NotificationFieldType =
  | "string"
  | "number"
  | "boolean"
  | "select"
  | "json";

export interface NotificationFieldOption {
  label: string;
  value: string;
}

export interface NotificationSchemaField {
  key: string;
  label: string;
  description?: string;
  placeholder?: string;
  type: NotificationFieldType;
  required?: boolean;
  sensitive?: boolean;
  default_value?: string | number | boolean | null;
  options?: NotificationFieldOption[];
  min?: number;
  max?: number;
}

export interface NotificationProviderCapabilities {
  supports_text: boolean;
  supports_markdown: boolean;
  supports_rich_blocks: boolean;
  supports_actions: boolean;
  supports_mentions: boolean;
  supports_attachments: boolean;
  supports_provider_dedupe_key: boolean;
  max_body_length?: number | null;
}

export interface NotificationProviderDefinition {
  type: NotificationProviderType;
  label: string;
  description: string;
  connection_schema: NotificationSchemaField[];
  target_schema: NotificationSchemaField[];
  sensitive_fields: string[];
  capabilities: NotificationProviderCapabilities;
}

export interface NotificationMessageFact {
  label: string;
  value: string;
}

export interface NotificationMessageAction {
  label: string;
  url: string;
}

export interface NotificationMessage {
  title: string;
  summary: string;
  body_text: string;
  body_markdown?: string;
  severity: NotificationSeverity;
  facts: NotificationMessageFact[];
  actions: NotificationMessageAction[];
  mentions: string[];
  dedupe_key?: string;
  occurred_at: string;
  event_id?: string;
  metadata?: Record<string, unknown>;
}

export interface NotificationTemplate {
  title?: string;
  body_text?: string;
  body_markdown?: string;
}

export interface NotificationDeliveryPolicy {
  timeout_seconds?: number;
  max_attempts?: number;
  backoff_seconds?: number;
}

export interface NotificationProviderView {
  id: string;
  name: string;
  type: NotificationProviderType;
  enabled: boolean;
  connection_config_masked: Record<string, unknown>;
  created_at: string;
  updated_at: string;
  last_test_at?: string;
  last_test_status?: NotificationTestStatus;
  last_error?: string | null;
}

export interface NotificationProviderDetailView
  extends NotificationProviderView {
  connection_config: Record<string, unknown>;
}

export interface NotificationTargetBinding {
  id: string;
  provider_id: string;
  enabled: boolean;
  target_config: Record<string, unknown>;
  template_override_mode: NotificationTemplateOverrideMode;
  template_override?: NotificationTemplate | null;
  delivery_policy?: NotificationDeliveryPolicy | null;
  created_at: string;
  updated_at: string;
}

export interface NotificationRule {
  id: string;
  name: string;
  enabled: boolean;
  event_type: SystemEventType;
  event_level_filter?: SystemEventLevel[];
  event_source_filter?: SystemEventSource[];
  window_seconds: number;
  threshold_count: number;
  group_by: NotificationGroupBy;
  cooldown_seconds: number;
  targets: NotificationTargetBinding[];
  message_template_mode: NotificationMessageTemplateMode;
  message_template?: NotificationTemplate | null;
  created_at: string;
  updated_at: string;
  last_triggered_at?: string | null;
}

export interface NotificationTrigger {
  id: string;
  rule_id: string;
  event_id: string;
  group_key: string;
  matched_count: number;
  message_snapshot: NotificationMessage;
  rule_snapshot: NotificationRule;
  status: NotificationTriggerStatus;
  created_at: string;
}

export interface NotificationDelivery {
  id: string;
  trigger_id: string;
  rule_id: string;
  target_id: string;
  provider_id: string;
  event_id: string;
  status: NotificationDeliveryStatus;
  reason?: string | null;
  provider_type: NotificationProviderType;
  message_snapshot: NotificationMessage;
  target_snapshot: NotificationTargetBinding;
  provider_snapshot: NotificationProviderView;
  request_summary?: Record<string, unknown> | null;
  response_summary?: Record<string, unknown> | null;
  attempt_count: number;
  triggered_at: string;
  sent_at?: string | null;
  next_retry_at?: string | null;
}

export interface NotificationProviderCatalogPayload {
  providers: NotificationProviderDefinition[];
}

export interface NotificationProviderListPayload {
  providers: NotificationProviderView[];
}

export interface NotificationRuleListPayload {
  rules: NotificationRule[];
}

export interface NotificationTriggerListPayload {
  triggers: NotificationTrigger[];
  total: number;
}

export interface NotificationDeliveryListPayload {
  deliveries: NotificationDelivery[];
  total: number;
}
