#!/usr/bin/env bash
# Test Rust against the release-pinned Go sources without modifying either checkout.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
GO_REPO="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"
GATEWAY_COMMIT="$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["gatewayCommit"])' "${ROOT_DIR}/version.json")"
TEST_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-auth-contract.XXXXXX")"
trap 'rm -rf "${TEST_DIR}"' EXIT
mkdir -p "${TEST_DIR}/gateway"
git -C "${GO_REPO}" archive "${GATEWAY_COMMIT}" | tar -x -C "${TEST_DIR}/gateway"
cp "${ROOT_DIR}/scripts/tests/fixtures/advanced-auth-control_test.go" "${TEST_DIR}/gateway/pkg/admin/fn_knock_auth_contract_test.go"
cp "${ROOT_DIR}/scripts/tests/fixtures/advanced-auth-matching_test.go" "${TEST_DIR}/gateway/pkg/proxy/fn_knock_auth_contract_test.go"
export FN_KNOCK_TEST_RUST_ROOT="${ROOT_DIR}/apps/server-admin-rs"
export FN_KNOCK_TEST_AUTH_PAYLOAD="${TEST_DIR}/payload.json"
printf 'Testing advanced authentication against gateway %s\n' "${GATEWAY_COMMIT}"
cd "${TEST_DIR}/gateway"
go test ./pkg/admin -run '^TestFnKnockAdvancedAuthControlContract$' -count=1 -v
go test ./pkg/proxy -run '^TestFnKnockAdvancedAuthRequestMatching$' -count=1 -v
