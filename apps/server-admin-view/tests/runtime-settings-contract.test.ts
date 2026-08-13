import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type PropertySchema = {
  const?: string | number | boolean;
  enum?: Array<string | number>;
  format?: string;
  items?: PropertySchema;
  maxItems?: number;
  maximum?: number;
  minimum?: number;
  uniqueItems?: boolean;
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
  paths: Record<
    string,
    Record<string, { "x-fn-knock-contract-source"?: string }>
  >;
};

describe("runtime settings API contract", () => {
  it("keeps locale and appearance settings bound to actual typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/locale"],
      ["post", "/api/admin/config/locale"],
      ["get", "/api/admin/config/appearance"],
      ["post", "/api/admin/config/appearance"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps proxy protocol and run mode prompt operations bound to actual typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/proxy_protocol_force"],
      ["post", "/api/admin/config/proxy_protocol_force"],
      ["get", "/api/admin/config/run_mode_prompt_preferences"],
      ["post", "/api/admin/config/run_mode_prompt_preferences"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps protocol, HTTPS, and default route operations bound to actual typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/protocol_mapping_feature"],
      ["post", "/api/admin/config/protocol_mapping_feature"],
      ["get", "/api/admin/config/auto_https"],
      ["post", "/api/admin/config/auto_https"],
      ["get", "/api/admin/config/default_route"],
      ["post", "/api/admin/config/default_route"],
      ["post", "/api/admin/config/default_tunnel"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps firewall port settings bound to the actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/firewall_additional_ports"]?.[
          method
        ]?.["x-fn-knock-contract-source"],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("preserves closed enums and always-present runtime state", () => {
    assert.deepEqual(
      contract.components.schemas.PanelAppearanceData.properties
        ?.theme_color_preset?.enum,
      ["default", "hermes_orange", "prussian_blue", "dynamic_white"],
    );
    assert.deepEqual(
      contract.components.schemas.DefaultTunnelUpdateData.properties?.tunnel
        ?.enum,
      ["frp", "cloudflared"],
    );
    assert.equal(
      contract.components.schemas.AutoHttpsRuntimeData.properties?.listen_port
        ?.const,
      80,
    );
    for (const field of ["last_error", "last_error_at"]) {
      assert.ok(
        contract.components.schemas.AutoHttpsRuntimeData.required?.includes(
          field,
        ),
      );
    }
  });

  it("documents firewall and protocol availability boundaries", () => {
    const ports =
      contract.components.schemas.FirewallAdditionalPortsUpdateData.properties
        ?.ports;
    assert.equal(ports?.maxItems, 128);
    assert.equal(ports?.uniqueItems, true);
    assert.equal(ports?.items?.minimum, 1);
    assert.equal(ports?.items?.maximum, 65_535);
    assert.ok(
      contract.components.schemas.ProtocolMappingFeatureData.required?.includes(
        "availability",
      ),
    );
    assert.deepEqual(
      contract.components.schemas.RunModePromptPreferencesUpdateData.required ??
        [],
      [],
    );
  });

  it("derives frontend settings models and write payloads from the contract", () => {
    const types =
      readSource("../src/types/core.ts") +
      readSource("../src/types/gateway.ts");
    const configApi = readSource("../src/lib/api/config-core-api.ts");
    const systemApi = readSource("../src/lib/api/system.ts");
    const configStore = readSource("../src/store/config.ts");

    for (const schema of [
      "ProtocolMappingFeatureData",
      "AutoHttpsRuntimeData",
      "AutoHttpsDetailsData",
      "FirewallAdditionalPortsData",
      "ProxyProtocolForceData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(systemApi, /\["RunModePromptPreferencesData"\]/u);
    assert.match(systemApi, /satisfies FirewallAdditionalPortsUpdate/u);
    assert.match(configApi, /satisfies DefaultTunnelUpdate/u);
    assert.doesNotMatch(
      configStore,
      /saveAppearanceConfig\(next: Partial<AppearanceConfig>\)/u,
    );
  });
});
