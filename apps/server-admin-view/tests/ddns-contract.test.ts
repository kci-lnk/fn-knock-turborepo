import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  $ref?: string;
  const?: string | number | boolean;
  enum?: string[];
  maximum?: number;
  minimum?: number;
  oneOf?: Schema[];
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{
    in?: string;
    name?: string;
    schema?: Schema;
  }>;
  responses?: Record<
    string,
    { content?: Record<string, { schema?: Schema }> }
  >;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("DDNS API contract", () => {
  it("keeps all settings, target, interface, test, and log operations on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/ddns/status"],
      ["post", "/api/admin/ddns/toggle"],
      ["get", "/api/admin/ddns/providers"],
      ["get", "/api/admin/ddns/settings"],
      ["post", "/api/admin/ddns/settings"],
      ["post", "/api/admin/ddns/public-check/test"],
      ["get", "/api/admin/ddns/interfaces"],
      ["post", "/api/admin/ddns/interfaces/resolve"],
      ["post", "/api/admin/ddns/provider"],
      ["get", "/api/admin/ddns/config/{provider}"],
      ["post", "/api/admin/ddns/config/{provider}"],
      ["get", "/api/admin/ddns/targets"],
      ["post", "/api/admin/ddns/targets"],
      ["get", "/api/admin/ddns/targets/{id}"],
      ["put", "/api/admin/ddns/targets/{id}"],
      ["delete", "/api/admin/ddns/targets/{id}"],
      ["post", "/api/admin/ddns/targets/{id}/enabled"],
      ["post", "/api/admin/ddns/test"],
      ["post", "/api/admin/ddns/targets/{id}/test"],
      ["get", "/api/admin/ddns/logs"],
      ["delete", "/api/admin/ddns/logs"],
      ["get", "/api/admin/ddns/poll"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves settings bounds, enums, and nullable status fields", () => {
    const settings = contract.components.schemas.DdnsSettingsData;
    const status = contract.components.schemas.DdnsStatusData;
    assert.equal(settings.properties?.updateIntervalMinutes?.minimum, 5);
    assert.equal(settings.properties?.updateIntervalMinutes?.maximum, 1440);
    assert.deepEqual(settings.properties?.httpTransport?.enum, [
      "curl",
      "node",
    ]);
    assert.deepEqual(settings.properties?.publicDnsProvider?.enum, [
      "none",
      "alidns",
      "tencent",
      "cloudflare",
      "google",
    ]);
    for (const field of ["provider", "primaryTargetId"]) {
      assert.ok(status.required?.includes(field), field);
    }
  });

  it("marks credential-bearing writes without hiding authenticated reads", () => {
    const config = contract.components.schemas.DdnsConfigData;
    const configBody = contract.components.schemas.DdnsConfigBodyData;
    const targetBody = contract.components.schemas.DdnsTargetBodyData;
    const target = contract.components.schemas.DdnsTargetDetailData;
    assert.equal(config.writeOnly, undefined);
    assert.equal(configBody.properties?.config?.writeOnly, true);
    assert.equal(targetBody.properties?.config?.writeOnly, true);
    assert.equal(target.properties?.config?.writeOnly, undefined);
    assert.equal(targetBody.required?.includes("config") ?? false, false);
  });

  it("documents selector, legacy log parsing, safe paths, and direct tests", () => {
    const selector = contract.components.schemas.DdnsInterfaceSelectorData;
    assert.equal(selector.properties?.version?.const, 1);
    assert.deepEqual(selector.properties?.mode?.enum, ["auto", "rules"]);

    const logs = contract.paths["/api/admin/ddns/logs"].get.parameters ?? [];
    const poll = contract.paths["/api/admin/ddns/poll"].get.parameters ?? [];
    const target =
      contract.paths["/api/admin/ddns/targets/{id}/test"].post.parameters ?? [];
    assert.equal(
      logs.find((parameter) => parameter.name === "limit")?.schema?.oneOf?.[1]
        ?.pattern,
      "^\\s*[+-]?\\d+",
    );
    assert.equal(
      poll.find((parameter) => parameter.name === "cursor")?.schema?.oneOf?.[0]
        ?.minimum,
      0,
    );
    assert.equal(
      target.find((parameter) => parameter.name === "id")?.schema?.pattern,
      "^[A-Za-z0-9-]{1,80}$",
    );
    assert.equal(
      contract.paths["/api/admin/ddns/test"].post.responses?.["200"]
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/DdnsTestResponseData",
    );
  });

  it("derives frontend DDNS models, requests, and queries from OpenAPI", () => {
    const api = readSource("../src/lib/api/ddns.ts");
    for (const schema of [
      "DdnsStatusData",
      "DdnsSettingsUpdateData",
      "DdnsTargetDetailData",
      "DdnsNetworkInterfaceData",
      "DdnsInterfaceSelectorData",
      "DdnsPollData",
      "DdnsConfigBodyData",
      "DdnsTargetBodyData",
    ]) {
      assert.match(api, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_ddns_logs/u);
    assert.match(api, /get_api_admin_ddns_poll/u);
    assert.match(api, /satisfies DdnsPublicCheckTestBody/u);
    assert.match(api, /satisfies DdnsTargetBody/u);
    assert.match(api, /satisfies DdnsPollQuery/u);
  });
});
