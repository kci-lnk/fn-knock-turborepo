import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

type PropertySchema = {
  const?: number;
  enum?: string[];
  maximum?: number;
  minimum?: number;
  oneOf?: PropertySchema[];
  pattern?: string;
};

type Parameter = {
  in?: string;
  name?: string;
  required?: boolean;
  schema?: PropertySchema;
};

type Operation = {
  "x-fn-knock-contract-source"?: string;
  parameters?: Parameter[];
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

describe("deep monitor API contract", () => {
  it("keeps all JSON and streaming operations on the runtime Utoipa router", () => {
    for (const [method, path] of [
      ["get", "/api/admin/deep-monitor/sessions"],
      ["post", "/api/admin/deep-monitor/sessions"],
      ["get", "/api/admin/deep-monitor/sessions/{session_id}"],
      ["delete", "/api/admin/deep-monitor/sessions/{session_id}"],
      ["post", "/api/admin/deep-monitor/sessions/{session_id}/extend"],
      ["post", "/api/admin/deep-monitor/sessions/{session_id}/stop"],
      ["get", "/api/admin/deep-monitor/sessions/{session_id}/events"],
      [
        "get",
        "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}",
      ],
      [
        "get",
        "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload",
      ],
      ["get", "/api/admin/deep-monitor/sessions/{session_id}/live"],
      ["get", "/api/admin/deep-monitor/sessions/{session_id}/download"],
    ] as const) {
      assert.equal(
        contract.paths[path]?.[method]?.["x-fn-knock-contract-source"],
        "utoipa",
        `${method.toUpperCase()} ${path}`,
      );
    }
  });

  it("preserves duration, page, and nullable event boundaries", () => {
    const duration =
      contract.components.schemas.DeepMonitorStartBodyData.properties
        ?.duration_seconds;
    assert.ok(duration?.oneOf?.some((schema) => schema.const === 0));
    assert.ok(
      duration?.oneOf?.some(
        (schema) => schema.minimum === 300 && schema.maximum === 7_200,
      ),
    );
    const extendDuration =
      contract.components.schemas.DeepMonitorExtendBodyData.properties
        ?.duration_seconds;
    assert.equal(extendDuration?.minimum, 300);
    assert.equal(extendDuration?.maximum, 7_200);
    assert.equal(
      contract.components.schemas.DeepMonitorStartBodyData.required?.includes(
        "duration_seconds",
      ) ?? false,
      false,
    );

    const eventRequired =
      contract.components.schemas.DeepMonitorEventData.required ?? [];
    for (const field of ["summary", "timing", "websocket_frame"]) {
      assert.ok(eventRequired.includes(field), field);
    }

    const eventParameters =
      contract.paths["/api/admin/deep-monitor/sessions/{session_id}/events"].get
        .parameters ?? [];
    const limit = eventParameters.find(
      (parameter) => parameter.name === "limit",
    );
    assert.equal(limit?.schema?.minimum, 1);
    assert.equal(limit?.schema?.maximum, 200);
  });

  it("documents SSE, binary payloads, empty streams, and ZIP downloads", () => {
    const payload =
      contract.paths[
        "/api/admin/deep-monitor/sessions/{session_id}/events/{event_id}/payload"
      ].get;
    assert.ok(
      payload.responses?.["200"]?.content?.["application/octet-stream"],
    );
    assert.ok(payload.responses?.["204"]);
    assert.equal(
      payload.parameters?.find((parameter) => parameter.name === "part")
        ?.required,
      true,
    );

    const live =
      contract.paths["/api/admin/deep-monitor/sessions/{session_id}/live"].get;
    assert.ok(live.responses?.["200"]?.content?.["text/event-stream"]);
    assert.ok(
      live.parameters?.some(
        (parameter) =>
          parameter.in === "header" && parameter.name === "Last-Event-ID",
      ),
    );

    const download =
      contract.paths["/api/admin/deep-monitor/sessions/{session_id}/download"]
        .get;
    assert.ok(download.responses?.["200"]?.content?.["application/zip"]);
    assert.ok(download.responses?.["204"]);
  });

  it("derives frontend session, event, timing, and request models", () => {
    const types = readSource("../src/types.ts");
    const api = readSource("../src/lib/api/deep-monitor.ts");
    for (const schema of [
      "DeepMonitorSessionData",
      "DeepMonitorEventSummaryData",
      "DeepMonitorTimingData",
      "DeepMonitorWebSocketFrameData",
      "DeepMonitorEventData",
    ]) {
      assert.match(types, new RegExp(`\\["${schema}"\\]`, "u"), schema);
    }
    assert.match(api, /DeepMonitorSchemas\["DeepMonitorStartBodyData"\]/u);
    assert.match(api, /DeepMonitorSchemas\["DeepMonitorEventListData"\]/u);
    assert.match(api, /satisfies DeepMonitorExtendRequest/u);
  });
});
