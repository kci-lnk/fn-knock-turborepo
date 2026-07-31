#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-finalize-test.XXXXXX")"
ASSETS_DIR="${WORK_DIR}/assets"
WINDOWS_METADATA_DIR="${WORK_DIR}/windows-metadata"
COS_OUTPUT_DIR="${WORK_DIR}/cos-output"
VERSION="$(jq -r '.version' "${ROOT_DIR}/version.json")"
CONTROL_API_VERSION="$(bash "${ROOT_DIR}/scripts/control-api-version.sh")"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

fail() {
  printf '[test-release-finalize] ERROR: %s\n' "$*" >&2
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

run_finalize() {
  FN_KNOCK_VERSION="${VERSION}" \
  FN_KNOCK_RELEASE_TAG="v${VERSION}" \
  FN_KNOCK_SOURCE_COMMIT=1111111111111111111111111111111111111111 \
  FN_KNOCK_GO_SOURCE_COMMIT=2222222222222222222222222222222222222222 \
  FN_KNOCK_CONTROL_API_VERSION="${CONTROL_API_VERSION}" \
  FN_KNOCK_DOCKER_IMAGE=kcilnk/fn-knock \
  FN_KNOCK_DOCKER_DIGEST=sha256:3333333333333333333333333333333333333333333333333333333333333333 \
  FN_KNOCK_REQUIRE_DOCKER=1 \
    node "${ROOT_DIR}/scripts/fn-knock-release-finalize.mjs" "${ASSETS_DIR}"
}

mkdir -p "${ASSETS_DIR}"
for name in \
  "fn-knock-${VERSION}-fnos-amd64.fpk" \
  "fn-knock-${VERSION}-fnos-arm64.fpk" \
  "fn-knock-linux-${VERSION}-amd64.tar.gz" \
  "fn-knock-linux-${VERSION}-amd64.tar.gz.sha256" \
  "fn-knock-linux-${VERSION}-arm64.tar.gz" \
  "fn-knock-linux-${VERSION}-arm64.tar.gz.sha256" \
  "fn-knock-linux-${VERSION}-arm.tar.gz" \
  "fn-knock-linux-${VERSION}-arm.tar.gz.sha256" \
  "fn-knock_${VERSION}-1_aarch64_cortex-a53.ipk" \
  "fn-knock_${VERSION}-r1_aarch64_cortex-a53.apk" \
  "fn-knock_${VERSION}-1_aarch64_generic.ipk" \
  "fn-knock_${VERSION}-r1_aarch64_generic.apk" \
  "fn-knock_${VERSION}-1_arm_cortex-a7_neon-vfpv4.ipk" \
  "fn-knock_${VERSION}-r1_arm_cortex-a7_neon-vfpv4.apk" \
  "fn-knock_${VERSION}-1_arm_cortex-a5_vfpv4.ipk" \
  "fn-knock_${VERSION}-r1_arm_cortex-a5_vfpv4.apk" \
  "fn-knock_${VERSION}-1_x86_64.ipk" \
  "fn-knock_${VERSION}-r1_x86_64.apk" \
  "app-meta-fn-knock_${VERSION}-r1_all.ipk" \
  "app-meta-fn-knock-${VERSION}-r1.apk" \
  "fn-knock-synology-x86_64-${VERSION}-0017.spk" \
  "fn-knock-synology-x86_64-${VERSION}-0017.spk.sha256" \
  "fn-knock-synology-armv8-${VERSION}-0017.spk" \
  "fn-knock-synology-armv8-${VERSION}-0017.spk.sha256" \
  "fn-knock-synology-armv7-${VERSION}-0017.spk" \
  "fn-knock-synology-armv7-${VERSION}-0017.spk.sha256" \
  "fn-knock-${VERSION}-windows-x86_64-unsigned-setup.exe" \
  "fn-knock-${VERSION}-windows-x86_64-unsigned-setup.exe.sha256" \
  "fn-knock-${VERSION}-windows-x86_64-unsigned-release.json" \
  "fn-knock-${VERSION}-windows-x86_64-unsigned-updater.json"
do
  printf 'fixture:%s\n' "${name}" > "${ASSETS_DIR}/${name}"
done

mkdir -p "${WINDOWS_METADATA_DIR}"
WINDOWS_SETUP="fn-knock-${VERSION}-windows-x86_64-unsigned-setup.exe"
WINDOWS_SHA256="$(sha256sum "${ASSETS_DIR}/${WINDOWS_SETUP}" | awk '{print $1}')"
printf '%s  %s\n' "${WINDOWS_SHA256}" "${WINDOWS_SETUP}" \
  > "${WINDOWS_METADATA_DIR}/${WINDOWS_SETUP}.sha256"
printf '{"version":"%s","runtime_target":"windows","architecture":"x86_64","published_at":"2026-07-22T00:00:00.000Z"}\n' \
  "${VERSION}" \
  > "${WINDOWS_METADATA_DIR}/fn-knock-${VERSION}-windows-x86_64-unsigned-release.json"
printf '{"version":"%s","pub_date":"2026-07-22T00:00:00.000Z"}\n' \
  "${VERSION}" \
  > "${WINDOWS_METADATA_DIR}/fn-knock-${VERSION}-windows-x86_64-unsigned-updater.json"
printf '#!/bin/sh\necho install\n' > "${WORK_DIR}/install.sh"
printf '# fn-knock %s\n\n- Integration fixture\n' "${VERSION}" \
  > "${WORK_DIR}/release-notes.md"
printf '[]\n' > "${WORK_DIR}/release-history.json"

run_finalize >/dev/null
jq -e \
  --arg version "${VERSION}" \
  --argjson control_api_version "${CONTROL_API_VERSION}" \
  '
    .schema_version == 1 and
    .version == $version and
    .tag == ("v" + $version) and
    .control_api_version == $control_api_version and
    (.artifacts | length) == 21 and
    ([.artifacts[].name | endswith(".sha256") or endswith(".json")] | any | not) and
    ([.artifacts[].name | select(startswith("app-meta-"))] | length) == 2 and
    ([.artifacts[] | select(.platform == "openwrt" and (.name | endswith(".ipk"))) | .architecture] | sort) == ["aarch64_cortex-a53", "aarch64_generic", "all", "arm_cortex-a5_vfpv4", "arm_cortex-a7_neon-vfpv4", "x86_64"] and
    ([.artifacts[] | select(.platform == "openwrt" and (.name | endswith(".apk"))) | .architecture] | sort) == ["aarch64_cortex-a53", "aarch64_generic", "all", "arm_cortex-a5_vfpv4", "arm_cortex-a7_neon-vfpv4", "x86_64"] and
    ([.artifacts[] | select(.platform == "synology") | .architecture] | sort) == ["armv7", "armv8", "x86_64"] and
    .metadata_files == ["release-manifest.json", "SHA256SUMS"] and
    .docker.published == true and
    .docker.reference == ("kcilnk/fn-knock:" + $version) and
    .docker.platforms == ["linux/amd64", "linux/arm64", "linux/arm/v7"]
  ' \
  "${ASSETS_DIR}/release-manifest.json" >/dev/null
[ "$(wc -l < "${ASSETS_DIR}/SHA256SUMS" | tr -d ' ')" = "22" ] || \
  fail "SHA256SUMS does not cover 21 public deliverables and release-manifest.json"
if find "${ASSETS_DIR}" -maxdepth 1 -type f \
  \( -name '*.sha256' -o \( -name '*.json' ! -name 'release-manifest.json' \) \) |
    grep -q .
then
  fail "per-artifact metadata files remain in the public release directory"
fi

COS_PUBLICBASICURL=https://cdn.example.test \
FN_KNOCK_COS_OUTPUT_DIR="${COS_OUTPUT_DIR}" \
FN_KNOCK_INSTALL_SCRIPT="${WORK_DIR}/install.sh" \
FN_KNOCK_RELEASE_ASSETS_DIR="${ASSETS_DIR}" \
FN_KNOCK_RELEASE_HISTORY_FILE="${WORK_DIR}/release-history.json" \
FN_KNOCK_RELEASE_NOTES_PATH="${WORK_DIR}/release-notes.md" \
FN_KNOCK_VERSION="${VERSION}" \
FN_KNOCK_WINDOWS_METADATA_DIR="${WINDOWS_METADATA_DIR}" \
  node "${ROOT_DIR}/scripts/fn-knock-cos-publish.mjs" plan >/dev/null
jq -e \
  '
    (.version_objects | length) == 24 and
    (.mutable_objects | length) == 6
  ' \
  "${COS_OUTPUT_DIR}/publish-plan.json" >/dev/null
jq -e \
  '
    (.packages.ipk | keys | sort) == ["aarch64_cortex-a53", "aarch64_generic", "all", "arm_cortex-a5_vfpv4", "arm_cortex-a7_neon-vfpv4", "x86_64"] and
    (.packages.apk | keys | sort) == ["aarch64_cortex-a53", "aarch64_generic", "all", "arm_cortex-a5_vfpv4", "arm_cortex-a7_neon-vfpv4", "x86_64"]
  ' \
  "${COS_OUTPUT_DIR}/latest.json" >/dev/null

run_finalize >/dev/null

printf 'unexpected\n' > "${ASSETS_DIR}/unexpected.bin"
expect_failure "exactly 21 deliverables" run_finalize

printf '[test-release-finalize] all inventory tests passed\n'
