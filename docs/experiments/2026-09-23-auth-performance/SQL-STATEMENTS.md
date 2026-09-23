# SQLite statement diagnostic

This diagnostic counts SQLite statement execution starts within one awaited
method invocation, including repeated executions or retries if any. A trace
callback does not prove individual statement completion or affected row counts,
although all three enclosing operations must satisfy their success assertions.
It does **not** measure end-to-end SQL per HTTP request,
executor jobs, query plans, rows visited, disk I/O, or latency. The trace callback
adds overhead; its timings must not be used as performance results.

## Boundaries and fixture

The baseline is `d4f8805f39d9f4480bf83f4382ba59e5f49e7dc4`; the corrected candidate
is `5fddf896eaf9b1cf3eb300c08315320498c943b8`. The driver requires the candidate's
full frozen commit SHA explicitly. Each revision gets a separate archive,
test binary, exact-test process, temporary database, logs, and JSON report.
The driver never edits the working checkout or builds a production binary.

For each population of 1, 100, and 1,000, the fixture has that many live browser
sessions, TOTP credentials, grants for one protected host, and fresh active-IP
details. Exactly one session owns the requested IP. The selected session has a
canonical IP, nonempty location, a valid session cookie, and explicit permission
for the protected host. Mobility is enabled with a 1,200-second window; live
sessions and grants have a one-hour lifetime. All credentials are synthetic.

| Operation | Exact measured boundary | Success assertion |
| --- | --- | --- |
| `healthy_grant_get` | `Store::get_string_value_auth(grant_key)` | Existing grant returned |
| `valid_session_normal_access` | `resolve_preflight_normal_access` with `LoginFirst` | Authorized as `browser_session`, not a local/whitelist bypass |
| `auto_ip_owner_lookup` | `list_active_sessions_by_ip` | Exactly the selected session returned |

Configuration save, bulk fixture creation, initialization, shadow repair, and one
warmup invocation of each method are outside the windows. Candidate revisions
with `auth::request_context` receive a fresh request scope for each invocation;
baseline revisions without it directly await the same operation. No session or
authorization result is reused across measured invocations.

These boundaries omit Go handling, bridge/RPC dispatch, other preflight phases,
and full verification's recent-IP background work. They therefore complement,
but cannot be substituted for, the experiment's executor-job measurements or a
future per-request distributed trace. Healthy data is intentionally tested;
expiry, corruption repair, revocation, and renewal have other SQL paths.
The grant fixture tests a healthy storage/shadow read; it does not configure an
advanced-auth policy or test complete grant authorization.

## Instrumentation and safety

The existing `tokio-rusqlite` reexport of `rusqlite::ffi` exposes
`sqlite3_trace_v2`; no Cargo feature or dependency change is required. An archive
overlay registers `SQLITE_TRACE_STMT` after connection initialization, on each
connection's owning worker. Reader count is fixed to one, so the four registered
connections are `primary`, `analytics`, `auth_read`, and `health`.
The overlay also adds the same test-only alias for the owner-query function in
both revisions, because the baseline's private module has no top-level export.
Fixture configuration uses the repository's existing test-only `replace_config`
API; no product method implementation is replaced.

Before measurement, calibration starts and commits a read transaction on each
connection and executes the **same prepared** `SELECT ?1` twice. It must observe
16 statement executions: eight reads and eight transaction statements, four per
connection. This verifies that counts reflect executions, not preparation.
An idle control after warmup must observe zero callbacks. Only one exact test is
run, with one test-harness thread; the runtime has two worker threads. AppState
initialization is awaited, and this fixture does not start maintenance workers.

The callback copies `sqlite3_sql`'s unexpanded SQL template, never expanded bound
values. Its state has static lifetime; its context is a role number, not a
pointer into movable Rust data. It catches panics at the C boundary. Connection
handles remain retained until callbacks are unregistered on their owning
workers; unregistration precedes fixture/DB destruction, including ordinary
operation error results. Registration failures clear callbacks already installed
on earlier connections. If unregistering fails, retained handles and static
callback state remain alive and the test fails without an accepted report.
A panic fails the diagnostic rather than producing an accepted report; static
state cannot become a dangling pointer.

Each report includes every statement, including `BEGIN`, `COMMIT`, and writes.
Classification uses transaction keywords first, then `PRAGMA`, then SQLite's
`sqlite3_stmt_readonly`. SQLite's possible trigger-subprogram callbacks are
separately classified and excluded from `statement_executions`; these fixtures
assert that there are none. SQL templates have whitespace normalized only.
The JSON contains category totals, connection totals, and complete template
histograms, plus calibration and idle-control evidence.

## Reproduce

Use an exclusive **native** Cargo target directory, not the shared Linux
cross-compilation target. Existing dependency artifacts may be reused. The
driver requires at least 5 GiB free at each output/target check and never deletes
existing caches. The output directory must be new; failed runs are preserved,
and a fresh output directory is required to retry.

```sh
python3 scripts/auth-sql-diagnostic.py prepare \
  --repo "$PWD" \
  --candidate-commit 5fddf896eaf9b1cf3eb300c08315320498c943b8 \
  --output /tmp/fn-knock-sql-statements

python3 scripts/auth-sql-diagnostic.py run \
  --output /tmp/fn-knock-sql-statements \
  --target "$PWD/apps/server-admin-rs/target" \
  --jobs 2
```

`prepare` does not compile. `run` performs two sequential locked native test
builds and immediately copies each test executable before building the next
revision. `manifest.json` records full source SHAs, source-archive hashes,
overlay/driver/tree hashes, Cargo.lock hashes, toolchain versions, complete build
and test commands, explicit diagnostic environment, binary hashes and sizes,
and raw-report hashes. `baseline/statements.json` and
`candidate/statements.json` are the primary observations. Build/test logs remain
beside each binary. No production or remote instance is traced or modified.

## Results

Both isolated tests passed. All six calibrations observed exactly 16 starts;
all six idle controls observed zero. Each table entry is one successful warmed
method invocation. `Total` includes reads, writes, and transaction statements;
`Reads` is the read-only non-transaction subset.

| Method | Population | Baseline total | Candidate total | Baseline reads | Candidate reads |
| --- | ---: | ---: | ---: | ---: | ---: |
| Grant GET | 1 | 9 | 6 | 7 | 4 |
| Grant GET | 100 | 108 | 6 | 106 | 4 |
| Grant GET | 1,000 | 1,008 | 6 | 1,006 | 4 |
| Session normal access | 1 | 40 | 13 | 19 | 7 |
| Session normal access | 100 | 40 | 13 | 19 | 7 |
| Session normal access | 1,000 | 40 | 13 | 19 | 7 |
| autoIP owner lookup | 1 | 25 | 9 | 10 | 5 |
| autoIP owner lookup | 100 | 817 | 9 | 208 | 5 |
| autoIP owner lookup | 1,000 | 8,019 | 9 | 2,010 | 5 |

The grant baseline repeatedly loads each active member's compatibility record;
at N=1,000 the same live-string query starts 1,002 times. The candidate's combined
read and joined active-index validation need four reads plus one transaction's
`BEGIN`/`COMMIT`, independent of this fixture's population. This does **not** make
the amount of data scanned or JSON parsed constant; the same distinction applies
to credential parsing and batched IP discovery.

The session baseline uses 19 reads, 14 transaction statements, and seven writes,
with 32 starts on `primary` and eight on `auth_read`. The candidate uses seven
reads and six transaction statements, all on `auth_read`, with no write starts.

At N=1,000, the owner lookup baseline uses 2,010 reads, 4,006 transaction
statements, and 2,003 writes, all on `primary`. Most writes are expiry-guard
`DELETE` attempts and can affect zero rows; this is not evidence of 2,003 changed
records or disk writes. The candidate uses five reads and four transaction
statements on `auth_read`, with no write starts. Candidate discovery is batched
and the one matching session still receives a fresh authority check. More
matching owners require more confirmation work, so nine is not a universal
count for all IP populations.

Raw evidence:

- [Baseline JSON](results/sql-statements/baseline/statements.json) and
  [candidate JSON](results/sql-statements/candidate/statements.json) contain every
  template, category, connection, success outcome, calibration, and idle control.
- [Summary JSON](results/sql-statements/summary.json) contains the table's
  source values; [manifest](results/sql-statements/manifest.json) pins both
  source/binary identities and commands. Successful binaries remain at
  `/tmp/fn-knock-sql-diagnostic-final-v4-20260923/{baseline,candidate}/server-admin-rs-sql-diagnostic.test`.
- [Baseline log](results/sql-statements/baseline/test.log) and
  [candidate log](results/sql-statements/candidate/test.log) each show one passed
  exact test. Their 17.49 s / 13.19 s durations include fixture creation, repair,
  warmup, calibration, and tracing; they are **not latency benchmarks**.
- Compressed Cargo JSON logs retain build artifact ownership and compiler
  identity. Native builds took 98.63 s and 110.30 s; dependency artifacts were
  reused, with application/path dependencies rebuilt at each archive path.

Three failed diagnostic attempts are retained under
[failed-attempts](results/sql-statements/failed-attempts): v1 failed compilation
because the baseline function needed a test-only export; v2 failed fixture setup
because a configuration generation marker was absent; v3 reached the separate
host-mapping CAS restriction. The final fixture uses the existing test replacement
API. These failures occurred before measured operations and contribute no counts.
Their manifests, compiler/test logs, and actual operation overlays are retained.

## Build-artifact cleanup

The completed/failed archives generated about 9.58 GB of incremental files and
0.98 GB of explicit target artifacts. The task cleaned only those confirmed as
its own: Cargo JSON's archive package ID and `fresh=false` establish artifact
ownership; incremental directories additionally have an object-file source-path
match to one of these archives, with object hash and byte offset recorded. No
directory was selected from modification time alone. An incomplete directory
without source-path evidence was left untouched.

All copied test binaries, dependency copies, archives, manifests, raw counts,
and logs remain available. The
[before inventory](results/sql-statements/cleanup-inventory-before.json) and
[after inventory](results/sql-statements/cleanup-inventory-after.json) record
paths, sizes, ownership proof, copy hashes, and verified removals. Available disk
space after cleanup was 12,390,273,024 bytes (about 11.54 GiB). Logical file bytes
removed differ from physical space reclaimed on this filesystem.
