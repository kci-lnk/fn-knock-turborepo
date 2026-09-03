import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  buildSchemaPayload,
  createEditableSchemaRecord,
} from "../src/views/event-center/notifications/form-utils";
import { buildRuleTargetConfigPayload } from "../src/views/event-center/notifications/rule-form";
import {
  DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
  normalizeWebhookBodyConfig,
  parseWebhookSampleContext,
  validateWebhookBodyConfig,
  validateWebhookSampleContext,
} from "../src/views/event-center/notifications/webhook-body";
import type {
  NotificationProviderDefinition,
  NotificationSchemaField,
} from "../src/types";

const providerBodyField: NotificationSchemaField = {
  key: "body_config",
  label: "Request body",
  type: "webhook_body",
  sensitive: true,
  default_value: { mode: "standard" },
  constraints: DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
};

describe("webhook body form helpers", () => {
  it("round-trips a custom config and omits transient preview state", () => {
    const source = {
      body_config: {
        mode: "custom",
        format: "text",
        content_type: " text/plain; charset=utf-8 ",
        template: "{{message.title}}",
      },
    };
    const editable = createEditableSchemaRecord([providerBodyField], source);
    editable.__webhook_sample_context =
      '{"event":{"payload":{"ip":"192.0.2.1"}}}';
    editable.__webhook_body_preview = { body: "secret preview" };
    assert.deepEqual(
      buildSchemaPayload({ fields: [providerBodyField], value: editable }),
      {
        body_config: {
          mode: "custom",
          format: "text",
          content_type: "text/plain; charset=utf-8",
          template: "{{message.title}}",
        },
      },
    );
  });

  it("keeps hidden legacy webhook data in rule save and test payloads", () => {
    const targetBodyField: NotificationSchemaField = {
      ...providerBodyField,
      key: "body_override",
      sensitive: false,
      default_value: { mode: "inherit" },
      constraints: {
        ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        scope: "target",
      },
    };
    const definition = {
      type: "webhook",
      target_schema: [targetBodyField],
    } as NotificationProviderDefinition;
    const target = {
      provider_id: "ntfprov_webhook",
      target_config: {
        body_override: { mode: "inherit" },
        extra_headers_json: { Authorization: "Bearer legacy" },
        extra_body_json: { legacy: true },
        __webhook_sample_context: "{}",
      },
      delivery_policy: {
        timeout_seconds: "",
        max_attempts: "",
        backoff_seconds: "",
      },
      template_override_mode: "inherit" as const,
      template_override: null,
    };
    assert.deepEqual(buildRuleTargetConfigPayload({ target, definition }), {
      body_override: { mode: "inherit" },
      extra_headers_json: { Authorization: "Bearer legacy" },
      extra_body_json: { legacy: true },
    });
  });

  it("normalizes provider and target defaults without inventing templates", () => {
    assert.deepEqual(normalizeWebhookBodyConfig(undefined, "provider"), {
      mode: "standard",
    });
    assert.deepEqual(normalizeWebhookBodyConfig(undefined, "target"), {
      mode: "inherit",
    });
    assert.deepEqual(
      normalizeWebhookBodyConfig(
        { mode: "custom", format: "json", template: "{}" },
        "provider",
      ),
      {
        mode: "custom",
        format: "json",
        content_type: "application/json",
        template: "{}",
      },
    );
  });

  it("rejects invalid JSON, unsafe roots, MIME values, and configured limits", () => {
    const validate = (value: unknown) =>
      validateWebhookBodyConfig(
        value,
        DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        "provider",
      ).map((issue) => issue.code);
    assert.ok(
      validate({ mode: "custom", format: "json", template: "{" }).includes(
        "invalidJson",
      ),
    );
    assert.ok(validate(0).includes("invalidMode"));
    assert.ok(validate({ mode: true }).includes("invalidMode"));
    assert.ok(
      validate({ mode: "custom", format: false, template: "{}" }).includes(
        "invalidFormat",
      ),
    );
    assert.ok(
      validate({ mode: "custom", format: "text", template: 42 }).includes(
        "templateRequired",
      ),
    );
    assert.ok(
      validate({
        mode: "custom",
        format: "text",
        template: "",
        content_type: 42,
      }).includes("invalidContentType"),
    );
    assert.ok(
      validate({
        mode: "custom",
        format: "text",
        template: "{{shared_secret}}",
      }).includes("invalidVariable"),
    );
    assert.ok(
      validate({
        mode: "custom",
        format: "text",
        template: "ok",
        content_type: "text/plain\r\nX-Evil: yes",
      }).includes("invalidContentType"),
    );
    assert.ok(
      validateWebhookBodyConfig(
        { mode: "custom", format: "text", template: "12345" },
        { ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS, max_template_bytes: 4 },
        "provider",
      ).some((issue) => issue.code === "templateTooLarge"),
    );
    assert.ok(
      validateWebhookBodyConfig(
        {
          mode: "custom",
          format: "text",
          template: "{{message.title}}{{event.id}}",
        },
        { ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS, max_placeholders: 1 },
        "provider",
      ).some((issue) => issue.code === "tooManyVariables"),
    );
  });

  it("accepts only JSON objects as temporary sample context", () => {
    assert.deepEqual(parseWebhookSampleContext('{"event":{"id":"evt_1"}}'), {
      event: { id: "evt_1" },
    });
    assert.equal(parseWebhookSampleContext(""), undefined);
    assert.throws(() => parseWebhookSampleContext("[]"));
    assert.throws(() => parseWebhookSampleContext("not-json"));
    assert.deepEqual(
      validateWebhookSampleContext("[]", DEFAULT_WEBHOOK_BODY_CONSTRAINTS).map(
        (issue) => issue.code,
      ),
      ["invalidSample"],
    );
    assert.deepEqual(
      validateWebhookSampleContext(JSON.stringify({ value: "12345" }), {
        ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        max_sample_bytes: 4,
      }).map((issue) => issue.code),
      ["sampleTooLarge"],
    );
    assert.throws(() =>
      parseWebhookSampleContext(JSON.stringify({ value: "12345" }), {
        ...DEFAULT_WEBHOOK_BODY_CONSTRAINTS,
        max_sample_bytes: 4,
      }),
    );
  });
});
