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
grep -Fq 'redis_store.rs' "${CHECKER}" || fail 'Rust Redis Store hotspot is not guarded'
grep -Fq 'redis_compat.rs' "${CHECKER}" || fail 'Rust Redis compatibility hotspot is not guarded'
grep -A1 -F 'path: "apps/server-admin-rs/src/tunnels/cloudflared/optimization.rs"' "${CHECKER}" | grep -Fq 'maxLines: 200' || fail 'Rust optimization entry budget is not 200 lines'
grep -A1 -F 'path: "apps/server-admin-rs/src/storage/redis_store.rs"' "${CHECKER}" | grep -Fq 'maxLines: 120' || fail 'Rust Redis Store entry budget is not 120 lines'
grep -A1 -F 'path: "apps/server-admin-rs/src/storage/redis_compat.rs"' "${CHECKER}" | grep -Fq 'maxLines: 120' || fail 'Rust Redis compatibility entry budget is not 120 lines'
grep -Fq 'directoryBudgets' "${CHECKER}" || fail 'recursive Rust hotspot budgets are not configured'
grep -Fq 'entryPath: "apps/server-admin-rs/src/tunnels/cloudflared/optimization.rs"' "${CHECKER}" || fail 'Cloudflare optimization entry is not tied to its recursive guard'
grep -Fq 'entryPath: "apps/server-admin-rs/src/storage/redis_store.rs"' "${CHECKER}" || fail 'Redis Store entry is not tied to its recursive guard'
grep -Fq 'entryPath: "apps/server-admin-rs/src/storage/redis_compat.rs"' "${CHECKER}" || fail 'Redis compatibility entry is not tied to its recursive guard'
grep -Fq 'cloudflared/optimization"' "${CHECKER}" || fail 'Cloudflare optimization modules are not recursively guarded'
grep -Fq 'storage/redis_store"' "${CHECKER}" || fail 'Redis Store modules are not recursively guarded'
grep -Fq 'storage/redis_compat"' "${CHECKER}" || fail 'Redis compatibility modules are not recursively guarded'
grep -Fq 'testMaxLines: 500' "${CHECKER}" || fail 'Cloudflare optimization test shards are not guarded'
grep -Fq 'testMaxLines: 1_200' "${CHECKER}" || fail 'Redis Store test shards are not guarded'
grep -Fq 'testMaxLines: 500' "${CHECKER}" || fail 'Redis compatibility test shards are not guarded'
grep -Fq 'symbolic links are not allowed in guarded source trees' "${CHECKER}" || fail 'recursive guards can be bypassed with symlinks'
grep -Fq 'symbolic links are not allowed for guarded directories' "${CHECKER}" || fail 'guarded directories can be replaced with symlinks'
grep -Fq 'symbolic links are not allowed for guarded entry points' "${CHECKER}" || fail 'entry guards can be bypassed with symlinks'
grep -Fq 'uses #[path], which can escape recursive source budgets' "${CHECKER}" || fail '#[path] escapes are not rejected'
grep -Fq 'uses include!; guarded Rust trees require real modules' "${CHECKER}" || fail 'include! escapes are not rejected'
grep -Fq 'Rust reference guard self-test failed' "${CHECKER}" || fail 'Rust reference guards do not self-test multiline and whitespace escapes'
grep -Fq 'CloudflareOptimizationCard.vue' "${CHECKER}" || fail 'Vue optimization hotspot is not guarded'
node "${CHECKER}"

printf '[test-source-hotspots-contract] hotspot budgets are active\n'
