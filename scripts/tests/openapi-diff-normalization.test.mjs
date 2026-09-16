import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

test("normalizes boolean items without hiding nullable response changes", (context) => {
  const dir = mkdtempSync(join(tmpdir(), "openapi-diff-"));
  context.after(() => rmSync(dir, { recursive: true, force: true }));
  const baseline = {
    components: {
      schemas: {
        Empty: { type: "array", items: false, maxItems: 0 },
        Any: { type: "array", items: true },
        Value: { type: "integer", minimum: 0 },
      },
    },
  };
  const current = structuredClone(baseline);
  current.components.schemas.Value.type = ["integer", "null"];
  const files = ["base", "current", "filtered-base", "filtered-current"].map(
    (name) => join(dir, `${name}.json`),
  );
  writeFileSync(files[0], JSON.stringify(baseline));
  writeFileSync(files[1], JSON.stringify(current));
  const run = spawnSync(
    process.execPath,
    [
      new URL(
        "../filter-intentional-terminal-openapi-breaks.mjs",
        import.meta.url,
      ).pathname,
      ...files,
    ],
    { encoding: "utf8" },
  );
  assert.equal(run.status, 0, run.stderr);
  const before = JSON.parse(readFileSync(files[2], "utf8"));
  const after = JSON.parse(readFileSync(files[3], "utf8"));
  for (const document of [before, after]) {
    assert.deepEqual(document.components.schemas.Empty, {
      type: "array",
      items: { not: {} },
      maxItems: 0,
    });
    assert.deepEqual(document.components.schemas.Any, {
      type: "array",
      items: {},
    });
  }
  assert.deepEqual(
    before.components.schemas.Value,
    baseline.components.schemas.Value,
  );
  assert.deepEqual(
    after.components.schemas.Value,
    current.components.schemas.Value,
  );
});
