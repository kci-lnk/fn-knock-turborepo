#!/usr/bin/env sh
set -eu

exec /opt/fn-knock/bin/server-admin-rs reset-panel-password "$@"
