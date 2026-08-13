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
            enum?: Array<string | number>;
            format?: string;
            minimum?: number;
            maximum?: number;
            multipleOf?: number;
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
        requestBody?: {
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
      }
    >
  >;
};

describe("foundation settings API contract", () => {
  it("keeps terminal feature settings bound to the actual typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/terminal_feature"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps welcome guide operations bound to the actual typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/welcome_guide"],
      ["post", "/api/admin/config/welcome_guide/complete"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps proxy protocol and run mode prompt settings bound to typed routers", () => {
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

  it("keeps CAPTCHA, runtime mode, and Wake-on-LAN settings bound to typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/captcha"],
      ["post", "/api/admin/config/captcha"],
      ["post", "/api/admin/config/run_type"],
      ["get", "/api/admin/config/wol_feature"],
      ["post", "/api/admin/config/wol_feature"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps automatic firewall management bound to its typed router", () => {
    assert.equal(
      contract.paths["/api/admin/config/auto_manage_firewall"]?.post?.[
        "x-fn-knock-contract-source"
      ],
      "utoipa",
    );
  });

  it("keeps authentication mode operations bound to their typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/auth/mode"],
      ["post", "/api/admin/auth/mode/preview"],
      ["post", "/api/admin/auth/mode/switch"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps authentication account lifecycle bound to its typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/auth/accounts"],
      ["post", "/api/admin/auth/accounts"],
      ["patch", "/api/admin/auth/accounts/{id}"],
      ["delete", "/api/admin/auth/accounts/{id}"],
      ["post", "/api/admin/auth/accounts/{id}/password"],
      ["post", "/api/admin/auth/accounts/{id}/setup"],
      ["post", "/api/admin/auth/accounts/{id}/totp/setup"],
      ["post", "/api/admin/auth/accounts/{id}/totp/bind"],
      ["patch", "/api/admin/auth/accounts/{id}/access-scopes"],
      ["patch", "/api/admin/auth/accounts/{id}/subdomain-access"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps session lifecycle and mobility routes bound to its typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/sessions"],
      ["get", "/api/admin/sessions/{id}"],
      ["delete", "/api/admin/sessions/{id}"],
      ["patch", "/api/admin/sessions/{id}/comment"],
      ["get", "/api/admin/sessions/{id}/mobility"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps backup export, import, and automatic backup routes bound to typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/maintenance/backup/automatic"],
      ["put", "/api/admin/maintenance/backup/automatic"],
      ["get", "/api/admin/maintenance/backup/automatic/files"],
      ["get", "/api/admin/maintenance/backup/export"],
      ["get", "/api/admin/maintenance/backup/files"],
      ["post", "/api/admin/maintenance/backup/export/fnos"],
      ["post", "/api/admin/maintenance/backup/import"],
      ["post", "/api/admin/maintenance/backup/import/automatic"],
      ["post", "/api/admin/maintenance/backup/import/fnos"],
      ["post", "/api/admin/maintenance/data/clear"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps Host mapping routes bound to the executable typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/config/host_mappings"],
      ["post", "/api/admin/config/host_mappings"],
      ["get", "/api/admin/config/host_mapping_catalog"],
      ["post", "/api/admin/config/host_mapping_catalog"],
      ["post", "/api/admin/config/host_mappings/basic_auth_probe"],
      ["get", "/api/admin/config/host_mappings/bookmarks/export"],
      ["post", "/api/admin/config/host_mappings/metadata"],
      ["post", "/api/admin/config/host_mappings/refresh_titles"],
      ["get", "/api/admin/config/host_mappings/{host}/advanced_auth"],
      ["put", "/api/admin/config/host_mappings/{host}/advanced_auth"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps authentication credential settings bound to their typed router", () => {
    for (const method of ["get", "post"] as const) {
      assert.equal(
        contract.paths["/api/admin/config/auth_credential_settings"]?.[method]?.[
          "x-fn-knock-contract-source"
        ],
        "utoipa",
        method.toUpperCase(),
      );
    }
  });

  it("keeps admin panel session operations bound to their typed router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/panel/bootstrap"],
      ["post", "/api/admin/panel/password"],
      ["post", "/api/admin/panel/password/change"],
      ["post", "/api/admin/panel/login"],
      ["post", "/api/admin/panel/logout"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("keeps TOTP lifecycle and credential mutations bound to typed routers", () => {
    for (const [method, path] of [
      ["get", "/api/admin/totp/status"],
      ["post", "/api/admin/totp/setup"],
      ["post", "/api/admin/totp/bind"],
      ["delete", "/api/admin/totp/{id}"],
      ["patch", "/api/admin/totp/{id}/access-scopes"],
      ["patch", "/api/admin/totp/{id}/subdomain-access"],
      ["patch", "/api/admin/totp/{id}/comment"],
      ["delete", "/api/admin/passkeys/{id}"],
      ["get", "/api/admin/totp/credentials/export"],
      ["post", "/api/admin/totp/credentials/import"],
      ["get", "/api/admin/totp/{totp_id}/passkeys"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("separates partial captcha input from the complete sensitive output", () => {
    assert.equal(
      contract.paths["/api/admin/config/captcha"].post.requestBody?.content?.[
        "application/json"
      ]?.schema?.$ref,
      "#/components/schemas/CaptchaSettingsUpdateData",
    );
    assert.deepEqual(
      contract.components.schemas.CaptchaSettingsUpdateData.required ?? [],
      [],
    );
    assert.equal(
      contract.components.schemas.CaptchaPowData.properties?.base_max_number
        ?.minimum,
      10_000,
    );
    assert.equal(
      contract.components.schemas.CaptchaPowData.properties?.base_max_number
        ?.maximum,
      1_000_000,
    );
    assert.equal(
      contract.components.schemas.CaptchaPowData.properties?.base_max_number
        ?.multipleOf,
      10_000,
    );
    assert.equal(
      contract.components.schemas.CaptchaTurnstileData.properties?.secret_key
        ?.format,
      "password",
    );
  });

  it("preserves runtime modes, terminal bounds, and nullable completion time", () => {
    assert.deepEqual(
      contract.components.schemas.RunTypeUpdateData.properties?.run_type?.enum,
      [0, 1, 3],
    );
    assert.equal(
      contract.components.schemas.TerminalFeatureUpdateData.properties
        ?.resume_backend,
      undefined,
    );
    assert.equal(
      contract.components.schemas.TerminalFeatureData.properties?.max_sessions
        ?.maximum,
      12,
    );
    assert.ok(
      contract.components.schemas.WelcomeGuideData.required?.includes(
        "completed_at",
      ),
    );
  });

  it("derives frontend models and requests from the generated contract", () => {
    const types = readSource("../src/types/core.ts");
    const configApi =
      readSource("../src/lib/api/config.ts") +
      readSource("../src/lib/api/config-core-api.ts");

    assert.match(types, /\["RunTypeUpdateData"\]\["run_type"\]/u);
    assert.match(types, /\["WelcomeGuideData"\]/u);
    assert.match(types, /\["TerminalFeatureData"\]/u);
    assert.doesNotMatch(types, /export interface WelcomeGuideStatus/u);
    assert.doesNotMatch(types, /export interface TerminalFeatureConfig/u);
    assert.match(configApi, /\["CaptchaSettingsUpdateData"\]/u);
    assert.match(configApi, /\["AutoManageFirewallUpdateData"\]/u);
    assert.match(configApi, /\["TerminalFeatureUpdateData"\]/u);
  });
});
