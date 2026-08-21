/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { ref } from "vue";

import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import { jaJPAdmin } from "../../../packages/i18n/src/messages/admin/ja-JP";
import { koKRAdmin } from "../../../packages/i18n/src/messages/admin/ko-KR";
import { zhCNAdmin } from "../../../packages/i18n/src/messages/admin/zh-CN";
import { zhHantAdmin } from "../../../packages/i18n/src/messages/admin/zh-Hant";
import type { SystemEventRecord } from "../src/types";
import {
  DEFAULT_GROUP_BY_BY_EVENT_TYPE,
  SYSTEM_EVENT_TYPE_OPTIONS,
} from "../src/views/event-center/constants";
import { resolveSystemEventOrigins } from "../src/views/event-center/systemEventDescription";
import { useSystemEventDisplay } from "../src/views/event-center/useSystemEventDisplay";

const visibilityEvent: SystemEventRecord = {
  id: "evt_visibility_1",
  type: "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
  source: "GO_REAUTH_PROXY",
  level: "WARN",
  happened_at: "2026-07-27T10:11:12Z",
  subject: { kind: "IP", id: "203.0.113.8" },
  tags: ["gateway", "visibility", "security"],
  payload: {
    ip: "203.0.113.8",
    ip_location: "Shanghai|Shanghai|Unicom",
    blocked_at: "2026-07-27T10:11:12Z",
    method: "GET",
    scheme: "https",
    host: "app.example.test",
    path: "/private",
    route_type: "host_rule",
    route_key: "app.example.test",
    visibility_scope: "host",
    visibility_mode: "custom",
    status: 499,
  },
};

describe("gateway visibility system event", () => {
  it("is available to filters and defaults notification grouping globally", () => {
    assert.ok(
      SYSTEM_EVENT_TYPE_OPTIONS.some(
        (option) => option.value === "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED",
      ),
    );
    assert.equal(
      DEFAULT_GROUP_BY_BY_EVENT_TYPE.FN_EVENT_GATEWAY_VISIBILITY_BLOCKED,
      "GLOBAL",
    );
  });

  it("renders a dedicated summary and policy detail fields", () => {
    const activeEvent = ref<SystemEventRecord | null>(visibilityEvent);
    const display = useSystemEventDisplay({
      activeEvent,
      translate: (key, params) =>
        params ? `${key}:${JSON.stringify(params)}` : key,
    });

    const summary = display.describeEvent(visibilityEvent);
    assert.match(summary, /gatewayVisibilityBlocked/u);
    assert.match(summary, /app\.example\.test/u);

    const details = new Map(
      display.detailItems.value.map((item) => [item.key, item.value]),
    );
    assert.equal(
      display.detailItems.value.find((item) => item.key === "method")?.label,
      "admin.eventCenter.events.detailFields.request_method",
    );
    assert.equal(details.get("method"), "GET");
    assert.equal(details.get("scheme"), "https");
    assert.equal(
      details.get("visibility_scope"),
      "admin.eventCenter.events.visibilityScope.host",
    );
    assert.equal(
      details.get("visibility_mode"),
      "admin.eventCenter.events.visibilityMode.custom",
    );
    assert.deepEqual(resolveSystemEventOrigins(visibilityEvent), [
      {
        key: "ip:203.0.113.8",
        ip: "203.0.113.8",
        location: "Shanghai|Shanghai|Unicom",
      },
    ]);
  });

  it("shows both endpoints and locations for an IP drift", () => {
    assert.deepEqual(
      resolveSystemEventOrigins({
        ...visibilityEvent,
        type: "FN_EVENT_AUTH_SESSION_IP_DRIFT",
        payload: {
          from_ip: "203.0.113.8",
          from_ip_location: "Shanghai|Shanghai|Unicom",
          to_ip: "198.51.100.9",
          to_ip_location: "Tokyo|Tokyo|Example ISP",
        },
      }),
      [
        {
          key: "from_ip:203.0.113.8",
          ip: "203.0.113.8",
          location: "Shanghai|Shanghai|Unicom",
        },
        {
          key: "to_ip:198.51.100.9",
          ip: "198.51.100.9",
          location: "Tokyo|Tokyo|Example ISP",
        },
      ],
    );
  });

  it("provides labels and descriptions in every supported locale", () => {
    for (const catalog of [
      zhCNAdmin,
      zhHantAdmin,
      enAdmin,
      jaJPAdmin,
      koKRAdmin,
    ]) {
      assert.ok(
        catalog.eventCenter.eventTypes.FN_EVENT_GATEWAY_VISIBILITY_BLOCKED,
      );
      assert.ok(catalog.eventCenter.events.gatewayVisibilityBlocked);
      assert.ok(catalog.eventCenter.events.detailFields.visibility_scope);
      assert.ok(catalog.eventCenter.events.detailFields.visibility_mode);
    }
  });
});
