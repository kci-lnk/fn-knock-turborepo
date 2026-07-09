#!/usr/bin/env bash
set -euo pipefail

DEFAULT_BASE_URL="https://cdn.fnknock.cn"
BASE_URL="${FN_KNOCK_BASE_URL:-${DEFAULT_BASE_URL}}"
WORK_DIR=""

log() { printf '[fn-knock-installer] %s\n' "$*"; }
fail() { printf '[fn-knock-installer] ERROR: %s\n' "$*" >&2; exit 1; }

cleanup() {
  [ -z "${WORK_DIR}" ] || rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

[ -n "${BASH_VERSION:-}" ] || fail "this installer requires bash"
[ "$(id -u)" -eq 0 ] || fail "root privilege is required; pipe this script to: sudo bash"
[ "$(uname -s)" = "Linux" ] || fail "only Linux is supported"
command -v systemctl >/dev/null 2>&1 || fail "systemd is required"
[ -d /run/systemd/system ] || fail "systemd is not the active init system"

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
  for command_name in curl openssl unzip tar gzip; do
    command -v "${command_name}" >/dev/null 2>&1 || missing=1
  done
  [ "${missing}" = "1" ] || return 0

  log "installing required system packages"
  if command -v apt-get >/dev/null 2>&1; then
    DEBIAN_FRONTEND=noninteractive apt-get update -y
    DEBIAN_FRONTEND=noninteractive apt-get install -y ca-certificates curl openssl unzip tar gzip
  elif command -v dnf >/dev/null 2>&1; then
    dnf install -y ca-certificates curl openssl unzip tar gzip
  elif command -v yum >/dev/null 2>&1; then
    yum install -y ca-certificates curl openssl unzip tar gzip
  else
    fail "missing runtime dependencies and no supported package manager was found"
  fi
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

install_dependencies
ARCH="$(normalize_arch)" || fail "unsupported architecture: $(uname -m)"
WORK_DIR="$(mktemp -d /tmp/fn-knock-installer.XXXXXX)"
MANIFEST_FILE="${WORK_DIR}/latest.env"
ARCHIVE_FILE="${WORK_DIR}/release.tar.gz"

log "detected architecture: ${ARCH}"
curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
  -o "${MANIFEST_FILE}" "${BASE_URL%/}/linux/latest/${ARCH}.env"

VERSION="$(manifest_value "${MANIFEST_FILE}" VERSION)" || fail "invalid VERSION in release manifest"
URL="$(manifest_value "${MANIFEST_FILE}" URL)" || fail "invalid URL in release manifest"
SHA256="$(manifest_value "${MANIFEST_FILE}" SHA256)" || fail "invalid SHA256 in release manifest"
SIZE="$(manifest_value "${MANIFEST_FILE}" SIZE)" || fail "invalid SIZE in release manifest"

printf '%s' "${VERSION}" | grep -Eq '^[0-9][0-9A-Za-z._+-]*$' || fail "invalid release version"
printf '%s' "${SHA256}" | grep -Eq '^[0-9a-fA-F]{64}$' || fail "invalid release checksum"
printf '%s' "${SIZE}" | grep -Eq '^[1-9][0-9]*$' || fail "invalid release size"
case "${URL}" in
  https://*) ;;
  http://*) [ "${FN_KNOCK_ALLOW_INSECURE_HTTP:-0}" = "1" ] || fail "release URL must use HTTPS" ;;
  *) fail "release URL must be absolute" ;;
esac

log "downloading fn-knock ${VERSION}"
curl --fail --silent --show-error --location --retry 3 --connect-timeout 15 \
  -o "${ARCHIVE_FILE}" "${URL}"

ACTUAL_SIZE="$(wc -c < "${ARCHIVE_FILE}" | tr -d '[:space:]')"
[ "${ACTUAL_SIZE}" = "${SIZE}" ] || fail "download size mismatch"
ACTUAL_SHA256="$(file_sha256 "${ARCHIVE_FILE}")"
[ "${ACTUAL_SHA256}" = "${SHA256}" ] || fail "download SHA256 mismatch"
tar --warning=no-unknown-keyword --warning=no-timestamp -tzf "${ARCHIVE_FILE}" > "${WORK_DIR}/archive.list"
grep -qx 'fn-knock/release.json' "${WORK_DIR}/archive.list" || fail "invalid release archive layout"
tar --warning=no-unknown-keyword --warning=no-timestamp -xzf "${ARCHIVE_FILE}" -C "${WORK_DIR}"
[ -x "${WORK_DIR}/fn-knock/bin/knock" ] || fail "release does not contain the management command"

"${WORK_DIR}/fn-knock/bin/knock" _install-extracted "${WORK_DIR}/fn-knock" "${VERSION}"

log "installation completed"
log "control plane: http://<device-ip>:7991 (listens on all interfaces by default)"
log "for public Internet use, HTTPS Nginx reverse proxy and source-IP restrictions are strongly recommended; run 'sudo knock nginx' for a template"
log "port 7998 is an internal Rust API listener bound only to loopback; do not expose or forward it"
log "proxy port: 7999"
log "run 'sudo knock' at any time to open the management menu"
log "fn-knock does not modify the host firewall; only expose the ports required by your deployment"
