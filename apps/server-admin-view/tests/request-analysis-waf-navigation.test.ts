/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("request analysis WAF navigation", () => {
  it("shows WAF logs between request logs and analytics only when enabled", () => {
    const page = readSource("../src/views/RequestAnalysis.vue");
    const logs = page.indexOf('<TabsTrigger value="logs">');
    const waf = page.indexOf('<TabsTrigger v-if="showWafTab" value="waf">');
    const analytics = page.indexOf('<TabsTrigger value="analytics">');

    assert.ok(logs >= 0);
    assert.ok(waf > logs);
    assert.ok(analytics > waf);
    assert.match(page, /configStore\.config\?\.waf\?\.enabled === true/u);
    assert.match(
      page,
      /showWafTab\.value \? \["logs", "waf", "analytics"\] : \["logs", "analytics"\]/u,
    );
    assert.match(page, /defaultTab: "logs"/u);
  });

  it("embeds WAF logs and places their controls in the shared tab action area", () => {
    const page = readSource("../src/views/RequestAnalysis.vue");
    const wafLogs = readSource("../src/views/WAFLogs.vue");

    assert.match(page, /id="request-analysis-waf-actions"/u);
    assert.match(page, /<WAFLogs v-if="currentTab === 'waf'" \/>/u);
    assert.match(page, /v-if="currentTab !== 'waf' && !isLoggingEnabled"/u);
    assert.match(
      wafLogs,
      /<Teleport defer to="#request-analysis-waf-actions">/u,
    );
    assert.doesNotMatch(wafLogs, /WAFLogsDisabledNotice/u);
  });

  it("redirects the legacy route while preserving trace and filter queries", () => {
    const router = readSource("../src/router/index.ts");

    assert.match(router, /path: "waf-logs",\s*redirect: \(to\) =>/u);
    assert.match(router, /query: \{ \.\.\.to\.query, tab: "waf" \}/u);
  });

  it("uses only the unified request analysis sidebar entry", () => {
    const navigation = readSource("../src/views/layout/useLayoutNavigation.ts");

    assert.match(navigation, /id: "gateway_request_logs"/u);
    assert.match(navigation, /path: "\/request-analysis"/u);
    assert.doesNotMatch(navigation, /id: "waf_logs"/u);
    assert.doesNotMatch(navigation, /path: "\/waf-logs"/u);
  });
});
