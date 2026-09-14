# Terminal metrics

`MetricsService` owns one demand-driven, bounded worker per running session.
The worker shares a cache and Linux CPU baseline among all attachments, sleeps
until requested, and is cancelled and joined when the shell ends. Its collector
never owns the PTY channel. SSH opens a separate exec channel on the authenticated
connection; local sampling keeps the service's effective uid/gid in a separate
process group. No persistent data or credentials are added.

The fixed POSIX script emits raw, delimited sections. Platform parsing and unit
conversion live in pure Rust functions. Missing fields produce null with a
stable reason; Linux memory fallback is explicitly estimated. macOS memory is
active + wired + compressor pages, using the reported page size. Disk percentage
is df's Capacity for `/`, including its reserved-space accounting, not used/total
and not the aggregate APFS container. Capacity above 100% is clamped to the API's
0–100 range, so an exhausted filesystem remains visible as full.

Responses include `sampleAgeMs`, measured using the server's monotonic clock.
The browser adds its own monotonic elapsed time, rather than comparing wall
clocks across machines. Returning a cached sample does not renew its freshness.
The sampler reads `/proc` using shell builtins, including on BusyBox builds
without a `cat` applet.

Run unit and protocol tests with:

```sh
cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml system::terminal
```

For the opt-in real BusyBox/Dropbear test, start an isolated localhost fixture:

```sh
docker run --rm -d --name fn-terminal-metrics-dropbear -p 127.0.0.1:22229:2222 alpine:3.23 sh -c 'apk add --no-cache dropbear >/dev/null && adduser -D metrics && printf "metrics:metrics-fixture\n" | chpasswd && exec dropbear -R -F -E -p 2222'
# Wait until Dropbear reports it is listening, then:
FN_TERMINAL_SSH_TEST_PORT=22229 cargo test --locked --manifest-path apps/server-admin-rs/Cargo.toml busybox_dropbear_metrics -- --ignored
# Always remove the fixture after testing:
docker stop fn-terminal-metrics-dropbear
```

The fixed test password is only for this disposable, localhost-bound fixture.
