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
  security?: Array<Record<string, string[]>>;
  summary?: string;
  tags?: string[];
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: {
    schemas: Record<string, Schema>;
    securitySchemes?: Record<string, { scheme?: string; type?: string }>;
  };
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
      ["get", "/api/admin/ssl/external-bindings"],
      ["post", "/api/admin/ssl/external-bindings"],
      ["patch", "/api/admin/ssl/external-bindings/{id}"],
      ["post", "/api/admin/ssl/external-bindings/{id}/rotate-token"],
      ["delete", "/api/admin/ssl/external-bindings/{id}"],
      ["put", "/api/integrations/certificates/{binding_id}"],
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
    assert.match(sslTag?.description ?? "", /管理会话/u);
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
    assert.equal(sslOperations.length, 26);
    for (const operation of sslOperations) {
      assert.match(operation.summary ?? "", /[\u4e00-\u9fff]/u);
      assert.match(operation.description ?? "", /[\u4e00-\u9fff]/u);
      assert.match(
        operation.responses?.["200"]?.description ?? "",
        /[\u4e00-\u9fff]/u,
      );
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

  it("keeps external deployment credentials scoped and write-only", () => {
    const binding = contract.components.schemas.ExternalCertificateBindingData;
    const credential =
      contract.components.schemas.ExternalCertificateBindingCredentialData;
    const deployment =
      contract.components.schemas.ExternalCertificateDeployBodyData;
    const deployOperation =
      contract.paths["/api/integrations/certificates/{binding_id}"].put;

    assert.equal(credential.properties?.token?.writeOnly, true);
    assert.equal(deployment.properties?.key?.writeOnly, true);
    assert.equal(binding.properties?.token, undefined);
    assert.ok(binding.required?.includes("certificate_id"));
    assert.ok(binding.required?.includes("deploy_port"));
    assert.match(
      binding.properties?.deploy_port?.description ?? "",
      /BACKEND_PORT/u,
    );
    assert.ok(binding.required?.includes("setup_kind"));
    assert.deepEqual(binding.properties?.provider?.enum, [
      "certd",
      "acme_sh",
      "lego",
      "certbot",
    ]);
    assert.deepEqual(binding.properties?.setup_kind?.enum, [
      "webhook",
      "deploy_hook",
    ]);
    assert.ok(binding.properties?.request_body_template);
    assert.ok(binding.properties?.script_template);
    assert.ok(binding.properties?.usage_instructions);
    assert.deepEqual(deployOperation.security, [
      { certificateDeploymentToken: [] },
    ]);
    assert.equal(
      contract.components.securitySchemes?.certificateDeploymentToken?.type,
      "http",
    );
    assert.equal(
      contract.components.securitySchemes?.certificateDeploymentToken?.scheme,
      "bearer",
    );
    for (const status of ["400", "401", "404", "409", "413", "500", "502"]) {
      assert.ok(deployOperation.responses?.[status], status);
    }
    assert.doesNotMatch(
      deployOperation.description ?? "",
      /必须通过 HTTPS|HTTPS only|must (?:use|be called over) HTTPS/iu,
      "the receiving endpoint must remain usable over either HTTP or HTTPS",
    );
    assert.equal(
      deployOperation.requestBody?.content?.["application/json"]?.examples,
      undefined,
      "external deployment must not publish a fake PEM or private-key example",
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
    assert.ok(contract.paths["/api/admin/ssl/ca/init"].post.responses?.["500"]);
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
      removeHosts.responses?.["200"]?.content?.[
        "application/json"
      ]?.schema?.required?.includes("data") ?? false,
      false,
    );

    assert.ok(
      contract.paths["/api/admin/ssl/cert.pem"].get.responses?.["200"]
        ?.content?.["application/x-pem-file"],
    );
    assert.ok(
      contract.paths["/api/admin/ssl/ca/server-cert.zip"].get.responses?.["200"]
        ?.content?.["application/zip"],
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
      "ExternalCertificateBindingData",
      "ExternalCertificateBindingCredentialData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_ssl_shared_files_content/u);
    assert.match(api, /satisfies SslCaHostBody/u);
    assert.match(api, /satisfies SslCaHostsDeleteBody/u);
    assert.match(api, /satisfies SslDeploymentModeBody/u);
    assert.match(api, /satisfies SslActivateBody/u);
    assert.match(api, /getExternalCertificateBindings/u);
    assert.match(api, /satisfies ExternalCertificateBindingCreateBody/u);
    assert.match(api, /rotateExternalCertificateBindingToken/u);
  });

  it("encapsulates external deployment state outside the SSL composition root", () => {
    const root = readSource("../src/views/ssl-settings/CertConfig.vue");
    const card = readSource(
      "../src/views/ssl-settings/ExternalCertificateDeploymentCard.vue",
    );
    const controller = readSource(
      "../src/views/ssl-settings/useExternalCertificateBindings.ts",
    );
    const zhCN = readSource(
      "../../../packages/i18n/src/messages/admin/zh-CN.ts",
    );

    assert.match(root, /<ExternalCertificateDeploymentCard\s*\/>/u);
    assert.match(card, /useExternalCertificateBindings/u);
    assert.match(card, /SelectItem/u);
    assert.match(card, /providerName\(binding\.provider\)/u);
    assert.doesNotMatch(card, /binding\.certificate_id/u);
    assert.doesNotMatch(card, /ConfigAPI\./u);
    assert.match(controller, /ConfigAPI\.createExternalCertificateBinding/u);
    for (const provider of ["certd", "acme_sh", "lego", "certbot"]) {
      assert.match(controller, new RegExp(`${provider}:`, "u"));
    }
    assert.match(controller, /__FN_KNOCK_DEPLOY_URL__/u);
    assert.match(controller, /__FN_KNOCK_DEPLOY_TOKEN__/u);
    assert.match(controller, /binding\.deploy_port/u);
    assert.match(controller, /http:\/\/127\.0\.0\.1:\$\{binding\.deploy_port\}/u);
    assert.doesNotMatch(controller, /window\.location\.(?:hostname|origin)/u);
    assert.match(
      controller,
      /ConfigAPI\.rotateExternalCertificateBindingToken/u,
    );
    assert.match(controller, /function clearCredential\(\)/u);
    assert.match(controller, /credential\.value = null/u);
    assert.match(card, /collapseAndClear\(collapse\)/u);
    assert.doesNotMatch(controller, /localStorage|sessionStorage/u);
    assert.match(zhCN, /externalAutomationTitle: "接收外部证书"/u);
    assert.match(zhCN, /这里负责接收，不负责申请证书/u);
    assert.match(zhCN, /只在本机 127\.0\.0\.1:\{port\} 接收证书/u);
    assert.match(zhCN, /反向代理/u);
  });
});
