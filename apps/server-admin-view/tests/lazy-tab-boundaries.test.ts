import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";

const readView = (name: string) =>
  readFileSync(new URL(`../src/views/${name}.vue`, import.meta.url), "utf8");

const assertLazyViews = (source: string, minimum: number) => {
  assert.match(source, /defineAsyncComponent/u);
  assert.equal(
    (source.match(/defineAsyncComponent\(/gu) ?? []).length >= minimum,
    true,
  );
  assert.doesNotMatch(source, /^import\s+\w+\s+from\s+"\.\/[^\n]+\.vue";/gmu);
};

describe("tab route chunk boundaries", () => {
  it("loads the large system settings sections on demand", () => {
    assertLazyViews(readView("SystemSettings"), 15);
  });

  it("keeps secondary tab pages out of their parent route chunks", () => {
    assertLazyViews(readView("EventCenter"), 3);
    assertLazyViews(readView("RequestAnalysis"), 2);
    assertLazyViews(readView("SessionManagement"), 4);
    assertLazyViews(readView("SSLSettings"), 3);
  });
});
