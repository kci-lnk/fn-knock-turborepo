#!/bin/sh
set -eu
while pgrep -f '^python3 /tmp/fn-knock-ab-20260918/(matrix|mixed).py' >/dev/null; do sleep 1; done
exec unshare -n sh -c '
set -eu
ip link set lo up
/tmp/fn-knock-ab-20260918/upstream > /tmp/fn-knock-ab-20260918/upstream.log 2>&1 &
fixture_pid=$!
trap "kill $fixture_pid 2>/dev/null || true" EXIT INT TERM
python3 /tmp/fn-knock-ab-20260918/business.py
'
