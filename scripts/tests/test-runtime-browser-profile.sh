#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CARGO_MANIFEST="${ROOT_DIR}/apps/server-admin-rs/Cargo.toml"
PACKAGE_MANIFEST="${ROOT_DIR}/package.json"
HARNESS="${ROOT_DIR}/scripts/runtime-test-harness.mjs"
RUNTIME_E2E="${ROOT_DIR}/scripts/runtime-e2e.mjs"
CI_WORKFLOW="${ROOT_DIR}/.github/workflows/ci.yml"

fail() {
  printf '[test-runtime-browser-profile] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_contains() {
  local file="$1" expected="$2" label="$3"
  grep -Fq -- "${expected}" "${file}" || fail "${label}: ${file} is missing ${expected}"
}

assert_contains "${CARGO_MANIFEST}" '[profile.release]' 'release profile'
assert_contains "${CARGO_MANIFEST}" 'lto = "fat"' 'release artifact fat LTO'
assert_contains "${CARGO_MANIFEST}" '[profile.runtime-test]' 'browser runtime profile'
assert_contains "${CARGO_MANIFEST}" 'inherits = "release"' 'browser runtime release inheritance'
assert_contains "${CARGO_MANIFEST}" 'lto = "thin"' 'browser runtime thin LTO'

node -e '
  const manifest = require(process.argv[1]);
  const expected = "cargo build --locked --manifest-path apps/server-admin-rs/Cargo.toml --profile runtime-test --bin server-admin-rs";
  if (manifest.scripts["runtime:build"] !== expected) process.exit(1);
' "${PACKAGE_MANIFEST}" || fail 'runtime:build must compile only the server binary with runtime-test'

assert_contains "${HARNESS}" 'process.env.FN_KNOCK_RUNTIME_SERVER_BIN' 'explicit runtime server override'
assert_contains "${HARNESS}" 'const child = spawn(resolvedServerBinary' 'resolved runtime server launch'
assert_contains "${RUNTIME_E2E}" 'await page.goto("about:blank")' 'in-flight session refresh cancellation'
assert_contains "${RUNTIME_E2E}" 'await context.clearCookies()' 'fresh browser session cookie removal'
assert_contains "${CI_WORKFLOW}" 'npm run runtime:build' 'daily runtime profile build'
assert_contains "${CI_WORKFLOW}" 'target/runtime-test/server-admin-rs' 'daily runtime binary selection'
assert_contains "${CI_WORKFLOW}" 'npx turbo run build --filter=server-admin-view --filter=server-auth-view' 'frontend-only Turbo build'
if grep -Fq 'npm run build' "${CI_WORKFLOW}"; then
  fail 'daily CI must not trigger the distributable fat-LTO Rust build'
fi

printf '[test-runtime-browser-profile] release-compatible browser runtime build contract passed\n'
