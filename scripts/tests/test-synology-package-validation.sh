#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT_DIR}/dist"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/synology-package-test.XXXXXX")"
FAKE_BIN="${WORK_DIR}/bin"
RUNTIME_DIR="${WORK_DIR}/runtime"
RUST_DIR="${WORK_DIR}/rust"
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
printf '%s\n' "${EXPECTED_FILE_DESCRIPTION:?}"
EOF
chmod 755 "${FAKE_BIN}/file"

for runtime_arch in amd64 arm64 arm; do
  cp /usr/bin/true "${WORK_DIR}/go-reauth-proxy-linux-${runtime_arch}"
  cp /usr/bin/true "${RUST_DIR}/server-admin-rs-linux-${runtime_arch}"
  chmod 755 \
    "${WORK_DIR}/go-reauth-proxy-linux-${runtime_arch}" \
    "${RUST_DIR}/server-admin-rs-linux-${runtime_arch}"
done
printf 'fixture\n' > "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip"
printf '<html>admin</html>\n' > "${RUNTIME_DIR}/ui/www/index.html"
printf '<html>auth</html>\n' > "${RUNTIME_DIR}/server-auth-view/dist/index.html"

# Keep the payload listing larger than a pipe buffer. The former
# `tar ... | grep -q` validation closed stdout after an early match, causing
# tar to fail with EPIPE under `set -o pipefail`.
for index in $(seq 1 2500); do
  printf 'fixture\n' > "${RUNTIME_DIR}/ui/www/zz-validation-${index}.txt"
done

for target in \
  'x86_64:amd64:ELF 64-bit LSB executable, x86-64, statically linked' \
  'armv8:arm64:ELF 64-bit LSB executable, ARM aarch64, statically linked' \
  'armv7:arm:ELF 32-bit LSB executable, ARM, EABI5 version 1 (SYSV), statically linked'
do
  IFS=: read -r synology_arch runtime_arch file_description <<< "${target}"
  gateway_bin="${WORK_DIR}/go-reauth-proxy-linux-${runtime_arch}"
  output_path="${WORK_DIR}/fn-knock-synology-${synology_arch}-test.spk"

  PATH="${FAKE_BIN}:${PATH}" \
  EXPECTED_FILE_DESCRIPTION="${file_description}" \
  FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE=1 \
  FN_KNOCK_SYNOLOGY_GATEWAY_BIN="${gateway_bin}" \
  FN_KNOCK_PREPARED_RUNTIME_DIR="${RUNTIME_DIR}" \
  FN_KNOCK_PREPARED_MUSL_RUST_BACKEND_DIR="${RUST_DIR}" \
  FN_KNOCK_SYNOLOGY_OUTPUT="${output_path}" \
    bash "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" "${synology_arch}" >/dev/null

  [ -s "${output_path}" ] || fail "Synology builder did not produce ${synology_arch} SPK"
  spk_listing="$(tar -tf "${output_path}")"
  grep -Fqx 'INFO' <<< "${spk_listing}" || fail "${synology_arch} SPK is missing INFO"
  grep -Fqx 'package.tgz' <<< "${spk_listing}" || fail "${synology_arch} SPK is missing package.tgz"
  tar -xOf "${output_path}" INFO | grep -Fqx "arch=\"${synology_arch}\"" || \
    fail "${synology_arch} SPK INFO has the wrong architecture"
  expected_beta="$(jq -r 'if (.releaseChannel // "stable") == "stable" then "no" else "yes" end' "${ROOT_DIR}/version.json")"
  tar -xOf "${output_path}" INFO | grep -Fqx "beta=\"${expected_beta}\"" || \
    fail "${synology_arch} SPK INFO has the wrong beta marker"

  tar -xOf "${output_path}" package.tgz > "${PACKAGE_TGZ}"
  payload_listing="$(tar -tzf "${PACKAGE_TGZ}")"
  grep -Fqx './bin/server-admin-rs' <<< "${payload_listing}" || fail "${synology_arch} SPK is missing backend"
  grep -Fqx './bin/go-reauth-proxy' <<< "${payload_listing}" || fail "${synology_arch} SPK is missing gateway"
done

[ "$(find "${WORK_DIR}" -maxdepth 1 -name 'fn-knock-synology-*-test.spk' -type f | wc -l | tr -d ' ')" = "3" ] || \
  fail "consecutive architecture builds removed another Synology package"

jq -e '
  .scripts["fn-knock:spk:build"] | contains("build-all-packages.sh")
' "${ROOT_DIR}/package.json" >/dev/null || fail "default SPK command must build all architectures"
for synology_arch in x86_64 armv8 armv7; do
  jq -e --arg command "fn-knock:spk:build:${synology_arch}" --arg arch "${synology_arch}" '
    .scripts[$command] | contains("build-package.sh " + $arch)
  ' "${ROOT_DIR}/package.json" >/dev/null || fail "missing local command for ${synology_arch}"
done
grep -Fq 'build-all-packages.sh' "${ROOT_DIR}/scripts/fn-knock-assemble-release.sh" || \
  fail "release assembly must build all Synology architectures"
grep -Fq 'run: bash ./scripts/fn-knock-assemble-release.sh' "${ROOT_DIR}/.github/workflows/release.yml" || \
  fail "release workflow must invoke the release assembly script"

if PATH="${FAKE_BIN}:${PATH}" \
  EXPECTED_FILE_DESCRIPTION='ELF 64-bit LSB executable, x86-64, statically linked' \
  FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE=1 \
  FN_KNOCK_SYNOLOGY_GATEWAY_BIN="${WORK_DIR}/go-reauth-proxy-linux-arm64" \
  FN_KNOCK_PREPARED_RUNTIME_DIR="${RUNTIME_DIR}" \
  FN_KNOCK_PREPARED_MUSL_RUST_BACKEND_DIR="${RUST_DIR}" \
  FN_KNOCK_SYNOLOGY_OUTPUT="${WORK_DIR}/mismatched-armv8.spk" \
    bash "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh" armv8 >/dev/null 2>&1
then
  fail "Synology builder accepted x86 binaries for armv8"
fi

if grep -Eq 'tar[[:space:]]+-t(z)?f[^|]*[|][[:space:]]*grep .*-[A-Za-z]*q' \
  "${ROOT_DIR}/apps/fn-knock-synology/scripts/build-package.sh"; then
  fail "Synology validation must not stream tar output into grep -q"
fi

printf '[test-synology-package-validation] SPK validation passed\n'
