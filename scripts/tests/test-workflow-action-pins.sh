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

printf '[test-workflow-action-pins] Node.js 24 action pins passed\n'
