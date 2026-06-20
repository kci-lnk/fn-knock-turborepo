import type {
  NotificationDeliveryPolicy,
  NotificationGroupBy,
  NotificationProviderDefinition,
  NotificationTemplate,
  NotificationTemplateOverrideMode,
  SystemEventType,
} from "../../../types";
import { DEFAULT_GROUP_BY_BY_EVENT_TYPE } from "../constants";
import { buildSchemaPayload } from "./form-utils";

export type EditableRuleTarget = {
  id?: string;
  provider_id: string;
  target_config: Record<string, unknown>;
  delivery_policy: {
    timeout_seconds: string;
    max_attempts: string;
    backoff_seconds: string;
  };
  template_override_mode: NotificationTemplateOverrideMode;
  template_override: NotificationTemplate | null;
};

export type EditableRuleForm = {
  event_types: SystemEventType[];
  window_seconds: string;
  threshold_count: string;
  group_by: NotificationGroupBy | "auto";
  cooldown_seconds: string;
  targets: EditableRuleTarget[];
};

export const DEFAULT_RULE_WINDOW_SECONDS = "60";
export const DEFAULT_RULE_COOLDOWN_SECONDS = "60";

export const createEmptyDeliveryPolicy = () => ({
  timeout_seconds: "",
  max_attempts: "",
  backoff_seconds: "",
});

export const createEmptyRuleForm = (
  eventTypes: SystemEventType[],
): EditableRuleForm => ({
  event_types: [...eventTypes],
  window_seconds: DEFAULT_RULE_WINDOW_SECONDS,
  threshold_count: "1",
  group_by: "auto",
  cooldown_seconds: DEFAULT_RULE_COOLDOWN_SECONDS,
  targets: [],
});

export const parseOptionalPolicyNumber = (value: string) => {
  const parsed = Number.parseInt(String(value || "").trim(), 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
};

export const buildDeliveryPolicyPayload = (
  policy: EditableRuleTarget["delivery_policy"],
): NotificationDeliveryPolicy | undefined => {
  const payload: NotificationDeliveryPolicy = {
    timeout_seconds: parseOptionalPolicyNumber(policy.timeout_seconds),
    max_attempts: parseOptionalPolicyNumber(policy.max_attempts),
    backoff_seconds: parseOptionalPolicyNumber(policy.backoff_seconds),
  };
  if (
    payload.timeout_seconds === undefined &&
    payload.max_attempts === undefined &&
    payload.backoff_seconds === undefined
  ) {
    return undefined;
  }
  return payload;
};

export const resolveGroupByForEventType = ({
  eventType,
  form,
}: {
  eventType: SystemEventType;
  form: EditableRuleForm;
}): NotificationGroupBy =>
  form.group_by === "auto"
    ? DEFAULT_GROUP_BY_BY_EVENT_TYPE[eventType]
    : form.group_by;

export const buildRulePayload = ({
  eventType,
  form,
  groupBy,
  resolveProviderDefinitionById,
}: {
  eventType: SystemEventType;
  form: EditableRuleForm;
  groupBy: NotificationGroupBy;
  resolveProviderDefinitionById: (
    providerId: string,
  ) => NotificationProviderDefinition | null;
}) => ({
  enabled: true,
  event_type: eventType,
  event_level_filter: [],
  event_source_filter: [],
  window_seconds: Number(form.window_seconds || 0),
  threshold_count: Number(form.threshold_count || 0),
  group_by: groupBy,
  cooldown_seconds: Number(form.cooldown_seconds || 0),
  message_template_mode: "default",
  targets: form.targets.map((target) => {
    const definition = resolveProviderDefinitionById(target.provider_id);
    return {
      ...(target.id ? { id: target.id } : {}),
      provider_id: target.provider_id,
      enabled: true,
      target_config: buildSchemaPayload({
        fields: definition?.target_schema || [],
        value: target.target_config,
      }),
      delivery_policy: buildDeliveryPolicyPayload(target.delivery_policy),
      template_override_mode: target.template_override_mode,
      template_override: target.template_override,
    };
  }),
});
