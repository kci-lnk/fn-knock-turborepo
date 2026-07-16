#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
APP_DIR="${ROOT_DIR}/apps/fn-knock-synology"
DIST_DIR="${ROOT_DIR}/dist/synology"
LEGACY_DIST_DIR="${APP_DIR}/dist"
ARTIFACTS_DIR="${FN_KNOCK_ARTIFACTS_DIR:-${ROOT_DIR}/dist/fn-knock-artifacts}"
RUNTIME_DIR="${FN_KNOCK_PREPARED_RUNTIME_DIR:-${ARTIFACTS_DIR}/runtime}"
MUSL_RUST_DIR="${FN_KNOCK_PREPARED_MUSL_RUST_BACKEND_DIR:-${ARTIFACTS_DIR}/musl-rust-backends}"
PACKAGE_NAME="fn-knock-synology"
PRODUCT_VERSION="$(jq -er '.version' "${ROOT_DIR}/version.json")"
BUILD_NUMBER="${FN_KNOCK_SYNOLOGY_BUILD_NUMBER:-0017}"
PACKAGE_VERSION="${PRODUCT_VERSION}-${BUILD_NUMBER}"
OUTPUT_PATH="${FN_KNOCK_SYNOLOGY_OUTPUT:-${DIST_DIR}/${PACKAGE_NAME}-x86_64-${PACKAGE_VERSION}.spk}"
BUILD_WORK_DIR=""
GATEWAY_ARTIFACT=""

log() {
  printf '[fn-knock-synology] %s\n' "$*"
}

fail() {
  printf '[fn-knock-synology] ERROR: %s\n' "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

cleanup() {
  if [ -n "${BUILD_WORK_DIR}" ]; then
    rm -rf "${BUILD_WORK_DIR}"
  fi
}

prepare_artifacts() {
  if [ "${FN_KNOCK_SYNOLOGY_SKIP_ARTIFACT_PREPARE:-0}" = "1" ]; then
    log "using existing prepared artifacts"
    return
  fi

  log "preparing amd64 runtime artifacts"
  FN_KNOCK_MUSL_ARCHES=amd64 \
  FN_KNOCK_RUNTIME_GATEWAY_ARCHES=amd64 \
  FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD="${FN_KNOCK_GO_REAUTH_PROXY_FORCE_BUILD:-1}" \
    bash "${ROOT_DIR}/scripts/fn-knock-prepare-artifacts.sh" openwrt
}

build_gateway_artifact() {
  local gateway_dir="${FN_KNOCK_GO_REAUTH_PROXY_DIR:-${ROOT_DIR}/../Go-Reauth-Proxy}"
  local commit="unknown"

  [ -d "${gateway_dir}" ] || fail "missing Go-Reauth-Proxy checkout: ${gateway_dir}"
  if git -C "${gateway_dir}" rev-parse --short HEAD >/dev/null 2>&1; then
    commit="$(git -C "${gateway_dir}" rev-parse --short HEAD)"
  fi

  log "building Synology gateway ${PRODUCT_VERSION} (${commit})"
  mkdir -p "$(dirname "${GATEWAY_ARTIFACT}")"
  (
    cd "${gateway_dir}"
    CGO_ENABLED=0 GOOS=linux GOARCH=amd64 \
      go build \
        -ldflags="-s -w -X go-reauth-proxy/pkg/version.Version=${PRODUCT_VERSION} -X go-reauth-proxy/pkg/version.Commit=${commit}" \
        -trimpath \
        -o "${GATEWAY_ARTIFACT}" \
        ./cmd/server
  )
  chmod +x "${GATEWAY_ARTIFACT}"
}

validate_artifacts() {
  local backend="${MUSL_RUST_DIR}/server-admin-rs-linux-amd64"
  local file_info

  [ -x "${GATEWAY_ARTIFACT}" ] || fail "missing gateway artifact: ${GATEWAY_ARTIFACT}"
  [ -x "${backend}" ] || fail "missing Rust backend artifact: ${backend}"
  [ -d "${RUNTIME_DIR}/ui/www" ] || fail "missing admin UI artifacts"
  [ -d "${RUNTIME_DIR}/server-auth-view/dist" ] || fail "missing auth UI artifacts"
  [ -f "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" ] || fail "missing ACME bundle"

  file_info="$(file -b "${GATEWAY_ARTIFACT}")"
  printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*x86-64' || \
    fail "gateway is not a Linux x86-64 ELF: ${file_info}"
  file_info="$(file -b "${backend}")"
  printf '%s\n' "${file_info}" | grep -Eq 'ELF 64-bit LSB.*x86-64' || \
    fail "backend is not a Linux x86-64 ELF: ${file_info}"
}

clean_old_packages() {
  local output_dir

  output_dir="$(dirname "${OUTPUT_PATH}")"
  mkdir -p "${output_dir}"
  rm -f \
    "${output_dir}/${PACKAGE_NAME}-"*.spk \
    "${output_dir}/${PACKAGE_NAME}-"*.spk.sha256 \
    "${output_dir}/${PACKAGE_NAME}-"*.spk.tmp \
    "${OUTPUT_PATH}" \
    "${OUTPUT_PATH}.sha256" \
    "${OUTPUT_PATH}.tmp"

  if [ "${LEGACY_DIST_DIR}" != "${output_dir}" ]; then
    rm -f \
      "${LEGACY_DIST_DIR}/${PACKAGE_NAME}-"*.spk \
      "${LEGACY_DIST_DIR}/${PACKAGE_NAME}-"*.spk.sha256 \
      "${LEGACY_DIST_DIR}/${PACKAGE_NAME}-"*.spk.tmp
  fi
}

write_info() {
  local path="$1"
  local extract_size="$2"

  cat > "${path}" <<EOF
package="${PACKAGE_NAME}"
version="${PACKAGE_VERSION}"
os_min_ver="7.0-40000"
arch="x86_64"
maintainer="fn-knock"
maintainer_url="https://www.fnknock.cn/synology"
distributor="fn-knock"
distributor_url="https://www.fnknock.cn/synology"
support_url="https://www.fnknock.cn/synology"
helpurl="https://docs.fnknock.cn/"
displayname="敲门 knock"
description="fn-knock is a self-hosted secure access gateway for Synology DSM. It provides authenticated reverse proxy access, zero-trust policies, active threat protection, DDNS, certificate management, and end-to-end observability for NAS services. Official website: https://www.fnknock.cn/synology"
description_enu="fn-knock is a self-hosted secure access gateway for Synology DSM. It provides authenticated reverse proxy access, zero-trust policies, active threat protection, DDNS, certificate management, and end-to-end observability for NAS services. Official website: https://www.fnknock.cn/synology"
description_chs="敲门 knock 是面向 Synology DSM 的自托管安全访问网关，为 NAS 服务提供鉴权反向代理、零信任访问策略、主动威胁防护、DDNS、证书管理与全链路观测，并通过 DSM 桌面安全管理。官方网站：https://www.fnknock.cn/synology"
description_cht="敲門 knock 是面向 Synology DSM 的自託管安全存取閘道，為 NAS 服務提供驗證反向代理、零信任存取策略、主動威脅防護、DDNS、憑證管理與全鏈路觀測，並透過 DSM 桌面安全管理。官方網站：https://www.fnknock.cn/synology"
thirdparty="yes"
dsmuidir="ui"
dsmappname="fn-knock-synology.Application"
dsmapplaunchname="fn-knock-synology.Application"
ctl_stop="yes"
precheckstartstop="yes"
silent_install="yes"
silent_upgrade="yes"
silent_uninstall="yes"
extractsize="${extract_size}"
EOF
}

build_package() {
  local work_dir
  local payload_dir
  local spk_root
  local package_tgz
  local temp_output
  local extract_size
  local checksum
  local -a owner_args
  local -a lifecycle_scripts
  local script_name
  local icon_size

  work_dir="${BUILD_WORK_DIR}/package"
  payload_dir="${work_dir}/payload"
  spk_root="${work_dir}/spk"
  package_tgz="${spk_root}/package.tgz"
  temp_output="${OUTPUT_PATH}.tmp"
  mkdir -p \
    "${payload_dir}/bin" \
    "${payload_dir}/server/server-admin/resources" \
    "${payload_dir}/server-auth-view/dist" \
    "${payload_dir}/ui/www" \
    "${payload_dir}/ui/images" \
    "${spk_root}/scripts" \
    "${spk_root}/conf" \
    "$(dirname "${OUTPUT_PATH}")"

  rsync -a "${APP_DIR}/package/" "${payload_dir}/"
  rsync -a --delete "${RUNTIME_DIR}/ui/www/" "${payload_dir}/ui/www/"
  rsync -a --delete "${RUNTIME_DIR}/server-auth-view/dist/" "${payload_dir}/server-auth-view/dist/"
  cp "${RUNTIME_DIR}/server/server-admin/resources/acmesh.zip" \
    "${payload_dir}/server/server-admin/resources/acmesh.zip"
  cp "${GATEWAY_ARTIFACT}" "${payload_dir}/bin/go-reauth-proxy"
  cp "${MUSL_RUST_DIR}/server-admin-rs-linux-amd64" "${payload_dir}/bin/server-admin-rs"

  cp "${ROOT_DIR}/apps/fn-knock/ICON_64.PNG" "${spk_root}/PACKAGE_ICON.PNG"
  cp "${ROOT_DIR}/apps/fn-knock/ICON_256.PNG" "${spk_root}/PACKAGE_ICON_256.PNG"
  cp "${ROOT_DIR}/apps/fn-knock/ICON.PNG" "${payload_dir}/ui/images/icon.png"
  for icon_size in 16 24 32 48 64 72; do
    cp "${ROOT_DIR}/apps/fn-knock/ICON_64.PNG" "${payload_dir}/ui/images/icon_${icon_size}.png"
  done
  cp "${ROOT_DIR}/apps/fn-knock/ICON_256.PNG" "${payload_dir}/ui/images/icon_256.png"

  lifecycle_scripts=(
    start-stop-status
    preinst
    postinst
    preuninst
    postuninst
    preupgrade
    postupgrade
  )
  for script_name in "${lifecycle_scripts[@]}"; do
    cp "${APP_DIR}/scripts/${script_name}" "${spk_root}/scripts/${script_name}"
  done
  rsync -a "${APP_DIR}/conf/" "${spk_root}/conf/"

  chmod 755 \
    "${payload_dir}/bin/fn-knock-entrypoint" \
    "${payload_dir}/bin/go-reauth-proxy" \
    "${payload_dir}/bin/server-admin-rs" \
    "${payload_dir}/ui/index.cgi" \
    "${spk_root}/scripts/"*
  chmod 644 "${spk_root}/conf/privilege" "${spk_root}/conf/resource"

  jq -e . "${payload_dir}/ui/config" "${spk_root}/conf/privilege" "${spk_root}/conf/resource" >/dev/null
  node --check "${payload_dir}/ui/launch.js"
  sh -n "${payload_dir}/ui/index.cgi" "${spk_root}/scripts/"*
  bash -n "${payload_dir}/bin/fn-knock-entrypoint"

  find "${payload_dir}" -depth -exec touch -t 197001010000 {} +
  if tar --version 2>/dev/null | grep -qi bsdtar; then
    owner_args=(--uid 0 --gid 0 --uname root --gname root)
  else
    owner_args=(--owner=0 --group=0 --numeric-owner --sort=name)
  fi
  COPYFILE_DISABLE=1 tar "${owner_args[@]}" -cf - -C "${payload_dir}" . | gzip -n -9 > "${package_tgz}"
  extract_size="$(du -sk "${payload_dir}" | awk '{print $1}')"
  write_info "${spk_root}/INFO" "${extract_size}"

  find "${spk_root}" -depth -exec touch -t 197001010000 {} +
  rm -f "${temp_output}"
  COPYFILE_DISABLE=1 tar "${owner_args[@]}" -cf "${temp_output}" \
    -C "${spk_root}" \
    INFO PACKAGE_ICON.PNG PACKAGE_ICON_256.PNG package.tgz scripts conf
  mv "${temp_output}" "${OUTPUT_PATH}"

  tar -tf "${OUTPUT_PATH}" | grep -qx 'INFO' || fail "SPK is missing INFO"
  tar -tf "${OUTPUT_PATH}" | grep -qx 'package.tgz' || fail "SPK is missing package.tgz"
  tar -tzf "${package_tgz}" | grep -Eq '^\./bin/server-admin-rs$' || fail "payload is missing backend"
  tar -tzf "${package_tgz}" | grep -Eq '^\./ui/index.cgi$' || fail "payload is missing DSM CGI"
  tar -tzf "${package_tgz}" | grep -Eq '^\./ui/launch.html$' || fail "payload is missing DSM launcher"
  tar -tzf "${package_tgz}" | grep -Eq '^\./ui/launch.js$' || fail "payload is missing DSM launcher script"

  checksum="$(shasum -a 256 "${OUTPUT_PATH}" | awk '{print $1}')"
  printf '%s  %s\n' "${checksum}" "$(basename "${OUTPUT_PATH}")" > "${OUTPUT_PATH}.sha256"
  log "built ${OUTPUT_PATH}"
  log "sha256 ${checksum}"
}

require_cmd jq
require_cmd node
require_cmd rsync
require_cmd file
require_cmd tar
require_cmd gzip
require_cmd shasum
require_cmd go
BUILD_WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/fn-knock-synology.XXXXXX")"
GATEWAY_ARTIFACT="${BUILD_WORK_DIR}/gateway/go-reauth-proxy-linux-amd64"
trap cleanup EXIT
prepare_artifacts
build_gateway_artifact
validate_artifacts
clean_old_packages
build_package
