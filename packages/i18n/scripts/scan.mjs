import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const repoRoot = new URL("../../..", import.meta.url);
const allowlist = readFileSync(
  new URL("../scan-allowlist.txt", import.meta.url),
  "utf8",
)
  .split("\n")
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith("#"));

const args = [
  "-n",
  "[\\p{Han}]",
  "-g",
  "!node_modules",
  "-g",
  "!.turbo",
  "-g",
  "!dist",
  "-g",
  "!build",
  "-g",
  "!*.lock",
  "-g",
  "!apps/fn-knock/**",
  "-g",
  "!apps/fn-knock-docker/**",
  "-g",
  "!packages/i18n/src/locales.ts",
  "-g",
  "!packages/i18n/src/locale-options.ts",
  "-g",
  "!packages/i18n/src/messages/**",
  ".",
];

const result = spawnSync("rg", args, {
  cwd: repoRoot,
  encoding: "utf8",
  maxBuffer: 4 * 1024 * 1024,
});

if (result.status === 1) {
  console.log("[i18n] no residual Chinese text outside locale files.");
  process.exit(0);
}

if (result.error || result.status == null || result.status > 1) {
  console.error(result.stderr || result.error?.message || "[i18n] scan failed");
  process.exit(result.status || 1);
}

const lines = result.stdout
  .trim()
  .split("\n")
  .filter(Boolean)
  .filter((line) => !allowlist.some((pattern) => line.includes(pattern)));
console.log(
  `[i18n] found ${lines.length} residual Chinese text locations outside locale files.`,
);
console.log(lines.slice(0, 80).join("\n"));
if (lines.length > 80) {
  console.log(`[i18n] ... ${lines.length - 80} more`);
}
