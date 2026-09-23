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
} from "../auth-performance-lib.mjs";
import { summarizeOperationProfile } from "../auth-performance-profile.mjs";

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
  assert.match(summary.measurement, /not SQL statement count/);
  report.capture.operations.operations = [operation];
  assert.equal(
    summarizeOperationProfile(report, 10).admission_calls_per_success,
    null,
  );
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
    execFileSync("python3", [
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
    ]);
    const check = `import sqlite3,sys,json,time
c=sqlite3.connect(sys.argv[1]);
for sid,raw,expires,_ in c.execute('SELECT * FROM mobility_session_aggregates'):
 a=json.loads(raw); b=json.loads(c.execute('SELECT value FROM kv_strings WHERE key=?',('fn_knock:session:'+sid,)).fetchone()[0]); assert a['session']['value']==b; assert a['session']['expires_at_ms']==expires
assert c.execute('SELECT COUNT(*) FROM subdomain_rule_grants').fetchone()[0]==18
for digest,host,policy,group,issued,last,hard,expires,_ in c.execute('SELECT * FROM subdomain_rule_grants'):
 b=json.loads(c.execute('SELECT value FROM kv_strings WHERE key=?',('fn_knock:auth:subdomain_rule_grant:'+digest,)).fetchone()[0]); assert (b['host'],b['policy_version'],b['group_id'],b['issued_at'],b['last_access_at'],b['hard_expires_at'])==(host,policy,group,issued,last,hard); assert int(time.time())-last>=60
config=json.loads(c.execute('SELECT document_json FROM config_documents').fetchone()[0]); assert config['subdomain_mode']['auth_cache_ttl_seconds']==0; assert config==json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:config'").fetchone()[0])
assert len(json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:totps'").fetchone()[0]))==3
assert len(json.loads(c.execute("SELECT value FROM kv_strings WHERE key='fn_knock:auth:accounts:v1'").fetchone()[0]))==3`;
    execFileSync("python3", ["-c", check, database]);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
