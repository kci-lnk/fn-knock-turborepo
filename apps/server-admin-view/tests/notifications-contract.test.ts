import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  $ref?: string;
  const?: boolean | number | string;
  default?: number | string;
  enum?: string[];
  maximum?: number;
  minimum?: number;
  minItems?: number;
  oneOf?: Schema[];
  properties?: Record<string, Schema>;
  required?: string[];
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{ name?: string; schema?: Schema }>;
  requestBody?: {
    content?: Record<string, { schema?: Schema }>;
  };
  responses?: Record<string, { content?: Record<string, { schema?: Schema }> }>;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("notifications API contract", () => {
  it("keeps every notifications operation on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/notifications/providers/catalog"],
      ["get", "/api/admin/notifications/providers"],
      ["post", "/api/admin/notifications/providers"],
      ["post", "/api/admin/notifications/providers/test"],
      ["get", "/api/admin/notifications/providers/{id}"],
      ["patch", "/api/admin/notifications/providers/{id}"],
      ["delete", "/api/admin/notifications/providers/{id}"],
      ["post", "/api/admin/notifications/providers/{id}/test"],
      ["get", "/api/admin/notifications/rules"],
      ["post", "/api/admin/notifications/rules"],
      ["patch", "/api/admin/notifications/rules/{id}"],
      ["delete", "/api/admin/notifications/rules/{id}"],
      ["get", "/api/admin/notifications/triggers"],
      ["get", "/api/admin/notifications/deliveries"],
      ["delete", "/api/admin/notifications/deliveries"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("separates masked lists, authenticated detail, and secret-bearing writes", () => {
    const create =
      contract.components.schemas.NotificationProviderCreateBodyData;
    const update =
      contract.components.schemas.NotificationProviderUpdateBodyData;
    const detail = contract.components.schemas.NotificationProviderDetailData;
    assert.equal(create.properties?.connection_config?.writeOnly, true);
    assert.equal(update.properties?.connection_config?.writeOnly, true);
    assert.equal(detail.properties?.connection_config?.writeOnly, undefined);
    assert.equal(
      contract.components.schemas.NotificationProviderData.properties
        ?.connection_config,
      undefined,
    );
    assert.ok(
      contract.components.schemas.NotificationProviderData.required?.includes(
        "connection_config_masked",
      ),
    );
  });

  it("uses a direct provider-test response whose success may be false", () => {
    for (const path of [
      "/api/admin/notifications/providers/test",
      "/api/admin/notifications/providers/{id}/test",
    ]) {
      assert.equal(
        contract.paths[path].post.responses?.["200"]?.content?.[
          "application/json"
        ]?.schema?.$ref,
        "#/components/schemas/NotificationProviderTestResponseData",
      );
    }
    assert.equal(
      contract.components.schemas.NotificationProviderTestResponseData
        .properties?.success?.const,
      undefined,
    );
  });

  it("preserves rule and retry-policy bounds", () => {
    const rule = contract.components.schemas.NotificationRuleCreateBodyData;
    assert.equal(rule.properties?.targets?.minItems, 1);
    assert.equal(rule.properties?.window_seconds?.minimum, 1);
    assert.equal(rule.properties?.window_seconds?.maximum, 86_400);
    assert.equal(rule.properties?.threshold_count?.maximum, 9_999);
    assert.equal(rule.properties?.cooldown_seconds?.minimum, 0);

    const policy = contract.components.schemas.NotificationDeliveryPolicyData;
    assert.equal(policy.properties?.timeout_seconds?.maximum, 30);
    assert.equal(policy.properties?.max_attempts?.maximum, 10);
    assert.equal(policy.properties?.backoff_seconds?.minimum, 5);
    assert.equal(policy.properties?.backoff_seconds?.maximum, 3_600);
  });

  it("derives frontend provider, rule, history, and clear request types", () => {
    const api = readSource("../src/lib/api/events.ts");
    for (const schema of [
      "NotificationProviderCreateBodyData",
      "NotificationProviderUpdateBodyData",
      "NotificationProviderTestBodyData",
      "NotificationRuleCreateBodyData",
      "NotificationRuleUpdateBodyData",
      "NotificationDeliveryClearBodyData",
    ]) {
      assert.match(api, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_notifications_triggers/u);
    assert.match(api, /get_api_admin_notifications_deliveries/u);
    assert.match(api, /satisfies NotificationTriggerQuery/u);
    assert.match(api, /satisfies NotificationDeliveryQuery/u);
    assert.match(api, /satisfies NotificationDeliveryClearBody/u);
  });
});
