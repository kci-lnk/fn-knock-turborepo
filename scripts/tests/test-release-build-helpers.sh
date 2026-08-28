#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT_DIR}/dist"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/release-build-helper-test.XXXXXX")"
FAKE_BIN="${WORK_DIR}/bin"
GO_FIXTURE="${WORK_DIR}/go-repository"
GO_OUTPUT="${WORK_DIR}/go-output"
RUST_OUTPUT="${WORK_DIR}/server-admin-rs-linux-amd64"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-release-build-helpers] ERROR: %s\n' "$*" >&2
  exit 1
}

assert_vendored_vt100_freshness_guard() {
  local script="$1"
  local function_body

  function_body="$(awk '
    /^rust_backend_is_fresh\(\) \{/ { capture = 1 }
    capture { print }
    capture && /^}$/ { exit }
  ' "${script}")"
  [ -n "${function_body}" ] || fail "missing rust_backend_is_fresh in ${script}"
  printf '%s\n' "${function_body}" | grep -Fq '"${ROOT_DIR}/third_party/vt100"' || \
    fail "Rust freshness check ignores vendored vt100 sources in ${script}"
}

mkdir -p "${FAKE_BIN}" "${GO_FIXTURE}/pkg/grpc/pb"

cat > "${FAKE_BIN}/go" <<'EOF'
#!/bin/bash
set -euo pipefail

[ "${1:-}" = "build" ] || exit 0
shift
output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    output="$2"
    break
  fi
  shift
done
[ -n "${output}" ]
cp /usr/bin/true "${output}"
EOF

cat > "${FAKE_BIN}/file" <<'EOF'
#!/bin/bash
set -euo pipefail

path="${@: -1}"
case "${path}" in
  *windows-amd64.exe)
    printf 'PE32+ executable (console) x86-64\n'
    ;;
  *linux-arm64)
    printf 'ELF 64-bit LSB executable, ARM aarch64, statically linked, Go BuildID=test\n'
    ;;
  *linux-arm)
    printf 'ELF 32-bit LSB executable, ARM, statically linked, Go BuildID=test\n'
    ;;
  *linux-amd64)
    printf 'ELF 64-bit LSB executable, x86-64, statically linked, Go BuildID=test\n'
    ;;
  *)
    exit 1
    ;;
esac
EOF

cat > "${FAKE_BIN}/readelf" <<'EOF'
#!/bin/bash
set -euo pipefail

mode="$1"
path="$2"
case "${mode}" in
  -h)
    case "${path}" in
      *linux-arm64) printf '  Machine: AArch64\n' ;;
      *linux-arm) printf '  Machine: ARM\n' ;;
      *linux-amd64) printf '  Machine: Advanced Micro Devices X86-64\n' ;;
      *) exit 1 ;;
    esac
    ;;
  -d)
    printf 'There is no dynamic section in this file.\n'
    ;;
  --version-info)
    :
    ;;
  *)
    exit 1
    ;;
esac
EOF

cat > "${FAKE_BIN}/docker" <<'EOF'
#!/bin/bash
set -euo pipefail

container_output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-e" ] && [ "$#" -ge 2 ]; then
    case "$2" in
      FN_KNOCK_RUST_OUT=*) container_output="${2#FN_KNOCK_RUST_OUT=}" ;;
    esac
    shift 2
    continue
  fi
  shift
done
[ -n "${container_output}" ]
host_output="${FN_TEST_ROOT_DIR}/${container_output#/workspace/}"
mkdir -p "$(dirname "${host_output}")"
cp /usr/bin/true "${host_output}"
/bin/chmod 755 "${host_output}"
EOF

cat > "${FAKE_BIN}/chmod" <<'EOF'
#!/bin/bash
printf 'unexpected host chmod: %s\n' "$*" >&2
exit 99
EOF

/bin/chmod 755 \
  "${FAKE_BIN}/go" \
  "${FAKE_BIN}/file" \
  "${FAKE_BIN}/readelf" \
  "${FAKE_BIN}/docker" \
  "${FAKE_BIN}/chmod"

git -C "${GO_FIXTURE}" init -q
git -C "${GO_FIXTURE}" config user.email test@example.invalid
git -C "${GO_FIXTURE}" config user.name "Release Test"
printf 'module go-reauth-proxy\n\ngo 1.22\n' > "${GO_FIXTURE}/go.mod"
CONTROL_API_VERSION="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
printf 'package pb\n\ntype ControlApiVersion int32\n\nconst (\n\tControlApiVersion_CONTROL_API_VERSION_CURRENT ControlApiVersion = %s\n)\n' \
  "${CONTROL_API_VERSION}" > "${GO_FIXTURE}/pkg/grpc/pb/gateway.pb.go"
git -C "${GO_FIXTURE}" add go.mod pkg/grpc/pb/gateway.pb.go
git -C "${GO_FIXTURE}" commit -qm fixture
GO_COMMIT="$(git -C "${GO_FIXTURE}" rev-parse HEAD)"

PATH="${FAKE_BIN}:${PATH}" \
FN_KNOCK_GO_SOURCE_COMMIT="${GO_COMMIT}" \
FN_KNOCK_GO_SKIP_TESTS=1 \
  bash "${ROOT_DIR}/scripts/build-go-release.sh" "${GO_FIXTURE}" "${GO_OUTPUT}" >/dev/null

for name in \
  go-reauth-proxy-linux-amd64 \
  go-reauth-proxy-linux-arm64 \
  go-reauth-proxy-linux-arm \
  go-reauth-proxy-windows-amd64.exe
do
  [ -x "${GO_OUTPUT}/${name}" ] || fail "missing executable fixture output: ${name}"
done

PATH="${FAKE_BIN}:${PATH}" \
FN_TEST_ROOT_DIR="${ROOT_DIR}" \
CI=true \
  bash "${ROOT_DIR}/scripts/build-rust-backend.sh" musl amd64 "${RUST_OUTPUT}" >/dev/null

[ -x "${RUST_OUTPUT}" ] || fail "musl helper did not preserve executable mode"

assert_vendored_vt100_freshness_guard "${ROOT_DIR}/scripts/fn-knock-prepare-artifacts.sh"
assert_vendored_vt100_freshness_guard "${ROOT_DIR}/scripts/fn-knock-docker.sh"

printf '[test-release-build-helpers] all build helper tests passed\n'
