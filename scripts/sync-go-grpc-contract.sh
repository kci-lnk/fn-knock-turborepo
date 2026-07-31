#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GO_REPOSITORY="${1:-${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}}"
PROTO_ROOT="${ROOT_DIR}/packages/grpc-contracts/proto"
PROTO_FILE="${PROTO_ROOT}/fnknock/v1/gateway.proto"
PROTOC_GEN_GO_VERSION="v1.36.11"
PROTOC_GEN_GO_GRPC_VERSION="v1.5.1"

fail() {
  printf '[sync-go-grpc-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

[ -d "${GO_REPOSITORY}" ] || fail "missing Go repository: ${GO_REPOSITORY}"
for command_name in go protoc; do
  command -v "${command_name}" >/dev/null 2>&1 || fail "missing required command: ${command_name}"
done

TOOL_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fnknock-protoc-tools.XXXXXX")"
trap 'rm -rf "${TOOL_DIR}"' EXIT
GOBIN="${TOOL_DIR}" go install \
  "google.golang.org/protobuf/cmd/protoc-gen-go@${PROTOC_GEN_GO_VERSION}"
GOBIN="${TOOL_DIR}" go install \
  "google.golang.org/grpc/cmd/protoc-gen-go-grpc@${PROTOC_GEN_GO_GRPC_VERSION}"
export PATH="${TOOL_DIR}:${PATH}"

protoc \
  --proto_path "${PROTO_ROOT}" \
  --go_out "${GO_REPOSITORY}" \
  --go_opt module=go-reauth-proxy \
  --go-grpc_out "${GO_REPOSITORY}" \
  --go-grpc_opt module=go-reauth-proxy \
  "${PROTO_FILE}"

bash "${ROOT_DIR}/scripts/verify-go-control-api-contract.sh" "${GO_REPOSITORY}"
