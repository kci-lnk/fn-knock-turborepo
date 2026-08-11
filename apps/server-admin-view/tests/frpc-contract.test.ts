import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type Schema = {
  const?: string;
  enum?: string[];
  maximum?: number;
  minimum?: number;
  oneOf?: Schema[];
  pattern?: string;
  properties?: Record<string, Schema>;
  required?: string[];
  writeOnly?: boolean;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Array<{
    in?: string;
    name?: string;
    schema?: Schema;
  }>;
};

const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  components: { schemas: Record<string, Schema> };
  paths: Record<string, Record<string, Operation>>;
};

describe("FRPC API contract", () => {
  it("keeps legacy and multi-instance FRPC operations on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/frpc/status"],
      ["get", "/api/admin/frpc/overview"],
      ["get", "/api/admin/frpc/web-status"],
      ["get", "/api/admin/frpc/config"],
      ["post", "/api/admin/frpc/config"],
      ["post", "/api/admin/frpc/start"],
      ["post", "/api/admin/frpc/stop"],
      ["get", "/api/admin/frpc/logs"],
      ["delete", "/api/admin/frpc/logs"],
      ["get", "/api/admin/frpc/poll"],
      ["get", "/api/admin/frpc/instances"],
      ["post", "/api/admin/frpc/instances"],
      ["post", "/api/admin/frpc/instances/draft"],
      ["get", "/api/admin/frpc/instances/{id}"],
      ["put", "/api/admin/frpc/instances/{id}"],
      ["delete", "/api/admin/frpc/instances/{id}"],
      ["post", "/api/admin/frpc/instances/{id}/start"],
      ["post", "/api/admin/frpc/instances/{id}/stop"],
      ["post", "/api/admin/frpc/instances/{id}/restart"],
      ["get", "/api/admin/frpc/instances/{id}/logs"],
      ["delete", "/api/admin/frpc/instances/{id}/logs"],
      ["get", "/api/admin/frpc/instances/{id}/poll"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves platform, instance-limit, and nullable runtime fields", () => {
    const status = contract.components.schemas.FrpcStatusData;
    const overview = contract.components.schemas.FrpcInstancesOverviewData;
    const instance = contract.components.schemas.FrpcInstanceStatusData;

    assert.deepEqual(status.properties?.platform?.enum, [
      "darwin-amd64",
      "darwin-arm64",
      "linux-amd64",
      "linux-arm64",
      "linux-arm",
      "unsupported",
    ]);
    assert.equal(overview.properties?.primaryInstanceId?.const, "primary");
    assert.equal(overview.properties?.extraCount?.maximum, 20);
    for (const field of [
      "pid",
      "startedAt",
      "stoppedAt",
      "lastExitCode",
      "lastMessage",
    ]) {
      assert.ok(instance.required?.includes(field), field);
    }
  });

  it("marks write payloads sensitive without hiding authenticated reads", () => {
    const config = contract.components.schemas.FrpcConfigData;
    const update = contract.components.schemas.FrpcConfigUpdateData;
    const instanceBody = contract.components.schemas.FrpcInstanceBodyData;
    assert.equal(config.properties?.content?.writeOnly, undefined);
    assert.equal(update.properties?.content?.writeOnly, true);
    assert.equal(instanceBody.properties?.content?.writeOnly, true);
    assert.deepEqual(instanceBody.required ?? [], []);
  });

  it("documents legacy log parsing and safe instance identifiers", () => {
    const logParameters =
      contract.paths["/api/admin/frpc/instances/{id}/logs"].get.parameters ??
      [];
    const pollParameters =
      contract.paths["/api/admin/frpc/instances/{id}/poll"].get.parameters ??
      [];
    const limit = logParameters.find(
      (parameter) => parameter.name === "limit",
    )?.schema;
    const cursor = pollParameters.find(
      (parameter) => parameter.name === "cursor",
    )?.schema;
    const id = pollParameters.find(
      (parameter) => parameter.name === "id" && parameter.in === "path",
    )?.schema;
    assert.equal(limit?.oneOf?.[1]?.pattern, "^\\s*[+-]?\\d+");
    assert.equal(cursor?.oneOf?.[0]?.minimum, 0);
    assert.equal(cursor?.oneOf?.[1]?.pattern, "^[0-9]+$");
    assert.equal(id?.pattern, "^[A-Za-z0-9-]{1,80}$");
  });

  it("derives frontend FRPC models, requests, and queries from OpenAPI", () => {
    const api = readSource("../src/lib/api/tunnel.ts");
    for (const schema of [
      "FrpcInstanceSummaryData",
      "FrpcInstanceStatusData",
      "FrpcInstancesOverviewData",
      "FrpcPrimaryStatusData",
      "FrpcPollData",
      "FrpcInstancePollData",
      "FrpcConfigUpdateData",
      "FrpcInstanceBodyData",
    ]) {
      assert.match(api, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /get_api_admin_frpc_instances__id__logs/u);
    assert.match(api, /get_api_admin_frpc_instances__id__poll/u);
    assert.match(api, /satisfies FrpcConfigUpdate/u);
    assert.match(api, /satisfies FrpcOverviewQuery/u);
    assert.match(api, /satisfies FrpcInstancePollQuery/u);
  });
});
