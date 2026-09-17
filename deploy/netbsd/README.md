# fn-knock for NetBSD 11.0/amd64

This archive contains the native Rust backend, a NetBSD Go gateway, and
prebuilt admin/auth frontends. It needs NetBSD 11.0/amd64; it does not need
Node.js, Rust, Go, or pkgsrc at runtime. No service or firewall is installed.

From the extracted `fn-knock` directory, run as the dedicated service user:

```sh
umask 077
mkdir -p data logs waf
# Generate this only on first installation. Keep it private and reuse it.
if [ ! -f data/internal-rpc-token ]; then
  openssl rand -hex 32 > data/internal-rpc-token
fi
export FN_KNOCK_INTERNAL_RPC_TOKEN="$(cat data/internal-rpc-token)"
export FN_KNOCK_RUNTIME_TARGET=netbsd
export FN_KNOCK_DATA_DIR="$PWD/data"
export FN_KNOCK_GATEWAY_CONFIG_DIR="$PWD/data"
export ADMIN_STATIC_PATH="$PWD/ui/www"
export AUTH_STATIC_PATH="$PWD/server-auth-view/dist"
export ADMIN_VIEW_HOST=127.0.0.1
./bin/go-reauth-proxy -waf-dir "$PWD/waf" -logs-dir "$PWD/logs" &
gateway_pid=$!
trap 'kill "$gateway_pid" 2>/dev/null || true' EXIT HUP INT TERM
./bin/server-admin-rs
```

The management panel is at `http://127.0.0.1:7991`. For remote management,
use an SSH tunnel. Set the initial password before exposing any management
listener. Binding to other interfaces requires your own firewall/access
controls; automatic host-firewall management is unavailable on NetBSD.
Configure proxy/listener ports through the panel as for a generic server.

To reset the panel password, use the same OS user, working directory, and
data-directory environment as the service:

```sh
./bin/server-admin-rs reset-panel-password
```

This clears the password and sessions. Immediately configure a replacement
password through the restricted management connection. The panel's
`/path/to/server-admin-rs` placeholder means this archive's
`bin/server-admin-rs` executable.

Current process RSS, process/thread CPU, and allocator statistics are native
measurements. Memory-map counts and virtual sizes are available; per-map
RSS/PSS/anonymous/dirty/swap/huge-page values are unavailable and shown as
unknown. The API exposes virtual sizes separately in `virtual_memory_maps`,
leaving the legacy resident-metric arrays empty.
Largest anonymous regions are ranked by virtual size.

The build uses Rust 1.96.0 on NetBSD with thin LTO and eight codegen units to
fit hosted CI memory limits. The gateway source revision is in
`GATEWAY_COMMIT`; the product version is in `VERSION`. The release's
`SHA256SUMS` and `release-manifest.json` cover this archive.
