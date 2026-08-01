#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

source "${ROOT_DIR}/scripts/fn-knock-lite-sync-go-grpc.sh"

export FN_KNOCK_APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock-lite}"
export FN_KNOCK_LOCAL_APP_DIR="${FN_KNOCK_LOCAL_APP_DIR:-apps/fn-knock-lite}"
export FN_KNOCK_LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock-lite/dist/fn-knock-lite.fpk}"
export FN_KNOCK_REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-lite-fpk}"
export FN_KNOCK_REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
export FN_KNOCK_WIZARD_ADMIN_VIEW_PORT="${FN_KNOCK_WIZARD_ADMIN_VIEW_PORT:-8991}"
export FN_KNOCK_WIZARD_BACKEND_PORT="${FN_KNOCK_WIZARD_BACKEND_PORT:-8998}"
export FN_KNOCK_WIZARD_AUTH_PORT="${FN_KNOCK_WIZARD_AUTH_PORT:-8997}"
export FN_KNOCK_WIZARD_GO_BACKEND_PORT="${FN_KNOCK_WIZARD_GO_BACKEND_PORT:-8996}"
export FN_KNOCK_WIZARD_GO_REPROXY_PORT="${FN_KNOCK_WIZARD_GO_REPROXY_PORT:-8999}"

exec bash "${ROOT_DIR}/scripts/fn-knock-deploy.sh" "$@"
