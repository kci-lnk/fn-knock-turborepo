import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import {
  compareRuntimeSummaries,
  percentile,
  summarizeRuntimeSamples,
} from "../runtime-performance-lib.mjs";

const execFileAsync = promisify(execFile);
const rootDir = path.resolve(import.meta.dirname, "../..");
const checkScript = path.join(rootDir, "scripts/check-runtime-performance.mjs");

const performanceResult = (summary) =>
  JSON.stringify({ schema_version: 1, summary });

const gatewayLoadRssFields = [
  "gateway_load_peak_rss_p95_bytes",
  "gateway_post_load_rss_p95_bytes",
  "gateway_post_reclaim_rss_p95_bytes",
];
const gatewayLoadBaseline = {
  readiness_p95_ms: 100,
  management_rss_p95_bytes: 1_000,
  ...Object.fromEntries(gatewayLoadRssFields.map((field) => [field, 10_000])),
};

test("runtime performance summary uses nearest-rank p95", () => {
  const summary = summarizeRuntimeSamples([
    { readiness_ms: 20, management_rss_bytes: 100, gateway_rss_bytes: 200 },
    { readiness_ms: 10, management_rss_bytes: 110, gateway_rss_bytes: 210 },
    { readiness_ms: 40, management_rss_bytes: 120, gateway_rss_bytes: 220 },
    { readiness_ms: 30, management_rss_bytes: 130, gateway_rss_bytes: 230 },
  ]);
  assert.deepEqual(summary, {
    readiness_p95_ms: 40,
    management_rss_p95_bytes: 130,
    management_lifetime_peak_rss_p95_bytes: null,
    management_load_peak_rss_p95_bytes: null,
    management_post_load_rss_p95_bytes: null,
    management_post_reclaim_rss_p95_bytes: null,
    management_locale_rps_p50: null,
    gateway_rss_p95_bytes: 230,
    gateway_load_peak_rss_p95_bytes: null,
    gateway_post_load_rss_p95_bytes: null,
    gateway_post_reclaim_rss_p95_bytes: null,
    proxy_2mib_rps_p50: null,
  });
  assert.equal(percentile([1, 2, 3, 4, 5], 0.95), 5);
});

test("runtime performance schema v2 summarizes stable and load checkpoints", () => {
  const sample = (stable, peak, retained, reclaimed, throughput) => ({
    readiness_ms: 10,
    checkpoints: {
      stable_10s: {
        management_rss_bytes: stable,
        gateway_rss_bytes: stable * 2,
      },
      load_peak: { gateway_rss_bytes: peak, management_rss_bytes: peak / 2 },
      post_load_30s: {
        gateway_rss_bytes: retained,
        management_rss_bytes: retained / 2,
      },
      post_reclaim: {
        gateway_rss_bytes: reclaimed,
        management_rss_bytes: reclaimed / 2,
      },
    },
    loads: [
      { name: "proxy_2mib", requests_per_second: throughput },
      { name: "management_locale", requests_per_second: throughput * 10 },
    ],
  });
  const summary = summarizeRuntimeSamples([
    sample(100, 500, 400, 300, 90),
    sample(110, 550, 440, 330, 100),
  ]);
  assert.deepEqual(summary, {
    readiness_p95_ms: 10,
    management_rss_p95_bytes: 110,
    management_lifetime_peak_rss_p95_bytes: null,
    management_load_peak_rss_p95_bytes: 275,
    management_post_load_rss_p95_bytes: 220,
    management_post_reclaim_rss_p95_bytes: 165,
    management_locale_rps_p50: 900,
    gateway_rss_p95_bytes: 220,
    gateway_load_peak_rss_p95_bytes: 550,
    gateway_post_load_rss_p95_bytes: 440,
    gateway_post_reclaim_rss_p95_bytes: 330,
    proxy_2mib_rps_p50: 90,
  });
});

test("Rust load and retained RSS regressions cannot hide behind unchanged idle RSS", () => {
  const sample = (peak, retained) => ({
    readiness_ms: 100,
    checkpoints: {
      stable_10s: { management_rss_bytes: 32, gateway_rss_bytes: 64 },
      load_peak: { management_rss_bytes: peak },
      post_load_30s: { management_rss_bytes: retained },
      post_reclaim: { management_rss_bytes: retained },
    },
  });
  const base = summarizeRuntimeSamples([sample(48, 32)]);
  const current = summarizeRuntimeSamples([sample(1024, 768)]);
  const failures = compareRuntimeSummaries(base, current, {
    readiness: 0.1,
    rss: 0.05,
  });
  assert.equal(failures.length, 3);
  for (const stage of ["load_peak", "post_load", "post_reclaim"]) {
    assert.ok(
      failures.some((failure) =>
        failure.startsWith(`management_${stage}_rss_p95_bytes regressed`),
      ),
    );
  }
});

test("Linux high-water RSS catches bursts between periodic samples", () => {
  const sample = (peak) => ({
    readiness_ms: 100,
    checkpoints: {
      stable_10s: { management_rss_bytes: 32, management_peak_rss_bytes: 32 },
      load_peak: { management_rss_bytes: 32, management_peak_rss_bytes: peak },
      post_reclaim: {
        management_rss_bytes: 32,
        management_peak_rss_bytes: peak,
      },
    },
  });
  const base = summarizeRuntimeSamples([sample(32)]);
  const current = summarizeRuntimeSamples([sample(512)]);
  assert.equal(current.management_load_peak_rss_p95_bytes, 32);
  assert.equal(current.management_lifetime_peak_rss_p95_bytes, 512);
  const failures = compareRuntimeSummaries(base, current, { rss: 0.05 });
  assert.equal(failures.length, 1);
  assert.match(
    failures[0],
    /^management_lifetime_peak_rss_p95_bytes regressed/,
  );
});

test("missing Rust load metrics and management throughput regression fail the gate", () => {
  const base = {
    readiness_p95_ms: 100,
    management_rss_p95_bytes: 100,
    management_load_peak_rss_p95_bytes: 100,
    management_post_load_rss_p95_bytes: 80,
    management_locale_rps_p50: 1000,
  };
  const failures = compareRuntimeSummaries(
    base,
    {
      readiness_p95_ms: 100,
      management_rss_p95_bytes: 100,
      management_locale_rps_p50: 800,
    },
    { rss: 0.05, throughput: 0.05 },
  );
  assert.equal(failures.length, 3);
  assert.ok(
    failures.some((failure) =>
      failure.includes("management_load_peak_rss_p95_bytes is missing"),
    ),
  );
  assert.ok(
    failures.some((failure) =>
      failure.includes("management_post_load_rss_p95_bytes is missing"),
    ),
  );
  assert.ok(
    failures.some((failure) =>
      failure.includes("management_locale_rps_p50 regressed"),
    ),
  );
});

test("unchanged Go load RSS passes the default regression gate", () => {
  assert.deepEqual(
    compareRuntimeSummaries(gatewayLoadBaseline, gatewayLoadBaseline),
    [],
  );
  for (const field of gatewayLoadRssFields) {
    const atLimit = { ...gatewayLoadBaseline, [field]: 10_500 };
    assert.deepEqual(compareRuntimeSummaries(gatewayLoadBaseline, atLimit), []);
    assert.deepEqual(
      compareRuntimeSummaries(gatewayLoadBaseline, atLimit, {
        loadRssImprovement: 0,
      }),
      [],
    );
    const failures = compareRuntimeSummaries(gatewayLoadBaseline, {
      ...gatewayLoadBaseline,
      [field]: 10_501,
    });
    assert.equal(failures.length, 1);
    assert.ok(failures[0].startsWith(`${field} regressed`));
    assert.deepEqual(
      compareRuntimeSummaries(
        gatewayLoadBaseline,
        { ...gatewayLoadBaseline, [field]: 11_000 },
        { rss: 0.1 },
      ),
      [],
    );
  }
});

test("an explicit load RSS improvement target rejects unchanged Go metrics", () => {
  const failures = compareRuntimeSummaries(
    gatewayLoadBaseline,
    gatewayLoadBaseline,
    { loadRssImprovement: 0.2 },
  );
  assert.equal(failures.length, 3);
  for (const [index, field] of gatewayLoadRssFields.entries()) {
    assert.ok(failures[index].startsWith(`${field} improved 0.0%`));
    assert.match(failures[index], /required 20\.0%/);
  }
});

test("load RSS improvement requirements must be finite fractions", () => {
  const base = { readiness_p95_ms: 100, management_rss_p95_bytes: 100 };
  for (const value of [-0.01, 1.01, NaN, Infinity, "0.2", null]) {
    assert.deepEqual(
      compareRuntimeSummaries(base, base, { loadRssImprovement: value }),
      ["load RSS has an invalid improvement requirement"],
    );
  }
  assert.deepEqual(
    compareRuntimeSummaries(
      gatewayLoadBaseline,
      {
        ...gatewayLoadBaseline,
        ...Object.fromEntries(gatewayLoadRssFields.map((field) => [field, 0])),
      },
      { loadRssImprovement: 1 },
    ),
    [],
  );
});

test("runtime comparison enforces throughput and explicit load RSS improvements", () => {
  const failures = compareRuntimeSummaries(
    {
      readiness_p95_ms: 100,
      management_rss_p95_bytes: 1_000,
      gateway_rss_p95_bytes: 2_000,
      gateway_load_peak_rss_p95_bytes: 10_000,
      gateway_post_load_rss_p95_bytes: 8_000,
      gateway_post_reclaim_rss_p95_bytes: 5_000,
      proxy_2mib_rps_p50: 100,
    },
    {
      readiness_p95_ms: 100,
      management_rss_p95_bytes: 1_000,
      gateway_rss_p95_bytes: 2_000,
      gateway_load_peak_rss_p95_bytes: 8_000,
      gateway_post_load_rss_p95_bytes: 6_400,
      gateway_post_reclaim_rss_p95_bytes: 4_000,
      proxy_2mib_rps_p50: 95,
    },
    {
      readiness: 0.1,
      rss: 0.05,
      throughput: 0.05,
      loadRssImprovement: 0.2,
    },
  );
  assert.deepEqual(failures, []);
});

test("runtime performance comparison permits configured regressions", () => {
  const failures = compareRuntimeSummaries(
    {
      readiness_p95_ms: 100,
      management_rss_p95_bytes: 1_000,
      gateway_rss_p95_bytes: null,
    },
    {
      readiness_p95_ms: 110,
      management_rss_p95_bytes: 1_050,
      gateway_rss_p95_bytes: null,
    },
    { readiness: 0.1, rss: 0.05 },
  );
  assert.deepEqual(failures, []);
});

test("runtime performance comparison reports missing and regressed metrics", () => {
  const failures = compareRuntimeSummaries(
    {
      readiness_p95_ms: 100,
      management_rss_p95_bytes: 1_000,
      gateway_rss_p95_bytes: 2_000,
    },
    {
      readiness_p95_ms: 111,
      management_rss_p95_bytes: null,
      gateway_rss_p95_bytes: 2_101,
    },
    { readiness: 0.1, rss: 0.05 },
  );
  assert.equal(failures.length, 3);
  assert.match(failures[0], /readiness_p95_ms regressed/);
  assert.match(failures[1], /management_rss_p95_bytes is missing/);
  assert.match(failures[2], /gateway_rss_p95_bytes regressed/);
});

test("runtime performance CLI enforces explicitly configured PR tolerances", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "fn-knock-runtime-perf-"),
  );
  const basePath = path.join(directory, "base.json");
  const currentPath = path.join(directory, "current.json");
  const base = {
    readiness_p95_ms: 100,
    management_rss_p95_bytes: 1_000,
    gateway_rss_p95_bytes: 2_000,
  };

  try {
    await writeFile(basePath, performanceResult(base));
    await writeFile(
      currentPath,
      performanceResult({
        readiness_p95_ms: 110,
        management_rss_p95_bytes: 1_050,
        gateway_rss_p95_bytes: 2_100,
      }),
    );
    const passed = await execFileAsync(process.execPath, [
      checkScript,
      "--base",
      basePath,
      "--current",
      currentPath,
      "--max-readiness-regression",
      "0.10",
      "--max-rss-regression",
      "0.05",
    ]);
    assert.match(passed.stdout, /\[runtime-performance\] passed/);

    await writeFile(
      currentPath,
      performanceResult({
        ...base,
        readiness_p95_ms: 111,
      }),
    );
    await assert.rejects(
      execFileAsync(process.execPath, [
        checkScript,
        "--base",
        basePath,
        "--current",
        currentPath,
        "--max-readiness-regression",
        "0.10",
        "--max-rss-regression",
        "0.05",
      ]),
      /runtime performance regression: readiness_p95_ms regressed/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("runtime gate rejects empty summaries, non-finite metrics and invalid tolerances", () => {
  assert.equal(compareRuntimeSummaries({}, {}).length, 2);
  const valid = { readiness_p95_ms: 100, management_rss_p95_bytes: 100 };
  assert.match(
    compareRuntimeSummaries(valid, {
      ...valid,
      management_rss_p95_bytes: Infinity,
    })[0],
    /management_rss_p95_bytes is missing from the current/,
  );
  assert.match(
    compareRuntimeSummaries(
      valid,
      { ...valid, management_rss_p95_bytes: 500 },
      { rss: NaN },
    )[0],
    /invalid regression tolerance/,
  );
  assert.throws(() => percentile([1, Infinity], 0.95), /finite non-negative/);
  assert.throws(
    () =>
      summarizeRuntimeSamples([{ readiness_ms: -1, management_rss_bytes: 10 }]),
    /finite non-negative/,
  );
});

test("runtime performance CLI cannot pass with two empty summaries", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "fn-knock-runtime-empty-"),
  );
  const file = path.join(directory, "empty.json");
  try {
    await writeFile(file, performanceResult({}));
    await assert.rejects(
      execFileAsync(process.execPath, [
        checkScript,
        "--base",
        file,
        "--current",
        file,
      ]),
      /runtime performance regression: readiness_p95_ms is missing/,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test("runtime performance CLI uses a regression gate unless improvement is requested", async () => {
  const directory = await mkdtemp(
    path.join(os.tmpdir(), "fn-knock-runtime-load-gate-"),
  );
  const basePath = path.join(directory, "base.json");
  const currentPath = path.join(directory, "current.json");
  const args = [checkScript, "--base", basePath, "--current", currentPath];
  try {
    await writeFile(basePath, performanceResult(gatewayLoadBaseline));
    for (const currentRss of [10_000, 10_500]) {
      await writeFile(
        currentPath,
        performanceResult({
          ...gatewayLoadBaseline,
          ...Object.fromEntries(
            gatewayLoadRssFields.map((field) => [field, currentRss]),
          ),
        }),
      );
      const passed = await execFileAsync(process.execPath, args);
      assert.match(passed.stdout, /\[runtime-performance\] passed/);
    }
    await writeFile(
      currentPath,
      performanceResult({
        ...gatewayLoadBaseline,
        gateway_post_reclaim_rss_p95_bytes: 10_501,
      }),
    );
    await assert.rejects(
      execFileAsync(process.execPath, args),
      /gateway_post_reclaim_rss_p95_bytes regressed/,
    );
    await writeFile(currentPath, performanceResult(gatewayLoadBaseline));
    await assert.rejects(
      execFileAsync(process.execPath, [
        ...args,
        "--min-load-rss-improvement",
        "0.2",
      ]),
      /required 20\.0%/,
    );
    for (const value of ["", " ", "-0.01", "1.01", "NaN", "Infinity", "abc"]) {
      await assert.rejects(
        execFileAsync(process.execPath, [
          ...args,
          "--min-load-rss-improvement",
          value,
        ]),
        /--min-load-rss-improvement must be a fraction from 0 to 1/,
      );
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
