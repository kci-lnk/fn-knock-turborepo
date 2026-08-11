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
    gateway_rss_p95_bytes: 230,
  });
  assert.equal(percentile([1, 2, 3, 4, 5], 0.95), 5);
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
