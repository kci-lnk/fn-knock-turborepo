#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

APP_NAME="${FN_KNOCK_OPENWRT_PACKAGE_NAME:-fn-knock}"
PACKAGE_RELEASE="${FN_KNOCK_OPENWRT_RELEASE:-1}"
OUTPUT_DIR="${FN_KNOCK_OPENWRT_OUTPUT_DIR:-${ROOT_DIR}/dist/openwrt}"
case "${OUTPUT_DIR}" in
  /*) ;;
  *) OUTPUT_DIR="${ROOT_DIR}/${OUTPUT_DIR}" ;;
esac
WORK_DIR="${OUTPUT_DIR}/work"
RUNTIME_DIR="${OUTPUT_DIR}/runtime"
TEMPLATE_DIR="${ROOT_DIR}/deploy/openwrt"
VERSION_FILE="${ROOT_DIR}/apps/server-admin/src/lib/app-version.ts"
DEFAULT_ARCH_MATRIX="aarch64_cortex-a53:arm64,aarch64_generic:arm64,arm_cortex-a7_neon-vfpv4:arm,arm_cortex-a5_vfpv4:arm,x86_64:amd64"
ARCH_MATRIX="${FN_KNOCK_OPENWRT_ARCHES:-${DEFAULT_ARCH_MATRIX}}"
PACKAGE_FORMATS_RAW="${FN_KNOCK_OPENWRT_FORMATS:-ipk,apk}"
DEPENDS="${FN_KNOCK_OPENWRT_DEPENDS:-libc, node, redis-server, bash, curl, unzip, ca-bundle, ca-certificates, iptables-nft, ip6tables-nft, luci-base}"
DESCRIPTION="${FN_KNOCK_OPENWRT_DESCRIPTION:-fn-knock secure reverse proxy and knock authentication gateway}"
HOMEPAGE="${FN_KNOCK_OPENWRT_HOMEPAGE:-https://github.com/kci-lnk/fn-knock}"
LICENSE="${FN_KNOCK_OPENWRT_LICENSE:-MIT}"
IPK_CONTAINER_FORMAT="${FN_KNOCK_OPENWRT_IPK_FORMAT:-tar}"
APK_DOCKER_IMAGE="${FN_KNOCK_OPENWRT_APK_DOCKER_IMAGE:-alpine:3.23}"

log() {
  echo "[fn-knock-openwrt] $*"
}

fail() {
  echo "[fn-knock-openwrt] ERROR: $*" >&2
  exit 1
}

require_cmd() {
  local cmd="$1"
  command -v "${cmd}" >/dev/null 2>&1 || fail "missing required command: ${cmd}"
}

clean_output_dir() {
  local normalized_output_dir="${OUTPUT_DIR%/}"

  case "${normalized_output_dir}" in
    ""|"/"|"."|".."|"${ROOT_DIR}")
      fail "refusing to clean unsafe output directory: ${OUTPUT_DIR}"
      ;;
  esac

  log "Cleaning output directory ${OUTPUT_DIR}"
  rm -rf "${OUTPUT_DIR}"
  mkdir -p "${OUTPUT_DIR}" "${WORK_DIR}"
}

parse_app_version() {
  local version
  [ -f "${VERSION_FILE}" ] || fail "missing version file: ${VERSION_FILE}"
  version="$(sed -nE 's/^[[:space:]]*export[[:space:]]+const[[:space:]]+APP_LOCAL_VERSION[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "${VERSION_FILE}" | head -n1)"
  [ -n "${version}" ] || fail "failed to parse APP_LOCAL_VERSION from ${VERSION_FILE}"
  printf '%s\n' "${version}"
}

normalize_tar_listing() {
  sed -e 's#^\./##' -e '/^$/d'
}

read_arch_matrix() {
  local raw_matrix="$1"
  local old_ifs="${IFS}"
  local item

  IFS=","
  for item in ${raw_matrix}; do
    item="$(printf '%s' "${item}" | xargs)"
    [ -n "${item}" ] || continue
    case "${item}" in
      *:*)
        printf '%s\n' "${item}"
        ;;
      *)
        fail "invalid architecture matrix item: ${item}; expected openwrt_arch:gateway_arch"
        ;;
    esac
  done
  IFS="${old_ifs}"
}

read_package_formats() {
  local raw_formats="$1"
  local old_ifs="${IFS}"
  local item

  IFS=","
  for item in ${raw_formats}; do
    item="$(printf '%s' "${item}" | xargs | tr '[:upper:]' '[:lower:]')"
    [ -n "${item}" ] || continue
    case "${item}" in
      ipk|apk)
        printf '%s\n' "${item}"
        ;;
      *)
        fail "invalid OpenWrt package format: ${item}; expected ipk or apk"
        ;;
    esac
  done
  IFS="${old_ifs}"
}

format_enabled() {
  local needle="$1"
  shift
  local item

  for item in "$@"; do
    [ "${item}" = "${needle}" ] && return 0
  done

  return 1
}

collect_gateway_arches() {
  local matrix_items=("$@")
  local item
  local gateway_arch
  local seen=" "

  for item in "${matrix_items[@]}"; do
    gateway_arch="${item#*:}"
    case " ${seen} " in
      *" ${gateway_arch} "*)
        ;;
      *)
        printf '%s\n' "${gateway_arch}"
        seen="${seen}${gateway_arch} "
        ;;
    esac
  done
}

assert_gateway_arch() {
  case "$1" in
    amd64|arm64|arm)
      return 0
      ;;
    *)
      fail "unsupported gateway architecture: $1"
      ;;
  esac
}

prepare_runtime() {
  local gateway_arches=("$@")

  log "Assembling shared runtime"
  rm -rf "${RUNTIME_DIR}"
  bash "${ROOT_DIR}/scripts/assemble-runtime.sh" "${RUNTIME_DIR}"

  log "Preparing gateway binaries: ${gateway_arches[*]}"
  bash "${ROOT_DIR}/scripts/prepare-go-reauth-proxy.sh" "${RUNTIME_DIR}/server" "${gateway_arches[@]}"
}

ensure_apk_tooling() {
  require_cmd docker

  if ! docker image inspect "${APK_DOCKER_IMAGE}" >/dev/null 2>&1; then
    log "Pulling APK tooling image ${APK_DOCKER_IMAGE}"
    docker pull "${APK_DOCKER_IMAGE}" >/dev/null
  fi

  docker run --rm "${APK_DOCKER_IMAGE}" apk --version >/dev/null || \
    fail "APK tooling image does not provide apk: ${APK_DOCKER_IMAGE}"
}

write_control_files() {
  local control_dir="$1"
  local openwrt_arch="$2"
  local version="$3"
  local installed_size="$4"

  mkdir -p "${control_dir}"
  cat > "${control_dir}/control" <<EOF
Package: ${APP_NAME}
Version: ${version}-${PACKAGE_RELEASE}
Architecture: ${openwrt_arch}
Maintainer: kci-lnk <https://github.com/kci-lnk>
Section: net
Priority: optional
Depends: ${DEPENDS}
Homepage: ${HOMEPAGE}
License: ${LICENSE}
Installed-Size: ${installed_size}
Description: ${DESCRIPTION}
 fn-knock packages the admin UI, auth UI, Node backend, Go gateway, and LuCI
 launcher/configuration page for OpenWrt.
EOF

  printf '%s\n' "/etc/config/fn-knock" > "${control_dir}/conffiles"
  rsync -a "${TEMPLATE_DIR}/control/" "${control_dir}/"
  chmod 755 "${control_dir}/postinst" "${control_dir}/prerm" "${control_dir}/postrm"
}

copy_runtime_payload() {
  local data_dir="$1"
  local gateway_arch="$2"
  local app_root="${data_dir}/usr/lib/fn-knock"

  assert_gateway_arch "${gateway_arch}"

  mkdir -p \
    "${app_root}/ui/www" \
    "${app_root}/server-auth-view/dist" \
    "${app_root}/server/server-admin" \
    "${app_root}/server" \
    "${app_root}/bin"

  rsync -a --delete "${RUNTIME_DIR}/ui/www/" "${app_root}/ui/www/"
  rsync -a --delete "${RUNTIME_DIR}/server-auth-view/dist/" "${app_root}/server-auth-view/dist/"
  rsync -a --delete "${RUNTIME_DIR}/server/server-admin/" "${app_root}/server/server-admin/"

  local gateway_src="${RUNTIME_DIR}/server/go-reauth-proxy-linux-${gateway_arch}"
  local gateway_dst="${app_root}/server/go-reauth-proxy-linux-${gateway_arch}"
  [ -f "${gateway_src}" ] || fail "missing gateway binary: ${gateway_src}"
  cp "${gateway_src}" "${gateway_dst}"
  chmod 755 "${gateway_dst}"
  ln -s "../server/go-reauth-proxy-linux-${gateway_arch}" "${app_root}/bin/go-reauth-proxy"

  rsync -a "${TEMPLATE_DIR}/etc/" "${data_dir}/etc/"
  rsync -a "${TEMPLATE_DIR}/usr/" "${data_dir}/usr/"
  rsync -a "${TEMPLATE_DIR}/www/" "${data_dir}/www/"
  chmod 755 \
    "${data_dir}/etc/init.d/fn-knock" \
    "${data_dir}/usr/bin/fn-knock-reset-panel-password"
}

create_tarball() {
  local source_dir="$1"
  local output_file="$2"

  tar \
    --format=ustar \
    --uid 0 \
    --gid 0 \
    --uname root \
    --gname root \
    -czf "${output_file}" \
    -C "${source_dir}" \
    .
}

validate_control_metadata() {
  local control_tar="$1"
  local openwrt_arch="$2"
  local version="$3"
  local control_text

  control_text="$(tar -xOzf "${control_tar}" ./control)"
  printf '%s\n' "${control_text}" | grep -Fxq "Package: ${APP_NAME}" || \
    fail "control metadata missing package name"
  printf '%s\n' "${control_text}" | grep -Fxq "Version: ${version}-${PACKAGE_RELEASE}" || \
    fail "control metadata missing version"
  printf '%s\n' "${control_text}" | grep -Fxq "Architecture: ${openwrt_arch}" || \
    fail "control metadata missing architecture ${openwrt_arch}"
  printf '%s\n' "${control_text}" | grep -Fxq "Depends: ${DEPENDS}" || \
    fail "control metadata missing dependency list"
  printf '%s\n' "${control_text}" | grep -Fxq "Description: ${DESCRIPTION}" || \
    fail "control metadata missing package description"
  printf '%s\n' "${control_text}" | grep -Fxq "Homepage: ${HOMEPAGE}" || \
    fail "control metadata missing homepage"
  printf '%s\n' "${control_text}" | grep -Fxq "License: ${LICENSE}" || \
    fail "control metadata missing license"
}

validate_root_ownership() {
  local tar_file="$1"
  local bad_entries

  bad_entries="$(tar -tvzf "${tar_file}" | awk '$3 != "root" || $4 != "root" { print }')"
  if [ -n "${bad_entries}" ]; then
    printf '%s\n' "${bad_entries}" >&2
    fail "tarball contains non-root-owned entries: ${tar_file}"
  fi
}

validate_payload_listing() {
  local listing="$1"
  local gateway_arch="$2"
  local gateway_listing

  printf '%s\n' "${listing}" | grep -Fxq "etc/config/fn-knock" || \
    fail "data payload missing /etc/config/fn-knock"
  printf '%s\n' "${listing}" | grep -Fxq "etc/init.d/fn-knock" || \
    fail "data payload missing /etc/init.d/fn-knock"
  printf '%s\n' "${listing}" | grep -Fxq "usr/lib/fn-knock/server/server-admin/index.js" || \
    fail "data payload missing server-admin index.js"
  printf '%s\n' "${listing}" | grep -Fxq "usr/lib/fn-knock/server/server-admin/package.json" || \
    fail "data payload missing server-admin package.json"
  printf '%s\n' "${listing}" | grep -Fxq "usr/lib/fn-knock/ui/www/index.html" || \
    fail "data payload missing admin UI index.html"
  printf '%s\n' "${listing}" | grep -Fxq "usr/lib/fn-knock/server/go-reauth-proxy-linux-${gateway_arch}" || \
    fail "data payload missing selected gateway binary"
  printf '%s\n' "${listing}" | grep -Fxq "usr/share/luci/menu.d/luci-app-fn-knock.json" || \
    fail "data payload missing LuCI menu"
  printf '%s\n' "${listing}" | grep -Fxq "usr/share/rpcd/acl.d/luci-app-fn-knock.json" || \
    fail "data payload missing LuCI ACL"
  printf '%s\n' "${listing}" | grep -Fxq "www/luci-static/resources/view/fn-knock.js" || \
    fail "data payload missing LuCI view"
  printf '%s\n' "${listing}" | grep -Fxq "www/luci-static/resources/view/fn-knock-openwrt.js" || \
    fail "data payload missing OpenWrt LuCI view"
  printf '%s\n' "${listing}" | grep -Fxq "www/luci-static/resources/fn-knock/fn-knock.png" || \
    fail "data payload missing LuCI icon"

  gateway_listing="$(printf '%s\n' "${listing}" | grep 'usr/lib/fn-knock/server/go-reauth-proxy-linux-' || true)"
  if [ "${gateway_listing}" != "usr/lib/fn-knock/server/go-reauth-proxy-linux-${gateway_arch}" ]; then
    printf '%s\n' "${gateway_listing}" >&2
    fail "data payload contains unexpected gateway binaries"
  fi
}

validate_extracted_payload() {
  local extract_dir="$1"
  local gateway_arch="$2"
  local listing

  listing="$(
    cd "${extract_dir}" && \
      find . \( -type f -o -type l \) | normalize_tar_listing | sort
  )"
  validate_payload_listing "${listing}" "${gateway_arch}"

  [ -x "${extract_dir}/usr/lib/fn-knock/server/go-reauth-proxy-linux-${gateway_arch}" ] || \
    fail "gateway binary is not executable"
  [ -x "${extract_dir}/usr/lib/fn-knock/bin/go-reauth-proxy" ] || \
    fail "gateway symlink is not executable"
  [ -x "${extract_dir}/etc/init.d/fn-knock" ] || \
    fail "init script is not executable"
  [ -x "${extract_dir}/usr/bin/fn-knock-reset-panel-password" ] || \
    fail "reset command is not executable"
}

validate_data_payload() {
  local data_tar="$1"
  local gateway_arch="$2"
  local extract_dir="$3"
  local listing

  listing="$(tar -tzf "${data_tar}" | normalize_tar_listing)"
  validate_payload_listing "${listing}" "${gateway_arch}"

  rm -rf "${extract_dir}"
  mkdir -p "${extract_dir}"
  tar -xzf "${data_tar}" -C "${extract_dir}"
  validate_extracted_payload "${extract_dir}" "${gateway_arch}"
}

append_ar_member() {
  local archive_path="$1"
  local member_name="$2"
  local source_file="$3"
  local size
  local ar_name

  size="$(wc -c < "${source_file}" | tr -d '[:space:]')"
  ar_name="${member_name}/"
  [ "${#ar_name}" -le 16 ] || fail "ar member name is too long: ${member_name}"

  printf '%-16s%-12s%-6s%-6s%-8s%-10s`\n' \
    "${ar_name}" \
    "0" \
    "0" \
    "0" \
    "100644" \
    "${size}" >> "${archive_path}"
  cat "${source_file}" >> "${archive_path}"
  if [ $((size % 2)) -eq 1 ]; then
    printf '\n' >> "${archive_path}"
  fi
}

create_ar_archive() {
  local output_file="$1"
  shift
  local item
  local member_name
  local source_file

  printf '!<arch>\n' > "${output_file}"
  for item in "$@"; do
    member_name="${item%%:*}"
    source_file="${item#*:}"
    [ -f "${source_file}" ] || fail "missing ar member source: ${source_file}"
    append_ar_member "${output_file}" "${member_name}" "${source_file}"
  done
}

create_tar_ipk_archive() {
  local package_work_dir="$1"
  local output_file="$2"

  tar \
    --format=ustar \
    --uid 0 \
    --gid 0 \
    --uname root \
    --gname root \
    -czf "${output_file}" \
    -C "${package_work_dir}" \
    ./debian-binary \
    ./data.tar.gz \
    ./control.tar.gz
}

validate_ar_ipk() {
  local ipk_path="$1"
  local control_tar="$2"
  local data_tar="$3"
  local openwrt_arch="$4"
  local gateway_arch="$5"
  local version="$6"
  local ar_listing
  local extract_dir

  ar_listing="$(ar -t "${ipk_path}" | sed 's#/$##')"
  [ "${ar_listing}" = $'debian-binary\ncontrol.tar.gz\ndata.tar.gz' ] || {
    printf '%s\n' "${ar_listing}" >&2
    fail "unexpected ar member order for ${ipk_path}"
  }

  validate_control_metadata "${control_tar}" "${openwrt_arch}" "${version}"
  validate_root_ownership "${control_tar}"
  validate_root_ownership "${data_tar}"

  extract_dir="$(mktemp -d "${WORK_DIR}/inspect.XXXXXX")"
  validate_data_payload "${data_tar}" "${gateway_arch}" "${extract_dir}"
  rm -rf "${extract_dir}"
}

validate_tar_ipk() {
  local ipk_path="$1"
  local control_tar="$2"
  local data_tar="$3"
  local openwrt_arch="$4"
  local gateway_arch="$5"
  local version="$6"
  local listing
  local extract_dir

  listing="$(tar -tzf "${ipk_path}" | normalize_tar_listing)"
  [ "${listing}" = $'debian-binary\ndata.tar.gz\ncontrol.tar.gz' ] || {
    printf '%s\n' "${listing}" >&2
    fail "unexpected tar ipk member order for ${ipk_path}"
  }

  validate_control_metadata "${control_tar}" "${openwrt_arch}" "${version}"
  validate_root_ownership "${control_tar}"
  validate_root_ownership "${data_tar}"
  validate_root_ownership "${ipk_path}"

  extract_dir="$(mktemp -d "${WORK_DIR}/inspect.XXXXXX")"
  validate_data_payload "${data_tar}" "${gateway_arch}" "${extract_dir}"
  rm -rf "${extract_dir}"
}

create_ipk_archive() {
  local package_work_dir="$1"
  local ipk_path="$2"
  local debian_binary="$3"
  local control_tar="$4"
  local data_tar="$5"

  case "${IPK_CONTAINER_FORMAT}" in
    tar|tar.gz|tgz)
      create_tar_ipk_archive "${package_work_dir}" "${ipk_path}"
      ;;
    ar)
      create_ar_archive \
        "${ipk_path}" \
        "debian-binary:${debian_binary}" \
        "control.tar.gz:${control_tar}" \
        "data.tar.gz:${data_tar}"
      ;;
    *)
      fail "unsupported IPK container format: ${IPK_CONTAINER_FORMAT}; expected tar or ar"
      ;;
  esac
}

validate_ipk() {
  local ipk_path="$1"
  local control_tar="$2"
  local data_tar="$3"
  local openwrt_arch="$4"
  local gateway_arch="$5"
  local version="$6"

  case "${IPK_CONTAINER_FORMAT}" in
    tar|tar.gz|tgz)
      validate_tar_ipk "${ipk_path}" "${control_tar}" "${data_tar}" "${openwrt_arch}" "${gateway_arch}" "${version}"
      ;;
    ar)
      validate_ar_ipk "${ipk_path}" "${control_tar}" "${data_tar}" "${openwrt_arch}" "${gateway_arch}" "${version}"
      ;;
  esac
}

compute_file_sha256() {
  local file_path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${file_path}" | awk '{ print $1 }'
  else
    shasum -a 256 "${file_path}" | awk '{ print $1 }'
  fi
}

apk_package_version() {
  local version="$1"

  case "${PACKAGE_RELEASE}" in
    r*)
      printf '%s-%s\n' "${version}" "${PACKAGE_RELEASE}"
      ;;
    *)
      printf '%s-r%s\n' "${version}" "${PACKAGE_RELEASE}"
      ;;
  esac
}

apk_package_depends() {
  printf '%s\n' "${DEPENDS}" | tr ',' ' ' | xargs
}

write_apk_lifecycle_script() {
  local output_file="$1"
  local source_file="$2"
  shift 2
  local line

  [ -f "${source_file}" ] || fail "missing APK lifecycle script source: ${source_file}"

  {
    printf '#!/bin/sh\n'
    for line in "$@"; do
      printf '%s\n' "${line}"
    done
    sed '1{/^[[:space:]]*#!/d;}' "${source_file}"
  } > "${output_file}"
  chmod 755 "${output_file}"
}

prepare_apk_lifecycle_scripts() {
  local control_dir="$1"
  local scripts_dir="$2"

  rm -rf "${scripts_dir}"
  mkdir -p "${scripts_dir}"

  write_apk_lifecycle_script \
    "${scripts_dir}/post-install" \
    "${control_dir}/postinst"
  write_apk_lifecycle_script \
    "${scripts_dir}/post-upgrade" \
    "${control_dir}/postinst" \
    "export PKG_UPGRADE=1"
  write_apk_lifecycle_script \
    "${scripts_dir}/pre-deinstall" \
    "${control_dir}/prerm"
  write_apk_lifecycle_script \
    "${scripts_dir}/post-deinstall" \
    "${control_dir}/postrm"
}

prepare_apk_metadata_files() {
  local data_dir="$1"
  local control_dir="$2"
  local metadata_dir="${data_dir}/lib/apk/packages"
  local conffiles_src="${control_dir}/conffiles"
  local conffiles_dst="${metadata_dir}/${APP_NAME}.conffiles"
  local conffiles_static_dst="${metadata_dir}/${APP_NAME}.conffiles_static"
  local list_dst="${metadata_dir}/${APP_NAME}.list"
  local config_file
  local normalized_file
  local full_path
  local checksum

  mkdir -p "${metadata_dir}"

  if [ -f "${conffiles_src}" ]; then
    cp "${conffiles_src}" "${conffiles_dst}"
    : > "${conffiles_static_dst}"

    while IFS= read -r config_file || [ -n "${config_file}" ]; do
      [ -n "${config_file}" ] || continue
      normalized_file="${config_file#/}"
      full_path="${data_dir}/${normalized_file}"
      [ -f "${full_path}" ] || continue
      checksum="$(compute_file_sha256 "${full_path}")"
      printf '%s %s\n' "${config_file}" "${checksum}" >> "${conffiles_static_dst}"
    done < "${conffiles_src}"
  fi

  (
    cd "${data_dir}"
    find . \( -type f -o -type l \) ! -path './lib/apk/packages/*' | \
      normalize_tar_listing | \
      sed 's#^#/#' | \
      sort
  ) > "${list_dst}"
}

create_apk_archive() {
  local data_dir="$1"
  local scripts_dir="$2"
  local apk_path="$3"
  local openwrt_arch="$4"
  local version="$5"
  local apk_file_name
  local apk_version
  local apk_depends

  apk_file_name="$(basename "${apk_path}")"
  apk_version="$(apk_package_version "${version}")"
  apk_depends="$(apk_package_depends)"

  docker run --rm \
    -e APK_PACKAGE_NAME="${APP_NAME}" \
    -e APK_PACKAGE_VERSION="${apk_version}" \
    -e APK_PACKAGE_ARCH="${openwrt_arch}" \
    -e APK_PACKAGE_DESCRIPTION="${DESCRIPTION}" \
    -e APK_PACKAGE_LICENSE="${LICENSE}" \
    -e APK_PACKAGE_URL="${HOMEPAGE}" \
    -e APK_PACKAGE_MAINTAINER="kci-lnk <https://github.com/kci-lnk>" \
    -e APK_PACKAGE_ORIGIN="fn-knock/openwrt" \
    -e APK_PACKAGE_DEPENDS="${apk_depends}" \
    -e APK_OUTPUT_NAME="${apk_file_name}" \
    -v "${data_dir}:/src:ro" \
    -v "${scripts_dir}:/scripts:ro" \
    -v "${OUTPUT_DIR}:/out" \
    "${APK_DOCKER_IMAGE}" \
    sh -eu -c '
      rm -rf /pkg
      mkdir -p /pkg
      cp -a /src/. /pkg/
      chown -R 0:0 /pkg
      apk mkpkg \
        --info "name:${APK_PACKAGE_NAME}" \
        --info "version:${APK_PACKAGE_VERSION}" \
        --info "description:${APK_PACKAGE_DESCRIPTION}" \
        --info "arch:${APK_PACKAGE_ARCH}" \
        --info "license:${APK_PACKAGE_LICENSE}" \
        --info "origin:${APK_PACKAGE_ORIGIN}" \
        --info "url:${APK_PACKAGE_URL}" \
        --info "maintainer:${APK_PACKAGE_MAINTAINER}" \
        --info "depends:${APK_PACKAGE_DEPENDS}" \
        --script "post-install:/scripts/post-install" \
        --script "post-upgrade:/scripts/post-upgrade" \
        --script "pre-deinstall:/scripts/pre-deinstall" \
        --script "post-deinstall:/scripts/post-deinstall" \
        --files /pkg \
        --output "/out/${APK_OUTPUT_NAME}"
    '
}

validate_apk() {
  local apk_path="$1"
  local gateway_arch="$2"
  local apk_dir
  local apk_file_name
  local extract_dir

  apk_dir="$(dirname "${apk_path}")"
  apk_file_name="$(basename "${apk_path}")"
  extract_dir="$(mktemp -d "${WORK_DIR}/apk-inspect.XXXXXX")"

  docker run --rm \
    -e APK_FILE_NAME="${apk_file_name}" \
    -v "${apk_dir}:/packages:ro" \
    -v "${extract_dir}:/inspect" \
    "${APK_DOCKER_IMAGE}" \
    sh -eu -c '
      apk extract --allow-untrusted --destination /inspect "/packages/${APK_FILE_NAME}" >/dev/null
      bad_entry="$(find /inspect \( ! -user root -o ! -group root \) -print -quit)"
      if [ -n "${bad_entry}" ]; then
        echo "APK extract contains non-root-owned entry: ${bad_entry}" >&2
        exit 1
      fi
    '

  validate_extracted_payload "${extract_dir}" "${gateway_arch}"
  rm -rf "${extract_dir}"
}

build_packages_for_arch() {
  local item="$1"
  local version="$2"
  shift 2
  local package_formats=("$@")
  local openwrt_arch="${item%%:*}"
  local gateway_arch="${item#*:}"
  local package_work_dir="${WORK_DIR}/${openwrt_arch}"
  local control_dir="${package_work_dir}/CONTROL"
  local data_dir="${package_work_dir}/data"
  local control_tar="${package_work_dir}/control.tar.gz"
  local data_tar="${package_work_dir}/data.tar.gz"
  local debian_binary="${package_work_dir}/debian-binary"
  local ipk_path="${OUTPUT_DIR}/${APP_NAME}_${version}-${PACKAGE_RELEASE}_${openwrt_arch}.ipk"
  local apk_path="${OUTPUT_DIR}/${APP_NAME}_$(apk_package_version "${version}")_${openwrt_arch}.apk"
  local apk_scripts_dir="${package_work_dir}/apk-scripts"
  local installed_size

  log "Preparing ${openwrt_arch} package payload using gateway ${gateway_arch}"
  rm -rf "${package_work_dir}"
  mkdir -p "${control_dir}" "${data_dir}"

  copy_runtime_payload "${data_dir}" "${gateway_arch}"
  installed_size="$(du -sk "${data_dir}" | awk '{ print $1 }')"
  write_control_files "${control_dir}" "${openwrt_arch}" "${version}" "${installed_size}"

  if format_enabled ipk "${package_formats[@]}"; then
    printf '2.0\n' > "${debian_binary}"
    create_tarball "${control_dir}" "${control_tar}"
    create_tarball "${data_dir}" "${data_tar}"

    rm -f "${ipk_path}"
    create_ipk_archive "${package_work_dir}" "${ipk_path}" "${debian_binary}" "${control_tar}" "${data_tar}"

    validate_ipk "${ipk_path}" "${control_tar}" "${data_tar}" "${openwrt_arch}" "${gateway_arch}" "${version}"
    log "Built ${ipk_path}"
  fi

  if format_enabled apk "${package_formats[@]}"; then
    prepare_apk_lifecycle_scripts "${control_dir}" "${apk_scripts_dir}"
    prepare_apk_metadata_files "${data_dir}" "${control_dir}"

    rm -f "${apk_path}"
    create_apk_archive "${data_dir}" "${apk_scripts_dir}" "${apk_path}" "${openwrt_arch}" "${version}"

    validate_apk "${apk_path}" "${gateway_arch}"
    log "Built ${apk_path}"
  fi
}

main() {
  require_cmd tar
  require_cmd rsync

  if [ "${IPK_CONTAINER_FORMAT}" = "ar" ]; then
    require_cmd ar
  fi

  local version
  local package_formats=()
  local package_format_seen=" "
  local matrix_items=()
  local gateway_arches=()
  local item

  version="$(parse_app_version)"
  clean_output_dir

  while IFS= read -r item; do
    [ -n "${item}" ] || continue
    assert_gateway_arch "${item#*:}"
    matrix_items+=("${item}")
  done < <(read_arch_matrix "${ARCH_MATRIX}")

  [ "${#matrix_items[@]}" -gt 0 ] || fail "architecture matrix is empty"

  while IFS= read -r item; do
    [ -n "${item}" ] || continue
    case "${package_format_seen}" in
      *" ${item} "*)
        ;;
      *)
        package_formats+=("${item}")
        package_format_seen="${package_format_seen}${item} "
        ;;
    esac
  done < <(read_package_formats "${PACKAGE_FORMATS_RAW}")

  [ "${#package_formats[@]}" -gt 0 ] || fail "OpenWrt package format list is empty"

  if format_enabled apk "${package_formats[@]}"; then
    ensure_apk_tooling
  fi

  while IFS= read -r item; do
    [ -n "${item}" ] || continue
    gateway_arches+=("${item}")
  done < <(collect_gateway_arches "${matrix_items[@]}")

  prepare_runtime "${gateway_arches[@]}"

  for item in "${matrix_items[@]}"; do
    build_packages_for_arch "${item}" "${version}" "${package_formats[@]}"
  done

  log "OpenWrt package build completed (${package_formats[*]})"
}

main "$@"
