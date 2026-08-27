import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { isTraceId, normalizeTraceId } from "../src/lib/trace-id";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");
const contract = JSON.parse(
  readSource("../../../packages/api-contract/openapi.json"),
) as {
  paths: Record<string, Record<string, unknown>>;
  components: {
    schemas: Record<
      string,
      {
        properties?: Record<string, unknown>;
        required?: string[];
        enum?: string[];
      }
    >;
  };
};

describe("unified Trace ID contract", () => {
  it("publishes the aggregate lookup and exact list filters", () => {
    const traceOperation = contract.paths["/api/admin/traces/{trace_id}"]?.get;
    assert.ok(traceOperation);
    assert.match(JSON.stringify(traceOperation), /"400"/u);
    assert.match(JSON.stringify(traceOperation), /\(\?:trc\|waf\)/u);
    for (const path of [
      "/api/admin/events",
      "/api/admin/gateway-logs/entries",
      "/api/admin/notifications/triggers",
      "/api/admin/notifications/deliveries",
    ]) {
      const operation = JSON.stringify(contract.paths[path]?.get);
      assert.match(operation, /trace_id/u, path);
      assert.match(operation, /\(\?:trc\|waf\)/u, path);
    }
    const trace = contract.components.schemas.TraceLookupData;
    for (const field of [
      "trace_id",
      "found",
      "request",
      "waf_event",
      "system_events",
      "notification_triggers",
      "notification_deliveries",
      "sources",
    ]) {
      assert.ok(trace.properties?.[field], field);
    }
    assert.deepEqual(contract.components.schemas.TraceSourceStatusData.enum, [
      "found",
      "not_found",
      "unavailable",
    ]);
  });

  it("normalizes input and accepts only canonical current or legacy IDs", () => {
    const current = "trc_3f93d40a-89ea-4dbe-a04f-67692778d973";
    const legacy = "waf_3f93d40a-89ea-4dbe-a04f-67692778d973";
    assert.equal(normalizeTraceId(`  ${current}  `), current);
    assert.equal(isTraceId(current), true);
    assert.equal(isTraceId(legacy), true);
    assert.equal(isTraceId("trc_3F93D40A-89EA-4DBE-A04F-67692778D973"), false);
    assert.equal(isTraceId("trc_not-a-uuid"), false);
  });

  it("keeps entity identifiers separate while exposing trace_id", () => {
    for (const schema of [
      "SystemEventData",
      "NotificationMessageData",
      "NotificationTriggerData",
      "NotificationDeliveryData",
      "GatewayLogEntryData",
    ]) {
      assert.ok(
        contract.components.schemas[schema].properties?.trace_id,
        schema,
      );
    }
    assert.ok(
      contract.components.schemas.NotificationTriggerData.properties?.id,
    );
    assert.ok(
      contract.components.schemas.NotificationDeliveryData.properties?.id,
    );
  });

  it("registers a hidden trace page with timeline, partial failure, and lookup entry points", () => {
    const router = readSource("../src/router/index.ts");
    const tracePage = readSource("../src/views/TraceDetails.vue");
    const requestAnalysis = readSource("../src/views/RequestAnalysis.vue");
    const eventCenter = readSource("../src/views/EventCenter.vue");
    assert.match(router, /path: "traces\/:trace_id"/u);
    assert.match(tracePage, /notification_deliveries/u);
    assert.match(tracePage, /unavailableSources/u);
    assert.match(tracePage, /timeline/u);
    assert.match(requestAnalysis, /TraceLookupButton/u);
    assert.match(eventCenter, /TraceLookupButton/u);
  });
});
