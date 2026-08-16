import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const contract = JSON.parse(
  readFileSync(
    new URL("../../../packages/api-contract/openapi.json", import.meta.url),
    "utf8",
  ),
) as {
  components: {
    schemas: Record<
      string,
      { properties?: Record<string, { writeOnly?: boolean }> }
    >;
  };
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

describe("Wake-on-LAN target API contract", () => {
  it("uses typed routes and preserves target writes", () => {
    for (const [method, path] of [
      ["get", "/api/admin/wol/targets"],
      ["post", "/api/admin/wol/targets"],
      ["get", "/api/admin/wol/targets/{id}"],
      ["put", "/api/admin/wol/targets/{id}"],
      ["delete", "/api/admin/wol/targets/{id}"],
      ["post", "/api/admin/wol/targets/{id}/wake"],
      ["post", "/api/admin/wol/targets/{id}/ssh/test"],
      ["post", "/api/admin/wol/targets/{id}/shutdown"],
    ] as const)
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
      );
    assert.equal(
      contract.paths["/api/admin/wol/targets/{id}/ssh/host-key"],
      undefined,
    );
    assert.equal(
      contract.paths["/api/admin/wol/targets"]?.post?.requestBody?.content?.[
        "application/json"
      ]?.schema?.$ref,
      "#/components/schemas/WolTargetInputData",
    );
  });

  it("keeps SSH credentials write-only and out of target responses", () => {
    const input = contract.components.schemas.WolTargetSshInputData.properties;
    const response = contract.components.schemas.WolTargetSshData.properties;

    for (const secret of ["password", "privateKey", "privateKeyPassphrase"]) {
      assert.equal(input?.[secret]?.writeOnly, true, secret);
      assert.equal(response?.[secret], undefined, secret);
    }
    assert.ok(response?.credentialConfigured);
    assert.ok(response?.passphraseConfigured);
    const testResult =
      contract.components.schemas.WolSshConnectionTestData.properties;
    assert.ok(testResult?.hostKeyAlgorithm);
    assert.ok(testResult?.hostKeyFingerprint);
  });
});
