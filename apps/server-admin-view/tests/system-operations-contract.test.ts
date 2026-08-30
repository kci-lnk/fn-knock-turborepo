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
            const?: number | string;
            enum?: Array<number | string>;
            maximum?: number;
            minimum?: number;
          }
        >;
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
          required?: boolean;
          schema?: { minimum?: number; maximum?: number };
        }>;
        requestBody?: {
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
        responses?: Record<
          string,
          {
            content?: Record<
              string,
              {
                schema?: {
                  $ref?: string;
                  properties?: {
                    data?: { anyOf?: Array<{ $ref?: string; type?: string }> };
                  };
                };
              }
            >;
          }
        >;
      }
    >
  >;
};

describe("dashboard and update API contract", () => {
  it("keeps all system operation routes typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/dashboard_display"],
      ["post", "/api/admin/config/dashboard_display"],
      ["get", "/api/admin/dashboard/stats"],
      ["get", "/api/admin/dashboard/realtime"],
      ["get", "/api/admin/dashboard/active-ips"],
      ["get", "/api/admin/dashboard/stream-active-ips"],
      ["get", "/api/admin/update/status"],
      ["post", "/api/admin/update/check"],
      ["post", "/api/admin/update/check-and-download"],
      ["post", "/api/admin/update/download"],
      ["post", "/api/admin/update/install"],
      ["get", "/api/admin/update/confirm"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves dashboard query and always-present response fields", () => {
    const range = contract.paths[
      "/api/admin/dashboard/stats"
    ].get.parameters?.find(
      (parameter) => parameter.name === "rangeSec",
    )?.schema;
    assert.equal(range?.minimum, 60);
    assert.equal(range?.maximum, 2_592_000);
    assert.equal(
      contract.paths["/api/admin/dashboard/active-ips"].get.parameters?.find(
        (parameter) => parameter.name === "host",
      )?.required,
      true,
    );
    assert.equal(
      contract.paths[
        "/api/admin/dashboard/stream-active-ips"
      ].get.parameters?.find((parameter) => parameter.name === "stream")
        ?.required,
      true,
    );

    for (const [schema, fields] of [
      ["DashboardRealtimeData", ["by_host", "timestamp"]],
      ["DashboardHostTrafficData", ["active_ip_count"]],
      ["DashboardStreamTrafficData", ["active_ip_count"]],
      ["DashboardActiveIpsData", ["timestamp"]],
      ["DashboardStreamActiveIpsData", ["timestamp"]],
    ] as const) {
      const required = contract.components.schemas[schema].required ?? [];
      for (const field of fields) assert.ok(required.includes(field), field);
    }
  });

  it("separates dashboard input and preserves nullable update confirmation", () => {
    assert.equal(
      contract.paths["/api/admin/config/dashboard_display"].post.requestBody
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/DashboardDisplayUpdateData",
    );
    assert.deepEqual(
      contract.components.schemas.UpdateDownloadData.properties?.status?.enum,
      ["idle", "downloading", "verifying", "downloaded", "installing", "error"],
    );
    assert.ok(
      contract.components.schemas.UpdateStatusData.required?.includes("latest"),
    );
    const confirm =
      contract.paths["/api/admin/update/confirm"].get.responses?.["200"]
        ?.content?.["application/json"]?.schema?.properties?.data?.anyOf ?? [];
    assert.ok(
      confirm.some(
        (schema) => schema.$ref === "#/components/schemas/UpdateConfirmData",
      ),
    );
    assert.ok(confirm.some((schema) => schema.type === "null"));
  });

  it("derives frontend dashboard and update models from generated types", () => {
    const types = readSource("../src/types/core.ts");
    const dashboardApi = readSource("../src/lib/api/dashboard.ts");
    const configApi =
      readSource("../src/lib/api/config.ts") +
      readSource("../src/lib/api/config-core-api.ts");

    assert.match(types, /\["DashboardRealtimeData"\]/u);
    assert.match(types, /\["DashboardStatsData"\]/u);
    assert.doesNotMatch(types, /export type TrafficStats = \{/u);
    assert.match(dashboardApi, /satisfies DashboardStatsQuery/u);
    assert.match(dashboardApi, /satisfies DashboardActiveIpsQuery/u);
    assert.match(configApi, /\["DashboardDisplayUpdateData"\]/u);
    assert.match(configApi, /\["UpdateStatusData"\]/u);
    assert.match(configApi, /get_api_admin_update_confirm/u);
    assert.doesNotMatch(configApi, /export type UpdateStatusPayload = \{/u);
  });
});

describe("firewall, route sync, and maintenance API contract", () => {
  it("keeps all system mutation routes typed", () => {
    for (const path of [
      "/api/admin/firewall/reset",
      "/api/admin/firewall/clear",
      "/api/admin/sync-routes",
      "/api/admin/maintenance/data/clear",
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.post?.["x-fn-knock-contract-source"],
        ),
        `POST ${path}`,
      );
    }
  });

  it("keeps route synchronization bound to its actual typed router", () => {
    assert.equal(
      contract.paths["/api/admin/sync-routes"].post?.[
        "x-fn-knock-contract-source"
      ],
      "utoipa",
    );
  });

  it("preserves firewall reset input and port boundaries", () => {
    assert.equal(
      contract.paths["/api/admin/firewall/reset"].post.requestBody?.content?.[
        "application/json"
      ]?.schema?.$ref,
      "#/components/schemas/FirewallResetBodyData",
    );
    assert.deepEqual(
      contract.components.schemas.FirewallResetBodyData.properties?.run_type
        ?.enum,
      [0, 1, 3],
    );
    for (const schema of ["FirewallResetData", "FirewallClearData"] as const) {
      const gatewayPort =
        contract.components.schemas[schema].properties?.gatewayPort;
      assert.equal(gatewayPort?.minimum, 1);
      assert.equal(gatewayPort?.maximum, 65_535);
    }
  });

  it("keeps complete route-sync and maintenance results", () => {
    const syncRequired =
      contract.components.schemas.SyncRoutesData.required ?? [];
    for (const field of [
      "synced_rules",
      "synced_host_rules",
      "synced_stream_rules",
      "synced_gateway_logging",
      "synced_waf",
      "waf_bundle_id",
    ]) {
      assert.ok(syncRequired.includes(field), field);
    }

    assert.equal(
      contract.paths["/api/admin/maintenance/data/clear"].post.requestBody
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/MaintenanceClearBodyData",
    );
    assert.ok(
      contract.components.schemas.MaintenanceClearData.required?.includes(
        "gateway_reset",
      ),
    );
  });

  it("derives frontend mutation requests and responses from generated types", () => {
    const configApi =
      readSource("../src/lib/api/config.ts") +
      readSource("../src/lib/api/config-core-api.ts");
    const systemApi = readSource("../src/lib/api/system.ts");
    const mappingActions = readSource(
      "../src/views/subdomain-proxy/useSubdomainMappingListActions.ts",
    );

    assert.match(systemApi, /\["FirewallResetBodyData"\]/u);
    assert.match(configApi, /\["MaintenanceClearData"\]/u);
    assert.match(configApi, /post_api_admin_sync_routes/u);
    assert.match(systemApi, /satisfies FirewallResetBody/u);
    assert.match(configApi, /satisfies MaintenanceClearBody/u);
    assert.match(mappingActions, /post_api_admin_sync_routes/u);
  });
});

describe("system access entry and clock API contract", () => {
  it("keeps access and clock operations typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/system/access-entry"],
      ["get", "/api/admin/system/clock/status"],
      ["post", "/api/admin/system/clock/check"],
      ["post", "/api/admin/system/clock/sync"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves access sources and clock constants", () => {
    assert.deepEqual(
      contract.components.schemas.AccessEntryData.properties?.env?.enum,
      ["GO_REPROXY_PORT", "FRP_REMOTE_PORT"],
    );
    const clock = contract.components.schemas.SystemClockStatusData;
    assert.equal(clock.properties?.expectedTimeZone?.const, "Asia/Shanghai");
    assert.equal(clock.properties?.driftThresholdMs?.const, 90_000);
    assert.deepEqual(
      contract.components.schemas.SystemClockIssueData.properties?.code?.enum,
      ["timezone_mismatch", "time_mismatch"],
    );
  });

  it("keeps nullable status keys present and sync message non-null", () => {
    const required =
      contract.components.schemas.SystemClockStatusData.required ?? [];
    for (const field of [
      "systemTimeZone",
      "checkedAt",
      "networkSource",
      "lastCheckError",
      "systemTimeMs",
      "remoteTimeMs",
      "systemBeijingTime",
      "remoteBeijingTime",
      "driftMs",
      "lastSyncAt",
      "lastSyncError",
      "syncSummary",
    ]) {
      assert.ok(required.includes(field), field);
    }
    assert.equal(
      contract.paths["/api/admin/system/clock/sync"].post.responses?.["200"]
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/SystemClockSyncResponseData",
    );
    for (const field of ["success", "message", "data"]) {
      assert.ok(
        contract.components.schemas.SystemClockSyncResponseData.required?.includes(
          field,
        ),
        field,
      );
    }
  });

  it("derives frontend access and clock models from generated schemas", () => {
    const systemApi = readSource("../src/lib/api/system.ts");
    assert.match(systemApi, /\["AccessEntryData"\]/u);
    assert.match(systemApi, /\["SystemClockIssueData"\]/u);
    assert.match(systemApi, /\["SystemClockStatusData"\]/u);
    assert.match(systemApi, /post_api_admin_system_clock_sync/u);
    assert.doesNotMatch(systemApi, /expectedTimeZone: string/u);
  });
});

describe("system-managed binary and dnsmasq API contract", () => {
  it("keeps every asset operation typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/system/cloudflared/status"],
      ["post", "/api/admin/system/cloudflared/download"],
      ["post", "/api/admin/system/cloudflared/cancel"],
      ["delete", "/api/admin/system/cloudflared"],
      ["get", "/api/admin/system/frp/status"],
      ["post", "/api/admin/system/frp/download"],
      ["post", "/api/admin/system/frp/cancel"],
      ["delete", "/api/admin/system/frp"],
      ["get", "/api/admin/system/dnsmasq/status"],
      ["post", "/api/admin/system/dnsmasq/install"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves progress state, percentage, and nullable errors", () => {
    const progress =
      contract.components.schemas.SystemAssetDownloadProgressData;
    assert.deepEqual(progress.properties?.status?.enum, [
      "idle",
      "downloading",
      "completed",
      "error",
    ]);
    assert.equal(progress.properties?.percent?.minimum, 0);
    assert.equal(progress.properties?.percent?.maximum, 100);
    assert.ok(progress.required?.includes("error"));

    const dnsmasq = contract.components.schemas.DnsmasqInstallStateData;
    assert.deepEqual(dnsmasq.properties?.status?.enum, [
      "uninstalled",
      "installing",
      "installed",
      "error",
    ]);
    assert.equal(dnsmasq.properties?.progress?.minimum, 0);
    assert.equal(dnsmasq.properties?.progress?.maximum, 100);
  });

  it("separates platform support and requires mutation messages", () => {
    const cloudflaredPlatforms =
      contract.components.schemas.CloudflaredAssetStatusData.properties
        ?.platform?.enum ?? [];
    const frpPlatforms =
      contract.components.schemas.FrpAssetStatusData.properties?.platform
        ?.enum ?? [];
    assert.ok(cloudflaredPlatforms.includes("windows-amd64"));
    assert.ok(cloudflaredPlatforms.includes("linux-armhf"));
    assert.ok(!frpPlatforms.includes("windows-amd64"));
    assert.ok(frpPlatforms.includes("unsupported"));
    assert.deepEqual(
      contract.components.schemas.CloudflaredAssetStatusData.properties
        ?.installation_status?.enum,
      ["missing", "outdated", "current"],
    );
    for (const schema of [
      contract.components.schemas.CloudflaredAssetStatusData,
      contract.components.schemas.FrpAssetStatusData,
    ]) {
      assert.deepEqual(schema.properties?.installation_status?.enum, [
        "missing",
        "outdated",
        "current",
      ]);
      for (const field of ["installation_status", "target_version"] as const) {
        assert.ok(schema.required?.includes(field), field);
      }
    }

    for (const [method, path] of [
      ["post", "/api/admin/system/cloudflared/download"],
      ["post", "/api/admin/system/cloudflared/cancel"],
      ["delete", "/api/admin/system/cloudflared"],
      ["post", "/api/admin/system/frp/download"],
      ["post", "/api/admin/system/frp/cancel"],
      ["delete", "/api/admin/system/frp"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.responses?.["200"]?.content?.[
          "application/json"
        ]?.schema?.$ref,
        "#/components/schemas/SystemAssetMutationResponseData",
      );
    }
    assert.ok(
      contract.components.schemas.SystemAssetMutationResponseData.required?.includes(
        "message",
      ),
    );
  });

  it("derives frontend binary and dnsmasq models from generated schemas", () => {
    const types = readSource("../src/types/core.ts");
    const systemApi = readSource("../src/lib/api/system.ts");
    const binarySettings = readSource(
      "../src/views/system-settings/BinaryResourceSettings.vue",
    );

    assert.match(types, /\["DnsmasqInstallStateData"\]/u);
    assert.match(types, /\["DnsmasqStatusData"\]/u);
    assert.match(systemApi, /get_api_admin_system_frp_status/u);
    assert.match(systemApi, /get_api_admin_system_cloudflared_status/u);
    assert.match(systemApi, /\["SystemAssetMutationResponseData"\]/u);
    assert.match(binarySettings, /\["SystemAssetDownloadProgressData"\]/u);
    assert.match(binarySettings, /\["CloudflaredAssetStatusData"\]/u);
    assert.doesNotMatch(
      binarySettings,
      /type ResourceDownloadStatus = "idle"/u,
    );
  });
});
