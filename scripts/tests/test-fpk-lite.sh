#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"
mkdir -p "${ROOT_DIR}/dist"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/fpk-lite-test.XXXXXX")"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  echo "[test-fpk-lite] ERROR: $*" >&2
  exit 1
}

assert_file_contains() {
  local file="$1"
  local pattern="$2"
  rg -q -- "${pattern}" "${file}" || fail "${file} does not contain ${pattern}"
}

assert_tree_does_not_contain() {
  local path="$1"
  local pattern="$2"
  if rg -q --hidden --glob '!dist/**' --glob '!node_modules/**' -- "${pattern}" "${path}"; then
    fail "${path} unexpectedly contains ${pattern}"
  fi
}

assert_fpk_has_no_container_paths() {
  local fpk="$1"
  local matches

  [ -f "${fpk}" ] || return 0
  matches="$(
    tar -xzOf "${fpk}" app.tgz \
      | tar -tzf - \
      | rg -i '(^|/)[^/]*(docker|compose|container)[^/]*($|/)' \
      || true
  )"
  [ -z "${matches}" ] || fail "${fpk} contains container-related paths: ${matches}"
}

[ ! -e apps/fn-knock-docker ] || fail "legacy apps/fn-knock-docker still exists"
[ ! -e scripts/fn-knock-docker-fpk.sh ] || fail "legacy Docker FPK deploy script still exists"
[ -f scripts/fn-knock-docker.sh ] || fail "supported Docker deployment script was removed"
[ -d deploy/docker ] || fail "supported Docker deployment context was removed"

assert_file_contains apps/fn-knock-lite/manifest '^appname=fn-knock-lite$'
assert_file_contains apps/fn-knock-lite/manifest '^display_name=敲门knock Lite$'
assert_file_contains apps/fn-knock-lite/manifest '^desktop_applaunchname=fn-knock-lite\.Application$'
assert_file_contains apps/fn-knock-lite/manifest '^service_port=8999$'
assert_file_contains apps/fn-knock-lite/manifest 'https://www\.fnknock\.cn/'
assert_file_contains apps/fn-knock-lite/manifest 'https://docs\.fnknock\.cn/'
assert_file_contains apps/fn-knock-lite/manifest '1081609274'
assert_file_contains apps/fn-knock/manifest 'https://www\.fnknock\.cn/'
assert_file_contains apps/fn-knock/manifest 'https://docs\.fnknock\.cn/'
assert_file_contains apps/fn-knock/manifest '1081609274'
assert_file_contains apps/fn-knock/manifest '^service_port=7999$'

assert_file_contains apps/fn-knock-lite/config/privilege '"run-as": "package"'
assert_file_contains apps/fn-knock-lite/config/privilege '"username": "fn-knock-lite"'
assert_file_contains apps/fn-knock-lite/config/resource '"name": "fn-knock-lite"'
assert_file_contains apps/fn-knock-lite/app/ui/config '"fn-knock-lite.Application"'
assert_file_contains apps/fn-knock-lite/app/ui/config '/cgi/ThirdParty/fn-knock-lite/index\.cgi/'

assert_file_contains apps/fn-knock-lite/cmd/main 'FN_KNOCK_RUNTIME_TARGET="fpk-lite"'
assert_file_contains apps/fn-knock-lite/cmd/main 'FN_KNOCK_DISABLE_REDIS_MIGRATION="1"'
assert_file_contains apps/fn-knock-lite/cmd/main 'GATEWAY_CONFIG_DIR="\$\{PKG_VAR_DIR}/gateway"'
assert_file_contains apps/fn-knock-lite/cmd/main 'requires an integer between 1024 and 65535'
assert_file_contains apps/fn-knock-lite/cmd/main '"8991"'
assert_file_contains apps/fn-knock-lite/cmd/main 'READINESS_MARKER=.*runtime\.ready'
assert_file_contains apps/fn-knock-lite/cmd/main 'FN_KNOCK_START_TIMEOUT_SECONDS:-300'
assert_file_contains apps/fn-knock-lite/cmd/main 'FN_KNOCK_STOP_TIMEOUT_SECONDS:-75'
assert_file_contains apps/fn-knock-lite/cmd/main 'FN_KNOCK_FORCE_KILL_TIMEOUT_SECONDS:-10'
assert_file_contains apps/fn-knock-lite/cmd/main 'wait_runtime_ready'
assert_file_contains apps/fn-knock-lite/cmd/main 'Incomplete runtime detected'
assert_file_contains apps/fn-knock-lite/cmd/main 'stop_matching_processes "\$\{BACKEND_ENTRY\}"'
assert_file_contains apps/fn-knock-lite/cmd/main 'stop_matching_processes "\$\{GATEWAY_BIN\}"'
assert_file_contains apps/fn-knock-lite/app/ui/index.cgi 'TARGET_PORT="8998"'
assert_tree_does_not_contain apps/fn-knock-lite '/usr/local/etc/fn-knock'
assert_tree_does_not_contain apps/fn-knock-lite '"run-as"[[:space:]]*:[[:space:]]*"root"'
assert_tree_does_not_contain apps/fn-knock-lite 'docker-compose|compose\\.ya?ml|container_name:'
assert_tree_does_not_contain apps/fn-knock-lite 'iptables|ip6tables|nftables|sysctl'

for wizard in apps/fn-knock-lite/wizard/install apps/fn-knock-lite/wizard/config; do
  assert_file_contains "${wizard}" '"8998"'
  assert_file_contains "${wizard}" '"8997"'
  assert_file_contains "${wizard}" '"8996"'
  assert_file_contains "${wizard}" '"8999"'
  assert_file_contains "${wizard}" '1024-65535'
done

assert_file_contains package.json '"fn-knock:lite:build-package"'
assert_file_contains package.json '"fn-knock:lite:fpk:deploy"'
assert_file_contains scripts/fn-knock-lite-deploy.sh 'FN_KNOCK_WIZARD_ADMIN_VIEW_PORT.*8991'
assert_file_contains scripts/fn-knock-lite-deploy.sh 'FN_KNOCK_WIZARD_GO_REPROXY_PORT.*8999'
assert_file_contains scripts/fn-knock-lite-deploy.sh \
  'source.*fn-knock-lite-sync-go-grpc\.sh'
assert_file_contains apps/server-admin-view/src/lib/update-presentation.ts \
  'https://www\.fnknock\.cn/'
assert_file_contains apps/fn-knock-lite/scripts/build-package.sh \
  'FN_KNOCK_FRONTEND_TARGET="fpk-lite"'
assert_file_contains apps/fn-knock-lite/scripts/build-package.sh \
  'VITE_FN_KNOCK_DEFAULT_AUTH_PORT.*8997'
assert_file_contains apps/fn-knock-lite/scripts/build-package.sh \
  'source.*fn-knock-lite-sync-go-grpc\.sh'
assert_file_contains scripts/fn-knock-lite-sync-go-grpc.sh \
  'scripts/sync-go-grpc-contract\.sh'
assert_file_contains scripts/fn-knock-lite-sync-go-grpc.sh \
  'FN_KNOCK_LITE_GRPC_SYNC_GO_COMPLETED'
assert_tree_does_not_contain package.json 'fn-knock:fpk-docker'
assert_tree_does_not_contain .github/workflows 'fn-knock-lite'

FAKE_BIN="${WORK_DIR}/bin"
SYNC_LOG="${WORK_DIR}/grpc-sync.log"
mkdir -p "${FAKE_BIN}"
cat > "${FAKE_BIN}/bash" <<'EOF'
#!/bin/bash
set -euo pipefail
printf '%s\n' "$*" >> "${FN_KNOCK_TEST_SYNC_LOG}"
EOF
/bin/chmod 755 "${FAKE_BIN}/bash"

PATH="${FAKE_BIN}:${PATH}" \
FN_KNOCK_TEST_SYNC_LOG="${SYNC_LOG}" \
  /bin/bash -s -- "${ROOT_DIR}/scripts/fn-knock-lite-sync-go-grpc.sh" <<'EOF'
set -euo pipefail
sync_helper="$1"
source "${sync_helper}"
/bin/bash -s -- "${sync_helper}" <<'INNER'
set -euo pipefail
source "$1"
INNER
EOF

[ "$(wc -l < "${SYNC_LOG}" | tr -d '[:space:]')" = "1" ] || \
  fail "nested Lite build/deploy entrypoints must synchronize the Go gRPC contract exactly once"
assert_file_contains "${SYNC_LOG}" 'scripts/sync-go-grpc-contract\.sh'

for release_file in \
  scripts/fn-knock-package-fpk.sh \
  scripts/fn-knock-assemble-release.sh \
  scripts/release-preflight.sh \
  scripts/fn-knock-release-finalize.mjs; do
  [ -f "${release_file}" ] || fail "release isolation check target is missing: ${release_file}"
  assert_tree_does_not_contain "${release_file}" 'fn-knock-lite'
done

assert_fpk_has_no_container_paths apps/fn-knock-lite/dist/fn-knock-lite-amd64.fpk
assert_fpk_has_no_container_paths apps/fn-knock-lite/dist/fn-knock-lite-arm64.fpk

echo "[test-fpk-lite] OK"
