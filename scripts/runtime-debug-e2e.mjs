import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { chromium } from "playwright";
import { fetchRuntime, startRuntime } from "./runtime-test-harness.mjs";

const password = "runtime123";
const debugPath = "/runtime-health/debug";
const capturePath = `${debugPath}/capture`;
const memoryPath = `${debugPath}/memory`;
const pollingQuietWindowMs = 2_500;
const artifactDir = process.env.FN_KNOCK_RUNTIME_DEBUG_E2E_OUTPUT_DIR
  ? path.resolve(process.env.FN_KNOCK_RUNTIME_DEBUG_E2E_OUTPUT_DIR)
  : await mkdtemp(path.join(os.tmpdir(), "fn-knock-debug-evidence-"));

const apiRequest = async (page, requestPath, method = "GET") => {
  const result = await page.evaluate(
    async ({ requestPath, method }) => {
      const response = await fetch(`/api/admin${requestPath}`, {
        method,
        credentials: "include",
        signal: AbortSignal.timeout(10_000),
      });
      return { status: response.status, body: await response.json() };
    },
    { requestPath, method },
  );
  assert.equal(result.status, 200, `${method} ${requestPath} failed`);
  assert.ok(result.body.success, `${method} ${requestPath} was unsuccessful`);
  return result.body.data;
};

const submitGatePassword = async (page, autocomplete) => {
  const input = page.locator(`input[autocomplete="${autocomplete}"]`);
  await input.waitFor({ state: "visible" });
  await input.fill(password);
  if (autocomplete === "current-password") {
    await page.locator("#dockerAdminRememberMe").click();
  }
  await page.locator('form button[type="submit"]').click();
  await page.locator("#main-content").waitFor({ state: "visible" });
};

const waitForApi = (page, requestPath, method = "GET") =>
  page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === `/api/admin${requestPath}` &&
      response.request().method() === method,
  );

const clickAndRead = async (page, button, requestPath, method) => {
  const [response] = await Promise.all([
    waitForApi(page, requestPath, method),
    button.click(),
  ]);
  assert.equal(response.status(), 200, `${method} ${requestPath} failed`);
  const body = await response.json();
  assert.ok(body.success);
  return body.data;
};

const assertCaptureIdle = (report) => {
  assert.equal(report.capture.status, "idle");
  assert.equal(report.capture.id, null);
  assert.deepEqual(report.capture.samples, []);
  assert.equal(report.capture.operations.active, false);
  assert.deepEqual(report.capture.operations.operations, []);
};

const assertNoSensitiveTestValues = (value) => {
  const serialized = JSON.stringify(value);
  for (const marker of [
    password,
    "runtime-audit-internal-token",
    "runtime-audit-hmac-secret",
  ]) {
    assert.ok(!serialized.includes(marker), "diagnostics exposed a credential");
  }
};

const assertSamples = (capture, runtimeOs) => {
  assert.ok(capture.samples.length >= 50, "too few samples in the real minute");
  assert.ok(capture.samples.length <= 61, "capture exceeded its sample bound");
  assert.equal(capture.samples[0].resource.cpu_percent, null);
  if (["linux", "macos"].includes(runtimeOs)) {
    assert.ok(
      capture.samples
        .slice(1)
        .some((sample) => Number.isFinite(sample.resource.cpu_percent)),
      "capture never measured a CPU delta",
    );
  } else {
    assert.ok(
      capture.samples.every(
        (sample) =>
          sample.resource.cpu_percent === null &&
          sample.resource.errors.includes("process_cpu_unsupported"),
      ),
      "unsupported platform did not explicitly report missing CPU support",
    );
  }
  for (const [index, sample] of capture.samples.entries()) {
    assert.ok(sample.elapsed_ms >= 0);
    if (index > 0) {
      assert.ok(sample.elapsed_ms >= capture.samples[index - 1].elapsed_ms);
    }
    assert.ok(sample.resource.thread_cpu.length <= 8);
    assert.ok(
      sample.resource.cpu_percent === null || sample.resource.cpu_percent >= 0,
    );
    assert.ok(
      sample.resource.rss_bytes === null || sample.resource.rss_bytes > 0,
    );
  }
};

let runtime;
let browser;
let page;
let visibilityRestore;
const checks = [];
const pageErrors = [];
const summary = {
  started_at: new Date().toISOString(),
  checks,
  unauthenticated_methods: [],
  artifacts: {},
};

const passed = (name) => {
  checks.push(name);
  console.log(`[runtime-debug-e2e] ${name}`);
};

try {
  await mkdir(artifactDir, { recursive: true });
  // Deliberately do not accept an external URL: setup and mutations belong only
  // to the isolated runtime and its temporary SQLite database.
  console.log("[runtime-debug-e2e] starting isolated Rust/Go runtime");
  runtime = await startRuntime({
    gatewayBinary:
      process.env.FN_KNOCK_RUNTIME_E2E_GATEWAY_BIN ??
      process.env.FN_KNOCK_A11Y_GATEWAY_BIN,
    protectedAdmin: true,
    tempPrefix: "fn-knock-runtime-debug-e2e-",
  });
  console.log(
    "[runtime-debug-e2e] runtime ready; checking protected setup/login",
  );
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({
    locale: "zh-CN",
    viewport: { width: 1440, height: 1080 },
    acceptDownloads: true,
  });
  page = await context.newPage();
  page.setDefaultTimeout(20_000);
  page.on("pageerror", (error) => pageErrors.push(error.message));
  let debugReads = 0;
  page.on("request", (request) => {
    if (
      request.method() === "GET" &&
      new URL(request.url()).pathname === `/api/admin${debugPath}`
    ) {
      debugReads += 1;
    }
  });

  await page.goto(runtime.adminUrl, { waitUntil: "domcontentloaded" });
  await submitGatePassword(page, "new-password");
  await page.goto("about:blank");
  await context.clearCookies();

  for (const [method, requestPath] of [
    ["GET", debugPath],
    ["POST", capturePath],
    ["DELETE", capturePath],
    ["POST", memoryPath],
  ]) {
    const response = await fetchRuntime(
      `${runtime.adminUrl}/api/admin${requestPath}`,
      { method, redirect: "manual" },
    );
    const status = response.status;
    await response.body?.cancel();
    assert.ok(
      status === 401 || status === 403,
      `unauthenticated ${method} ${requestPath} returned ${status}`,
    );
    summary.unauthenticated_methods.push({ method, path: requestPath, status });
  }
  passed("all four debug methods reject unauthenticated requests");

  await page.goto(runtime.adminUrl, { waitUntil: "domcontentloaded" });
  await submitGatePassword(page, "current-password");
  assertCaptureIdle(await apiRequest(page, debugPath));
  assertCaptureIdle(await apiRequest(page, debugPath));
  passed("protected setup/login succeeds; initial GET never starts capture");

  await page.goto(`${runtime.adminUrl}/#/events?tab=runtime`, {
    waitUntil: "domcontentloaded",
  });
  const runtimeTab = page.getByRole("tab", { name: "状态", exact: true });
  await runtimeTab.waitFor({ state: "visible" });
  assert.equal(await runtimeTab.getAttribute("aria-selected"), "true");
  const toolbarEntry = page.getByRole("button", {
    name: "运行诊断",
    exact: true,
  });
  const managementCard = page.locator("article").filter({
    has: page.getByText("管理服务", { exact: true }),
  });
  const managementEntry = managementCard.getByRole("button", {
    name: "查看诊断",
    exact: true,
  });
  await toolbarEntry.waitFor({ state: "visible" });
  await managementEntry.waitFor({ state: "visible" });
  const screenshot = async (fileName) => {
    // Vue route transitions can still be fading after the target is visible.
    await delay(350);
    const filePath = path.join(artifactDir, fileName);
    await page.screenshot({
      path: filePath,
      fullPage: false,
      animations: "disabled",
    });
    return filePath;
  };
  summary.artifacts.status_desktop = await screenshot("status-desktop.png");

  const dialog = page.getByRole("dialog", {
    name: "Rust 运行诊断",
    exact: true,
  });
  const openDialog = async (button) => {
    const [response] = await Promise.all([
      waitForApi(page, debugPath),
      button.click(),
      dialog.waitFor({ state: "visible" }),
    ]);
    assert.equal(response.status(), 200);
    return (await response.json()).data;
  };
  const closeDialog = async () => {
    await page.keyboard.press("Escape");
    await dialog.waitFor({ state: "hidden" });
  };
  const assertNoPolling = async (label) => {
    // Allow a request already emitted before close/visibilitychange to settle.
    await delay(150);
    const before = debugReads;
    await delay(pollingQuietWindowMs);
    assert.equal(debugReads, before, `${label} continued debug GET polling`);
  };

  assertCaptureIdle(await openDialog(toolbarEntry));
  const readsWhenOpen = debugReads;
  await delay(pollingQuietWindowMs);
  assert.ok(debugReads > readsWhenOpen, "visible dialog did not poll");
  await closeDialog();
  await assertNoPolling("closed dialog");
  assertCaptureIdle(await openDialog(managementEntry));
  passed(
    "status toolbar and management card open the dialog; close stops polling",
  );

  const startButton = dialog.getByRole("button", {
    name: "开始 60 秒采样",
    exact: true,
  });
  const stopButton = dialog.getByRole("button", {
    name: "停止并保留结果",
    exact: true,
  });
  const first = await clickAndRead(page, startButton, capturePath, "POST");
  assert.equal(first.capture.status, "running");
  assert.ok(first.capture.id);
  assert.equal(first.capture.operations.active, true);
  const repeated = await apiRequest(page, capturePath, "POST");
  assert.equal(repeated.capture.id, first.capture.id);
  await apiRequest(page, "/config/appearance");
  await delay(1_250);
  const stopped = await clickAndRead(page, stopButton, capturePath, "DELETE");
  assert.equal(stopped.capture.status, "stopped");
  assert.equal(stopped.capture.operations.active, false);
  await delay(2_100);
  const laterStopped = await apiRequest(page, debugPath);
  assert.deepEqual(
    laterStopped.capture,
    stopped.capture,
    "stopped report changed",
  );
  summary.manual_stop_report_frozen = true;
  passed(
    "repeated start is idempotent and stop freezes samples and operations",
  );

  const memory = await clickAndRead(
    page,
    dialog.getByRole("button", { name: "采集内存详情", exact: true }),
    memoryPath,
    "POST",
  );
  assert.ok(memory.memory?.collected_at);
  assert.ok(
    ["available", "partial", "unsupported", "unavailable"].includes(
      memory.memory.status,
    ),
  );
  assert.ok(memory.memory.largest_anonymous_regions.length <= 8);
  assert.equal(memory.capture.status, "stopped");
  assert.equal(memory.memory_refreshing, false);
  assertNoSensitiveTestValues(memory);
  summary.memory_status = memory.memory.status;
  passed("manual memory collection is bounded and does not restart capture");

  const fullStartedAt = performance.now();
  const full = await clickAndRead(page, startButton, capturePath, "POST");
  assert.equal(full.capture.status, "running");
  assert.notEqual(full.capture.id, first.capture.id);
  await apiRequest(page, "/config/appearance");
  await closeDialog();
  await assertNoPolling("dialog closed during capture");
  assert.equal((await openDialog(toolbarEntry)).capture.id, full.capture.id);

  // Prefer real browser visibility. Headless Chromium builds that keep every
  // tab visible use a documented DOM visibilitychange simulation; record which
  // mode ran rather than claiming an OS-level background transition occurred.
  const foreground = await context.newPage();
  await foreground.goto("about:blank");
  await foreground.bringToFront();
  const nativeHidden = await page.evaluate(() => document.hidden);
  if (nativeHidden) {
    summary.background_visibility_mode = "native_background_tab";
    visibilityRestore = async () => {
      await page.bringToFront();
      await foreground.close();
    };
  } else {
    summary.background_visibility_mode = "simulated_visibilitychange";
    await page.evaluate(() => {
      window.__runtimeDebugVisibilityDescriptor =
        Object.getOwnPropertyDescriptor(document, "hidden");
      Object.defineProperty(document, "hidden", {
        configurable: true,
        get: () => true,
      });
      document.dispatchEvent(new Event("visibilitychange"));
    });
    visibilityRestore = async () => {
      await page.evaluate(() => {
        const descriptor = window.__runtimeDebugVisibilityDescriptor;
        if (descriptor) Object.defineProperty(document, "hidden", descriptor);
        else delete document.hidden;
        delete window.__runtimeDebugVisibilityDescriptor;
        document.dispatchEvent(new Event("visibilitychange"));
      });
      await page.bringToFront();
      await foreground.close();
    };
  }
  await assertNoPolling("background dialog");
  const [resumed] = await Promise.all([
    waitForApi(page, debugPath),
    visibilityRestore(),
  ]);
  visibilityRestore = null;
  assert.equal(resumed.status(), 200);
  await closeDialog();
  passed(
    `background visibility stops polling (${summary.background_visibility_mode})`,
  );

  let completed;
  let lastProgressAt = performance.now();
  const completionDeadline = fullStartedAt + 75_000;
  while (performance.now() < completionDeadline) {
    const report = await apiRequest(page, debugPath);
    assert.equal(report.capture.id, full.capture.id);
    if (report.capture.status === "completed") {
      completed = report;
      break;
    }
    assert.equal(report.capture.status, "running");
    if (performance.now() - lastProgressAt >= 15_000) {
      console.log(
        `[runtime-debug-e2e] real capture elapsed ${report.capture.elapsed_ms} ms`,
      );
      lastProgressAt = performance.now();
    }
    await delay(2_000);
  }
  assert.ok(completed, "capture failed to complete within its deadline");
  const actualDuration = performance.now() - fullStartedAt;
  assert.ok(
    actualDuration >= 59_000,
    "capture finished before a real minute elapsed",
  );
  assert.ok(completed.capture.elapsed_ms >= 59_000);
  assert.equal(completed.capture.operations.active, false);
  assertSamples(completed.capture, completed.process.os);
  assert.ok(
    completed.capture.operations.operations.some((row) => row.calls > 0),
  );
  await delay(2_100);
  assert.deepEqual(
    (await apiRequest(page, debugPath)).capture,
    completed.capture,
  );
  summary.completed_report_frozen = true;
  assertNoSensitiveTestValues(completed);
  summary.auto_stop_elapsed_ms = Math.round(actualDuration);
  summary.sample_count = completed.capture.samples.length;
  summary.operation_labels = completed.capture.operations.operations.map(
    (row) => row.label,
  );
  passed("real 60-second capture auto-stops with bounded, frozen measurements");

  await openDialog(toolbarEntry);
  await dialog
    .getByText("已完成", { exact: true })
    .waitFor({ state: "visible" });
  summary.artifacts.debug_desktop = await screenshot("debug-desktop.png");
  const sqliteHeading = dialog.getByRole("heading", {
    name: "SQLite 操作",
    exact: true,
  });
  await sqliteHeading.scrollIntoViewIfNeeded();
  summary.artifacts.operations_desktop = await screenshot(
    "operations-desktop.png",
  );
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    dialog.getByRole("button", { name: "导出 JSON", exact: true }).click(),
  ]);
  assert.match(
    download.suggestedFilename(),
    /^fn-knock-runtime-debug-\d+-\d+\.json$/,
  );
  summary.artifacts.report = path.join(
    artifactDir,
    "runtime-debug-report.json",
  );
  await download.saveAs(summary.artifacts.report);
  const exported = JSON.parse(await readFile(summary.artifacts.report, "utf8"));
  assert.deepEqual(exported.capture, completed.capture);
  assertNoSensitiveTestValues(exported);

  const diagnostics = await apiRequest(page, "/runtime-health/diagnostics");
  const oldExportOperations =
    diagnostics.runtime_debug.capture.operations.operations;
  assert.deepEqual(
    oldExportOperations.map(({ kind, label }) => ({ kind, label })),
    exported.capture.operations.operations.map(({ kind, label }) => ({
      kind,
      label,
    })),
    "existing diagnostics export redacted or lost operation names",
  );
  assertNoSensitiveTestValues(diagnostics);
  passed(
    "JSON download matches capture; existing diagnostics preserves operation names",
  );

  await page.setViewportSize({ width: 390, height: 844 });
  await delay(200);
  const dialogScroll = dialog.locator(".overflow-y-auto").first();
  await dialogScroll.evaluate((element) => {
    element.scrollTop = 0;
  });
  const bounds = await dialog.boundingBox();
  assert.ok(
    bounds && bounds.x >= -1 && bounds.x + bounds.width <= 391,
    "mobile dialog overflows the viewport",
  );
  summary.artifacts.debug_mobile = await screenshot("debug-mobile.png");
  await sqliteHeading.scrollIntoViewIfNeeded();
  const operationSection = sqliteHeading.locator("xpath=ancestor::section[1]");
  const operationBounds = await operationSection.boundingBox();
  assert.ok(
    operationBounds &&
      operationBounds.x >= bounds.x &&
      operationBounds.x + operationBounds.width <= bounds.x + bounds.width,
    "long operation labels expand the mobile dialog",
  );
  summary.artifacts.operations_mobile = await screenshot(
    "operations-mobile.png",
  );
  await page.evaluate(() => {
    window.location.hash = "#/events?tab=events";
  });
  // The default Events tab is canonicalized to #/events without a query.
  await page.waitForFunction(() => {
    const [pathname, query] = window.location.hash.slice(1).split("?");
    return (
      pathname === "/events" &&
      (new URLSearchParams(query).get("tab") ?? "events") === "events" &&
      [...document.querySelectorAll('[role="tab"]')].some(
        (tab) =>
          tab.textContent.trim() === "事件" &&
          tab.getAttribute("aria-selected") === "true",
      )
    );
  });
  await assertNoPolling("inactive runtime tab");
  passed(
    "390px dialog stays within viewport; leaving runtime tab stops polling",
  );

  assert.deepEqual(
    pageErrors,
    [],
    "browser emitted unhandled JavaScript errors",
  );
  summary.page_errors = pageErrors;
  summary.page_error_count = pageErrors.length;
  summary.status = "passed";
  summary.finished_at = new Date().toISOString();
  await writeFile(
    path.join(artifactDir, "summary.json"),
    JSON.stringify(summary, null, 2) + "\n",
  );
  console.log(`[runtime-debug-e2e] passed; artifacts: ${artifactDir}`);
} catch (error) {
  summary.status = "failed";
  summary.error = String(error?.stack ?? error);
  summary.page_errors = pageErrors;
  summary.page_error_count = pageErrors.length;
  summary.failed_url = page?.url();
  summary.finished_at = new Date().toISOString();
  if (page && !page.isClosed()) {
    await page
      .screenshot({
        path: path.join(artifactDir, "failure.png"),
        fullPage: true,
      })
      .catch(() => {});
  }
  await writeFile(
    path.join(artifactDir, "summary.json"),
    JSON.stringify(summary, null, 2) + "\n",
  );
  throw error;
} finally {
  await visibilityRestore?.().catch(() => {});
  try {
    await browser?.close();
  } finally {
    await runtime?.stop();
  }
}
