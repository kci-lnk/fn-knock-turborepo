#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"

ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
RUNTIME_DIR="${FN_KNOCK_PREPARED_RUNTIME_DIR:-${ARTIFACTS_DIR}/runtime}"
RUST_DIR="${FN_KNOCK_PREPARED_FPK_RUST_BACKEND_DIR:-${ARTIFACTS_DIR}/fpk-rust-backends}"
SOURCE_DIR="${FN_KNOCK_FPK_SOURCE_DIR:-${ROOT_DIR}/apps/fn-knock}"
OUTPUT_DIR="${FN_KNOCK_FPK_OUTPUT_DIR:-${ARTIFACTS_DIR}/fpk}"
FNPACK_BIN="${FN_KNOCK_FNPACK_BIN:-fnpack}"
VERSION="$(fn_knock_app_version "${ROOT_DIR}")"
WORK_DIR=""

log() {
  printf '[fn-knock-fpk] %s\n' "$*"
}

fail() {
  printf '[fn-knock-fpk] ERROR: %s\n' "$*" >&2
  exit 1
}

cleanup() {
  if [ -n "${WORK_DIR}" ] && [ -d "${WORK_DIR}" ]; then
    rm -rf "${WORK_DIR}"
  fi
}

validate_elf_arch() {
  local path="$1"
  local arch="$2"
  local info

  [ -f "${path}" ] || fail "missing binary: ${path}"
  info="$(file -b "${path}")"
  case "${arch}" in
    amd64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*x86-64' || \
        fail "${path} is not Linux amd64: ${info}"
      ;;
    arm64)
      printf '%s\n' "${info}" | grep -Eq 'ELF 64-bit LSB.*(ARM aarch64|aarch64)' || \
        fail "${path} is not Linux arm64: ${info}"
      ;;
    *)
      fail "unsupported FPK architecture: ${arch}"
      ;;
  esac
}

rewrite_manifest() {
  local manifest="$1"
  local platform="$2"
  local temp="${manifest}.tmp"

  awk -v version="${VERSION}" -v platform="${platform}" '
    /^version=/ { print "version=" version; next }
    /^platform=/ { print "platform=" platform; next }
    { print }
  ' "${manifest}" > "${temp}"
  mv "${temp}" "${manifest}"
}

validate_fpk() {
  local fpk="$1"
  local arch="$2"
  local platform="$3"
  local inspect_dir="$4"
  local listing
  local manifest
  local gateway

  [ -s "${fpk}" ] || fail "fnpack did not produce ${fpk}"
  manifest="$(tar -xOzf "${fpk}" manifest)" || fail "cannot read FPK manifest: ${fpk}"
  printf '%s\n' "${manifest}" | grep -Eq '^appname[[:space:]]*=[[:space:]]*fn-knock[[:space:]]*$' || \
    fail "unexpected FPK appname"
  printf '%s\n' "${manifest}" | grep -Eq "^version[[:space:]]*=[[:space:]]*${VERSION//./\\.}[[:space:]]*$" || \
    fail "unexpected FPK version"
  printf '%s\n' "${manifest}" | grep -Eq "^platform[[:space:]]*=[[:space:]]*${platform}[[:space:]]*$" || \
    fail "unexpected FPK platform"

  mkdir -p "${inspect_dir}"
  tar -xOzf "${fpk}" app.tgz > "${inspect_dir}/app.tgz"
  tar -xzf "${inspect_dir}/app.tgz" -C "${inspect_dir}"
  listing="$(tar -tzf "${inspect_dir}/app.tgz" | sed 's#^\./##')"
  printf '%s\n' "${listing}" | grep -qx 'server/server-admin-rs' || fail "FPK is missing Rust backend"
  printf '%s\n' "${listing}" | grep -qx "server/go-reauth-proxy-linux-${arch}" || \
    fail "FPK is missing Go gateway ${arch}"
  if [ "$(printf '%s\n' "${listing}" | grep -Ec '^server/go-reauth-proxy-linux-' || true)" -ne 1 ]; then
    fail "FPK contains gateways for more than one architecture"
  fi

  gateway="${inspect_dir}/server/go-reauth-proxy-linux-${arch}"
  validate_elf_arch "${inspect_dir}/server/server-admin-rs" "${arch}"
  validate_elf_arch "${gateway}" "${arch}"
}

build_arch() {
  local arch="$1"
  local platform="$2"
  local stage="${WORK_DIR}/stage-${arch}"
  local inspect="${WORK_DIR}/inspect-${arch}"
  local built
  local output="${OUTPUT_DIR}/fn-knock-${VERSION}-fnos-${arch}.fpk"

  mkdir -p "${stage}"
  rsync -a \
    --exclude dist \
    --exclude '*.fpk' \
    --exclude 'app/ui/www' \
    --exclude 'app/server-auth-view/dist' \
    --exclude 'app/server/server-admin' \
    --exclude 'app/server/go-reauth-proxy-linux-*' \
    --exclude 'app/server/server-admin-rs*' \
    "${SOURCE_DIR}/" "${stage}/"

  mkdir -p \
    "${stage}/app/ui/www" \
    "${stage}/app/server-auth-view/dist" \
    "${stage}/app/server/server-admin/resources" \
    "${stage}/app/server"
  rsync -a "${RUNTIME_DIR}/ui/www/" "${stage}/app/ui/www/"
  rsync -a "${RUNTIME_DIR}/server-auth-view/dist/" "${stage}/app/server-auth-view/dist/"
  cp "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" \
    "${stage}/app/server/server-admin/resources/acmesh.zip"
  cp "${RUNTIME_DIR}/server/go-reauth-proxy-linux-${arch}" \
    "${stage}/app/server/go-reauth-proxy-linux-${arch}"
  cp "${RUST_DIR}/server-admin-rs-linux-${arch}" "${stage}/app/server/server-admin-rs"

  chmod 755 \
    "${stage}/cmd/main" \
    "${stage}/app/ui/index.cgi" \
    "${stage}/app/server/go-reauth-proxy-linux-${arch}" \
    "${stage}/app/server/server-admin-rs"
  validate_elf_arch "${stage}/app/server/go-reauth-proxy-linux-${arch}" "${arch}"
  validate_elf_arch "${stage}/app/server/server-admin-rs" "${arch}"
  rewrite_manifest "${stage}/manifest" "${platform}"

  (
    cd "${stage}"
    "${FNPACK_BIN}" build -d .
  )
  built="${stage}/fn-knock.fpk"
  [ -f "${built}" ] || built="$(find "${stage}" -maxdepth 1 -type f -name '*.fpk' | head -n1)"
  [ -n "${built}" ] || fail "unable to locate fnpack output for ${arch}"
  mv "${built}" "${output}"
  validate_fpk "${output}" "${arch}" "${platform}" "${inspect}"
  log "built ${output}"
}

command -v file >/dev/null 2>&1 || fail "missing required command: file"
command -v rsync >/dev/null 2>&1 || fail "missing required command: rsync"
command -v tar >/dev/null 2>&1 || fail "missing required command: tar"
command -v "${FNPACK_BIN}" >/dev/null 2>&1 || fail "missing fnpack binary: ${FNPACK_BIN}"
[ -d "${RUNTIME_DIR}/ui/www" ] || fail "missing prepared admin UI: ${RUNTIME_DIR}"
[ -d "${RUNTIME_DIR}/server-auth-view/dist" ] || fail "missing prepared auth UI: ${RUNTIME_DIR}"

mkdir -p "${OUTPUT_DIR}"
rm -f "${OUTPUT_DIR}"/fn-knock-*-fnos-*.fpk
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-fpk.XXXXXX")"
trap cleanup EXIT

build_arch amd64 x86
build_arch arm64 arm
