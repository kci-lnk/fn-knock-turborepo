#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-fpk}"
LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock/dist/fn-knock.fpk}"
REMOTE_FPK_PATH="${REMOTE_DIR}/fn-knock.fpk"
VERSION_FILE="${ROOT_DIR}/apps/server-admin/src/lib/app-version.ts"
MANIFEST_FILE="${ROOT_DIR}/apps/fn-knock/manifest"

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

  echo "[fn-knock] Building frontend apps..."
  npx turbo run build --filter=server-admin-view --filter=server-auth-view

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

  echo "[fn-knock] Copying server-admin bundle -> app/server/server-admin/index.js"
  cp "${ROOT_DIR}/apps/server-admin/dist/index.js" "${SERVER_ADMIN_DIR}/index.js"
  rm -f "${SERVER_ADMIN_DIR}/index.cjs"

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
  mkdir -p "$(dirname "${LOCAL_FPK_PATH}")"
  echo "[fn-knock] Pulling remote FPK: ${REMOTE_HOST}:${REMOTE_FPK_PATH} -> ${LOCAL_FPK_PATH}"
  scp "${REMOTE_HOST}:${REMOTE_FPK_PATH}" "${LOCAL_FPK_PATH}"
  echo "[fn-knock] FPK copied to ${LOCAL_FPK_PATH}"
}

usage() {
  cat <<'EOF'
Usage:
  ./apps/fn-knock/scripts/build-package.sh [build-assets|copy-fpk]

Commands:
  build-assets  Build and sync package assets (default)
  copy-fpk      Copy packaged FPK from remote host to local dist path
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
