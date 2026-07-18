#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-fpk-test.XXXXXX")"
SOURCE_DIR="${WORK_DIR}/source"
ARTIFACTS_DIR="${WORK_DIR}/artifacts"
RUNTIME_DIR="${ARTIFACTS_DIR}/runtime"
RUST_DIR="${ARTIFACTS_DIR}/fpk-rust-backends"
FNPACK_BIN="${WORK_DIR}/fnpack"
VERSION="$(jq -r '.version' "${ROOT_DIR}/version.json")"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-fpk-staging] ERROR: %s\n' "$*" >&2
  exit 1
}

make_elf() {
  local path="$1"
  local machine="$2"
  mkdir -p "$(dirname "${path}")"
  {
    printf '\177ELF\002\001\001\000'
    dd if=/dev/zero bs=1 count=8 2>/dev/null
    printf '\002\000'
    case "${machine}" in
      amd64) printf '\076\000' ;;
      arm64) printf '\267\000' ;;
      *) fail "unknown fake ELF architecture: ${machine}" ;;
    esac
    dd if=/dev/zero bs=1 count=44 2>/dev/null
  } > "${path}"
  chmod 755 "${path}"
}

expect_failure() {
  local expected="$1"
  shift
  local output
  if output="$("$@" 2>&1)"; then
    fail "command unexpectedly succeeded: $*"
  fi
  printf '%s\n' "${output}" | grep -Fq "${expected}" || \
    fail "failure did not contain '${expected}': ${output}"
}

mkdir -p \
  "${SOURCE_DIR}/cmd" \
  "${SOURCE_DIR}/app/ui" \
  "${RUNTIME_DIR}/ui/www" \
  "${RUNTIME_DIR}/server-auth-view/dist" \
  "${RUNTIME_DIR}/server/server-admin/resources" \
  "${RUNTIME_DIR}/server" \
  "${RUST_DIR}"
printf 'appname=fn-knock\nversion=%s\nplatform=x86\n' "${VERSION}" > "${SOURCE_DIR}/manifest"
printf '#!/bin/sh\nexit 0\n' > "${SOURCE_DIR}/cmd/main"
printf '#!/bin/sh\nexit 0\n' > "${SOURCE_DIR}/app/ui/index.cgi"
printf '<html>admin</html>\n' > "${RUNTIME_DIR}/ui/www/index.html"
printf '<html>auth</html>\n' > "${RUNTIME_DIR}/server-auth-view/dist/index.html"
printf 'fixture\n' > "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip"
chmod 755 "${SOURCE_DIR}/cmd/main" "${SOURCE_DIR}/app/ui/index.cgi"

make_elf "${RUNTIME_DIR}/server/go-reauth-proxy-linux-amd64" amd64
make_elf "${RUNTIME_DIR}/server/go-reauth-proxy-linux-arm64" arm64
make_elf "${RUST_DIR}/server-admin-rs-linux-amd64" amd64
make_elf "${RUST_DIR}/server-admin-rs-linux-arm64" arm64

cat > "${FNPACK_BIN}" <<'FNPACK'
#!/bin/bash
set -euo pipefail
if [ "${FAKE_FNPACK_PLATFORM:-}" = "wrong" ]; then
  sed -i.bak 's/^platform=.*/platform=wrong/' manifest
fi
if [ "${FAKE_FNPACK_EXTRA_GATEWAY:-0}" = "1" ]; then
  cp app/server/server-admin-rs app/server/go-reauth-proxy-linux-extra
fi
tar -czf app.tgz -C app .
tar -czf fn-knock.fpk manifest app.tgz
FNPACK
chmod 755 "${FNPACK_BIN}"

run_fpk() {
  FN_KNOCK_ARTIFACTS_DIR="${ARTIFACTS_DIR}" \
  FN_KNOCK_PREPARED_RUNTIME_DIR="${RUNTIME_DIR}" \
  FN_KNOCK_PREPARED_FPK_RUST_BACKEND_DIR="${RUST_DIR}" \
  FN_KNOCK_FPK_SOURCE_DIR="${SOURCE_DIR}" \
  FN_KNOCK_FPK_OUTPUT_DIR="${ARTIFACTS_DIR}/fpk" \
  FN_KNOCK_FNPACK_BIN="${FNPACK_BIN}" \
    bash "${ROOT_DIR}/scripts/fn-knock-package-fpk.sh"
}

run_fpk_wrong_platform() {
  FAKE_FNPACK_PLATFORM=wrong run_fpk
}

run_fpk_with_extra_gateway() {
  FAKE_FNPACK_EXTRA_GATEWAY=1 run_fpk
}

run_fpk >/dev/null
[ -s "${ARTIFACTS_DIR}/fpk/fn-knock-${VERSION}-fnos-amd64.fpk" ] || fail "amd64 FPK is missing"
[ -s "${ARTIFACTS_DIR}/fpk/fn-knock-${VERSION}-fnos-arm64.fpk" ] || fail "arm64 FPK is missing"

expect_failure "unexpected FPK platform" run_fpk_wrong_platform
expect_failure \
  "contains gateways for more than one architecture" \
  run_fpk_with_extra_gateway

make_elf "${RUNTIME_DIR}/server/go-reauth-proxy-linux-arm64" amd64
expect_failure "is not Linux arm64" run_fpk

printf '[test-fpk-staging] all staging tests passed\n'
