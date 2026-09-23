import test from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import {
  marker,
  requestSpec,
  validateResponse,
  mergeMeasurements,
  pairOrder,
  compareRuns,
  comparisonFailures,
  benchmarkEnvironment,
} from "../auth-performance-lib.mjs";
import { summarizeOperationProfile } from "../auth-performance-profile.mjs";
import {
  captureFailureSnapshot,
  runRecoveryProbe,
} from "../auth-performance-recovery.mjs";

test("bridge capacity uses product defaults unless the variant explicitly overrides it", () => {
  const name = "FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT";
  assert.equal(benchmarkEnvironment({}, {})[name], undefined);
  assert.equal(
    benchmarkEnvironment({ [name]: "32", KEEP: "yes" }, {})[name],
    undefined,
  );
  assert.equal(benchmarkEnvironment({ KEEP: "yes" }, {}).KEEP, "yes");
  assert.equal(
    benchmarkEnvironment({ [name]: "1024" }, { [name]: "32" })[name],
    "32",
  );
  assert.equal(benchmarkEnvironment({}, { [name]: "256" })[name], "256");
});

test("a valid session/grant must reach the owned origin, never count login redirects as work", () => {
  for (const scenario of ["session_hit", "grant_hit", "auto_ip_hit"]) {
    const spec = requestSpec(scenario);
    assert.ok(
      validateResponse(spec, {
        status: 302,
        headers: { location: "/login" },
        body: "",
      }),
    );
    assert.ok(
      validateResponse(spec, {
        status: 200,
        headers: {},
        body: "<html>login</html>",
      }),
    );
    assert.equal(
      validateResponse(spec, {
        status: 200,
        headers: { "x-authperf-origin": marker },
        body: marker,
      }),
      null,
    );
  }
});

test("renewal is a distinct operation with disjoint one-use warm and load cookies", () => {
  const spec = requestSpec("grant_renewal", 19, "load");
  assert.notEqual(
    spec.headers.cookie,
    requestSpec("grant_renewal", 19, "warm").headers.cookie,
  );
  const response = {
    status: 200,
    headers: { "x-authperf-origin": marker },
    body: marker,
  };
  assert.ok(validateResponse(spec, response));
  response.headers["set-cookie"] = [
    "fn-knock-subdomain-rule-grant=authperf-grant-load-19; Max-Age=3600",
  ];
  assert.equal(validateResponse(spec, response), null);
});

test("bootstrap verifies non-loopback source and session API verifies authenticated state", () => {
  const response = {
    status: 200,
    headers: {},
    body: JSON.stringify({
      success: true,
      data: { client: { ip: "127.0.0.1" } },
    }),
  };
  assert.ok(validateResponse(requestSpec("bootstrap"), response));
  response.body = JSON.stringify({
    success: true,
    data: { client: { ip: "198.18.0.1" } },
  });
  assert.equal(validateResponse(requestSpec("bootstrap"), response), null);
  response.body = JSON.stringify({
    success: true,
    data: { auth: { authenticated: false } },
  });
  assert.ok(validateResponse(requestSpec("session_api"), response));
});

test("all worker latency samples and counters are aggregated, not averages of percentiles", () => {
  const row = {
    elapsed_ms: 1000,
    requests: 2,
    failures: 0,
    statuses: { 200: 2 },
    anomalies: [],
    histogram: [0, 2],
    event_loop_delay_max_ms: 4,
    start_delay_ms: 1,
    all_tokens_consumed: false,
  };
  const merged = mergeMeasurements([
    row,
    { ...row, elapsed_ms: 1100, histogram: [0, 0, 1, 1], failures: 1 },
  ]);
  assert.equal(merged.requests, 4);
  assert.equal(merged.p50_ms, 1);
  assert.equal(merged.p99_ms, 3);
  assert.equal(merged.successful_requests, 3);
  assert.equal(merged.requests_per_second, 4000 / 1100);
  assert.equal(merged.statuses[200], 4);
});

test("paired comparison alternates order and rejects incomplete or invalid evidence", () => {
  assert.deepEqual(pairOrder(0), ["baseline", "candidate"]);
  assert.deepEqual(pairOrder(1), ["candidate", "baseline"]);
  const runs = [];
  for (let pair = 0; pair < 6; pair++)
    for (const role of pairOrder(pair))
      runs.push({
        pair,
        role,
        candidate: "opt",
        scenario: "session_hit",
        concurrency: 16,
        cache_ttl_seconds: 0,
        validation: { passed: true },
        measurement: {
          successful_requests_per_second: role === "baseline" ? 100 : 120,
          p99_ms: role === "baseline" ? 30 : 20,
        },
      });
  let [comparison] = compareRuns(runs);
  assert.equal(comparison.six_valid_pairs, true);
  assert.ok(Math.abs(comparison.throughput_change_median - 0.2) < 1e-10);
  assert.deepEqual(comparison.throughput_change_bootstrap_95, [
    comparison.throughput_change_median,
    comparison.throughput_change_median,
  ]);
  runs[0].validation.passed = false;
  [comparison] = compareRuns(runs);
  assert.equal(comparison.six_valid_pairs, false);
  [comparison] = compareRuns([{ ...runs[0], measurement: undefined }]);
  assert.equal(comparison.incomplete_pairs, 1);
});

function gateRuns({
  throughput = [1.1],
  p99 = [1],
  go = [60],
  rust = [45],
  pairs = 6,
} = {}) {
  return Array.from({ length: pairs }, (_, pair) =>
    pairOrder(pair).map((role) => ({
      pair,
      role,
      candidate: "opt",
      scenario: "session_hit",
      concurrency: 16,
      cache_ttl_seconds: 0,
      seed: {
        sessions: 64,
        accounts: 1,
        ordinary_grants: 2,
        total_grants: 2,
        renewal_tokens_per_phase: 0,
      },
      validation: { passed: true },
      measurement: {
        successful_requests_per_second:
          100 *
          (role === "baseline" ? 1 : throughput[pair % throughput.length]),
        p99_ms: 100 * (role === "baseline" ? 1 : p99[pair % p99.length]),
      },
      resources: {
        go: { peak_rss_bytes: role === "baseline" ? 50 : go[pair % go.length] },
        rust: {
          peak_rss_bytes: role === "baseline" ? 50 : rust[pair % rust.length],
        },
      },
    })),
  ).flat();
}

test("six-pair gates apply paired combined RSS growth and report individual process changes", () => {
  const runs = gateRuns();
  let comparison = compareRuns(runs);
  assert.deepEqual(
    comparisonFailures(comparison, { requireSixPairs: true }),
    [],
  );
  assert.ok(Math.abs(comparison[0].peak_rss_change_median.go - 0.2) < 1e-12);
  assert.ok(Math.abs(comparison[0].peak_rss_change_median.rust + 0.1) < 1e-12);
  assert.ok(
    Math.abs(comparison[0].peak_rss_change_median.combined - 0.05) < 1e-12,
  );
  assert.equal(comparison[0].rss_complete_pairs, 6);
  comparison = compareRuns(gateRuns({ go: [70, 70, 60, 60, 60, 60] }));
  assert.deepEqual(
    comparisonFailures(comparison, { requireSixPairs: true }),
    [],
  );
  comparison = compareRuns(gateRuns({ go: [60.01] }));
  assert.match(
    comparisonFailures(comparison, { requireSixPairs: true }).join("\n"),
    /RSS median growth exceeded/,
  );
  delete runs[0].resources.rust;
  comparison = compareRuns(runs);
  assert.equal(comparison[0].peak_rss_change_median.combined, null);
  assert.equal(comparison[0].rss_complete_pairs, 5);
  assert.match(
    comparisonFailures(comparison, { requireSixPairs: true }).join("\n"),
    /missing or invalid.*RSS/,
  );
  assert.deepEqual(
    comparisonFailures(comparison),
    [],
    "smoke does not require RSS or six pairs",
  );
});

test("improvement gate accepts either target only with a directional paired interval", () => {
  const check = (options) =>
    comparisonFailures(compareRuns(gateRuns(options)), {
      requireImprovement: true,
    });
  assert.deepEqual(check(), [], "exact 10% throughput target passes");
  assert.deepEqual(
    check({ throughput: [1.09], p99: [0.85] }),
    [],
    "exact 15% P99 target passes",
  );
  assert.match(
    check({ throughput: [1.09], p99: [0.86] }).join("\n"),
    /improvement target not established/,
  );
  assert.match(
    check({ throughput: [0.8, 0.9, 1.1, 1.2, 1.3, 1.4] }).join("\n"),
    /improvement target not established/,
  );
  assert.match(
    check({ throughput: [1, 1, 1.1, 1.1, 1.2, 1.2] }).join("\n"),
    /improvement target not established/,
    "an interval lower bound of exactly zero fails",
  );
  assert.match(
    check({ throughput: [1], p99: [0.5, 0.6, 0.7, 0.9, 1.1, 1.2] }).join("\n"),
    /improvement target not established/,
  );
  assert.match(check({ pairs: 5 }).join("\n"), /six valid complete pairs/);
  assert.match(check({ go: [61] }).join("\n"), /RSS median growth exceeded/);
  assert.match(check({ p99: [1.11] }).join("\n"), /regression budget exceeded/);
});

test("comparison groups cannot combine different session, account, grant or renewal scales", () => {
  const seedFields = [
    "sessions",
    "accounts",
    "ordinary_grants",
    "total_grants",
    "renewal_tokens_per_phase",
  ];
  const original = gateRuns();
  const runs = [
    ...original,
    ...seedFields.flatMap((field) =>
      original.map((run) => ({
        ...run,
        seed: { ...run.seed, [field]: run.seed[field] + 1 },
      })),
    ),
  ];
  const comparison = compareRuns(runs);
  assert.equal(comparison.length, 6);
  assert.ok(comparison.every((group) => group.complete_pairs === 6));
  const mismatched = gateRuns({ pairs: 1 });
  mismatched[0].seed.accounts++;
  assert.equal(compareRuns(mismatched).length, 2);
  assert.match(
    comparisonFailures(compareRuns(mismatched)).join("\n"),
    /incomplete pairs/,
  );
  const kinds = ["totp", "password"].flatMap((credential_kind) =>
    original.map((run) => ({ ...run, seed: { ...run.seed, credential_kind } })),
  );
  assert.equal(compareRuns(kinds).length, 2);
  assert.ok(compareRuns(kinds).every((group) => group.complete_pairs === 6));
});

test("check CLI keeps smoke permissive while optional gates reject missing RSS and uncertain improvement", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "auth-performance-gate-"),
  );
  try {
    const file = path.join(directory, "results.json");
    const check = fileURLToPath(
      new URL("../check-auth-performance.mjs", import.meta.url),
    );
    const run = (...flags) =>
      spawnSync(process.execPath, [check, file, ...flags], {
        encoding: "utf8",
      });
    await writeFile(
      file,
      JSON.stringify({ schema_version: 1, runs: gateRuns() }),
    );
    assert.equal(run("--require-improvement").status, 0);
    assert.notEqual(run("--unknown").status, 0);
    const runs = gateRuns({ throughput: [1.01] });
    await writeFile(file, JSON.stringify({ schema_version: 1, runs }));
    assert.equal(run("--require-six-pairs").status, 0);
    assert.notEqual(run("--require-improvement").status, 0);
    delete runs[0].resources;
    await writeFile(file, JSON.stringify({ schema_version: 1, runs }));
    assert.equal(run().status, 0);
    assert.notEqual(run("--require-six-pairs").status, 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("operation profile distinguishes executor jobs, admissions, background rate and missing instrumentation", () => {
  const operation = {
    kind: "sqlite_primary",
    calls: 20,
    total_wall_ms: 40,
    total_cpu_ms: 10,
    in_flight: 0,
  };
  const report = {
    capture: {
      operations: {
        elapsed_ms: 1000,
        dropped_operations: 0,
        operations: [
          operation,
          { ...operation, kind: "sqlite_auth_read", calls: 10 },
          {
            ...operation,
            kind: "sqlite_admission",
            calls: 30,
            total_wall_ms: 60,
          },
        ],
      },
    },
  };
  const idle = {
    capture: {
      operations: {
        elapsed_ms: 2000,
        operations: [{ ...operation, calls: 4 }],
      },
    },
  };
  const summary = summarizeOperationProfile(report, 10, idle);
  assert.equal(summary.executor_calls, 30);
  assert.equal(summary.executor_calls_per_success, 3);
  assert.equal(summary.admission_calls_per_success, 3);
  assert.equal(summary.admission_wall_ms_per_success, 6);
  assert.equal(summary.idle_executor_calls_per_second, 2);
  assert.equal(summary.auth_reader.executor_calls, 10);
  assert.equal(summary.auth_reader.admission_instrumentation_observed, false);
  assert.equal(summary.auth_reader.admission_wall_ms_per_success, null);
  assert.match(summary.measurement, /not SQL statement count/);
  report.capture.operations.operations = [operation];
  assert.equal(
    summarizeOperationProfile(report, 10).admission_calls_per_success,
    null,
  );
  assert.equal(
    summarizeOperationProfile(report, 10).auth_reader
      .executor_calls_per_success,
    null,
  );
});

function mixedReaderProfile() {
  const operation = (kind, label, calls, wall, cpu, inFlight = 0) => ({
    kind,
    label,
    calls,
    total_wall_ms: wall,
    total_cpu_ms: cpu,
    in_flight: inFlight,
  });
  const report = (operations, elapsed = 1000) => ({
    capture: {
      operations: { elapsed_ms: elapsed, dropped_operations: 0, operations },
    },
  });
  return summarizeOperationProfile(
    report([
      operation("sqlite_primary", "write", 20, 200, 100, 4),
      operation("sqlite_auth_read", "read_accounts", 6, 60, 20, 1),
      operation("sqlite_auth_read", "read_sessions", 4, 40, 10),
      operation("sqlite_admission", "sqlite_primary", 20, 400, null),
      operation("sqlite_admission", "sqlite_auth_read", 10, 30, null, 2),
      operation("sqlite_admission", "sqlite_health", 2, 90, null),
      operation("sqlite_analytics", "read_analytics", 100, 900, 500),
    ]),
    10,
    report(
      [
        operation("sqlite_primary", "write", 4, 80, 20),
        operation("sqlite_auth_read", "read_sessions", 2, 40, 10),
      ],
      2000,
    ),
  );
}

test("auth reader profile aggregates all auth labels without primary or health admissions", () => {
  const summary = mixedReaderProfile();
  // Existing overall fields retain their original aggregation semantics.
  assert.equal(summary.executor_calls, 30);
  assert.equal(summary.executor_wall_ms, 300);
  assert.equal(summary.executor_cpu_ms, 130);
  assert.equal(summary.admission_calls, 32);
  assert.equal(summary.admission_wall_ms, 520);
  assert.equal(summary.idle_executor_calls_per_second, 3);
  assert.equal(summary.unfinished_scopes, 7);
  assert.deepEqual(summary.auth_reader, {
    measurement: "all auth reader slots in the shared operation recorder",
    checkpoint_wait_included: false,
    executor_calls: 10,
    executor_wall_ms: 100,
    executor_cpu_ms: 30,
    admission_calls: 10,
    admission_wall_ms: 30,
    executor_instrumentation_observed: true,
    admission_instrumentation_observed: true,
    unfinished_scopes: 3,
    executor_calls_per_success: 1,
    executor_wall_ms_per_success: 10,
    executor_cpu_ms_per_success: 3,
    admission_calls_per_success: 1,
    admission_wall_ms_per_success: 3,
    idle_executor_calls_per_second: 1,
  });
});

test("profile report separates auth readers and labels health queues as primary for old and new summaries", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "auth-profile-report-"));
  try {
    const runs = gateRuns({ pairs: 1 });
    const summary = mixedReaderProfile();
    const { auth_reader: _authReader, ...legacy } = summary;
    runs[0].operation_profile = legacy;
    runs[1].operation_profile = summary;
    for (const run of runs)
      run.runtime_health = [
        {
          storage: { queue_depth: 7, queue_wait_ms: 8, active_operation_ms: 9 },
        },
      ];
    const file = path.join(directory, "results.json");
    await writeFile(file, JSON.stringify({ schema_version: 1, runs }));
    const output = execFileSync(
      process.execPath,
      [
        fileURLToPath(
          new URL("../report-auth-performance.mjs", import.meta.url),
        ),
        file,
      ],
      { encoding: "utf8" },
    );
    assert.match(output, /auth reader调用\/成功请求/);
    assert.ok(output.includes("| 1.000 | 10.000 | 3.000 | 3.000 | 1.000 |"));
    assert.ok(output.includes("| n/a | n/a | n/a | n/a | n/a |"));
    assert.match(output, /SQLite primary采样最大队列深度 7/);
    assert.match(output, /不代表auth reader单slot或全池/);
    assert.match(output, /不包含中间的checkpoint gate等待/);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("failure health capture preserves the original error even if diagnostics fail", async () => {
  const row = {};
  const original = new Error("warmup returned 503");
  await captureFailureSnapshot(row, original, async () => {
    throw new Error("admin probe timed out");
  });
  assert.equal(row.validation.error, original.stack);
  assert.equal(row.validation.passed, false);
  assert.match(row.failure_health.error, /admin probe timed out/);
  await captureFailureSnapshot(row, original, async () => ({
    status: 200,
    storage: { queue_depth: 32 },
  }));
  assert.equal(row.failure_health.storage.queue_depth, 32);
  assert.equal(row.validation.error, original.stack);
});

test("recovery records burst rejections separately and requires a full successful bounded window", async () => {
  let clock = 1000,
    attempt = 0;
  const report = {};
  await runRecoveryProbe(report, {
    now: () => clock,
    readHealth: async () => ({ status: 200, storage: { queue_depth: 0 } }),
    runBurst: async () => {
      clock += 2000;
      return {
        measurement: {
          statuses: { 200: 12, 503: 330 },
          failures: 330,
          successful_requests: 12,
        },
      };
    },
    runCheck: async (deadline) => {
      assert.equal(deadline, 13000);
      clock += 2000;
      attempt++;
      return {
        requests_finished_at_epoch_ms: clock,
        measurement: {
          elapsed_ms: 2000,
          successful_requests: 30,
          failures: attempt === 1 ? 2 : 0,
        },
      };
    },
  });
  assert.equal(report.passed, true);
  assert.equal(report.burst_503_responses, 330);
  assert.equal(report.checks.length, 2);
  assert.equal(report.recovery_elapsed_ms, 4000);
  assert.equal(report.checks[0].continuous_success, false);
  assert.equal(report.performance_claim_allowed, false);
  assert.equal(
    report.measurement,
    undefined,
    "burst/check requests are never the benchmark measurement",
  );
});

test("persistent errors and late successful responses cannot claim recovery", async () => {
  for (const late of [false, true]) {
    let clock = 0;
    const report = {};
    await runRecoveryProbe(report, {
      now: () => clock,
      readHealth: async () => ({ status: 200 }),
      runBurst: async () => ({ measurement: { statuses: { 503: 10 } } }),
      runCheck: async () => {
        clock += late ? 10001 : 2000;
        return {
          requests_finished_at_epoch_ms: clock,
          measurement: {
            elapsed_ms: late ? 10001 : 2000,
            successful_requests: 2,
            failures: late ? 0 : 1,
          },
        };
      },
    });
    assert.equal(report.recovered, false);
    assert.equal(report.passed, false);
    assert.equal(report.checks.length, late ? 1 : 5);
  }
  const runs = gateRuns().map((run) => ({ ...run, recovery_probe: true }));
  assert.deepEqual(comparisonFailures(compareRuns(runs)), []);
  assert.match(
    comparisonFailures(compareRuns(runs), { requireSixPairs: true }).join("\n"),
    /cannot support performance claims/,
  );
  assert.equal(compareRuns([...runs, ...gateRuns()]).length, 2);
});

test("recovery CLI refuses performance-sized runs before touching the environment", () => {
  const script = fileURLToPath(
    new URL("../auth-performance.mjs", import.meta.url),
  );
  for (const extra of [
    ["--pairs", "6"],
    ["--seconds", "60"],
    ["--routes", "grant_renewal"],
    ["--profile", "1"],
  ]) {
    const result = spawnSync(
      process.execPath,
      [
        script,
        "--config",
        "/not-read.json",
        "--out",
        "/not-created",
        "--pairs",
        "1",
        "--warmup",
        "1",
        "--seconds",
        "3",
        "--routes",
        "session_hit",
        "--recovery-probe",
        "1",
        ...extra,
      ],
      { encoding: "utf8" },
    );
    assert.notEqual(result.status, 0);
    assert.match(result.stderr, /recovery probes are short smoke only/);
  }
});

test("synthetic seeder refuses unowned databases and keeps legacy and typed grant/session authority equal", async () => {
  const directory = await mkdtemp(
    path.join(tmpdir(), "auth-performance-test-"),
  );
  const seeder = fileURLToPath(
    new URL("../auth-performance-seed.py", import.meta.url),
  );
  try {
    assert.notEqual(
      spawnSync("python3", [seeder, directory, "grant_renewal"]).status,
      0,
    );
    await writeFile(
      path.join(directory, ".auth-performance-owned"),
      "synthetic-auth-performance-v1\n",
    );
    const schema = `import sqlite3,sys,json
c=sqlite3.connect(sys.argv[1]);c.executescript('''
CREATE TABLE kv_keys(key TEXT PRIMARY KEY,kind TEXT,expires_at_ms INTEGER);
CREATE TABLE kv_strings(key TEXT PRIMARY KEY,value TEXT);
CREATE TABLE kv_zset(key TEXT,member TEXT,score REAL,PRIMARY KEY(key,member));
CREATE TABLE kv_set(key TEXT,member TEXT,PRIMARY KEY(key,member));
CREATE TABLE kv_hash(key TEXT,field TEXT,value TEXT,PRIMARY KEY(key,field));
CREATE TABLE config_documents(singleton INTEGER,document_json TEXT,revision INTEGER,updated_at_ms INTEGER);
CREATE TABLE mobility_session_aggregates(session_id TEXT PRIMARY KEY,aggregate_json TEXT,session_expires_at_ms INTEGER,updated_at_ms INTEGER);
CREATE TABLE subdomain_rule_grants(grant_digest TEXT PRIMARY KEY,host TEXT,policy_version TEXT,group_id TEXT,issued_at INTEGER,last_access_at INTEGER,hard_expires_at INTEGER,expires_at_ms INTEGER,updated_at_ms INTEGER);
CREATE TABLE subdomain_rule_grant_active_entries(host_digest TEXT,grant_digest TEXT,expires_at_score INTEGER,updated_at_ms INTEGER,PRIMARY KEY(host_digest,grant_digest));
CREATE TABLE whitelist_documents(kind TEXT,id TEXT,document_json TEXT,sort_score INTEGER,expires_at INTEGER,status TEXT,updated_at_ms INTEGER,PRIMARY KEY(kind,id));
INSERT INTO config_documents VALUES(1,'{}',1,0);
''');c.commit()`;
    const database = path.join(directory, "state.sqlite3");
    execFileSync("python3", ["-c", schema, database]);
    const seeded = JSON.parse(
      execFileSync(
        "python3",
        [
          seeder,
          directory,
          "grant_renewal",
          "--sessions",
          "4",
          "--renewals",
          "8",
          "--cache-ttl",
          "0",
          "--grants",
          "1",
          "--accounts",
          "3",
        ],
        { encoding: "utf8" },
      ),
    );
    assert.equal(seeded.credential_kind, "totp");
    const check = `import sqlite3,sys,json,time
c=sqlite3.connect(sys.argv[1]); kind=sys.argv[2]
accounts=json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:auth:accounts:v1'").fetchone()[0]); by_id={a['id']:a for a in accounts}
totps=json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:totps'").fetchone()[0]); totp_ids={t['id'] for t in totps}
assert c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:auth:login_mode:v1'").fetchone()[0]==kind
for sid,raw,expires,_ in c.execute('SELECT * FROM mobility_session_aggregates'):
 a=json.loads(raw); b=json.loads(c.execute('SELECT value FROM kv_strings WHERE key=?',('fn_knock:session:'+sid,)).fetchone()[0]); assert a['session']['value']==b; assert a['session']['expires_at_ms']==expires
 assert b['method']==('PASSWORD' if kind=='password' else 'TOTP'); assert b['totpId'] in totp_ids
 if kind=='password':
  account=by_id[b['credentialId']]; assert account['sourceTotpId']==b['totpId']; assert account['subdomain_access']['mode']=='all'; assert b['credentialName']==account['displayName']
 else: assert b['credentialId']==b['totpId']
assert c.execute('SELECT COUNT(*) FROM subdomain_rule_grants').fetchone()[0]==18
for digest,host,policy,group,issued,last,hard,expires,_ in c.execute('SELECT * FROM subdomain_rule_grants'):
 b=json.loads(c.execute('SELECT value FROM kv_strings WHERE key=?',('fn_knock:auth:subdomain_rule_grant:'+digest,)).fetchone()[0]); assert (b['host'],b['policy_version'],b['group_id'],b['issued_at'],b['last_access_at'],b['hard_expires_at'])==(host,policy,group,issued,last,hard); assert int(time.time())-last>=60
config=json.loads(c.execute('SELECT document_json FROM config_documents').fetchone()[0]); assert config['subdomain_mode']['auth_cache_ttl_seconds']==0; assert config==json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:config'").fetchone()[0])
assert len(json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:totps'").fetchone()[0]))==3
assert len(json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:auth:accounts:v1'").fetchone()[0]))==3`;
    execFileSync("python3", ["-c", check, database, "totp"]);
    const passwordSeed = JSON.parse(
      execFileSync(
        "python3",
        [
          seeder,
          directory,
          "grant_renewal",
          "--sessions",
          "4",
          "--renewals",
          "8",
          "--cache-ttl",
          "0",
          "--grants",
          "1",
          "--accounts",
          "3",
          "--credential-kind",
          "password",
        ],
        { encoding: "utf8" },
      ),
    );
    assert.equal(passwordSeed.credential_kind, "password");
    execFileSync("python3", ["-c", check, database, "password"]);
    // Exercise a real 1000-account list and password-backed automatic IP owner.
    await rm(database);
    execFileSync("python3", ["-c", schema, database]);
    const autoSeed = JSON.parse(
      execFileSync(
        "python3",
        [
          seeder,
          directory,
          "auto_ip_hit",
          "--sessions",
          "4",
          "--accounts",
          "1000",
          "--credential-kind",
          "password",
        ],
        { encoding: "utf8" },
      ),
    );
    assert.equal(autoSeed.accounts, 1000);
    assert.equal(autoSeed.credential_kind, "password");
    execFileSync("python3", [
      "-c",
      `import sqlite3,json,sys
c=sqlite3.connect(sys.argv[1]); load=lambda k: json.loads(c.execute('SELECT value FROM kv_strings WHERE key=?',(k,)).fetchone()[0])
accounts=load('fn_knock:auth:accounts:v1'); assert len(accounts)==1000; assert len(load('fn_knock:totps'))==1000
s=load('fn_knock:session:authperf-session-0'); assert s['method']=='PASSWORD'; assert s['credentialId']==accounts[0]['id']; assert s['totpId']==accounts[0]['sourceTotpId']; assert s['grantType']=='login_ip_grant'; assert s['postLoginIpGrantMode']=='follow_session'; assert s['ip']=='198.18.0.1'
assert all(a['subdomain_access']['mode']=='all' for a in accounts)
assert c.execute("SELECT COUNT(*) FROM kv_strings WHERE key LIKE 'fn_knock:auth:password_credentials:%'").fetchone()[0]==0
assert c.execute("SELECT COUNT(*) FROM whitelist_documents WHERE kind='record'").fetchone()[0]==1`,
      database,
    ]);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("credential kind accepts only documented session fixture types", () => {
  const runner = fileURLToPath(
    new URL("../auth-performance.mjs", import.meta.url),
  );
  const result = spawnSync(
    process.execPath,
    [
      runner,
      "--config",
      "/not-read.json",
      "--out",
      "/not-created",
      "--credential-kind",
      "PASSWORD",
    ],
    { encoding: "utf8" },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /credential-kind must be totp or password/);
});
