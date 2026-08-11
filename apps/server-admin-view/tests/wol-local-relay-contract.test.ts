import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(
  readFileSync(
    new URL("../../../packages/api-contract/openapi.json", import.meta.url),
    "utf8",
  ),
) as {
  paths: Record<
    string,
    Record<
      string,
      {
        "x-fn-knock-contract-source"?: string;
        requestBody?: {
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
      }
    >
  >;
};

describe("Wake-on-LAN local Relay API contract", () => {
  it("uses typed runtime routes while retaining both relay write schemas", () => {
    const relay = contract.paths["/api/admin/wol/local-relay"];
    const pair = contract.paths["/api/admin/wol/local-relay/pair"]?.post;

    assert.equal(relay?.get?.["x-fn-knock-contract-source"], "utoipa");
    assert.equal(relay?.put?.["x-fn-knock-contract-source"], "utoipa");
    assert.equal(pair?.["x-fn-knock-contract-source"], "utoipa");
    assert.equal(
      relay?.put?.requestBody?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/WolLocalRelayInputData",
    );
    assert.equal(
      pair?.requestBody?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/WolLocalRelayPairBodyData",
    );
  });
});
