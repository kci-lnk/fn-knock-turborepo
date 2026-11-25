import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import { chromium } from "playwright";
import { startRuntime } from "./runtime-test-harness.mjs";

const password = "runtime123";

const apiRequest = async (
  page,
  path,
  { method = "GET", body, binary = false } = {},
) => {
  const response = await page.evaluate(
    async ({ requestPath, requestMethod, requestBody, expectBinary }) => {
      const result = await fetch(`/api/admin${requestPath}`, {
        method: requestMethod,
        credentials: "include",
        headers:
          requestBody === undefined
            ? {}
            : { "content-type": "application/json" },
        body:
          requestBody === undefined ? undefined : JSON.stringify(requestBody),
      });
      const headers = Object.fromEntries(result.headers.entries());
      if (expectBinary) {
        const bytes = new Uint8Array(await result.arrayBuffer());
        let encoded = "";
        const chunkSize = 32_768;
        for (let offset = 0; offset < bytes.length; offset += chunkSize) {
          encoded += String.fromCharCode(
            ...bytes.subarray(offset, offset + chunkSize),
          );
        }
        return {
          status: result.status,
          headers,
          body: btoa(encoded),
        };
      }
      const text = await result.text();
      return {
        status: result.status,
        headers,
        body: text ? JSON.parse(text) : null,
      };
    },
    {
      requestPath: path,
      requestMethod: method,
      requestBody: body,
      expectBinary: binary,
    },
  );
  assert.ok(
    response.status >= 200 && response.status < 300,
    `${method} ${path} returned ${response.status}: ${JSON.stringify(response.body)}`,
  );
  return response;
};

const submitGatePassword = async (page, autocomplete) => {
  const input = page.locator(`input[autocomplete="${autocomplete}"]`);
  try {
    await input.waitFor({ state: "visible" });
  } catch (error) {
    const state = await page.evaluate(async () => {
      const bootstrap = await fetch("/api/admin/panel/bootstrap", {
        credentials: "include",
      })
        .then(async (response) => ({
          status: response.status,
          body: await response.text(),
        }))
        .catch((requestError) => ({ error: String(requestError) }));
      return {
        url: window.location.href,
        inputs: Array.from(document.querySelectorAll("input")).map(
          (element) => ({
            id: element.id,
            autocomplete: element.autocomplete,
            type: element.type,
            visible: Boolean(element.offsetWidth || element.offsetHeight),
          }),
        ),
        bootstrap,
      };
    });
    throw new Error(
      `Expected ${autocomplete} gate was not visible: ${JSON.stringify(state)}`,
      { cause: error },
    );
  }
  await input.fill(password);
  if (autocomplete === "current-password") {
    await page.locator("#dockerAdminRememberMe").click();
  }
  await page.locator('form button[type="submit"]').click();
  await page.locator("#main-content").waitFor({ state: "visible" });
};

const loginWithFreshSession = async (context, page) => {
  const adminUrl = new URL(page.url()).origin;
  await page.goto("about:blank");
  await context.clearCookies();
  await page.goto(adminUrl, { waitUntil: "domcontentloaded" });
  await submitGatePassword(page, "current-password");
};

const verifyAdminIframeEmbedding = async (runtime, context, page) => {
  const settingsUrl = `${runtime.adminUrl}/#/system?tab=features`;
  const iframeSettingName = /^(是否可被 iframe 嵌入|Iframe embedding)$/u;
  const allowLabel = /^(可被嵌入|Allow embedding)$/u;
  const blockLabel = /^(不可被嵌入|Block embedding)$/u;
  await page.goto(settingsUrl);
  const setting = page.getByRole("group", { name: iframeSettingName });
  const allow = setting.getByRole("button", { name: allowLabel });
  const block = setting.getByRole("button", { name: blockLabel });
  assert.equal(await block.getAttribute("aria-pressed"), "true");

  const parent = await context.newPage();
  const parentServer = createServer((_request, response) => {
    response.writeHead(200, {
      "content-type": "text/html; charset=utf-8",
      "cache-control": "no-store",
    });
    response.end(
      `<!doctype html><iframe width="960" height="600" src="${settingsUrl}"></iframe>`,
    );
  });
  try {
    const listening = once(parentServer, "listening");
    parentServer.listen(0, "127.0.0.1");
    await listening;
    const parentUrl = `http://127.0.0.1:${parentServer.address().port}/`;
    const assertFrameBlocked = async () => {
      const [failure] = await Promise.all([
        parent.waitForEvent(
          "requestfailed",
          (request) =>
            request.isNavigationRequest() &&
            new URL(request.url()).origin === runtime.adminUrl,
        ),
        parent.goto(parentUrl),
      ]);
      assert.match(
        failure.failure()?.errorText ?? "",
        /ERR_BLOCKED_BY_RESPONSE/u,
      );
    };
    const saveOption = async (option, enabled) => {
      const [response] = await Promise.all([
        page.waitForResponse(
          (response) =>
            response.url().endsWith("/api/admin/config/dashboard_display") &&
            response.request().method() === "POST",
        ),
        option.click(),
      ]);
      assert.equal(response.status(), 200);
      assert.equal(
        (await response.json()).data.allow_iframe_embedding,
        enabled,
      );
    };

    await assertFrameBlocked();
    await saveOption(allow, true);
    await parent.goto(parentUrl);
    const embeddedAllow = parent
      .frameLocator("iframe")
      .getByRole("group", { name: iframeSettingName })
      .getByRole("button", { name: allowLabel });
    assert.equal(await embeddedAllow.getAttribute("aria-pressed"), "true");

    const auth = await context.request.head(runtime.authUrl);
    assert.equal(auth.headers()["x-frame-options"], "DENY");
    assert.equal(
      auth.headers()["content-security-policy"],
      "frame-ancestors 'none'",
    );

    await saveOption(block, false);
    await assertFrameBlocked();
  } finally {
    await parent.close();
    if (parentServer.listening) {
      await new Promise((resolve) => parentServer.close(resolve));
    }
  }
};

let runtime;
let browser;
try {
  runtime = await startRuntime({
    gatewayBinary:
      process.env.FN_KNOCK_RUNTIME_E2E_GATEWAY_BIN ??
      process.env.FN_KNOCK_A11Y_GATEWAY_BIN,
    protectedAdmin: true,
    tempPrefix: "fn-knock-runtime-e2e-",
  });
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const page = await context.newPage();

  await page.goto(runtime.adminUrl, { waitUntil: "domcontentloaded" });
  await submitGatePassword(page, "new-password");
  await loginWithFreshSession(context, page);
  await verifyAdminIframeEmbedding(runtime, context, page);
  await page.goto(runtime.adminUrl, { waitUntil: "domcontentloaded" });

  const originalAppearance = (await apiRequest(page, "/config/appearance")).body
    .data;
  const originalMappingsResponse = await apiRequest(
    page,
    "/config/host_mappings",
  );
  const originalMappings = originalMappingsResponse.body.data;
  const backup = await apiRequest(page, "/maintenance/backup/export", {
    binary: true,
  });
  assert.ok(backup.body.length > 0, "backup export was empty");

  const appearanceSave = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/admin/config/appearance") &&
      response.request().method() === "POST",
  );
  await page.locator('[data-testid="theme-preset-trigger"]').click();
  await page.locator('[data-theme-preset="prussian_blue"]').click();
  assert.equal((await appearanceSave).status(), 200);
  assert.deepEqual((await apiRequest(page, "/config/appearance")).body.data, {
    theme_color_preset: "prussian_blue",
  });

  const e2eHost = "runtime-e2e.invalid";
  const updatedMappings = [
    ...originalMappings,
    {
      host: e2eHost,
      target: "http://127.0.0.1:65535",
      disabled: true,
      use_auth: false,
    },
  ];
  const updateResponse = await apiRequest(page, "/config/host_mappings", {
    method: "POST",
    body: {
      mappings: updatedMappings,
      revision:
        originalMappingsResponse.headers["x-host-mappings-revision"] ||
        undefined,
    },
  });
  assert.ok(
    updateResponse.body.data.some((mapping) => mapping.host === e2eHost),
    "Host mapping update did not persist",
  );

  const importResult = await apiRequest(page, "/maintenance/backup/import", {
    method: "POST",
    body: {
      archive_base64: backup.body,
      filename: "runtime-e2e.knock",
    },
  });
  assert.equal(importResult.body.success, true);

  await loginWithFreshSession(context, page);
  assert.deepEqual(
    (await apiRequest(page, "/config/appearance")).body.data,
    originalAppearance,
    "backup restore did not recover appearance config",
  );
  assert.deepEqual(
    (await apiRequest(page, "/config/host_mappings")).body.data.map(
      (mapping) => mapping.host,
    ),
    originalMappings.map((mapping) => mapping.host),
    "backup restore did not recover Host mappings",
  );

  await context.close();
  console.log(
    "[runtime-e2e] passed: protected setup/login, component-driven config save, " +
      "iframe embedding, Host mapping update, and backup restore",
  );
} finally {
  await browser?.close();
  await runtime?.stop();
}
