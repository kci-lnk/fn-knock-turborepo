#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUTPUT_DIR="${1:-${ROOT_DIR}/apps/fn-knock/app/server}"
shift || true

ARCHES=("$@")
if [ "${#ARCHES[@]}" -eq 0 ]; then
  ARCHES=(amd64 arm64 arm)
fi

GO_REAUTH_PROXY_DIR="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"
GO_REAUTH_PROXY_BUILD_DIR="${FN_KNOCK_GO_REAUTH_PROXY_BUILD_DIR:-${GO_REAUTH_PROXY_DIR}/build}"
SKIP_BUILD="${FN_KNOCK_GO_REAUTH_PROXY_SKIP_BUILD:-0}"
FORCE_BUILD="${FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD:-0}"

log() {
  echo "[fn-knock] $*"
}

fail() {
  echo "[fn-knock] ERROR: $*" >&2
  exit 1
}

needs_build() {
  local arch

  if [ "${FORCE_BUILD}" = "1" ]; then
    return 0
  fi

  for arch in "${ARCHES[@]}"; do
    if [ ! -f "${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}" ]; then
      return 0
    fi
  done

  return 1
}

[ -d "${GO_REAUTH_PROXY_DIR}" ] || \
  fail "missing Go-Reauth-Proxy checkout: ${GO_REAUTH_PROXY_DIR}. Set FN_KNOCK_GO_REAUTH_PROXY_DIR to override."

if needs_build; then
  if [ "${SKIP_BUILD}" = "1" ]; then
    fail "missing gateway binaries in ${GO_REAUTH_PROXY_BUILD_DIR} and FN_KNOCK_GO_REAUTH_PROXY_SKIP_BUILD=1"
  fi

  command -v task >/dev/null 2>&1 || \
    fail "missing required command: task"

  if [ "${FORCE_BUILD}" = "1" ]; then
    log "Force rebuilding go-reauth-proxy binaries with task build in ${GO_REAUTH_PROXY_DIR}"
  else
    log "Building go-reauth-proxy binaries with task build in ${GO_REAUTH_PROXY_DIR}"
  fi
  (cd "${GO_REAUTH_PROXY_DIR}" && task build)
fi

mkdir -p "${OUTPUT_DIR}"

for arch in "${ARCHES[@]}"; do
  src="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
  dst="${OUTPUT_DIR}/go-reauth-proxy-linux-${arch}"

  [ -f "${src}" ] || fail "missing gateway binary after build: ${src}"

  cp "${src}" "${dst}"
  chmod +x "${dst}"
  log "Prepared ${dst}"
done
