import test from "node:test";
import assert from "node:assert/strict";
import {
  mkdtemp,
  mkdir,
  writeFile,
  readFile,
  readdir,
  rm,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { buildSuite } from "../auth-performance-suite.mjs";

const config = {
  baseline: { name: "base", env: { FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT: "32" } },
  candidates: [
    {
      name: "candidate-z",
      metadata: { rust_opt_level: "z" },
      env: { FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT: "1024" },
    },
    { name: "candidate-3", metadata: { rust_opt_level: "3" } },
  ],
};

test("formal suite fixes six pairs/20s warm/60s load without overriding bridge product defaults", () => {
  const plan = buildSuite(config, {
    compilers: "3",
    readers: "1,2,4",
    concurrency: "16,64",
    "credential-kind": "totp,password",
  });
  assert.equal(plan.jobs.length, 12);
  assert.equal(plan.gate, "--require-six-pairs");
  for (const job of plan.jobs) {
    assert.equal(job.args["--pairs"], "6");
    assert.equal(job.args["--warmup"], "20");
    assert.equal(job.args["--seconds"], "60");
    for (const variant of [job.config.baseline, ...job.config.candidates]) {
      assert.equal(variant.env.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT, undefined);
      assert.equal(variant.env.FN_KNOCK_TOKIO_WORKER_THREADS, "2");
    }
  }
});

test("soak/profile/recovery remain distinct and renewal cannot masquerade as sustained soak", () => {
  const [soak] = buildSuite(config, {
    mode: "soak",
    candidates: "candidate-z",
  }).jobs;
  assert.equal(soak.args["--seconds"], "1800");
  assert.equal(soak.args["--roles"], "candidate");
  assert.equal(soak.args["--pairs"], "1");
  const [profile] = buildSuite(config, { mode: "profile" }).jobs;
  assert.equal(profile.args["--profile"], "1");
  assert.equal(profile.args["--seconds"], "30");
  const [recovery] = buildSuite(config, { mode: "recovery" }).jobs;
  assert.equal(recovery.args["--recovery-probe"], "1");
  assert.equal(
    recovery.config.baseline.env.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT,
    "32",
  );
  assert.equal(
    recovery.config.candidates[0].env.FN_KNOCK_AUTH_BRIDGE_MAX_IN_FLIGHT,
    "32",
  );
  assert.throws(
    () => buildSuite(config, { routes: "grant_renewal" }),
    /explicit --renewals/,
  );
  assert.throws(
    () =>
      buildSuite(config, {
        mode: "soak",
        routes: "grant_renewal",
        renewals: "64",
      }),
    /not a soak/,
  );
});

test("collection allows JSON measurements only and excludes database, keys, configs and binaries", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "auth-suite-test-"));
  try {
    const output = path.join(directory, "case");
    await mkdir(path.join(output, "trial"), { recursive: true });
    await writeFile(
      path.join(output, "results.json"),
      JSON.stringify({ metric: 12 }),
    );
    await writeFile(
      path.join(output, "manifest.json"),
      JSON.stringify({
        env: {
          HMAC_SECRET: "synthetic-key",
          FN_KNOCK_INTERNAL_RPC_TOKEN: "synthetic-token",
        },
      }),
    );
    await writeFile(
      path.join(output, "trial", "result.json"),
      JSON.stringify({ samples: [1, 2] }),
    );
    for (const name of [
      "state.sqlite3",
      "private.key",
      "server-admin-rs",
      "config.json",
      "rust.log",
    ])
      await writeFile(path.join(output, "trial", name), "excluded");
    const plan = path.join(directory, "plan.json");
    await writeFile(
      plan,
      JSON.stringify({
        schema_version: 1,
        mode: "paired",
        jobs: [{ name: "case", output }],
      }),
    );
    const destination = path.join(directory, "collected");
    execFileSync(process.execPath, [
      fileURLToPath(new URL("../auth-performance-suite.mjs", import.meta.url)),
      "collect",
      "--plan",
      plan,
      "--out",
      destination,
    ]);
    assert.deepEqual(await readdir(path.join(destination, "case", "trial")), [
      "result.json",
    ]);
    const manifest = JSON.parse(
      await readFile(path.join(destination, "case", "manifest.json"), "utf8"),
    );
    assert.equal(manifest.env.HMAC_SECRET, "[REDACTED]");
    assert.equal(manifest.env.FN_KNOCK_INTERNAL_RPC_TOKEN, "[REDACTED]");
    const collection = JSON.parse(
      await readFile(path.join(destination, "collection.json"), "utf8"),
    );
    assert.equal(collection.collected.length, 3);
    assert.ok(
      collection.collected.every(
        (file) =>
          file.source_sha256.length === 64 &&
          file.collected_sha256.length === 64,
      ),
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
