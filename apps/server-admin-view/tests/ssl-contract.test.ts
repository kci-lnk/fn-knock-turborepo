import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  description?: string;
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
  description?: string;
  parameters?: Array<{
    in?: string;
    name?: string;
    required?: boolean;
    schema?: Schema;
  }>;
  requestBody?: {
    required?: boolean;
    content?: Record<
      string,
      { examples?: Record<string, { value?: unknown }>; schema?: Schema }
    >;
  };
  responses?: Record<
    string,
    {
      content?: Record<
        string,
        { examples?: Record<string, { value?: unknown }>; schema?: Schema }
      >;
      description?: string;
    }
  >;
  summary?: string;
  tags?: string[];
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
  tags?: Array<{ description?: string; name?: string }>;
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
    assert.match(save.properties?.key?.description ?? "", /仅写入/u);
    assert.match(save.properties?.cert?.description ?? "", /PEM/u);
    assert.equal(status.properties?.key, undefined);
    assert.match(status.description ?? "", /状态快照/u);
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

  it("documents every SSL operation in Chinese without publishing private keys", () => {
    const sslTag = contract.tags?.find((tag) => tag.name === "ssl");
    assert.match(sslTag?.description ?? "", /证书库/u);
    assert.match(sslTag?.description ?? "", /同源管理面板/u);
    assert.doesNotMatch(
      sslTag?.description ?? "",
      /\\n/u,
      "tag descriptions must use real line breaks instead of literal escape sequences",
    );

    const sslOperations = Object.values(contract.paths).flatMap((pathItem) =>
      Object.values(pathItem).filter(
        (operation) =>
          operation["x-fn-knock-contract-source"] === "utoipa" &&
          operation.tags?.includes("ssl"),
      ),
    );
    assert.equal(sslOperations.length, 20);
    for (const operation of sslOperations) {
      assert.match(operation.summary ?? "", /[\u4e00-\u9fff]/u);
      assert.match(operation.description ?? "", /[\u4e00-\u9fff]/u);
      assert.match(operation.responses?.["200"]?.description ?? "", /[\u4e00-\u9fff]/u);
    }

    const save = contract.paths["/api/admin/ssl/certificates"].post;
    assert.equal(
      save.requestBody?.content?.["application/json"]?.examples,
      undefined,
      "certificate import must not publish a fake PEM or private-key example",
    );
    assert.doesNotMatch(
      JSON.stringify(contract),
      /-----BEGIN(?: [A-Z]+)? PRIVATE KEY-----/u,
    );
  });

  it("documents SSL examples and operation-specific errors", () => {
    const activate = contract.paths["/api/admin/ssl/activate"].post;
    const deployment = contract.paths["/api/admin/ssl/deployment-mode"].post;
    const caHosts = contract.paths["/api/admin/ssl/ca/hosts"];
    assert.ok(
      activate.requestBody?.content?.["application/json"]?.examples?.activate,
    );
    assert.ok(
      deployment.requestBody?.content?.["application/json"]?.examples?.multiSni,
    );
    assert.ok(
      caHosts.post?.requestBody?.content?.["application/json"]?.examples
        ?.addHost,
    );
    assert.ok(
      caHosts.delete?.requestBody?.content?.["application/json"]?.examples
        ?.clearAll,
    );

    assert.ok(
      contract.paths["/api/admin/ssl/certificates"].post.responses?.["400"],
    );
    assert.ok(
      contract.paths["/api/admin/ssl/activate"].post.responses?.["404"],
    );
    assert.ok(
      contract.paths["/api/admin/ssl/shared-files/content"].get.responses?.[
        "403"
      ],
    );
    assert.ok(
      contract.paths["/api/admin/ssl/ca/init"].post.responses?.["500"],
    );
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
    const types = readSource("../src/types/core.ts");
    const api = readSource("../src/lib/api/config-proxy-api.ts");
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
