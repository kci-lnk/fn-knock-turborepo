#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${ROOT_DIR}"

fail() {
  printf '[test-openwrt-firewall] ERROR: %s\n' "$*" >&2
  exit 1
}

HELPER="${ROOT_DIR}/deploy/openwrt/usr/libexec/fn-knock-firewall"
INIT_SCRIPT="${ROOT_DIR}/deploy/openwrt/etc/init.d/fn-knock"
PRERM="${ROOT_DIR}/deploy/openwrt/control/prerm"
ACL="${ROOT_DIR}/deploy/openwrt/usr/share/rpcd/acl.d/luci-app-fn-knock.json"
POSTINST="${ROOT_DIR}/deploy/openwrt/control/postinst"
TEST_DIR="$(mktemp -d "${ROOT_DIR}/dist/openwrt-firewall.XXXXXX")"
trap 'rm -rf "${TEST_DIR}"' EXIT

[ -x "${HELPER}" ] || fail "OpenWrt firewall helper is not executable"
sh -n "${HELPER}"

grep -Fq -- "option auto_open_firewall '1'" deploy/openwrt/etc/config/fn-knock || \
  fail "fresh OpenWrt installs do not enable gateway ingress by default"
grep -Fq -- 'config_get_bool auto_open_firewall main auto_open_firewall 0' "${INIT_SCRIPT}" || \
  fail "upgraded OpenWrt installs do not default a missing firewall option to disabled"
grep -Fq -- '/usr/libexec/fn-knock-firewall status' "${ACL}" || \
  fail "LuCI ACL does not expose the read-only firewall status command"
grep -Fq -- 'if ! "$helper" sync; then' "${INIT_SCRIPT}" || \
  fail "OpenWrt startup does not treat firewall synchronization as non-fatal"
grep -Fq -- '/usr/libexec/fn-knock-firewall remove' "${PRERM}" || \
  fail "OpenWrt uninstall does not remove the owned firewall rule"
grep -Fq -- 'call session create' "${HELPER}" || \
  fail "firewall helper does not isolate its UCI transaction in an rpcd session"
grep -Fq -- 'call uci commit' "${HELPER}" || \
  fail "firewall helper does not commit through the isolated UCI session"

for luci_view in \
  deploy/openwrt/www/luci-static/resources/view/fn-knock.js \
  deploy/openwrt/www/luci-static/resources/view/fn-knock-openwrt.js; do
  grep -Fq -- "form.Flag, 'auto_open_firewall'" "${luci_view}" || \
    fail "LuCI view is missing the automatic firewall toggle: ${luci_view}"
  grep -Fq -- "fs.exec('/usr/libexec/fn-knock-firewall', [ 'status' ])" "${luci_view}" || \
    fail "LuCI view is missing firewall status loading: ${luci_view}"
done

if grep -Eq -- '(^|[ /])(iptables|ip6tables|nft|nftables)([[:space:]"/]|$)' "${HELPER}"; then
  fail "OpenWrt firewall helper directly invokes a packet-filtering command"
fi

FAKE_UCI="${TEST_DIR}/uci"
FAKE_UBUS="${TEST_DIR}/ubus"
FAKE_JSONFILTER="${TEST_DIR}/jsonfilter"
FAKE_FIREWALL="${TEST_DIR}/firewall"
FAKE_LOCK="${TEST_DIR}/lock"
STATE_FILE="${TEST_DIR}/uci-state"
SESSION_STATE_FILE="${TEST_DIR}/uci-session-state"
CALL_LOG="${TEST_DIR}/calls"
STATUS_FILE="${TEST_DIR}/status.json"

cat > "${FAKE_UCI}" <<'EOF'
#!/bin/sh
set -eu

state="${FN_KNOCK_TEST_UCI_STATE:?}"
log="${FN_KNOCK_TEST_CALL_LOG:?}"
save_dir=""

while [ "$#" -gt 0 ]; do
	case "$1" in
		-q) shift ;;
		-t) save_dir="${2:?}"; shift 2 ;;
		*) break ;;
	esac
done

active_state="$state"
if [ -n "$save_dir" ]; then
	mkdir -p "$save_dir"
	active_state="$save_dir/firewall"
	if [ ! -e "$active_state" ]; then
		cp "$state" "$active_state"
	fi
fi

set_value() {
	assignment="$1"
	key="${assignment%%=*}"
	value="${assignment#*=}"
	case "$value" in
		\'*) value="${value#\'}"; value="${value%\'}" ;;
	esac
	temporary="${state}.$$"
	awk -F= -v key="$key" '$1 != key { print }' "$active_state" > "$temporary"
	printf '%s=%s\n' "$key" "$value" >> "$temporary"
	mv -f "$temporary" "$active_state"
}

case "${1:-}" in
	get)
		key="${2:?}"
		line="$(grep -F "${key}=" "$active_state" | tail -n 1 || true)"
		[ -n "$line" ] || exit 1
		printf '%s\n' "${line#*=}"
		;;
	set)
		set_value "${2:?}"
		;;
	delete)
		key="${2:?}"
		temporary="${state}.$$"
		awk -F= -v key="$key" '$1 != key && index($1, key ".") != 1 { print }' "$active_state" > "$temporary"
		mv -f "$temporary" "$active_state"
		;;
	batch)
		while IFS=' ' read -r command assignment; do
			[ -n "${command:-}" ] || continue
			[ "$command" = "set" ] || exit 1
			set_value "$assignment"
		done
		;;
	commit)
		[ "${FN_KNOCK_TEST_UCI_COMMIT_FAIL:-0}" != "1" ] || exit 1
		[ -z "$save_dir" ] || cp "$active_state" "$state"
		printf 'commit %s\n' "${2:?}" >> "$log"
		;;
	*)
		exit 1
		;;
esac
EOF

cat > "${FAKE_UBUS}" <<'EOF'
#!/bin/sh
set -eu

state="${FN_KNOCK_TEST_UCI_STATE:?}"
session_state="${FN_KNOCK_TEST_SESSION_STATE:?}"
log="${FN_KNOCK_TEST_CALL_LOG:?}"

json_first() {
	printf '%s\n' "$request" | grep -o "\"$1\":\"[^\"]*\"" | head -n 1 | cut -d '"' -f 4
}

json_last() {
	printf '%s\n' "$request" | grep -o "\"$1\":\"[^\"]*\"" | tail -n 1 | cut -d '"' -f 4
}

set_value() {
	assignment="$1"
	key="${assignment%%=*}"
	value="${assignment#*=}"
	temporary="${session_state}.$$"
	awk -F= -v key="$key" '$1 != key { print }' "$session_state" > "$temporary"
	printf '%s=%s\n' "$key" "$value" >> "$temporary"
	mv -f "$temporary" "$session_state"
}

delete_section() {
	key="$1"
	temporary="${session_state}.$$"
	awk -F= -v key="$key" '$1 != key && index($1, key ".") != 1 { print }' "$session_state" > "$temporary"
	mv -f "$temporary" "$session_state"
}

[ "${1:-}" = "call" ] || exit 1
object="${2:?}"
method="${3:?}"
request="${4:-{}}"

case "$object:$method" in
	session:create)
		cp "$state" "$session_state"
		printf '{"ubus_rpc_session":"0123456789abcdef0123456789abcdef"}\n'
		;;
	session:grant)
		grep -q '"firewall","write"' <<-JSON || exit 1
		$request
		JSON
		;;
	session:destroy)
		rm -f "$session_state"
		;;
	uci:get)
		config="$(json_first config)"
		section="$(json_first section)"
		option="$(json_first option 2>/dev/null || true)"
		key="$config.$section"
		[ -z "$option" ] || key="$key.$option"
		line="$(grep -F "${key}=" "$session_state" | tail -n 1 || true)"
		[ -n "$line" ] || exit 1
		value="${line#*=}"
		if [ -n "$option" ]; then
			printf '{"value":"%s"}\n' "$value"
		else
			printf '{"values":{".type":"%s"}}\n' "$value"
		fi
		;;
	uci:add|uci:set)
		config="$(json_first config)"
		if [ "$method" = "add" ]; then
			section="$(json_first name)"
			if grep -Fq "${config}.${section}=" "$session_state"; then
				exit 1
			fi
			set_value "${config}.${section}=$(json_first type)"
		else
			section="$(json_first section)"
			grep -Fq "${config}.${section}=" "$session_state" || exit 1
		fi
		for option in name src proto dest_port target family enabled; do
			value="$(json_last "$option" 2>/dev/null || true)"
			[ -z "$value" ] || set_value "${config}.${section}.${option}=$value"
		done
		;;
	uci:delete)
		delete_section "$(json_first config).$(json_first section)"
		;;
	uci:commit)
		[ "${FN_KNOCK_TEST_UCI_COMMIT_FAIL:-0}" != "1" ] || exit 1
		cp "$session_state" "$state"
		printf 'commit %s\n' "$(json_first config)" >> "$log"
		;;
	*)
		exit 1
		;;
esac
EOF

cat > "${FAKE_JSONFILTER}" <<'EOF'
#!/bin/sh
set -eu

[ "${1:-}" = "-e" ] || exit 1
expression="${2:?}"
input="$(cat)"
case "$expression" in
	'@.ubus_rpc_session') key='ubus_rpc_session' ;;
	'@.value') key='value' ;;
	'@.values[".type"]') key='.type' ;;
	*) exit 1 ;;
esac
printf '%s\n' "$input" | grep -o "\"$key\":\"[^\"]*\"" | head -n 1 | cut -d '"' -f 4
EOF

cat > "${FAKE_LOCK}" <<'EOF'
#!/bin/sh
set -eu

if [ "${1:-}" = "-u" ]; then
	rmdir "${2:?}.held" 2>/dev/null || true
	exit 0
fi
mkdir "${1:?}.held"
EOF

cat > "${FAKE_FIREWALL}" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "${1:-}" >> "${FN_KNOCK_TEST_CALL_LOG:?}"
[ "${FN_KNOCK_TEST_FIREWALL_FAIL:-0}" != "1" ]
EOF
chmod 755 "${FAKE_UCI}" "${FAKE_UBUS}" "${FAKE_JSONFILTER}" \
  "${FAKE_FIREWALL}" "${FAKE_LOCK}"

run_helper() {
  FN_KNOCK_FIREWALL_UBUS_BIN="${FAKE_UBUS}" \
  FN_KNOCK_FIREWALL_JSONFILTER_BIN="${FAKE_JSONFILTER}" \
  FN_KNOCK_FIREWALL_INIT="${FAKE_FIREWALL}" \
  FN_KNOCK_FIREWALL_LOGGER_BIN="/usr/bin/true" \
  FN_KNOCK_FIREWALL_STATUS_FILE="${STATUS_FILE}" \
  FN_KNOCK_FIREWALL_LOCK_BIN="${FAKE_LOCK}" \
  FN_KNOCK_FIREWALL_LOCK_FILE="${TEST_DIR}/firewall.lock" \
  FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
  FN_KNOCK_TEST_SESSION_STATE="${SESSION_STATE_FILE}" \
  FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
  FN_KNOCK_TEST_FIREWALL_FAIL="${FN_KNOCK_TEST_FIREWALL_FAIL:-0}" \
  FN_KNOCK_TEST_UCI_COMMIT_FAIL="${FN_KNOCK_TEST_UCI_COMMIT_FAIL:-0}" \
    "${HELPER}" "$@"
}

uci_value() {
  FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
  FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
    "${FAKE_UCI}" -q get "$1"
}

set_uci_value() {
  FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
  FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
    "${FAKE_UCI}" -q set "$1=$2"
}

reset_state() {
  printf '%s\n' "$@" > "${STATE_FILE}"
  : > "${CALL_LOG}"
  rm -f "${STATUS_FILE}" "${SESSION_STATE_FILE}"
}

DEFAULT_STATUS="$(run_helper status)"
[ "${DEFAULT_STATUS}" = '{"state":"disabled","port":null,"source_zone":"wan","family":"any"}' ] || \
  fail "firewall status command does not return the stable default JSON"
[ ! -e "${STATUS_FILE}" ] || fail "read-only firewall status command created a state file"

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999'
mkdir "${TEST_DIR}/firewall.lock.held"
if run_helper sync >/dev/null 2>&1; then
  fail "concurrent firewall synchronization bypassed the exclusive lock"
fi
rmdir "${TEST_DIR}/firewall.lock.held"
[ ! -s "${CALL_LOG}" ] || \
  fail "lock contention changed or reloaded the firewall"
grep -Fq -- '"state":"error","port":null' "${STATUS_FILE}" || \
  fail "lock contention was not persisted as an error state"

run_firewall_default_migration() {
  local upgrade="$1"
  local marker="$2"

  PKG_UPGRADE="${upgrade}" \
  FN_KNOCK_POSTINST_UCI_BIN="${FAKE_UCI}" \
  FN_KNOCK_FIREWALL_DEFAULT_MARKER="${marker}" \
  FN_KNOCK_POSTINST_TEST_FIREWALL_DEFAULT_ONLY=1 \
  FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
  FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
    "${POSTINST}"
}

FRESH_MARKER="${TEST_DIR}/fresh-data/.firewall-default-v1"
reset_state \
  'fn-knock.main.data_dir=/unused/fresh-data' \
  'fn-knock.main.auto_open_firewall=1'
run_firewall_default_migration 0 "${FRESH_MARKER}"
[ "$(uci_value fn-knock.main.auto_open_firewall)" = "1" ] || \
  fail "fresh install firewall default was not kept enabled"
[ -f "${FRESH_MARKER}" ] || fail "fresh install did not persist the firewall-default marker"
[ ! -s "${CALL_LOG}" ] || fail "fresh install rewrote its explicit firewall default"

UPGRADE_MARKER="${TEST_DIR}/upgrade-data/.firewall-default-v1"
reset_state \
  'fn-knock.main.data_dir=/unused/upgrade-data' \
  'fn-knock.main.auto_open_firewall=1'
run_firewall_default_migration 1 "${UPGRADE_MARKER}"
[ "$(uci_value fn-knock.main.auto_open_firewall)" = "0" ] || \
  fail "first upgrade did not force the automatic firewall default off"
[ -f "${UPGRADE_MARKER}" ] || fail "first upgrade did not persist the firewall-default marker"
grep -Fxq -- 'commit fn-knock' "${CALL_LOG}" || \
  fail "first upgrade did not commit its safe firewall default"

set_uci_value fn-knock.main.auto_open_firewall 1
: > "${CALL_LOG}"
run_firewall_default_migration 1 "${UPGRADE_MARKER}"
[ "$(uci_value fn-knock.main.auto_open_firewall)" = "1" ] || \
  fail "later upgrade overwrote the user's automatic firewall choice"
[ ! -s "${CALL_LOG}" ] || fail "later upgrade committed an unchanged firewall choice"

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.go_reproxy_port=8888'
run_helper sync
grep -Fq -- '"state":"disabled"' "${STATUS_FILE}" || \
  fail "an upgraded config without auto_open_firewall was not kept disabled"
[ ! -s "${CALL_LOG}" ] || fail "disabled upgraded config changed the firewall"

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999'
run_helper sync
[ "$(uci_value firewall.fn_knock_gateway_ingress)" = "rule" ] || \
  fail "automatic firewall sync did not create a rule section"
[ "$(uci_value firewall.fn_knock_gateway_ingress.name)" = "Allow-FnKnock-Gateway" ] || \
  fail "automatic firewall rule is missing its ownership marker"
[ "$(uci_value firewall.fn_knock_gateway_ingress.src)" = "wan" ] || \
  fail "automatic firewall rule does not target the wan zone"
[ "$(uci_value firewall.fn_knock_gateway_ingress.proto)" = "tcp" ] || \
  fail "automatic firewall rule is not TCP-only"
[ "$(uci_value firewall.fn_knock_gateway_ingress.dest_port)" = "7999" ] || \
  fail "automatic firewall rule does not use the default gateway port"
[ "$(uci_value firewall.fn_knock_gateway_ingress.family)" = "any" ] || \
  fail "automatic firewall rule does not cover IPv4 and IPv6"
[ "$(uci_value firewall.fn_knock_gateway_ingress.target)" = "ACCEPT" ] || \
  fail "automatic firewall rule does not accept gateway traffic"
grep -Fq -- '"state":"active","port":7999,"source_zone":"wan","family":"any"' "${STATUS_FILE}" || \
  fail "active firewall status JSON does not match the stable contract"
[ "$(grep -c '^commit firewall$' "${CALL_LOG}")" -eq 1 ] || \
  fail "initial firewall sync did not commit exactly once"
[ "$(grep -c '^reload$' "${CALL_LOG}")" -eq 1 ] || \
  fail "initial firewall sync did not reload exactly once"

USER_SAVE_DIR="${TEST_DIR}/user-uci-staging"
FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
  "${FAKE_UCI}" -q -t "${USER_SAVE_DIR}" set firewall.user_pending=rule
set_uci_value fn-knock.main.go_reproxy_port 8888
run_helper sync
if uci_value firewall.user_pending >/dev/null 2>&1; then
  fail "isolated firewall sync committed an unrelated staged UCI change"
fi
FN_KNOCK_TEST_UCI_STATE="${STATE_FILE}" \
FN_KNOCK_TEST_CALL_LOG="${CALL_LOG}" \
  "${FAKE_UCI}" -q -t "${USER_SAVE_DIR}" get firewall.user_pending >/dev/null || \
  fail "isolated firewall sync discarded an unrelated staged UCI change"
[ "$(uci_value firewall.fn_knock_gateway_ingress.dest_port)" = "8888" ] || \
  fail "isolated firewall sync did not commit its own port update"

set_uci_value fn-knock.main.go_reproxy_port 7999
run_helper sync

run_helper sync
[ "$(grep -c '^commit firewall$' "${CALL_LOG}")" -eq 3 ] || \
  fail "idempotent firewall sync committed unchanged state"
[ "$(grep -c '^reload$' "${CALL_LOG}")" -eq 3 ] || \
  fail "idempotent firewall sync reloaded unchanged state"

set_uci_value fn-knock.main.go_reproxy_port 8888
run_helper sync
[ "$(uci_value firewall.fn_knock_gateway_ingress.dest_port)" = "8888" ] || \
  fail "custom Go gateway port was not synchronized"
[ "$(grep -c '^commit firewall$' "${CALL_LOG}")" -eq 4 ] || \
  fail "gateway port update did not commit exactly once"
[ "$(grep -c '^reload$' "${CALL_LOG}")" -eq 4 ] || \
  fail "gateway port update did not reload exactly once"

set_uci_value fn-knock.main.auto_open_firewall 0
run_helper sync
if uci_value firewall.fn_knock_gateway_ingress >/dev/null 2>&1; then
  fail "disabling automatic firewall management retained the owned rule"
fi
grep -Fq -- '"state":"disabled"' "${STATUS_FILE}" || \
  fail "disabled firewall state was not reported"

reset_state \
  'fn-knock.main.enabled=0' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999' \
  'firewall.fn_knock_gateway_ingress=rule' \
  'firewall.fn_knock_gateway_ingress.name=Allow-FnKnock-Gateway'
run_helper sync
if uci_value firewall.fn_knock_gateway_ingress >/dev/null 2>&1; then
  fail "disabling fn-knock retained the owned firewall rule"
fi

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999' \
  'firewall.fn_knock_gateway_ingress=rule' \
  'firewall.fn_knock_gateway_ingress.name=Unrelated-Rule'
if run_helper sync >/dev/null 2>&1; then
  fail "firewall sync overwrote an unowned named section"
fi
[ "$(uci_value firewall.fn_knock_gateway_ingress.name)" = "Unrelated-Rule" ] || \
  fail "firewall ownership conflict modified the unrelated rule"
grep -Fq -- '"state":"conflict"' "${STATUS_FILE}" || \
  fail "firewall ownership conflict was not reported"
[ ! -s "${CALL_LOG}" ] || fail "firewall ownership conflict committed or reloaded"
if run_helper remove >/dev/null 2>&1; then
  fail "firewall removal deleted an unowned named section"
fi

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=0'
if run_helper sync >/dev/null 2>&1; then
  fail "firewall sync accepted an invalid gateway port"
fi
[ ! -s "${CALL_LOG}" ] || fail "invalid gateway port modified the firewall"
grep -Fq -- '"state":"error","port":null' "${STATUS_FILE}" || \
  fail "invalid gateway port was not reported as an error"

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999'
FN_KNOCK_TEST_FIREWALL_FAIL=1
export FN_KNOCK_TEST_FIREWALL_FAIL
if run_helper sync >/dev/null 2>&1; then
  fail "firewall sync reported success after reload failed"
fi
unset FN_KNOCK_TEST_FIREWALL_FAIL
grep -Fq -- '"state":"error","port":7999' "${STATUS_FILE}" || \
  fail "firewall reload failure was not persisted in status"
[ "$(uci_value firewall.fn_knock_gateway_ingress.name)" = "Allow-FnKnock-Gateway" ] || \
  fail "firewall reload failure lost the owned UCI rule"

run_helper sync
grep -Fq -- '"state":"active","port":7999' "${STATUS_FILE}" || \
  fail "firewall sync did not recover after a transient reload failure"
[ "$(grep -c '^reload$' "${CALL_LOG}")" -eq 2 ] || \
  fail "firewall sync did not retry a previously failed reload"

FN_KNOCK_TEST_FIREWALL_FAIL=1
export FN_KNOCK_TEST_FIREWALL_FAIL
if run_helper remove >/dev/null 2>&1; then
  fail "firewall removal reported success after reload failed"
fi
unset FN_KNOCK_TEST_FIREWALL_FAIL
if uci_value firewall.fn_knock_gateway_ingress >/dev/null 2>&1; then
  fail "explicit firewall cleanup retained the owned rule"
fi
grep -Fq -- '"state":"error","port":7999' "${STATUS_FILE}" || \
  fail "firewall removal reload failure was not persisted in status"

run_helper remove
grep -Fq -- '"state":"disabled","port":7999' "${STATUS_FILE}" || \
  fail "firewall removal did not recover after a transient reload failure"
[ "$(grep -c '^reload$' "${CALL_LOG}")" -eq 4 ] || \
  fail "firewall removal did not retry a previously failed reload"

reset_state \
  'fn-knock.main.enabled=1' \
  'fn-knock.main.auto_open_firewall=1' \
  'fn-knock.main.go_reproxy_port=7999'
FN_KNOCK_TEST_UCI_COMMIT_FAIL=1
export FN_KNOCK_TEST_UCI_COMMIT_FAIL
if run_helper sync >/dev/null 2>&1; then
  fail "firewall sync reported success after its isolated UCI commit failed"
fi
unset FN_KNOCK_TEST_UCI_COMMIT_FAIL
if uci_value firewall.fn_knock_gateway_ingress >/dev/null 2>&1; then
  fail "failed isolated UCI commit leaked a staged firewall rule"
fi
[ ! -e "${SESSION_STATE_FILE}" ] || \
  fail "firewall helper left an isolated UCI session behind"
[ ! -d "${TEST_DIR}/firewall.lock.held" ] || \
  fail "firewall helper left its synchronization lock held"

printf '[test-openwrt-firewall] OpenWrt firewall contract passed\n'
