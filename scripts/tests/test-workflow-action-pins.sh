#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORKFLOW_DIR="${ROOT_DIR}/.github/workflows"

fail() {
  printf '[test-workflow-action-pins] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_action_pin() {
  local action="$1"
  local expected_sha="$2"
  local expected_version="$3"
  local calls
  local unexpected

  calls="$(grep -RHnF "uses: ${action}@" "${WORKFLOW_DIR}" || true)"
  [ -n "${calls}" ] || fail "no workflow uses ${action}"

  unexpected="$(
    printf '%s\n' "${calls}" |
      grep -vF "uses: ${action}@${expected_sha} # ${expected_version}" || true
  )"
  [ -z "${unexpected}" ] || \
    fail "${action} must be pinned to ${expected_version} (${expected_sha}): ${unexpected}"
}

assert_action_pin \
  "actions/checkout" \
  "de0fac2e4500dabe0009e67214ff5f5447ce83dd" \
  "v6.0.2"
assert_action_pin \
  "actions/upload-artifact" \
  "043fb46d1a93c77aae656e7c1c64a875d1fc6a0a" \
  "v7.0.1"
assert_action_pin \
  "actions/download-artifact" \
  "3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c" \
  "v8.0.1"
assert_action_pin \
  "actions/setup-node" \
  "249970729cb0ef3589644e2896645e5dc5ba9c38" \
  "v6.5.0"
assert_action_pin \
  "docker/setup-qemu-action" \
  "96fe6ef7f33517b61c61be40b68a1882f3264fb8" \
  "v4.2.0"
assert_action_pin \
  "docker/setup-buildx-action" \
  "bb05f3f5519dd87d3ba754cc423b652a5edd6d2c" \
  "v4.2.0"
assert_action_pin \
  "docker/login-action" \
  "af1e73f918a031802d376d3c8bbc3fe56130a9b0" \
  "v4.4.0"
assert_action_pin \
  "docker/build-push-action" \
  "53b7df96c91f9c12dcc8a07bcb9ccacbed38856a" \
  "v7.3.0"
assert_action_pin \
  "actions/attest-build-provenance" \
  "0f67c3f4856b2e3261c31976d6725780e5e4c373" \
  "v4.1.1"

if grep -RHFq \
  -e "uses: arduino/setup-protoc@" \
  -e "uses: mlugg/setup-zig@" \
  "${WORKFLOW_DIR}"; then
  fail "workflow still uses a Node.js 20 setup action"
fi

grep -Fq \
  'PROTOC_LINUX_X86_64_SHA256: "6930ebf62bd4ea607b98fff052596c6ee564b9835b4ce172c75a3f53ae9d91b7"' \
  "${WORKFLOW_DIR}/release.yml" ||
  fail "protoc 35.1 checksum is not pinned"
grep -Fq \
  'ZIG_LINUX_X86_64_SHA256: "70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00"' \
  "${WORKFLOW_DIR}/release.yml" ||
  fail "Zig 0.16.0 checksum is not pinned"

printf '[test-workflow-action-pins] Node.js 24 action pins passed\n'
