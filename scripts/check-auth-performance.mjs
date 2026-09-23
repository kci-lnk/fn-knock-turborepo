import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { compareRuns } from "./auth-performance-lib.mjs";

const [input, ...flags] = process.argv.slice(2);
assert.ok(
  input,
  "usage: node scripts/check-auth-performance.mjs results.json [--require-six-pairs]",
);
const result = JSON.parse(await readFile(input, "utf8"));
assert.equal(result.schema_version, 1);
const comparison = compareRuns(result.runs);
console.log(JSON.stringify(comparison, null, 2));
assert.ok(comparison.length > 0, "no comparisons");
assert.ok(
  comparison.every((group) => group.invalid_runs === 0),
  "invalid trials cannot support a performance claim",
);
assert.ok(
  comparison.every((group) => group.incomplete_pairs === 0),
  "comparison contains incomplete pairs",
);
if (flags.includes("--require-six-pairs")) {
  assert.ok(
    comparison.every((group) => group.six_valid_pairs),
    "at least six valid pairs per scenario are required",
  );
  assert.ok(
    comparison.every(
      (group) =>
        group.throughput_change_median >= -0.05 &&
        group.p99_change_median <= 0.1,
    ),
    "throughput or P99 exceeded regression budget",
  );
}
