import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  enum?: string[];
  format?: string;
  items?: Schema;
  minLength?: number;
  properties?: Record<string, Schema>;
  required?: string[];
  type?: string;
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{
    in?: string;
    name?: string;
    required?: boolean;
    schema?: Schema;
  }>;
  requestBody?: {
    required?: boolean;
    content?: Record<string, { schema?: Schema }>;
  };
  responses?: Record<
    string,
    { content?: Record<string, { schema?: Schema }> }
  >;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("SSL certificate API contract", () => {
  it("keeps every SSL and local CA operation on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/ssl/status"],
      ["get", "/api/admin/ssl/shared-files"],
      ["get", "/api/admin/ssl/shared-files/content"],
      ["get", "/api/admin/ssl/cert.pem"],
      ["get", "/api/admin/ssl/cert.zip"],
      ["get", "/api/admin/ssl/ca/status"],
      ["post", "/api/admin/ssl/ca/init"],
      ["delete", "/api/admin/ssl/ca"],
      ["get", "/api/admin/ssl/ca/cert.pem"],
      ["get", "/api/admin/ssl/ca/server-cert.zip"],
      ["get", "/api/admin/ssl/ca/hosts"],
      ["post", "/api/admin/ssl/ca/hosts"],
      ["delete", "/api/admin/ssl/ca/hosts"],
      ["post", "/api/admin/ssl/ca/issue"],
      ["post", "/api/admin/ssl/certificates"],
      ["delete", "/api/admin/ssl/certificates"],
      ["delete", "/api/admin/ssl/certificates/{id}"],
      ["post", "/api/admin/ssl/activate"],
      ["post", "/api/admin/ssl/deployment-mode"],
      ["delete", "/api/admin/ssl"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("separates private certificate input from public status output", () => {
    const save = contract.components.schemas.SslCertificateSaveBodyData;
    const status = contract.components.schemas.SslStatusData;
    assert.ok(save.required?.includes("cert"));
    assert.ok(save.required?.includes("key"));
    assert.equal(save.properties?.key?.writeOnly, true);
    assert.equal(status.properties?.key, undefined);
    assert.deepEqual(status.properties?.deploymentMode?.enum, [
      "single_active",
      "multi_sni",
    ]);
    for (const field of [
      "subdomain_coverage",
      "library_coverage",
      "gateway_status",
    ]) {
      assert.ok(status.required?.includes(field), field);
    }
  });

  it("preserves shared-file, CA deletion, and attachment compatibility", () => {
    const shared = contract.paths["/api/admin/ssl/shared-files/content"].get;
    const path = shared.parameters?.find(
      (parameter) => parameter.name === "path" && parameter.in === "query",
    );
    assert.equal(path?.required, true);
    assert.equal(path?.schema?.minLength, 1);

    const removeHosts = contract.paths["/api/admin/ssl/ca/hosts"].delete;
    assert.equal(removeHosts.requestBody?.required, false);
    assert.equal(
      removeHosts.responses?.["200"]?.content?.["application/json"]?.schema
        ?.properties?.data?.items?.type,
      "string",
    );
    assert.equal(
      removeHosts.responses?.["200"]?.content?.["application/json"]?.schema
        ?.required?.includes("data") ?? false,
      false,
    );

    assert.ok(
      contract.paths["/api/admin/ssl/cert.pem"].get.responses?.["200"]
        ?.content?.["application/x-pem-file"],
    );
    assert.ok(
      contract.paths["/api/admin/ssl/ca/server-cert.zip"].get.responses?.[
        "200"
      ]?.content?.["application/zip"],
    );
  });

  it("derives frontend SSL models, requests, and queries from OpenAPI", () => {
    const types = readSource("../src/types.ts");
    const api = readSource("../src/lib/api/config.ts");
    for (const schema of [
      "SslCertificateSaveBodyData",
      "SslCertificateInfoData",
      "SslCertificateSummaryData",
      "SslSubdomainCoverageData",
      "SslStatusData",
      "SslSharedFilesData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_ssl_shared_files_content/u);
    assert.match(api, /satisfies SslCaHostBody/u);
    assert.match(api, /satisfies SslCaHostsDeleteBody/u);
    assert.match(api, /satisfies SslDeploymentModeBody/u);
    assert.match(api, /satisfies SslActivateBody/u);
  });
});
