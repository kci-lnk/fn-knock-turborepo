// Run: node scripts/subdomain-batch-access-browser-test.mjs
// Isolated real-component fixture: no backend or credentials required.
import assert from "node:assert/strict";
import { readFile, mkdir } from "node:fs/promises";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";
import os from "node:os";
import { chromium } from "playwright";

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
  new URL("./fixtures/subdomain-batch-access-browser.js", import.meta.url),
  "utf8",
);
const output =
  process.env.SUBDOMAIN_SCREENSHOTS ||
  path.join(os.tmpdir(), "subdomain-batch-access");
await mkdir(output, { recursive: true });
const server = await createServer({
  root,
  logLevel: "error",
  server: { host: "127.0.0.1", port: 0 },
  plugins: [
    {
      name: "subdomain-batch-access-fixture",
      configureServer(dev) {
        dev.middlewares.use(async (request, response, next) => {
          if (request.url?.startsWith("/__batch-access.html")) {
            response.setHeader("Content-Type", "text/html");
            response.end(
              await dev.transformIndexHtml(
                request.url,
                '<!doctype html><html><head><meta name="viewport" content="width=device-width,initial-scale=1"></head><body><div id="app"></div><script type="module" src="/__batch-access.js"></script></body></html>',
              ),
            );
          } else next();
        });
      },
      resolveId(id) {
        if (id === "/__batch-access.js") return "\0batch-access-fixture";
      },
      load(id) {
        if (id === "\0batch-access-fixture") return fixture;
      },
    },
  ],
});
let browser;
try {
  await server.listen();
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const widths = process.env.SUBDOMAIN_TEST_WIDTHS
    ? process.env.SUBDOMAIN_TEST_WIDTHS.split(",").map(Number)
    : [320, 375, 390, 640, 768, 1280, 1440];
  let scenarios = 0;
  const assertInViewport = async (locator) => {
    const rect = await locator.boundingBox();
    const viewport = page.viewportSize();
    assert.ok(
      rect &&
        rect.x >= -1 &&
        rect.y >= -1 &&
        rect.x + rect.width <= viewport.width + 1 &&
        rect.y + rect.height <= viewport.height + 1,
      JSON.stringify({ rect, viewport }),
    );
  };
  for (const width of widths) {
    console.log(`Checking ${width}px layouts...`);
    for (const locale of ["zh-CN", "en"]) {
      for (const grouped of [false, true]) {
        for (const dark of [false, true]) {
          const height = width === 640 ? 320 : 900;
          await page.setViewportSize({ width, height });
          await page.goto(
            `${server.resolvedUrls.local[0]}__batch-access.html?${new URLSearchParams({ locale, grouped, dark })}`,
          );
          await page.waitForFunction(() => window.batchTest);
          const labels = await page.evaluate(() => window.batchTest.labels);
          await page
            .getByRole("button", { name: labels.select, exact: true })
            .click();
          await page
            .getByRole("checkbox", { name: labels.all, exact: true })
            .click();
          const trigger = page.getByTestId("batch-more-trigger");
          await trigger.click();
          const menu = page.getByRole("menu");
          await menu.waitFor();
          await menu.evaluate((node) =>
            Promise.all(
              node.getAnimations().map((animation) => animation.finished),
            ),
          );
          await assertInViewport(menu);
          if (width < 640) {
            for (const item of await menu.getByRole("menuitem").all()) {
              assert.ok(
                (await item.boundingBox()).height >= 43.99,
                JSON.stringify(
                  await item.evaluate((node) => ({
                    height: node.getBoundingClientRect().height,
                    minHeight: getComputedStyle(node).minHeight,
                    text: node.textContent,
                  })),
                ),
              );
            }
          }
          const tag = `${width}-${locale}-${grouped ? "grouped" : "flat"}-${dark ? "dark" : "light"}`;
          await page.screenshot({ path: path.join(output, `${tag}-menu.png`) });
          await page.getByTestId("batch-public-access").click();
          const dialog = page.getByRole("dialog");
          await dialog.waitFor();
          await dialog.evaluate((node) =>
            Promise.all(
              node.getAnimations().map((animation) => animation.finished),
            ),
          );
          assert.equal(
            await dialog.getByRole("heading").innerText(),
            labels.title,
          );
          await assertInViewport(dialog);
          assert.ok(
            await page.evaluate(
              () => document.documentElement.scrollWidth <= innerWidth,
            ),
          );
          const clipped = await dialog.evaluate((node) =>
            [...node.querySelectorAll("h2,p,button")].some(
              (el) => el.scrollWidth > el.clientWidth + 1,
            ),
          );
          assert.equal(clipped, false, `clipped text: ${tag}`);
          const confirm = dialog.getByRole("button").filter({
            hasText: locale === "en" ? "Allow public access" : "公开访问",
          });
          await confirm.scrollIntoViewIfNeeded();
          await assertInViewport(confirm);
          await page.screenshot({
            path: path.join(output, `${tag}-dialog.png`),
          });
          await confirm.click();
          await dialog.waitFor({ state: "hidden" });
          assert.deepEqual(
            (await page.evaluate(() => window.batchTest.snapshot())).auth,
            [false, false],
          );
          assert.equal(await trigger.count(), 0);
          scenarios++;
        }
      }
    }
  }
  // Both authentication-service markers must be excluded from selection on each layout.
  for (const width of [390, 1280]) {
    await page.setViewportSize({ width, height: 900 });
    await page.goto(
      `${server.resolvedUrls.local[0]}__batch-access.html?locale=en&protectedAuth=true`,
    );
    await page.waitForFunction(() => window.batchTest);
    const labels = await page.evaluate(() => window.batchTest.labels);
    await page
      .getByRole("button", { name: labels.select, exact: true })
      .click();
    assert.equal(
      await page.getByRole("checkbox").count(),
      3,
      "only select-all and the two application checkboxes should exist",
    );
    await page.getByRole("checkbox", { name: labels.all, exact: true }).click();
    assert.ok(
      (await page.getByRole("toolbar").innerText()).includes(
        "2 subdomains selected",
      ),
    );
    await page.getByTestId("batch-more-trigger").click();
    await page.getByTestId("batch-require-auth").click();
    await page
      .getByRole("dialog")
      .getByRole("button", { name: "Require sign-in (2)", exact: true })
      .click();
    await page.getByRole("dialog").waitFor({ state: "hidden" });
    assert.deepEqual(
      (await page.evaluate(() => window.batchTest.snapshot())).auth,
      [true, true, false, false],
    );
  }
  // Exercise keyboard focus restoration, cancellation, failed retries and in-flight dismissal.
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(
    `${server.resolvedUrls.local[0]}__batch-access.html?locale=en`,
  );
  await page.waitForFunction(() => window.batchTest);
  const labels = await page.evaluate(() => window.batchTest.labels);
  await page.getByRole("button", { name: labels.select, exact: true }).click();
  await page.getByRole("checkbox", { name: labels.all, exact: true }).click();
  const trigger = page.getByTestId("batch-more-trigger");
  await trigger.focus();
  await page.keyboard.press("Enter");
  await page.getByRole("menu").waitFor();
  await page.keyboard.press("Escape");
  await page.waitForFunction(
    () =>
      document.activeElement?.getAttribute("data-testid") ===
      "batch-more-trigger",
  );
  await trigger.click();
  await page.getByTestId("batch-require-auth").click();
  let dialog = page.getByRole("dialog");
  await dialog
    .getByRole("button", { name: labels.cancel, exact: true })
    .click();
  assert.equal(
    (await page.evaluate(() => window.batchTest.snapshot())).writes,
    0,
  );
  await dialog.waitFor({ state: "hidden" });
  assert.equal(
    await trigger.evaluate((node) => document.activeElement === node),
    true,
    "cancelling the dialog should restore focus to its menu trigger",
  );
  await trigger.click();
  await page.getByTestId("batch-require-auth").click();
  dialog = page.getByRole("dialog");
  await page.evaluate(() => window.batchTest.fail(true));
  const confirm = dialog.getByRole("button", {
    name: "Require sign-in (2)",
    exact: true,
  });
  await confirm.click();
  await page.waitForFunction(
    () => window.batchTest.snapshot().errors.length === 1,
  );
  assert.equal(await dialog.isVisible(), true);
  await page.evaluate(() => {
    window.batchTest.fail(false);
    window.batchTest.hold();
  });
  await confirm.click();
  assert.equal(await confirm.isDisabled(), true);
  await page.keyboard.press("Escape");
  await page.mouse.click(2, 2);
  assert.equal(await dialog.isVisible(), true);
  await page.evaluate(() => window.batchTest.release());
  await dialog.waitFor({ state: "hidden" });
  const state = await page.evaluate(() => window.batchTest.snapshot());
  assert.equal(state.writes, 2);
  assert.deepEqual(state.auth, [true, true]);
  assert.deepEqual(errors, []);
  console.log(
    `Passed ${scenarios} responsive scenarios plus keyboard/save/retry checks. Screenshots: ${output}`,
  );
} finally {
  await browser?.close();
  await server.close();
}
