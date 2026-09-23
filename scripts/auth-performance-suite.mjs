// Prepare serial Linux experiment jobs locally; execution is an explicit verb.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn } from "node:child_process";
import {
  readFile,
  writeFile,
  mkdir,
  readdir,
  lstat,
  open,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { scenarios } from "./auth-performance-lib.mjs";

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const capacityKey = "FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT";
const list = (value) => value?.split(",");
const safeName = (value) => assert.match(value, /^[a-zA-Z0-9_-]+$/);

export function buildSuite(config, options) {
  const mode = options.mode ?? "paired";
  assert.ok(
    ["paired", "soak", "profile", "recovery"].includes(mode),
    "unknown suite mode",
  );
  const routes = list(options.routes) ?? ["session_hit"];
  routes.forEach((route) =>
    assert.ok(scenarios.includes(route), `unknown route ${route}`),
  );
  if (routes.includes("grant_renewal")) {
    assert.ok(
      options.renewals,
      "finite renewal cases require an explicit --renewals batch size",
    );
    assert.ok(
      mode === "paired" || mode === "profile",
      "finite renewal is not a soak/recovery workload",
    );
  }
  const selectedNames = list(options.candidates);
  const compilers = list(options.compilers);
  const selected = config.candidates.filter(
    (variant) =>
      (!selectedNames || selectedNames.includes(variant.name)) &&
      (!compilers ||
        compilers.includes(String(variant.metadata?.rust_opt_level))),
  );
  assert.ok(
    selected.length,
    "no selected candidate (compiler filter uses metadata.rust_opt_level)",
  );
  const readers = list(options.readers) ?? ["1"];
  readers.forEach((reader) =>
    assert.ok(["1", "2", "4"].includes(reader), "readers must be 1,2,4"),
  );
  const concurrencies = (list(options.concurrency) ?? ["16"]).map(Number);
  concurrencies.forEach((value) =>
    assert.ok(
      Number.isInteger(value) && value >= 1 && value <= 1024,
      "invalid concurrency",
    ),
  );
  const kinds = list(options["credential-kind"]) ?? ["totp"];
  kinds.forEach((kind) =>
    assert.ok(["totp", "password"].includes(kind), "invalid credential kind"),
  );
  const bounded = (name, fallback, min, max) => {
    const value = Number(options[name] ?? fallback);
    assert.ok(
      Number.isInteger(value) && value >= min && value <= max,
      `invalid ${name}`,
    );
    return String(value);
  };
  const common = {
    "--clients": bounded("clients", 2, 1, 32),
    "--accounts": bounded("accounts", 1, 1, 1000),
    "--sessions": bounded("sessions", 64, 1, 10000),
    "--grants": bounded("grants", 2, 1, 10000),
    "--cache-ttl": bounded("cache-ttl", 0, 0, 1),
    "--renewals": bounded("renewals", 64, 1, 100000),
  };
  const normalizeVariant = (variant, reader) => {
    safeName(variant.name);
    const env = { ...variant.env, FN_KNOCK_TOKIO_WORKER_THREADS: "2" };
    // Normal comparisons retain each product's capacity default. Recovery is
    // an intentionally reduced-capacity fixture, never a production claim.
    delete env[capacityKey];
    if (mode === "recovery") env[capacityKey] = "32";
    if (reader) env.FN_KNOCK_SQLITE_AUTH_READERS = reader;
    else delete env.FN_KNOCK_SQLITE_AUTH_READERS;
    return { ...variant, env };
  };
  const jobs = [];
  for (const candidate of selected)
    for (const reader of readers)
      for (const concurrency of concurrencies)
        for (const kind of kinds)
          for (const route of routes) {
            const name = `${mode}-${candidate.name}-r${reader}-c${concurrency}-${kind}-${route}`;
            const args = {
              ...common,
              "--routes": route,
              "--credential-kind": kind,
              "--concurrency": String(concurrency),
              "--pairs": mode === "paired" ? "6" : "1",
              "--warmup": mode === "recovery" ? "1" : "20",
              "--seconds":
                mode === "paired"
                  ? "60"
                  : mode === "soak"
                    ? "1800"
                    : mode === "profile"
                      ? "30"
                      : "3",
            };
            if (mode === "soak") args["--roles"] = "candidate";
            if (mode === "profile")
              Object.assign(args, { "--profile": "1", "--profile-idle": "5" });
            if (mode === "recovery") args["--recovery-probe"] = "1";
            jobs.push({
              name,
              config: {
                ...config,
                baseline: normalizeVariant(config.baseline),
                candidates: [normalizeVariant(candidate, reader)],
              },
              args,
            });
          }
  assert.ok(
    new Set(jobs.map((job) => job.name)).size === jobs.length,
    "duplicate suite cases",
  );
  return {
    schema_version: 1,
    mode,
    jobs,
    gate:
      mode === "paired"
        ? options.improvement === "1"
          ? "--require-improvement"
          : "--require-six-pairs"
        : null,
    environment_note:
      "Normal jobs leave bridge capacity unset: Go default 1024; Rust linux CPU*16 clamped 32..256 (4 CPU =>64). Recovery explicitly sets32. Tokio2 is fixed, not a default-tuning experiment.",
    measurement_note:
      "RSS peaks cover sampled load only, not startup/warmup or idle retention. Renewal is finite. Soak is candidate-only and never paired evidence. Existing client/health quality gates remain unchanged.",
  };
}

function parseOptions(args) {
  const options = {};
  const allowed = new Set([
    "config",
    "out",
    "plan",
    "mode",
    "routes",
    "candidates",
    "compilers",
    "readers",
    "concurrency",
    "credential-kind",
    "clients",
    "accounts",
    "sessions",
    "grants",
    "cache-ttl",
    "renewals",
    "improvement",
  ]);
  for (let index = 0; index < args.length; index += 2) {
    assert.ok(
      args[index].startsWith("--") && args[index + 1] !== undefined,
      "use --name value pairs",
    );
    const name = args[index].slice(2);
    assert.ok(allowed.has(name), `unknown option ${name}`);
    options[name] = args[index + 1];
  }
  return options;
}

async function prepare(options) {
  assert.ok(
    options.config && options.out,
    "prepare requires --config and --out NEW_DIRECTORY",
  );
  const config = JSON.parse(await readFile(options.config, "utf8"));
  const plan = buildSuite(config, options);
  const root = path.resolve(options.out);
  await mkdir(root);
  for (const job of plan.jobs) {
    job.config_file = path.join(root, `${job.name}.config.json`);
    job.output = path.join(root, job.name);
    await writeFile(job.config_file, JSON.stringify(job.config, null, 2));
    delete job.config;
  }
  await writeFile(
    path.join(root, "suite-plan.json"),
    JSON.stringify(plan, null, 2),
  );
  process.stdout.write(
    `${path.join(root, "suite-plan.json")} (${plan.jobs.length} serial jobs; nothing executed)\n`,
  );
}

async function command(binary, args, logFile) {
  const file = await open(logFile, "wx");
  const log = file.createWriteStream();
  const child = spawn(binary, args, { stdio: ["ignore", "pipe", "pipe"] });
  const interrupt = () => child.kill("SIGTERM");
  process.once("SIGINT", interrupt);
  process.once("SIGTERM", interrupt);
  const finish = () => {
    process.off("SIGINT", interrupt);
    process.off("SIGTERM", interrupt);
    log.end();
  };
  child.stdout.pipe(log, { end: false });
  child.stderr.pipe(log, { end: false });
  child.stdout.pipe(process.stdout);
  child.stderr.pipe(process.stderr);
  return new Promise((resolve, reject) => {
    child.once("error", (error) => {
      finish();
      reject(error);
    });
    child.once("close", (code) => {
      finish();
      resolve(code ?? 1);
    });
  });
}

async function execute(options) {
  assert.equal(
    process.platform,
    "linux",
    "execution requires isolated Linux harness support",
  );
  assert.ok(options.plan, "run requires --plan suite-plan.json");
  const plan = JSON.parse(await readFile(options.plan, "utf8"));
  assert.equal(plan.schema_version, 1);
  for (const job of plan.jobs) {
    process.stderr.write(`[suite] ${job.name}\n`);
    const code = await command(
      "bash",
      [
        path.join(scriptDirectory, "run-auth-performance-isolated.sh"),
        "--config",
        job.config_file,
        "--out",
        job.output,
        ...Object.entries(job.args).flat(),
      ],
      `${job.output}.run.log`,
    );
    const outcome = { harness_exit: code, gate_exit: null };
    if (code === 0 && plan.gate)
      outcome.gate_exit = await command(
        process.execPath,
        [
          path.join(scriptDirectory, "check-auth-performance.mjs"),
          path.join(job.output, "results.json"),
          plan.gate,
        ],
        `${job.output}.gate.log`,
      );
    await writeFile(
      `${job.output}.outcome.json`,
      JSON.stringify(outcome, null, 2),
    );
    assert.ok(
      code === 0 && (outcome.gate_exit === null || outcome.gate_exit === 0),
      `suite stopped on failed case ${job.name}; all existing evidence retained`,
    );
  }
}

export function redactCollection(value) {
  if (Array.isArray(value)) return value.map(redactCollection);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [
      key,
      /(?:^|_)(?:secret|password|token|cookie|authorization|hmac)(?:_|$)/i.test(
        key,
      ) && typeof item === "string"
        ? "[REDACTED]"
        : redactCollection(item),
    ]),
  );
}

async function collect(options) {
  assert.ok(
    options.plan && options.out,
    "collect requires --plan and --out NEW_DIRECTORY",
  );
  const plan = JSON.parse(await readFile(options.plan, "utf8"));
  const destination = path.resolve(options.out);
  await mkdir(destination);
  const collected = [];
  const copyJson = async (source, relative) => {
    try {
      const stat = await lstat(source);
      assert.ok(
        stat.isFile() && !stat.isSymbolicLink(),
        "refusing non-regular evidence file",
      );
      const raw = await readFile(source, "utf8");
      const body = JSON.stringify(redactCollection(JSON.parse(raw)), null, 2);
      const target = path.join(destination, relative);
      await mkdir(path.dirname(target), { recursive: true });
      await writeFile(target, body, { flag: "wx" });
      collected.push({
        source,
        relative,
        source_sha256: createHash("sha256").update(raw).digest("hex"),
        collected_sha256: createHash("sha256").update(body).digest("hex"),
      });
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
  };
  for (const job of plan.jobs) {
    safeName(job.name);
    for (const name of ["results.json", "manifest.json"])
      await copyJson(path.join(job.output, name), path.join(job.name, name));
    await copyJson(`${job.output}.outcome.json`, `${job.name}/outcome.json`);
    let entries = [];
    try {
      entries = await readdir(job.output, { withFileTypes: true });
    } catch (error) {
      if (error.code !== "ENOENT") throw error;
    }
    for (const entry of entries.filter((entry) => entry.isDirectory())) {
      safeName(entry.name);
      await copyJson(
        path.join(job.output, entry.name, "result.json"),
        path.join(job.name, entry.name, "result.json"),
      );
    }
  }
  await writeFile(
    path.join(destination, "collection.json"),
    JSON.stringify(
      {
        mode: plan.mode,
        collected,
        policy:
          "JSON metrics/manifest/outcome allowlist only. No DB, key files, configs, logs, assets or binaries. Named secret values redacted; hashes record source and collected bytes.",
      },
      null,
      2,
    ),
  );
  process.stdout.write(
    `${destination} (${collected.length} JSON evidence files)\n`,
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  const verb = process.argv[2];
  const action = { prepare, run: execute, collect }[verb];
  assert.ok(
    action,
    "usage: node scripts/auth-performance-suite.mjs prepare|run|collect --name value ...",
  );
  await action(parseOptions(process.argv.slice(3)));
}
