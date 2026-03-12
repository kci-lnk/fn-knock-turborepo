#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

REMOTE_HOST="${FN_KNOCK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_REMOTE_DIR:-/tmp/fn-knock-fpk}"
APP_NAME="${FN_KNOCK_APP_NAME:-fn-knock}"
LOCAL_APP_DIR="${FN_KNOCK_LOCAL_APP_DIR:-apps/fn-knock}"
LOCAL_FPK_PATH="${FN_KNOCK_LOCAL_FPK_PATH:-apps/fn-knock/dist/fn-knock.fpk}"
REMOTE_FPK_PATH="${REMOTE_DIR}/fn-knock.fpk"
REMOTE_UI_INDEX="/usr/local/apps/@appcenter/${APP_NAME}/ui/index.cgi"
REMOTE_LOG_FILE="/usr/local/apps/@appdata/${APP_NAME}/info.log"
REMOTE_INSTALL_ENV_PATH="${REMOTE_DIR}/install.env"
REMOTE_APPCENTER_TMP_DIR="${FN_KNOCK_REMOTE_APPCENTER_TMP_DIR:-/tmp/appcenter}"
WIZARD_BACKEND_PORT="${FN_KNOCK_WIZARD_BACKEND_PORT:-7998}"
WIZARD_AUTH_PORT="${FN_KNOCK_WIZARD_AUTH_PORT:-7997}"
WIZARD_GO_BACKEND_PORT="${FN_KNOCK_WIZARD_GO_BACKEND_PORT:-7996}"
WIZARD_GO_REPROXY_PORT="${FN_KNOCK_WIZARD_GO_REPROXY_PORT:-7999}"

log() {
  echo "[fn-knock-deploy] $*"
}

get_remote_status() {
  ssh "${REMOTE_HOST}" "appcenter-cli status '${APP_NAME}' 2>/dev/null || true"
}

assert_remote_installed() {
  local status
  status="$(get_remote_status)"
  echo "${status}"
  if echo "${status}" | grep -qi "noinstall"; then
    echo "ERROR: application '${APP_NAME}' is not installed on remote host" >&2
    exit 1
  fi
}

resolve_remote_ui_index() {
  ssh "${REMOTE_HOST}" "for p in '${REMOTE_UI_INDEX}' '/usr/local/apps/@appcenter/${APP_NAME}/app/ui/index.cgi'; do if [ -f \"\$p\" ]; then echo \"\$p\"; exit 0; fi; done; exit 1"
}

run_local_package() {
  log "Step 1/4: Build package assets locally"
  ./apps/fn-knock/scripts/build-package.sh
}

run_remote_pack() {
  log "Step 2/4: Upload app sources to remote fnpack directory"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_DIR}'"
  rsync -az --delete "${LOCAL_APP_DIR}/" "${REMOTE_HOST}:${REMOTE_DIR}/"

  log "Step 2/4: Build FPK on remote host"
  ssh "${REMOTE_HOST}" "cd '${REMOTE_DIR}' && fnpack build -d ."

  log "Step 2/4: Pull generated FPK back to local workspace"
  mkdir -p "$(dirname "${LOCAL_FPK_PATH}")"
  scp "${REMOTE_HOST}:${REMOTE_FPK_PATH}" "${LOCAL_FPK_PATH}"
}

run_remote_install() {
  log "Step 3/4: Stop and uninstall old app version"
  ssh "${REMOTE_HOST}" "appcenter-cli stop '${APP_NAME}' || true"
  ssh "${REMOTE_HOST}" "appcenter-cli uninstall '${APP_NAME}' || true"

  log "Step 3/4: Prepare wizard env file for CLI installation"
  ssh "${REMOTE_HOST}" "cat > '${REMOTE_INSTALL_ENV_PATH}' <<'EOF'
wizard_backend_port=${WIZARD_BACKEND_PORT}
wizard_auth_port=${WIZARD_AUTH_PORT}
wizard_go_backend_port=${WIZARD_GO_BACKEND_PORT}
wizard_go_reproxy_port=${WIZARD_GO_REPROXY_PORT}
EOF"

  log "Step 3/4: Ensure appcenter temp directory exists"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_APPCENTER_TMP_DIR}'"

  log "Step 3/4: Install and start new FPK"
  if ! ssh "${REMOTE_HOST}" "appcenter-cli install-fpk '${REMOTE_FPK_PATH}' --env '${REMOTE_INSTALL_ENV_PATH}'"; then
    log "Step 3/4: Install failed, tailing appcenter error log for diagnostics"
    ssh "${REMOTE_HOST}" "tail -n 120 /var/log/trim_app_center/error.log || true"
    exit 1
  fi
  log "Step 3/4: Verify installation state"
  assert_remote_installed
  ssh "${REMOTE_HOST}" "appcenter-cli start '${APP_NAME}'"
  log "Step 3/4: Verify runtime state"
  assert_remote_installed

  log "Step 3/4: Tail runtime log"
  ssh "${REMOTE_HOST}" "tail -n 200 '${REMOTE_LOG_FILE}' || true"
}

run_remote_verify() {
  assert_remote_installed >/dev/null
  log "Step 4/4: Verify installed index.cgi hash"
  local local_hash
  local remote_hash
  local remote_ui_index
  local_hash="$(shasum -a 256 "${LOCAL_APP_DIR}/app/ui/index.cgi" | awk '{print $1}')"
  remote_ui_index="$(resolve_remote_ui_index)" || {
    echo "ERROR: unable to locate remote index.cgi for '${APP_NAME}'" >&2
    exit 1
  }
  remote_hash="$(ssh "${REMOTE_HOST}" "shasum -a 256 '${remote_ui_index}' | awk '{print \$1}'")"
  echo "local index.cgi  sha256: ${local_hash}"
  echo "remote index.cgi sha256: ${remote_hash}"
  echo "remote index.cgi path: ${remote_ui_index}"

  if [ "${local_hash}" != "${remote_hash}" ]; then
    echo "ERROR: installed index.cgi does not match local package file" >&2
    exit 1
  fi

  log "Step 4/4: Show key section from remote index.cgi"
  ssh "${REMOTE_HOST}" "sed -n '170,280p' '${remote_ui_index}'"
}

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/fn-knock-deploy.sh <command>

Commands:
  pack-remote     Run local package build + remote fnpack build + download FPK
  install-remote  Install/start app on remote host and print runtime logs
  verify-remote   Verify installed index.cgi hash and print key lines
  deploy          Run all steps in order (pack-remote -> install-remote -> verify-remote)

Optional env overrides:
  FN_KNOCK_REMOTE_HOST  (default: root@192.168.31.98)
  FN_KNOCK_REMOTE_DIR   (default: /tmp/fn-knock-fpk)
  FN_KNOCK_APP_NAME     (default: fn-knock)
  FN_KNOCK_LOCAL_APP_DIR (default: apps/fn-knock)
  FN_KNOCK_LOCAL_FPK_PATH (default: apps/fn-knock/dist/fn-knock.fpk)
  FN_KNOCK_WIZARD_BACKEND_PORT (default: 7998)
  FN_KNOCK_WIZARD_AUTH_PORT (default: 7997)
  FN_KNOCK_WIZARD_GO_BACKEND_PORT (default: 7996)
  FN_KNOCK_WIZARD_GO_REPROXY_PORT (default: 7999)
EOF
}

cmd="${1:-}"
case "${cmd}" in
  pack-remote)
    run_local_package
    run_remote_pack
    ;;
  install-remote)
    run_remote_install
    ;;
  verify-remote)
    run_remote_verify
    ;;
  deploy)
    run_local_package
    run_remote_pack
    run_remote_install
    run_remote_verify
    log "Completed deployment."
    ;;
  *)
    usage
    exit 1
    ;;
esac
