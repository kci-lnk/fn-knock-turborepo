#!/bin/sh
set -eu
printf 'nameserver 127.0.0.1\noptions timeout:1 attempts:1\n' > /tmp/fn-knock-ab-20260918/resolv.conf
rm -f /tmp/fn-knock-ab-20260918/dns-fail
exec unshare -mn sh -c '
set -eu
mount --make-rprivate /
mount --bind /tmp/fn-knock-ab-20260918/resolv.conf /etc/resolv.conf
ip link set lo up
python3 /tmp/fn-knock-ab-20260918/dns.py &
dns_pid=$!
/tmp/fn-knock-ab-20260918/upstream > /tmp/fn-knock-ab-20260918/dns-upstream.log 2>&1 &
fixture_pid=$!
trap "kill $dns_pid $fixture_pid 2>/dev/null || true" EXIT INT TERM
python3 /tmp/fn-knock-ab-20260918/dns-matrix.py
'
