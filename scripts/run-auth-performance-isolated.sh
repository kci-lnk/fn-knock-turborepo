#!/usr/bin/env bash
set -euo pipefail
script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
node_binary="${AUTH_PERF_NODE:-$(command -v node)}"
exec unshare --net -- bash -c '
  set -euo pipefail
  ip link set lo up
  ip addr add 198.18.0.1/32 dev lo
  exec "$@"
' auth-performance "$node_binary" "$script_dir/auth-performance.mjs" "$@"
