import { readFile } from "node:fs/promises";
import { compareFrontendSummaries } from "./frontend-performance-lib.mjs";

const args = Object.fromEntries(
  process.argv.slice(2).reduce((pairs, value, index, values) => {
    if (index % 2 === 0) pairs.push([value, values[index + 1]]);
    return pairs;
  }, []),
);
if (!args["--base"] || !args["--current"]) {
  throw new Error("--base and --current are required");
}
const tolerance = Number(args["--max-regression"] ?? 0.1);
if (!Number.isFinite(tolerance) || tolerance < 0) {
  throw new Error("--max-regression must be a non-negative fraction");
}
const readSummary = async (file) => {
  const value = JSON.parse(await readFile(file, "utf8"));
  if (value.schema_version !== 1 || !value.summary) {
    throw new Error(`${file} is not a frontend performance result`);
  }
  return value.summary;
};
const failures = compareFrontendSummaries(
  await readSummary(args["--base"]),
  await readSummary(args["--current"]),
  tolerance,
);
if (failures.length > 0) {
  throw new Error(`frontend performance regression: ${failures.join("; ")}`);
}
console.log("[frontend-performance] passed");
