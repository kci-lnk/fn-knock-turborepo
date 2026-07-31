#!/bin/bash
set -euo pipefail

ROOT_DIR="${FN_KNOCK_ROOT_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)}"
TAG="${1:-${FN_KNOCK_RELEASE_TAG:-${GITHUB_REF_NAME:-}}}"

log() {
  printf '[release-preflight] %s\n' "$*"
}

fail() {
  printf '[release-preflight] ERROR: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

read_manifest_value() {
  local path="$1"
  local key="$2"
  sed -nE "s/^${key}=(.*)$/\\1/p" "${path}" | head -n1
}

read_cargo_package_version() {
  local path="$1"
  awk '
    /^\[package\]/ { in_package = 1; next }
    /^\[/ && in_package { exit }
    in_package && /^[[:space:]]*version[[:space:]]*=/ {
      value = $0
      sub(/^[^"]*"/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' "${path}"
}

read_lock_package_version() {
  local path="$1"
  local package_name="$2"
  awk -v package_name="${package_name}" '
    /^\[\[package\]\]/ {
      in_package = 1
      name = ""
      version = ""
      next
    }
    in_package && /^name = "/ {
      name = $0
      sub(/^name = "/, "", name)
      sub(/"$/, "", name)
      next
    }
    in_package && /^version = "/ {
      version = $0
      sub(/^version = "/, "", version)
      sub(/"$/, "", version)
      if (name == package_name) {
        print version
        exit
      }
    }
  ' "${path}"
}

assert_equal() {
  local label="$1"
  local expected="$2"
  local actual="$3"
  [ "${actual}" = "${expected}" ] || \
    fail "${label} mismatch: expected ${expected}, got ${actual:-<empty>}"
}

require_cmd jq
require_cmd cargo

VERSION="$(jq -er '.version | strings | select(length > 0)' "${ROOT_DIR}/version.json")"
CONTROL_API_VERSION="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
[ -n "${TAG}" ] || TAG="v${VERSION}"
printf '%s\n' "${TAG}" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$' || \
  fail "release tag must match vX.Y.Z: ${TAG}"
assert_equal "tag/version" "v${VERSION}" "${TAG}"

assert_equal \
  "fnOS manifest version" \
  "${VERSION}" \
  "$(read_manifest_value "${ROOT_DIR}/apps/fn-knock/manifest" version)"
assert_equal \
  "server-admin-rs Cargo version" \
  "${VERSION}" \
  "$(read_cargo_package_version "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml")"
assert_equal \
  "server-admin-rs lock version" \
  "${VERSION}" \
  "$(read_lock_package_version "${ROOT_DIR}/apps/server-admin-rs/Cargo.lock" server-admin-rs)"
assert_equal \
  "desktop package version" \
  "${VERSION}" \
  "$(jq -er '.version' "${ROOT_DIR}/apps/fn-knock-desktop/package.json")"
assert_equal \
  "desktop package-lock version" \
  "${VERSION}" \
  "$(jq -er '.packages["apps/fn-knock-desktop"].version' "${ROOT_DIR}/package-lock.json")"
assert_equal \
  "desktop Cargo version" \
  "${VERSION}" \
  "$(read_cargo_package_version "${ROOT_DIR}/apps/fn-knock-desktop/native/Cargo.toml")"
assert_equal \
  "desktop lock version" \
  "${VERSION}" \
  "$(read_lock_package_version "${ROOT_DIR}/apps/fn-knock-desktop/native/Cargo.lock" fn-knock-desktop)"

RELEASE_NOTES="${ROOT_DIR}/release-notes/${VERSION}.md"
[ -s "${RELEASE_NOTES}" ] || fail "release notes are missing or empty: ${RELEASE_NOTES}"
grep -q '[^[:space:]]' "${RELEASE_NOTES}" || fail "release notes contain only whitespace"

if [ "${FN_KNOCK_PREFLIGHT_SKIP_CARGO_METADATA:-0}" != "1" ]; then
  cargo metadata \
    --locked \
    --no-deps \
    --format-version 1 \
    --manifest-path "${ROOT_DIR}/apps/server-admin-rs/Cargo.toml" >/dev/null
  cargo metadata \
    --locked \
    --no-deps \
    --format-version 1 \
    --manifest-path "${ROOT_DIR}/apps/fn-knock-desktop/native/Cargo.toml" >/dev/null
fi

if [ -n "${GITHUB_OUTPUT:-}" ]; then
  {
    printf 'version=%s\n' "${VERSION}"
    printf 'tag=%s\n' "${TAG}"
    printf 'control_api_version=%s\n' "${CONTROL_API_VERSION}"
    printf 'release_notes=%s\n' "${RELEASE_NOTES}"
  } >> "${GITHUB_OUTPUT}"
fi

log "release contract is valid: ${TAG}, control API ${CONTROL_API_VERSION}"
