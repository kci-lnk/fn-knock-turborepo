#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHECKER="${ROOT_DIR}/scripts/check-source-hotspots.mjs"
PACKAGE_MANIFEST="${ROOT_DIR}/package.json"

fail() {
  printf '[test-source-hotspots-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

[ -f "${CHECKER}" ] || fail "missing hotspot checker"

node -e '
  const manifest = require(process.argv[1]);
  if (manifest.scripts["source:hotspots"] !== "node ./scripts/check-source-hotspots.mjs") process.exit(1);
  if (!manifest.scripts["quality:check"].includes("npm run source:hotspots")) process.exit(1);
' "${PACKAGE_MANIFEST}" || fail 'source hotspot quality gate is missing from package scripts'

grep -Fq 'optimization.rs' "${CHECKER}" || fail 'Rust optimization hotspot is not guarded'
grep -Fq 'CloudflareOptimizationCard.vue' "${CHECKER}" || fail 'Vue optimization hotspot is not guarded'
node "${CHECKER}"

printf '[test-source-hotspots-contract] hotspot budgets are active\n'
