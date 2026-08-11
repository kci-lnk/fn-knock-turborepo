#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function fail(message) {
  throw new Error(`[turbo-contract] ${message}`);
}

const result = spawnSync(
  process.platform === "win32" ? "npx.cmd" : "npx",
  [
    "turbo",
    "run",
    "build",
    "check-types",
    "test",
    "--filter=server-admin-rs",
    "--filter=fn-knock-desktop",
    "--filter=server-admin-view",
    "--filter=server-auth-view",
    "--dry=json",
  ],
  { cwd: root, encoding: "utf8" },
);
if (result.status !== 0) {
  fail([result.stdout, result.stderr].filter(Boolean).join("\n").trim());
}

let dryRun;
try {
  dryRun = JSON.parse(result.stdout);
} catch (error) {
  fail(`Turbo dry-run did not return JSON: ${error.message}`);
}

const tasks = new Map(dryRun.tasks.map((task) => [task.taskId, task]));

function assertInputs(taskId, requiredInputs) {
  const task = tasks.get(taskId);
  if (!task) fail(`missing dry-run task ${taskId}`);
  const inputs = Object.keys(task.inputs ?? {});
  for (const requiredInput of requiredInputs) {
    if (
      !inputs.some(
        (input) =>
          input === requiredInput || input.startsWith(`${requiredInput}/`),
      )
    ) {
      fail(`${taskId} does not hash ${requiredInput}`);
    }
  }
}

for (const taskId of ["server-admin-view#build", "server-auth-view#build"]) {
  assertInputs(taskId, ["../../scripts/create-precompressed-assets.mjs"]);
}

const rustInputs = [
  "../../version.json",
  "../../packages/grpc-contracts",
  "../../packages/wol-protocol-rs/Cargo.toml",
  "../../packages/wol-protocol-rs/src",
];
for (const taskName of ["build", "check-types", "test"]) {
  assertInputs(`server-admin-rs#${taskName}`, rustInputs);
}

const rustBuild = tasks.get("server-admin-rs#build");
if (rustBuild.cache?.local || rustBuild.cache?.remote) {
  fail("server-admin-rs#build must not use Turbo artifact caching");
}
if ((rustBuild.outputs ?? []).length !== 0) {
  fail("server-admin-rs#build must not claim frontend dist outputs");
}

for (const taskName of ["build", "check-types"]) {
  assertInputs(`fn-knock-desktop#${taskName}`, [
    "../../version.json",
    "../../packages/grpc-contracts",
  ]);
}

const turboConfig = JSON.parse(
  readFileSync(path.join(root, "turbo.json"), "utf8"),
);
for (const taskName of ["build", "check-types"]) {
  const task = turboConfig.tasks[`fn-knock-desktop#${taskName}`];
  if (!task?.inputs?.includes("bundle/windows/runtime/bundle.json")) {
    fail(
      `fn-knock-desktop#${taskName} does not declare the Windows bundle identity`,
    );
  }
}

console.log(
  "[turbo-contract] external inputs and native cache policy are valid",
);
