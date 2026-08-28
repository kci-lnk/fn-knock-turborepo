import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  $ref?: string;
  const?: boolean | number | string;
  default?: number | string;
  description?: string;
  enum?: Array<null | number | string>;
  format?: string;
  maximum?: number;
  minimum?: number;
  minItems?: number;
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{ name?: string; schema?: Schema }>;
  requestBody?: {
    content?: Record<string, { schema?: Schema }>;
  };
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("WAF API contract", () => {
  it("keeps every WAF operation on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/waf/details"],
      ["get", "/api/admin/waf/status"],
      ["post", "/api/admin/waf/config"],
      ["post", "/api/admin/waf/manifest/refresh"],
      ["post", "/api/admin/waf/system/sync"],
      ["post", "/api/admin/waf/rules/recommended"],
      ["post", "/api/admin/waf/rules/enabled"],
      ["get", "/api/admin/waf/rules/{source}/{filename}"],
      ["post", "/api/admin/waf/custom/upload"],
      ["delete", "/api/admin/waf/custom/{filename}"],
      ["post", "/api/admin/waf/events/drain"],
      ["get", "/api/admin/waf/logs"],
      ["get", "/api/admin/waf/logs/{trace_id}"],
      ["delete", "/api/admin/waf/logs"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("separates mutable WAF settings from runtime-owned fields", () => {
    const config = contract.components.schemas.WafConfigData;
    assert.equal(config.properties?.mode?.const, "blocking");
    assert.equal(config.properties?.active_bundle_id?.const, "local");
    assert.equal(config.properties?.paranoia_level?.minimum, 1);
    assert.equal(config.properties?.paranoia_level?.maximum, 4);
    assert.deepEqual(config.properties?.block_behavior?.enum, [
      "error_page",
      "reset_connection",
    ]);

    const update = contract.components.schemas.WafConfigUpdateData;
    assert.deepEqual(update.required ?? [], []);
    assert.deepEqual(Object.keys(update.properties ?? {}).sort(), [
      "block_behavior",
      "common_location_exempt_enabled",
      "enabled",
      "executing_paranoia_level",
      "paranoia_level",
      "private_ip_exempt_enabled",
      "system_rules_auto_update_enabled",
    ]);
  });

  it("documents rule sources and upload safety boundaries", () => {
    const parameters =
      contract.paths["/api/admin/waf/rules/{source}/{filename}"].get
        .parameters ?? [];
    assert.deepEqual(
      parameters.find((parameter) => parameter.name === "source")?.schema?.enum,
      ["system", "custom"],
    );
    assert.equal(
      parameters.find((parameter) => parameter.name === "filename")?.schema
        ?.pattern,
      "(?i)\\.conf$",
    );
    assert.equal(
      contract.components.schemas.WafUploadBodyData.properties?.files?.minItems,
      1,
    );
    const file = contract.components.schemas.WafUploadFileData;
    assert.equal(file.properties?.content_base64?.format, "byte");
    assert.match(file.properties?.content_base64?.description ?? "", /1 MiB/u);
  });

  it("preserves log pagination compatibility and the real drain shape", () => {
    const parameters =
      contract.paths["/api/admin/waf/logs"].get.parameters ?? [];
    const cursor = parameters.find(
      (parameter) => parameter.name === "cursor",
    )?.schema;
    const limit = parameters.find(
      (parameter) => parameter.name === "limit",
    )?.schema;
    assert.equal(cursor?.default, "0");
    assert.equal(cursor?.pattern, "^\\s*[+-]?\\d+");
    assert.equal(limit?.default, "50");
    assert.equal(limit?.pattern, "^\\s*[+-]?\\d+");

    const drain = contract.components.schemas.WafDrainResultData;
    assert.ok(drain.required?.includes("drained"));
    assert.ok(drain.required?.includes("remaining"));
    assert.equal(drain.properties?.events, undefined);
  });

  it("derives frontend responses, request bodies, and log queries", () => {
    const types = readSource("../src/types/waf.ts");
    const api = readSource("../src/lib/api/gateway.ts");
    for (const schema of [
      "WafConfigData",
      "WafDetailsData",
      "WafRuleFileData",
      "WafEventData",
      "WafLogEntriesData",
      "WafDrainResultData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /\["WafConfigUpdateData"\]/u);
    assert.match(api, /\["WafRuleToggleBodyData"\]/u);
    assert.match(api, /\["WafUploadBodyData"\]/u);
    assert.match(api, /get_api_admin_waf_logs/u);
    assert.match(api, /satisfies WafLogDeleteBody/u);
    assert.doesNotMatch(types, /interface WAFConfig/u);
    assert.doesNotMatch(types, /interface WAFEvent/u);
  });

  it("wires the WAF block response selector to immediate save and rollback", () => {
    const view = readSource("../src/views/system-settings/WAFSettings.vue");
    const settings = readSource(
      "../src/views/system-settings/waf-settings/useWAFSettings.ts",
    );
    const row = readSource(
      "../src/views/system-settings/waf-settings/WAFBlockBehaviorSettingRow.vue",
    );

    assert.match(view, /<WAFBlockBehaviorSettingRow/u);
    assert.match(view, /v-if="form\.enabled"/u);
    assert.match(view, /@update:model-value="handleBlockBehaviorChange"/u);
    assert.match(settings, /block_behavior: "error_page"/u);
    assert.match(
      settings,
      /WAFAPI\.updateConfig\(\{ block_behavior: normalized \}\)/u,
    );
    assert.match(settings, /form\.block_behavior = previousBehavior/u);
    assert.match(row, /selectBehavior\('error_page'\)/u);
    assert.match(row, /selectBehavior\('reset_connection'\)/u);
    assert.match(row, /:aria-pressed=/u);
  });
});
