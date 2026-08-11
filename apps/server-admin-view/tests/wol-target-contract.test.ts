import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(readFileSync(new URL("../../../packages/api-contract/openapi.json", import.meta.url), "utf8")) as { paths: Record<string, Record<string, { "x-fn-knock-contract-source"?: string; requestBody?: { content?: Record<string, { schema?: { $ref?: string } }> } }>> };

describe("Wake-on-LAN target API contract", () => {
  it("uses typed routes and preserves target writes", () => {
    for (const [method, path] of [["get", "/api/admin/wol/targets"], ["post", "/api/admin/wol/targets"], ["get", "/api/admin/wol/targets/{id}"], ["put", "/api/admin/wol/targets/{id}"], ["delete", "/api/admin/wol/targets/{id}"], ["post", "/api/admin/wol/targets/{id}/wake"]] as const) assert.equal(contract.paths[path]?.[method]?.["x-fn-knock-contract-source"], "utoipa");
    assert.equal(contract.paths["/api/admin/wol/targets"]?.post?.requestBody?.content?.["application/json"]?.schema?.$ref, "#/components/schemas/WolTargetInputData");
  });
});
