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
        required?: string[];
      }
    >;
  };
  paths: Record<
    string,
    Record<
      string,
      {
        "x-fn-knock-contract-source"?: string;
        responses?: Record<
          string,
          {
            content?: Record<string, { schema?: { $ref?: string } }>;
          }
        >;
      }
    >
  >;
};

describe("whitelist API contract", () => {
  it("keeps every access-control operation on generated domain schemas", () => {
    for (const [method, path] of [
      ["get", "/api/admin/whitelist"],
      ["post", "/api/admin/whitelist"],
      ["get", "/api/admin/whitelist/regions"],
      ["post", "/api/admin/whitelist/regions"],
      ["delete", "/api/admin/whitelist/regions/{id}"],
      ["delete", "/api/admin/whitelist/{id}"],
      ["patch", "/api/admin/whitelist/{id}/comment"],
      ["post", "/api/admin/whitelist/{id}/refresh"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("models pending grants and data-bearing CNAME resolution failures", () => {
    const record = contract.components.schemas.WhitelistRecordData;
    assert.deepEqual(record.properties?.status.enum, [
      "active",
      "pending",
      "expired",
      "deleted",
    ]);
    assert.ok(record.required?.includes("expireAt"));
    assert.equal(
      contract.paths["/api/admin/whitelist/{id}/refresh"]?.post?.responses?.[
        "200"
      ]?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/WhitelistRefreshEnvelopeData",
    );
  });

  it("derives frontend records, requests, and responses from the contract", () => {
    const api = readSource("../src/lib/api/whitelist.ts");
    assert.match(api, /components as ApiContractComponents/u);
    assert.match(api, /operations as ApiContractOperations/u);
    assert.match(
      api,
      /WhiteListRecord = WhitelistSchemas\["WhitelistRecordData"\]/u,
    );
    assert.match(
      api,
      /WhitelistAddBody = WhitelistSchemas\["WhitelistAddBodyData"\]/u,
    );
    assert.doesNotMatch(api, /export interface WhiteListRecord/u);
  });
});
