#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GO_REPOSITORY="${1:-${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}}"
GENERATED_PATH="${GO_REPOSITORY}/pkg/grpc/pb/gateway.pb.go"

fail() {
  printf '[verify-go-control-api-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

EXPECTED="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
[ -f "${GENERATED_PATH}" ] || fail "missing generated Go contract: ${GENERATED_PATH}"
ACTUAL="$(
  sed -nE \
    's/^[[:space:]]*ControlApiVersion_CONTROL_API_VERSION_CURRENT[[:space:]]+ControlApiVersion[[:space:]]*=[[:space:]]*([0-9]+)[[:space:]]*$/\1/p' \
    "${GENERATED_PATH}"
)"
case "${ACTUAL}" in
  ''|*[!0-9]*) fail "generated Go contract does not define CONTROL_API_VERSION_CURRENT" ;;
esac
[ "${ACTUAL}" = "${EXPECTED}" ] || \
  fail "generated Go version ${ACTUAL} does not match gateway.proto ${EXPECTED}; run npm run fn-knock:grpc:sync-go"
printf '[verify-go-control-api-contract] control API version %s is synchronized\n' "${EXPECTED}"
