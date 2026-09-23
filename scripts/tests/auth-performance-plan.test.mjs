import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { pairOrder } from "../auth-performance-lib.mjs";
import { checkPerformancePlan } from "../check-auth-performance-plan.mjs";

function fixture(phase = "main") {
  const plan = {
    baseline: { rust: "a".repeat(40), go: "b".repeat(40) },
    candidate: {
      rust: "c".repeat(40),
      go: "d".repeat(40),
      opt_level: "z",
      lto: "fat",
      codegen_units: 1,
      auth_readers: 1,
    },
    primary_matrix: {
      pairs: 6,
      fresh_processes_and_database_per_trial: true,
      warmup_seconds: 20,
      measurement_seconds: 60,
      concurrency: 16,
      client_workers: 2,
      accounts: 1000,
      sessions: 1000,
      grants: 100,
      credential_kind: "totp",
      cache_ttl_seconds: 0,
      target_routes: ["session_hit"],
      guard_routes: ["challenge"],
    },
    cache_matrix: { route: "session_hit", pairs: 6, cache_ttl_seconds: 1 },
    renewal_matrix: {
      route: "grant_renewal",
      pairs: 6,
      measurement_deadline_seconds: 60,
      renewal_tokens_per_phase: 64,
      ordinary_grants: 1,
    },
  };
  const variants = Object.fromEntries(
    ["baseline", "candidate"].map((role) => [
      role,
      {
        name: role === "baseline" ? "base" : "candidate4",
        go: `/${role}/go`,
        rust: `/${role}/rust`,
        env: {},
        metadata: {
          main_commit: plan[role].rust,
          go_commit: plan[role].go,
          opt_level: "z",
          rust_opt_level: "z",
          lto: "fat",
          codegen_units: 1,
        },
      },
    ]),
  );
  const routes =
    phase === "main"
      ? ["session_hit", "challenge"]
      : [plan[`${phase}_matrix`].route];
  const ttl = phase === "cache" ? 1 : 0;
  const grants = phase === "renewal" ? 1 : 100;
  const renewals = phase === "renewal" ? 64 : 0;
  const manifest = {
    schema_version: 1,
    config: { baseline: variants.baseline, candidates: [variants.candidate] },
    options: {
      pairs: 6,
      warmup: 20,
      seconds: 60,
      concurrency: 16,
      clients: 2,
      accounts: 1000,
      sessions: 1000,
      grants,
      credentialKind: "totp",
      cacheTtl: ttl,
      renewals: 64,
      profile: false,
      recoveryProbe: false,
      roles: ["baseline", "candidate"],
      routes,
    },
    identities: Object.values(variants).flatMap((variant) =>
      ["rust", "go"].map((component) => ({
        variant: variant.name,
        component,
        path: variant[component],
        sha256: "e".repeat(64),
      })),
    ),
  };
  const runs = [];
  for (let pair = 0; pair < 6; pair++)
    for (const scenario of routes)
      for (const role of pairOrder(pair)) {
        const elapsed = phase === "renewal" ? 2000 : 60000;
        const rps =
          role === "candidate" && phase === "main" && scenario === "session_hit"
            ? 110
            : 100;
        const measurement = {
          elapsed_ms: elapsed,
          requests: renewals || rps * 60,
          successful_requests: renewals || rps * 60,
          failures: 0,
          successful_requests_per_second: phase === "renewal" ? 32 : rps,
          p99_ms: 100,
          all_tokens_consumed: true,
        };
        const cache = {
          runtime_verified: true,
          auth_cache_ttl_seconds: ttl,
          auth_cache_unauthorized_ttl_seconds: ttl,
        };
        runs.push({
          schema_version: 1,
          candidate: "candidate4",
          role,
          pair,
          scenario,
          variant: variants[role].name,
          variant_metadata: { ...variants[role].metadata },
          env: {},
          concurrency: 16,
          cache_ttl_seconds: ttl,
          profiling: false,
          recovery_probe: false,
          roles: ["baseline", "candidate"],
          directory: `/trial/${scenario}/${pair}/${role}`,
          effective_runtime_env: {
            FN_KNOCK_RUNTIME_TARGET: "linux",
            FN_KNOCK_TOKIO_WORKER_THREADS: "2",
            FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT: null,
            FN_KNOCK_SQLITE_AUTH_READERS: role === "candidate" ? "1" : null,
          },
          seed: {
            scenario,
            accounts: 1000,
            sessions: 1000,
            ordinary_grants: grants,
            total_grants: grants + (renewals ? 1 + 2 * renewals : 0),
            renewal_tokens_per_phase: renewals,
            credential_kind: "totp",
          },
          cache_control: { ...cache },
          cache_control_after: { ...cache },
          warmup: { measurement: { elapsed_ms: 20000, failures: 0 } },
          measurement,
          quality: {
            sampling_ok: true,
            health_ok: true,
            client_ok: true,
            duration_ok: true,
          },
          validation: { passed: true },
          resources: Object.fromEntries(
            ["go", "rust"].map((component, componentIndex) => {
              const identity = {
                pid: 1000 + runs.length * 2 + componentIndex,
                start_ticks: 5000 + runs.length,
              };
              return [
                component,
                {
                  peak_rss_bytes: 50,
                  before: { ...identity },
                  after: { ...identity },
                },
              ];
            }),
          ),
        });
      }
  return { plan, manifest, results: { schema_version: 1, runs }, phase };
}

function rejected(input, pattern) {
  const result = checkPerformancePlan(input);
  assert.equal(result.passed, false);
  assert.match(result.failures.join("\n"), pattern);
}

test("main requires improvement only on targets; cache and finite renewal use regression gates", () => {
  const main = checkPerformancePlan(fixture());
  assert.equal(main.passed, true, main.failures.join("\n"));
  assert.deepEqual(
    main.comparison.map((row) => row.gate),
    ["improvement", "no_regression"],
  );
  for (const phase of ["cache", "renewal"]) {
    const result = checkPerformancePlan(fixture(phase));
    assert.equal(result.passed, true, result.failures.join("\n"));
    assert.equal(result.comparison[0].gate, "no_regression");
  }
  assert.match(
    checkPerformancePlan(fixture("renewal")).measurement_semantics,
    /finite.*not sustained/,
  );
});

test("missing routes, duplicate pair/role, old sources, and AB/BA changes are rejected", () => {
  let input = fixture();
  input.results.runs = input.results.runs.filter(
    (row) => row.scenario !== "challenge",
  );
  rejected(input, /observed routes/);
  input = fixture();
  input.results.runs.push(structuredClone(input.results.runs[0]));
  rejected(input, /duplicate pair\/role/);
  input = fixture();
  input.results.runs[0].variant_metadata.main_commit = "f".repeat(40);
  rejected(input, /main_commit/);
  input = fixture();
  input.manifest.config.candidates[0].metadata.go_commit = "f".repeat(40);
  rejected(input, /go_commit/);
  input = fixture();
  [input.results.runs[0], input.results.runs[1]] = [
    input.results.runs[1],
    input.results.runs[0],
  ];
  rejected(input, /AB\/BA order/);
});

test("fresh-process evidence rejects reused identities and restarts", () => {
  let input = fixture();
  input.results.runs[1].resources.go = structuredClone(
    input.results.runs[0].resources.go,
  );
  rejected(input, /process reused across trials/);
  input = fixture();
  input.results.runs[0].resources.go.after.start_ticks += 1;
  rejected(input, /process changed during load/);
  input = fixture();
  delete input.results.runs[0].resources.rust.before;
  rejected(input, /missing process identity/);
});

test("manifest and actual trial workload must match inherited phase parameters", () => {
  for (const [mutate, pattern] of [
    [
      (x) => {
        x.manifest.options.seconds = 8;
      },
      /options.seconds/,
    ],
    [
      (x) => {
        x.results.runs[0].seed.accounts = 1;
      },
      /seed.accounts/,
    ],
    [
      (x) => {
        x.results.runs[0].cache_control_after.auth_cache_ttl_seconds = 1;
      },
      /cache_control_after.auth_cache_ttl_seconds/,
    ],
    [
      (x) => {
        x.results.runs[0].measurement.elapsed_ms = 8000;
      },
      /duration outside/,
    ],
    [
      (x) => {
        x.results.runs[0].effective_runtime_env.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT =
          "32";
      },
      /capacity override/,
    ],
    [
      (x) => {
        x.results.runs[0].profiling = true;
      },
      /profiling excluded/,
    ],
    [
      (x) => {
        x.results.runs[0].recovery_probe = true;
      },
      /recovery excluded/,
    ],
    [
      (x) => {
        x.manifest.options.roles = ["candidate"];
      },
      /single-role/,
    ],
  ]) {
    const input = fixture();
    mutate(input);
    rejected(input, pattern);
  }
  const renewal = fixture("renewal");
  renewal.results.runs[0].measurement.requests = 640;
  rejected(renewal, /finite batch requests/);
});

test("existing inclusive regression boundaries are preserved without weakening target improvement", () => {
  const input = fixture("cache");
  for (const row of input.results.runs.filter((x) => x.role === "candidate")) {
    row.measurement.successful_requests_per_second = 95;
    row.measurement.p99_ms = 110;
    row.resources.go.peak_rss_bytes = 52.5;
    row.resources.rust.peak_rss_bytes = 52.5;
  }
  assert.equal(checkPerformancePlan(input).passed, true);
  for (const row of input.results.runs.filter((x) => x.role === "candidate"))
    row.measurement.successful_requests_per_second = 94.99;
  rejected(input, /regression budget/);
  const main = fixture();
  for (const row of main.results.runs.filter(
    (x) => x.role === "candidate" && x.scenario === "session_hit",
  ))
    row.measurement.successful_requests_per_second = 109.99;
  rejected(main, /improvement target not established/);
});

test("CLI reads sibling or explicitly referenced manifest, emits JSON, and preserves inputs", async () => {
  const directory = await mkdtemp(path.join(os.tmpdir(), "auth-plan-check-"));
  const command = fileURLToPath(
    new URL("../check-auth-performance-plan.mjs", import.meta.url),
  );
  try {
    const input = fixture("cache");
    const files = {
      "plan.json": input.plan,
      "results.json": input.results,
      "manifest.json": input.manifest,
    };
    for (const [name, value] of Object.entries(files))
      await writeFile(path.join(directory, name), JSON.stringify(value));
    const argv = [
      command,
      "--plan",
      path.join(directory, "plan.json"),
      "--results",
      path.join(directory, "results.json"),
      "--phase",
      "cache",
    ];
    let child = spawnSync(process.execPath, argv, { encoding: "utf8" });
    assert.equal(child.status, 0, child.stdout + child.stderr);
    assert.equal(JSON.parse(child.stdout).passed, true);
    for (const [name, value] of Object.entries(files))
      assert.equal(
        await readFile(path.join(directory, name), "utf8"),
        JSON.stringify(value),
      );
    input.results.manifest_file = "referenced-manifest.json";
    input.manifest.options.roles = ["candidate"];
    await writeFile(
      path.join(directory, "referenced-manifest.json"),
      JSON.stringify(input.manifest),
    );
    await writeFile(
      path.join(directory, "results.json"),
      JSON.stringify(input.results),
    );
    child = spawnSync(process.execPath, argv, { encoding: "utf8" });
    assert.equal(child.status, 1);
    assert.match(JSON.parse(child.stdout).failures.join("\n"), /single-role/);
    child = spawnSync(process.execPath, [command, "--unknown", "x"], {
      encoding: "utf8",
    });
    assert.equal(child.status, 1);
    assert.equal(JSON.parse(child.stdout).passed, false);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
