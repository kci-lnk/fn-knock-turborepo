import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(readFileSync(new URL("../../../packages/api-contract/openapi.json", import.meta.url), "utf8")) as { paths: Record<string, Record<string, { "x-fn-knock-contract-source"?: string; requestBody?: { content?: Record<string, { schema?: { $ref?: string } }> } }>> };

describe("Wake-on-LAN Relay API contract", () => {
  it("uses typed routes and preserves relay input schema", () => {
    for (const [method, path] of [["get", "/api/admin/wol/relays"], ["post", "/api/admin/wol/relays"], ["get", "/api/admin/wol/relays/{id}"], ["put", "/api/admin/wol/relays/{id}"], ["delete", "/api/admin/wol/relays/{id}"], ["post", "/api/admin/wol/relays/{id}/rotate-psk"], ["post", "/api/admin/wol/relays/{id}/probe"]] as const) assert.equal(contract.paths[path]?.[method]?.["x-fn-knock-contract-source"], "utoipa");
    assert.equal(contract.paths["/api/admin/wol/relays"]?.post?.requestBody?.content?.["application/json"]?.schema?.$ref, "#/components/schemas/WolRelayInputData");
  });
});
