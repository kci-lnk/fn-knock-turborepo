#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
source "${ROOT_DIR}/scripts/version.sh"
APP_DIR="${ROOT_DIR}/apps/fn-knock-docker"
VERSION_FILE="${ROOT_DIR}/version.json"
MANIFEST_FILE="${APP_DIR}/manifest"

sync_manifest_version() {
  fn_knock_sync_manifest_version "${ROOT_DIR}" "${MANIFEST_FILE}" "[fn-knock-docker-fpk]"
  fn_knock_sync_rust_package_version "${ROOT_DIR}" "[fn-knock-docker-fpk]"
}

prepare_package() {
  sync_manifest_version

  chmod +x \
    "${APP_DIR}/cmd/main" \
    "${APP_DIR}/cmd/install_init" \
    "${APP_DIR}/cmd/install_callback" \
    "${APP_DIR}/cmd/uninstall_init" \
    "${APP_DIR}/cmd/uninstall_callback" \
    "${APP_DIR}/cmd/upgrade_init" \
    "${APP_DIR}/cmd/upgrade_callback" \
    "${APP_DIR}/cmd/config_init" \
    "${APP_DIR}/cmd/config_callback"

  mkdir -p "${APP_DIR}/dist"
  echo "[fn-knock-docker-fpk] Docker FPK package directory is ready: ${APP_DIR}"
}

usage() {
  cat <<'EOF'
Usage:
  ./apps/fn-knock-docker/scripts/build-package.sh [prepare]

Commands:
  prepare  Sync metadata and ensure executable lifecycle scripts (default)
EOF
}

cmd="${1:-prepare}"
case "${cmd}" in
  prepare)
    prepare_package
    ;;
  *)
    usage
    exit 1
    ;;
esac
