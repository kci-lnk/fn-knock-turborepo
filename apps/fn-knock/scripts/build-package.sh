#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-fpk}"
LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock/dist/fn-knock.fpk}"
APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock}"
REMOTE_FPK_AMD64_PATH="${REMOTE_DIR}/${APP_NAME}-amd64.fpk"
REMOTE_FPK_ARM64_PATH="${REMOTE_DIR}/${APP_NAME}-arm64.fpk"
VERSION_FILE="${ROOT_DIR}/apps/server-admin/src/lib/app-version.ts"
MANIFEST_FILE="${ROOT_DIR}/apps/fn-knock/manifest"

derive_arch_fpk_path() {
  local base_path="$1"
  local arch="$2"
  local dir_name
  local file_name
  local file_stem

  dir_name="$(dirname "${base_path}")"
  file_name="$(basename "${base_path}")"
  file_stem="${file_name%.fpk}"

  if [ "${file_stem}" = "${file_name}" ]; then
    echo "${dir_name}/${file_name}-${arch}.fpk"
    return 0
  fi

  echo "${dir_name}/${file_stem}-${arch}.fpk"
}

LOCAL_FPK_AMD64_PATH="$(derive_arch_fpk_path "${LOCAL_FPK_PATH}" "amd64")"
LOCAL_FPK_ARM64_PATH="$(derive_arch_fpk_path "${LOCAL_FPK_PATH}" "arm64")"

sync_manifest_version() {
  if [ ! -f "${VERSION_FILE}" ]; then
    echo "[fn-knock] Missing version file: ${VERSION_FILE}" >&2
    exit 1
  fi

  if [ ! -f "${MANIFEST_FILE}" ]; then
    echo "[fn-knock] Missing manifest file: ${MANIFEST_FILE}" >&2
    exit 1
  fi

  local app_version
  app_version="$(sed -nE 's/^[[:space:]]*export[[:space:]]+const[[:space:]]+APP_LOCAL_VERSION[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${VERSION_FILE}" | head -n1)"
  if [ -z "${app_version}" ]; then
    echo "[fn-knock] Failed to parse APP_LOCAL_VERSION from ${VERSION_FILE}" >&2
    exit 1
  fi

  local current_manifest_version
  current_manifest_version="$(sed -nE 's/^version=(.*)$/\1/p' "${MANIFEST_FILE}" | head -n1)"

  if [ "${current_manifest_version}" = "${app_version}" ]; then
    echo "[fn-knock] Manifest version is already up to date: ${app_version}"
    return
  fi

  local tmp_manifest
  tmp_manifest="$(mktemp)"
  awk -v version="${app_version}" '
    BEGIN { updated = 0 }
    /^version=/ {
      print "version=" version
      updated = 1
      next
    }
    { print }
    END {
      if (!updated) {
        print "version=" version
      }
    }
  ' "${MANIFEST_FILE}" > "${tmp_manifest}"
  mv "${tmp_manifest}" "${MANIFEST_FILE}"

  echo "[fn-knock] Synced manifest version: ${current_manifest_version:-<empty>} -> ${app_version}"
}

build_package_assets() {
  cd "${ROOT_DIR}"

  echo "[fn-knock] Syncing manifest version from server-admin app version..."
  sync_manifest_version

  # These frontend apps import workspace source via Vite aliases.
  # Force rebuild here so packaging never reuses a stale Turbo cache entry.
  echo "[fn-knock] Building frontend apps (force rebuild to avoid stale workspace alias cache)..."
  npx turbo run build --filter=server-admin-view --filter=server-auth-view --force

  echo "[fn-knock] Building server-admin..."
  npm run build --workspace server-admin

  PKG_DIR="${ROOT_DIR}/apps/fn-knock/app"
  ADMIN_WWW_DIR="${PKG_DIR}/ui/www"
  AUTH_DIST_DIR="${PKG_DIR}/server-auth-view/dist"
  SERVER_ADMIN_DIR="${PKG_DIR}/server/server-admin"
  SERVER_ADMIN_RES_DIR="${SERVER_ADMIN_DIR}/resources"
  ACME_RESOURCE_SRC="${ROOT_DIR}/apps/server-admin/resources/acmesh.zip"

  echo "[fn-knock] Preparing package directories..."
  mkdir -p "${ADMIN_WWW_DIR}" "${AUTH_DIST_DIR}" "${SERVER_ADMIN_DIR}" "${SERVER_ADMIN_RES_DIR}"

  echo "[fn-knock] Syncing server-admin-view dist -> app/ui/www"
  rsync -a --delete "${ROOT_DIR}/apps/server-admin-view/dist/" "${ADMIN_WWW_DIR}/"

  echo "[fn-knock] Syncing server-auth-view dist -> app/server-auth-view/dist"
  rsync -a --delete "${ROOT_DIR}/apps/server-auth-view/dist/" "${AUTH_DIST_DIR}/"

  echo "[fn-knock] Syncing server-admin dist -> app/server/server-admin"
  find "${SERVER_ADMIN_DIR}" -maxdepth 1 -type f \( -name '*.js' -o -name '*.mjs' -o -name '*.cjs' -o -name '*.map' \) -delete
  rm -rf "${SERVER_ADMIN_DIR}/chunks"
  rsync -a "${ROOT_DIR}/apps/server-admin/dist/" "${SERVER_ADMIN_DIR}/"

  if [ ! -f "${ACME_RESOURCE_SRC}" ]; then
    echo "[fn-knock] Missing acme resource: ${ACME_RESOURCE_SRC}" >&2
    exit 1
  fi
  echo "[fn-knock] Copying bundled acme resource -> app/server/server-admin/resources/acmesh.zip"
  cp "${ACME_RESOURCE_SRC}" "${SERVER_ADMIN_RES_DIR}/acmesh.zip"

  chmod +x "${ROOT_DIR}/apps/fn-knock/cmd/main" "${ROOT_DIR}/apps/fn-knock/app/ui/index.cgi"

  echo "[fn-knock] Package assets are ready under apps/fn-knock/app"
}

copy_remote_fpk() {
  cd "${ROOT_DIR}"
  mkdir -p "$(dirname "${LOCAL_FPK_AMD64_PATH}")"
  echo "[fn-knock] Pulling remote FPK: ${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH} -> ${LOCAL_FPK_AMD64_PATH}"
  scp "${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH}" "${LOCAL_FPK_AMD64_PATH}"
  echo "[fn-knock] Pulling remote FPK: ${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH} -> ${LOCAL_FPK_ARM64_PATH}"
  scp "${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH}" "${LOCAL_FPK_ARM64_PATH}"
  echo "[fn-knock] FPK copied to ${LOCAL_FPK_AMD64_PATH} and ${LOCAL_FPK_ARM64_PATH}"
}

usage() {
  cat <<'EOF'
Usage:
  ./apps/fn-knock/scripts/build-package.sh [build-assets|copy-fpk]

Commands:
  build-assets  Build and sync package assets (default)
  copy-fpk      Copy amd64/arm64 packaged FPKs from remote host to local dist paths
EOF
}

cmd="${1:-build-assets}"
case "${cmd}" in
  build-assets)
    build_package_assets
    ;;
  copy-fpk)
    copy_remote_fpk
    ;;
  *)
    usage
    exit 1
    ;;
esac
