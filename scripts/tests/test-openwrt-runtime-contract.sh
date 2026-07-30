#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

fail() {
  printf '[test-openwrt-runtime-contract] ERROR: %s\n' "$*" >&2
  exit 1
}

unset FN_KNOCK_OPENWRT_DEPENDS
source "${ROOT_DIR}/scripts/build-openwrt-ipk.sh"

EXPECTED_IPK_DEPENDS="libc, bash, curl, unzip, ca-bundle, ca-certificates, luci-base"
EXPECTED_APK_DEPENDS="libc bash curl unzip ca-bundle ca-certificates luci-base"
FORBIDDEN_DEPENDENCIES=(
  iptables
  iptables-nft
  ip6tables
  ip6tables-nft
  kmod-ip6tables
  kmod-nf-conntrack
  kmod-ipt-conntrack
  kmod-nft-compat
  nftables
)

[ "${DEPENDS}" = "${EXPECTED_IPK_DEPENDS}" ] || \
  fail "unexpected default dependencies: ${DEPENDS}"

validate_openwrt_dependencies "${DEPENDS}"
for forbidden_override in \
  "libc, iptables-nft" \
  "libc, kmod-ipt-conntrack" \
  "libc, nftables-json" \
  "libc, firewall4"; do
  if (validate_openwrt_dependencies "${forbidden_override}") >/dev/null 2>&1; then
    fail "firewall dependency override was accepted: ${forbidden_override}"
  fi
done

APK_DEPENDS="$(apk_package_depends "${DEPENDS}")"
[ "${APK_DEPENDS}" = "${EXPECTED_APK_DEPENDS}" ] || \
  fail "unexpected APK dependencies: ${APK_DEPENDS}"

for dependency in "${FORBIDDEN_DEPENDENCIES[@]}"; do
  case " ${APK_DEPENDS} " in
    *" ${dependency} "*)
      fail "OpenWrt package still depends on ${dependency}"
      ;;
  esac
done

TEST_DIR="$(mktemp -d "${ROOT_DIR}/dist/openwrt-runtime-contract.XXXXXX")"
cleanup() {
  rm -rf "${TEST_DIR}"
}
trap cleanup EXIT

write_control_files "${TEST_DIR}/CONTROL" "aarch64_cortex-a53" "2.1.4" "1"
CONTROL_FILE="${TEST_DIR}/CONTROL/control"
grep -Fxq -- "Depends: ${EXPECTED_IPK_DEPENDS}" "${CONTROL_FILE}" || \
  fail "IPK control metadata has unexpected dependencies"

for dependency in "${FORBIDDEN_DEPENDENCIES[@]}"; do
  if grep -Eq -- "(^|[ ,])${dependency}([ ,]|$)" "${CONTROL_FILE}"; then
    fail "IPK control metadata still contains ${dependency}"
  fi
done

grep -Fq -- '"FN_KNOCK_DISABLE_IPTABLES=1"' deploy/openwrt/etc/init.d/fn-knock || \
  fail "OpenWrt gateway does not force-disable iptables"

if grep -Eq -- 'clean\.sh|iptables|ip6tables|nftables' deploy/openwrt/control/prerm; then
  fail "OpenWrt uninstall lifecycle still performs firewall cleanup"
fi

grep -Fq -- 'rm -f /var/lib/fn-knock/clean.sh' deploy/openwrt/control/postinst || \
  fail "OpenWrt upgrade does not remove a legacy firewall cleanup script"

printf '[test-openwrt-runtime-contract] OpenWrt package and runtime contract passed\n'
