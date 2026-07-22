#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

REMOTE_HOST="${FN_KNOCK_DOCKER_FPK_REMOTE_HOST:-root@192.168.31.98}"
REMOTE_DIR="${FN_KNOCK_DOCKER_FPK_REMOTE_DIR:-/tmp/fn-knock-docker-fpk}"
APP_NAME="${FN_KNOCK_DOCKER_FPK_APP_NAME:-fn-knock-docker}"
LOCAL_APP_DIR="${FN_KNOCK_DOCKER_FPK_LOCAL_APP_DIR:-apps/fn-knock-docker}"
LOCAL_FPK_PATH="${FN_KNOCK_DOCKER_FPK_LOCAL_FPK_PATH:-apps/fn-knock-docker/dist/fn-knock-docker.fpk}"
REMOTE_SOURCE_DIR="${REMOTE_DIR}/src"
REMOTE_BUILD_AMD64_DIR="${REMOTE_DIR}/build-amd64"
REMOTE_BUILD_ARM64_DIR="${REMOTE_DIR}/build-arm64"
REMOTE_FPK_AMD64_PATH="${REMOTE_DIR}/${APP_NAME}-amd64.fpk"
REMOTE_FPK_ARM64_PATH="${REMOTE_DIR}/${APP_NAME}-arm64.fpk"
REMOTE_INSTALL_ENV_PATH="${REMOTE_DIR}/install.env"
WIZARD_ADMIN_VIEW_PORT="${FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_VIEW_PORT:-7991}"
WIZARD_BACKEND_PORT="${FN_KNOCK_DOCKER_FPK_WIZARD_BACKEND_PORT:-7998}"
WIZARD_AUTH_PORT="${FN_KNOCK_DOCKER_FPK_WIZARD_AUTH_PORT:-7997}"
WIZARD_GO_BACKEND_PORT="${FN_KNOCK_DOCKER_FPK_WIZARD_GO_BACKEND_PORT:-7996}"
WIZARD_GO_REPROXY_PORT="${FN_KNOCK_DOCKER_FPK_WIZARD_GO_REPROXY_PORT:-8999}"
WIZARD_TZ="${FN_KNOCK_DOCKER_FPK_WIZARD_TZ:-Asia/Shanghai}"
WIZARD_DOCKER_IPV4_SUBNET="${FN_KNOCK_DOCKER_FPK_WIZARD_DOCKER_IPV4_SUBNET:-172.30.0.0/16}"
WIZARD_DOCKER_IPV6_SUBNET="${FN_KNOCK_DOCKER_FPK_WIZARD_DOCKER_IPV6_SUBNET:-fd42:fb33:7f7a:100::/64}"
WIZARD_ADMIN_TRUSTED_PROXY_CIDRS="${FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_TRUSTED_PROXY_CIDRS:-}"
WIZARD_ADMIN_SESSION_BINDING="${FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_SESSION_BINDING:-soft}"
WIZARD_DISCOVER_PROXY_TOKEN="${FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_PROXY_TOKEN:-}"
WIZARD_DISCOVER_LAN_CIDRS="${FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_LAN_CIDRS:-}"
WIZARD_DISCOVER_LAN_IP="${FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_LAN_IP:-}"
REMOTE_WAIT_TIMEOUT="${FN_KNOCK_DOCKER_FPK_WAIT_TIMEOUT:-180}"

log() {
  echo "[fn-knock-docker-fpk] $*"
}

fail() {
  echo "ERROR: $*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || fail "missing required command: ${cmd}"
}

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

run_local_prepare() {
  log "Step 1/4: Prepare Docker FPK package directory"
  "${ROOT_DIR}/apps/fn-knock-docker/scripts/build-package.sh" prepare
}

run_remote_pack_for_arch() {
  local arch="$1"
  local build_dir="$2"
  local output_path="$3"

  log "Step 2/4: Build ${arch} Docker FPK on remote host"
  ssh "${REMOTE_HOST}" bash -s -- "${REMOTE_SOURCE_DIR}" "${build_dir}" "${output_path}" "${APP_NAME}" "${arch}" <<'EOF'
set -euo pipefail

source_dir="$1"
build_dir="$2"
output_path="$3"
app_name="$4"
arch="$5"

case "${arch}" in
  amd64)
    manifest_platform="x86"
    ;;
  arm64)
    manifest_platform="arm"
    ;;
  *)
    echo "[remote-fn-knock-docker-fpk] unsupported arch: ${arch}" >&2
    exit 1
    ;;
esac

rm -rf "${build_dir}"
mkdir -p "${build_dir}"
rsync -a --delete "${source_dir}/" "${build_dir}/"

manifest_file="${build_dir}/manifest"
tmp_manifest="$(mktemp)"
awk -v platform="${manifest_platform}" '
  /^platform=/ {
    print "platform=" platform
    updated = 1
    next
  }
  { print }
  END {
    if (!updated) {
      print "platform=" platform
    }
  }
' "${manifest_file}" > "${tmp_manifest}"
mv "${tmp_manifest}" "${manifest_file}"

cd "${build_dir}"
rm -f "${app_name}.fpk"
fnpack build -d .
mv -f "${app_name}.fpk" "${output_path}"
echo "[remote-fn-knock-docker-fpk] built ${arch} package -> ${output_path}"
EOF
}

verify_fpk_payload() {
  local fpk_path="$1"
  local expected_platform="$2"
  local app_listing
  local manifest_content
  local resource_content

  [ -f "${fpk_path}" ] || fail "missing local FPK: ${fpk_path}"

  app_listing="$(tar -xOzf "${fpk_path}" app.tgz | tar -tzf - | sed 's#^\./##')" || \
    fail "failed to inspect app payload: ${fpk_path}"
  printf '%s\n' "${app_listing}" | grep -Fxq "docker/docker-compose.yaml" || \
    fail "FPK ${fpk_path} is missing app/docker/docker-compose.yaml"

  manifest_content="$(tar -xOzf "${fpk_path}" manifest)" || \
    fail "failed to read manifest from ${fpk_path}"
  printf '%s\n' "${manifest_content}" | grep -Eq "^[[:space:]]*appname[[:space:]]*=[[:space:]]*${APP_NAME}$" || \
    fail "FPK ${fpk_path} has unexpected appname"
  printf '%s\n' "${manifest_content}" | grep -Eq "^[[:space:]]*platform[[:space:]]*=[[:space:]]*${expected_platform}$" || \
    fail "FPK ${fpk_path} has unexpected platform; expected ${expected_platform}"

  resource_content="$(tar -xOzf "${fpk_path}" config/resource)" || \
    fail "failed to read config/resource from ${fpk_path}"
  printf '%s\n' "${resource_content}" | grep -q '"docker-project"' || \
    fail "FPK ${fpk_path} is missing docker-project resource"
}

run_remote_pack() {
  require_cmd ssh
  require_cmd scp
  require_cmd rsync

  log "Step 2/4: Upload Docker FPK sources to ${REMOTE_HOST}:${REMOTE_SOURCE_DIR}"
  ssh "${REMOTE_HOST}" "mkdir -p '${REMOTE_DIR}' '${REMOTE_SOURCE_DIR}'"
  rsync -az --delete --exclude 'dist/' "${LOCAL_APP_DIR}/" "${REMOTE_HOST}:${REMOTE_SOURCE_DIR}/"

  run_remote_pack_for_arch "amd64" "${REMOTE_BUILD_AMD64_DIR}" "${REMOTE_FPK_AMD64_PATH}"
  run_remote_pack_for_arch "arm64" "${REMOTE_BUILD_ARM64_DIR}" "${REMOTE_FPK_ARM64_PATH}"

  copy_remote_fpk
}

copy_remote_fpk() {
  require_cmd scp

  log "Step 3/4: Pull generated Docker FPKs back to local workspace"
  mkdir -p "$(dirname "${LOCAL_FPK_AMD64_PATH}")"
  scp "${REMOTE_HOST}:${REMOTE_FPK_AMD64_PATH}" "${LOCAL_FPK_AMD64_PATH}"
  scp "${REMOTE_HOST}:${REMOTE_FPK_ARM64_PATH}" "${LOCAL_FPK_ARM64_PATH}"

  verify_fpk_payload "${LOCAL_FPK_AMD64_PATH}" "x86"
  verify_fpk_payload "${LOCAL_FPK_ARM64_PATH}" "arm"
  log "Docker FPK copied to ${LOCAL_FPK_AMD64_PATH} and ${LOCAL_FPK_ARM64_PATH}"
}

write_remote_install_env() {
  case "${WIZARD_ADMIN_SESSION_BINDING}" in
    soft | strict) ;;
    *) fail "admin session binding must be soft or strict" ;;
  esac
  if [ -n "${WIZARD_DISCOVER_PROXY_TOKEN}" ] && \
    [[ ! "${WIZARD_DISCOVER_PROXY_TOKEN}" =~ ^[A-Za-z0-9._~-]{32,256}$ ]]; then
    fail "discovery proxy token must contain 32-256 safe characters"
  fi
  log "Step 4/4: Prepare Docker FPK install env on remote host"
  ssh "${REMOTE_HOST}" "cat > '${REMOTE_INSTALL_ENV_PATH}' <<'EOF'
wizard_admin_view_port=${WIZARD_ADMIN_VIEW_PORT}
wizard_backend_port=${WIZARD_BACKEND_PORT}
wizard_auth_port=${WIZARD_AUTH_PORT}
wizard_go_backend_port=${WIZARD_GO_BACKEND_PORT}
wizard_go_reproxy_port=${WIZARD_GO_REPROXY_PORT}
wizard_tz=${WIZARD_TZ}
wizard_docker_ipv4_subnet=${WIZARD_DOCKER_IPV4_SUBNET}
wizard_docker_ipv6_subnet=${WIZARD_DOCKER_IPV6_SUBNET}
wizard_admin_trusted_proxy_cidrs=${WIZARD_ADMIN_TRUSTED_PROXY_CIDRS}
wizard_admin_session_binding=${WIZARD_ADMIN_SESSION_BINDING}
wizard_discover_proxy_token=${WIZARD_DISCOVER_PROXY_TOKEN}
wizard_discover_lan_cidrs=${WIZARD_DISCOVER_LAN_CIDRS}
wizard_discover_lan_ip=${WIZARD_DISCOVER_LAN_IP}
EOF"
}

wait_for_remote_docker_runtime() {
  local start_ts
  local now_ts
  local state
  local runtime_status
  local health_status

  log "Step 4/4: Wait for Docker runtime container"
  start_ts="$(date +%s)"

  while true; do
    state="$(
      ssh "${REMOTE_HOST}" "docker inspect --format '{{.State.Status}} {{if .State.Health}}{{.State.Health.Status}}{{end}}' fn-knock-docker 2>/dev/null || true"
    )"
    runtime_status="$(printf '%s' "${state}" | awk '{print $1}')"
    health_status="$(printf '%s' "${state}" | awk '{print $2}')"

    case "${runtime_status}:${health_status}" in
      running:healthy | running:)
        log "Remote Docker runtime is ${runtime_status}${health_status:+/${health_status}}"
        return 0
        ;;
      running:starting | created:* | restarting:* | :*)
        ;;
      running:unhealthy | exited:* | dead:*)
        ssh "${REMOTE_HOST}" "docker ps -a --filter name=fn-knock-docker --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' || true"
        ssh "${REMOTE_HOST}" "docker logs --tail=160 fn-knock-docker 2>&1 || true"
        ssh "${REMOTE_HOST}" "tail -n 160 /var/log/trim_app_center/error.log || true"
        fail "remote Docker runtime is not running: ${state:-<missing>}"
        ;;
      *)
        log "Remote Docker runtime state: ${state:-<missing>}"
        ;;
    esac

    now_ts="$(date +%s)"
    if [ $((now_ts - start_ts)) -ge "${REMOTE_WAIT_TIMEOUT}" ]; then
      ssh "${REMOTE_HOST}" "docker ps -a --filter name=fn-knock-docker --format 'table {{.Names}}\t{{.Status}}\t{{.Ports}}' || true"
      ssh "${REMOTE_HOST}" "docker logs --tail=160 fn-knock-docker 2>&1 || true"
      ssh "${REMOTE_HOST}" "tail -n 160 /var/log/trim_app_center/error.log || true"
      fail "timed out waiting for remote Docker runtime after ${REMOTE_WAIT_TIMEOUT}s"
    fi

    sleep 3
  done
}

run_remote_install_x86() {
  require_cmd ssh

  log "Step 4/4: Stop and uninstall old ${APP_NAME} on ${REMOTE_HOST}"
  ssh "${REMOTE_HOST}" "appcenter-cli stop '${APP_NAME}' || true"
  ssh "${REMOTE_HOST}" "appcenter-cli uninstall '${APP_NAME}' || true"
  write_remote_install_env

  log "Step 4/4: Install x86 Docker FPK: ${REMOTE_FPK_AMD64_PATH}"
  if ! ssh "${REMOTE_HOST}" "appcenter-cli install-fpk '${REMOTE_FPK_AMD64_PATH}' --env '${REMOTE_INSTALL_ENV_PATH}'"; then
    log "Step 4/4: Install failed, tailing appcenter error log for diagnostics"
    ssh "${REMOTE_HOST}" "tail -n 160 /var/log/trim_app_center/error.log || true"
    exit 1
  fi

  log "Step 4/4: Start ${APP_NAME}"
  if ! ssh "${REMOTE_HOST}" "appcenter-cli start '${APP_NAME}'"; then
    log "Step 4/4: Start failed, tailing appcenter error log for diagnostics"
    ssh "${REMOTE_HOST}" "tail -n 160 /var/log/trim_app_center/error.log || true"
    exit 1
  fi

  log "Step 4/4: Remote app status"
  ssh "${REMOTE_HOST}" "appcenter-cli status '${APP_NAME}' || true"
  wait_for_remote_docker_runtime
}

usage() {
  cat <<'EOF'
Usage:
  bash ./scripts/fn-knock-docker-fpk.sh <command>

Commands:
  pack-remote     Prepare local Docker FPK sources, build amd64/arm64 FPKs on remote fnOS, and copy them back
  install-remote  Stop/uninstall old remote app, install the x86 FPK, and start it
  deploy          Run pack-remote, then install the x86 FPK on the remote host
  copy-back       Copy previously generated remote amd64/arm64 FPKs back to local dist paths

Optional env overrides:
  FN_KNOCK_DOCKER_FPK_REMOTE_HOST       (default: root@192.168.31.98)
  FN_KNOCK_DOCKER_FPK_REMOTE_DIR        (default: /tmp/fn-knock-docker-fpk)
  FN_KNOCK_DOCKER_FPK_APP_NAME          (default: fn-knock-docker)
  FN_KNOCK_DOCKER_FPK_LOCAL_APP_DIR     (default: apps/fn-knock-docker)
  FN_KNOCK_DOCKER_FPK_LOCAL_FPK_PATH    (default: apps/fn-knock-docker/dist/fn-knock-docker.fpk; downloads as -amd64/-arm64)
  FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_VIEW_PORT              (default: 7991)
  FN_KNOCK_DOCKER_FPK_WIZARD_BACKEND_PORT                 (default: 7998)
  FN_KNOCK_DOCKER_FPK_WIZARD_AUTH_PORT                    (default: 7997)
  FN_KNOCK_DOCKER_FPK_WIZARD_GO_BACKEND_PORT              (default: 7996)
  FN_KNOCK_DOCKER_FPK_WIZARD_GO_REPROXY_PORT              (default: 8999; avoids the native fn-knock 7999 port)
  FN_KNOCK_DOCKER_FPK_WIZARD_TZ                           (default: Asia/Shanghai)
  FN_KNOCK_DOCKER_FPK_WIZARD_DOCKER_IPV4_SUBNET           (default: 172.30.0.0/16)
  FN_KNOCK_DOCKER_FPK_WIZARD_DOCKER_IPV6_SUBNET           (default: fd42:fb33:7f7a:100::/64)
  FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_TRUSTED_PROXY_CIDRS    (default: empty)
  FN_KNOCK_DOCKER_FPK_WIZARD_ADMIN_SESSION_BINDING        (default: soft; soft or strict)
  FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_PROXY_TOKEN         (default: empty; 32-256 safe characters)
  FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_LAN_CIDRS           (default: empty; comma/space-separated IPs or CIDRs)
  FN_KNOCK_DOCKER_FPK_WIZARD_DISCOVER_LAN_IP              (default: empty)
  FN_KNOCK_DOCKER_FPK_WAIT_TIMEOUT                        (default: 180 seconds)
EOF
}

case "${1:-}" in
  pack-remote)
    run_local_prepare
    run_remote_pack
    ;;
  install-remote)
    run_remote_install_x86
    ;;
  deploy)
    run_local_prepare
    run_remote_pack
    run_remote_install_x86
    ;;
  copy-back)
    copy_remote_fpk
    ;;
  *)
    usage
    exit 1
    ;;
esac
