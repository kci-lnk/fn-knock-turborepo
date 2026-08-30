import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("../index.html", import.meta.url), "utf8");

test("admin document renders a static shell before the module graph executes", () => {
  assert.match(source, /data-fn-knock-bootstrap-shell/u);
  assert.match(source, /data-fn-knock-mounted/u);
});

test("admin document exposes recovery when a module fails before main executes", () => {
  assert.match(source, /HTMLScriptElement/u);
  assert.match(source, /modulepreload/u);
  assert.match(source, /unhandledrejection/u);
  assert.match(source, /data-fn-knock-bootstrap-retry/u);
  assert.match(source, /claimAutomaticReload/u);
  assert.match(source, /automaticReloadClaimed/u);
  assert.match(source, /previousReason === "stale-asset"/u);
  assert.match(source, /_fn_knock_reload_reason/u);
  assert.ok(
    source.indexOf("__fnKnockEarlyResourceFailure") <
      source.indexOf('type="module"'),
    "the early resource failure listener must precede the entry module",
  );
});
