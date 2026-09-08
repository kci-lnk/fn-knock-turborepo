#!/bin/sh
set -eu
HELPER=/workspace/deploy/openwrt/usr/libexec/fn-knock-firewall
fail() { echo "FAIL: $*" >&2; exit 1; }
assert_value() { [ "$(uci -q get "firewall.fn_knock_gateway_ingress.$1")" = "$2" ] || fail "$1 != $2"; }
expect_error() {
	local expected="$1"; shift
	if "$HELPER" "$@" >/tmp/result 2>/tmp/error; then fail "expected $expected"; fi
	grep -q "\"state\":\"$expected\"" /tmp/result || { cat /tmp/result /tmp/error; fail "wrong error $expected"; }
}
reset_rule() { uci -q delete firewall.fn_knock_gateway_ingress || true; uci commit firewall; : >/tmp/reloads; }
set_port() { uci set "fn-knock.main.go_reproxy_port=$1"; uci commit fn-knock; }
mkdir -p /var/run/ubus
cp /workspace/deploy/openwrt/etc/config/fn-knock /etc/config/fn-knock
cp "$HELPER" /usr/libexec/fn-knock-firewall
cp /workspace/deploy/openwrt/usr/share/rpcd/acl.d/luci-app-fn-knock.json /usr/share/rpcd/acl.d/
ubusd >/tmp/ubusd.log 2>&1 &
sleep 1
rpcd >/tmp/rpcd.log 2>&1 &
sleep 1
ubus list uci | grep -q uci || fail 'rpcd not running'
# Exercise rpcd's actual ACL expansion for a read-only and a writable login.
for user in reader writer; do
	uci set "rpcd.$user=login"
	uci set "rpcd.$user.username=$user"
	uci set "rpcd.$user.password=\$p\$root"
	uci add_list "rpcd.$user.read=luci-app-fn-knock"
done
uci add_list rpcd.writer.write=luci-app-fn-knock
uci commit rpcd
for user in reader writer; do
	login="$(ubus call session login "{\"username\":\"$user\",\"password\":\"\"}")"
	sid="$(printf '%s' "$login" | jsonfilter -e '@.ubus_rpc_session')"
	[ -n "$sid" ] || fail 'ACL test login failed'
	access="$(ubus call session access "{\"ubus_rpc_session\":\"$sid\",\"scope\":\"file\",\"object\":\"/usr/libexec/fn-knock-firewall status\",\"function\":\"exec\"}")"
	printf '%s' "$access" | grep -q true || fail 'status ACL denied'
	result="$(ubus call file exec "{\"ubus_rpc_session\":\"$sid\",\"command\":\"/usr/libexec/fn-knock-firewall\",\"params\":[\"status\"]}")"
	[ "$(printf '%s' "$result" | jsonfilter -e '@.code')" = 0 ] || fail 'LuCI status execution failed'
	output="$(printf '%s' "$result" | jsonfilter -e '@.stdout')"
	jshn -r "$output" >/dev/null || fail 'invalid LuCI status output'
	access="$(ubus call session access "{\"ubus_rpc_session\":\"$sid\",\"scope\":\"file\",\"object\":\"/usr/libexec/fn-knock-firewall allow wan 7999\",\"function\":\"exec\"}")"
	if [ "$user" = writer ]; then expected=true; else expected=false; fi
	printf '%s' "$access" | grep -q "$expected" || fail 'incorrect allow ACL'
	access="$(ubus call session access "{\"ubus_rpc_session\":\"$sid\",\"scope\":\"uci\",\"object\":\"firewall\",\"function\":\"write\"}")"
	printf '%s' "$access" | grep -q false || fail 'broad firewall write access granted'
	ubus call session destroy "{\"ubus_rpc_session\":\"$sid\"}" >/dev/null
done

REAL_UBUS="$(command -v ubus)"
export REAL_UBUS
cat >/tmp/test-ubus <<'SH'
#!/bin/sh
case "$*" in
  *'call uci commit '*) [ ! -f /tmp/fail-commit ] || exit 1 ;;
  *'call uci add '*|*'call uci set '*) [ ! -f /tmp/fail-write ] || exit 1 ;;
esac
exec "$REAL_UBUS" "$@"
SH
cat >/tmp/test-firewall <<'SH'
#!/bin/sh
[ "$1" = reload ] || exit 1
echo reload >>/tmp/reloads
[ ! -f /tmp/slow-reload ] || sleep 2
[ ! -f /tmp/fail-reload ]
SH
chmod +x /tmp/test-ubus /tmp/test-firewall
# Fault injection belongs only in a temporary test copy, not production env.
sed -e 's|^UBUS_BIN=ubus$|UBUS_BIN=/tmp/test-ubus|' \
    -e 's|^FIREWALL_INIT=/etc/init.d/firewall$|FIREWALL_INIT=/tmp/test-firewall|' \
    "$HELPER" >/tmp/test-helper
chmod +x /tmp/test-helper
HELPER=/tmp/test-helper
cat >/tmp/untrusted-jshn <<'SH'
touch /tmp/environment-executed
SH
FN_KNOCK_FIREWALL_JSHN_LIB=/tmp/untrusted-jshn \
FN_KNOCK_FIREWALL_UBUS_BIN=/missing \
FN_KNOCK_FIREWALL_INIT=/missing "$HELPER" status >/tmp/result
[ ! -e /tmp/environment-executed ] || fail 'environment selected executable code'
jshn -R /tmp/result >/dev/null || fail 'environment affected status'

reset_rule
cp /etc/config/firewall /tmp/before
"$HELPER" status >/tmp/result
cmp /etc/config/firewall /tmp/before || fail 'status mutated firewall'
jshn -R /tmp/result >/dev/null || fail 'invalid status JSON'
grep -q 'absent' /tmp/result || fail 'status absent'
[ ! -s /tmp/reloads ] || fail 'status reloaded'

"$HELPER" allow wan 7999
assert_value name Allow-FnKnock-Gateway
assert_value src wan
assert_value proto tcp
assert_value dest_port 7999
assert_value target ACCEPT
assert_value family any
assert_value enabled 1
[ "$(uci show firewall | grep -c 'fn_knock_gateway_ingress=rule')" = 1 ] || fail duplicate
cp /etc/config/firewall /tmp/before
"$HELPER" allow wan 7999
cmp /etc/config/firewall /tmp/before || fail 'repeat changed configuration'
[ "$(wc -l </tmp/reloads)" = 2 ] || fail 'repeat did not retry reload'

# Real rpcd session and default CLI deltas both remain staged, not committed.
SID="$(ubus call session create '{"timeout":120}' | jsonfilter -e '@.ubus_rpc_session')"
ubus call session grant "{\"ubus_rpc_session\":\"$SID\",\"scope\":\"uci\",\"objects\":[[\"firewall\",\"read\"],[\"firewall\",\"write\"]]}" >/dev/null
ubus call uci add "{\"ubus_rpc_session\":\"$SID\",\"config\":\"firewall\",\"type\":\"rule\",\"name\":\"pending_luci\"}" >/dev/null
uci set firewall.pending_cli=rule
set_port 8888
cp /etc/config/firewall /tmp/before
reloads="$(wc -l </tmp/reloads)"
expect_error pending_firewall allow wan 8888
cmp /etc/config/firewall /tmp/before || fail 'pending CLI changes mutated committed config'
[ "$(wc -l </tmp/reloads)" = "$reloads" ] || fail 'pending CLI changes were activated'
! grep -q pending_ /etc/config/firewall || fail 'committed another session delta'
uci changes firewall | grep -q pending_cli || fail 'lost CLI delta'
ubus call uci changes "{\"ubus_rpc_session\":\"$SID\",\"config\":\"firewall\"}" | grep -q pending_luci || fail 'lost LuCI delta'
uci revert firewall
"$HELPER" allow wan 8888
assert_value dest_port 8888
! grep -q pending_ /etc/config/firewall || fail 'committed LuCI delta'
ubus call uci changes "{\"ubus_rpc_session\":\"$SID\",\"config\":\"firewall\"}" | grep -q pending_luci || fail 'lost LuCI delta after allow'
ubus call session destroy "{\"ubus_rpc_session\":\"$SID\"}" >/dev/null

uci set firewall.external=zone
uci set firewall.external.name=external
uci set firewall.external.input=REJECT
uci commit firewall
"$HELPER" allow external 8888
assert_value src external
"$HELPER" status >/tmp/result
grep -q external /tmp/result || fail 'missing custom zone'
# Zone discovery does not assume a wan section or create one automatically.
uci set firewall.@zone[1].name=uplink
uci commit firewall
"$HELPER" status >/tmp/result
jshn -R /tmp/result >/dev/null || fail 'invalid custom-zone status'
! jsonfilter -i /tmp/result -e '@.zones[*]' | grep -qx wan || fail 'invented wan zone'
expect_error invalid_zone allow wan 8888
"$HELPER" allow uplink 8888
assert_value src uplink
uci set firewall.@zone[1].name=wan
uci commit firewall

cp /etc/config/firewall /tmp/before
expect_error invalid_zone allow nonexistent 8888
expect_error invalid_zone allow 'wan";touch /tmp/injected' 8888
expect_error port_changed allow wan 7999
expect_error invalid_arguments sync
expect_error invalid_arguments remove
cmp /etc/config/firewall /tmp/before || fail 'invalid request mutated configuration'
[ ! -e /tmp/injected ] || fail injection

for extra in src_ip dest weekdays ipset future_option; do
	uci set "firewall.fn_knock_gateway_ingress.$extra=restriction"
	uci commit firewall
	cp /etc/config/firewall /tmp/before
	expect_error conflict allow wan 8888
	cmp /etc/config/firewall /tmp/before || fail 'overwrote restriction'
	uci delete "firewall.fn_knock_gateway_ingress.$extra"
	uci commit firewall
done
uci set firewall.fn_knock_gateway_ingress.name=UserRule
uci commit firewall
expect_error conflict allow wan 8888
reset_rule
for port in 0 65536 garbage '1;echo bad'; do
	set_port "$port"
	expect_error invalid_port allow wan 7999
done
set_port 07999
"$HELPER" allow wan 7999
assert_value dest_port 7999
set_port 7999

reset_rule
touch /tmp/fail-write
expect_error write_failed allow wan 7999
rm /tmp/fail-write
[ ! -s /tmp/reloads ] || fail 'write failure reloaded'
! uci -q get firewall.fn_knock_gateway_ingress || fail 'write failure leaked rule'
touch /tmp/fail-commit
expect_error commit_failed allow wan 7999
rm /tmp/fail-commit
! uci -q get firewall.fn_knock_gateway_ingress || fail 'commit failure leaked rule'
[ ! -d /var/run/fn-knock-firewall.lock.d ] || fail 'leaked lock'
touch /tmp/fail-reload
expect_error reload_failed allow wan 7999
assert_value dest_port 7999
rm /tmp/fail-reload
"$HELPER" allow wan 7999
[ "$(wc -l </tmp/reloads)" = 2 ] || fail 'missing reload retry'

mkdir /var/run/fn-knock-firewall.lock.d
expect_error busy allow wan 7999
rmdir /var/run/fn-knock-firewall.lock.d
touch /tmp/slow-reload
"$HELPER" allow wan 7999 >/tmp/first &
first=$!
sleep 1
"$HELPER" allow external 7999 >/tmp/second &
second=$!
wait "$first"
wait "$second"
rm /tmp/slow-reload
assert_value src external
[ "$(uci show firewall | grep -c 'fn_knock_gateway_ingress=rule')" = 1 ] || fail 'concurrent duplicate'

mv /tmp/test-firewall /tmp/test-firewall-disabled
expect_error unavailable allow wan 7999
mv /tmp/test-firewall-disabled /tmp/test-firewall
# Compile the standard UCI rule with the installed backend, without modifying
# the host/container packet filter. Kernel ingress/reboot tests need a router.
"$HELPER" allow wan 7999 >/dev/null
if command -v fw4 >/dev/null; then
	fw4 print >/tmp/compiled 2>/tmp/compile-errors || { cat /tmp/compile-errors; fail 'fw4 compile'; }
	grep -q 'tcp dport 7999.*Allow-FnKnock-Gateway' /tmp/compiled || fail 'missing fw4 rule'
elif command -v fw3 >/dev/null && grep -q filter /proc/net/ip_tables_names; then
	fw3 print >/tmp/compiled 2>/tmp/compile-errors || { cat /tmp/compile-errors; fail 'fw3 compile'; }
	grep -q 'Allow-FnKnock-Gateway' /tmp/compiled || { cat /tmp/compiled /tmp/compile-errors; fail 'missing fw3 rule'; }
else
	echo 'SKIP: fw3 rule generation requires kernel iptables filter support, unavailable in this container.'
fi
echo 'OpenWrt real ubus/rpcd/UCI firewall tests passed'
