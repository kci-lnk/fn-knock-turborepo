#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GO_REPOSITORY="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-gateway-commit-test.XXXXXX")"
FAKE_BIN="${WORK_DIR}/bin"
BUILD_DIR="${WORK_DIR}/build"
OUTPUT_DIR="${WORK_DIR}/output"
CAPTURE_FILE="${WORK_DIR}/commit.txt"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-gateway-commit-metadata] ERROR: %s\n' "$*" >&2
  exit 1
}

EXPECTED_COMMIT="$(jq -er '.gatewayCommit' "${ROOT_DIR}/version.json")"
[[ "${EXPECTED_COMMIT}" =~ ^[0-9a-f]{40}$ ]] || fail "fixture gateway commit is invalid"
[ "$(git -C "${GO_REPOSITORY}" rev-parse HEAD)" = "${EXPECTED_COMMIT}" ] || \
  fail "Go checkout does not match version.json"

mkdir -p "${FAKE_BIN}" "${BUILD_DIR}" "${OUTPUT_DIR}"
cat > "${FAKE_BIN}/task" <<'EOF'
#!/bin/bash
set -euo pipefail
[ "${1:-}" = "build" ] || exit 64
printf '%s\n' "${FN_KNOCK_COMMIT:?}" > "${FN_KNOCK_TEST_COMMIT_CAPTURE:?}"
cp /usr/bin/true "${FN_KNOCK_TEST_BUILD_DIR:?}/go-reauth-proxy-linux-amd64"
printf '%s\n' "${FN_KNOCK_VERSION:?}" > \
  "${FN_KNOCK_TEST_BUILD_DIR}/go-reauth-proxy-linux-amd64.version"
EOF
chmod 755 "${FAKE_BIN}/task"

PATH="${FAKE_BIN}:${PATH}" \
FN_KNOCK_GO_REAUTH_PROXY_DIR="${GO_REPOSITORY}" \
FN_KNOCK_GO_REAUTH_PROXY_BUILD_DIR="${BUILD_DIR}" \
FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD=1 \
FN_KNOCK_TEST_BUILD_DIR="${BUILD_DIR}" \
FN_KNOCK_TEST_COMMIT_CAPTURE="${CAPTURE_FILE}" \
  bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${OUTPUT_DIR}" amd64 >/dev/null

[ "$(tr -d '\r\n' < "${CAPTURE_FILE}")" = "${EXPECTED_COMMIT}" ] || \
  fail "shared builder did not inject the full gateway commit"
[ "$(tr -d '\r\n' < "${BUILD_DIR}/go-reauth-proxy-linux-amd64.commit")" = "${EXPECTED_COMMIT}" ] || \
  fail "shared builder did not persist full commit cache metadata"
[ -x "${OUTPUT_DIR}/go-reauth-proxy-linux-amd64" ] || \
  fail "shared builder did not prepare the gateway binary"

if grep -En 'rev-parse[[:space:]]+--short' \
  "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" \
  "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" >/dev/null; then
  fail "a package builder still truncates the gateway commit"
fi
grep -Fq 'rev-parse HEAD' "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" || \
  fail "Synology builder does not resolve the full gateway commit"

printf '[test-gateway-commit-metadata] gateway commit metadata validation passed\n'
