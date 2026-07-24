import AxeBuilder from "@axe-core/playwright";
import { chromium } from "playwright";
import { spawn } from "node:child_process";
import { access, mkdtemp, rm } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wcagTags = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
const themeColorPresets = [
  "default",
  "hermes_orange",
  "prussian_blue",
  "dynamic_white",
];
const failures = [];

const adminRoutes = [
  "/",
  "/whitelist",
  "/proxy",
  "/subdomains",
  "/streams",
  "/ssl",
  "/mode",
  "/auth",
  "/auth/oidc-providers",
  "/events",
  "/ssh-security",
  "/request-logs",
  "/waf-logs",
  "/system",
  "/system/gateway-visibility",
  "/system/gateway-portal",
  "/system/gateway-proxy-headers",
  "/system/gateway-host-response",
  "/system/gateway-locations",
  "/system/smart-connect",
  "/system/sidebar-menu-order",
  "/system/fnos-certificate-sync",
  "/sessions",
  "/terminal",
  "/tunnel",
  "/ddns",
  "/about",
];

const assert = (condition, scope, message) => {
  if (!condition) failures.push({ scope, message });
};

const jsonResponse = (data) => ({
  status: 200,
  contentType: "application/json",
  body: JSON.stringify({ success: true, data }),
});

const getFreePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolve(address.port));
    });
  });

const waitForHttp = async (url, timeoutMs = 60_000) => {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
};

const ensureRuntimeArtifacts = async () => {
  const required = [
    "apps/server-admin-rs/target/release/server-admin-rs",
    "apps/server-admin-view/dist/index.html",
    "apps/server-auth-view/dist/index.html",
  ];
  for (const relativePath of required) {
    try {
      await access(path.join(rootDir, relativePath));
    } catch {
      throw new Error(
        `Missing ${relativePath}. Run npm run build before npm run a11y:audit.`,
      );
    }
  }
};

const startRuntime = async () => {
  const externalAdminUrl = process.env.FN_KNOCK_A11Y_ADMIN_URL;
  const externalAuthUrl = process.env.FN_KNOCK_A11Y_AUTH_URL;
  if (externalAdminUrl || externalAuthUrl) {
    if (!externalAdminUrl || !externalAuthUrl) {
      throw new Error(
        "Set both FN_KNOCK_A11Y_ADMIN_URL and FN_KNOCK_A11Y_AUTH_URL.",
      );
    }
    await Promise.all([
      waitForHttp(externalAdminUrl),
      waitForHttp(externalAuthUrl),
    ]);
    return {
      adminUrl: externalAdminUrl.replace(/\/+$/, ""),
      authUrl: externalAuthUrl.replace(/\/+$/, ""),
      stop: async () => {},
    };
  }

  await ensureRuntimeArtifacts();
  const tempDir = await mkdtemp(path.join(os.tmpdir(), "fn-knock-a11y-"));
  const [adminPort, authPort, goBackendPort] = await Promise.all([
    getFreePort(),
    getFreePort(),
    getFreePort(),
  ]);
  const child = spawn(
    path.join(
      rootDir,
      "apps/server-admin-rs/target/release/server-admin-rs",
    ),
    [],
    {
      cwd: rootDir,
      env: {
        ...process.env,
        ADMIN_STATIC_PATH: path.join(rootDir, "apps/server-admin-view/dist"),
        AUTH_HOST: "127.0.0.1",
        AUTH_PORT: String(authPort),
        AUTH_STATIC_PATH: path.join(rootDir, "apps/server-auth-view/dist"),
        BACKEND_HOST: "127.0.0.1",
        BACKEND_PORT: String(adminPort),
        EXPOSE_RUNTIME_HMAC_SECRET: "1",
        FN_KNOCK_DATA_DIR: tempDir,
        FN_KNOCK_GATEWAY_CONFIG_DIR: path.join(tempDir, "gateway"),
        FN_KNOCK_INTERNAL_RPC_TOKEN: "a11y-audit-internal-token",
        FN_KNOCK_RUNTIME_TARGET: "fpk-lite",
        FN_KNOCK_SQLITE_PATH: path.join(tempDir, "state.sqlite3"),
        GO_BACKEND_PORT: String(goBackendPort),
        HMAC_SECRET: "a11y-audit-hmac-secret",
        RUST_LOG: "error",
      },
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  let output = "";
  const appendOutput = (chunk) => {
    output = `${output}${chunk}`.slice(-12_000);
  };
  child.stdout.on("data", appendOutput);
  child.stderr.on("data", appendOutput);

  const adminUrl = `http://127.0.0.1:${adminPort}`;
  const authUrl = `http://127.0.0.1:${authPort}`;
  try {
    await Promise.all([waitForHttp(adminUrl), waitForHttp(authUrl)]);
  } catch (error) {
    child.kill("SIGTERM");
    await rm(tempDir, { recursive: true, force: true });
    throw new Error(`${error.message}\n${output}`);
  }

  return {
    adminUrl,
    authUrl,
    stop: async () => {
      if (child.exitCode === null) {
        child.kill("SIGTERM");
        await Promise.race([
          new Promise((resolve) => child.once("exit", resolve)),
          new Promise((resolve) => setTimeout(resolve, 3_000)),
        ]);
      }
      await rm(tempDir, { recursive: true, force: true });
    },
  };
};

const disableMotion = async (page) => {
  await page.evaluate(() => {
    if (document.getElementById("a11y-disable-motion")) return;
    const style = document.createElement("style");
    style.id = "a11y-disable-motion";
    style.textContent =
      "*,*::before,*::after{animation:none!important;transition:none!important}";
    document.head.appendChild(style);
  });
};

const auditPage = async (page, scope) => {
  await disableMotion(page);
  const result = await new AxeBuilder({ page }).withTags(wcagTags).analyze();
  for (const violation of result.violations) {
    failures.push({
      scope,
      message: `${violation.id}: ${violation.help}`,
      targets: violation.nodes.map((node) => node.target.join(" ")),
    });
  }
};

const applyThemeVariant = async (page, colorScheme, themeColorPreset) => {
  await page.evaluate(
    ({ mode, preset }) => {
      document.documentElement.classList.toggle("dark", mode === "dark");
      document.documentElement.dataset.themeColor = preset;
    },
    { mode: colorScheme, preset: themeColorPreset },
  );
  await disableMotion(page);
};

const assertDocumentStructure = async (page, scope, expectedMain = true) => {
  const structure = await page.evaluate(() => ({
    h1Count: document.querySelectorAll("h1").length,
    mainCount: document.querySelectorAll("main").length,
    title: document.title.trim(),
  }));
  assert(structure.h1Count === 1, scope, `expected one h1, got ${structure.h1Count}`);
  if (expectedMain) {
    assert(
      structure.mainCount === 1,
      scope,
      `expected one main landmark, got ${structure.mainCount}`,
    );
  }
  assert(Boolean(structure.title), scope, "document title is empty");
};

const installCompletedWelcomeMock = async (page) => {
  await page.route("**/api/admin/config/welcome_guide", async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill(jsonResponse({ completed: true }));
      return;
    }
    await route.fallback();
  });
};

const scanAdminRoutes = async (browser, adminUrl) => {
  for (const colorScheme of ["light", "dark"]) {
    const context = await browser.newContext({
      colorScheme,
      viewport: { width: 1440, height: 900 },
    });
    await context.addInitScript((mode) => {
      window.localStorage.setItem("fn-knock:theme-mode", mode);
    }, colorScheme);
    for (const routePath of adminRoutes) {
      const page = await context.newPage();
      const scope = `admin ${colorScheme} ${routePath}`;
      await installCompletedWelcomeMock(page);
      try {
        await page.goto(`${adminUrl}/#${routePath}`, {
          waitUntil: "domcontentloaded",
        });
        await page.locator("#main-content").waitFor({ state: "attached" });
        await page.waitForTimeout(100);
        await assertDocumentStructure(page, scope);
        for (const themeColorPreset of themeColorPresets) {
          await applyThemeVariant(page, colorScheme, themeColorPreset);
          await auditPage(page, `${scope} ${themeColorPreset}`);
        }
      } catch (error) {
        failures.push({ scope, message: error.message });
      } finally {
        await page.close();
      }
    }
    await context.close();
  }
};

const testAdminKeyboardFlow = async (browser, adminUrl) => {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  await installCompletedWelcomeMock(page);
  await page.goto(`${adminUrl}/#/`, { waitUntil: "domcontentloaded" });
  await page.locator("#main-content").waitFor({ state: "attached" });

  await page.evaluate(() => {
    document.body.setAttribute("tabindex", "-1");
    document.body.focus();
    document.body.removeAttribute("tabindex");
  });
  await page.keyboard.press("Tab");
  const skipLink = await page.evaluate(() => {
    const active = document.activeElement;
    const style = active ? getComputedStyle(active) : null;
    return {
      href: active?.getAttribute("href"),
      outlineStyle: style?.outlineStyle,
      outlineWidth: style?.outlineWidth,
    };
  });
  assert(
    skipLink.href === "#main-content",
    "admin keyboard",
    "first Tab did not focus the skip link",
  );
  assert(
    skipLink.outlineStyle !== "none" && skipLink.outlineWidth !== "0px",
    "admin keyboard",
    "skip link has no visible focus indicator",
  );
  await page.keyboard.press("Enter");
  assert(
    await page.evaluate(() => document.activeElement?.id === "main-content"),
    "admin keyboard",
    "skip link did not focus main content",
  );
  assert(
    new URL(page.url()).hash === "#/",
    "admin keyboard",
    "skip link changed the hash-router route",
  );

  await page.evaluate(() => {
    window.location.hash = "#/whitelist";
  });
  await page.waitForURL("**/#/whitelist");
  await page.waitForTimeout(50);
  assert(
    await page.evaluate(() => document.activeElement?.id === "main-content"),
    "admin keyboard",
    "SPA route change did not focus main content",
  );

  const localeTrigger = page.locator('button[title="语言"]').last();
  await localeTrigger.click();
  const dialog = page.getByRole("dialog");
  await dialog.waitFor({ state: "visible" });
  assert(
    await page.evaluate(() => Boolean(document.activeElement?.closest('[role="dialog"]'))),
    "admin locale dialog",
    "initial focus is outside the dialog",
  );
  for (let index = 0; index < 20; index += 1) {
    await page.keyboard.press("Tab");
    assert(
      await page.evaluate(() => Boolean(document.activeElement?.closest('[role="dialog"]'))),
      "admin locale dialog",
      `focus escaped after ${index + 1} Tab presses`,
    );
  }
  await auditPage(page, "admin locale dialog");
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
  assert(
    await localeTrigger.evaluate((element) => document.activeElement === element),
    "admin locale dialog",
    "focus was not restored to the trigger",
  );
  await context.close();
};

const testWelcomeDialog = async (browser, adminUrl) => {
  const context = await browser.newContext({
    reducedMotion: "reduce",
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
  await page.route(
    "**/api/admin/config/welcome_guide/complete",
    (route) => route.fulfill(jsonResponse({ completed: true })),
  );
  await page.route("**/api/admin/config/welcome_guide", async (route) => {
    if (route.request().method() === "GET") {
      await route.fulfill(jsonResponse({ completed: false }));
      return;
    }
    await route.fallback();
  });
  await page.goto(`${adminUrl}/#/`, { waitUntil: "domcontentloaded" });
  const dialog = page.locator('dialog[aria-modal="true"]');
  await dialog.waitFor({ state: "visible" });
  await dialog.locator("button").waitFor({ state: "visible" });

  const initialState = await page.evaluate(() => ({
    activeInside: Boolean(document.activeElement?.closest("dialog")),
    backgroundHidden:
      document.querySelector(".contents")?.getAttribute("aria-hidden") === "true",
    backgroundInert:
      document.querySelector(".contents")?.hasAttribute("inert") === true,
  }));
  assert(initialState.activeInside, "admin welcome dialog", "initial focus is outside");
  assert(
    initialState.backgroundHidden && initialState.backgroundInert,
    "admin welcome dialog",
    "background content is not hidden and inert",
  );
  await page.keyboard.press("Tab");
  await page.keyboard.press("Shift+Tab");
  assert(
    await page.evaluate(() => Boolean(document.activeElement?.closest("dialog"))),
    "admin welcome dialog",
    "focus escaped the welcome dialog",
  );
  await auditPage(page, "admin welcome dialog");
  await dialog.locator("button").click();
  await dialog.waitFor({ state: "hidden" });
  assert(
    await page.evaluate(
      () =>
        document.activeElement?.id === "main-content" &&
        !document.querySelector(".contents")?.hasAttribute("inert"),
    ),
    "admin welcome dialog",
    "focus or background state was not restored",
  );
  await context.close();
};

const authBootstrap = (loginMode) => ({
  locale: { default_locale: "zh-CN" },
  appearance: { theme_color_preset: "default" },
  auth: {
    authenticated: false,
    message: "",
    login_mode: loginMode,
  },
  client: { ip: "127.0.0.1" },
  captcha: {
    provider: "pow",
    widget_mode: "normal",
    available: true,
    unavailable_reason: null,
    pow: {},
    turnstile: { site_key: "" },
  },
  passkey: { available: false },
  oidc: {
    providers: [{ id: "audit-oidc", type: "oidc", name: "Audit OIDC" }],
  },
});

const installAuthLocationMock = async (page) => {
  await page.route("**/api/auth/ip/location?*", (route) =>
    route.fulfill(
      jsonResponse({
        ip: "127.0.0.1",
        location: "Local",
        status: "success",
        attempts: 1,
        maxAttempts: 1,
      }),
    ),
  );
};

const scanAuthLoginStates = async (browser, authUrl) => {
  for (const colorScheme of ["light", "dark"]) {
    const context = await browser.newContext({ colorScheme });
    await context.addInitScript((mode) => {
      window.localStorage.setItem("fn-knock:theme-mode", mode);
    }, colorScheme);
    const page = await context.newPage();
    const scope = `auth ${colorScheme} /login unavailable`;
    try {
      await page.goto(`${authUrl}/login`, { waitUntil: "domcontentloaded" });
      await page.locator("h1").waitFor({ state: "visible" });
      await assertDocumentStructure(page, scope);
      for (const themeColorPreset of themeColorPresets) {
        await applyThemeVariant(page, colorScheme, themeColorPreset);
        await auditPage(page, `${scope} ${themeColorPreset}`);
      }
    } catch (error) {
      failures.push({ scope, message: error.message });
    }
    await context.close();
  }

  for (const loginMode of ["password", "totp"]) {
    for (const colorScheme of ["light", "dark"]) {
      const context = await browser.newContext({ colorScheme });
      await context.addInitScript((mode) => {
        window.localStorage.setItem("fn-knock:theme-mode", mode);
      }, colorScheme);
      const page = await context.newPage();
      const scope = `auth ${colorScheme} /login ${loginMode}`;
      await page.route("**/api/auth/bootstrap?*", (route) =>
        route.fulfill(jsonResponse(authBootstrap(loginMode))),
      );
      await installAuthLocationMock(page);
      try {
        await page.goto(`${authUrl}/login`, { waitUntil: "domcontentloaded" });
        const widget = page.locator("altcha-widget");
        await widget.waitFor({ state: "attached" });
        await widget.evaluate((element) => {
          element.dispatchEvent(
            new CustomEvent("statechange", {
              detail: { state: "verified", payload: "a11y-proof" },
            }),
          );
        });
        await page.locator('button[type="submit"]').waitFor({ state: "visible" });
        if (loginMode === "password") {
          assert(
            (await page.locator('input[autocomplete="username"]').count()) === 1 &&
              (await page.locator('input[autocomplete="current-password"]').count()) === 1,
            scope,
            "password fields were not rendered",
          );
        } else {
          assert(
            (await page.locator('[autocomplete="one-time-code"]').count()) > 0,
            scope,
            "TOTP input was not rendered",
          );
        }
        await assertDocumentStructure(page, scope);
        for (const themeColorPreset of themeColorPresets) {
          await applyThemeVariant(page, colorScheme, themeColorPreset);
          await auditPage(page, `${scope} ${themeColorPreset}`);
        }
      } catch (error) {
        failures.push({ scope, message: error.message });
      }
      await context.close();
    }
  }
};

const authSession = {
  locale: { default_locale: "zh-CN" },
  appearance: { theme_color_preset: "default" },
  auth: {
    authenticated: true,
    message: "",
    grant_type: "browser_session",
    login_mode: "password",
  },
  client: { ip: "127.0.0.1" },
  passkey: { available: false },
  oidc: { providers: [] },
};

const testAuthHomeDialog = async (browser, authUrl) => {
  const context = await browser.newContext();
  const page = await context.newPage();
  await page.route("**/api/auth/session?*", (route) =>
    route.fulfill(jsonResponse(authSession)),
  );
  await installAuthLocationMock(page);
  await page.goto(`${authUrl}/`, { waitUntil: "domcontentloaded" });
  await page.locator("h1").waitFor({ state: "visible" });
  await assertDocumentStructure(page, "auth home");
  await auditPage(page, "auth home");

  const trigger = page.getByRole("button", {
    name: /退出|log out|logout|ログアウト|로그아웃/i,
  });
  await trigger.click();
  const dialog = page.getByRole("dialog");
  await dialog.waitFor({ state: "visible" });
  assert(
    await page.evaluate(() => Boolean(document.activeElement?.closest('[role="dialog"]'))),
    "auth logout dialog",
    "initial focus is outside the dialog",
  );
  for (let index = 0; index < 12; index += 1) {
    await page.keyboard.press("Tab");
    assert(
      await page.evaluate(() => Boolean(document.activeElement?.closest('[role="dialog"]'))),
      "auth logout dialog",
      `focus escaped after ${index + 1} Tab presses`,
    );
  }
  await auditPage(page, "auth logout dialog");
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
  assert(
    await trigger.evaluate((element) => document.activeElement === element),
    "auth logout dialog",
    "focus was not restored to the trigger",
  );
  await context.close();
};

const oidcInvite = {
  locale: { default_locale: "zh-CN" },
  appearance: { theme_color_preset: "default" },
  totp: { id: "audit-totp", comment: "Audit account" },
  expires_at: "2099-01-01T00:00:00Z",
  providers: [{ id: "audit-provider", type: "oidc", name: "Audit OIDC" }],
};

const scanAuthRoutes = async (browser, authUrl) => {
  const routePaths = [
    "/oidc/bind?token=a11y",
    "/auth/oidc/bind?token=a11y",
    "/__auth__/oidc/bind?token=a11y",
    "/a11y-not-found",
  ];
  for (const routePath of routePaths) {
    const context = await browser.newContext();
    const page = await context.newPage();
    const scope = `auth ${routePath}`;
    const assetFailures = [];
    page.on("response", (response) => {
      const type = response.request().resourceType();
      if (
        ["script", "stylesheet"].includes(type) &&
        response.status() >= 400
      ) {
        assetFailures.push(`${response.status()} ${response.url()}`);
      }
    });
    await page.route("**/api/auth/oidc/invite?*", (route) =>
      route.fulfill(jsonResponse(oidcInvite)),
    );
    try {
      await page.goto(`${authUrl}${routePath}`, {
        waitUntil: "domcontentloaded",
      });
      await page.locator("h1").waitFor({ state: "visible" });
      await assertDocumentStructure(page, scope);
      await auditPage(page, scope);
      assert(
        assetFailures.length === 0,
        scope,
        `failed script or stylesheet loads: ${assetFailures.join(", ")}`,
      );
    } catch (error) {
      failures.push({ scope, message: error.message });
    }
    await context.close();
  }
};

let runtime;
let browser;
try {
  runtime = await startRuntime();
  browser = await chromium.launch({ headless: true });
  await scanAdminRoutes(browser, runtime.adminUrl);
  await testAdminKeyboardFlow(browser, runtime.adminUrl);
  await testWelcomeDialog(browser, runtime.adminUrl);
  await scanAuthLoginStates(browser, runtime.authUrl);
  await testAuthHomeDialog(browser, runtime.authUrl);
  await scanAuthRoutes(browser, runtime.authUrl);
} finally {
  await browser?.close();
  await runtime?.stop();
}

if (failures.length > 0) {
  console.error(JSON.stringify(failures, null, 2));
  process.exitCode = 1;
} else {
  console.log(
    `[a11y] passed: ${adminRoutes.length * 2 * themeColorPresets.length} ` +
      "admin route/theme scans, " +
      "auth route/state scans, and keyboard/focus flows; 0 violations",
  );
}
