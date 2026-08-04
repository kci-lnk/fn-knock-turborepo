#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fail() { printf '[test-macos-release-contract] ERROR: %s\n' "$*" >&2; exit 1; }

for script in \
  deploy/macos/install.sh \
  deploy/macos/knock \
  deploy/macos/fn-knock-entrypoint \
  deploy/macos/tests/test-macos-management.sh \
  scripts/fn-knock-macos-launchd-smoke.sh \
  scripts/fn-knock-macos-smoke.sh \
  scripts/fn-knock-macos-package.sh
do
  bash -n "${ROOT_DIR}/${script}" || fail "invalid shell syntax: ${script}"
done
sh -n "${ROOT_DIR}/deploy/macos/install.sh" || fail "installer is not valid POSIX shell"

grep -Fq 'FN_KNOCK_RUNTIME_TARGET="macos"' "${ROOT_DIR}/deploy/macos/fn-knock-entrypoint" || \
  fail "macOS runtime target is missing"
grep -Fq 'FN_KNOCK_DISABLE_IPTABLES=1' "${ROOT_DIR}/deploy/macos/fn-knock-entrypoint" || \
  fail "macOS gateway does not disable iptables"
grep -Fq 'ADMIN_VIEW_HOST=127.0.0.1' "${ROOT_DIR}/deploy/macos/fn-knock.env" || \
  fail "macOS admin view is not loopback-only by default"
grep -Fq 'macos-15-intel' "${ROOT_DIR}/.github/workflows/macos.yml" || \
  fail "Intel runner is missing"
grep -Fq 'runner: macos-15' "${ROOT_DIR}/.github/workflows/macos.yml" || \
  fail "Apple Silicon runner is missing"
grep -Fq 'fn-knock-macos-${version}-amd64.tar.gz' "${ROOT_DIR}/scripts/fn-knock-release-finalize.mjs" || \
  fail "amd64 release inventory entry is missing"
grep -Fq 'fn-knock-macos-${version}-arm64.tar.gz' "${ROOT_DIR}/scripts/fn-knock-release-finalize.mjs" || \
  fail "arm64 release inventory entry is missing"
grep -Fq 'host_firewall_available: false' "${ROOT_DIR}/apps/server-admin-rs/src/infra/runtime_profile.rs" || \
  fail "macOS firewall capability is not disabled"
grep -Fq '[ "${answer}" = "DELETE" ]' "${ROOT_DIR}/deploy/macos/knock" || \
  fail "purge uninstall does not require the DELETE confirmation"
grep -Fq 'fn-knock-macos-launchd-smoke.sh' "${ROOT_DIR}/.github/workflows/release.yml" || \
  fail "release CI does not exercise the real launchd lifecycle"
if grep -Fq '"${ARCHIVE}.sha256"' "${ROOT_DIR}/scripts/fn-knock-macos-package.sh"; then
  fail "macOS packager must not create independent SHA-256 sidecars"
fi
grep -Fq '<key>Umask</key>' "${ROOT_DIR}/deploy/macos/cn.fnknock.service.plist" || \
  fail "LaunchDaemon does not set a restrictive umask"

printf '[test-macos-release-contract] macOS release contract passed\n'
