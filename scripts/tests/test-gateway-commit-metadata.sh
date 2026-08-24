#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-gateway-commit-test.XXXXXX")"
GO_REPOSITORY="${WORK_DIR}/go-repository"
FAKE_BIN="${WORK_DIR}/bin"
BUILD_DIR="${WORK_DIR}/build"
OUTPUT_DIR="${WORK_DIR}/output"
CAPTURE_FILE="${WORK_DIR}/commit.txt"
ERROR_FILE="${WORK_DIR}/error.txt"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-gateway-commit-metadata] ERROR: %s\n' "$*" >&2
  exit 1
}

CONTROL_API_VERSION="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"
mkdir -p "${GO_REPOSITORY}/pkg/grpc/pb" "${FAKE_BIN}" "${BUILD_DIR}" "${OUTPUT_DIR}"
printf 'ControlApiVersion_CONTROL_API_VERSION_CURRENT ControlApiVersion = %s\n' \
  "${CONTROL_API_VERSION}" > "${GO_REPOSITORY}/pkg/grpc/pb/gateway.pb.go"
printf 'version: 3\n' > "${GO_REPOSITORY}/Taskfile.yml"
git -C "${GO_REPOSITORY}" init -q
git -C "${GO_REPOSITORY}" config user.name 'fn-knock test'
git -C "${GO_REPOSITORY}" config user.email 'fn-knock-test@example.invalid'
git -C "${GO_REPOSITORY}" add .
git -C "${GO_REPOSITORY}" commit -qm 'initial gateway fixture'

EXPECTED_COMMIT="$(git -C "${GO_REPOSITORY}" rev-parse HEAD)"
[[ "${EXPECTED_COMMIT}" =~ ^[0-9a-f]{40}$ ]] || fail "Go checkout commit is invalid"

cat > "${FAKE_BIN}/task" <<'EOF'
#!/bin/bash
set -euo pipefail
[ "${1:-}" = "build" ] || exit 64
printf '%s\n' "${FN_KNOCK_COMMIT:?}" > "${FN_KNOCK_TEST_COMMIT_CAPTURE:?}"
cp /usr/bin/true "${FN_KNOCK_TEST_BUILD_DIR:?}/go-reauth-proxy-linux-amd64"
printf '%s\n' "${FN_KNOCK_VERSION:?}" > \
  "${FN_KNOCK_TEST_BUILD_DIR}/go-reauth-proxy-linux-amd64.version"
if [ "${FN_KNOCK_TEST_MUTATE_HEAD:-0}" = "1" ]; then
  printf 'changed during build\n' > gateway-drift.txt
  git add gateway-drift.txt
  git commit -qm 'simulate concurrent gateway commit'
fi
EOF
chmod 755 "${FAKE_BIN}/task"

run_builder() {
  local expected_commit="$1"
  local mutate_head="${2:-0}"

  PATH="${FAKE_BIN}:${PATH}" \
  FN_KNOCK_GATEWAY_COMMIT="${expected_commit}" \
  FN_KNOCK_GO_REAUTH_PROXY_DIR="${GO_REPOSITORY}" \
  FN_KNOCK_GO_REAUTH_PROXY_BUILD_DIR="${BUILD_DIR}" \
  FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD=1 \
  FN_KNOCK_TEST_BUILD_DIR="${BUILD_DIR}" \
  FN_KNOCK_TEST_COMMIT_CAPTURE="${CAPTURE_FILE}" \
  FN_KNOCK_TEST_MUTATE_HEAD="${mutate_head}" \
    bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${OUTPUT_DIR}" amd64
}

run_builder "${EXPECTED_COMMIT}" >/dev/null

[ "$(tr -d '\r\n' < "${CAPTURE_FILE}")" = "${EXPECTED_COMMIT}" ] || \
  fail "shared builder did not inject the full gateway commit"
[ "$(tr -d '\r\n' < "${BUILD_DIR}/go-reauth-proxy-linux-amd64.commit")" = "${EXPECTED_COMMIT}" ] || \
  fail "shared builder did not persist full commit cache metadata"
[ -x "${OUTPUT_DIR}/go-reauth-proxy-linux-amd64" ] || \
  fail "shared builder did not prepare the gateway binary"

printf 'uncommitted gateway source\n' > "${GO_REPOSITORY}/dirty.go"
if run_builder "${EXPECTED_COMMIT}" > /dev/null 2> "${ERROR_FILE}"; then
  fail "shared builder accepted a dirty gateway checkout"
fi
grep -Fq 'working tree is not clean' "${ERROR_FILE}" || \
  fail "dirty checkout failure did not explain the cause"
rm -f "${GO_REPOSITORY}/dirty.go"

MISMATCHED_COMMIT='0000000000000000000000000000000000000000'
if run_builder "${MISMATCHED_COMMIT}" > /dev/null 2> "${ERROR_FILE}"; then
  fail "shared builder accepted a gateway checkout that did not match the locked commit"
fi
grep -Fq 'HEAD changed during artifact preparation (before gateway build)' "${ERROR_FILE}" || \
  fail "locked commit mismatch failure did not explain the cause"

if run_builder "${EXPECTED_COMMIT}" 1 > /dev/null 2> "${ERROR_FILE}"; then
  fail "shared builder accepted a gateway HEAD change during build"
fi
grep -Fq 'HEAD changed during artifact preparation (after gateway build)' "${ERROR_FILE}" || \
  fail "concurrent HEAD change failure did not identify the build phase"
for cache_file in \
  "${BUILD_DIR}/go-reauth-proxy-linux-amd64" \
  "${BUILD_DIR}/go-reauth-proxy-linux-amd64.commit" \
  "${BUILD_DIR}/go-reauth-proxy-linux-amd64.version"; do
  [ ! -e "${cache_file}" ] || \
    fail "shared builder left a reusable cache entry after gateway HEAD changed: ${cache_file}"
done

if grep -Fq 'gatewayCommit' "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh"; then
  fail "shared builder still pins the gateway checkout to version.json"
fi
if grep -Fq 'EXPECTED_GATEWAY_COMMIT' \
  "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh"; then
  fail "Synology builder still pins the gateway checkout to version.json"
fi

if grep -En 'rev-parse[[:space:]]+--short' \
  "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" \
  "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" >/dev/null; then
  fail "a package builder still truncates the gateway commit"
fi
grep -Fq 'rev-parse HEAD' "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" || \
  fail "Synology builder does not resolve the full gateway commit"
grep -Fq 'lock_docker_gateway_commit' "${ROOT_DIR}/scripts/fn-knock-docker.sh" || \
  fail "Docker builder does not lock the gateway commit for the whole image build"
grep -Fq 'FN_KNOCK_GATEWAY_COMMIT="${DOCKER_GATEWAY_COMMIT}"' \
  "${ROOT_DIR}/scripts/fn-knock-docker.sh" || \
  fail "Docker Rust build does not inject the locked gateway commit"
grep -Fq '"${out_bin}.gateway-commit"' "${ROOT_DIR}/scripts/fn-knock-docker.sh" || \
  fail "Docker Rust build does not persist gateway commit cache metadata"
if grep -Fq 'if [ -f "${dst}" ]; then' "${ROOT_DIR}/scripts/fn-knock-docker.sh"; then
  fail "Docker no-build mode accepts Rust binaries without commit metadata validation"
fi

printf '[test-gateway-commit-metadata] gateway commit metadata validation passed\n'
