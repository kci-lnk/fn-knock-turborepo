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

PERSISTENT_DATA_DIR="/etc/fn-knock/data"
grep -Fq -- "option data_dir '${PERSISTENT_DATA_DIR}'" deploy/openwrt/etc/config/fn-knock || \
  fail "OpenWrt UCI config does not default to persistent data storage"
grep -Fq -- "config_get data_dir main data_dir \"${PERSISTENT_DATA_DIR}\"" deploy/openwrt/etc/init.d/fn-knock || \
  fail "OpenWrt init fallback does not use persistent data storage"
grep -Fq -- "config_get data_dir main data_dir \"${PERSISTENT_DATA_DIR}\"" deploy/openwrt/usr/bin/fn-knock-reset-panel-password || \
  fail "OpenWrt password reset fallback does not use persistent data storage"
for luci_view in \
  deploy/openwrt/www/luci-static/resources/view/fn-knock.js \
  deploy/openwrt/www/luci-static/resources/view/fn-knock-openwrt.js; do
  grep -Fq -- "o.placeholder = '${PERSISTENT_DATA_DIR}';" "${luci_view}" || \
    fail "OpenWrt LuCI data directory placeholder is not persistent: ${luci_view}"
done

MIGRATION_SCRIPT="${ROOT_DIR}/deploy/openwrt/usr/libexec/fn-knock-migrate-data-dir"
[ -x "${MIGRATION_SCRIPT}" ] || fail "OpenWrt data directory migration helper is not executable"
grep -Fq -- ':-/var/lib/fn-knock}' "${MIGRATION_SCRIPT}" || \
  fail "OpenWrt migration helper does not recognize the exact legacy default"
grep -Fq -- ':-/etc/fn-knock/data}' "${MIGRATION_SCRIPT}" || \
  fail "OpenWrt migration helper does not target the persistent default"
grep -Fq -- '/usr/libexec/fn-knock-migrate-data-dir' deploy/openwrt/control/postinst || \
  fail "OpenWrt post-install lifecycle does not run the data directory migration helper"

FAKE_UCI="${TEST_DIR}/fake-uci"
cat > "${FAKE_UCI}" <<'EOF'
#!/bin/sh
set -e

if [ "$1" = "-q" ] && [ "$2" = "get" ] && [ "$3" = "fn-knock.main.data_dir" ]; then
  cat "${FN_KNOCK_TEST_UCI_STATE}"
  exit 0
fi
if [ "$1" = "set" ]; then
  printf '%s' "${2#fn-knock.main.data_dir=}" > "${FN_KNOCK_TEST_UCI_STATE}"
  printf 'set\n' >> "${FN_KNOCK_TEST_UCI_LOG}"
  exit 0
fi
if [ "$1" = "commit" ] && [ "$2" = "fn-knock" ]; then
  printf 'commit\n' >> "${FN_KNOCK_TEST_UCI_LOG}"
  exit 0
fi
exit 1
EOF
chmod 755 "${FAKE_UCI}"

FAKE_INIT="${TEST_DIR}/fake-init"
cat > "${FAKE_INIT}" <<'EOF'
#!/bin/sh
printf '%s\n' "$1" >> "${FN_KNOCK_TEST_INIT_LOG}"
[ "${FN_KNOCK_TEST_INIT_SHOULD_FAIL:-0}" != "1" ] || exit 1
EOF
chmod 755 "${FAKE_INIT}"

run_data_dir_migration() {
  local legacy_dir="$1"
  local persistent_dir="$2"
  local uci_state="$3"
  local uci_log="$4"
  local init_log="$5"
  local init_should_fail="${6:-0}"

  FN_KNOCK_LEGACY_DATA_DIR="${legacy_dir}" \
  FN_KNOCK_PERSISTENT_DATA_DIR="${persistent_dir}" \
  FN_KNOCK_UCI_BIN="${FAKE_UCI}" \
  FN_KNOCK_INIT_SCRIPT="${FAKE_INIT}" \
  FN_KNOCK_TEST_UCI_STATE="${uci_state}" \
  FN_KNOCK_TEST_UCI_LOG="${uci_log}" \
  FN_KNOCK_TEST_INIT_LOG="${init_log}" \
  FN_KNOCK_TEST_INIT_SHOULD_FAIL="${init_should_fail}" \
    "${MIGRATION_SCRIPT}"
}

LEGACY_CASE="${TEST_DIR}/legacy-default"
LEGACY_SOURCE="${LEGACY_CASE}/var/lib/fn-knock"
LEGACY_DESTINATION="${LEGACY_CASE}/etc/fn-knock/data"
LEGACY_UCI_STATE="${LEGACY_CASE}/uci-state"
LEGACY_UCI_LOG="${LEGACY_CASE}/uci-log"
LEGACY_INIT_LOG="${LEGACY_CASE}/init-log"
mkdir -p "${LEGACY_SOURCE}/.acme.sh" "${LEGACY_SOURCE}/ssl" "${LEGACY_DESTINATION}"
printf '%s' "${LEGACY_SOURCE}" > "${LEGACY_UCI_STATE}"
printf 'acme-client' > "${LEGACY_SOURCE}/.acme.sh/acme.sh"
printf 'pow-secret' > "${LEGACY_SOURCE}/altcha_hmac_key"
printf 'certificate' > "${LEGACY_SOURCE}/ssl/fullchain.cer"
printf 'active-source' > "${LEGACY_SOURCE}/conflict"
printf 'stale-destination' > "${LEGACY_DESTINATION}/conflict"

run_data_dir_migration \
  "${LEGACY_SOURCE}" \
  "${LEGACY_DESTINATION}" \
  "${LEGACY_UCI_STATE}" \
  "${LEGACY_UCI_LOG}" \
  "${LEGACY_INIT_LOG}"

[ "$(cat "${LEGACY_UCI_STATE}")" = "${LEGACY_DESTINATION}" ] || \
  fail "legacy default data directory migration did not commit the persistent UCI path"
[ "$(cat "${LEGACY_DESTINATION}/conflict")" = "active-source" ] || \
  fail "legacy active data did not win migration conflicts"
[ -f "${LEGACY_DESTINATION}/.acme.sh/acme.sh" ] || \
  fail "legacy ACME installation was not migrated"
[ -f "${LEGACY_DESTINATION}/altcha_hmac_key" ] || \
  fail "legacy PoW key was not migrated"
[ -f "${LEGACY_DESTINATION}/ssl/fullchain.cer" ] || \
  fail "legacy certificates were not migrated"
[ -f "${LEGACY_SOURCE}/.acme.sh/acme.sh" ] || \
  fail "legacy data was removed instead of retained for rollback"
grep -Fxq -- "stop" "${LEGACY_INIT_LOG}" || \
  fail "service was not stopped before migrating active legacy data"
[ "$(cat "${LEGACY_UCI_LOG}")" = $'set\ncommit' ] || \
  fail "legacy migration did not commit UCI only after copying data"

CUSTOM_CASE="${TEST_DIR}/custom-data-dir"
CUSTOM_SOURCE="${CUSTOM_CASE}/var/lib/fn-knock"
CUSTOM_DESTINATION="${CUSTOM_CASE}/etc/fn-knock/data"
CUSTOM_UCI_STATE="${CUSTOM_CASE}/uci-state"
CUSTOM_UCI_LOG="${CUSTOM_CASE}/uci-log"
CUSTOM_INIT_LOG="${CUSTOM_CASE}/init-log"
mkdir -p "${CUSTOM_SOURCE}"
printf '/mnt/persistent/fn-knock' > "${CUSTOM_UCI_STATE}"
printf 'custom-data' > "${CUSTOM_SOURCE}/marker"
: > "${CUSTOM_UCI_LOG}"
: > "${CUSTOM_INIT_LOG}"

run_data_dir_migration \
  "${CUSTOM_SOURCE}" \
  "${CUSTOM_DESTINATION}" \
  "${CUSTOM_UCI_STATE}" \
  "${CUSTOM_UCI_LOG}" \
  "${CUSTOM_INIT_LOG}"

[ "$(cat "${CUSTOM_UCI_STATE}")" = "/mnt/persistent/fn-knock" ] || \
  fail "custom OpenWrt data directory was modified"
[ ! -e "${CUSTOM_DESTINATION}" ] || \
  fail "custom OpenWrt data directory triggered legacy migration"
[ ! -s "${CUSTOM_UCI_LOG}" ] && [ ! -s "${CUSTOM_INIT_LOG}" ] || \
  fail "custom OpenWrt data directory caused migration side effects"

FAILURE_CASE="${TEST_DIR}/copy-failure"
FAILURE_SOURCE="${FAILURE_CASE}/var/lib/fn-knock"
FAILURE_DESTINATION="${FAILURE_CASE}/etc/fn-knock/data"
FAILURE_UCI_STATE="${FAILURE_CASE}/uci-state"
FAILURE_UCI_LOG="${FAILURE_CASE}/uci-log"
FAILURE_INIT_LOG="${FAILURE_CASE}/init-log"
mkdir -p "${FAILURE_SOURCE}/conflict" "${FAILURE_DESTINATION}"
printf '%s' "${FAILURE_SOURCE}" > "${FAILURE_UCI_STATE}"
printf 'source-child' > "${FAILURE_SOURCE}/conflict/child"
printf 'destination-file' > "${FAILURE_DESTINATION}/conflict"
: > "${FAILURE_UCI_LOG}"
: > "${FAILURE_INIT_LOG}"

if run_data_dir_migration \
  "${FAILURE_SOURCE}" \
  "${FAILURE_DESTINATION}" \
  "${FAILURE_UCI_STATE}" \
  "${FAILURE_UCI_LOG}" \
  "${FAILURE_INIT_LOG}" >/dev/null 2>&1; then
  fail "OpenWrt data migration accepted a failed copy"
fi
[ "$(cat "${FAILURE_UCI_STATE}")" = "${FAILURE_SOURCE}" ] || \
  fail "failed OpenWrt data migration changed the UCI path"
[ ! -s "${FAILURE_UCI_LOG}" ] || \
  fail "failed OpenWrt data migration committed UCI changes"

STOP_FAILURE_CASE="${TEST_DIR}/stop-failure"
STOP_FAILURE_SOURCE="${STOP_FAILURE_CASE}/var/lib/fn-knock"
STOP_FAILURE_DESTINATION="${STOP_FAILURE_CASE}/etc/fn-knock/data"
STOP_FAILURE_UCI_STATE="${STOP_FAILURE_CASE}/uci-state"
STOP_FAILURE_UCI_LOG="${STOP_FAILURE_CASE}/uci-log"
STOP_FAILURE_INIT_LOG="${STOP_FAILURE_CASE}/init-log"
mkdir -p "${STOP_FAILURE_SOURCE}"
printf '%s' "${STOP_FAILURE_SOURCE}" > "${STOP_FAILURE_UCI_STATE}"
printf 'active-data' > "${STOP_FAILURE_SOURCE}/marker"
: > "${STOP_FAILURE_UCI_LOG}"
: > "${STOP_FAILURE_INIT_LOG}"

if run_data_dir_migration \
  "${STOP_FAILURE_SOURCE}" \
  "${STOP_FAILURE_DESTINATION}" \
  "${STOP_FAILURE_UCI_STATE}" \
  "${STOP_FAILURE_UCI_LOG}" \
  "${STOP_FAILURE_INIT_LOG}" \
  "1" >/dev/null 2>&1; then
  fail "OpenWrt data migration continued after the service failed to stop"
fi
[ "$(cat "${STOP_FAILURE_UCI_STATE}")" = "${STOP_FAILURE_SOURCE}" ] || \
  fail "service stop failure changed the OpenWrt UCI data directory"
[ ! -e "${STOP_FAILURE_DESTINATION}" ] || \
  fail "service stop failure copied data into the persistent directory"
[ ! -s "${STOP_FAILURE_UCI_LOG}" ] || \
  fail "service stop failure committed OpenWrt UCI changes"
grep -Fxq -- "stop" "${STOP_FAILURE_INIT_LOG}" || \
  fail "service stop failure test did not attempt to stop fn-knock"

printf '[test-openwrt-runtime-contract] OpenWrt package and runtime contract passed\n'
