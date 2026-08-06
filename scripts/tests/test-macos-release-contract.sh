#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
MACOS_WORKFLOW="${ROOT_DIR}/.github/workflows/macos.yml"
SUPPLEMENTAL_WORKFLOW="${ROOT_DIR}/.github/workflows/macos-release.yml"
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
grep -Fq 'macos-15-intel' "${MACOS_WORKFLOW}" || \
  fail "Intel runner is missing"
grep -Fq 'runner: macos-15' "${MACOS_WORKFLOW}" || \
  fail "Apple Silicon runner is missing"
grep -Fq '      - "v*"' "${MACOS_WORKFLOW}" || \
  fail "macOS CLI workflow is not triggered by release tags"
if grep -Fq '  pull_request:' "${MACOS_WORKFLOW}" || grep -Fq '    branches: [main]' "${MACOS_WORKFLOW}"; then
  fail "macOS CLI workflow must not run for every pull request or main branch push"
fi
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

[ -f "${SUPPLEMENTAL_WORKFLOW}" ] || fail "macOS supplemental release workflow is missing"
grep -Fq 'uses: ./.github/workflows/macos.yml' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not reuse the native CI build"
grep -Fq 'group: fn-knock-stable-release' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not share the stable release lock"
grep -Fq 'node ./scripts/fn-knock-cos-publish.mjs plan-macos' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not generate a dry-run plan"
grep -Fq 'node ./scripts/fn-knock-cos-publish.mjs check-macos' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not preflight COS"
grep -Fq 'node ./scripts/fn-knock-cos-publish.mjs publish-macos' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not publish COS transactionally"
grep -Fq 'subject-path: dist/macos-release-assets/*.tar.gz' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not attest both archives"
grep -Fq -- '- name: Roll back GitHub Release if COS publication failed' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release does not compensate GitHub mutations"
grep -Fq 'gh release delete-asset "${tag}" "${uploaded_name}"' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental rollback does not remove newly added assets"
grep -Fq 'type: boolean' "${SUPPLEMENTAL_WORKFLOW}" || \
  fail "macOS supplemental release publish confirmation is not boolean"

preflight_line="$(grep -nF 'node ./scripts/fn-knock-cos-publish.mjs check-macos' "${SUPPLEMENTAL_WORKFLOW}" | cut -d: -f1)"
github_line="$(grep -nF -- '- name: Add macOS packages to the existing GitHub Release' "${SUPPLEMENTAL_WORKFLOW}" | cut -d: -f1)"
publish_line="$(grep -nF 'node ./scripts/fn-knock-cos-publish.mjs publish-macos' "${SUPPLEMENTAL_WORKFLOW}" | cut -d: -f1)"
[ "${preflight_line}" -lt "${github_line}" ] || \
  fail "COS preflight must happen before GitHub Release mutation"
[ "${github_line}" -lt "${publish_line}" ] || \
  fail "latest.json must be committed after GitHub Release assets"
grep -Fq 'publishMode: "macos-only"' "${ROOT_DIR}/scripts/fn-knock-cos-publish.mjs" || \
  fail "macOS COS plan is not marked as a partial publication"
grep -Fq '...current.packages,' "${ROOT_DIR}/scripts/fn-knock-cos-publish.mjs" || \
  fail "macOS latest merge does not preserve other package nodes"

printf '[test-macos-release-contract] macOS release contract passed\n'
