import assert from "node:assert/strict";
import { chromium } from "playwright";
import { summarizeFrontendRuns } from "./frontend-performance-lib.mjs";
import { startRuntime } from "./runtime-test-harness.mjs";

const runCount = Number.parseInt(
  process.env.FN_KNOCK_FRONTEND_PERF_RUNS ?? "5",
  10,
);
const enforceResourceSelection =
  process.env.FN_KNOCK_FRONTEND_PERF_ENFORCE_RESOURCES !== "0";
if (!Number.isInteger(runCount) || runCount < 1 || runCount > 10) {
  throw new Error(
    "FN_KNOCK_FRONTEND_PERF_RUNS must be an integer from 1 to 10",
  );
}

const responseBody = (data) =>
  JSON.stringify({ success: true, data, message: null });
const authBase = {
  locale: { default_locale: "zh-CN" },
  appearance: { theme_color_preset: "default" },
  auth: {
    authenticated: false,
    message: "",
    grant_type: "login_ip_grant",
    login_mode: "totp",
  },
  client: { ip: "127.0.0.1" },
  passkey: { available: false },
  oidc: { providers: [] },
  ldap: { providers: [] },
};

const installAuthMocks = async (page, scenario) => {
  const captcha = {
    provider: scenario === "login_base" ? "turnstile" : "pow",
    widget_mode: "normal",
    available: scenario !== "login_base",
    unavailable_reason: scenario === "login_base" ? "disabled" : null,
    pow: {},
    turnstile: { site_key: "" },
  };
  await page.route("**/api/auth/bootstrap**", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: responseBody({ ...authBase, captcha }),
    }),
  );
  await page.route("**/api/auth/session**", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: responseBody({
        ...authBase,
        auth: {
          ...authBase.auth,
          authenticated: true,
          grant_type: "browser_session",
        },
      }),
    }),
  );
  await page.route("**/api/auth/ip/location**", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: responseBody({
        ip: "127.0.0.1",
        location: "local",
        status: "success",
        attempts: 1,
        maxAttempts: 1,
      }),
    }),
  );
  await page.route("**/api/auth/passkey/bind-status**", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: responseBody({ can_bind: false, credential_ids: [] }),
    }),
  );
  await page.route("**/challenge?**", (route) =>
    route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        algorithm: "SHA-256",
        challenge:
          "ec18eac8d758b1eba52d3c10d39adc6dd9806472cb4ae069635d383d9086a513",
        maxnumber: 0,
        salt: "s",
        signature: "frontend-performance",
      }),
    }),
  );
};

const applyThrottling = async (context, page) => {
  const session = await context.newCDPSession(page);
  await session.send("Emulation.setCPUThrottlingRate", { rate: 4 });
  await session.send("Network.enable");
  await session.send("Network.setCacheDisabled", { cacheDisabled: true });
  await session.send("Network.emulateNetworkConditions", {
    offline: false,
    latency: 150,
    downloadThroughput: (1.6 * 1024 * 1024) / 8,
    uploadThroughput: (750 * 1024) / 8,
    connectionType: "cellular3g",
  });
};

const measure = async ({
  browser,
  initScript,
  scenario,
  storageState,
  url,
  ready,
}) => {
  const context = await browser.newContext({ storageState });
  await context.addInitScript(initScript);
  if (scenario === "login_pow") {
    await context.addInitScript(() => {
      Object.defineProperty(window, "isSecureContext", { value: false });
    });
  }
  const page = await context.newPage();
  if (scenario.startsWith("login_") || scenario === "home") {
    await installAuthMocks(page, scenario);
  }
  const scripts = new Set();
  page.on("response", (response) => {
    const pathname = new URL(response.url()).pathname;
    if (
      response.request().resourceType() === "script" ||
      pathname.endsWith(".js")
    ) {
      scripts.add(pathname);
    }
  });
  await applyThrottling(context, page);
  await page.goto(url, { waitUntil: "domcontentloaded" });
  await page.locator(ready).first().waitFor({ state: "visible" });
  const routeReady = Math.round(await page.evaluate(() => performance.now()));
  if (scenario === "login_pow") {
    await page.locator("form button").first().click();
  }
  await page.waitForTimeout(250);
  const longTasks = Math.round(
    await page.evaluate(() => window.__fnKnockLongTaskTotal ?? 0),
  );
  const scriptList = [...scripts];
  const localeScripts = scriptList.filter((value) =>
    /\/(?:zh-CN|zh-Hant|en|ko-KR|ja-JP)-[^/]+\.js$/u.test(value),
  );
  const requestedLocales = new Set(
    localeScripts
      .map(
        (value) =>
          value.match(/\/(zh-CN|zh-Hant|en|ko-KR|ja-JP)-[^/]+\.js$/u)?.[1],
      )
      .filter(Boolean),
  );
  if (enforceResourceSelection) {
    assert.equal(
      requestedLocales.size,
      1,
      `${scenario} requested locales ${[...requestedLocales].join(", ")}: ${localeScripts.join(", ")}`,
    );
    assert.ok(requestedLocales.has("zh-CN"), `${scenario} did not load zh-CN`);
    if (scenario === "login_altcha") {
      assert.ok(scriptList.some((value) => /\/altcha-[^/]+\.js$/u.test(value)));
    } else {
      assert.ok(
        !scriptList.some((value) => /\/altcha-[^/]+\.js$/u.test(value)),
        `${scenario} unexpectedly loaded ALTCHA`,
      );
    }
    if (scenario === "login_pow") {
      assert.ok(
        scriptList.some((value) => /\/pow\.worker-[^/]+\.js$/u.test(value)),
        "login_pow did not load the PoW worker",
      );
    } else {
      assert.ok(
        !scriptList.some((value) => /\/pow\.worker-[^/]+\.js$/u.test(value)),
        `${scenario} unexpectedly loaded the PoW worker`,
      );
    }
  }
  await context.close();
  return {
    scenario,
    route_ready_ms: routeReady,
    long_task_total_ms: longTasks,
    scripts: scriptList,
  };
};

const setupAdminStorage = async (browser, adminUrl) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.goto(adminUrl, { waitUntil: "domcontentloaded" });
  const input = page.locator('input[autocomplete="new-password"]');
  await input.fill("runtime123");
  await page.locator('form button[type="submit"]').click();
  await page.locator("#main-content").waitFor({ state: "visible" });
  const storage = await context.storageState();
  await context.close();
  return storage;
};

const initScript = () => {
  window.__fnKnockLongTaskTotal = 0;
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      window.__fnKnockLongTaskTotal += entry.duration;
    }
  }).observe({ type: "longtask", buffered: true });
  window.localStorage.setItem("fn-knock:locale", "zh-CN");
};

let runtime;
let browser;
try {
  runtime = await startRuntime({
    gatewayBinary: process.env.FN_KNOCK_FRONTEND_PERF_GATEWAY_BIN,
    serverBinary: process.env.FN_KNOCK_RUNTIME_SERVER_BIN,
    protectedAdmin: true,
    tempPrefix: "fn-knock-frontend-performance-",
  });
  browser = await chromium.launch({ headless: true });
  const adminStorage = await setupAdminStorage(browser, runtime.adminUrl);
  const scenarios = [
    {
      scenario: "dashboard",
      storageState: adminStorage,
      url: runtime.adminUrl,
      ready: '[data-testid="theme-preset-trigger"]',
    },
    { scenario: "home", url: runtime.authUrl, ready: "button" },
    {
      scenario: "login_base",
      url: `${runtime.authUrl}/login`,
      ready: "form",
    },
    {
      scenario: "login_altcha",
      url: `${runtime.authUrl}/login`,
      ready: "altcha-widget",
    },
    {
      scenario: "login_pow",
      url: `${runtime.authUrl}/login`,
      ready: "form button",
    },
  ];
  const runs = [];
  for (const scenario of scenarios) {
    for (let index = 0; index < runCount; index += 1) {
      runs.push(await measure({ browser, initScript, ...scenario }));
    }
  }
  process.stdout.write(
    `${JSON.stringify(
      {
        schema_version: 1,
        throttle: { cpu: 4, network: "Fast 3G", cache: "cold" },
        runs,
        summary: summarizeFrontendRuns(runs),
      },
      null,
      2,
    )}\n`,
  );
} finally {
  await browser?.close();
  await runtime?.stop();
}
