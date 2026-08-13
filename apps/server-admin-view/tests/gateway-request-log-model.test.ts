import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import type { GatewayLogEntry } from "../src/types";
import {
  buildGatewayLogDetailItems,
  buildGatewayLogSelectionKey,
  formatAuthCredential,
  getGatewayLogOptionLabel,
  isWAFBlocked,
  STATUS_FILTER_OPTIONS,
} from "../src/views/gateway-request-logs/model";

const translate = (key: string, params?: Record<string, unknown>) =>
  params ? `${key}:${JSON.stringify(params)}` : key;

describe("gateway request log model", () => {
  it("keeps the compatibility entrypoint as a focused barrel", () => {
    const source = readFileSync(
      new URL("../src/views/gateway-request-logs/model.ts", import.meta.url),
      "utf8",
    );
    assert.match(source, /gatewayRequestLogFilters/u);
    assert.match(source, /gatewayRequestLogPresentation/u);
    assert.match(source, /gatewayRequestLogDetails/u);
    assert.equal(source.includes("detailFields"), false);
  });

  it("preserves filter labels and stable row selection keys", () => {
    assert.equal(
      getGatewayLogOptionLabel(
        STATUS_FILTER_OPTIONS,
        "404",
        "fallback",
        translate,
      ),
      "admin.gatewayRequestLogs.statusFilters.notFound404",
    );
    const entry = {
      time: "2026-08-14T00:00:00Z",
      method: "GET",
      host: "example.test",
      path: "/health",
      status: 200,
      client_ip: "192.0.2.1",
    } as GatewayLogEntry;
    assert.equal(
      buildGatewayLogSelectionKey(entry, 3, "cursor-a"),
      "cursor-a|3|2026-08-14T00:00:00Z|GET|example.test|/health|200||192.0.2.1||",
    );
  });

  it("preserves WAF, credential, and detail presentation", () => {
    const entry = {
      duration_ms: 12,
      logged_in: true,
      waf_action: "deny",
      auth_credential_method: "OIDC",
      auth_credential_name: "example-user",
      auth_linked_totp_name: "backup-token",
    } as GatewayLogEntry;
    assert.equal(isWAFBlocked(entry), true);
    assert.match(formatAuthCredential(entry, translate), /example-user/u);
    assert.match(formatAuthCredential(entry, translate), /backup-token/u);

    const details = buildGatewayLogDetailItems(entry, translate, "en-US");
    assert.equal(
      details.find((item) => item.label.includes("duration"))?.value,
      "12 ms",
    );
    assert.equal(
      details.find((item) => item.label.includes("loggedIn"))?.value,
      "admin.gatewayRequestLogs.boolean.yes",
    );
  });
});
