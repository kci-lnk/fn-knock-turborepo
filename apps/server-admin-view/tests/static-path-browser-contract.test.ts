import { readFileSync } from "node:fs";
import test from "node:test";
import assert from "node:assert/strict";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

test("static path browser stays inside the mapping dialog and keeps responsive accessible controls", () => {
  const dialog = readSource(
    "../src/views/subdomain-proxy/SubdomainMappingDialog.vue",
  );
  const browser = readSource(
    "../src/views/subdomain-proxy/SubdomainMappingStaticPathBrowser.vue",
  );

  assert.match(dialog, /mappingDialogView === 'path-browser'/);
  assert.match(dialog, /sm:!max-w-\[760px\]/);
  assert.match(dialog, /<DialogTitle class="sr-only">\{\{ dialogTitle \}\}/);
  assert.match(dialog, /<DialogDescription class="sr-only">/);
  assert.match(dialog, /staticServe\.browser\.title/);
  assert.match(dialog, /staticServe\.browser\.hint/);
  assert.doesNotMatch(browser, /<Dialog\b|from "@\/components\/ui\/dialog"/);
  assert.match(browser, /configStore\.isDockerDeployment/);
  assert.match(browser, /staticServe\.browser\.dockerHint/);
  assert.match(browser, /max-sm:grid-cols-1/);
  assert.match(browser, /:aria-busy="editor\.isLoading"/);
  assert.match(browser, /id="static-path-browser-address"/);
  assert.match(browser, /:model-value="editor\.pathDraft"/);
  assert.match(browser, /@update:model-value="editor\.updatePathDraft"/);
  assert.match(browser, /@keydown\.enter="handlePathEnter"/);
  assert.match(browser, /@submit\.prevent="editor\.navigateToPath"/);
  assert.match(browser, /event\.isComposing/);
  assert.match(browser, /:aria-label="entryAriaLabel\(entry\)"/);
  assert.match(browser, /:aria-pressed=/);
  assert.match(browser, /editor\.selectionPath === entry\.path/);
  assert.doesNotMatch(browser, /editor\.selectedPath === entry\.path/);

  const nativeButtons = browser.match(/<button\b[^>]*>/g) ?? [];
  assert.ok(nativeButtons.length > 0);
  assert.ok(
    nativeButtons.every((button) => /type="button"/.test(button)),
    "every native browser control must have explicit button semantics",
  );
});
