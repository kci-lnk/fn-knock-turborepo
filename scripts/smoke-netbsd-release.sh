#!/bin/sh
# Exercise the actual archive inside NetBSD, including both native executables.
set -eu
[ "$(uname -s)" = NetBSD ] || exit 1
archive=${1:?archive is required}
work=$(mktemp -d /tmp/fn-knock-netbsd-smoke.XXXXXX)
backend_pid=
gateway_pid=
cleanup() {
  code=$?
  trap - EXIT HUP INT TERM
  for pid in "$backend_pid" "$gateway_pid"; do
    [ -z "$pid" ] || kill "$pid" 2>/dev/null || true
  done
  for pid in "$backend_pid" "$gateway_pid"; do
    [ -z "$pid" ] || wait "$pid" 2>/dev/null || true
  done
  if [ "$code" -ne 0 ]; then
    cat "$work/backend.log" "$work/gateway.log" >&2 || true
  fi
  rm -rf "$work"
  exit "$code"
}
trap cleanup EXIT
trap 'exit 1' HUP INT TERM
umask 077
tar -xzf "$archive" -C "$work"
cd "$work/fn-knock"
export FN_KNOCK_RUNTIME_TARGET=netbsd
export FN_KNOCK_DATA_DIR="$work/data"
export FN_KNOCK_GATEWAY_CONFIG_DIR="$work/data"
export FN_KNOCK_SQLITE_PATH="$work/data/fn-knock.sqlite"
export FN_KNOCK_INTERNAL_RPC_TOKEN="$(openssl rand -hex 32)"
export ADMIN_VIEW_HOST=127.0.0.1
export ADMIN_STATIC_PATH="$PWD/ui/www"
export AUTH_STATIC_PATH="$PWD/server-auth-view/dist"
mkdir -p "$FN_KNOCK_DATA_DIR" "$work/waf" "$work/logs"
./bin/go-reauth-proxy -waf-dir "$work/waf" -logs-dir "$work/logs" > "$work/gateway.log" 2>&1 &
gateway_pid=$!
./bin/server-admin-rs > "$work/backend.log" 2>&1 &
backend_pid=$!
attempt=0
ready=0
while [ "$attempt" -lt 60 ]; do
  if curl --fail --silent --max-time 2 http://127.0.0.1:7991/__fn-knock/readyz > "$work/ready.json" &&
     grep -Fq '"ready":true' "$work/ready.json"; then
    ready=1
    break
  fi
  kill -0 "$backend_pid" && kill -0 "$gateway_pid"
  attempt=$((attempt + 1))
  sleep 2
done
[ "$ready" -eq 1 ]
curl --fail --silent http://127.0.0.1:7991/ > "$work/panel.html"
grep -qi '<html' "$work/panel.html"
status=$(curl --silent --output /dev/null --write-out '%{http_code}' http://127.0.0.1:7991/api/admin/runtime-health/debug)
[ "$status" = 401 ]
./bin/server-admin-rs reset-panel-password
printf 'NetBSD archive smoke test passed (native startup, gateway readiness, panel assets, auth gate, reset CLI).\n'
