import assert from "node:assert/strict";
import http from "node:http";
import { chromium } from "playwright";
import { startRuntime } from "./runtime-test-harness.mjs";

// Build AUTH and the Rust runtime first. The harness builds the sibling gateway.
const runtime = await startRuntime({
  collectMetrics: false,
  serverBinary:
    process.env.FN_KNOCK_RUNTIME_SERVER_BIN ??
    "apps/server-admin-rs/target/runtime-test/server-admin-rs",
});
let browser;
try {
  // Keep dot segments intact: fetch and browsers normalize them before sending.
  for (const [pathname, base] of [
    ["/auth/./index.html", "/auth/"],
    ["/__auth__/./index.html", "/__auth__/"],
    ["/./index.html", "/"],
  ]) {
    const response = await new Promise((resolve, reject) => {
      const url = new URL(runtime.authUrl);
      const request = http.get(
        { hostname: url.hostname, port: url.port, path: pathname },
        (res) => {
          let body = "";
          res.setEncoding("utf8");
          res.on("data", (chunk) => (body += chunk));
          res.on("error", reject);
          res.on("end", () =>
            resolve({ status: res.statusCode, headers: res.headers, body }),
          );
        },
      );
      request.setTimeout(10000, () =>
        request.destroy(new Error(`Timeout: ${pathname}`)),
      );
      request.on("error", reject);
    });
    assert.equal(response.status, 200);
    assert.match(response.headers["cache-control"], /no-store/);
    assert.equal(response.headers.etag, undefined);
    assert.ok(
      response.body.includes(`<base id="auth-app-base" href="${base}"`),
      pathname,
    );
    console.log(`PASS raw HTML alias ${pathname}`);
  }
  browser = await chromium.launch({ headless: true });
  for (const [origin, pathname, base] of [
    [runtime.authUrl, "/login", "/"],
    [runtime.authUrl, "/auth/login", "/auth/"],
    [runtime.authUrl, "/auth/index.html", "/auth/"],
    [runtime.gatewayProxyUrl, "/__auth__/", "/__auth__/"],
    [runtime.authUrl, "/auth/oidc/bind?token=test", "/auth/"],
    [runtime.authUrl, "/__auth__/ldap/bind?token=test", "/__auth__/"],
    [
      runtime.gatewayProxyUrl,
      "/__auth__/login?redirect_uri=%2Fprivate",
      "/__auth__/",
    ],
    [runtime.gatewayProxyUrl, "/__auth__/ldap/bind?token=test", "/__auth__/"],
  ]) {
    const page = await browser.newPage();
    const assets = [];
    page.on("request", (request) => {
      const url = new URL(request.url());
      if (
        url.pathname.includes("/assets/") ||
        /\/(?:favicon[^/]*|apple-touch-icon\.png)$/.test(url.pathname)
      )
        assets.push(request);
    });
    const document = await page.goto(origin + pathname);
    assert.equal(document.status(), 200, pathname);
    assert.match(document.headers()["cache-control"], /no-store/);
    await page.waitForLoadState("networkidle");
    assert.equal(
      new URL(await page.evaluate(() => document.baseURI)).pathname,
      base,
    );
    assert.ok(assets.length > 0, `No assets requested for ${pathname}`);
    for (const request of assets) {
      const url = new URL(request.url());
      assert.equal(url.origin, origin);
      assert.ok(
        url.pathname.startsWith(base),
        `Wrong asset ${url.pathname} at ${pathname}`,
      );
      if (url.pathname.includes("/assets/")) {
        assert.ok(url.pathname.startsWith(`${base}assets/`), url.pathname);
      }
      // Inspect the browser's original request: a later successful refetch
      // must not hide a speculative redirect, failed request, or HTML response.
      const response = await request.response();
      assert.ok(response, `${request.url()}: ${request.failure()?.errorText}`);
      assert.equal(response.status(), 200, request.url());
      const contentType = response.headers()["content-type"] ?? "";
      if (url.pathname.endsWith(".js")) assert.match(contentType, /javascript/);
      if (url.pathname.endsWith(".css")) assert.match(contentType, /text\/css/);
    }
    const missing = await fetch(
      `${origin}${base}assets/missing-regression.js`,
      { redirect: "manual" },
    );
    assert.equal(missing.status, 404);
    console.log(
      `PASS ${origin}${pathname}: ${assets.length} correctly scoped asset requests`,
    );
    await page.close();
  }
} finally {
  await browser?.close();
  await runtime.stop();
}
