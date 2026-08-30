import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type PropertySchema = {
  maximum?: number;
  minimum?: number;
  maxItems?: number;
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  responses?: Record<
    string,
    { content?: Record<string, Record<string, unknown>> }
  >;
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

describe("host mapping utility API contract", () => {
  it("keeps metadata, bookmarks, probes, refresh, and advanced auth typed", () => {
    for (const [method, path] of [
      ["post", "/api/admin/config/host_mappings/basic_auth_probe"],
      ["post", "/api/admin/config/host_mappings/static_path_probe"],
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

  it("documents static serving and its non-authoritative path probe", () => {
    const staticServe = contract.components.schemas.StaticServeConfigData;
    const probeBody = contract.components.schemas.StaticPathProbeBodyData;
    const probeResult = contract.components.schemas.StaticPathProbeResultData;
    assert.equal(staticServe.properties?.index_files?.maxItems, 16);
    assert.deepEqual(probeBody.required, ["target_type", "path"]);
    for (const field of [
      "target_type",
      "normalized_path",
      "exists",
      "readable",
      "actual_type",
      "error_code",
    ]) {
      assert.ok(probeResult.required?.includes(field), field);
    }
  });

  it("shows gateway-local Linux, Windows, and read-only Docker path examples", () => {
    const field = readSource(
      "../src/views/subdomain-proxy/SubdomainMappingStaticTargetField.vue",
    );
    assert.match(field, /staticServe\.pathHint/u);
    assert.match(field, /v-if="configStore\.isDockerDeployment"/u);
    assert.match(field, /staticServe\.pathDockerHint/u);
    assert.doesNotMatch(
      field,
      /probeHostMappingStaticPath|staticServe\.probe/u,
    );

    for (const locale of ["en", "ja-JP", "ko-KR", "zh-CN", "zh-Hant"]) {
      const source = readSource(
        `../../../packages/i18n/src/messages/admin/${locale}.ts`,
      );
      assert.ok(source.includes("/srv/site"), `${locale} Linux example`);
      assert.ok(
        source.includes("C:\\\\Sites\\\\docs"),
        `${locale} Windows example`,
      );
      assert.ok(
        source.includes("/host/docs:/srv/docs:ro"),
        `${locale} read-only Docker example`,
      );
    }
  });

  it("keeps Basic Auth secrets write-only and models nullable probe status", () => {
    assert.equal(
      contract.components.schemas.HostMappingBasicAuthInputData.properties
        ?.password?.writeOnly,
      true,
    );
    const probe = contract.components.schemas.HostMappingBasicAuthProbeData;
    assert.ok(probe.required?.includes("httpStatus"));
    assert.equal(probe.required?.includes("error") ?? false, false);
    assert.equal(probe.properties?.httpStatus?.minimum, 100);
    assert.equal(probe.properties?.httpStatus?.maximum, 599);
  });

  it("documents bookmark HTML and advanced authentication limits", () => {
    const bookmarkContent =
      contract.paths["/api/admin/config/host_mappings/bookmarks/export"]?.get
        ?.responses?.["200"]?.content;
    assert.ok(bookmarkContent?.["text/html"]);
    assert.equal(bookmarkContent?.["application/json"], undefined);

    const advanced =
      contract.components.schemas.AdvancedAuthConfigInputData.properties;
    assert.equal(advanced?.idle_ttl_seconds?.minimum, 300);
    assert.equal(advanced?.idle_ttl_seconds?.maximum, 2_592_000);
    assert.equal(advanced?.max_lifetime_seconds?.maximum, 31_536_000);
    assert.equal(advanced?.groups?.maxItems, 16);
    assert.ok(
      contract.components.schemas.AdvancedAuthDetailsData.required?.includes(
        "revision",
      ),
    );
  });

  it("derives frontend models and normalizes the advanced-auth form boundary", () => {
    const types = readSource("../src/types/core.ts");
    const configApi = readSource("../src/lib/api/config-proxy-api.ts");
    const staticConfigApi = readSource(
      "../src/lib/api/config-host-mapping-static-api.ts",
    );
    const advancedAuthView = readSource(
      "../src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
    );
    const advancedAuthEditor = readSource(
      "../src/views/subdomain-proxy/SubdomainAdvancedAuthEditor.vue",
    );
    const advancedAuthPage = readSource(
      "../src/views/subdomain-proxy/useSubdomainAdvancedAuthPage.ts",
    );
    const advancedAuthRuleGroups = [
      readSource("../src/views/subdomain-proxy/AdvancedAuthRuleGroups.vue"),
      readSource("../src/views/subdomain-proxy/AdvancedAuthRuleGroupCard.vue"),
      readSource(
        "../src/views/subdomain-proxy/AdvancedAuthConditionEditor.vue",
      ),
    ].join("\n");
    const advancedAuthForm = readSource(
      "../src/views/subdomain-proxy/advanced-auth-form.ts",
    );

    for (const schema of [
      "AdvancedAuthConditionData",
      "AdvancedAuthRuleGroupData",
      "AdvancedAuthConfigData",
      "HostMappingRefreshSummaryData",
      "HostMappingMetadataData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(configApi, /satisfies AdvancedAuthUpdate/u);
    assert.match(configApi, /satisfies HostMappingMetadataBody/u);
    assert.match(configApi, /satisfies HostMappingBasicAuthProbeBody/u);
    assert.match(staticConfigApi, /StaticPathProbeBodyData/u);
    assert.match(staticConfigApi, /satisfies StaticPathProbeBody/u);
    assert.match(staticConfigApi, /StaticPathProbeResultData/u);
    assert.match(advancedAuthForm, /name: ""/u);
    assert.match(advancedAuthForm, /condition\.selections \?\? \[\]/u);
    assert.match(advancedAuthView, /useSubdomainAdvancedAuthPage/u);
    assert.match(advancedAuthView, /SubdomainAdvancedAuthEditor/u);
    assert.doesNotMatch(advancedAuthView, /ConfigAPI|cloneAdvancedAuthConfig/u);
    assert.match(advancedAuthPage, /cloneAdvancedAuthConfig/u);
    assert.match(advancedAuthEditor, /AdvancedAuthRuleGroups/u);
    assert.match(advancedAuthEditor, /AdvancedAuthDurationSettings/u);
    assert.match(
      advancedAuthRuleGroups,
      /advanced-auth-target-\$\{condition\.id\}/u,
    );
    assert.match(
      advancedAuthRuleGroups,
      /advanced-auth-operator-\$\{condition\.id\}/u,
    );
    assert.match(
      advancedAuthRuleGroups,
      /advanced-auth-value-\$\{condition\.id\}/u,
    );
  });
});
