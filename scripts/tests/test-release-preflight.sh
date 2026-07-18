#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-preflight-test.XXXXXX")"
FIXTURE="${WORK_DIR}/fixture"
VERSION="$(jq -r '.version' "${ROOT_DIR}/version.json")"
TAG="v${VERSION}"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-release-preflight] ERROR: %s\n' "$*" >&2
  exit 1
}

write_fixture() {
  rm -rf "${FIXTURE}"
  mkdir -p \
    "${FIXTURE}/apps/fn-knock" \
    "${FIXTURE}/apps/server-admin-rs" \
    "${FIXTURE}/apps/fn-knock-desktop/native" \
    "${FIXTURE}/apps/fn-knock-desktop" \
    "${FIXTURE}/release-notes"
  printf '{"version":"%s"}\n' "${VERSION}" > "${FIXTURE}/version.json"
  printf 'appname=fn-knock\nversion=%s\nplatform=x86\n' "${VERSION}" > "${FIXTURE}/apps/fn-knock/manifest"
  printf '[package]\nname = "server-admin-rs"\nversion = "%s"\n' "${VERSION}" > "${FIXTURE}/apps/server-admin-rs/Cargo.toml"
  printf '[[package]]\nname = "server-admin-rs"\nversion = "%s"\n' "${VERSION}" > "${FIXTURE}/apps/server-admin-rs/Cargo.lock"
  printf '{"name":"fn-knock-desktop","version":"%s"}\n' "${VERSION}" > "${FIXTURE}/apps/fn-knock-desktop/package.json"
  printf '{"packages":{"apps/fn-knock-desktop":{"version":"%s"}}}\n' "${VERSION}" > "${FIXTURE}/package-lock.json"
  printf '[package]\nname = "fn-knock-desktop"\nversion = "%s"\n' "${VERSION}" > "${FIXTURE}/apps/fn-knock-desktop/native/Cargo.toml"
  printf '[[package]]\nname = "fn-knock-desktop"\nversion = "%s"\n' "${VERSION}" > "${FIXTURE}/apps/fn-knock-desktop/native/Cargo.lock"
  printf '# %s\n\nRelease notes.\n' "${VERSION}" > "${FIXTURE}/release-notes/${VERSION}.md"
}

run_preflight() {
  FN_KNOCK_ROOT_DIR="${FIXTURE}" \
  FN_KNOCK_PREFLIGHT_SKIP_CARGO_METADATA=1 \
    bash "${ROOT_DIR}/scripts/release-preflight.sh" "$@"
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

write_fixture
run_preflight "${TAG}" >/dev/null

write_fixture
expect_failure "release tag must match vX.Y.Z" run_preflight "release-${VERSION}"

write_fixture
expect_failure "tag/version mismatch" run_preflight v0.0.0

write_fixture
sed -i.bak "s/version=${VERSION}/version=0.0.0/" "${FIXTURE}/apps/fn-knock/manifest"
expect_failure "fnOS manifest version mismatch" run_preflight "${TAG}"

write_fixture
printf '' > "${FIXTURE}/release-notes/${VERSION}.md"
expect_failure "release notes are missing or empty" run_preflight "${TAG}"

write_fixture
jq '.packages["apps/fn-knock-desktop"].version = "0.0.0"' \
  "${FIXTURE}/package-lock.json" > "${FIXTURE}/package-lock.json.tmp"
mv "${FIXTURE}/package-lock.json.tmp" "${FIXTURE}/package-lock.json"
expect_failure "desktop package-lock version mismatch" run_preflight "${TAG}"

GO_FIXTURE="${WORK_DIR}/go-repository"
mkdir -p "${GO_FIXTURE}"
git -C "${GO_FIXTURE}" init -q
git -C "${GO_FIXTURE}" config user.email test@example.invalid
git -C "${GO_FIXTURE}" config user.name "Release Test"
printf 'module example.invalid/release-test\n\ngo 1.22\n' > "${GO_FIXTURE}/go.mod"
git -C "${GO_FIXTURE}" add go.mod
git -C "${GO_FIXTURE}" commit -qm fixture
expect_failure \
  "does not match" \
  env \
    FN_KNOCK_GO_SOURCE_COMMIT=0000000000000000000000000000000000000000 \
    FN_KNOCK_GO_SKIP_TESTS=1 \
    bash "${ROOT_DIR}/scripts/build-go-release.sh" "${GO_FIXTURE}" "${WORK_DIR}/go-output"

printf '[test-release-preflight] all contract tests passed\n'
