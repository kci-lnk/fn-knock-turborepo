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
          { enum?: string[]; maxItems?: number }
        >;
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
          schema?: { enum?: string[] };
        }>;
        responses?: Record<
          string,
          { content?: Record<string, { schema?: { $ref?: string } }> }
        >;
      }
    >
  >;
};

describe("CIDR and IP location API contract", () => {
  it("keeps all location service operations typed", () => {
    for (const [method, path] of [
      ["get", "/api/admin/cidr/capabilities"],
      ["get", "/api/admin/cidr/provinces"],
      ["get", "/api/admin/cidr/cities"],
      ["get", "/api/admin/cidr/selector"],
      ["get", "/api/admin/cidr/cidrs"],
      ["post", "/api/admin/ip-location/batch"],
      ["get", "/api/admin/config/ip_location_api"],
      ["post", "/api/admin/config/ip_location_api"],
      ["post", "/api/admin/config/ip_location_api/test-ip-lookup"],
      ["post", "/api/admin/config/ip_location_api/test-cidr"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves query, batch, and direct-response boundaries", () => {
    const cities = contract.paths["/api/admin/cidr/cities"].get.parameters;
    assert.equal(
      cities?.find((parameter) => parameter.name === "province")?.required,
      true,
    );
    const lookup = contract.paths["/api/admin/cidr/cidrs"].get.parameters;
    assert.deepEqual(
      lookup?.find((parameter) => parameter.name === "operator")?.schema
        ?.enum,
      ["电信", "联通", "移动"],
    );
    assert.equal(
      contract.components.schemas.IpLocationBatchBodyData.properties?.ips
        .maxItems,
      20,
    );
    assert.ok(
      contract.components.schemas.IpLocationSnapshotData.properties?.result,
    );

    const testResponse =
      contract.paths["/api/admin/config/ip_location_api/test-ip-lookup"].post
        .responses?.["200"]?.content?.["application/json"]?.schema;
    assert.equal(
      testResponse?.$ref,
      "#/components/schemas/IpLocationConnectionTestData",
    );
  });

  it("derives frontend location models and requests from generated types", () => {
    const cidrTypes = readSource("../src/types/cidr.ts");
    const types = readSource("../src/types.ts");
    const gatewayApi = readSource("../src/lib/api/gateway.ts");
    const configApi = readSource("../src/lib/api/config.ts");
    const settings = readSource(
      "../src/views/system-settings/ip-location/useIpLocationSettings.ts",
    );

    assert.match(cidrTypes, /CidrCapabilitiesData/u);
    assert.doesNotMatch(cidrTypes, /interface CidrCapabilitiesPayload/u);
    assert.match(types, /IpLocationSnapshotData/u);
    assert.doesNotMatch(types, /interface IpLocationSnapshot/u);
    assert.match(gatewayApi, /satisfies IpLocationBatchBody/u);
    assert.match(gatewayApi, /satisfies CidrLookupQuery/u);
    assert.match(configApi, /IpLocationApiConfigData/u);
    assert.match(configApi, /satisfies IpLocationTestUrlBody/u);
    assert.match(settings, /result\.message \|\|\s+result\.msg/u);
    assert.match(settings, /ipLookupUnavailable/u);
    assert.match(settings, /cidrUnavailable/u);
  });
});
