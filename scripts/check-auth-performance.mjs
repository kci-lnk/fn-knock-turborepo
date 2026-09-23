import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { compareRuns, comparisonFailures } from "./auth-performance-lib.mjs";

const [input, ...flags] = process.argv.slice(2);
assert.ok(
  input,
  "usage: node scripts/check-auth-performance.mjs results.json [--require-six-pairs] [--require-improvement]",
);
assert.ok(
  flags.every((flag) =>
    ["--require-six-pairs", "--require-improvement"].includes(flag),
  ),
  "unknown check flag",
);
const result = JSON.parse(await readFile(input, "utf8"));
assert.equal(result.schema_version, 1);
const comparison = compareRuns(result.runs);
console.log(JSON.stringify(comparison, null, 2));
const failures = comparisonFailures(comparison, {
  requireSixPairs: flags.includes("--require-six-pairs"),
  requireImprovement: flags.includes("--require-improvement"),
});
assert.equal(failures.length, 0, failures.join("\n"));
