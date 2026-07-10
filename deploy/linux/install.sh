#!/usr/bin/env bash
set -euo pipefail

DEFAULT_BASE_URL="https://cdn.fnknock.cn"
BASE_URL="${FN_KNOCK_BASE_URL:-${DEFAULT_BASE_URL}}"
COMMAND_FILE="${FN_KNOCK_COMMAND_FILE:-/usr/local/bin/knock}"
APP_ROOT="${FN_KNOCK_APP_ROOT:-/opt/fn-knock}"
WORK_DIR=""

if [ -t 1 ] && [ "${NO_COLOR:-}" != "1" ]; then
  C_RESET=$'\033[0m' C_TITLE=$'\033[1;38;5;45m' C_OK=$'\033[1;38;5;82m' C_ERR=$'\033[1;38;5;203m'
else
  C_RESET='' C_TITLE='' C_OK='' C_ERR=''
fi
log() { printf '%s【fn-knock 安装器】%s %s\n' "${C_TITLE}" "${C_RESET}" "$*"; }
success() { printf '%s✓ %s%s\n' "${C_OK}" "$*" "${C_RESET}"; }
fail() { printf '%s✗ %s%s\n' "${C_ERR}" "$*" "${C_RESET}" >&2; exit 1; }

cache_bust_url() {
  local url="$1" separator="?"
  case "${url}" in *\?*) separator="&" ;; esac
  printf '%s%scb=%s-%s-%s\n' "${url}" "${separator}" "$(date +%s)" "$$" "${RANDOM}"
}

cleanup() {
  [ -z "${WORK_DIR}" ] || rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

[ -n "${BASH_VERSION:-}" ] || fail "安装脚本需要使用 bash 执行"
[ "$(id -u)" -eq 0 ] || fail "需要 root 权限，请使用：sudo bash"
[ "$(uname -s)" = "Linux" ] || fail "仅支持 Linux 系统"
command -v systemctl >/dev/null 2>&1 || fail "系统缺少 systemd"
[ -d /run/systemd/system ] || fail "当前未运行 systemd"

normalize_arch() {
  case "$(uname -m)" in
    x86_64|amd64) printf '%s\n' amd64 ;;
    aarch64|arm64) printf '%s\n' arm64 ;;
    armv7l|armv8l|armhf|arm) printf '%s\n' arm ;;
    *) return 1 ;;
  esac
}

install_dependencies() {
  local missing=0
  for command_name in curl openssl unzip tar gzip ss; do
    command -v "${command_name}" >/dev/null 2>&1 || missing=1
  done
  [ "${missing}" = "1" ] || return 0

  log "正在安装所需系统依赖…"
  if command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get update -y
    DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl openssl unzip tar gzip iproute2
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y ca-certificates curl openssl unzip tar gzip iproute
  elif command -v yum >/dev/null 2>&1; then
    yum install -y ca-certificates curl openssl unzip tar gzip iproute
  else
    fail "缺少运行依赖，且未找到受支持的软件包管理器"
  fi
}

installed_version() {
  local release_file="${APP_ROOT}/current/release.json"
  if [ -f "${release_file}" ]; then
    sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${release_file}" | head -n1
  fi
}

choose_action() {
  local answer version
  if [ -x "${COMMAND_FILE}" ] && [ -f "${APP_ROOT}/current/release.json" ]; then
    version="$(installed_version)"
    log "检测到已安装的 fn-knock${version:+（版本 ${version}）}"
    cat >&2 <<'EOF'

请选择操作：
  1. 下载并安装最新版本
  2. 打开 fn-knock 管理菜单
  3. 查看服务状态
  4. 卸载 fn-knock
  0. 退出
EOF
    read -r -p "请选择 [1]: " answer </dev/tty || fail "需要可交互的终端"
    case "${answer:-1}" in
      1) return 0 ;;
      2) exec "${COMMAND_FILE}" ;;
      3) exec "${COMMAND_FILE}" status ;;
      4) exec "${COMMAND_FILE}" uninstall ;;
      0) exit 0 ;;
      *) fail "无效选择" ;;
    esac
  fi

  cat >&2 <<'EOF'

未检测到已安装的 fn-knock。
  1. 安装 fn-knock
  0. 退出
EOF
  read -r -p "请选择 [1]: " answer </dev/tty || fail "需要可交互的终端"
  case "${answer:-1}" in
    1) ;;
    0) exit 0 ;;
    *) fail "无效选择" ;;
  esac
}

manifest_value() {
  local file="$1" key="$2"
  awk -v wanted="${key}" '
    index($0, wanted "=") == 1 { count++; value = substr($0, length(wanted) + 2) }
    END { if (count != 1) exit 1; print value }
  ' "${file}"
}

file_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}

choose_action
install_dependencies
ARCH="$(normalize_arch)" || fail "不支持的系统架构：$(uname -m)"
WORK_DIR="$(mktemp -d /tmp/fn-knock-installer.XXXXXX)"
MANIFEST_FILE="${WORK_DIR}/latest.env"
ARCHIVE_FILE="${WORK_DIR}/release.tar.gz"

log "检测到系统架构：${ARCH}"
curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
  -H 'Cache-Control: no-cache' -H 'Pragma: no-cache' \
  -o "${MANIFEST_FILE}" "$(cache_bust_url "${BASE_URL%/}/linux/latest/${ARCH}.env")"

VERSION="$(manifest_value "${MANIFEST_FILE}" VERSION)" || fail "发布清单中的 VERSION 无效"
URL="$(manifest_value "${MANIFEST_FILE}" URL)" || fail "发布清单中的 URL 无效"
SHA256="$(manifest_value "${MANIFEST_FILE}" SHA256)" || fail "发布清单中的 SHA256 无效"
SIZE="$(manifest_value "${MANIFEST_FILE}" SIZE)" || fail "发布清单中的 SIZE 无效"

printf '%s' "${VERSION}" | grep -Eq '^[0-9][0-9A-Za-z._+-]*$' || fail "发布版本号无效"
printf '%s' "${SHA256}" | grep -Eq '^[0-9a-fA-F]{64}$' || fail "发布校验和无效"
printf '%s' "${SIZE}" | grep -Eq '^[1-9][0-9]*$' || fail "发布包大小无效"
case "${URL}" in
  https://*) ;;
  http://*) [ "${FN_KNOCK_ALLOW_INSECURE_HTTP:-0}" = "1" ] || fail "发布地址必须使用 HTTPS" ;;
  *) fail "发布地址必须为绝对 URL" ;;
esac

log "正在下载 fn-knock ${VERSION}…"
curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
  -o "${ARCHIVE_FILE}" "${URL}"

ACTUAL_SIZE="$(wc -c < "${ARCHIVE_FILE}" | tr -d '[:space:]')"
[ "${ACTUAL_SIZE}" = "${SIZE}" ] || fail "下载文件大小不匹配"
ACTUAL_SHA256="$(file_sha256 "${ARCHIVE_FILE}")"
[ "${ACTUAL_SHA256}" = "${SHA256}" ] || fail "下载文件校验和不匹配"
tar --warning=no-unknown-keyword --warning=no-timestamp -tzf "${ARCHIVE_FILE}" > "${WORK_DIR}/archive.list"
grep -qx 'fn-knock/release.json' "${WORK_DIR}/archive.list" || fail "发布包目录结构无效"
tar --warning=no-unknown-keyword --warning=no-timestamp -xzf "${ARCHIVE_FILE}" -C "${WORK_DIR}"
[ -x "${WORK_DIR}/fn-knock/bin/knock" ] || fail "发布包不包含管理命令"

log "正在安装前检测所需端口…"
"${WORK_DIR}/fn-knock/bin/knock" _prepare-install
"${WORK_DIR}/fn-knock/bin/knock" _install-extracted "${WORK_DIR}/fn-knock" "${VERSION}"

success "fn-knock ${VERSION} 安装完成！"
log "管理面板：http://<设备 IP>:7991（默认监听所有网卡）"
log "Go 代理端口：7999"
log "7998 是仅本机可访问的 Rust 内部 API，请勿对外转发"
log "如需公网访问，建议使用 HTTPS Nginx 反向代理与来源 IP 限制；运行 sudo knock nginx 可查看模板"
log "随时运行 sudo knock 可打开彩色管理菜单"
log "fn-knock 不会修改主机防火墙；请仅开放部署所需端口"
