#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-release-cli-test.XXXXXX")"
FIXTURE="${WORK_DIR}/fixture"
GO_FIXTURE="${WORK_DIR}/Go-Reauth-Proxy"
CURRENT_VERSION="$(jq -r '.version' "${ROOT_DIR}/version.json")"
IFS=. read -r CURRENT_MAJOR CURRENT_MINOR CURRENT_PATCH <<< "${CURRENT_VERSION}"
NEXT_PATCH="${CURRENT_MAJOR}.${CURRENT_MINOR}.$((CURRENT_PATCH + 1))"
NEXT_MINOR="${CURRENT_MAJOR}.$((CURRENT_MINOR + 1)).0"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-release-cli] ERROR: %s\n' "$*" >&2
  exit 1
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

run_cli() {
  FN_KNOCK_ROOT_DIR="${FIXTURE}" \
  FN_KNOCK_GO_REAUTH_PROXY_DIR="${GO_FIXTURE}" \
  FN_KNOCK_PREFLIGHT_SKIP_CARGO_METADATA=1 \
    node "${ROOT_DIR}/scripts/fn-knock-release.mjs" "$@"
}

mkdir -p \
  "${FIXTURE}/apps/fn-knock" \
  "${FIXTURE}/apps/server-admin-rs" \
  "${FIXTURE}/apps/fn-knock-desktop/native" \
  "${FIXTURE}/apps/fn-knock-desktop" \
  "${FIXTURE}/release-notes" \
  "${FIXTURE}/scripts" \
  "${GO_FIXTURE}/pkg/version"

for relative_path in \
  version.json \
  apps/fn-knock/manifest \
  apps/server-admin-rs/Cargo.toml \
  apps/server-admin-rs/Cargo.lock \
  apps/fn-knock-desktop/package.json \
  package-lock.json \
  apps/fn-knock-desktop/native/Cargo.toml \
  apps/fn-knock-desktop/native/Cargo.lock \
  "release-notes/${CURRENT_VERSION}.md" \
  scripts/release-preflight.sh
do
  cp "${ROOT_DIR}/${relative_path}" "${FIXTURE}/${relative_path}"
done

git -C "${FIXTURE}" init -q
git -C "${FIXTURE}" config user.email test@example.invalid
git -C "${FIXTURE}" config user.name "Release CLI Test"
git -C "${FIXTURE}" add .
git -C "${FIXTURE}" commit -qm "chore: current release"
git -C "${FIXTURE}" tag -a "v${CURRENT_VERSION}" -m "Release v${CURRENT_VERSION}"
printf 'release fixture\n' > "${FIXTURE}/release-change.txt"
git -C "${FIXTURE}" add release-change.txt
git -C "${FIXTURE}" commit -qm "feat: include release CLI fixture"

printf 'package version\n\nvar (\n\tVersion = "%s"\n\tCommit = "unknown"\n)\n' \
  "${CURRENT_VERSION}" > "${GO_FIXTURE}/pkg/version/version.go"
printf "version: '3'\nvars:\n  VERSION: '{{.FN_KNOCK_VERSION | default \"%s\"}}'\n" \
  "${CURRENT_VERSION}" > "${GO_FIXTURE}/Taskfile.yml"
git -C "${GO_FIXTURE}" init -q
git -C "${GO_FIXTURE}" config user.email test@example.invalid
git -C "${GO_FIXTURE}" config user.name "Release CLI Test"
git -C "${GO_FIXTURE}" add .
git -C "${GO_FIXTURE}" commit -qm "chore: current gateway release"

run_cli status >/dev/null
run_cli gateway-check "${CURRENT_VERSION}" >/dev/null
run_cli prepare patch --dry-run > "${WORK_DIR}/patch-dry-run.txt"
grep -Fq "${CURRENT_VERSION} -> ${NEXT_PATCH}" "${WORK_DIR}/patch-dry-run.txt" || \
  fail "patch dry-run did not select ${NEXT_PATCH}"
[ "$(jq -r '.version' "${FIXTURE}/version.json")" = "${CURRENT_VERSION}" ] || \
  fail "dry-run modified version.json"
[ ! -e "${FIXTURE}/release-notes/${NEXT_PATCH}.md" ] || \
  fail "dry-run created release notes"

run_cli prepare minor --dry-run > "${WORK_DIR}/minor-dry-run.txt"
grep -Fq "${CURRENT_VERSION} -> ${NEXT_MINOR}" "${WORK_DIR}/minor-dry-run.txt" || \
  fail "minor dry-run did not select ${NEXT_MINOR}"

printf '# Custom release notes\n\n- Reviewed change.\n' > "${WORK_DIR}/custom-notes.md"
run_cli prepare patch --dry-run --notes-file "${WORK_DIR}/custom-notes.md" \
  > "${WORK_DIR}/custom-notes-dry-run.txt"
grep -Fq -- "- Reviewed change." "${WORK_DIR}/custom-notes-dry-run.txt" || \
  fail "custom release notes were not loaded"

run_cli prepare patch > "${WORK_DIR}/prepare.txt"
grep -Fq "prepared v${NEXT_PATCH}" "${WORK_DIR}/prepare.txt" || \
  fail "prepare did not finish successfully"
grep -Fq "# fn-knock ${NEXT_PATCH}" "${FIXTURE}/release-notes/${NEXT_PATCH}.md" || \
  fail "generated release notes have the wrong heading"
grep -Fq -- "- feat: include release CLI fixture" \
  "${FIXTURE}/release-notes/${NEXT_PATCH}.md" || \
  fail "generated release notes do not contain commit subjects"
grep -Fq "Version = \"${NEXT_PATCH}\"" "${GO_FIXTURE}/pkg/version/version.go" || \
  fail "Go gateway source version was not updated"
grep -Fq "default \"${NEXT_PATCH}\"" "${GO_FIXTURE}/Taskfile.yml" || \
  fail "Go gateway Taskfile version was not updated"

run_cli status >/dev/null
run_cli check "${NEXT_PATCH}" >/dev/null
expect_failure "Git worktrees must be clean" run_cli prepare minor

sed -i.bak "s/default \"${NEXT_PATCH}\"/default \"0.0.0\"/" \
  "${GO_FIXTURE}/Taskfile.yml"
expect_failure "Go gateway versions are not aligned" \
  run_cli gateway-check "${NEXT_PATCH}"

printf '[test-release-cli] all release CLI tests passed\n'
