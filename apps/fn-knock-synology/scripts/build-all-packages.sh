#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
RUNTIME_DIR="${FN_KNOCK_PREPARED_RUNTIME_DIR:-${ARTIFACTS_DIR}/runtime}"
BUILD_SCRIPT="${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh"

log() {
  printf '[fn-knock-synology-all] %s\n' "$*"
}

fail() {
  printf '[fn-knock-synology-all] ERROR: %s\n' "$*" >&2
  exit 1
}

[ -z "${FN_KNOCK_SYNOLOGY_OUTPUT:-}" ] || \
  fail "FN_KNOCK_SYNOLOGY_OUTPUT cannot be used when building all architectures"

if [ "${FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE:-0}" != "1" ]; then
  log "preparing shared runtime artifacts for x86_64, armv8, and armv7"
  FN_KNOCK_MUSL_ARCHES="amd64 arm64 arm" \
  FN_KNOCK_RUNTIME_GATEWAY_ARCHES="amd64 arm64 arm" \
  FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD="${FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD:-1}" \
    bash "${ROOT_DIR}/scripts/fn-knock-prepare-artifacts.sh" openwrt
else
  log "using existing prepared artifacts"
fi

for target in x86_64:amd64 armv8:arm64 armv7:arm; do
  synology_arch="${target%%:*}"
  runtime_arch="${target#*:}"
  gateway="${RUNTIME_DIR}/server/go-reauth-proxy-linux-${runtime_arch}"
  [ -x "${gateway}" ] || fail "missing prepared gateway: ${gateway}"

  log "building ${synology_arch} package"
  FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE=1 \
  FN_KNOCK_SYNOLOGY_ARCH="${synology_arch}" \
  FN_KNOCK_SYNOLOGY_GATEWAY_BIN="${gateway}" \
    bash "${BUILD_SCRIPT}" "${synology_arch}"
done

log "all Synology packages are ready in ${ROOT_DIR}/dist/synology"
