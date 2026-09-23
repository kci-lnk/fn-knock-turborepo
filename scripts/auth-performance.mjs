// Linux-only, synthetic isolated-process benchmark. See the experiment README.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import http from "node:http";
import { spawn, execFileSync } from "node:child_process";
import { createWriteStream } from "node:fs";
import {
  readFile,
  writeFile,
  mkdir,
  readdir,
  readlink,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { Worker } from "node:worker_threads";
import { setTimeout as delay } from "node:timers/promises";
import {
  scenarios,
  marker,
  requestSpec,
  validateResponse,
  pairOrder,
  mergeMeasurements,
  compareRuns,
} from "./auth-performance-lib.mjs";
import { summarizeOperationProfile } from "./auth-performance-profile.mjs";
import {
  captureFailureSnapshot,
  runRecoveryProbe,
} from "./auth-performance-recovery.mjs";

const script = fileURLToPath(import.meta.url);
const token = "isolated-auth-performance-20260923";
const now = () => performance.timeOrigin + performance.now();
const children = new Set();
let stopping = false;

async function stop(child) {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    delay(3000),
  ]);
  if (child.exitCode === null && child.signalCode === null) {
    child.kill("SIGKILL");
    await new Promise((resolve) => child.once("exit", resolve));
  }
  children.delete(child);
}
async function cleanup() {
  await Promise.all([...children].map(stop));
}
for (const signal of ["SIGINT", "SIGTERM"])
  process.on(signal, () => {
    if (stopping) return;
    stopping = true;
    cleanup().finally(() => process.exit(128 + (signal === "SIGINT" ? 2 : 15)));
  });

function launch(binary, args, options, logFile) {
  const log = createWriteStream(logFile, { flags: "a" });
  const child = spawn(binary, args, {
    ...options,
    stdio: ["ignore", "pipe", "pipe"],
  });
  children.add(child);
  child.stdout.pipe(log, { end: false });
  child.stderr.pipe(log, { end: false });
  child.once("close", () => log.end());
  child.once("error", (error) => {
    child.launchError = error;
  });
  return child;
}

function probe(
  spec,
  port = 27999,
  localAddress = "198.18.0.1",
  method = "GET",
  body,
) {
  return new Promise((resolve, reject) => {
    const request = http.request(
      {
        hostname: "127.0.0.1",
        port,
        localAddress,
        path: spec.path,
        headers: spec.headers,
        method,
      },
      (response) => {
        let text = "";
        response.setEncoding("utf8");
        response.on("data", (chunk) => {
          text += chunk;
          if (text.length > 2 * 1024 * 1024)
            request.destroy(new Error("oversize probe response"));
        });
        response.on("error", reject);
        response.on("end", () =>
          resolve({
            status: response.statusCode,
            headers: response.headers,
            body: text,
          }),
        );
      },
    );
    const timeout = setTimeout(
      () => request.destroy(new Error("probe timeout")),
      spec.timeoutMs ?? 3000,
    );
    request.on("close", () => clearTimeout(timeout));
    request.on("error", reject);
    request.end(body);
  });
}

async function waitReady(processes) {
  const deadline = Date.now() + 90000;
  let last;
  while (Date.now() < deadline) {
    for (const child of processes) {
      if (child.launchError) throw child.launchError;
      if (child.exitCode !== null || child.signalCode !== null)
        throw new Error(
          `experiment child ${child.pid} exited: ${child.exitCode}/${child.signalCode}`,
        );
    }
    try {
      const response = await probe(requestSpec("bootstrap"));
      if (response.status === 200) return;
      last = response.status;
    } catch (error) {
      last = error.message;
    }
    await delay(250);
  }
  throw new Error(`readiness failed: ${last}`);
}

async function processStats(pid, hz) {
  try {
    const [raw, status, files] = await Promise.all([
      readFile(`/proc/${pid}/stat`, "utf8"),
      readFile(`/proc/${pid}/status`, "utf8"),
      readdir(`/proc/${pid}/fd`),
    ]);
    const fields = raw.slice(raw.lastIndexOf(") ") + 2).split(" ");
    const number = (name) =>
      Number(status.match(new RegExp(`^${name}:\\s+(\\d+)`, "m"))?.[1] ?? 0);
    return {
      pid,
      start_ticks: Number(fields[19]),
      cpu_seconds: (Number(fields[11]) + Number(fields[12])) / hz,
      rss_bytes: number("VmRSS") * 1024,
      peak_rss_bytes: number("VmHWM") * 1024,
      threads: number("Threads"),
      fds: files.length,
    };
  } catch {
    return null;
  }
}

async function createAdminSession() {
  const response = await probe(
    {
      path: "/api/admin/panel/password",
      headers: { "content-type": "application/json" },
      timeoutMs: 15000,
    },
    27991,
    "127.0.0.1",
    "POST",
    JSON.stringify({ password: "Authperf-Control-20260923!" }),
  );
  assert.equal(response.status, 200, "synthetic admin session setup failed");
  assert.equal(JSON.parse(response.body).success, true);
  const cookie = (response.headers["set-cookie"] ?? [])
    .find((value) => value.startsWith("fn-knock-admin-panel-session="))
    ?.split(";")[0];
  assert.ok(cookie, "synthetic admin session cookie missing");
  return { cookie };
}

async function operationCapture(method, headers) {
  const response = await probe(
    { path: "/api/admin/runtime-health/debug/capture", headers },
    27991,
    "127.0.0.1",
    method,
  );
  assert.equal(response.status, 200, "operation capture API unavailable");
  const payload = JSON.parse(response.body);
  assert.equal(payload.success, true);
  assert.equal(payload.data.capture.operations.active, method === "POST");
  return payload.data;
}

async function healthSnapshot(headers, timeoutMs = 3000) {
  let response;
  try {
    assert.ok(headers, "admin session is not available");
    response = await probe(
      { path: "/api/admin/runtime-health", headers, timeoutMs },
      27991,
      "127.0.0.1",
    );
    const payload = JSON.parse(response.body);
    assert.equal(response.status, 200, "runtime health API unavailable");
    assert.equal(payload.success, true);
    return {
      observed_at_epoch_ms: now(),
      status: response.status,
      storage: payload.data?.components?.storage ?? null,
      auth_bridge: payload.data?.components?.auth_bridge ?? null,
      snapshot: payload.data,
    };
  } catch (error) {
    return {
      observed_at_epoch_ms: now(),
      status: response?.status ?? null,
      error: error.message,
    };
  }
}

async function startWorkers(options, scenario, ownedWorkers) {
  const workers = [];
  for (let index = 0; index < options.clients; index++) {
    const concurrency =
      Math.floor(options.concurrency / options.clients) +
      (index < options.concurrency % options.clients ? 1 : 0);
    if (!concurrency) continue;
    const worker = new Worker(
      new URL("./auth-performance-worker.mjs", import.meta.url),
      {
        workerData: {
          concurrency,
          scenario,
          renewals: options.renewals,
          workerIndex: index,
          workerCount: Math.min(options.clients, options.concurrency),
          timeoutMs: options.timeoutMs,
        },
      },
    );
    workers.push(worker);
    ownedWorkers.push(worker);
    await new Promise((resolve, reject) => {
      worker.once("message", (message) =>
        message.type === "ready"
          ? resolve()
          : reject(new Error("worker startup failed")),
      );
      worker.once("error", reject);
    });
  }
  return workers;
}

async function runPhase(workers, processes, options, phase, collect) {
  const durationMs =
    (phase === "warm" ? options.warmup : options.seconds) * 1000;
  const completedRows = [];
  const results = workers.map(
    (worker) =>
      new Promise((resolve, reject) => {
        const message = (value) => {
          if (value.type === "result" && value.phase === phase) {
            worker.off("message", message);
            completedRows.push(value);
            resolve(value);
          }
          if (value.type === "failure") {
            worker.off("message", message);
            reject(new Error(value.error));
          }
        };
        worker.on("message", message);
        worker.once("error", reject);
      }),
  );
  const samples = [];
  const stats = async () =>
    Object.fromEntries(
      await Promise.all(
        Object.entries(processes).map(async ([name, child]) => [
          name,
          await processStats(child.pid, options.hz),
        ]),
      ),
    );
  const before = await stats();
  let active = collect;
  const startedAt = now();
  const sampling = collect
    ? (async () => {
        while (active) {
          samples.push({
            elapsed_ms: now() - startedAt,
            processes: await stats(),
          });
          await delay(200);
        }
      })()
    : Promise.resolve();
  const health = [];
  const healthSampling = collect
    ? (async () => {
        while (active) {
          try {
            const { snapshot, ...sample } = await healthSnapshot(
              options.adminHeaders,
              options.healthTimeoutMs,
            );
            health.push({
              elapsed_ms: now() - startedAt,
              ...sample,
            });
          } catch (error) {
            health.push({
              elapsed_ms: now() - startedAt,
              error: error.message,
            });
          }
          await delay(1000);
        }
      })()
    : Promise.resolve();
  for (const worker of workers)
    worker.postMessage({ type: "run", startedAt, durationMs, phase });
  let rows;
  let timer;
  try {
    rows = await Promise.race([
      Promise.all(results),
      new Promise((_, reject) => {
        timer = setTimeout(
          () => reject(new Error("client phase exceeded bounded duration")),
          Math.max(
            1,
            Math.min(
              durationMs + options.timeoutMs + 5000,
              (options.deadlineAt ?? Infinity) - now(),
            ),
          ),
        );
      }),
    ]);
  } catch (error) {
    error.phase_evidence = {
      phase,
      elapsed_ms: now() - startedAt,
      completed_workers: completedRows.length,
      expected_workers: workers.length,
      partial_measurement: completedRows.length
        ? mergeMeasurements(completedRows)
        : null,
      samples,
      runtime_health: health,
    };
    throw error;
  } finally {
    clearTimeout(timer);
    active = false;
  }
  if (phase === "load" && options.afterRequests) await options.afterRequests();
  const after = await stats();
  await Promise.all([sampling, healthSampling]);
  const measurement = mergeMeasurements(rows);
  const resources = {};
  for (const name of Object.keys(processes)) {
    const a = before[name],
      b = after[name];
    assert.ok(
      a && b && a.start_ticks === b.start_ticks,
      `${name} process disappeared or restarted`,
    );
    const cpu = b.cpu_seconds - a.cpu_seconds;
    const values = samples
      .map((sample) => sample.processes[name])
      .filter(Boolean);
    resources[name] = {
      cpu_seconds: cpu,
      cpu_ms_per_1000_success: measurement.successful_requests
        ? (cpu * 1e6) / measurement.successful_requests
        : null,
      peak_rss_bytes: Math.max(
        a.rss_bytes,
        b.rss_bytes,
        ...values.map((value) => value.rss_bytes),
      ),
      end_rss_bytes: b.rss_bytes,
      peak_fds: Math.max(a.fds, b.fds, ...values.map((value) => value.fds)),
      peak_threads: Math.max(
        a.threads,
        b.threads,
        ...values.map((value) => value.threads),
      ),
      before: a,
      after: b,
    };
  }
  const times = [
    0,
    ...samples.map((sample) => sample.elapsed_ms),
    measurement.elapsed_ms,
  ];
  const maxGap = collect
    ? Math.max(...times.slice(1).map((time, index) => time - times[index]))
    : null;
  return {
    started_at_epoch_ms: startedAt,
    requests_finished_at_epoch_ms: startedAt + measurement.elapsed_ms,
    measurement,
    resources,
    samples,
    runtime_health: health,
    quality: {
      max_sampling_gap_ms: maxGap,
      sampling_ok: !collect || maxGap <= 1000,
      health_ok:
        !collect ||
        (health.length > 0 &&
          health.every(
            (sample) =>
              sample.status === 200 &&
              sample.storage !== null &&
              sample.auth_bridge !== null,
          )),
      client_ok:
        measurement.client_event_loop_delay_max_ms <= 100 &&
        measurement.client_start_delay_max_ms <= 100,
      duration_ok:
        options.scenario === "grant_renewal"
          ? measurement.all_tokens_consumed
          : measurement.elapsed_ms >= durationMs &&
            measurement.elapsed_ms <=
              durationMs + Math.max(500, durationMs * 0.05),
    },
  };
}

async function trial(
  variant,
  role,
  candidate,
  pair,
  scenario,
  options,
  config,
) {
  const directory = path.join(
    options.out,
    `${candidate}-${pair + 1}-${role}-${scenario}`,
  );
  await mkdir(directory);
  await writeFile(
    path.join(directory, ".auth-performance-owned"),
    "synthetic-auth-performance-v1\n",
  );
  const gatewayConfig = path.join(directory, "config.json");
  await writeFile(
    gatewayConfig,
    JSON.stringify({
      gateway_listener: { scope: "loopback" },
      auth_config: {
        auth_port: 27997,
        auth_cache_ttl_seconds: options.cacheTtl,
        auth_cache_unauthorized_ttl_seconds: options.cacheTtl,
      },
      rules: [],
    }),
  );
  const env = {
    ...process.env,
    ...variant.env,
    FN_KNOCK_RUNTIME_TARGET: "linux",
    FN_KNOCK_DISABLE_REDIS_MIGRATION: "1",
    FN_KNOCK_INTERNAL_RPC_TOKEN: token,
    HMAC_SECRET: token,
    FN_KNOCK_DATA_DIR: directory,
    FN_KNOCK_GATEWAY_CONFIG_DIR: directory,
    FN_KNOCK_SQLITE_PATH: path.join(directory, "state.sqlite3"),
    GO_BACKEND_GRPC_ADDR: "127.0.0.1:27996",
    GO_BACKEND_PORT: "27996",
    BACKEND_HOST: "127.0.0.1",
    BACKEND_PORT: "27998",
    AUTH_HOST: "127.0.0.1",
    AUTH_PORT: "27997",
    ADMIN_VIEW_HOST: "127.0.0.1",
    ADMIN_VIEW_PORT: "27991",
    ADMIN_STATIC_PATH: config.admin_static,
    AUTH_STATIC_PATH: config.auth_static,
    FN_KNOCK_TOKIO_WORKER_THREADS:
      variant.env?.FN_KNOCK_TOKIO_WORKER_THREADS ?? "2",
    FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT:
      variant.env?.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT ?? "32",
    SESSION_COOKIE_SECURE: "0",
  };
  let go, rust, adminHeaders;
  const start = () => {
    go = launch(
      variant.go,
      ["-c", gatewayConfig, "-admin-port", "27996", "-proxy-port", "27999"],
      { cwd: directory, env },
      path.join(directory, "go.log"),
    );
    rust = launch(
      variant.rust,
      [],
      { cwd: directory, env },
      path.join(directory, "rust.log"),
    );
  };
  const workers = [];
  const row = {
    schema_version: 1,
    candidate,
    role,
    variant: variant.name,
    pair,
    scenario,
    concurrency: options.concurrency,
    directory,
    variant_metadata: variant.metadata ?? {},
    env: variant.env ?? {},
    cache_ttl_seconds: options.cacheTtl,
    profiling: options.profile,
    recovery_probe: options.recoveryProbe,
    roles: options.roles,
    started_at: new Date().toISOString(),
  };
  row.effective_runtime_env = Object.fromEntries(
    [
      "FN_KNOCK_RUNTIME_TARGET",
      "FN_KNOCK_TOKIO_WORKER_THREADS",
      "FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT",
      "FN_KNOCK_SQLITE_AUTH_READERS",
      "GLIBC_TUNABLES",
      "LD_PRELOAD",
      "MALLOC_ARENA_MAX",
      "GOMEMLIMIT",
      "GOGC",
      "GOMAXPROCS",
    ].map((name) => [name, env[name] ?? null]),
  );
  try {
    start();
    await waitReady([go, rust]);
    await Promise.all([stop(rust), stop(go)]);
    row.seed = JSON.parse(
      execFileSync(
        "python3",
        [
          path.join(path.dirname(script), "auth-performance-seed.py"),
          directory,
          scenario,
          "--sessions",
          String(options.sessions),
          "--renewals",
          String(options.renewals),
          "--cache-ttl",
          String(options.cacheTtl),
          "--grants",
          String(options.grants),
          "--accounts",
          String(options.accounts),
        ],
        { encoding: "utf8" },
      ),
    );
    start();
    await waitReady([go, rust]);
    await delay(1000);
    // Finish control-plane writes before enforcing and reading live cache TTLs.
    adminHeaders = await createAdminSession();
    if (config.control_binary) {
      row.cache_control = JSON.parse(
        execFileSync(
          config.control_binary,
          [
            "-addr",
            "127.0.0.1:27996",
            "-token",
            token,
            "-cache-ttl",
            String(options.cacheTtl),
          ],
          { encoding: "utf8", timeout: 10000 },
        ),
      );
      assert.equal(row.cache_control.runtime_verified, true);
      assert.equal(row.cache_control.auth_cache_ttl_seconds, options.cacheTtl);
      assert.equal(
        row.cache_control.auth_cache_unauthorized_ttl_seconds,
        options.cacheTtl,
      );
    } else {
      const actual = JSON.parse(
        await readFile(gatewayConfig, "utf8"),
      ).auth_config;
      assert.equal(
        actual.auth_cache_ttl_seconds ?? 0,
        options.cacheTtl,
        "generated positive cache TTL mismatch",
      );
      assert.equal(
        actual.auth_cache_unauthorized_ttl_seconds ?? 0,
        options.cacheTtl,
        "generated negative cache TTL mismatch",
      );
      row.cache_control = {
        source: "generated_config_only",
        runtime_verified: false,
        ...actual,
      };
      assert.notEqual(
        options.cacheTtl,
        0,
        "cache-disabled trials require control_binary to read back the live Go configuration",
      );
    }
    // No response redirect-following: success must be the owned upstream marker.
    const spec =
      scenario === "grant_renewal"
        ? {
            ...requestSpec(scenario),
            headers: {
              ...requestSpec(scenario).headers,
              cookie: "fn-knock-subdomain-rule-grant=authperf-grant-preflight",
            },
          }
        : requestSpec(scenario, 0, "preflight");
    const response = await probe(spec);
    assert.equal(
      validateResponse(spec, response),
      null,
      `preflight ${scenario}: ${JSON.stringify({ status: response.status, headers: response.headers, body: response.body.slice(0, 500) })}`,
    );
    row.preflight = {
      status: response.status,
      expected: spec.expected,
      origin: response.headers["x-authperf-origin"] ?? null,
      renewal: Boolean(response.headers["set-cookie"]),
    };
    const processes = {
      go,
      rust,
      client: { pid: process.pid },
      fixture: options.fixture,
    };
    const phaseOptions = { ...options, scenario, adminHeaders };
    if (options.recoveryProbe) {
      const runProbePhase = async (concurrency, phase, deadlineAt) => {
        const probeOptions = {
          ...phaseOptions,
          concurrency,
          clients: concurrency === 1 ? 1 : Math.min(2, options.clients),
          seconds: 2,
          timeoutMs: 1000,
          healthTimeoutMs: 1000,
          deadlineAt,
        };
        const probeWorkers = await startWorkers(
          probeOptions,
          scenario,
          workers,
        );
        try {
          return await runPhase(
            probeWorkers,
            processes,
            probeOptions,
            phase,
            true,
          );
        } finally {
          await Promise.all(probeWorkers.map((worker) => worker.terminate()));
        }
      };
      row.recovery = {};
      await runRecoveryProbe(row.recovery, {
        readHealth: () => healthSnapshot(adminHeaders, 1000),
        runBurst: () => runProbePhase(64, "recovery-burst"),
        runCheck: (deadline) => runProbePhase(1, "recovery-check", deadline),
      });
    }
    const benchmarkWorkers = await startWorkers(options, scenario, workers);
    if (options.profile) {
      if (config.control_binary) {
        row.cache_control_before_profile = JSON.parse(
          execFileSync(
            config.control_binary,
            [
              "-addr",
              "127.0.0.1:27996",
              "-token",
              token,
              "-cache-ttl",
              String(options.cacheTtl),
              "-read-only",
            ],
            { encoding: "utf8", timeout: 10000 },
          ),
        );
        assert.equal(row.cache_control_before_profile.runtime_verified, true);
      }
      await operationCapture("POST", adminHeaders);
      await delay(options.profileIdle * 1000);
      row.profile_idle = await operationCapture("DELETE", adminHeaders);
    }
    row.warmup = await runPhase(
      benchmarkWorkers,
      processes,
      phaseOptions,
      "warm",
      false,
    );
    assert.equal(
      row.warmup.measurement.failures,
      0,
      "warmup included incorrect or failed responses",
    );
    if (options.profile) {
      row.profile_start = await operationCapture("POST", adminHeaders);
      phaseOptions.afterRequests = async () => {
        row.profile_capture = await operationCapture("DELETE", adminHeaders);
      };
    }
    const load = await runPhase(
      benchmarkWorkers,
      processes,
      phaseOptions,
      "load",
      true,
    );
    Object.assign(row, load);
    if (options.profile) {
      assert.equal(
        row.profile_capture.capture.status,
        "stopped",
        "capture expired before load completed",
      );
      row.operation_profile = summarizeOperationProfile(
        row.profile_capture,
        load.measurement.successful_requests,
        row.profile_idle,
      );
    }
    if (config.control_binary)
      row.cache_control_after = JSON.parse(
        execFileSync(
          config.control_binary,
          [
            "-addr",
            "127.0.0.1:27996",
            "-token",
            token,
            "-cache-ttl",
            String(options.cacheTtl),
            "-read-only",
          ],
          { encoding: "utf8", timeout: 10000 },
        ),
      );
    row.validation = {
      passed:
        load.measurement.failures === 0 &&
        load.quality.sampling_ok &&
        load.quality.health_ok &&
        load.quality.client_ok &&
        load.quality.duration_ok &&
        (!options.recoveryProbe || row.recovery.passed) &&
        (!options.profile || row.operation_profile.dropped_operations === 0),
      note:
        scenario === "grant_renewal"
          ? "finite unique-token renewal batch; throughput is not a sustained fixed-duration workload"
          : "closed-loop keep-alive per-route workload",
    };
  } catch (error) {
    if (error.phase_evidence) row.failed_phase = error.phase_evidence;
    await captureFailureSnapshot(row, error, () =>
      healthSnapshot(adminHeaders),
    );
  } finally {
    if (!row.validation?.passed && !row.failure_health)
      row.failure_health = await healthSnapshot(adminHeaders);
    await Promise.all(workers.map((worker) => worker.terminate()));
    await Promise.all([stop(rust), stop(go)]);
    row.timeout_phase_events = [];
    for (const filename of ["management.jsonl.1", "management.jsonl"]) {
      try {
        const content = await readFile(
          path.join(directory, "runtime/logs", filename),
          "utf8",
        );
        for (const line of content.split("\n")) {
          try {
            const value = JSON.parse(line);
            if (value.fields?.phase_elapsed_ms)
              row.timeout_phase_events.push(value);
          } catch {
            /* partial final log line */
          }
        }
      } catch {
        /* this binary may not expose phase diagnostics */
      }
    }
    row.phase_metrics_note =
      "phase timings are timeout-event diagnostics, not an all-successful-request histogram; raw runtime/logs are retained";
    await writeFile(
      path.join(directory, "result.json"),
      JSON.stringify(row, null, 2),
    );
  }
  return row;
}

function parseArgs(args) {
  const values = {};
  for (let index = 0; index < args.length; index += 2) {
    assert.ok(
      args[index].startsWith("--") && args[index + 1] !== undefined,
      "arguments must be --name value pairs",
    );
    values[args[index].slice(2)] = args[index + 1];
  }
  assert.ok(
    values.config && values.out,
    "usage: node scripts/auth-performance.mjs --config variants.json --out NEW_DIRECTORY [--routes bootstrap,session_hit --pairs 6 --warmup 20 --seconds 60 --concurrency 16 --clients 2 --cache-ttl 1 --sessions 64 --renewals 4096]",
  );
  const options = {
    config: path.resolve(values.config),
    out: path.resolve(values.out),
    routes: (values.routes ?? scenarios.join(",")).split(","),
    pairs: Number(values.pairs ?? 6),
    warmup: Number(values.warmup ?? 20),
    seconds: Number(values.seconds ?? 60),
    concurrency: Number(values.concurrency ?? 16),
    clients: Number(values.clients ?? 2),
    sessions: Number(values.sessions ?? 64),
    grants: Number(values.grants ?? 2),
    accounts: Number(values.accounts ?? 1),
    renewals: Number(values.renewals ?? 4096),
    cacheTtl: Number(values["cache-ttl"] ?? 1),
    timeoutMs: Number(values["timeout-ms"] ?? 10000),
    selected: values.candidates?.split(","),
    profile: values.profile === "1",
    recoveryProbe: values["recovery-probe"] === "1",
    profileIdle: Number(values["profile-idle"] ?? 5),
    roles: (values.roles ?? "baseline,candidate").split(","),
  };
  for (const name of [
    "pairs",
    "warmup",
    "seconds",
    "concurrency",
    "clients",
    "sessions",
    "grants",
    "accounts",
    "profileIdle",
    "renewals",
    "timeoutMs",
  ])
    assert.ok(
      Number.isInteger(options[name]) && options[name] >= 1,
      `invalid ${name}`,
    );
  assert.ok(
    options.concurrency <= 1024 &&
      options.clients <= 32 &&
      options.seconds <= 1800 &&
      options.pairs <= 30 &&
      options.accounts <= 1000 &&
      options.grants <= 10000 &&
      options.sessions <= 10000 &&
      [0, 1].includes(options.cacheTtl),
    "parameter exceeds bounded experiment limits",
  );
  assert.ok(
    !options.profile || (options.seconds <= 45 && options.profileIdle <= 45),
    "profile captures must be at most 45 seconds to avoid the built-in 60 second deadline",
  );
  assert.ok(
    values["recovery-probe"] === undefined ||
      ["0", "1"].includes(values["recovery-probe"]),
    "recovery-probe must be 0 or 1",
  );
  assert.ok(
    !options.recoveryProbe ||
      (options.pairs === 1 &&
        options.seconds <= 10 &&
        options.warmup <= 10 &&
        !options.profile &&
        !options.routes.includes("grant_renewal")),
    "recovery probes are short smoke only: pairs=1, warmup/seconds<=10, profiling disabled, no finite renewal route",
  );
  assert.ok(
    options.roles.length > 0 &&
      options.roles.every((role) => ["baseline", "candidate"].includes(role)) &&
      new Set(options.roles).size === options.roles.length,
    "roles must be baseline,candidate or one of them",
  );
  assert.ok(
    options.roles.length === 2 ||
      (options.pairs === 1 && options.seconds >= 60 && !options.profile),
    "single-role runs are soak only: pairs=1, seconds>=60, profiling disabled",
  );
  options.routes.forEach((name) =>
    assert.ok(scenarios.includes(name), `unknown scenario ${name}`),
  );
  return options;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  assert.equal(
    process.platform,
    "linux",
    "run inside the isolated Linux network namespace",
  );
  assert.notEqual(
    await readlink("/proc/self/ns/net"),
    await readlink("/proc/1/ns/net"),
    "refusing host network namespace",
  );
  const addresses = JSON.parse(
    execFileSync("ip", ["-json", "addr", "show", "lo"], { encoding: "utf8" }),
  );
  assert.ok(
    addresses.some((entry) =>
      entry.addr_info.some((address) => address.local === "198.18.0.1"),
    ),
    "namespace must own source 198.18.0.1",
  );
  options.hz = Number(
    execFileSync("getconf", ["CLK_TCK"], { encoding: "utf8" }),
  );
  const config = JSON.parse(await readFile(options.config, "utf8"));
  for (const key of ["admin_static", "auth_static"]) {
    assert.ok(
      path.isAbsolute(config[key] ?? ""),
      `${key} must be an absolute asset directory`,
    );
    await readdir(config[key]);
  }
  assert.ok(
    config.baseline &&
      Array.isArray(config.candidates) &&
      config.candidates.length,
    "provide baseline and candidate variants",
  );
  const selected = config.candidates.filter(
    (variant) => !options.selected || options.selected.includes(variant.name),
  );
  assert.ok(selected.length, "no selected candidates");
  await mkdir(options.out); // Never overwrite evidence.
  const identities = [];
  for (const variant of [config.baseline, ...selected]) {
    assert.match(variant.name, /^[a-zA-Z0-9_-]+$/, "unsafe variant name");
    for (const component of ["go", "rust"]) {
      assert.ok(
        path.isAbsolute(variant[component]),
        `${component} binary path must be absolute`,
      );
      identities.push({
        variant: variant.name,
        component,
        path: variant[component],
        sha256: createHash("sha256")
          .update(await readFile(variant[component]))
          .digest("hex"),
      });
    }
  }
  const manifest = {
    schema_version: 1,
    created_at: new Date().toISOString(),
    config,
    options: { ...options },
    identities,
    host: {
      kernel: execFileSync("uname", ["-a"], { encoding: "utf8" }).trim(),
      node: process.version,
    },
  };
  await writeFile(
    path.join(options.out, "manifest.json"),
    JSON.stringify(manifest, null, 2),
  );
  options.fixture = launch(
    process.execPath,
    [script, "--fixture"],
    {},
    path.join(options.out, "fixture.log"),
  );
  await delay(300);
  const results = [];
  try {
    for (const candidate of selected)
      for (let pair = 0; pair < options.pairs; pair++)
        for (const scenario of options.routes)
          for (const role of pairOrder(pair).filter((role) =>
            options.roles.includes(role),
          )) {
            process.stderr.write(
              `[auth-performance] ${candidate.name} pair=${pair + 1} ${scenario} ${role}\n`,
            );
            const row = await trial(
              role === "baseline" ? config.baseline : candidate,
              role,
              candidate.name,
              pair,
              scenario,
              options,
              config,
            );
            // Raw time-series live in each trial file. Keeping all previous
            // trials resident would steadily inflate the benchmark client.
            const {
              samples,
              runtime_health,
              timeout_phase_events,
              ...summary
            } = row;
            summary.raw_result_file = path.join(row.directory, "result.json");
            summary.observation_counts = {
              process_samples: samples?.length ?? 0,
              health_samples: runtime_health?.length ?? 0,
              timeout_phase_events: timeout_phase_events?.length ?? 0,
            };
            results.push(summary);
            await writeFile(
              path.join(options.out, "results.json"),
              JSON.stringify(
                {
                  schema_version: 1,
                  runs: results,
                  comparison: compareRuns(results),
                },
                null,
                2,
              ),
            );
            if (!row.validation.passed)
              throw new Error(
                `${row.directory} failed validation: ${row.validation.error ?? JSON.stringify(row.quality)}`,
              );
          }
  } finally {
    await cleanup();
  }
  process.stdout.write(
    JSON.stringify(
      { output: options.out, comparison: compareRuns(results) },
      null,
      2,
    ) + "\n",
  );
}

if (process.argv[2] === "--fixture") {
  const server = http.createServer((_request, response) => {
    response.writeHead(200, {
      "content-type": "text/plain",
      "content-length": Buffer.byteLength(marker),
      "x-authperf-origin": marker,
    });
    response.end(marker);
  });
  server.listen(28081, "127.0.0.1");
  process.on("SIGTERM", () => server.close(() => process.exit(0)));
} else
  main().catch(async (error) => {
    console.error(error.stack);
    await cleanup();
    process.exitCode = 1;
  });
