import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import type { GatewayLogEntry } from "../src/types";
import { AUTH_DECISION_LABEL_KEYS } from "../src/lib/gatewayLogLabels";
import {
  accessModeLabel,
  authDecisionLabel,
  buildGatewayLogDetailItems,
  buildGatewayLogSelectionKey,
  formatAuthCredential,
  getGatewayLogOptionLabel,
  isWAFBlocked,
  STATUS_FILTER_OPTIONS,
} from "../src/views/gateway-request-logs/model";

const gatewayAuthDecisions = [
  "access_denied",
  "advanced_bypass",
  "auth_error",
  "auth_unconfigured",
  "binding_token",
  "bridge_unavailable",
  "bypassed",
  "connection_reset",
  "crawler_blocked",
  "denied",
  "disabled",
  "disconnected",
  "error",
  "fn_app_prompt",
  "general_blacklist_blocked",
  "http1_required",
  "http2_required",
  "internal",
  "invalid_response",
  "not_required",
  "outside_window",
  "passed",
  "proxy",
  "public",
  "queue_full",
  "rate_limited",
  "redirected",
  "robots_txt_served",
  "root_mode_redirect",
  "rule_missing",
  "schedule_closed",
  "scope_denied",
  "subdomain_rule_allowed",
  "timeout",
  "visibility_denied",
  "waf_blocked",
] as const;

const translate = (key: string, params?: Record<string, unknown>) =>
  params ? `${key}:${JSON.stringify(params)}` : key;

describe("gateway request log model", () => {
  it("localizes every gateway authentication decision", () => {
    assert.deepEqual(
      Object.keys(AUTH_DECISION_LABEL_KEYS).sort(),
      [...gatewayAuthDecisions].sort(),
    );

    for (const decision of gatewayAuthDecisions) {
      const label = authDecisionLabel(decision, translate);
      assert.match(label, /^admin\.gatewayRequestLogs\.authDecisions\./u);
      assert.notEqual(label, decision);
    }
  });

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

  it("routes every detail field label through i18n", () => {
    const source = readFileSync(
      new URL(
        "../src/views/gateway-request-logs/gatewayRequestLogDetails.ts",
        import.meta.url,
      ),
      "utf8",
    );
    assert.doesNotMatch(source, /\blabel:\s*["']/u);
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

  it("localizes access modes and credential methods in details", () => {
    assert.equal(
      accessModeLabel("login_first", translate),
      "admin.gatewayRequestLogs.accessModes.loginFirst",
    );
    const details = buildGatewayLogDetailItems(
      {
        access_mode: "strict_whitelist",
        auth_credential_method: "PASSWORD",
      } as GatewayLogEntry,
      translate,
      "zh-CN",
    );
    assert.equal(
      details.find((item) => item.label.includes("accessMode"))?.value,
      "admin.gatewayRequestLogs.accessModes.strictWhitelist",
    );
    assert.equal(
      details.find((item) => item.label.includes("authCredentialMethod"))
        ?.value,
      "admin.gatewayRequestLogs.credentialMethods.password",
    );
  });
});
