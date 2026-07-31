#!/bin/bash
set -euo pipefail

ROOT_DIR="${FN_KNOCK_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
CONTRACT_PATH="${ROOT_DIR}/packages/grpc-contracts/proto/fnknock/v1/gateway.proto"

fail() {
  printf '[control-api-version] ERROR: %s\n' "$*" >&2
  exit 1
}

[ -f "${CONTRACT_PATH}" ] || fail "missing contract: ${CONTRACT_PATH}"
VERSION="$(
  sed -nE \
    's/^[[:space:]]*CONTROL_API_VERSION_CURRENT[[:space:]]*=[[:space:]]*([0-9]+)[[:space:]]*;.*/\1/p' \
    "${CONTRACT_PATH}"
)"
case "${VERSION}" in
  ''|0|*[!0-9]*) fail "CONTROL_API_VERSION_CURRENT must be a single positive integer" ;;
esac
[ "$(printf '%s\n' "${VERSION}" | wc -l | tr -d '[:space:]')" = "1" ] || \
  fail "CONTROL_API_VERSION_CURRENT must be defined exactly once"
printf '%s\n' "${VERSION}"
