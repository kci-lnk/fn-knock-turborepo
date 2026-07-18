#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT_DIR}/dist"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/synology-package-test.XXXXXX")"
FAKE_BIN="${WORK_DIR}/bin"
RUNTIME_DIR="${WORK_DIR}/runtime"
RUST_DIR="${WORK_DIR}/rust"
GATEWAY_BIN="${WORK_DIR}/go-reauth-proxy-linux-amd64"
OUTPUT_PATH="${WORK_DIR}/fn-knock-synology-test.spk"
PACKAGE_TGZ="${WORK_DIR}/package.tgz"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-synology-package-validation] ERROR: %s\n' "$*" >&2
  exit 1
}

mkdir -p \
  "${FAKE_BIN}" \
  "${RUNTIME_DIR}/ui/www" \
  "${RUNTIME_DIR}/server-auth-view/dist" \
  "${RUNTIME_DIR}/server/server-admin/resources" \
  "${RUST_DIR}"

cat > "${FAKE_BIN}/file" <<'EOF'
#!/bin/bash
printf 'ELF 64-bit LSB executable, x86-64, statically linked\n'
EOF
chmod 755 "${FAKE_BIN}/file"

cp /usr/bin/true "${GATEWAY_BIN}"
cp /usr/bin/true "${RUST_DIR}/server-admin-rs-linux-amd64"
chmod 755 "${GATEWAY_BIN}" "${RUST_DIR}/server-admin-rs-linux-amd64"
printf 'fixture\n' > "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip"
printf '<html>admin</html>\n' > "${RUNTIME_DIR}/ui/www/index.html"
printf '<html>auth</html>\n' > "${RUNTIME_DIR}/server-auth-view/dist/index.html"

# Keep the payload listing larger than a pipe buffer. The former
# `tar ... | grep -q` validation closed stdout after an early match, causing
# tar to fail with EPIPE under `set -o pipefail`.
for index in $(seq 1 2500); do
  printf 'fixture\n' > "${RUNTIME_DIR}/ui/www/zz-validation-${index}.txt"
done

PATH="${FAKE_BIN}:${PATH}" \
FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE=1 \
FN_KNOCK_SYNOLOGY_GATEWAY_BIN="${GATEWAY_BIN}" \
FN_KNOCK_PREPARED_RUNTIME_DIR="${RUNTIME_DIR}" \
FN_KNOCK_PREPARED_MUSL_RUST_BACKEND_DIR="${RUST_DIR}" \
FN_KNOCK_SYNOLOGY_OUTPUT="${OUTPUT_PATH}" \
  bash "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" >/dev/null

[ -s "${OUTPUT_PATH}" ] || fail "Synology builder did not produce an SPK"
spk_listing="$(tar -tf "${OUTPUT_PATH}")"
grep -Fqx 'INFO' <<< "${spk_listing}" || fail "SPK fixture is missing INFO"
grep -Fqx 'package.tgz' <<< "${spk_listing}" || fail "SPK fixture is missing package.tgz"

tar -xOf "${OUTPUT_PATH}" package.tgz > "${PACKAGE_TGZ}"
payload_listing="$(tar -tzf "${PACKAGE_TGZ}")"
grep -Fqx './bin/server-admin-rs' <<< "${payload_listing}" || fail "SPK fixture is missing backend"
grep -Fqx './bin/go-reauth-proxy' <<< "${payload_listing}" || fail "SPK fixture is missing gateway"

if grep -Eq 'tar[[:space:]]+-t(z)?f[^|]*[|][[:space:]]*grep .*-[A-Za-z]*q' \
  "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh"; then
  fail "Synology validation must not stream tar output into grep -q"
fi

printf '[test-synology-package-validation] SPK validation passed\n'
