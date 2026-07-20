#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
OUTPUT_DIR="${FN_KNOCK_RELEASE_ASSETS_DIR:-${ROOT_DIR}/dist/release-assets}"
FNPACK_BIN="${FN_KNOCK_FNPACK_BIN:-fnpack}"

log() {
  printf '[fn-knock-release-assemble] %s\n' "$*"
}

fail() {
  printf '[fn-knock-release-assemble] ERROR: %s\n' "$*" >&2
  exit 1
}

copy_matches() {
  local label="$1"
  shift
  local matches=()
  local candidate

  for candidate in "$@"; do
    [ -f "${candidate}" ] || continue
    matches+=("${candidate}")
  done
  [ "${#matches[@]}" -gt 0 ] || fail "no ${label} outputs were found"
  cp "${matches[@]}" "${OUTPUT_DIR}/"
  log "collected ${#matches[@]} ${label} files"
}

[ -d "${ARTIFACTS_DIR}/runtime" ] || fail "missing prepared runtime"
[ -d "${ARTIFACTS_DIR}/fpk-rust-backends" ] || fail "missing GNU Rust backends"
[ -d "${ARTIFACTS_DIR}/musl-rust-backends" ] || fail "missing musl Rust backends"

mkdir -p "${OUTPUT_DIR}"
rm -f "${OUTPUT_DIR}"/*

log "building generic Linux archives from prebuilt inputs"
FN_KNOCK_PREBUILT_ONLY=1 \
FN_KNOCK_RUNTIME_GATEWAY_ARCHES="amd64 arm64 arm" \
FN_KNOCK_MUSL_ARCHES="amd64 arm64 arm" \
  bash "${ROOT_DIR}/scripts/fn-knock-prepare-artifacts.sh" linux

log "building fnOS FPK packages locally"
FN_KNOCK_FNPACK_BIN="${FNPACK_BIN}" \
  bash "${ROOT_DIR}/scripts/fn-knock-package-fpk.sh"

log "building OpenWrt IPK and APK packages from prebuilt inputs"
FN_KNOCK_ARTIFACTS_ALREADY_PREPARED=1 \
FN_KNOCK_USE_PREPARED_ARTIFACTS=1 \
FN_KNOCK_OPENWRT_RUST_BACKEND_BIN_DIR="${ARTIFACTS_DIR}/musl-rust-backends" \
  bash "${ROOT_DIR}/scripts/build-openwrt-ipk.sh"

log "building Synology SPKs from prebuilt inputs"
FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE=1 \
  bash "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-all-packages.sh"

copy_matches "FPK" "${ARTIFACTS_DIR}/fpk/"*.fpk
copy_matches "Linux" "${ARTIFACTS_DIR}/linux/"*.tar.gz "${ARTIFACTS_DIR}/linux/"*.sha256
copy_matches "OpenWrt" "${ROOT_DIR}/dist/openwrt/"*.ipk "${ROOT_DIR}/dist/openwrt/"*.apk
copy_matches "Synology" "${ROOT_DIR}/dist/synology/"*.spk "${ROOT_DIR}/dist/synology/"*.spk.sha256

log "release assets are ready in ${OUTPUT_DIR}"
