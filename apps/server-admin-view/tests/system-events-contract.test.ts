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
        properties?: Record<string, { enum?: string[]; description?: string }>;
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
        parameters?: Array<{
          name?: string;
          in?: string;
          required?: boolean;
          schema?: { enum?: string[] };
        }>;
        requestBody?: {
          content?: Record<string, { schema?: { $ref?: string } }>;
        };
        responses?: Record<
          string,
          { content?: Record<string, { schema?: { $ref?: string } }> }
        >;
      }
    >
  >;
};

describe("system event and login backoff API contract", () => {
  it("keeps all event and backoff operations typed", () => {
    for (const [method, path] of [
      ["post", "/api/internal/system-events"],
      ["get", "/api/admin/events"],
      ["delete", "/api/admin/events"],
      ["delete", "/api/admin/events/clear"],
      ["get", "/api/admin/backoff/list"],
      ["get", "/api/admin/backoff/status"],
      ["post", "/api/admin/backoff/reset"],
    ] as const) {
      assert.ok(
        isGeneratedContractSource(
          contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        ),
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves event filters, deletion body, and direct publication response", () => {
    const events = contract.paths["/api/admin/events"];
    assert.equal(
      events.delete.requestBody?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/SystemEventDeleteBodyData",
    );
    const parameters = events.get.parameters ?? [];
    assert.deepEqual(
      parameters.find((parameter) => parameter.name === "level")?.schema?.enum,
      ["INFO", "WARN", "ERROR", "CRITICAL"],
    );
    assert.deepEqual(
      parameters.find((parameter) => parameter.name === "source")?.schema?.enum,
      ["SERVER_ADMIN", "GO_REAUTH_PROXY", "SYSTEM_MONITOR", "RUNTIME_MONITOR"],
    );
    assert.equal(
      contract.paths["/api/internal/system-events"].post.responses?.["200"]
        ?.content?.["application/json"]?.schema?.$ref,
      "#/components/schemas/SystemEventPublishResultData",
    );
    assert.ok(
      contract.components.schemas.SystemEventPublishResultData.required?.includes(
        "data",
      ),
    );
  });

  it("documents backoff query and time units without snake-case drift", () => {
    const status = contract.paths["/api/admin/backoff/status"].get;
    assert.equal(
      status.parameters?.find((parameter) => parameter.name === "ip")?.required,
      true,
    );
    const backoff = contract.components.schemas.LoginBackoffData.properties;
    for (const field of [
      "ip",
      "attempts",
      "blocked",
      "retryAfter",
      "blockedUntil",
    ]) {
      assert.ok(backoff?.[field], field);
    }
    assert.match(backoff?.retryAfter?.description ?? "", /Seconds/u);
    assert.match(backoff?.blockedUntil?.description ?? "", /milliseconds/u);
    assert.equal(backoff?.blocked_until, undefined);
  });

  it("derives frontend event and backoff types and requests", () => {
    const types = readSource("../src/types/system-events.ts");
    const eventApi = readSource("../src/lib/api/events.ts");
    const configApi = readSource("../src/lib/api/config.ts");

    assert.match(types, /SystemEventSchemas\["SystemEventData"\]/u);
    assert.doesNotMatch(types, /export interface SystemEventRecord/u);
    assert.match(eventApi, /operations as ApiContractOperations/u);
    assert.match(eventApi, /satisfies GetEventsQuery/u);
    assert.match(eventApi, /satisfies DeleteEventsBody/u);
    assert.match(configApi, /\["LoginBackoffData"\]/u);
    assert.match(configApi, /satisfies BackoffStatusQuery/u);
    assert.match(configApi, /satisfies BackoffResetBody/u);
    assert.doesNotMatch(configApi, /export type BackoffItem = \{/u);
  });
});
