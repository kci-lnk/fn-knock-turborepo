import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");
const isGeneratedContractSource = (source: unknown) =>
  source === "utoipa" || source === "utoipa-domain";

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: {
    schemas: Record<
      string,
      {
        properties?: Record<
          string,
          {
            items?: { items?: boolean; prefixItems?: unknown[] };
          }
        >;
        required?: string[];
      }
    >;
  };
  paths: Record<
    string,
    Record<string, { "x-fn-knock-contract-source"?: string }>
  >;
};

describe("core security API contract", () => {
  it("keeps overview, scanner, and general blacklist operations typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/security/overview"],
      ["get", "/api/admin/scanner/settings"],
      ["post", "/api/admin/scanner/settings"],
      ["get", "/api/admin/scanner/path-whitelist"],
      ["put", "/api/admin/scanner/path-whitelist"],
      ["post", "/api/admin/scanner/path-whitelist/false-positive"],
      ["get", "/api/admin/scanner/blacklist"],
      ["delete", "/api/admin/scanner/blacklist"],
      ["get", "/api/admin/scanner/blacklist/{ip}"],
      ["delete", "/api/admin/scanner/blacklist/{ip}"],
      ["get", "/api/admin/general-blacklist"],
      ["post", "/api/admin/general-blacklist"],
      ["delete", "/api/admin/general-blacklist"],
      ["post", "/api/admin/general-blacklist/status"],
      ["delete", "/api/admin/general-blacklist/{ip}"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("separates scanner writes and models stable response fields", () => {
    const scanner = contract.components.schemas.ScannerSettingsData.properties;
    for (const field of [
      "cidrExemptionPolicyId",
      "cidrExemptionSourceCidrCount",
      "cidrExemptionRangeCount",
    ]) {
      assert.ok(scanner?.[field], field);
    }

    const update =
      contract.components.schemas.ScannerSettingsUpdateData.properties;
    for (const field of [
      "windowSeconds",
      "cidrExemptionPolicyId",
      "cidrExemptionSourceCidrCount",
      "cidrExemptionRangeCount",
    ]) {
      assert.equal(update?.[field], undefined, field);
    }

    const required =
      contract.components.schemas.GeneralBlacklistRecordData.required ?? [];
    for (const field of ["source", "comment", "created_at", "updated_at"]) {
      assert.ok(required.includes(field), field);
    }

    const pathWhitelistRequired =
      contract.components.schemas.ScannerPathWhitelistData.required ?? [];
    for (const field of ["paths", "defaultPaths"]) {
      assert.ok(pathWhitelistRequired.includes(field), field);
    }

    const falsePositiveRequired =
      contract.components.schemas.ScannerFalsePositiveResultData.required ?? [];
    for (const field of ["ip", "path", "added", "unblocked"]) {
      assert.ok(falsePositiveRequired.includes(field), field);
    }

    const point =
      contract.components.schemas.SecurityOverviewSeriesData.properties
        ?.failedLogins.items;
    assert.equal(point?.items, false);
    assert.equal(point?.prefixItems?.length, 2);
  });

  it("derives frontend models and request boundaries from generated types", () => {
    const api = readSource("../src/lib/api/security.ts");
    const types = readSource("../src/types/gateway.ts");
    const generalView = readSource(
      "../src/views/session-management/GeneralBlacklistTab.vue",
    );

    assert.match(api, /ScannerSettingsData/u);
    assert.match(api, /ScannerPathWhitelistData/u);
    assert.match(api, /ScannerFalsePositiveResultData/u);
    assert.match(api, /GeneralBlacklistRecordData/u);
    assert.match(api, /operations as ApiContractOperations/u);
    assert.match(api, /satisfies ScannerBlacklistQuery/u);
    assert.match(api, /satisfies GeneralBlacklistAddBody/u);
    assert.doesNotMatch(api, /export type ScannerSettings = \{/u);
    assert.doesNotMatch(api, /export type GeneralBlacklistRecord = \{/u);
    assert.match(types, /SecurityOverviewData"\]/u);
    assert.doesNotMatch(generalView, /record\.ipLocation/u);
  });
});
