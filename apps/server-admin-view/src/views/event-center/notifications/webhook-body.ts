import type {
  NotificationWebhookBodyConstraints,
  NotificationWebhookBodyPreview,
} from "../../../types";

export type WebhookBodyScope = "provider" | "target";
export type WebhookBodyFormat = "json" | "text";
export type WebhookBodyMode = "standard" | "inherit" | "custom";

export interface WebhookBodyConfig {
  mode: WebhookBodyMode;
  format?: WebhookBodyFormat;
  content_type?: string;
  template?: string;
}

export type WebhookBodyConstraints = NotificationWebhookBodyConstraints;
export type WebhookBodyPreview = NotificationWebhookBodyPreview;

export type WebhookBodyIssueCode =
  | "invalidMode"
  | "invalidFormat"
  | "templateRequired"
  | "templateTooLarge"
  | "invalidJson"
  | "unclosedVariable"
  | "invalidVariable"
  | "tooManyVariables"
  | "invalidContentType"
  | "contentTypeTooLong"
  | "sampleTooLarge"
  | "invalidSample";

export interface WebhookBodyIssue {
  code: WebhookBodyIssueCode;
  detail?: string;
}

export const DEFAULT_WEBHOOK_BODY_CONSTRAINTS = {
  kind: "webhook_body",
  scope: "provider",
  formats: ["json", "text"],
  variable_roots: [
    "message",
    "event",
    "context",
    "rule",
    "target",
    "provider",
    "legacy",
  ],
  max_template_bytes: 64 * 1024,
  max_sample_bytes: 64 * 1024,
  max_placeholders: 256,
  max_rendered_bytes: 256 * 1024,
  max_content_type_bytes: 256,
} satisfies WebhookBodyConstraints;

export const WEBHOOK_BODY_VARIABLES = [
  "message",
  "message.title",
  "message.summary",
  "message.body_text",
  "message.body_markdown",
  "message.severity",
  "message.facts",
  "message.actions",
  "message.mentions",
  "message.dedupe_key",
  "message.occurred_at",
  "message.event_id",
  "message.metadata",
  "event",
  "event.id",
  "event.type",
  "event.source",
  "event.level",
  "event.happened_at",
  "event.dedupe_key",
  "event.subject",
  "event.tags",
  "event.payload",
  "context",
  "context.mode",
  "context.trigger_id",
  "context.delivery_id",
  "context.event_id",
  "context.rule_id",
  "context.target_id",
  "context.provider_id",
  "rule",
  "rule.id",
  "rule.name",
  "rule.event_type",
  "rule.group_by",
  "rule.window_seconds",
  "rule.threshold_count",
  "rule.cooldown_seconds",
  "target",
  "target.id",
  "target.provider_id",
  "provider",
  "provider.id",
  "provider.name",
  "provider.type",
  "legacy.extra_body",
] as const;

const encoder = new TextEncoder();
const CONTENT_TYPE_PATTERN =
  /^[!#$%&'*+.^_`|~0-9A-Za-z-]+\/[!#$%&'*+.^_`|~0-9A-Za-z-]+(?:\s*;\s*[!#$%&'*+.^_`|~0-9A-Za-z-]+=(?:[!#$%&'*+.^_`|~0-9A-Za-z-]+|"[^"\r\n]*"))*$/u;
const VARIABLE_PATH_PATTERN = /^[A-Za-z0-9_-]+(?:\.[A-Za-z0-9_-]+)*$/u;

const scopeDefaultMode = (scope: WebhookBodyScope): WebhookBodyMode =>
  scope === "provider" ? "standard" : "inherit";

const defaultContentType = (format: WebhookBodyFormat) =>
  format === "json" ? "application/json" : "text/plain; charset=utf-8";

export const coerceWebhookBodyConfig = (
  value: unknown,
  scope: WebhookBodyScope,
): WebhookBodyConfig => {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return { mode: scopeDefaultMode(scope) };
  }
  const input = value as Record<string, unknown>;
  const mode = String(input.mode || scopeDefaultMode(scope)).toLowerCase();
  const validModes =
    scope === "provider" ? ["standard", "custom"] : ["inherit", "custom"];
  return {
    mode: validModes.includes(mode)
      ? (mode as WebhookBodyMode)
      : scopeDefaultMode(scope),
    format:
      String(input.format || "json").toLowerCase() === "text" ? "text" : "json",
    content_type: String(input.content_type || ""),
    template: typeof input.template === "string" ? input.template : "",
  };
};

export const normalizeWebhookBodyConfig = (
  value: unknown,
  scope: WebhookBodyScope,
): WebhookBodyConfig => {
  const config = coerceWebhookBodyConfig(value, scope);
  if (config.mode !== "custom") return { mode: config.mode };
  const format = config.format || "json";
  return {
    mode: "custom",
    format,
    content_type: config.content_type?.trim() || defaultContentType(format),
    template: config.template || "",
  };
};

type ScanResult = { count: number; issue?: WebhookBodyIssue };

const scanTemplateText = (
  input: string,
  variableRoots: readonly string[],
): ScanResult => {
  let cursor = 0;
  let count = 0;
  while (true) {
    const offset = input.indexOf("{{", cursor);
    if (offset < 0) return { count };
    let slashes = 0;
    for (
      let index = offset - 1;
      index >= 0 && input[index] === "\\";
      index -= 1
    ) {
      slashes += 1;
    }
    if (slashes % 2 === 1) {
      cursor = offset + 2;
      continue;
    }
    const end = input.indexOf("}}", offset + 2);
    if (end < 0) return { count, issue: { code: "unclosedVariable" } };
    const path = input.slice(offset + 2, end).trim();
    if (
      encoder.encode(path).length > 256 ||
      !VARIABLE_PATH_PATTERN.test(path) ||
      !variableRoots.includes(path.split(".")[0] || "")
    ) {
      return { count, issue: { code: "invalidVariable", detail: path } };
    }
    count += 1;
    cursor = end + 2;
  }
};

const scanJsonTemplate = (
  value: unknown,
  variableRoots: readonly string[],
): ScanResult => {
  let count = 0;
  let issue: WebhookBodyIssue | undefined;
  const visit = (current: unknown) => {
    if (issue) return;
    if (typeof current === "string") {
      const result = scanTemplateText(current, variableRoots);
      count += result.count;
      issue = result.issue;
      return;
    }
    if (Array.isArray(current)) {
      current.forEach(visit);
      return;
    }
    if (current && typeof current === "object") {
      for (const [key, child] of Object.entries(current)) {
        const keyResult = scanTemplateText(key, variableRoots);
        count += keyResult.count;
        issue = keyResult.issue;
        if (issue) return;
        visit(child);
      }
    }
  };
  visit(value);
  return { count, issue };
};

export const validateWebhookBodyConfig = (
  value: unknown,
  constraints: WebhookBodyConstraints | undefined,
  scope: WebhookBodyScope,
): WebhookBodyIssue[] => {
  if (
    value !== undefined &&
    value !== null &&
    (typeof value !== "object" || Array.isArray(value))
  ) {
    return [{ code: "invalidMode" }];
  }
  const input = (value || {}) as Record<string, unknown>;
  if (input.mode !== undefined && typeof input.mode !== "string") {
    return [{ code: "invalidMode" }];
  }
  const rawMode = String(input.mode || scopeDefaultMode(scope)).toLowerCase();
  const modes =
    scope === "provider" ? ["standard", "custom"] : ["inherit", "custom"];
  if (!modes.includes(rawMode)) return [{ code: "invalidMode" }];
  if (rawMode !== "custom") return [];

  const merged = { ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS, ...constraints };
  const variableRoots =
    merged.variable_roots || DEFAULT_WEBHOOK_BODY_CONSTRAINTS.variable_roots;
  const format = String(input.format || "json").toLowerCase();
  const issues: WebhookBodyIssue[] = [];
  if (
    (input.format !== undefined && typeof input.format !== "string") ||
    !["json", "text"].includes(format)
  )
    issues.push({ code: "invalidFormat" });
  if (input.template !== undefined && typeof input.template !== "string") {
    issues.push({ code: "templateRequired" });
  }
  const template = typeof input.template === "string" ? input.template : "";
  if (format === "json" && !template.trim()) {
    issues.push({ code: "templateRequired" });
  }
  if (encoder.encode(template).length > (merged.max_template_bytes || 0)) {
    issues.push({ code: "templateTooLarge" });
  }

  let scan: ScanResult = { count: 0 };
  if (format === "json" && template.trim()) {
    try {
      scan = scanJsonTemplate(JSON.parse(template), variableRoots);
    } catch (error) {
      issues.push({
        code: "invalidJson",
        detail: error instanceof Error ? error.message : "",
      });
    }
  } else if (format === "text") {
    scan = scanTemplateText(template, variableRoots);
  }
  if (scan.issue) issues.push(scan.issue);
  if (scan.count > (merged.max_placeholders || 0)) {
    issues.push({ code: "tooManyVariables" });
  }

  const contentType = String(
    input.content_type || defaultContentType(format as WebhookBodyFormat),
  );
  if (
    input.content_type !== undefined &&
    typeof input.content_type !== "string"
  ) {
    issues.push({ code: "invalidContentType" });
  } else if (
    encoder.encode(contentType).length > (merged.max_content_type_bytes || 0)
  ) {
    issues.push({ code: "contentTypeTooLong" });
  } else if (
    Array.from(contentType).some((character) => {
      const code = character.codePointAt(0) || 0;
      return code <= 31 || (code >= 127 && code <= 159);
    }) ||
    !CONTENT_TYPE_PATTERN.test(contentType.trim())
  ) {
    issues.push({ code: "invalidContentType" });
  }
  return issues;
};

export const createWebhookSampleContext = () => {
  const occurredAt = new Date().toISOString();
  return JSON.stringify(
    {
      message: {
        title: "fn-knock test notification",
        summary: "Webhook body template preview",
        body_text: "This is editable sample data.",
        body_markdown: "**Webhook body template preview**",
        severity: "info",
        facts: [{ label: "Mode", value: "test" }],
        actions: [],
        mentions: [],
        dedupe_key: null,
        occurred_at: occurredAt,
        event_id: "evt_webhook_test",
        metadata: { test: true },
      },
      event: {
        id: "evt_webhook_test",
        type: "FN_EVENT_AUTH_LOGIN_SUCCESS",
        source: "SERVER_ADMIN",
        level: "INFO",
        happened_at: occurredAt,
        dedupe_key: null,
        subject: { kind: "APPLICATION", id: "fn-knock" },
        tags: ["test"],
        payload: { test: true, ip: "192.0.2.10" },
      },
      context: {
        mode: "provider_test",
        trigger_id: null,
        delivery_id: null,
        event_id: "evt_webhook_test",
        rule_id: "ntfrule_test",
        target_id: "ntftarget_test",
        provider_id: "ntfprov_test",
      },
      rule: {
        id: "ntfrule_test",
        name: "Webhook test",
        event_type: "FN_EVENT_AUTH_LOGIN_SUCCESS",
        group_by: "GLOBAL",
        window_seconds: 60,
        threshold_count: 1,
        cooldown_seconds: 60,
      },
      target: { id: "ntftarget_test", provider_id: "ntfprov_test" },
      legacy: { extra_body: {} },
    },
    null,
    2,
  );
};

export const validateWebhookSampleContext = (
  value: unknown,
  constraints?: WebhookBodyConstraints,
): WebhookBodyIssue[] => {
  const text = String(value || "").trim();
  if (!text) return [];
  const maxSampleBytes =
    constraints?.max_sample_bytes ||
    DEFAULT_WEBHOOK_BODY_CONSTRAINTS.max_sample_bytes;
  if (encoder.encode(text).length > maxSampleBytes) {
    return [{ code: "sampleTooLarge" }];
  }
  try {
    const parsed = JSON.parse(text) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      return [{ code: "invalidSample" }];
    }
  } catch {
    return [{ code: "invalidSample" }];
  }
  return [];
};

export const parseWebhookSampleContext = (
  value: unknown,
  constraints?: WebhookBodyConstraints,
) => {
  const text = String(value || "").trim();
  if (!text) return undefined;
  const issue = validateWebhookSampleContext(text, constraints)[0];
  if (issue) throw new Error(issue.code);
  const parsed = JSON.parse(text) as Record<string, unknown>;
  return parsed;
};
