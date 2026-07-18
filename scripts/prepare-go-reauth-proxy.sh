#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"
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
BUNDLE_VERSION="$(fn_knock_app_version "${ROOT_DIR}")"
BUNDLE_COMMIT="unknown"

log() {
  echo "[fn-knock] $*"
}

fail() {
  echo "[fn-knock] ERROR: $*" >&2
  exit 1
}

needs_build() {
  local arch
  local binary
  local binary_version_file
  local binary_version

  if [ "${FORCE_BUILD}" = "1" ]; then
    return 0
  fi

  for arch in "${ARCHES[@]}"; do
    binary="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
    binary_version_file="${binary}.version"
    if [ ! -f "${binary}" ]; then
      return 0
    fi
    if [ ! -f "${binary_version_file}" ]; then
      return 0
    fi
    binary_version="$(tr -d '\r\n' < "${binary_version_file}")"
    if [ "${binary_version}" != "${BUNDLE_VERSION}" ]; then
      return 0
    fi
    if find "${GO_REAUTH_PROXY_DIR}" \
      \( -path "${GO_REAUTH_PROXY_BUILD_DIR}" -o -path "${GO_REAUTH_PROXY_BUILD_DIR}/*" \) -prune \
      -o \( -name '*.go' -o -name 'go.mod' -o -name 'go.sum' -o -name 'Taskfile.yml' \) \
      -newer "${binary}" -print -quit | grep -q .; then
      return 0
    fi
  done

  return 1
}

[ -d "${GO_REAUTH_PROXY_DIR}" ] || \
  fail "missing Go-Reauth-Proxy checkout: ${GO_REAUTH_PROXY_DIR}. Set FN_KNOCK_GO_REAUTH_PROXY_DIR to override."

BUNDLE_COMMIT="$(git -C "${GO_REAUTH_PROXY_DIR}" rev-parse --short=12 HEAD 2>/dev/null || printf 'unknown')"

if needs_build; then
  if [ "${SKIP_BUILD}" = "1" ]; then
    fail "missing gateway binaries in ${GO_REAUTH_PROXY_BUILD_DIR} and FN_KNOCK_GO_REAUTH_PROXY_SKIP_BUILD=1"
  fi

  command -v task >/dev/null 2>&1 || \
    fail "missing required command: task"

  if [ "${FORCE_BUILD}" = "1" ]; then
    log "Force rebuilding go-reauth-proxy ${BUNDLE_VERSION} binaries with task build in ${GO_REAUTH_PROXY_DIR}"
  else
    log "Building go-reauth-proxy ${BUNDLE_VERSION} binaries with task build in ${GO_REAUTH_PROXY_DIR}"
  fi
  (
    cd "${GO_REAUTH_PROXY_DIR}"
    FN_KNOCK_VERSION="${BUNDLE_VERSION}" \
      FN_KNOCK_COMMIT="${BUNDLE_COMMIT}" \
      task build
  )
fi

mkdir -p "${OUTPUT_DIR}"

for arch in "${ARCHES[@]}"; do
  src="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
  src_version_file="${src}.version"
  dst="${OUTPUT_DIR}/go-reauth-proxy-linux-${arch}"

  [ -f "${src}" ] || fail "missing gateway binary after build: ${src}"
  [ -f "${src_version_file}" ] || \
    fail "missing gateway bundle version metadata after build: ${src_version_file}"
  src_version="$(tr -d '\r\n' < "${src_version_file}")"
  [ "${src_version}" = "${BUNDLE_VERSION}" ] || \
    fail "gateway bundle version mismatch after build: expected ${BUNDLE_VERSION}, got ${src_version:-<empty>}"

  cp "${src}" "${dst}"
  chmod +x "${dst}"
  log "Prepared ${dst}"
done
