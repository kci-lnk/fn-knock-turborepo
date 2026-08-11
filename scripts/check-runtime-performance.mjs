import { readFile } from "node:fs/promises";
import { compareRuntimeSummaries } from "./runtime-performance-lib.mjs";

const parseArguments = (args) => {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index];
    const value = args[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error(
        "usage: check-runtime-performance.mjs --base result.json --current result.json [--max-readiness-regression fraction] [--max-rss-regression fraction]",
      );
    }
    values.set(key, value);
  }
  if (!values.has("--base") || !values.has("--current")) {
    throw new Error("--base and --current are required");
  }
  return values;
};

const parseTolerance = (raw, label, fallback) => {
  const value = raw === undefined ? fallback : Number(raw);
  if (!Number.isFinite(value) || value < 0 || value > 10) {
    throw new Error(`${label} must be a fraction from 0 to 10`);
  }
  return value;
};

const readResult = async (path) => {
  const value = JSON.parse(await readFile(path, "utf8"));
  if (value?.schema_version !== 1 || !value.summary) {
    throw new Error(`${path} is not a runtime performance result`);
  }
  return value.summary;
};

const args = parseArguments(process.argv.slice(2));
const tolerances = {
  readiness: parseTolerance(
    args.get("--max-readiness-regression"),
    "--max-readiness-regression",
    0.1,
  ),
  rss: parseTolerance(
    args.get("--max-rss-regression"),
    "--max-rss-regression",
    0.05,
  ),
};
const [base, current] = await Promise.all([
  readResult(args.get("--base")),
  readResult(args.get("--current")),
]);
const failures = compareRuntimeSummaries(base, current, tolerances);
if (failures.length > 0) {
  throw new Error(`runtime performance regression: ${failures.join("; ")}`);
}
console.log(
  `[runtime-performance] passed: readiness p95 ${base.readiness_p95_ms} -> ${current.readiness_p95_ms} ms, management RSS p95 ${base.management_rss_p95_bytes} -> ${current.management_rss_p95_bytes} bytes`,
);
