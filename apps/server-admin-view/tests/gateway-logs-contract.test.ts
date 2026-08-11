import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(
  readFileSync(
    new URL("../../../packages/api-contract/openapi.json", import.meta.url),
    "utf8",
  ),
) as {
  paths: Record<string, Record<string, {
    "x-fn-knock-contract-source"?: string;
    parameters?: Array<{ name?: string; schema?: { format?: string; enum?: string[]; pattern?: string } }>;
    requestBody?: { content?: Record<string, { schema?: { $ref?: string } }> };
  }>>;
};

describe("gateway logs API contract", () => {
  it("binds every gateway-log operation to an actual typed route", () => {
    for (const [method, path] of [
      ["get", "/api/admin/gateway-logs/config"],
      ["post", "/api/admin/gateway-logs/config"],
      ["get", "/api/admin/gateway-logs/directory"],
      ["get", "/api/admin/gateway-logs/dates"],
      ["get", "/api/admin/gateway-logs/entries"],
      ["delete", "/api/admin/gateway-logs/entries"],
      ["get", "/api/admin/gateway-logs/analytics"],
      ["post", "/api/admin/gateway-logs/analytics"],
    ] as const) {
      assert.equal(contract.paths[path]?.[method]?.["x-fn-knock-contract-source"], "utoipa");
    }
  });

  it("retains filtering and deletion compatibility", () => {
    const entries = contract.paths["/api/admin/gateway-logs/entries"].get;
    assert.equal(entries.parameters?.find((parameter) => parameter.name === "date")?.schema?.format, "date");
    assert.deepEqual(entries.parameters?.find((parameter) => parameter.name === "waf_status")?.schema?.enum, ["has_waf", "none"]);
    assert.equal(entries.parameters?.find((parameter) => parameter.name === "limit")?.schema?.pattern, "^[1-9][0-9]*$");
    assert.equal(
      contract.paths["/api/admin/gateway-logs/entries"].delete.requestBody?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/GatewayLogDeleteBodyData",
    );
  });
});
