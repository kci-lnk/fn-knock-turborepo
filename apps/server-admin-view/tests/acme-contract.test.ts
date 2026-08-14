import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  enum?: string[];
  format?: string;
  minItems?: number;
  minLength?: number;
  oneOf?: Schema[];
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  type?: string | string[];
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{
    in?: string;
    name?: string;
    schema?: Schema;
  }>;
  responses?: Record<
    string,
    {
      content?: Record<string, { schema?: Schema }>;
      headers?: Record<string, { schema?: Schema }>;
    }
  >;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("ACME API contract", () => {
  it("keeps every ACME operation on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["delete", "/api/admin/acme"],
      ["get", "/api/admin/acme/status"],
      ["get", "/api/admin/acme/resource/status"],
      ["post", "/api/admin/acme/resource/initialize"],
      ["post", "/api/admin/acme/resource/cancel"],
      ["delete", "/api/admin/acme/resource"],
      ["get", "/api/admin/acme/overview"],
      ["get", "/api/admin/acme/dns-providers"],
      ["get", "/api/admin/acme/subdomain-recommendation"],
      ["post", "/api/admin/acme/init"],
      ["post", "/api/admin/acme/client-settings"],
      ["get", "/api/admin/acme/config"],
      ["post", "/api/admin/acme/config"],
      ["get", "/api/admin/acme/applications"],
      ["post", "/api/admin/acme/applications"],
      ["get", "/api/admin/acme/applications/{id}"],
      ["patch", "/api/admin/acme/applications/{id}"],
      ["delete", "/api/admin/acme/applications/{id}"],
      ["delete", "/api/admin/acme/applications/{id}/certificate"],
      ["post", "/api/admin/acme/applications/{id}/library/sync"],
      ["post", "/api/admin/acme/applications/{id}/deploy"],
      ["post", "/api/admin/acme/applications/{id}/request"],
      ["post", "/api/admin/acme/request"],
      ["post", "/api/admin/acme/jobs/active/stop"],
      ["get", "/api/admin/acme/jobs/{id}"],
      ["get", "/api/admin/acme/jobs/{id}/logs"],
      ["get", "/api/admin/acme/jobs/{id}/poll"],
      ["get", "/api/admin/acme/certs/{domain}"],
      ["delete", "/api/admin/acme/certs/{domain}"],
      ["get", "/api/admin/acme/certs/{domain}/download"],
      ["post", "/api/admin/acme/certs/{domain}/deploy"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("documents lifecycle enums, nullable results, and secret write boundaries", () => {
    const status = contract.components.schemas.AcmeStatusData;
    const job = contract.components.schemas.AcmeJobData;
    const poll = contract.components.schemas.AcmeJobPollData;
    const application = contract.components.schemas.AcmeApplicationData;
    const applicationBody = contract.components.schemas.AcmeApplicationBodyData;
    assert.deepEqual(status.properties?.status?.enum, [
      "uninstalled",
      "installing",
      "installed",
      "error",
    ]);
    assert.ok(status.required?.includes("acmeCert"));
    assert.deepEqual(job.properties?.status?.enum, [
      "queued",
      "running",
      "succeeded",
      "failed",
      "stopped",
    ]);
    assert.ok(poll.required?.includes("analysis"));
    assert.equal(applicationBody.properties?.domains?.minItems, 1);
    assert.equal(applicationBody.properties?.credentials?.writeOnly, true);
    assert.equal(application.properties?.credentials?.writeOnly, undefined);
  });

  it("preserves permissive poll parsing, opaque IDs, and ZIP downloads", () => {
    const poll =
      contract.paths["/api/admin/acme/jobs/{id}/poll"].get.parameters ?? [];
    const limit = poll.find((parameter) => parameter.name === "limit")?.schema;
    const order = poll.find((parameter) => parameter.name === "order")?.schema;
    const id = poll.find(
      (parameter) => parameter.name === "id" && parameter.in === "path",
    )?.schema;
    assert.equal(limit?.oneOf?.[1]?.type, "string");
    assert.equal(limit?.oneOf?.[1]?.pattern, undefined);
    assert.equal(order?.type, "string");
    assert.equal(order?.enum, undefined);
    assert.equal(id?.minLength, 1);
    assert.equal(id?.format, undefined);

    const download =
      contract.paths["/api/admin/acme/certs/{domain}/download"].get.responses?.[
        "200"
      ];
    assert.equal(
      download?.content?.["application/zip"]?.schema?.format,
      "binary",
    );
    assert.equal(
      download?.headers?.["Content-Disposition"]?.schema?.type,
      "string",
    );
    assert.equal(
      contract.paths["/api/admin/acme/certs/{domain}/download"].get.responses?.[
        "204"
      ],
      undefined,
    );
  });

  it("derives frontend models, writes, and polling queries from OpenAPI", () => {
    const api = readSource("../src/lib/api/acme.ts");
    for (const schema of [
      "AcmeStatusData",
      "AcmeApplicationData",
      "AcmeOverviewData",
      "AcmeJobData",
      "AcmeApplicationBodyData",
      "AcmeLegacyRequestBodyData",
      "AcmeJobPollData",
    ]) {
      assert.match(api, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_acme_jobs__id__poll/u);
    assert.match(api, /satisfies AcmeApplicationPayload/u);
    assert.match(api, /satisfies AcmeLegacyRequestBody/u);
    assert.match(api, /satisfies AcmePollQuery/u);
  });
});

describe("ACME stuck-job recovery UI", () => {
  it("keeps DNS configuration reachable while a certificate job owns the lock", () => {
    const table = readSource(
      "../src/views/ssl-settings/AcmeCertificateApplicationsTable.vue",
    );
    const header = readSource(
      "../src/views/ssl-settings/AcmeCertificateHeader.vue",
    );
    const dialog = readSource(
      "../src/views/ssl-settings/AcmeApplicationDialog.vue",
    );

    assert.doesNotMatch(table, /absolute inset-0/u);
    assert.match(table, /isConfigurationEditBlocked\(\)/u);
    assert.doesNotMatch(
      header,
      /!isAcmeInstalled\s*\|\|\s*isTableLocked\s*\|\|/u,
    );
    assert.match(dialog, /props\.runtimeLocked/u);
  });

  it("surfaces incomplete stop results instead of reporting no active job", () => {
    const polling = readSource(
      "../src/views/ssl-settings/useAcmeJobPolling.ts",
    );

    assert.match(polling, /result\.processResult\.remainingPids/u);
    assert.match(polling, /result\.processResult\.errors/u);
    assert.match(polling, /admin\.acmeCert\.stopJobFailed/u);
  });
});
