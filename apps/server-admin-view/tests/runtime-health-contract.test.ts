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
        properties?: Record<string, unknown>;
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
        parameters?: Array<{
          name?: string;
          in?: string;
          schema?: { enum?: string[]; minimum?: number; maximum?: number };
        }>;
        responses?: Record<
          string,
          { content?: Record<string, { schema?: { format?: string } }> }
        >;
      }
    >
  >;
};

describe("runtime health API contract", () => {
  it("keeps all runtime health operations typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/runtime-health"],
      ["get", "/api/admin/runtime-health/gateway-memory"],
      ["put", "/api/admin/runtime-health/gateway-memory"],
      ["post", "/api/admin/runtime-health/gateway-memory/reclaim"],
      ["get", "/api/admin/runtime-health/logs/{component}"],
      ["delete", "/api/admin/runtime-health/logs/{component}"],
      ["get", "/api/admin/runtime-health/diagnostics"],
      ["get", "/api/admin/runtime-health/diagnostics/archive"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("bounds the Go runtime memory contract", () => {
    const config = contract.components.schemas.GatewayMemoryConfigUpdateData
      .properties?.gc_percent as
      { minimum?: number; maximum?: number } | undefined;
    assert.equal(config?.minimum, 25);
    assert.equal(config?.maximum, 500);
    const memoryLimit = contract.components.schemas
      .GatewayMemoryConfigUpdateData.properties?.memory_limit_mib as
      { minimum?: number; maximum?: number } | undefined;
    assert.equal(memoryLimit?.minimum, 64);
    assert.equal(memoryLimit?.maximum, 4096);
    assert.ok(
      contract.components.schemas.GatewayMemoryConfigData.properties
        ?.effective_memory_limit_bytes,
    );
    assert.ok(
      contract.components.schemas.GatewayMemoryReclaimData.properties
        ?.rss_bytes,
    );
    assert.ok(
      contract.components.schemas.GatewayMemoryReclaimData.properties
        ?.managed_memory_bytes,
    );
  });

  it("preserves the log boundary and ZIP response", () => {
    const parameters =
      contract.paths["/api/admin/runtime-health/logs/{component}"].get
        .parameters ?? [];
    const component = parameters.find(
      (parameter) => parameter.name === "component",
    );
    assert.deepEqual(component?.schema?.enum, [
      "management",
      "gateway_process",
    ]);
    const limit = parameters.find((parameter) => parameter.name === "limit");
    assert.equal(limit?.schema?.minimum, 1);
    assert.equal(limit?.schema?.maximum, 500);

    const archive =
      contract.paths["/api/admin/runtime-health/diagnostics/archive"].get
        .responses?.["200"]?.content ?? {};
    assert.equal(archive["application/zip"]?.schema?.format, "binary");
    assert.equal(archive["application/json"], undefined);
  });

  it("generates frontend models including the collection boundary", () => {
    const component =
      contract.components.schemas.RuntimeComponentHealthData.required ?? [];
    for (const field of [
      "version",
      "commit",
      "pid",
      "instance_id",
      "started_at",
      "last_checked_at",
      "last_success_at",
      "reason_code",
    ]) {
      assert.ok(component.includes(field), field);
    }
    assert.ok(
      contract.components.schemas.RuntimeDiagnosticsData.properties?.collection,
    );

    const types = readSource("../src/types/runtime-health.ts");
    const api = readSource("../src/lib/api/runtime-health.ts");
    assert.match(types, /RuntimeHealthSnapshotData/u);
    assert.match(types, /RuntimeDiagnosticsData/u);
    assert.doesNotMatch(types, /export interface RuntimeHealthSnapshot/u);
    assert.match(api, /operations as ApiContractOperations/u);
    assert.match(api, /satisfies RuntimeLogsQuery/u);
  });
});
