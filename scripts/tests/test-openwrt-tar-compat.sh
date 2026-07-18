#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mkdir -p "${ROOT_DIR}/dist"
WORK_DIR="$(mktemp -d "${ROOT_DIR}/dist/openwrt-tar-compat-test.XXXXXX")"

cleanup() {
  rm -rf "${WORK_DIR}"
}
trap cleanup EXIT

test_fail() {
  printf '[test-openwrt-tar-compat] ERROR: %s\n' "$*" >&2
  exit 1
}

source "${ROOT_DIR}/scripts/build-openwrt-ipk.sh"

MOUNT_SAFE_CHECK_COUNT="$(
  grep -Fc 'find /inspect -mindepth 1' "${ROOT_DIR}/scripts/build-openwrt-ipk.sh"
)"
[ "${MOUNT_SAFE_CHECK_COUNT}" -eq 2 ] || \
  test_fail "both APK ownership checks must skip the bind-mounted extraction root"

configure_tar_compatibility
case "${TAR_FLAVOR}" in
  bsd)
    [ "${TAR_OWNER_ARGS[*]}" = "--uid 0 --gid 0 --uname root --gname root" ] || \
      test_fail "unexpected bsdtar owner arguments: ${TAR_OWNER_ARGS[*]}"
    ;;
  gnu)
    [ "${TAR_OWNER_ARGS[*]}" = "--owner=0 --group=0 --numeric-owner --sort=name" ] || \
      test_fail "unexpected GNU tar owner arguments: ${TAR_OWNER_ARGS[*]}"
    ;;
  *)
    test_fail "unexpected tar flavor: ${TAR_FLAVOR}"
    ;;
esac

SOURCE_DIR="${WORK_DIR}/source"
PACKAGE_DIR="${WORK_DIR}/package"
CONTENT_TAR="${WORK_DIR}/content.tar.gz"
NON_ROOT_TAR="${WORK_DIR}/non-root.tar.gz"
IPK_TAR="${WORK_DIR}/package.ipk"
mkdir -p "${SOURCE_DIR}" "${PACKAGE_DIR}"
printf 'fixture\n' > "${SOURCE_DIR}/payload"

create_tarball "${SOURCE_DIR}" "${CONTENT_TAR}"
validate_root_ownership "${CONTENT_TAR}"
tar -tzf "${CONTENT_TAR}" | normalize_tar_listing | grep -Fxq "payload" || \
  test_fail "content tarball is missing its payload"

case "${TAR_FLAVOR}" in
  bsd)
    COPYFILE_DISABLE=1 tar --uid 1 --gid 1 --uname daemon --gname daemon \
      --format=ustar -czf "${NON_ROOT_TAR}" -C "${SOURCE_DIR}" .
    ;;
  gnu)
    tar --owner=1 --group=1 --numeric-owner --format=ustar \
      -czf "${NON_ROOT_TAR}" -C "${SOURCE_DIR}" .
    ;;
esac
if (validate_root_ownership "${NON_ROOT_TAR}") >/dev/null 2>&1; then
  test_fail "${TAR_FLAVOR} ownership validation accepted a non-root archive"
fi

printf '2.0\n' > "${PACKAGE_DIR}/debian-binary"
cp "${CONTENT_TAR}" "${PACKAGE_DIR}/data.tar.gz"
cp "${CONTENT_TAR}" "${PACKAGE_DIR}/control.tar.gz"
create_tar_ipk_archive "${PACKAGE_DIR}" "${IPK_TAR}"
validate_root_ownership "${IPK_TAR}"

IPK_LISTING="$(tar -tzf "${IPK_TAR}" | normalize_tar_listing)"
[ "${IPK_LISTING}" = $'debian-binary\ndata.tar.gz\ncontrol.tar.gz' ] || {
  printf '%s\n' "${IPK_LISTING}" >&2
  test_fail "tar-format IPK member order changed"
}

printf '[test-openwrt-tar-compat] %s tar compatibility passed\n' "${TAR_FLAVOR}"
