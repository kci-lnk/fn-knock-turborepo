import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  properties?: Record<
    string,
    {
      enum?: string[];
      maximum?: number;
      minimum?: number;
      pattern?: string;
    }
  >;
  required?: string[];
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<
    string,
    Record<string, { "x-fn-knock-contract-source"?: string }>
  >;
};

describe("proxy and subdomain routing API contract", () => {
  it("keeps proxy, stream, and subdomain operations typed", () => {
    for (const [method, path] of [
      ["post", "/api/admin/config/proxy_mappings"],
      ["get", "/api/admin/config/stream_mappings"],
      ["post", "/api/admin/config/stream_mappings"],
      ["get", "/api/admin/config/subdomain_mode"],
      ["post", "/api/admin/config/subdomain_mode"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves stream protocol and port boundaries", () => {
    for (const schema of ["StreamMappingData", "StreamMappingInputData"]) {
      const properties = contract.components.schemas[schema].properties;
      assert.deepEqual(properties?.protocol?.enum, ["tcp", "udp"]);
      assert.equal(properties?.listen_port?.minimum, 1);
      assert.equal(properties?.listen_port?.maximum, 65_535);
    }
  });

  it("models normalized subdomain fields and write-only selection feedback", () => {
    const data = contract.components.schemas.SubdomainModeData;
    for (const field of [
      "public_http_port",
      "public_https_port",
      "passkey_rp_id",
    ]) {
      assert.ok(data.required?.includes(field), field);
    }
    assert.deepEqual(data.properties?.default_access_mode?.enum, [
      "login_first",
      "strict_whitelist",
    ]);
    assert.deepEqual(data.properties?.passkey_rp_mode?.enum, [
      "auth_host",
      "parent_domain",
    ]);
    assert.ok(
      contract.components.schemas.SubdomainModeResponseData.required?.includes(
        "ssl_auto_selection",
      ),
    );
  });

  it("derives stream and subdomain frontend boundaries from OpenAPI", () => {
    const types = readSource("../src/types.ts");
    const configApi = readSource("../src/lib/api/config.ts");

    assert.match(types, /\["StreamMappingData"\]/u);
    assert.match(types, /\["SubdomainModeData"\]/u);
    assert.match(configApi, /satisfies ProxyMappingsUpdate/u);
    assert.match(configApi, /satisfies StreamMappingsUpdate/u);
    assert.match(configApi, /config: SubdomainModeUpdate/u);
    assert.match(configApi, /Promise<SubdomainModeResponse>/u);
  });
});
