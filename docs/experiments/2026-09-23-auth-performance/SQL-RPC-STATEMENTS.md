# In-process AuthorizeHttp SQL diagnostic

This diagnostic counts SQLite statement execution starts for **one in-process
AuthorizeHttp complete Rust handler**, in `PreflightAndVerify` mode. It measures
one actual awaited handler invocation; it does not add together the earlier
method-level counts. The Rust baseline is `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`
and the candidate is `5fddf896eaf9b1cf3eb300c08315320498c943b8`.

The boundary begins before the candidate's request context scope and covers the
bridge envelope dispatch, AuthContext/header/URI preparation, configuration and
credential reads, normal-access resolution, preflight, verification, and response
construction. The candidate gets exactly one scope, as in its production bridge
worker; the baseline has no request context implementation. No handler body or
authorization decision is replaced.

It excludes Go, cache lookup, HTTP handling, gRPC encoding/transport, bridge
admission and response queues, upstream proxying, and separately issued
`InspectSubdomainGrant` calls. It is **not SQL per external HTTP request**. A Go
cache-disabled request can use this combined payload for both authorization
stages, but the diagnostic does not claim every Go request consists of only this
RPC. Statement starts include transaction boundaries and repeated executions;
they do not establish statement success, affected rows, data scanned, disk I/O,
or latency. Callback overhead makes timing unsuitable for performance claims.

## Fixtures and successful authorization

Each of three scenarios runs once at each population of 1, 100, and 1,000. Each
scenario/population receives a separate AppState and temporary database, with
that many live TOTP credentials, sessions, and host grants. Exactly one session
owns the public client IP. Credentials restrict access to the protected host.
The fixture asserts the absence of private-network and manual-whitelist bypasses.
`session_ip_mobility_enabled=false`, matching the final remote HTTP matrix.
This differs from the original method diagnostic's mobility-enabled fixture:
its 8,019-to-9 owner-method count is not the SQL count for this RPC or that HTTP
traffic. Mobility-enabled complete-RPC counts are not measured here.

- `session_hit` presents a valid browser session cookie. The observed access source
  must be `browser_session` and the response must have the Login grant kind.
- `grant_hit` presents a valid host grant, bound to an enabled advanced-auth policy,
  matching policy version and group. The response must be `subdomain_rule`,
  `subdomain_rule_allowed`, and the expected group; it must not renew or issue a
  cookie, and the grant state must be `reused`.
- `auto_ip_hit` presents no cookie. It has an auto-whitelist record and one live
  `login_ip_grant` owner with `follow_session`; the auto-whitelist runtime snapshot
  is rebuilt before measurement. The observed source must be `login_ip_grant`.

All preflight results must be non-denying without a redirect/denial reason; every
verify result must be successful, host-authorized, and status 200. A test-only
observer records the actual `AuthAccess.grant_type` during normal response
conversion, so an unrelated successful authorization source cannot pass these
assertions. The owner ID is independently checked during setup, outside capture.
Policy setup, bulk seeding, shadow repair, and warmup are excluded.

## Background work and trace controls

Full verification schedules recent-IP bookkeeping, which can schedule a delayed
common-location rebuild. The fixture does not disable or cancel this work to
obtain smaller counts. After a complete warmup request, it waits until the actual
BackgroundTaskRegistry is empty, with a bounded 20-second wait, then executes the
measured request inside the existing 30-second per-IP write-coalescing interval.
A test-only read of task names and spawn generation verifies that no task starts
during the measured handler. The registry must be empty before and after.
Both pre- and post-handler idle trace controls must observe zero SQL callbacks.
Failure to settle, new tasks, or idle SQL fails the diagnostic and preserves logs.

Consequently, these counts describe the **warmed synchronous handler**. SQL from
a cold request's or periodic first touch's asynchronous bookkeeping is not
included. They are not whole-system SQL attribution. No maintenance workers,
bridge network loop, or remote service is started by the fixture.

The existing trace overlay registers `SQLITE_TRACE_STMT` on all four connections
(primary, analytics, one auth reader, health), on their owning workers. Each
fixture must calibrate to 16 starts (8 reads, 8 transaction statements). The
callback uses unexpanded `sqlite3_sql`, catches panics across FFI, retains
connection handles, and unregisters before fixture destruction. See
[the original method diagnostic](SQL-STATEMENTS.md) for instrumentation details.

## Reproduction

Preparation copies pinned git archives and applies test-only overlays. The
working product checkout and frozen release binaries remain untouched. Use an
exclusive native Cargo target; do not share it with another build while running.
The driver sets `CARGO_INCREMENTAL=0`, requires at least 5 GiB free at its disk
checks, and never removes existing caches. A retry requires a new output directory
so failed evidence is retained.

```sh
python3 scripts/auth-sql-diagnostic.py prepare \
  --repo "$PWD" --boundary rpc \
  --candidate-commit 5fddf896eaf9b1cf3eb300c08315320498c943b8 \
  --output /tmp/fn-knock-auth-sql-rpc-NEW

python3 scripts/auth-sql-diagnostic.py run \
  --output /tmp/fn-knock-auth-sql-rpc-NEW \
  --target "$PWD/apps/server-admin-rs/target" --jobs 2
```

The original `--boundary method` remains the default. Source archives, modified
overlay files, Cargo.lock, toolchains, explicit environments, commands, copied
test executables, and report hashes are recorded in the run manifest. Each
revision executes only the exact diagnostic test in a separate process.

## Results

Both exact diagnostic tests passed: 18 successful measured handlers, 18
calibrations at 16 starts each, and 36 pre/post idle controls at zero. Background
spawn generation stayed unchanged for every measured request. Grant responses
explicitly reported `reused`; all scenarios returned the expected access source.
These are **mobility-disabled, warmed complete-handler** counts.

| Scenario | Population | Baseline total | Candidate total | Baseline reads | Candidate reads |
| --- | ---: | ---: | ---: | ---: | ---: |
| Session hit | 1 | 52 | 31 | 25 | 15 |
| Session hit | 100 | 52 | 31 | 25 | 15 |
| Session hit | 1,000 | 52 | 31 | 25 | 15 |
| Grant hit | 1 | 27 | 12 | 21 | 8 |
| Grant hit | 100 | 324 | 12 | 318 | 8 |
| Grant hit | 1,000 | 3,024 | 12 | 3,018 | 8 |
| auto-IP hit | 1 | 13 | 11 | 5 | 5 |
| auto-IP hit | 100 | 13 | 11 | 5 | 5 |
| auto-IP hit | 1,000 | 15 | 11 | 7 | 5 |

`Total` includes reads, transaction boundaries, and writes. Session authorization
uses 18→14 transaction starts/boundaries and 9→2 write starts; its connection
counts are primary/auth-reader 40/12→9/22. Grant authorization uses 6→4 transaction
statements with no writes, all on the auth reader. auto-IP uses six transaction
statements in both versions and 2→0 write starts; baseline work is on primary,
candidate work on the auth reader. Expiry-guard DELETE attempts can affect no
rows, so write starts are not rows changed or disk writes.

The candidate session's remaining nine primary statements come from successful
verify's `sync_trusted_request` →
[`refresh_proxy_session_binding`](../../../apps/server-admin-rs/src/auth/mobility/trusted_sync.rs#L237)
→ `Store::get_config` →
[`load_shadow`](../../../apps/server-admin-rs/src/storage/typed_config.rs#L212).
Even this healthy fixture, with no mobility binding and mobility disabled, reads
live configuration before deciding no refresh is needed. That read uses
`BEGIN IMMEDIATE`/`COMMIT` (two starts), reads the two legacy config/generation
keys with an expiry-guard DELETE plus kind/value SELECTs (six), and reads the
typed config document (one). The live getter checks authority and legacy/typed
consistency and can trigger repair; this fixture takes its healthy path, without
repair. It still queues on primary and acquires the SQLite writer lock.

Writer-lock tests cover stable session subpaths and authorization metadata, not
the complete `AuthorizeHttp` handler. These results do not establish that the
whole RPC is read-only or unaffected by a writer lock. A future early return for
the no-binding, mobility-disabled case needs separate validation of concurrent
configuration updates and repair timing before replacing the live getter.

The grant template histograms show three baseline grant reads versus two
candidate reads across this combined preflight/verify handler. The baseline's
active-grant compatibility validation scales with the grant population; the
candidate uses joined validation. For this healthy N=1,000 fixture, those full
handler totals are 3,024 and 12, not the earlier getter-only 1,008 and 6.

With mobility disabled, the baseline auto-IP path already batches session loads:
at N=1,000 it performs three `WHERE key IN (...)` reads alongside its key scan.
The candidate uses a session query and a selected-owner authority recheck. Low
statement counts do not make rows visited, JSON parsing, or retained memory
constant. The original mobility-enabled owner-method result of 8,019→9 is a
different configuration and boundary; it must not be attributed to these RPCs or
to the final remote matrix. Complete mobility-enabled RPCs, cache hits in Go,
cold/periodic background touches, expired or malformed credentials, revocation,
and renewal are outside this diagnostic table.

Final evidence is under [results/sql-rpc-statements](results/sql-rpc-statements):

- [Baseline JSON](results/sql-rpc-statements/baseline/statements.json),
  [candidate JSON](results/sql-rpc-statements/candidate/statements.json), and
  [summary](results/sql-rpc-statements/summary.json) contain all count categories,
  SQL templates, connection roles, outcomes, background and calibration controls.
- [Manifest](results/sql-rpc-statements/manifest.json) records both source/archive,
  overlay, native test-binary, report, and command identities. Build JSON streams
  are gzip-compressed; build and test logs are retained per role.
- The first instrumentation run also passed but was superseded. The final run
  moved the post-handler barrier inside the idle capture and added the explicit
  `reused` grant-state assertion. Its predecessor's separate
  [manifest](results/sql-rpc-statements/superseded-instrumentation-v1/manifest.json),
  original overlay and complete reports/logs are retained; its observations are
  not mixed into this table. Neither run encountered a build or assertion failure.
- Cleanup records and compressed before-inventories retain proof for removal of
  only this diagnostic's new native objects/executables: absent before, matching
  Cargo artifact identity, embedded archive source path, and SHA256. Copied test
  binaries remain under `/tmp/fn-knock-auth-sql-rpc-v2-20260923/{baseline,candidate}`;
  existing user caches, dependencies, and frozen release binaries were not removed.

The final native test binary SHA256 values are
`e3a7ead02c40b6db8a96974c61243ce02594b6f9555e899ee9a4c986d37baa63`
(baseline) and
`91bdbba1df424a606c89afa1e7e1bcce7dabb555104727b2434ae432445c30c7`
(candidate). They are diagnostic executables, not the Linux release artifacts.
