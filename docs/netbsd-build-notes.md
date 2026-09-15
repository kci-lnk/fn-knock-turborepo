# Building and running fn-knock on NetBSD (community-tested)

This is **not an officially supported platform** (see the main README's
install table), but the control plane (`server-admin-rs`), the admin/auth
frontends, and the [Go-Reauth-Proxy](https://github.com/kci-lnk/Go-Reauth-Proxy)
gateway all build and run natively on **NetBSD 11.0/amd64**, with a few
platform-specific workarounds noted below. Verified in a `qemux/qemu`
Docker VM (4 vCPU / 8 GB RAM); a fat-LTO release build of `server-admin-rs`
needs roughly that much RAM to avoid getting OOM-killed during the final
link step.

## `netbsd` is a recognized runtime target

`FN_KNOCK_RUNTIME_TARGET=netbsd` is now accepted the same way `linux`,
`docker`, `macos`, etc. are (`normalize_deployment_target` /
`normalize_runtime_target_env`). It gets the same capability set as a
generic `linux` deployment: the admin panel listener unlocks, and the
auth-bridge concurrency profile matches other generic server/desktop
targets. Host-firewall auto-management stays off, exactly as it does for
`linux` — see "Known gaps" below. It is *not* auto-detected from
`target_os`, matching how `linux` itself is opt-in-only in this codebase
(only set by the target-specific install/entrypoint scripts); pass it
explicitly.

## Toolchain (pkgsrc)

Everything here is a binary pkgsrc package except where noted:

```sh
pkgin install rust-bin go protobuf git gmake
```

- `rust-bin` (pre-built Rust) is much faster to install than compiling
  `rust` from source, and was new enough (1.96.0) to satisfy this repo's
  `rust-toolchain`/edition requirements at the time of testing.
- `go` installs to a *versioned* prefix, not directly on `$PATH` — e.g.
  `/usr/pkg/go126/bin/go` for the 1.26.x package. Add that directory to
  `PATH` explicitly.
- `gmake` (GNU make) is required — see the `openssl-sys` note below.

## Building `server-admin-rs`

### `openssl-sys`'s vendored OpenSSL build needs GNU make, not bmake

`cargo build` invokes `openssl-sys`'s vendored build (`openssl-src`), which
shells out to the literal command `make` (it does **not** honor a `MAKE`
environment variable). NetBSD's `/usr/bin/make` is bmake, which chokes on
the GNU-make jobserver flags Cargo passes (`--jobserver-fds=N,M`):

```
make: argument '--jobserver-fds=13,14' to option '-j' must be a positive number
```

Fix: put a `make` -> `gmake` symlink in a directory that's *earlier* on
`PATH` than `/usr/bin`:

```sh
mkdir -p ~/bin
ln -sf /usr/pkg/bin/gmake ~/bin/make
export PATH="$HOME/bin:/usr/pkg/bin:/usr/pkg/sbin:$PATH"
```

### `protoc` resolution

`protoc_bin_vendored` only ships prebuilt `protoc` binaries for
Linux/macOS/Windows. `build.rs` falls back to a `protoc` found on `PATH`
(or an explicit `PROTOC` env var) when no vendored binary exists for the
host platform, so simply having pkgsrc's `protobuf` installed and on
`PATH` is enough — no extra configuration needed.

### Release profile RAM usage

`server-admin-rs`'s release profile uses `lto = "fat"` and
`codegen-units = 1` for a smaller binary. On a memory-constrained VM this
can SIGKILL the final `rustc` codegen/link step with no other error output
(no NetBSD OOM-killer log line, just a bare `signal: 9, SIGKILL`). 8 GB was
comfortably enough for a clean build; 4 GB was not.

With those two fixed, `cargo build --release --manifest-path
apps/server-admin-rs/Cargo.toml` behaves exactly as it does on Linux/macOS.

## Building the Go gateway (`Go-Reauth-Proxy`)

Go officially supports `netbsd/amd64` as a build target, and this repo's
Linux-only code (iptables/netlink-based firewall management) is already
cleanly gated behind build tags with non-Linux stub fallbacks — the same
pattern already exercised for macOS support. A plain native build works
with no changes:

```sh
cd Go-Reauth-Proxy
go build -ldflags="-X go-reauth-proxy/pkg/version.Version=<version> -X go-reauth-proxy/pkg/version.Commit=<commit>" -o server ./cmd/server
```

Set `-X ...version.Commit` to the `gatewayCommit` value in this repo's
`version.json` — `server-admin-rs` refuses to talk to a gateway whose
compiled-in commit string doesn't match (`gateway source commit
mismatch`), as a supply-chain integrity check. Passing the two `-X` flags
above (matching this repo's own `Taskfile.yml`) satisfies it for a manual
build.

## Building the frontend (`server-admin-view`, `server-auth-view`)

Two separate issues, both encountered running `npm run build` in
`apps/server-admin-view` (the same applies to `server-auth-view`):

**1. `turbo` refuses to start at all:**

```
Turborepo detected that you are running: netbsd 64
Turborepo does not presently support your platform.
```

Turbo (the task orchestrator/cache layer) has no NetBSD support and hard
-exits. This doesn't affect the underlying build tools, so just bypass
Turbo and run each app's own script directly instead of
`npx turbo run build --filter=...`:

```sh
cd apps/server-admin-view && npm run build
cd apps/server-auth-view && npm run build
```

**2. A cascade of "native binding not found" errors from Rust-native npm
tools**, one per tool in the pipeline: `rolldown` (bundler), `lightningcss`
(CSS engine), `@tailwindcss/oxide` (Tailwind v4 engine) all fail the same
way:

```
Error: Cannot find native binding ... Unsupported OS: netbsd, architecture: x64
```

None of these ship a NetBSD-native build. Two of them (`rolldown`,
`@tailwindcss/oxide`) already have a WASM/WASI fallback path built into
their loader — it's just never installed automatically, because npm's
`optionalDependencies` platform matching skips it for an unrecognized
platform even though it would work fine. Install the matching wasm32-wasi
package explicitly:

```sh
npm install @rolldown/binding-wasm32-wasi@<version matching your rolldown> \
            @tailwindcss/oxide-wasm32-wasi@<version matching your oxide> \
            --no-save --force
```

`lightningcss` has no such fallback wired into its own loader, but the
same team publishes a separate `lightningcss-wasm` package with a
synchronous WASM init (safe as a drop-in for the native API). It needs one
line patched into `node_modules/lightningcss/node/index.js`'s `catch`
block to `require('lightningcss-wasm')` as a final fallback.

Install all of these **together in one `npm install ... --force`
command** — installing them separately causes each to evict the other's
transitive deps (npm treats them as "extraneous" against the lockfile).

**Unrelated but also hit along the way:** `vue-tsc -b` on this monorepo's
full TS project graph exceeds Node's default V8 heap on memory-constrained
hosts. `NODE_OPTIONS=--max-old-space-size=6144 npm run build` fixes it.

## Running the full stack

Both `server-admin-rs` and the Go gateway need the **same**
`FN_KNOCK_INTERNAL_RPC_TOKEN` (their shared gRPC secret). A few other env
vars matter for a manual/dev run that aren't obvious from the README:

- `FN_KNOCK_RUNTIME_TARGET=netbsd` — `netbsd` is now a recognized
  deployment target (see above), and the browser admin panel listener
  (port 7991) only starts when the target is one of
  `docker|openwrt|linux|netbsd|macos|windows`; left unset, there's no
  listener at all (silently — no error, no 7991 port). Host-firewall
  auto-management stays off either way, same as a plain Linux install —
  see "Known gaps" below.
- `ADMIN_STATIC_PATH` / `AUTH_STATIC_PATH` — point these at the two
  frontend apps' `dist/` output, or the panel serves 200s with no content.

Example (single host, both processes started manually):

```sh
export FN_KNOCK_INTERNAL_RPC_TOKEN=<shared-secret>
export FN_KNOCK_RUNTIME_TARGET=netbsd
export ADMIN_STATIC_PATH=$(pwd)/apps/server-admin-view/dist
export AUTH_STATIC_PATH=$(pwd)/apps/server-auth-view/dist

./Go-Reauth-Proxy/server -waf-dir ./waf -logs-dir ./logs &
./apps/server-admin-rs/target/release/server-admin-rs
```

A successful handshake looks like this in the gateway's JSON log
(`gateway.jsonl`):

```
{"component":"auth_bridge","event":"connected","reason_code":"stream_attached"}
{"component":"auth_bridge","event":"ready","reason_code":"handshake_completed"}
{"component":"gateway_dataplane","event":"listener_bound","reason_code":"proxy_stack_started"}
```

## Known gaps

- No per-thread CPU accounting in the runtime diagnostics panel
  (`thread_cpu_unsupported` stays set) — would need
  `sysctl(KERN_LWP)`/`kvm(3)` bindings, not implemented here.
- No smaps-equivalent memory breakdown (anonymous/file/swap split) — only
  total RSS is reported.
- Host firewall auto-management (`host_firewall_available`) is off, same
  as any other non-`fpk` deployment target — this is a deliberate product
  restriction unrelated to NetBSD (it's also off for a plain Linux
  install), not a missing platform feature.
