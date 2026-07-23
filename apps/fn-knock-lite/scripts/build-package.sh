#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"

export FN_KNOCK_APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock-lite}"
export FN_KNOCK_FPK_PACKAGE_DIR="${FN_KNOCK_FPK_PACKAGE_DIR:-${ROOT_DIR}/apps/fn-knock-lite}"
export FN_KNOCK_LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock-lite/dist/fn-knock-lite.fpk}"
export FN_KNOCK_REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-lite-fpk}"
export FN_KNOCK_ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-lite-artifacts}"
export FN_KNOCK_FRONTEND_TARGET="fpk-lite"
export VITE_FN_KNOCK_DEFAULT_AUTH_PORT="${VITE_FN_KNOCK_DEFAULT_AUTH_PORT:-8997}"
export FN_KNOCK_FORCE_FRONTEND_REBUILD="${FN_KNOCK_FORCE_FRONTEND_REBUILD:-1}"

exec "${ROOT_DIR}/apps/fn-knock/scripts/build-package.sh" "$@"
