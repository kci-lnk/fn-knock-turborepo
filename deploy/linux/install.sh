#!/bin/sh
set -eu

DEFAULT_BASE_URL="https://cdn.fnknock.cn"
BASE_URL="${FN_KNOCK_BASE_URL:-${DEFAULT_BASE_URL}}"
COMMAND_FILE="${FN_KNOCK_COMMAND_FILE:-/usr/local/bin/knock}"
APP_ROOT="${FN_KNOCK_APP_ROOT:-/opt/fn-knock}"
WORK_DIR=""

if [ -t 1 ] && [ "${NO_COLOR:-}" != "1" ]; then
  ESC="$(printf '\033')"
  C_RESET="${ESC}[0m" C_TITLE="${ESC}[1;38;5;45m" C_ACCENT="${ESC}[1;38;5;213m" C_OK="${ESC}[1;38;5;82m" C_ERR="${ESC}[1;38;5;203m" C_DIM="${ESC}[38;5;245m"
else
  C_RESET='' C_TITLE='' C_ACCENT='' C_OK='' C_ERR='' C_DIM=''
fi
log() { printf '%s【fn-knock 安装器】%s %s\n' "${C_TITLE}" "${C_RESET}" "$*"; }
success() { printf '%s✓ %s%s\n' "${C_OK}" "$*" "${C_RESET}"; }
fail() { printf '%s✗ %s%s\n' "${C_ERR}" "$*" "${C_RESET}" >&2; exit 1; }

show_banner() {
  cat <<EOF
${C_TITLE}
 ███████╗███╗   ██╗      ██╗  ██╗███╗   ██╗ ██████╗  ██████╗██╗  ██╗
 ██╔════╝████╗  ██║      ██║ ██╔╝████╗  ██║██╔═══██╗██╔════╝██║ ██╔╝
 █████╗  ██╔██╗ ██║█████╗█████╔╝ ██╔██╗ ██║██║   ██║██║     █████╔╝
 ██╔══╝  ██║╚██╗██║╚════╝██╔═██╗ ██║╚██╗██║██║   ██║██║     ██╔═██╗
 ██║     ██║ ╚████║      ██║  ██╗██║ ╚████║╚██████╔╝╚██████╗██║  ██╗
 ╚═╝     ╚═╝  ╚═══╝      ╚═╝  ╚═╝╚═╝  ╚═══╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝${C_RESET}
${C_DIM}                         Linux 一键安装器${C_RESET}
EOF
}

cleanup() {
  [ -z "${WORK_DIR}" ] || rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

show_banner
[ "$(id -u)" -eq 0 ] || fail "需要 root 权限，请使用：sudo sh"
[ "$(uname -s)" = "Linux" ] || fail "仅支持 Linux 系统"

normalize_arch() {
  case "$(uname -m)" in
    x86_64|amd64) printf '%s\n' amd64 ;;
    aarch64|arm64) printf '%s\n' arm64 ;;
    armv7l|armv8l|armhf|arm) printf '%s\n' arm ;;
    *) return 1 ;;
  esac
}

install_dependencies() {
  missing=0
  for command_name in bash curl openssl unzip tar gzip ss install; do
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
  elif command -v apk >/dev/null 2>&1; then
    apk add --no-cache bash ca-certificates curl openssl unzip tar gzip iproute2 coreutils procps
  else
    fail "缺少运行依赖，且未找到受支持的软件包管理器"
  fi
}

detect_service_manager() {
  if command -v systemctl >/dev/null 2>&1 && [ -d /run/systemd/system ]; then
    SERVICE_MANAGER=systemd
  elif command -v rc-service >/dev/null 2>&1 && command -v rc-update >/dev/null 2>&1; then
    [ -d /run/openrc ] || fail "检测到 OpenRC，但当前未运行 OpenRC"
    SERVICE_MANAGER=openrc
  else
    fail "仅支持正在运行的 systemd 或 OpenRC"
  fi
  export FN_KNOCK_SERVICE_MANAGER="${SERVICE_MANAGER}"
  log "检测到服务管理器：${SERVICE_MANAGER}"
}

installed_version() {
  release_file="${APP_ROOT}/current/release.json"
  if [ -f "${release_file}" ]; then
    sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${release_file}" | head -n1
  fi
}

choose_action() {
  answer='' version=''
  if [ -x "${COMMAND_FILE}" ] && [ -f "${APP_ROOT}/current/release.json" ]; then
    version="$(installed_version)"
    log "检测到已安装的 fn-knock${version:+（版本 ${version}）}"
    printf '\n%s━━━ 请选择操作 ━━━%s\n' "${C_ACCENT}" "${C_RESET}" >&2
    printf '  %s[1]%s 下载并安装最新版本\n' "${C_OK}" "${C_RESET}" >&2
    printf '  %s[2]%s 打开 fn-knock 管理菜单\n' "${C_ACCENT}" "${C_RESET}" >&2
    printf '  %s[3]%s 查看服务状态\n' "${C_ACCENT}" "${C_RESET}" >&2
    printf '  %s[4]%s 卸载 fn-knock（需输入 Y 确认）\n' "${C_ERR}" "${C_RESET}" >&2
    printf '  %s[0]%s 退出\n' "${C_DIM}" "${C_RESET}" >&2
    printf '请选择 [1]: ' >&2
    read -r answer </dev/tty || fail "需要可交互的终端"
    case "${answer:-1}" in
      1) return 0 ;;
      2) exec "${COMMAND_FILE}" ;;
      3) exec "${COMMAND_FILE}" status ;;
      4) exec "${COMMAND_FILE}" uninstall ;;
      0) exit 0 ;;
      *) fail "无效选择" ;;
    esac
  fi

  printf '\n%s未检测到已安装的 fn-knock%s\n' "${C_ACCENT}" "${C_RESET}" >&2
  printf '  %s[1]%s 安装 fn-knock\n' "${C_OK}" "${C_RESET}" >&2
  printf '  %s[0]%s 退出\n' "${C_DIM}" "${C_RESET}" >&2
  printf '请选择 [1]: ' >&2
  read -r answer </dev/tty || fail "需要可交互的终端"
  case "${answer:-1}" in
    1) ;;
    0) exit 0 ;;
    *) fail "无效选择" ;;
  esac
}

manifest_value() {
  file="$1" key="$2"
  awk -v wanted="${key}" '
    index($0, wanted "=") == 1 { count++; value = substr($0, length(wanted) + 2) }
    END { if (count != 1) exit 1; print value }
  ' "${file}"
}

configured_port() {
  key="$1" fallback="$2" config_file="${FN_KNOCK_ENV_FILE:-/etc/fn-knock/fn-knock.env}" value=""
  if [ -f "${config_file}" ]; then
    value="$(awk -v wanted="${key}" '
      index($0, wanted "=") == 1 { print substr($0, length(wanted) + 2); exit }
    ' "${config_file}")"
  fi
  printf '%s\n' "${value:-${fallback}}"
}

file_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  else
    openssl dgst -sha256 "$1" | awk '{print $NF}'
  fi
}

tar_list_archive() {
  archive_path="$1" list_path="$2"
  if tar --help 2>&1 | grep -- '--warning' >/dev/null; then
    tar --warning=no-unknown-keyword --warning=no-timestamp -tzf "${archive_path}" > "${list_path}"
  else
    tar -tzf "${archive_path}" > "${list_path}"
  fi
}

tar_extract_archive() {
  archive_path="$1" destination="$2"
  if tar --help 2>&1 | grep -- '--warning' >/dev/null; then
    tar --warning=no-unknown-keyword --warning=no-timestamp -xzf "${archive_path}" -C "${destination}"
  else
    tar -xzf "${archive_path}" -C "${destination}"
  fi
}

install_dependencies
detect_service_manager
choose_action
ARCH="$(normalize_arch)" || fail "不支持的系统架构：$(uname -m)"
WORK_DIR="$(mktemp -d /tmp/fn-knock-installer.XXXXXX)"
MANIFEST_FILE="${WORK_DIR}/latest.env"
ARCHIVE_FILE="${WORK_DIR}/release.tar.gz"

log "检测到系统架构：${ARCH}"
curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
  -o "${MANIFEST_FILE}" "${BASE_URL%/}/linux/latest/${ARCH}.env"

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
tar_list_archive "${ARCHIVE_FILE}" "${WORK_DIR}/archive.list"
grep -qx 'fn-knock/release.json' "${WORK_DIR}/archive.list" || fail "发布包目录结构无效"
tar_extract_archive "${ARCHIVE_FILE}" "${WORK_DIR}"
[ -x "${WORK_DIR}/fn-knock/bin/knock" ] || fail "发布包不包含管理命令"

log "正在安装前检测所需端口…"
"${WORK_DIR}/fn-knock/bin/knock" _prepare-install
"${WORK_DIR}/fn-knock/bin/knock" _install-extracted "${WORK_DIR}/fn-knock" "${VERSION}"

success "fn-knock ${VERSION} 安装完成！"
ADMIN_PORT="$(configured_port ADMIN_VIEW_PORT 7991)"
PROXY_PORT="$(configured_port GO_REPROXY_PORT 7999)"
printf '\n%s━━━ 下一步 ━━━%s\n' "${C_ACCENT}" "${C_RESET}"
printf '%s局域网配置：%s请在局域网浏览器打开 %shttp://<服务器局域网 IP>:%s/%s\n' \
  "${C_OK}" "${C_RESET}" "${C_TITLE}" "${ADMIN_PORT}" "${C_RESET}"
printf '%s公网管理：%s不要直接暴露或端口转发 %s%s%s；必须通过 HTTPS 反向代理访问，并设置访问控制。\n' \
  "${C_ERR}" "${C_RESET}" "${C_TITLE}" "${ADMIN_PORT}" "${C_RESET}"
printf '%s反向代理：%s运行 %ssudo knock nginx%s 查看 Nginx 配置模板。\n' \
  "${C_ACCENT}" "${C_RESET}" "${C_TITLE}" "${C_RESET}"
printf '%s管理菜单：%s随时运行 %ssudo knock%s 打开服务管理、端口配置、更新和日志。\n' \
  "${C_ACCENT}" "${C_RESET}" "${C_TITLE}" "${C_RESET}"
printf '%s网关入口：%sGo 代理默认端口为 %s%s%s；仅开放部署实际需要的端口。\n' \
  "${C_DIM}" "${C_RESET}" "${C_TITLE}" "${PROXY_PORT}" "${C_RESET}"
