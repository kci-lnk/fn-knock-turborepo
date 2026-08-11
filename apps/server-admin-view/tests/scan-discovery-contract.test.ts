import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type PropertySchema = {
  enum?: string[];
  maximum?: number;
  minimum?: number;
  maxItems?: number;
  minItems?: number;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{
    name?: string;
    schema?: PropertySchema;
  }>;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: {
    schemas: Record<
      string,
      {
        properties?: Record<string, PropertySchema>;
        required?: string[];
      }
    >;
  };
  paths: Record<string, Record<string, Operation>>;
};

describe("scan discovery API contract", () => {
  it("keeps settings, targets, jobs, and host probes typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/scan/discover-settings"],
      ["post", "/api/admin/scan/discover-settings"],
      ["get", "/api/admin/scan/discover-targets"],
      ["post", "/api/admin/scan/discover-targets"],
      ["post", "/api/admin/scan/discover/jobs"],
      ["get", "/api/admin/scan/discover/jobs/{job_id}"],
      ["delete", "/api/admin/scan/discover/jobs/{job_id}"],
      ["post", "/api/admin/scan/host-mappings/probe"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves intensity, CIDR, cursor, and probe boundaries", () => {
    const settings =
      contract.components.schemas.ScanDiscoverySettingsData.properties;
    assert.deepEqual(settings?.intensityMode?.enum, ["auto", "manual"]);
    assert.deepEqual(settings?.effectiveLevel?.enum, [
      "low",
      "medium",
      "high",
      "extreme",
    ]);
    assert.equal(settings?.effectiveConcurrency?.minimum, 1);

    const targetCidrs =
      contract.components.schemas.ScanDiscoverJobBodyData.properties
        ?.target_cidrs;
    assert.equal(targetCidrs?.minItems, 1);
    assert.equal(targetCidrs?.maxItems, 16);

    const jobGet =
      contract.paths["/api/admin/scan/discover/jobs/{job_id}"].get;
    assert.equal(
      jobGet.parameters?.find((parameter) => parameter.name === "cursor")
        ?.schema?.minimum,
      0,
    );
    assert.deepEqual(
      contract.components.schemas.HostMappingProbeResultData.properties?.status
        ?.enum,
      ["online", "stale", "unsupported"],
    );
  });

  it("separates startup metadata from completed results and keeps job nulls", () => {
    const metaRequired =
      contract.components.schemas.ScanDiscoverMetaData.required ?? [];
    const resultRequired =
      contract.components.schemas.ScanDiscoverResultData.required ?? [];
    assert.ok(metaRequired.includes("portRange"));
    assert.equal(metaRequired.includes("services"), false);
    assert.ok(resultRequired.includes("services"));

    const jobRequired =
      contract.components.schemas.ScanDiscoverJobData.required ?? [];
    for (const field of ["meta", "progress", "result", "error"]) {
      assert.ok(jobRequired.includes(field), field);
    }
  });

  it("derives frontend network models and normalizes empty UI state", () => {
    const scanApi = readSource("../src/lib/api/scan.ts");
    const reverseFlow = readSource(
      "../src/views/reverse-proxy/useReverseProxyDiscoverFlow.ts",
    );
    const subdomainFlow = readSource(
      "../src/views/subdomain-proxy/useSubdomainDiscoverFlow.ts",
    );

    for (const schema of [
      "ScanDiscoverySettingsData",
      "ScanDiscoveryTargetsData",
      "ScanDiscoverMetaData",
      "ScanDiscoverResultData",
      "ScanDiscoverJobData",
      "HostMappingProbeResultData",
    ]) {
      assert.match(scanApi, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    for (const source of [reverseFlow, subdomainFlow]) {
      assert.match(source, /scanScope: patch\.scanScope \?\? null/u);
      assert.match(source, /scanCidrs: patch\.scanCidrs \?\? \[\]/u);
      assert.match(source, /intensityMode: patch\.intensityMode \?\? "auto"/u);
    }
  });
});
