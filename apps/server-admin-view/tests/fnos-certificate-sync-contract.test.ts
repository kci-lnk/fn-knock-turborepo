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
        properties?: Record<string, { enum?: string[]; pattern?: string }>;
        required?: string[];
      }
    >;
  };
  paths: Record<
    string,
    Record<string, { "x-fn-knock-contract-source"?: string }>
  >;
};

describe("fnOS certificate synchronization API contract", () => {
  it("keeps details, configuration, and manual sync operations on their actual typed routes", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/fnos_certificate_sync/details"],
      ["post", "/api/admin/config/fnos_certificate_sync"],
      ["post", "/api/admin/config/fnos_certificate_sync/sync"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves nullable runtime and certificate comparison fields", () => {
    const runtimeRequired =
      contract.components.schemas.FnosCertificateSyncRuntimeData.required ?? [];
    for (const field of [
      "last_sync_at",
      "last_result",
      "last_error",
      "failed_target_ids",
    ]) {
      assert.ok(runtimeRequired.includes(field), field);
    }

    const itemRequired =
      contract.components.schemas.FnosCertificateSyncItemData.required ?? [];
    for (const field of [
      "valid_from",
      "valid_to",
      "fingerprint",
      "reason",
      "local",
    ]) {
      assert.ok(itemRequired.includes(field), field);
    }
  });

  it("documents every comparison status and keeps target selection optional", () => {
    assert.deepEqual(
      contract.components.schemas.FnosCertificateSyncItemData.properties?.status
        ?.enum,
      [
        "unmatched",
        "up_to_date",
        "syncable",
        "source_invalid",
        "target_invalid",
        "protected",
        "sync_failed",
      ],
    );
    assert.equal(
      contract.components.schemas.FnosCertificateSyncBodyData.required?.includes(
        "target_ids",
      ) ?? false,
      false,
    );
  });

  it("derives frontend models and write payloads from OpenAPI", () => {
    const types = readSource("../src/types/core.ts");
    const systemApi = readSource("../src/lib/api/system.ts");

    for (const schema of [
      "FnosCertificateSyncItemData",
      "FnosCertificateSyncDetailsData",
      "FnosCertificateSyncSummaryData",
      "FnosCertificateSyncResponseData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(systemApi, /satisfies FnosCertificateSyncUpdate/u);
    assert.match(systemApi, /satisfies FnosCertificateSyncBody/u);
  });
});
