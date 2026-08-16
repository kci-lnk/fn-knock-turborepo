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
BUNDLE_COMMIT=""
ACTUAL_COMMIT=""

log() {
  echo "[fn-knock] $*"
}

fail() {
  echo "[fn-knock] ERROR: $*" >&2
  exit 1
}

invalidate_build_cache() {
  local arch
  local binary

  for arch in "${ARCHES[@]}"; do
    binary="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
    rm -f "${binary}" "${binary}.commit" "${binary}.version"
  done
}

assert_checkout_locked() {
  local phase="$1"
  local invalidate="${2:-0}"
  local actual_commit
  local worktree_state

  actual_commit="$(git -C "${GO_REAUTH_PROXY_DIR}" rev-parse HEAD 2>/dev/null)" || \
    fail "unable to resolve Go gateway commit from ${GO_REAUTH_PROXY_DIR} during ${phase}"
  if [ "${actual_commit}" != "${BUNDLE_COMMIT}" ]; then
    if [ "${invalidate}" = "1" ]; then
      invalidate_build_cache
    fi
    fail "Go gateway HEAD changed during artifact preparation (${phase}): expected ${BUNDLE_COMMIT}, got ${actual_commit}"
  fi

  worktree_state="$(git -C "${GO_REAUTH_PROXY_DIR}" status --porcelain --untracked-files=normal)"
  if [ -n "${worktree_state}" ]; then
    if [ "${invalidate}" = "1" ]; then
      invalidate_build_cache
    fi
    fail "Go gateway working tree is not clean during artifact preparation (${phase}); commit or discard changes before packaging"
  fi
}

needs_build() {
  local arch
  local binary
  local binary_commit_file
  local binary_commit
  local binary_version_file
  local binary_version

  if [ "${FORCE_BUILD}" = "1" ]; then
    return 0
  fi

  for arch in "${ARCHES[@]}"; do
    binary="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
    binary_commit_file="${binary}.commit"
    binary_version_file="${binary}.version"
    if [ ! -f "${binary}" ]; then
      return 0
    fi
    if [ ! -f "${binary_version_file}" ]; then
      return 0
    fi
    if [ ! -f "${binary_commit_file}" ]; then
      return 0
    fi
    binary_version="$(tr -d '\r\n' < "${binary_version_file}")"
    if [ "${binary_version}" != "${BUNDLE_VERSION}" ]; then
      return 0
    fi
    binary_commit="$(tr -d '\r\n' < "${binary_commit_file}")"
    if [ "${binary_commit}" != "${BUNDLE_COMMIT}" ]; then
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
bash "${ROOT_DIR}/scripts/verify-go-control-api-contract.sh" "${GO_REAUTH_PROXY_DIR}"

ACTUAL_COMMIT="$(git -C "${GO_REAUTH_PROXY_DIR}" rev-parse HEAD 2>/dev/null)" || \
  fail "unable to resolve Go gateway commit from ${GO_REAUTH_PROXY_DIR}"
if [ -n "${FN_KNOCK_GATEWAY_COMMIT:-}" ]; then
  BUNDLE_COMMIT="${FN_KNOCK_GATEWAY_COMMIT}"
else
  BUNDLE_COMMIT="${ACTUAL_COMMIT}"
fi
[[ "${BUNDLE_COMMIT}" =~ ^[0-9a-f]{40}$ ]] || \
  fail "Go gateway commit must be a 40-character lowercase Git commit: ${BUNDLE_COMMIT:-<empty>}"
assert_checkout_locked "before gateway build"

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
  assert_checkout_locked "after gateway build" 1
  for arch in "${ARCHES[@]}"; do
    binary="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
    [ -f "${binary}" ] || fail "missing gateway binary after build: ${binary}"
    printf '%s\n' "${BUNDLE_COMMIT}" > "${binary}.commit"
  done
fi

mkdir -p "${OUTPUT_DIR}"

for arch in "${ARCHES[@]}"; do
  src="${GO_REAUTH_PROXY_BUILD_DIR}/go-reauth-proxy-linux-${arch}"
  src_commit_file="${src}.commit"
  src_version_file="${src}.version"
  dst="${OUTPUT_DIR}/go-reauth-proxy-linux-${arch}"

  [ -f "${src}" ] || fail "missing gateway binary after build: ${src}"
  [ -f "${src_version_file}" ] || \
    fail "missing gateway bundle version metadata after build: ${src_version_file}"
  [ -f "${src_commit_file}" ] || \
    fail "missing gateway source commit metadata after build: ${src_commit_file}"
  src_version="$(tr -d '\r\n' < "${src_version_file}")"
  [ "${src_version}" = "${BUNDLE_VERSION}" ] || \
    fail "gateway bundle version mismatch after build: expected ${BUNDLE_VERSION}, got ${src_version:-<empty>}"
  src_commit="$(tr -d '\r\n' < "${src_commit_file}")"
  [ "${src_commit}" = "${BUNDLE_COMMIT}" ] || \
    fail "gateway source commit mismatch after build: expected ${BUNDLE_COMMIT}, got ${src_commit:-<empty>}"

  cp "${src}" "${dst}"
  chmod +x "${dst}"
  log "Prepared ${dst}"
done

assert_checkout_locked "after gateway staging"
