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
        properties?: Record<
          string,
          {
            enum?: string[];
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
    Record<string, { "x-fn-knock-contract-source"?: string }>
  >;
};

describe("FNOS and Smart Connect API contract", () => {
  it("keeps all migrated FNOS capability operations typed", () => {
    for (const [method, path] of [
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa-domain",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps fnOS port icon configuration bound to its actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/fnos_port_icon_hijack"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps fnOS network tuning bound to its actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/fnos_network_tuning"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps FN Connect WAF bound to its actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/fnos_connect_waf"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps fnOS share bypass bound to its actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/fnos_share_bypass"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps Smart Connect bound to its actual typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/smart_connect/details"],
      ["post", "/api/admin/config/smart_connect"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves local network metadata and normalized timeout bounds", () => {
    const localIpRequired =
      contract.components.schemas.SmartConnectLocalIpData.required ?? [];
    for (const field of ["interface", "netmask", "prefix"]) {
      assert.ok(localIpRequired.includes(field), field);
    }

    const share = contract.components.schemas.FnosShareBypassData.properties;
    assert.equal(share?.upstream_timeout_ms?.minimum, 500);
    assert.equal(share?.upstream_timeout_ms?.maximum, 15_000);
    assert.equal(share?.session_ttl_seconds?.minimum, 30);
    assert.equal(share?.session_ttl_seconds?.maximum, 3_600);
  });

  it("captures Lite blocking and server-owned runtime fields", () => {
    assert.deepEqual(
      contract.components.schemas.FnosNetworkTuningData.properties
        ?.blocked_reason_code?.enum,
      ["lite", "deployment", "platform", "permission"],
    );
    assert.equal(
      contract.components.schemas.FnosPortIconHijackUpdateData.properties
        ?.updated_at,
      undefined,
    );
    const runtimeRequired =
      contract.components.schemas.FnosConnectWafRuntimeData.required ?? [];
    for (const field of [
      "detected_http_port",
      "listener_port",
      "local_networks",
      "source",
      "last_sync_at",
      "last_error",
    ]) {
      assert.ok(runtimeRequired.includes(field), field);
    }
  });

  it("derives the frontend FNOS and Smart Connect models from OpenAPI", () => {
    const types = readSource("../src/types.ts");
    const systemApi = readSource("../src/lib/api/system.ts");
    const smartViewModel = readSource(
      "../src/views/system-settings/smart-connect/useSmartConnectViewModel.ts",
    );

    for (const schema of [
      "SmartConnectDetailsData",
      "FnosShareBypassData",
      "FnosPortIconHijackData",
      "FnosNetworkTuningData",
      "FnosConnectWafData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(systemApi, /\["FnosNetworkTuningUpdateData"\]/u);
    assert.match(systemApi, /satisfies FnosConnectWafUpdate/u);
    assert.match(smartViewModel, /netmask: ""/u);
    assert.match(smartViewModel, /prefix: null/u);
  });
});
