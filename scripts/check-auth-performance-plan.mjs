import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { isDeepStrictEqual, parseArgs } from "node:util";
import {
  compareRuns,
  comparisonFailures,
  pairOrder,
} from "./auth-performance-lib.mjs";

const roles = ["baseline", "candidate"];
const usage = `Usage: node scripts/check-auth-performance-plan.mjs \
  --plan EXPERIMENT_PLAN.json --results results.json --phase main|cache|renewal

Reads manifest.json beside results.json (or its explicit manifest_file).
Cache/renewal inherit unspecified workload fields from primary_matrix.
Renewal uses ordinary_grants and a finite renewal_tokens_per_phase batch;
measurement_deadline_seconds limits request starts, not sustained-load duration.
Uses existing comparison gates unchanged. Prints JSON; exits 1 on rejection.
Inputs are never changed. This checks recorded evidence, not live binaries.`;

function expectedPhase(plan, phase) {
  assert.ok(["main", "cache", "renewal"].includes(phase), "invalid phase");
  const primary = plan.primary_matrix;
  const matrix = phase === "main" ? primary : plan[`${phase}_matrix`];
  assert.ok(primary && matrix, "missing declared matrix");
  const expected = { ...primary, ...matrix };
  expected.routes =
    phase === "main"
      ? [...primary.target_routes, ...primary.guard_routes]
      : [matrix.route];
  expected.targets = phase === "main" ? primary.target_routes : [];
  expected.grants =
    phase === "renewal" ? matrix.ordinary_grants : expected.grants;
  expected.seconds =
    phase === "renewal"
      ? matrix.measurement_deadline_seconds
      : expected.measurement_seconds;
  expected.renewals = phase === "renewal" ? matrix.renewal_tokens_per_phase : 0;
  assert.equal(expected.pairs, 6, "phase must declare six pairs");
  assert.ok(expected.routes.length > 0, "plan has no routes");
  assert.equal(
    new Set(expected.routes).size,
    expected.routes.length,
    "duplicate plan route",
  );
  assert.ok(
    expected.routes.every((name) => typeof name === "string" && name),
    "invalid plan route",
  );
  for (const name of [
    "concurrency",
    "client_workers",
    "warmup_seconds",
    "seconds",
    "accounts",
    "sessions",
    "grants",
  ])
    assert.ok(
      Number.isInteger(expected[name]) && expected[name] > 0,
      `invalid plan ${name}`,
    );
  assert.ok(
    [0, 1].includes(expected.cache_ttl_seconds),
    "invalid plan cache TTL",
  );
  if (phase === "renewal") {
    assert.equal(
      matrix.route,
      "grant_renewal",
      "renewal route must be grant_renewal",
    );
    assert.ok(
      Number.isInteger(expected.renewals) && expected.renewals > 0,
      "invalid renewal batch size",
    );
  } else
    assert.ok(
      !expected.routes.includes("grant_renewal"),
      "finite renewal requires renewal phase",
    );
  for (const role of roles)
    for (const component of ["rust", "go"])
      assert.match(
        plan[role]?.[component] ?? "",
        /^[a-f0-9]{40}$/,
        `invalid ${role} ${component} source SHA`,
      );
  return expected;
}

// Pure offline checker; every failure is retained rather than filtering rows
// until a comparison happens to pass. Performance statistics stay in the lib.
export function checkPerformancePlan({ plan, results, manifest, phase }) {
  const expected = expectedPhase(plan, phase);
  const failures = [];
  const check = (condition, message) => {
    if (!condition) failures.push(message);
  };
  const equal = (actual, wanted, name) =>
    check(
      isDeepStrictEqual(actual, wanted),
      `${name}: expected ${JSON.stringify(wanted)}, got ${JSON.stringify(actual)}`,
    );
  const setEqual = (actual, wanted, name) => {
    check(
      Array.isArray(actual) &&
        actual.length === wanted.length &&
        new Set(actual).size === actual.length &&
        wanted.every((value) => actual.includes(value)),
      `${name}: expected exactly ${wanted.join(",")}`,
    );
  };
  equal(results?.schema_version, 1, "results schema");
  equal(manifest?.schema_version, 1, "manifest schema");
  const options = manifest?.options ?? {};
  const config = manifest?.config ?? {};
  const optionFields = {
    pairs: expected.pairs,
    concurrency: expected.concurrency,
    clients: expected.client_workers,
    warmup: expected.warmup_seconds,
    seconds: expected.seconds,
    accounts: expected.accounts,
    sessions: expected.sessions,
    grants: expected.grants,
    credentialKind: expected.credential_kind,
    cacheTtl: expected.cache_ttl_seconds,
    profile: false,
    recoveryProbe: false,
  };
  for (const [name, value] of Object.entries(optionFields))
    equal(options[name], value, `manifest.options.${name}`);
  if (phase === "renewal")
    equal(options.renewals, expected.renewals, "manifest.options.renewals");
  setEqual(
    options.roles,
    roles,
    "manifest roles (single-role/soak is excluded)",
  );
  setEqual(options.routes, expected.routes, "manifest routes");

  const candidates = Array.isArray(config.candidates) ? config.candidates : [];
  if (options.selected !== undefined)
    check(
      Array.isArray(options.selected) &&
        new Set(options.selected).size === options.selected.length,
      "manifest selected candidates must be a unique array",
    );
  const selected = candidates.filter(
    (item) =>
      options.selected === undefined || options.selected?.includes(item.name),
  );
  equal(selected.length, 1, "selected candidate count");
  const variants = { baseline: config.baseline, candidate: selected[0] };
  const candidateName = variants.candidate?.name;
  check(
    typeof candidateName === "string" && candidateName.length > 0,
    "missing candidate name",
  );
  check(
    variants.baseline?.name !== candidateName,
    "baseline and candidate variant names must differ",
  );
  const metadata = (value, role, prefix) => {
    equal(value?.main_commit, plan[role].rust, `${prefix}.main_commit`);
    equal(value?.go_commit, plan[role].go, `${prefix}.go_commit`);
    if (role === "candidate") {
      equal(
        value?.opt_level ?? value?.rust_opt_level,
        plan.candidate.opt_level,
        `${prefix}.opt_level`,
      );
      if (value?.rust_opt_level !== undefined)
        equal(
          value.rust_opt_level,
          plan.candidate.opt_level,
          `${prefix}.rust_opt_level`,
        );
      equal(value?.lto, plan.candidate.lto, `${prefix}.lto`);
      equal(
        value?.codegen_units,
        plan.candidate.codegen_units,
        `${prefix}.codegen_units`,
      );
    }
  };
  for (const role of roles)
    metadata(variants[role]?.metadata, role, `manifest ${role} metadata`);
  const identities = Array.isArray(manifest?.identities)
    ? manifest.identities
    : [];
  equal(identities.length, 4, "manifest binary identity count");
  for (const role of roles)
    for (const component of ["rust", "go"]) {
      const matches = identities.filter(
        (entry) =>
          entry.variant === variants[role]?.name &&
          entry.component === component,
      );
      equal(matches.length, 1, `${role}/${component} identity count`);
      if (matches.length === 1) {
        equal(
          matches[0].path,
          variants[role]?.[component],
          `${role}/${component} binary path`,
        );
        check(
          /^[a-f0-9]{64}$/.test(matches[0].sha256 ?? ""),
          `${role}/${component} missing binary SHA256`,
        );
      }
    }

  const runs = Array.isArray(results?.runs) ? results.runs : [];
  equal(
    runs.length,
    expected.routes.length * expected.pairs * 2,
    "trial count",
  );
  setEqual(
    [...new Set(runs.map((row) => row.scenario))],
    expected.routes,
    "observed routes",
  );
  const seen = new Set();
  const directories = new Set();
  const processIdentities = new Set();
  const byPair = new Map();
  const duration = (measurement, seconds, prefix, finite = false) => {
    const elapsed = measurement?.elapsed_ms;
    check(
      Number.isFinite(elapsed) && elapsed > 0,
      `${prefix}: missing positive actual elapsed_ms`,
    );
    if (!finite)
      check(
        elapsed >= seconds * 1000 &&
          elapsed <= seconds * 1000 + Math.max(500, seconds * 50),
        `${prefix}: actual duration outside declared window`,
      );
    equal(measurement?.failures, 0, `${prefix}.failures`);
  };
  runs.forEach((row, index) => {
    const prefix = `trial[${index}] ${row.scenario}/${row.pair}/${row.role}`;
    check(roles.includes(row.role), `${prefix}: unknown role`);
    check(
      Number.isInteger(row.pair) && row.pair >= 0 && row.pair < expected.pairs,
      `${prefix}: pair outside declared range`,
    );
    equal(row.candidate, candidateName, `${prefix}.candidate`);
    equal(row.variant, variants[row.role]?.name, `${prefix}.variant`);
    if (roles.includes(row.role)) {
      metadata(row.variant_metadata, row.role, `${prefix} metadata`);
      equal(
        row.variant_metadata,
        variants[row.role]?.metadata,
        `${prefix} metadata vs manifest`,
      );
      equal(
        row.env,
        variants[row.role]?.env ?? {},
        `${prefix} env vs manifest`,
      );
    }
    equal(row.concurrency, expected.concurrency, `${prefix}.concurrency`);
    equal(
      row.cache_ttl_seconds,
      expected.cache_ttl_seconds,
      `${prefix}.cache_ttl_seconds`,
    );
    equal(row.profiling, false, `${prefix}: profiling excluded`);
    equal(row.recovery_probe, false, `${prefix}: recovery excluded`);
    setEqual(row.roles, roles, `${prefix} roles`);
    equal(row.validation?.passed, true, `${prefix} validation`);
    for (const name of ["sampling_ok", "health_ok", "client_ok", "duration_ok"])
      equal(row.quality?.[name], true, `${prefix} quality.${name}`);
    const env = row.effective_runtime_env ?? {};
    equal(env.FN_KNOCK_RUNTIME_TARGET, "linux", `${prefix} runtime target`);
    equal(env.FN_KNOCK_TOKIO_WORKER_THREADS, "2", `${prefix} Tokio workers`);
    equal(
      env.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT,
      null,
      `${prefix} bridge capacity override excluded`,
    );
    const reader = env.FN_KNOCK_SQLITE_AUTH_READERS;
    const readers = reader === null || reader === "" ? 1 : Number(reader);
    equal(
      readers,
      row.role === "candidate" ? plan.candidate.auth_readers : 1,
      `${prefix} auth readers`,
    );
    const seedFields = {
      scenario: row.scenario,
      accounts: expected.accounts,
      sessions: expected.sessions,
      ordinary_grants: expected.grants,
      credential_kind: expected.credential_kind,
      renewal_tokens_per_phase: expected.renewals,
      total_grants:
        expected.grants + (phase === "renewal" ? 1 + 2 * expected.renewals : 0),
    };
    for (const [name, value] of Object.entries(seedFields))
      equal(row.seed?.[name], value, `${prefix} seed.${name}`);
    for (const name of ["cache_control", "cache_control_after"]) {
      equal(
        row[name]?.runtime_verified,
        true,
        `${prefix} ${name}.runtime_verified`,
      );
      for (const field of [
        "auth_cache_ttl_seconds",
        "auth_cache_unauthorized_ttl_seconds",
      ])
        equal(
          row[name]?.[field],
          expected.cache_ttl_seconds,
          `${prefix} ${name}.${field}`,
        );
    }
    duration(
      row.warmup?.measurement,
      expected.warmup_seconds,
      `${prefix} warmup`,
    );
    duration(
      row.measurement,
      expected.seconds,
      `${prefix} load`,
      phase === "renewal",
    );
    if (phase === "renewal") {
      equal(
        row.measurement?.all_tokens_consumed,
        true,
        `${prefix} finite batch consumed`,
      );
      equal(
        row.measurement?.requests,
        expected.renewals,
        `${prefix} finite batch requests`,
      );
      equal(
        row.measurement?.successful_requests,
        expected.renewals,
        `${prefix} finite batch successes`,
      );
    }
    const key = `${row.scenario}/${row.pair}/${row.role}`;
    check(!seen.has(key), `${prefix}: duplicate pair/role`);
    seen.add(key);
    const pairKey = `${row.scenario}/${row.pair}`;
    if (!byPair.has(pairKey)) byPair.set(pairKey, []);
    byPair.get(pairKey).push(row.role);
    check(
      typeof row.directory === "string" &&
        row.directory.length > 0 &&
        !directories.has(row.directory),
      `${prefix}: missing or reused trial directory`,
    );
    directories.add(row.directory);
    if (expected.fresh_processes_and_database_per_trial)
      for (const component of ["go", "rust"]) {
        const before = row.resources?.[component]?.before;
        const after = row.resources?.[component]?.after;
        const validIdentity = [before, after].every(
          (sample) =>
            Number.isSafeInteger(sample?.pid) &&
            sample.pid > 0 &&
            Number.isSafeInteger(sample?.start_ticks) &&
            sample.start_ticks > 0,
        );
        check(
          validIdentity,
          `${prefix} ${component}: missing process identity`,
        );
        if (validIdentity) {
          const identity = `${before.pid}/${before.start_ticks}`;
          equal(
            `${after.pid}/${after.start_ticks}`,
            identity,
            `${prefix} ${component}: process changed during load`,
          );
          check(
            !processIdentities.has(identity),
            `${prefix} ${component}: process reused across trials`,
          );
          processIdentities.add(identity);
        }
      }
  });
  for (const route of expected.routes)
    for (let pair = 0; pair < expected.pairs; pair++) {
      const key = `${route}/${pair}`;
      equal(
        byPair.get(key),
        pairOrder(pair),
        `${key} AB/BA order and complete roles`,
      );
    }
  const comparison = compareRuns(runs).map((group) => {
    const route = group.key.split("/")[0];
    const improvement = expected.targets.includes(route);
    const gateFailures = comparisonFailures([group], {
      requireSixPairs: true,
      requireImprovement: improvement,
    });
    failures.push(...gateFailures);
    return {
      ...group,
      gate: improvement ? "improvement" : "no_regression",
      gate_failures: gateFailures,
    };
  });
  equal(comparison.length, expected.routes.length, "comparison group count");
  return {
    schema_version: 1,
    phase,
    passed: failures.length === 0,
    failures,
    expected: {
      ...optionFields,
      routes: expected.routes,
      target_routes: expected.targets,
      renewal_tokens_per_phase: expected.renewals,
    },
    measurement_semantics:
      phase === "renewal"
        ? "finite unique-token batch; actual elapsed, not sustained 60-second throughput"
        : "fixed-duration closed-loop paired workload",
    observed_load_elapsed_ms: runs.map((row) => ({
      route: row.scenario,
      pair: row.pair,
      role: row.role,
      elapsed_ms: row.measurement?.elapsed_ms,
    })),
    identities,
    comparison,
  };
}

async function main() {
  const { values } = parseArgs({
    options: {
      plan: { type: "string" },
      results: { type: "string" },
      phase: { type: "string" },
      help: { type: "boolean" },
    },
  });
  if (values.help) {
    console.log(usage);
    return;
  }
  assert.ok(values.plan && values.results && values.phase, usage);
  const planPath = path.resolve(values.plan);
  const resultsPath = path.resolve(values.results);
  const [planText, resultsText] = await Promise.all([
    readFile(planPath, "utf8"),
    readFile(resultsPath, "utf8"),
  ]);
  const results = JSON.parse(resultsText);
  if (results.manifest_file !== undefined)
    assert.equal(
      typeof results.manifest_file,
      "string",
      "manifest_file must be a path string",
    );
  const manifestPath = path.resolve(
    path.dirname(resultsPath),
    results.manifest_file ?? "manifest.json",
  );
  const manifestText = await readFile(manifestPath, "utf8");
  const report = checkPerformancePlan({
    plan: JSON.parse(planText),
    results,
    manifest: JSON.parse(manifestText),
    phase: values.phase,
  });
  report.inputs = Object.fromEntries(
    [
      ["plan", planPath, planText],
      ["results", resultsPath, resultsText],
      ["manifest", manifestPath, manifestText],
    ].map(([name, file, text]) => [
      name,
      { path: file, sha256: createHash("sha256").update(text).digest("hex") },
    ]),
  );
  console.log(JSON.stringify(report, null, 2));
  if (!report.passed) process.exitCode = 1;
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  main().catch((error) => {
    console.log(
      JSON.stringify(
        { schema_version: 1, passed: false, failures: [error.message] },
        null,
        2,
      ),
    );
    process.exitCode = 1;
  });
}
