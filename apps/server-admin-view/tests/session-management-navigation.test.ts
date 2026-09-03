/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("session management navigation", () => {
  it("redirects the legacy whitelist route and preserves its query", () => {
    const routerSource = readSource("../src/router/index.ts");

    assert.match(routerSource, /path: "whitelist",\s*redirect: \(to\) =>/u);
    assert.match(
      routerSource,
      /query: \{ \.\.\.to\.query, tab: "ip-whitelist" \}/u,
    );
  });

  it("places the IP whitelist directly after sessions", () => {
    const pageSource = readSource("../src/views/SessionManagement.vue");
    const sessionsTrigger = pageSource.indexOf('value="sessions"');
    const whitelistTrigger = pageSource.indexOf('value="ip-whitelist"');
    const loginBackoffTrigger = pageSource.indexOf('value="login-backoff"');

    assert.ok(sessionsTrigger >= 0);
    assert.ok(whitelistTrigger > sessionsTrigger);
    assert.ok(loginBackoffTrigger > whitelistTrigger);
    assert.match(
      pageSource,
      /showSessionsTab\.value \? "sessions" : "ip-whitelist"/u,
    );
    assert.match(pageSource, /docsUrls\.guides\.whitelist/u);
  });

  it("uses sessions as the only sidebar entry for both features", () => {
    const navigationSource = readSource(
      "../src/views/layout/useLayoutNavigation.ts",
    );

    assert.match(navigationSource, /id: "sessions"/u);
    assert.doesNotMatch(navigationSource, /id: "ip_whitelist"/u);
    assert.doesNotMatch(navigationSource, /path: "\/whitelist"/u);
  });
});
