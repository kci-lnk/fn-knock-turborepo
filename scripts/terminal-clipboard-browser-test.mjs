// Run with node scripts/terminal-clipboard-browser-test.mjs.
// Set TERMINAL_TEST_BROWSER=firefox|webkit or TERMINAL_TEST_CHANNEL=chrome|msedge
// on hosts with those browsers installed; no backend/session credentials are needed.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import { chromium, firefox, webkit } from "playwright";

const root = fileURLToPath(
  new URL("../apps/server-admin-view/", import.meta.url),
);
const require = createRequire(
  new URL("../apps/server-admin-view/package.json", import.meta.url),
);
const { createServer } = await import(
  pathToFileURL(require.resolve("vite")).href
);
const fixture = await readFile(
  new URL("./fixtures/terminal-clipboard-browser.js", import.meta.url),
  "utf8",
);
const server = await createServer({
  root,
  logLevel: "error",
  server: { host: "127.0.0.1", port: 0 },
  plugins: [
    {
      name: "terminal-clipboard-test-fixture",
      configureServer(dev) {
        dev.middlewares.use(async (request, response, next) => {
          if (request.url === "/__terminal-copy.html") {
            response.setHeader("Content-Type", "text/html");
            response.end(
              await dev.transformIndexHtml(
                request.url,
                '<html><body><div id="app"></div><script type="module" src="/__terminal-copy-module.js"></script></body></html>',
              ),
            );
          } else next();
        });
      },
      resolveId(id) {
        if (id === "/__terminal-copy-module.js")
          return "\0terminal-copy-fixture";
      },
      load(id) {
        if (id === "\0terminal-copy-fixture") return fixture;
      },
    },
  ],
});
let browser;
try {
  await server.listen();
  const origin = server.resolvedUrls.local[0];
  const browserName = process.env.TERMINAL_TEST_BROWSER || "chromium";
  browser = await { chromium, firefox, webkit }[browserName].launch({
    headless: true,
    ...(process.env.TERMINAL_TEST_CHANNEL
      ? { channel: process.env.TERMINAL_TEST_CHANNEL }
      : {}),
  });
  const context = await browser.newContext({
    ...(browserName === "chromium"
      ? { permissions: ["clipboard-read", "clipboard-write"] }
      : {}),
  });
  const page = await context.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto(`${origin}__terminal-copy.html`);
  await page.waitForFunction(() => window.terminalTest);
  const nativeModifier = process.platform === "darwin" ? "Meta" : "Control";
  const select = () =>
    page.evaluate(() => {
      const { emulator } = window.terminalTest;
      emulator.getTerminal().select(0, 0, 7);
      emulator.focusTerminal();
    });
  const paste = async () => {
    await page.waitForFunction(
      () => !window.terminalTest.actions.copying.value,
    );
    const target = page.locator("#paste-target");
    await target.fill("");
    await target.press(`${nativeModifier}+v`);
    return target.inputValue();
  };
  const seedClipboard = async () => {
    const target = page.locator("#paste-target");
    await target.fill("PREVIOUS CLIPBOARD");
    await target.press(`${nativeModifier}+a`);
    await target.press(`${nativeModifier}+c`);
  };
  for (const shortcut of ["Meta+c", "Control+c", "Control+Shift+c"]) {
    await seedClipboard();
    await select();
    await page.keyboard.press(shortcut);
    await page.waitForFunction(
      () => !window.terminalTest.actions.copying.value,
    );
    assert.equal(await paste(), "LOG-123", shortcut);
  }
  assert.deepEqual(await page.evaluate(() => window.terminalTest.sent), []);

  // Exercise real canvas mouse coordinates, not only Ghostty's programmatic select().
  await seedClipboard();
  const canvas = page.locator("#terminal canvas");
  const rect = await canvas.boundingBox();
  const cols = await page.evaluate(
    () => window.terminalTest.emulator.getTerminal().cols,
  );
  assert.ok(rect);
  const cellWidth = rect.width / cols;
  await page.mouse.move(rect.x + cellWidth / 2, rect.y + 8);
  await page.mouse.down();
  await page.mouse.move(rect.x + cellWidth * 6.5, rect.y + 8, { steps: 7 });
  await page.mouse.up();
  assert.equal(
    await page.evaluate(() =>
      window.terminalTest.emulator.getTerminal().getSelection(),
    ),
    "LOG-123",
  );
  await page.keyboard.press(`${nativeModifier}+c`);
  assert.equal(
    await paste(),
    "LOG-123",
    "actual mouse selection copies the highlighted text",
  );
  await page.evaluate(() => {
    const t = window.terminalTest;
    t.emulator.getTerminal().clearSelection();
    t.emulator.focusTerminal();
  });
  await page.keyboard.press("Control+c");
  assert.deepEqual(await page.evaluate(() => window.terminalTest.sent), [
    "\u0003",
  ]);

  // A real browser copy event uses the same selection without an async writer.
  await select();
  await page.evaluate(() => document.execCommand("copy"));
  assert.equal(await paste(), "LOG-123");

  await select();
  await page
    .locator("#terminal canvas")
    .click({ button: "right", position: { x: 30, y: 12 } });
  const copy = page.getByRole("button", {
    name: "admin.webTerminal.copy",
    exact: true,
  });
  await copy.waitFor();
  await page.evaluate(async () => {
    const t = window.terminalTest;
    for (let i = 0; i < 30; i++) {
      t.write(`\r\nnew log ${i}`);
      await new Promise(requestAnimationFrame);
    }
  });
  assert.equal(
    await copy.isVisible(),
    true,
    "streaming output keeps the menu open",
  );
  await copy.click();
  assert.equal(await paste(), "LOG-123", "menu uses its original snapshot");

  await page
    .locator("#terminal canvas")
    .click({ button: "right", position: { x: 30, y: 12 } });
  await page
    .getByRole("button", { name: "admin.webTerminal.copyAllLogs" })
    .click();
  const snapshotText = await page.evaluate(() =>
    window.terminalTest.emulator.getTerminalText(),
  );
  const fullText = await paste();
  // WebKit normalizes decomposed Unicode when it crosses the OS clipboard.
  assert.equal(fullText.normalize("NFC"), snapshotText.normalize("NFC"));
  assert.ok(fullText.normalize("NFC").startsWith("LOG-123 中文 👩‍💻 é"));
  assert.ok(fullText.endsWith("new log 29"));

  // Model an HTTP context with no async clipboard API, but a real native copy command.
  await page.evaluate(() =>
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: undefined,
    }),
  );
  await page
    .locator("#terminal canvas")
    .click({ button: "right", position: { x: 30, y: 12 } });
  await page
    .getByRole("button", { name: "admin.webTerminal.copyAllLogs" })
    .click();
  assert.equal(
    await paste(),
    fullText,
    "native fallback pastes the full snapshot",
  );

  await page.evaluate(() => {
    window.originalExecCommand = document.execCommand.bind(document);
    document.execCommand = () => false;
  });
  await page
    .locator("#terminal canvas")
    .click({ button: "right", position: { x: 30, y: 12 } });
  await page
    .getByRole("button", { name: "admin.webTerminal.copyAllLogs" })
    .click();
  const dialog = page.getByRole("dialog");
  await dialog.waitFor();
  const text = dialog.locator("textarea");
  assert.equal(await text.inputValue(), snapshotText);
  await page.evaluate(() => window.terminalTest.write("\r\nnewer output"));
  assert.equal(
    await text.inputValue(),
    snapshotText,
    "fallback is a fixed snapshot",
  );
  assert.equal(
    await text.evaluate((element) => document.activeElement === element),
    true,
  );
  await page.evaluate(() => {
    window.retryCopyCalls = 0;
    document.execCommand = (...args) => {
      window.retryCopyCalls += 1;
      return window.originalExecCommand(...args);
    };
  });
  await page
    .getByRole("button", { name: "admin.webTerminal.retryCopy" })
    .click();
  await page.waitForFunction(() => !window.terminalTest.actions.copying.value);
  assert.equal(
    await page.evaluate(() => window.retryCopyCalls),
    1,
    "retry works inside the modal focus trap",
  );
  await text.focus();
  await text.press(`${nativeModifier}+a`);
  await text.press(`${nativeModifier}+c`);
  const [download] = await Promise.all([
    page.waitForEvent("download"),
    page
      .getByRole("button", { name: "admin.webTerminal.downloadCopyText" })
      .click(),
  ]);
  assert.equal(await readFile(await download.path(), "utf8"), snapshotText);
  await page
    .getByRole("button", { name: "shared.detailDialog.close", exact: true })
    .click();
  await dialog.waitFor({ state: "hidden" });
  assert.equal(
    await paste(),
    fullText,
    "manual copy in the dialog uses native text selection",
  );

  // Reset/reconnect must keep the selection manager attached to the live parser.
  const resetState = await page.evaluate(() => {
    document.execCommand = window.originalExecCommand;
    const t = window.terminalTest;
    const term = t.emulator.getTerminal();
    const parser = term.wasmTerm;
    term.write("\u001b[?1049h\u001b[?1000h");
    const focused = document.activeElement;
    t.emulator.clearTerminal();
    const state = {
      sameParser: term.wasmTerm === parser,
      alternate: term.wasmTerm.isAlternateScreen(),
      mouse: term.wasmTerm.hasMouseTracking(),
      text: t.emulator.getTerminalText(),
      sameFocus: document.activeElement === focused,
    };
    t.write("LOG-123 new session");
    return state;
  });
  assert.deepEqual(resetState, {
    sameParser: true,
    alternate: false,
    mouse: false,
    text: "",
    sameFocus: true,
  });
  await select();
  await page.keyboard.press(`${nativeModifier}+c`);
  assert.equal(await paste(), "LOG-123", "selection survives session resets");

  // A delayed rejection from an old session cannot open its text in the new one.
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: () =>
          new Promise((_, reject) => {
            window.rejectOldCopy = reject;
          }),
      },
    });
    window.terminalTest.emulator.focusTerminal();
    window.pendingOldCopy =
      window.terminalTest.actions.copyTerminalText("old session text");
    window.terminalTest.selectedSession.value = {
      id: "session-b",
      title: "new session",
    };
    window.rejectOldCopy(new Error("denied"));
  });
  await page.evaluate(() => window.pendingOldCopy);
  assert.equal(
    await dialog.count(),
    0,
    "session switch suppresses a stale failure dialog",
  );
  const sentBeforePaste = await page.evaluate(
    () => window.terminalTest.sent.length,
  );
  await page.evaluate(() => {
    const t = window.terminalTest;
    t.activeAttachment.value = { id: "attachment-a" };
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        readText: () =>
          new Promise((resolve) => {
            window.resolveOldPaste = resolve;
          }),
      },
    });
    window.pendingOldPaste = t.actions.pasteClipboardToTerminal();
    // The selected session changes before the old attachment has detached.
    t.selectedSession.value = { id: "session-c", title: "other session" };
    window.resolveOldPaste("old-session-command\n");
  });
  await page.evaluate(() => window.pendingOldPaste);
  assert.equal(
    await page.evaluate(() => window.terminalTest.sent.length),
    sentBeforePaste,
    "a pending clipboard read cannot send commands to another session",
  );
  assert.deepEqual(errors, []);
  console.log(
    `PASS ${browserName}: shortcuts, native copy, streamed output/menu snapshot, full buffer, API-unavailable fallback, manual copy/download, reset and stale-session protection`,
  );
} finally {
  await browser?.close();
  await server.close();
}
