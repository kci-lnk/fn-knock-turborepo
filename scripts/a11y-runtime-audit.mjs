import AxeBuilder from "@axe-core/playwright";
import { chromium } from "playwright";
import { startRuntime as startRuntimeHarness } from "./runtime-test-harness.mjs";
const wcagTags = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"];
const themeColorPresets = [
  "default",
  "hermes_orange",
  "prussian_blue",
  "dynamic_white",
];
const affordanceOnly = process.env.FN_KNOCK_A11Y_AFFORDANCE_ONLY === "1";
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
  "/request-analysis",
  "/waf-logs",
  "/system",
  "/system?tab=maintenance",
  "/system/gateway-visibility",
  "/system/gateway-portal",
  "/system/gateway-proxy-headers",
  "/system/gateway-host-response",
  "/subdomains/a11y.invalid/paths",
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

const startRuntime = async () => {
  const externalAdminUrl = process.env.FN_KNOCK_A11Y_ADMIN_URL;
  const externalAuthUrl = process.env.FN_KNOCK_A11Y_AUTH_URL;
  return startRuntimeHarness({
    externalAdminUrl,
    externalAuthUrl,
    gatewayBinary: process.env.FN_KNOCK_A11Y_GATEWAY_BIN,
    tempPrefix: "fn-knock-a11y-",
  });
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

const auditPage = async (page, scope, include) => {
  if (affordanceOnly) return;
  await disableMotion(page);
  const builder = new AxeBuilder({ page })
    .withTags(wcagTags)
    .disableRules(["color-contrast"]);
  if (include) builder.include(include);
  const result = await builder.analyze();
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

const assertInteractiveAffordances = async (page, scope) => {
  const result = await page.evaluate(() => {
    const isVisible = (element) => {
      const style = getComputedStyle(element);
      const rect = element.getBoundingClientRect();
      return (
        style.display !== "none" &&
        style.visibility !== "hidden" &&
        rect.width > 0 &&
        rect.height > 0
      );
    };
    const label = (element) =>
      (
        element.getAttribute("aria-label") ||
        element.textContent ||
        element.tagName
      )
        .trim()
        .replace(/\s+/g, " ")
        .slice(0, 80);
    const enabledButtons = Array.from(
      document.querySelectorAll(
        'button, [role="button"], [data-slot="button"]',
      ),
    ).filter(
      (element) =>
        isVisible(element) &&
        !element.matches(":disabled") &&
        element.getAttribute("aria-disabled") !== "true",
    );
    const pointerFailures = enabledButtons
      .filter((element) =>
        ["auto", "default"].includes(getComputedStyle(element).cursor),
      )
      .map(label);
    const linkFailures = enabledButtons
      .filter(
        (element) =>
          element.getAttribute("data-variant") === "link" &&
          !getComputedStyle(element).textDecorationLine.includes("underline"),
      )
      .map(label);
    const helpFailures = Array.from(
      document.querySelectorAll('[data-affordance="help"]'),
    )
      .filter(isVisible)
      .filter((element) => {
        const style = getComputedStyle(element);
        return (
          !style.textDecorationLine.includes("underline") ||
          style.textDecorationStyle !== "dotted"
        );
      })
      .map(label);
    const actionFailures = Array.from(
      document.querySelectorAll(
        '[data-affordance="copy"], [data-affordance="edit"], [data-affordance="details"]',
      ),
    )
      .filter(
        (element) =>
          isVisible(element) &&
          !element.matches(":disabled") &&
          element.getAttribute("aria-disabled") !== "true",
      )
      .filter((element) => {
        const style = getComputedStyle(element);
        const hasIcon = Boolean(element.querySelector("svg, img"));
        const hasUnderline = style.textDecorationLine.includes("underline");
        const hasBorder = Number.parseFloat(style.borderTopWidth) > 0;
        const hasBackground =
          style.backgroundColor !== "rgba(0, 0, 0, 0)" &&
          style.backgroundColor !== "transparent";
        return !hasIcon && !hasUnderline && !hasBorder && !hasBackground;
      })
      .map(label);
    return {
      actionFailures,
      helpFailures,
      linkFailures,
      pointerFailures,
    };
  });

  assert(
    result.pointerFailures.length === 0,
    scope,
    `buttons missing pointer cursor: ${result.pointerFailures.join(", ")}`,
  );
  assert(
    result.linkFailures.length === 0,
    scope,
    `link buttons missing persistent underline: ${result.linkFailures.join(", ")}`,
  );
  assert(
    result.helpFailures.length === 0,
    scope,
    `help triggers missing dotted underline: ${result.helpFailures.join(", ")}`,
  );
  assert(
    result.actionFailures.length === 0,
    scope,
    `text actions missing a stable visual cue: ${result.actionFailures.join(", ")}`,
  );
};

const assertDocumentStructure = async (page, scope, expectedMain = true) => {
  const structure = await page.evaluate(() => ({
    h1Count: document.querySelectorAll("h1").length,
    mainCount: document.querySelectorAll("main").length,
    title: document.title.trim(),
  }));
  assert(
    structure.h1Count === 1,
    scope,
    `expected one h1, got ${structure.h1Count}`,
  );
  if (expectedMain) {
    assert(
      structure.mainCount === 1,
      scope,
      `expected one main landmark, got ${structure.mainCount}`,
    );
  }
  assert(Boolean(structure.title), scope, "document title is empty");
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
          await assertInteractiveAffordances(
            page,
            `${scope} ${themeColorPreset}`,
          );
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

const testAdminInteractiveAffordances = async (browser, adminUrl) => {
  const viewports = [
    { label: "desktop", width: 1440, height: 900 },
    { label: "mobile", width: 390, height: 844 },
  ];
  for (const viewport of viewports) {
    for (const colorScheme of ["light", "dark"]) {
      const context = await browser.newContext({
        colorScheme,
        viewport,
      });
      const page = await context.newPage();
      const scope = `admin affordances ${viewport.label} ${colorScheme}`;
      await page.route("**/api/admin/totp/status", (route) =>
        route.fulfill(
          jsonResponse({
            bound: true,
            credentials: [
              {
                id: "audit-totp",
                secret: "audit-secret",
                comment: "Audit token",
                createdAt: "2026-07-01T00:00:00Z",
                access_scopes: [],
                subdomain_access: {
                  mode: "all",
                  hosts: [],
                  streams: [],
                },
              },
            ],
          }),
        ),
      );
      await page.route("**/api/admin/auth/mode", (route) =>
        route.fulfill(
          jsonResponse({
            mode: "totp",
            totpCount: 1,
            accountCount: 0,
            passwordConfiguredCount: 0,
            passwordMissingCount: 0,
          }),
        ),
      );
      try {
        await page.goto(`${adminUrl}/#/auth`, {
          waitUntil: "domcontentloaded",
        });
        await page
          .getByRole("button", { name: "管理快捷登录" })
          .waitFor({ state: "visible" });
        for (const themeColorPreset of themeColorPresets) {
          await applyThemeVariant(page, colorScheme, themeColorPreset);
          await assertInteractiveAffordances(
            page,
            `${scope} ${themeColorPreset}`,
          );
        }
      } catch (error) {
        failures.push({ scope, message: error.message });
      }
      await context.close();
    }
  }
};

const testAdminKeyboardFlow = async (browser, adminUrl) => {
  const context = await browser.newContext({
    viewport: { width: 1440, height: 900 },
  });
  const page = await context.newPage();
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
      boxShadow: style?.boxShadow,
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
    (skipLink.outlineStyle !== "none" && skipLink.outlineWidth !== "0px") ||
      (skipLink.boxShadow && skipLink.boxShadow !== "none"),
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
    await page.evaluate(() =>
      Boolean(document.activeElement?.closest('[role="dialog"]')),
    ),
    "admin locale dialog",
    "initial focus is outside the dialog",
  );
  for (let index = 0; index < 20; index += 1) {
    await page.keyboard.press("Tab");
    assert(
      await page.evaluate(() =>
        Boolean(document.activeElement?.closest('[role="dialog"]')),
      ),
      "admin locale dialog",
      `focus escaped after ${index + 1} Tab presses`,
    );
  }
  await auditPage(page, "admin locale dialog");
  await page.keyboard.press("Escape");
  await dialog.waitFor({ state: "hidden" });
  assert(
    await localeTrigger.evaluate(
      (element) => document.activeElement === element,
    ),
    "admin locale dialog",
    "focus was not restored to the trigger",
  );
  await context.close();
};

const automaticBackupDetails = {
  config: {
    enabled: false,
    interval_hours: 24,
    retention_days: 7,
    updated_at: null,
  },
  status: {
    directory_path: "/data/backups/automatic",
    last_attempt_at: null,
    last_success_at: null,
    last_error: null,
    last_filename: null,
    next_backup_at: null,
  },
};

const testAutomaticBackupSettings = async (browser, adminUrl) => {
  const selector = '[data-a11y-scope="automatic-backup-settings"]';
  for (const colorScheme of ["light", "dark"]) {
    const context = await browser.newContext({
      colorScheme,
      viewport: { width: 1440, height: 900 },
    });
    const page = await context.newPage();
    await page.route(
      "**/api/admin/maintenance/backup/automatic",
      async (route) => {
        if (route.request().method() === "GET") {
          await route.fulfill(jsonResponse(automaticBackupDetails));
          return;
        }
        await route.fallback();
      },
    );
    const scope = `admin automatic backup ${colorScheme}`;
    try {
      await page.goto(`${adminUrl}/#/system?tab=maintenance`, {
        waitUntil: "domcontentloaded",
      });
      const region = page.locator(selector);
      await region.waitFor({ state: "visible" });

      assert(
        (await page.getByRole("region", { name: "自动备份" }).count()) === 1,
        scope,
        "settings region has no accessible name",
      );
      const backupSwitch = region.getByRole("switch");
      await backupSwitch.press("Space");
      assert(
        (await backupSwitch.getAttribute("aria-checked")) === "true",
        scope,
        "switch could not be operated from the keyboard",
      );

      const numericInputs = region.locator('input[type="number"]');
      assert(
        (await numericInputs.count()) === 2,
        scope,
        "expected interval and retention numeric inputs",
      );
      const intervalInput = numericInputs.first();
      await intervalInput.fill("0");
      assert(
        (await intervalInput.getAttribute("aria-invalid")) === "true",
        scope,
        "invalid interval is not exposed to assistive technology",
      );
      const describedBy = await intervalInput.getAttribute("aria-describedby");
      assert(
        Boolean(describedBy) &&
          (await region
            .locator(`[id="${describedBy}"][role="alert"]`)
            .count()) === 1,
        scope,
        "invalid interval is not connected to its inline error",
      );

      for (const themeColorPreset of themeColorPresets) {
        await applyThemeVariant(page, colorScheme, themeColorPreset);
        await auditPage(page, `${scope} ${themeColorPreset}`, selector);
      }
    } catch (error) {
      failures.push({ scope, message: error.message });
    }
    await context.close();
  }
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
        await assertInteractiveAffordances(
          page,
          `${scope} ${themeColorPreset}`,
        );
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
        await page
          .locator('button[type="submit"]')
          .waitFor({ state: "visible" });
        if (loginMode === "password") {
          assert(
            (await page.locator('input[autocomplete="username"]').count()) ===
              1 &&
              (await page
                .locator('input[autocomplete="current-password"]')
                .count()) === 1,
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
          await assertInteractiveAffordances(
            page,
            `${scope} ${themeColorPreset}`,
          );
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
    await page.evaluate(() =>
      Boolean(document.activeElement?.closest('[role="dialog"]')),
    ),
    "auth logout dialog",
    "initial focus is outside the dialog",
  );
  for (let index = 0; index < 12; index += 1) {
    await page.keyboard.press("Tab");
    assert(
      await page.evaluate(() =>
        Boolean(document.activeElement?.closest('[role="dialog"]')),
      ),
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
      if (["script", "stylesheet"].includes(type) && response.status() >= 400) {
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
  await testAdminInteractiveAffordances(browser, runtime.adminUrl);
  await scanAuthLoginStates(browser, runtime.authUrl);
  if (!affordanceOnly) {
    await testAdminKeyboardFlow(browser, runtime.adminUrl);
    await testAutomaticBackupSettings(browser, runtime.adminUrl);
    await testAuthHomeDialog(browser, runtime.authUrl);
    await scanAuthRoutes(browser, runtime.authUrl);
  }
} finally {
  await browser?.close();
  await runtime?.stop();
}

if (failures.length > 0) {
  console.error(JSON.stringify(failures, null, 2));
  process.exitCode = 1;
} else {
  console.log(
    affordanceOnly
      ? `[a11y] passed: ${adminRoutes.length * 2 * themeColorPresets.length} ` +
          "admin route/theme affordance scans plus desktop/mobile TOTP and " +
          "auth route/state affordance scans; 0 violations"
      : `[a11y] passed: ${adminRoutes.length * 2 * themeColorPresets.length} ` +
          "admin route/theme scans, auth route/state scans, and " +
          "keyboard/focus flows; 0 violations",
  );
}
