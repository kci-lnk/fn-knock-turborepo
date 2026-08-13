import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  const?: boolean | number | string;
  enum?: string[];
  format?: string;
  maximum?: number;
  minimum?: number;
  maxItems?: number;
  oneOf?: Schema[];
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  uniqueItems?: boolean;
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{ name?: string; schema?: Schema }>;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("Cloudflared API contract", () => {
  it("keeps every Cloudflared operation on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/cloudflared/status"],
      ["get", "/api/admin/cloudflared/config"],
      ["post", "/api/admin/cloudflared/config"],
      ["post", "/api/admin/cloudflared/start"],
      ["post", "/api/admin/cloudflared/stop"],
      ["get", "/api/admin/cloudflared/logs"],
      ["delete", "/api/admin/cloudflared/logs"],
      ["get", "/api/admin/cloudflared/poll"],
      ["put", "/api/admin/cloudflared/cloudflare/credential"],
      ["delete", "/api/admin/cloudflared/cloudflare/credential"],
      ["get", "/api/admin/cloudflared/cloudflare/state"],
      ["post", "/api/admin/cloudflared/reconcile/preview"],
      ["post", "/api/admin/cloudflared/reconcile/apply"],
      ["get", "/api/admin/cloudflared/reconcile/jobs/active"],
      ["get", "/api/admin/cloudflared/reconcile/jobs/{id}"],
      [
        "get",
        "/api/admin/cloudflared/reconcile/jobs/by-plan/{plan_id}",
      ],
      ["post", "/api/admin/cloudflared/optimization/scans"],
      ["get", "/api/admin/cloudflared/optimization/scans/{id}"],
      ["delete", "/api/admin/cloudflared/optimization/scans/{id}"],
      ["post", "/api/admin/cloudflared/optimization/apply"],
      ["post", "/api/admin/cloudflared/optimization/fallback"],
      ["put", "/api/admin/cloudflared/optimization/settings"],
      ["put", "/api/admin/cloudflared/optimization/domains/{hostname}"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("documents public configuration without exposing credentials", () => {
    const config = contract.components.schemas.CloudflaredConfigData;
    const update = contract.components.schemas.CloudflaredConfigUpdateData;
    const credential = contract.components.schemas.CloudflareCredentialBodyData;
    assert.ok(config.required?.includes("rootDomain"));
    assert.equal(config.properties?.token, undefined);
    assert.deepEqual(update.required ?? [], []);
    assert.equal(update.properties?.token?.writeOnly, true);
    assert.equal(credential.properties?.apiToken?.writeOnly, true);
    assert.deepEqual(config.properties?.protocol?.enum, [
      "auto",
      "http2",
      "quic",
    ]);
  });

  it("preserves supervisor and log cursor compatibility", () => {
    const failure =
      contract.components.schemas.CloudflaredSupervisorFailureData;
    assert.ok(failure.required?.includes("resources"));
    assert.equal(failure.properties?.at?.format, "date-time");

    const logParameters =
      contract.paths["/api/admin/cloudflared/logs"].get.parameters ?? [];
    const pollParameters =
      contract.paths["/api/admin/cloudflared/poll"].get.parameters ?? [];
    const limit = logParameters.find(
      (parameter) => parameter.name === "limit",
    )?.schema;
    const cursor = pollParameters.find(
      (parameter) => parameter.name === "cursor",
    )?.schema;
    assert.equal(limit?.oneOf?.[1]?.pattern, "^\\s*[+-]?\\d+");
    assert.equal(cursor?.oneOf?.[0]?.minimum, 0);
    assert.equal(cursor?.oneOf?.[1]?.pattern, "^[0-9]+$");
  });

  it("captures reconcile defaults and optimization safety bounds", () => {
    const reconcile =
      contract.components.schemas.CloudflareReconcileRequestData;
    assert.deepEqual(reconcile.required ?? [], []);
    assert.deepEqual(reconcile.properties?.action?.enum, ["apply", "cleanup"]);
    assert.deepEqual(reconcile.properties?.tunnelMode?.enum, [
      "dedicated",
      "existing",
    ]);

    const sources =
      contract.components.schemas.CloudflareOptimizationSourceSettingsBodyData;
    assert.equal(sources.properties?.customHostnames?.maxItems, 16);
    assert.equal(sources.properties?.customHostnames?.uniqueItems, true);
    const scan = contract.components.schemas.CloudflareOptimizationScanData;
    assert.equal(scan.properties?.progress?.minimum, 0);
    assert.equal(scan.properties?.progress?.maximum, 100);
    assert.deepEqual(scan.properties?.resolutionPath?.enum, [
      "multi-doh",
      "official-ranges",
      "current-candidate",
      "unavailable",
    ]);
    const reconcileJob =
      contract.components.schemas.CloudflareReconcileJobData;
    assert.equal(reconcileJob.properties?.progress?.minimum, 0);
    assert.equal(reconcileJob.properties?.progress?.maximum, 100);
    assert.deepEqual(reconcileJob.properties?.status?.enum, [
      "queued",
      "running",
      "succeeded",
      "failed",
      "interrupted",
    ]);
    const diagnostics =
      contract.components.schemas.CloudflareOptimizationResolverDiagnosticData;
    assert.deepEqual(diagnostics.properties?.provider?.enum, [
      "cloudflare",
      "google",
      "dnspod",
      "alidns",
    ]);
    assert.deepEqual(diagnostics.properties?.status?.enum, [
      "healthy",
      "degraded",
      "unavailable",
    ]);
  });

  it("derives frontend Cloudflared models, requests, and queries", () => {
    const api = readSource("../src/lib/api/tunnel.ts");
    for (const schema of [
      "CloudflaredConfigData",
      "CloudflaredSupervisorData",
      "CloudflareManagedStateData",
      "CloudflareOptimizationScanData",
      "CloudflareReconcileJobData",
      "CloudflareReconcilePlanData",
      "CloudflaredConfigUpdateData",
      "CloudflareCredentialBodyData",
      "CloudflareReconcileRequestData",
      "CloudflareOptimizationSourceSettingsBodyData",
    ]) {
      assert.match(api, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_cloudflared_logs/u);
    assert.match(api, /get_api_admin_cloudflared_poll/u);
    assert.match(api, /satisfies CloudflareCredentialBody/u);
    assert.match(api, /satisfies CloudflaredLogsQuery/u);
  });
});
