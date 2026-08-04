#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-macos-management.XXXXXX")"
MOCK_BIN="${WORK_DIR}/bin"
STATE_FILE="${WORK_DIR}/launchd.loaded"

cleanup() { rm -rf "${WORK_DIR}"; }
trap cleanup EXIT
fail() { printf '[test-macos-management] ERROR: %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = Darwin ] || fail "this test must run on macOS"
mkdir -p "${MOCK_BIN}"

cat > "${MOCK_BIN}/id" <<'EOF'
#!/bin/sh
if [ "${1:-}" = "-u" ]; then printf '0\n'; else exec /usr/bin/id "$@"; fi
EOF
cat > "${MOCK_BIN}/uname" <<'EOF'
#!/bin/sh
case "${1:-}" in
  -s) printf 'Darwin\n' ;;
  -m) /usr/bin/uname -m ;;
  *) /usr/bin/uname "$@" ;;
esac
EOF
cat > "${MOCK_BIN}/sw_vers" <<'EOF'
#!/bin/sh
[ "${1:-}" = "-productVersion" ] && { printf '13.6.1\n'; exit 0; }
exit 1
EOF
cat > "${MOCK_BIN}/launchctl" <<'EOF'
#!/bin/sh
[ "${FN_KNOCK_TEST_LAUNCHCTL_FAIL:-}" != "${1:-}" ] || exit 64
case "$1" in
  print) [ -f "${FN_KNOCK_TEST_LAUNCHD_STATE}" ] ;;
  bootstrap)
    if [ "${FN_KNOCK_TEST_BOOTSTRAP_EIO_ONCE:-0}" = "1" ] && \
      [ -f "${FN_KNOCK_TEST_LAUNCHD_STATE}.teardown" ]; then
      rm -f "${FN_KNOCK_TEST_LAUNCHD_STATE}.teardown"
      printf 'Bootstrap failed: 5: Input/output error\n' >&2
      exit 5
    fi
    : > "${FN_KNOCK_TEST_LAUNCHD_STATE}"
    ;;
  kickstart) : > "${FN_KNOCK_TEST_LAUNCHD_STATE}" ;;
  bootout)
    rm -f "${FN_KNOCK_TEST_LAUNCHD_STATE}"
    [ "${FN_KNOCK_TEST_BOOTSTRAP_EIO_ONCE:-0}" != "1" ] || \
      : > "${FN_KNOCK_TEST_LAUNCHD_STATE}.teardown"
    ;;
  *) exit 1 ;;
esac
EOF
cat > "${MOCK_BIN}/lsof" <<'EOF'
#!/bin/sh
[ "${FN_KNOCK_TEST_PORT_CONFLICT:-0}" != "1" ] || {
  printf 'COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME\n'
  printf 'conflict 42 root 3u IPv4 0t0 TCP 127.0.0.1:7991 (LISTEN)\n'
}
exit 0
EOF
cat > "${MOCK_BIN}/pgrep" <<'EOF'
#!/bin/sh
case "${*}" in
  *server-admin-rs) printf '4242\n4243\n' ;;
  *go-reauth-proxy) printf '4342\n' ;;
  *) exit 1 ;;
esac
EOF
cat > "${MOCK_BIN}/ps" <<'EOF'
#!/bin/sh
pid="${2:-}"
case "${*}" in
  *comm=*)
    case "${pid}" in
      4242) printf '%s/current/bin/server-admin-rs\n' "${FN_KNOCK_APP_ROOT}" ;;
      4243) printf '/private/tmp/fn-knock-macos-smoke.old/fn-knock/bin/server-admin-rs\n' ;;
      4342) printf '%s/current/bin/go-reauth-proxy\n' "${FN_KNOCK_APP_ROOT}" ;;
      *) exit 1 ;;
    esac
    ;;
  *rss=*etime=*) printf '2048 00:42\n' ;;
  *) exit 1 ;;
esac
EOF
cat > "${MOCK_BIN}/ditto" <<'EOF'
#!/bin/sh
mkdir -p "$2"
cp -R "$1/." "$2/"
EOF
cat > "${MOCK_BIN}/sysctl" <<'EOF'
#!/bin/sh
printf '%s\n' "${FN_KNOCK_TEST_ROSETTA:-0}"
EOF
cat > "${MOCK_BIN}/curl" <<'EOF'
#!/bin/sh
output=""
url=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o|--output) output="$2"; shift 2 ;;
    http://*|https://*) url="$1"; shift ;;
    *) shift ;;
  esac
done
[ -n "${output}" ] || {
  [ "${FN_KNOCK_TEST_HEALTH_FAIL:-0}" != "1" ] || exit 22
  exit 0
}
case "${url}" in
  */latest/*.env) cp "${FN_KNOCK_TEST_UPDATE_MANIFEST}" "${output}" ;;
  *) cp "${FN_KNOCK_TEST_UPDATE_ARCHIVE}" "${output}" ;;
esac
EOF
chmod 0755 "${MOCK_BIN}/"*

export PATH="${MOCK_BIN}:/usr/bin:/bin:/usr/sbin:/sbin"
export FN_KNOCK_TEST_MODE=1
export FN_KNOCK_TEST_LAUNCHD_STATE="${STATE_FILE}"
export FN_KNOCK_APP_ROOT="${WORK_DIR}/Application Support/FnKnock"
export FN_KNOCK_CONFIG_DIR="${FN_KNOCK_APP_ROOT}/config"
export FN_KNOCK_DATA_DIR="${FN_KNOCK_APP_ROOT}/data"
export FN_KNOCK_LOG_DIR="${WORK_DIR}/Logs/FnKnock"
export FN_KNOCK_UNIT_FILE="${WORK_DIR}/LaunchDaemons/cn.fnknock.service.plist"
export FN_KNOCK_COMMAND_FILE="${WORK_DIR}/commands/knock"
export FN_KNOCK_LSOF_BIN="${MOCK_BIN}/lsof"
export FN_KNOCK_ASSUME_YES=1
export FN_KNOCK_HEALTH_ATTEMPTS=1
export FN_KNOCK_LAUNCHCTL_ATTEMPTS=3
export FN_KNOCK_LAUNCHCTL_DELAY=0

FN_KNOCK_TEST_ROSETTA=0 "${ROOT_DIR}/deploy/macos/knock" _normalize-arch x86_64 | grep -qx amd64
FN_KNOCK_TEST_ROSETTA=1 "${ROOT_DIR}/deploy/macos/knock" _normalize-arch x86_64 | grep -qx arm64

native_arch="$(${ROOT_DIR}/deploy/macos/knock _normalize-arch)"
archive="$(find "${ROOT_DIR}/dist/fn-knock-macos" -maxdepth 1 -name "fn-knock-macos-*-${native_arch}.tar.gz" -print -quit)"
[ -n "${archive}" ] || fail "native package was not found under dist/fn-knock-macos"
mkdir -p "${WORK_DIR}/release-one"
tar -xzf "${archive}" -C "${WORK_DIR}/release-one"
release_one="${WORK_DIR}/release-one/fn-knock"
version_one="$(sed -nE 's/^[[:space:]]*"version"[[:space:]]*:[[:space:]]*"([^"]+)".*/\1/p' "${release_one}/release.json" | head -n1)"

if FN_KNOCK_TEST_PORT_CONFLICT=1 "${release_one}/bin/knock" _prepare-install >/dev/null 2>&1; then
  fail "non-interactive installation accepted a conflicting default port"
fi

wrong_release="${WORK_DIR}/wrong-architecture"
mkdir -p "${wrong_release}"
cp -R "${release_one}/." "${wrong_release}/"
wrong_arch=amd64
[ "${native_arch}" = amd64 ] && wrong_arch=arm64
sed -E "s/\"architecture\"[[:space:]]*:[[:space:]]*\"[^\"]+\"/\"architecture\": \"${wrong_arch}\"/" \
  "${release_one}/release.json" > "${wrong_release}/release.json"
if "${wrong_release}/bin/knock" _install-extracted "${wrong_release}" "${version_one}" >/dev/null 2>&1; then
  fail "wrong-architecture package was accepted"
fi
[ ! -e "${FN_KNOCK_APP_ROOT}/current" ] || fail "wrong-architecture package mutated the installation"

tampered_release="${WORK_DIR}/tampered-binary"
mkdir -p "${tampered_release}"
cp -R "${release_one}/." "${tampered_release}/"
printf '#!/bin/sh\nexit 0\n' > "${tampered_release}/bin/go-reauth-proxy"
chmod 0755 "${tampered_release}/bin/go-reauth-proxy"
if "${tampered_release}/bin/knock" _install-extracted "${tampered_release}" "${version_one}" >/dev/null 2>&1; then
  fail "non-Mach-O backend was accepted"
fi
[ ! -e "${FN_KNOCK_APP_ROOT}/current" ] || fail "tampered package mutated the installation"

installer_sha="$(shasum -a 256 "${archive}" | awk '{print $1}')"
installer_size="$(wc -c < "${archive}" | tr -d '[:space:]')"
installer_manifest="${WORK_DIR}/installer-latest.env"
cat > "${installer_manifest}" <<EOF
VERSION=${version_one}
URL=https://cdn.example.test/fn-knock-macos-${version_one}-${native_arch}.tar.gz
SHA256=${installer_sha}
SIZE=${installer_size}
EOF
export FN_KNOCK_TEST_UPDATE_MANIFEST="${installer_manifest}"
export FN_KNOCK_TEST_UPDATE_ARCHIVE="${archive}"
export FN_KNOCK_BASE_URL=https://cdn.example.test
sh "${ROOT_DIR}/deploy/macos/install.sh"
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || fail "installer activated the wrong version"
"${FN_KNOCK_COMMAND_FILE}" uninstall --yes
[ ! -e "${FN_KNOCK_APP_ROOT}/current" ] || fail "installer test uninstall left current link"
[ ! -e "${FN_KNOCK_COMMAND_FILE}" ] || fail "installer test uninstall left management command"

"${release_one}/bin/knock" _prepare-install
"${release_one}/bin/knock" _install-extracted "${release_one}" "${version_one}"
[ -L "${FN_KNOCK_APP_ROOT}/current" ] || fail "current link was not installed"
[ -x "${FN_KNOCK_COMMAND_FILE}" ] || fail "management command was not installed"
[ -f "${FN_KNOCK_UNIT_FILE}" ] || fail "LaunchDaemon was not installed"
grep -Fq "${FN_KNOCK_APP_ROOT}/current/bin/fn-knock-entrypoint" "${FN_KNOCK_UNIT_FILE}" || \
  fail "LaunchDaemon application path was not rendered"
grep -Fq "${FN_KNOCK_LOG_DIR}/stdout.log" "${FN_KNOCK_UNIT_FILE}" || \
  fail "LaunchDaemon log path was not rendered"
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || fail "installed version is incorrect"
status_output="$("${FN_KNOCK_COMMAND_FILE}" status)"
printf '%s\n' "${status_output}" | grep -Eq 'server-admin-rs.*PID:[[:space:]]+4242' || \
  fail "status did not report the installed Rust backend"
printf '%s\n' "${status_output}" | grep -Eq 'go-reauth-proxy.*PID:[[:space:]]+4342' || \
  fail "status did not report the installed Go gateway"
if printf '%s\n' "${status_output}" | grep -Eq 'PID:[[:space:]]+4243'; then
  fail "status counted a same-name process from another installation"
fi
printf '%s\n' "${status_output}" | grep -Fq '合计：2 个进程' || \
  fail "status process total does not include exactly the installed pair"

"${FN_KNOCK_COMMAND_FILE}" stop
if FN_KNOCK_TEST_LAUNCHCTL_FAIL=bootstrap "${FN_KNOCK_COMMAND_FILE}" start >/dev/null 2>&1; then
  fail "failed launchctl bootstrap was reported as success"
fi
[ ! -f "${STATE_FILE}" ] || fail "failed bootstrap unexpectedly marked launchd active"
"${FN_KNOCK_COMMAND_FILE}" start
FN_KNOCK_TEST_BOOTSTRAP_EIO_ONCE=1 "${FN_KNOCK_COMMAND_FILE}" restart
[ -f "${STATE_FILE}" ] || fail "restart did not recover from launchd bootstrap EIO"
[ ! -f "${STATE_FILE}.teardown" ] || fail "restart left launchd teardown state behind"
FN_KNOCK_TEST_BOOTSTRAP_EIO_ONCE=1 \
  "${release_one}/bin/knock" _install-extracted "${release_one}" "${version_one}"
[ -f "${STATE_FILE}" ] || fail "reinstall did not recover from launchd bootstrap EIO"
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || \
  fail "reinstall selected the wrong version after launchd bootstrap EIO"

version_two="9.9.9"
release_two_parent="${WORK_DIR}/release-two"
mkdir -p "${release_two_parent}/fn-knock"
cp -R "${release_one}/." "${release_two_parent}/fn-knock/"
sed -E "s/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]+\"/\"version\": \"${version_two}\"/" \
  "${release_one}/release.json" > "${release_two_parent}/fn-knock/release.json"
update_archive="${WORK_DIR}/update.tar.gz"
tar -czf "${update_archive}" -C "${release_two_parent}" fn-knock
update_sha="$(shasum -a 256 "${update_archive}" | awk '{print $1}')"
update_size="$(wc -c < "${update_archive}" | tr -d '[:space:]')"
update_manifest="${WORK_DIR}/latest.env"
cat > "${update_manifest}" <<EOF
VERSION=${version_two}
URL=https://cdn.example.test/fn-knock-macos-${version_two}-${native_arch}.tar.gz
SHA256=${update_sha}
SIZE=${update_size}
EOF
export FN_KNOCK_TEST_UPDATE_MANIFEST="${update_manifest}"
export FN_KNOCK_TEST_UPDATE_ARCHIVE="${update_archive}"
export FN_KNOCK_BASE_URL=https://cdn.example.test

"${FN_KNOCK_COMMAND_FILE}" update --yes
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_two}" ] || fail "update did not activate the new version"
"${FN_KNOCK_COMMAND_FILE}" rollback
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || fail "rollback did not restore the prior version"

unsafe_version="9.9.8"
unsafe_parent="${WORK_DIR}/release-unsafe"
mkdir -p "${unsafe_parent}/fn-knock"
cp -R "${release_one}/." "${unsafe_parent}/fn-knock/"
sed -E "s/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]+\"/\"version\": \"${unsafe_version}\"/" \
  "${release_one}/release.json" > "${unsafe_parent}/fn-knock/release.json"
ln -s release.json "${unsafe_parent}/fn-knock/unsafe-link"
unsafe_archive="${WORK_DIR}/unsafe-update.tar.gz"
tar -czf "${unsafe_archive}" -C "${unsafe_parent}" fn-knock
unsafe_sha="$(shasum -a 256 "${unsafe_archive}" | awk '{print $1}')"
unsafe_size="$(wc -c < "${unsafe_archive}" | tr -d '[:space:]')"
unsafe_manifest="${WORK_DIR}/unsafe-latest.env"
cat > "${unsafe_manifest}" <<EOF
VERSION=${unsafe_version}
URL=https://cdn.example.test/fn-knock-macos-${unsafe_version}-${native_arch}.tar.gz
SHA256=${unsafe_sha}
SIZE=${unsafe_size}
EOF
export FN_KNOCK_TEST_UPDATE_MANIFEST="${unsafe_manifest}"
export FN_KNOCK_TEST_UPDATE_ARCHIVE="${unsafe_archive}"
if "${FN_KNOCK_COMMAND_FILE}" update --yes >/dev/null 2>&1; then
  fail "archive containing a symbolic link was accepted"
fi
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || \
  fail "unsafe archive mutated the current version"

version_failed="10.0.0"
failed_parent="${WORK_DIR}/release-failed"
mkdir -p "${failed_parent}/fn-knock"
cp -R "${release_one}/." "${failed_parent}/fn-knock/"
sed -E "s/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]+\"/\"version\": \"${version_failed}\"/" \
  "${release_one}/release.json" > "${failed_parent}/fn-knock/release.json"
failed_archive="${WORK_DIR}/failed-update.tar.gz"
tar -czf "${failed_archive}" -C "${failed_parent}" fn-knock
failed_sha="$(shasum -a 256 "${failed_archive}" | awk '{print $1}')"
failed_size="$(wc -c < "${failed_archive}" | tr -d '[:space:]')"
failed_manifest="${WORK_DIR}/failed-latest.env"
cat > "${failed_manifest}" <<EOF
VERSION=${version_failed}
URL=https://cdn.example.test/fn-knock-macos-${version_failed}-${native_arch}.tar.gz
SHA256=${failed_sha}
SIZE=${failed_size}
EOF
export FN_KNOCK_TEST_UPDATE_MANIFEST="${failed_manifest}"
export FN_KNOCK_TEST_UPDATE_ARCHIVE="${failed_archive}"
if FN_KNOCK_TEST_HEALTH_FAIL=1 "${FN_KNOCK_COMMAND_FILE}" update --yes >/dev/null 2>&1; then
  fail "update unexpectedly succeeded after its readiness check failed"
fi
[ "$(${FN_KNOCK_COMMAND_FILE} version)" = "${version_one}" ] || \
  fail "failed update did not restore the original current version"
[ -f "${STATE_FILE}" ] || fail "failed update did not restore the running launchd state"

mkdir -p "${FN_KNOCK_DATA_DIR}"
printf 'preserve\n' > "${FN_KNOCK_DATA_DIR}/preserve.txt"
if FN_KNOCK_TEST_LAUNCHCTL_FAIL=bootout "${FN_KNOCK_COMMAND_FILE}" uninstall --yes >/dev/null 2>&1; then
  fail "uninstall succeeded after launchctl bootout failed"
fi
[ -e "${FN_KNOCK_UNIT_FILE}" ] || fail "failed uninstall removed the LaunchDaemon"
[ -x "${FN_KNOCK_COMMAND_FILE}" ] || fail "failed uninstall removed the management command"
"${FN_KNOCK_COMMAND_FILE}" uninstall --yes
[ -f "${FN_KNOCK_DATA_DIR}/preserve.txt" ] || fail "uninstall removed preserved data"
[ ! -e "${FN_KNOCK_UNIT_FILE}" ] || fail "uninstall left the LaunchDaemon"
[ ! -e "${FN_KNOCK_COMMAND_FILE}" ] || fail "uninstall left the management command"

grep -Fq 'FN_KNOCK_DISABLE_IPTABLES=1' "${ROOT_DIR}/deploy/macos/fn-knock-entrypoint" || \
  fail "macOS gateway does not explicitly disable iptables"
if grep -Eq '(^|[[:space:]])(iptables|ip6tables)([[:space:]]|$)' \
  "${ROOT_DIR}/deploy/macos/knock" "${ROOT_DIR}/deploy/macos/fn-knock-entrypoint"; then
  fail "macOS management scripts invoke iptables"
fi

printf '[test-macos-management] all macOS management tests passed\n'
