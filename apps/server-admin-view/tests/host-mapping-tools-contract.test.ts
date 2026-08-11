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
    const types = readSource("../src/types.ts");
    const configApi = readSource("../src/lib/api/config.ts");
    const advancedAuthView = readSource(
      "../src/views/subdomain-proxy/SubdomainAdvancedAuth.vue",
    );
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
    assert.match(advancedAuthForm, /name: ""/u);
    assert.match(advancedAuthForm, /condition\.selections \?\? \[\]/u);
    assert.match(advancedAuthView, /cloneAdvancedAuthConfig/u);
  });
});
