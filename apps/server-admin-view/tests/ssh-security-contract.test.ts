import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: {
    schemas: Record<
      string,
      {
        properties?: Record<string, { enum?: string[] }>;
      }
    >;
  };
  paths: Record<
    string,
    Record<
      string,
      {
        "x-fn-knock-contract-source"?: string;
        parameters?: Array<{
          name?: string;
          schema?: { enum?: string[] };
        }>;
        requestBody?: {
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
      }
    >
  >;
};

describe("SSH security API contract", () => {
  it("keeps all SSH security operations on typed runtime routes", () => {
    for (const [method, path] of [
      ["get", "/api/admin/ssh-security/config"],
      ["post", "/api/admin/ssh-security/config"],
      ["post", "/api/admin/ssh-security/firewall/sync"],
      ["post", "/api/admin/ssh-security/firewall/clear"],
      ["get", "/api/admin/ssh-security/login-logs"],
      ["get", "/api/admin/ssh-security/blocks"],
      ["delete", "/api/admin/ssh-security/blocks"],
      ["get", "/api/admin/ssh-security/blocks/{ip}"],
      ["delete", "/api/admin/ssh-security/blocks/{ip}"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves runtime summary fields and write-only request boundaries", () => {
    const config = contract.components.schemas.SshSecurityConfigData;
    const summary = contract.components.schemas.SshSecuritySummaryData;
    assert.ok(summary.properties?.allowed_range_count);

    const update = contract.components.schemas.SshSecurityConfigUpdateData;
    const durationUnits = ["minute", "hour", "day", "month"];
    assert.deepEqual(
      config.properties?.block_duration_unit?.enum,
      durationUnits,
    );
    assert.deepEqual(
      update.properties?.block_duration_unit?.enum,
      durationUnits,
    );
    assert.equal(update.properties?.configured_at, undefined);
    assert.equal(update.properties?.updated_at, undefined);

    assert.equal(
      contract.paths["/api/admin/ssh-security/blocks"]?.delete?.requestBody
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/SshBlocksDeleteBodyData",
    );

    const outcome = contract.paths[
      "/api/admin/ssh-security/login-logs"
    ]?.get?.parameters?.find((parameter) => parameter.name === "outcome");
    assert.deepEqual(outcome?.schema?.enum, ["success", "failure"]);
  });

  it("derives frontend models, requests, and queries from the contract", () => {
    const types = readSource("../src/types/gateway.ts");
    const api = readSource("../src/lib/api/security.ts");
    const form = readSource(
      "../src/views/ssh-security/SSHSecurityFormFields.vue",
    );

    assert.match(types, /SshSecurityConfigData"\]/u);
    assert.match(types, /SshSecuritySummaryData"\]/u);
    assert.doesNotMatch(types, /export type SSHSecurityConfig = \{/u);
    assert.match(api, /operations as ApiContractOperations/u);
    assert.match(api, /SshSecurityConfigUpdateData/u);
    assert.match(api, /satisfies SshBlocksDeleteBody/u);
    assert.doesNotMatch(api, /Partial<Omit<SSHSecurityConfig/u);
    assert.match(form, /<SelectItem value="month">/u);
    assert.match(form, /admin\.sshSecurity\.month/u);
  });
});
