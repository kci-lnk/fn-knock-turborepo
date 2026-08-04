#!/bin/sh
set -eu

DEFAULT_BASE_URL="https://cdn.fnknock.cn"
BASE_URL="${FN_KNOCK_BASE_URL:-${DEFAULT_BASE_URL}}"
COMMAND_FILE="/usr/local/bin/knock"
APP_ROOT="/Library/Application Support/FnKnock"
if [ "${FN_KNOCK_TEST_MODE:-0}" = "1" ]; then
  COMMAND_FILE="${FN_KNOCK_COMMAND_FILE:-${COMMAND_FILE}}"
  APP_ROOT="${FN_KNOCK_APP_ROOT:-${APP_ROOT}}"
  for test_path in "${COMMAND_FILE}" "${APP_ROOT}"; do
    case "${test_path}" in
      /tmp/*|/private/tmp/*|/var/folders/*) ;;
      *) printf '✗ 测试路径必须位于临时目录：%s\n' "${test_path}" >&2; exit 1 ;;
    esac
  done
else
  PATH=/usr/bin:/bin:/usr/sbin:/sbin
  export PATH
  readonly PATH
fi
WORK_DIR=""
MAX_ARCHIVE_SIZE=1073741824

log() { printf '【fn-knock macOS 安装器】 %s\n' "$*"; }
success() { printf '✓ %s\n' "$*"; }
fail() { printf '✗ %s\n' "$*" >&2; exit 1; }

cleanup() {
  [ -z "${WORK_DIR}" ] || rm -rf "${WORK_DIR}"
}
trap cleanup EXIT INT TERM

normalize_arch() {
  machine="$(uname -m)"
  if [ "${machine}" = "x86_64" ] && [ "$(sysctl -in sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
    machine="arm64"
  fi
  case "${machine}" in
    x86_64|amd64) printf '%s\n' amd64 ;;
    arm64|aarch64) printf '%s\n' arm64 ;;
    *) return 1 ;;
  esac
}

manifest_value() {
  awk -v wanted="$2" '
    index($0, wanted "=") == 1 { count++; value = substr($0, length(wanted) + 2) }
    END { if (count != 1) exit 1; print value }
  ' "$1"
}

file_sha256() {
  shasum -a 256 "$1" | awk '{print $1}'
}

url_is_allowed() {
  case "$1" in
    https://*) return 0 ;;
    http://*) [ "${FN_KNOCK_ALLOW_INSECURE_HTTP:-0}" = "1" ] ;;
    *) return 1 ;;
  esac
}

download_file() {
  url="$1"
  output="$2"
  max_size="$3"
  url_is_allowed "${url}" || fail "下载地址必须使用 HTTPS"
  protocols='=https'
  [ "${FN_KNOCK_ALLOW_INSECURE_HTTP:-0}" != "1" ] || protocols='=http,https'
  curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
    --max-time 600 --max-filesize "${max_size}" \
    --proto "${protocols}" --proto-redir "${protocols}" \
    -o "${output}" "${url}"
}

validate_archive() {
  archive="$1"
  list_file="$2"
  verbose_file="$3"
  tar -tzf "${archive}" > "${list_file}"
  grep -qx 'fn-knock/release.json' "${list_file}" || fail "发布包目录结构无效"
  if awk '
    $0 != "fn-knock" && index($0, "fn-knock/") != 1 { bad=1 }
    $0 ~ /(^|\/)\.\.?($|\/)/ || $0 ~ /[[:cntrl:]\\]/ { bad=1 }
    END { exit bad ? 0 : 1 }
  ' "${list_file}"; then
    fail "发布包包含不安全路径"
  fi
  if awk 'seen[$0]++ { duplicate=1 } END { exit duplicate ? 0 : 1 }' "${list_file}"; then
    fail "发布包包含重复路径"
  fi
  tar -tvzf "${archive}" > "${verbose_file}"
  if awk 'substr($0, 1, 1) != "-" && substr($0, 1, 1) != "d" { bad=1 } END { exit bad ? 0 : 1 }' \
    "${verbose_file}"; then
    fail "发布包包含符号链接、硬链接或特殊文件"
  fi
}

installed_version() {
  release_file="${APP_ROOT}/current/release.json"
  if [ -f "${release_file}" ]; then
    sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${release_file}" | head -n1
  fi
}

release_value() {
  /usr/bin/plutil -extract "$2" raw -o - "$1" 2>/dev/null
}

validate_macho() {
  binary="$1"
  expected_arch="$2"
  [ -f "${binary}" ] && [ ! -L "${binary}" ] || fail "发布包缺少二进制文件：${binary}"
  description="$(/usr/bin/file -b "${binary}")" || fail "无法识别二进制文件：${binary}"
  case "${expected_arch}:${description}" in
    amd64:Mach-O\ 64-bit\ executable\ x86_64*) ;;
    arm64:Mach-O\ 64-bit\ executable\ arm64*) ;;
    *) fail "发布包二进制架构不匹配：${description}" ;;
  esac
}

choose_action() {
  [ "${FN_KNOCK_ASSUME_YES:-0}" != "1" ] || return 0
  if [ -x "${COMMAND_FILE}" ] && [ -f "${APP_ROOT}/current/release.json" ]; then
    log "检测到已安装的 fn-knock $(installed_version)"
    printf '1. 下载并安装最新版本\n2. 打开管理菜单\n3. 查看状态\n4. 卸载\n0. 退出\n请选择 [1]: ' >&2
    read -r answer </dev/tty || fail "需要可交互的终端"
    case "${answer:-1}" in
      1) ;;
      2) exec "${COMMAND_FILE}" ;;
      3) exec "${COMMAND_FILE}" status ;;
      4) exec "${COMMAND_FILE}" uninstall ;;
      0) exit 0 ;;
      *) fail "无效选择" ;;
    esac
    return
  fi
  printf '未检测到 fn-knock。安装最新版本？[Y/n] ' >&2
  read -r answer </dev/tty || fail "需要可交互的终端"
  case "${answer:-Y}" in n|N) exit 0 ;; esac
}

[ "$(id -u)" -eq 0 ] || fail "需要 root 权限，请使用：curl -fsSL ${BASE_URL%/}/macos/install.sh | sudo bash"
[ "$(uname -s)" = "Darwin" ] || fail "仅支持 macOS"
url_is_allowed "${BASE_URL%/}/macos/install.sh" || fail "发布基础地址必须使用 HTTPS"
major_version="$(sw_vers -productVersion | awk -F. '{print $1}')"
case "${major_version}" in ''|*[!0-9]*) fail "无法识别 macOS 版本" ;; esac
[ "${major_version}" -ge 13 ] || fail "最低支持 macOS 13"
for command_name in curl shasum tar awk sed sw_vers sysctl grep wc tr ditto file plutil; do
  command -v "${command_name}" >/dev/null 2>&1 || fail "系统缺少命令：${command_name}"
done

choose_action
ARCH="$(normalize_arch)" || fail "不支持的系统架构：$(uname -m)"
WORK_DIR="$(mktemp -d /tmp/fn-knock-macos-installer.XXXXXX)"
MANIFEST_FILE="${WORK_DIR}/latest.env"
ARCHIVE_FILE="${WORK_DIR}/release.tar.gz"

log "检测到系统架构：${ARCH}"
download_file "${BASE_URL%/}/macos/latest/${ARCH}.env" "${MANIFEST_FILE}" 65536

VERSION="$(manifest_value "${MANIFEST_FILE}" VERSION)" || fail "发布清单中的 VERSION 无效"
URL="$(manifest_value "${MANIFEST_FILE}" URL)" || fail "发布清单中的 URL 无效"
SHA256="$(manifest_value "${MANIFEST_FILE}" SHA256)" || fail "发布清单中的 SHA256 无效"
SIZE="$(manifest_value "${MANIFEST_FILE}" SIZE)" || fail "发布清单中的 SIZE 无效"
printf '%s' "${VERSION}" | grep -Eq '^[0-9][0-9A-Za-z._+-]*$' || fail "发布版本号无效"
printf '%s' "${SHA256}" | grep -Eq '^[0-9a-fA-F]{64}$' || fail "发布校验和无效"
printf '%s' "${SIZE}" | grep -Eq '^[1-9][0-9]*$' || fail "发布包大小无效"
awk -v size="${SIZE}" -v maximum="${MAX_ARCHIVE_SIZE}" \
  'BEGIN { exit !(size <= maximum) }' || fail "发布包超过 1 GiB 安全上限"
case "${URL}" in
  https://*) ;;
  http://*) [ "${FN_KNOCK_ALLOW_INSECURE_HTTP:-0}" = "1" ] || fail "发布地址必须使用 HTTPS" ;;
  *) fail "发布地址必须为绝对 URL" ;;
esac

log "正在下载 fn-knock ${VERSION}…"
download_file "${URL}" "${ARCHIVE_FILE}" "${SIZE}"
ACTUAL_SIZE="$(wc -c < "${ARCHIVE_FILE}" | tr -d '[:space:]')"
[ "${ACTUAL_SIZE}" = "${SIZE}" ] || fail "下载文件大小不匹配"
[ "$(file_sha256 "${ARCHIVE_FILE}")" = "${SHA256}" ] || fail "下载文件 SHA-256 不匹配"

validate_archive "${ARCHIVE_FILE}" "${WORK_DIR}/archive.list" "${WORK_DIR}/archive.verbose"
tar -xzf "${ARCHIVE_FILE}" -C "${WORK_DIR}"
[ -x "${WORK_DIR}/fn-knock/bin/knock" ] || fail "发布包不包含 knock 管理命令"
RELEASE_JSON="${WORK_DIR}/fn-knock/release.json"
/usr/bin/plutil -convert json -o /dev/null "${RELEASE_JSON}" >/dev/null || fail "发布包 release.json 无效"
[ "$(release_value "${RELEASE_JSON}" version)" = "${VERSION}" ] || fail "发布包内嵌版本不匹配"
[ "$(release_value "${RELEASE_JSON}" runtime_target)" = macos ] || fail "发布包不是 macOS 运行时"
[ "$(release_value "${RELEASE_JSON}" architecture)" = "${ARCH}" ] || fail "发布包架构与当前 Mac 不匹配"
validate_macho "${WORK_DIR}/fn-knock/bin/go-reauth-proxy" "${ARCH}"
validate_macho "${WORK_DIR}/fn-knock/bin/server-admin-rs" "${ARCH}"

log "正在安装前检查端口…"
FN_KNOCK_ASSUME_YES="${FN_KNOCK_ASSUME_YES:-0}" "${WORK_DIR}/fn-knock/bin/knock" _prepare-install
"${WORK_DIR}/fn-knock/bin/knock" _install-extracted "${WORK_DIR}/fn-knock" "${VERSION}"

success "fn-knock ${VERSION} 安装完成"
printf '管理面板：http://127.0.0.1:%s/\n' "$(awk -F= '$1 == "ADMIN_VIEW_PORT" { print $2; exit }' "${APP_ROOT}/config/fn-knock.env")"
printf '管理命令：sudo knock\n'
printf '本发行版未使用 Apple Developer ID 签名；完整校验值记录在 GitHub Release 的 SHA256SUMS。\n'
