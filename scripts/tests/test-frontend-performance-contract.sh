#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW="${ROOT_DIR}/.github/workflows/ci.yml"

node --input-type=module - "${ROOT_DIR}/package.json" <<'NODE'
import { readFile } from "node:fs/promises";
const manifest = JSON.parse(await readFile(process.argv[2], "utf8"));
if (manifest.scripts["frontend:measure"] !== "node ./scripts/frontend-performance.mjs") process.exit(1);
if (manifest.scripts["frontend:measure:check"] !== "node ./scripts/check-frontend-performance.mjs") process.exit(1);
NODE

rg -q 'FN_KNOCK_FRONTEND_PERF_RUNS: "5"' "${WORKFLOW}"
rg -q 'frontend-performance-current.json' "${WORKFLOW}"
rg -q -- '--max-regression 0.10' "${WORKFLOW}"
node --test "${ROOT_DIR}/scripts/tests/frontend-performance.test.mjs"

printf '[test-frontend-performance-contract] cold-cache frontend performance gate passed\n'
