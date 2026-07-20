#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const SCRIPT_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const ROOT_DIR = path.resolve(process.env.FN_KNOCK_ROOT_DIR ?? SCRIPT_ROOT);
const GO_REPOSITORY = path.resolve(
  process.env.FN_KNOCK_GO_REAUTH_PROXY_DIR ??
    path.join(ROOT_DIR, "..", "Go-Reauth-Proxy"),
);
const VERSION_EXPRESSION = /^[0-9]+\.[0-9]+\.[0-9]+$/;

function fail(message) {
  throw new Error(`[release] ${message}`);
}

function run(command, args, { allowFailure = false, inherit = false } = {}) {
  const result = spawnSync(command, args, {
    cwd: ROOT_DIR,
    encoding: "utf8",
    env: process.env,
    stdio: inherit ? "inherit" : "pipe",
  });
  if (result.error) fail(`failed to run ${command}: ${result.error.message}`);
  if (result.status !== 0 && !allowFailure) {
    const detail = [result.stdout, result.stderr]
      .filter(Boolean)
      .map((value) => value.trim())
      .filter(Boolean)
      .join("\n");
    fail(`${command} ${args.join(" ")} failed${detail ? `:\n${detail}` : ""}`);
  }
  return {
    ok: result.status === 0,
    stdout: (result.stdout ?? "").trim(),
    stderr: (result.stderr ?? "").trim(),
  };
}

function parseVersion(value, label = "version") {
  if (!VERSION_EXPRESSION.test(value)) {
    fail(`${label} must match X.Y.Z: ${value}`);
  }
  const parts = value.split(".").map(Number);
  if (parts.some((part) => !Number.isSafeInteger(part))) {
    fail(`${label} contains an unsafe numeric component: ${value}`);
  }
  return parts;
}

function compareVersions(left, right) {
  const leftParts = parseVersion(left);
  const rightParts = parseVersion(right);
  for (let index = 0; index < 3; index += 1) {
    if (leftParts[index] !== rightParts[index]) {
      return leftParts[index] < rightParts[index] ? -1 : 1;
    }
  }
  return 0;
}

function resolveTargetVersion(current, target) {
  const [major, minor, patch] = parseVersion(current, "current version");
  if (target === "patch") return `${major}.${minor}.${patch + 1}`;
  if (target === "minor") return `${major}.${minor + 1}.0`;
  if (target === "major") return `${major + 1}.0.0`;
  parseVersion(target, "target version");
  return target;
}

function replaceSingle(content, expression, replacement, label) {
  const matches = [...content.matchAll(expression)];
  if (matches.length !== 1) {
    fail(
      `${label}: expected exactly one version field, found ${matches.length}`,
    );
  }
  return content.replace(expression, replacement);
}

function readJsonVersion(content, label) {
  let document;
  try {
    document = JSON.parse(content);
  } catch (error) {
    fail(`${label}: invalid JSON: ${error.message}`);
  }
  if (typeof document.version !== "string")
    fail(`${label}: missing string version`);
  return document.version;
}

function updateJsonVersion(content, nextVersion, label) {
  return replaceSingle(
    content,
    /^(\s*"version"\s*:\s*")[^"]+(".*)$/gm,
    `$1${nextVersion}$2`,
    label,
  );
}

function findCargoPackageSection(content, label) {
  const startMatch = /^\[package\]\s*$/m.exec(content);
  if (!startMatch) fail(`${label}: missing [package] section`);
  const bodyStart = startMatch.index + startMatch[0].length;
  const remaining = content.slice(bodyStart);
  const nextSection = /^\[/m.exec(remaining);
  const bodyEnd = nextSection ? bodyStart + nextSection.index : content.length;
  return { bodyStart, bodyEnd };
}

function readCargoPackageVersion(content, label) {
  const { bodyStart, bodyEnd } = findCargoPackageSection(content, label);
  const section = content.slice(bodyStart, bodyEnd);
  const match = /^\s*version\s*=\s*"([^"]+)"\s*$/m.exec(section);
  if (!match) fail(`${label}: missing package version`);
  return match[1];
}

function updateCargoPackageVersion(content, nextVersion, label) {
  const { bodyStart, bodyEnd } = findCargoPackageSection(content, label);
  const section = content.slice(bodyStart, bodyEnd);
  const updated = replaceSingle(
    section,
    /^(\s*version\s*=\s*")[^"]+(".*)$/gm,
    `$1${nextVersion}$2`,
    label,
  );
  return `${content.slice(0, bodyStart)}${updated}${content.slice(bodyEnd)}`;
}

function findCargoLockPackage(content, packageName, label) {
  const markers = [...content.matchAll(/^\[\[package\]\]\s*$/gm)];
  for (let index = 0; index < markers.length; index += 1) {
    const start = markers[index].index;
    const end = markers[index + 1]?.index ?? content.length;
    const block = content.slice(start, end);
    const name = /^name\s*=\s*"([^"]+)"\s*$/m.exec(block)?.[1];
    if (name === packageName) return { start, end, block };
  }
  fail(`${label}: missing package ${packageName}`);
}

function readCargoLockVersion(content, packageName, label) {
  const { block } = findCargoLockPackage(content, packageName, label);
  const match = /^version\s*=\s*"([^"]+)"\s*$/m.exec(block);
  if (!match) fail(`${label}: missing version for ${packageName}`);
  return match[1];
}

function updateCargoLockVersion(content, packageName, nextVersion, label) {
  const { start, end, block } = findCargoLockPackage(
    content,
    packageName,
    label,
  );
  const updated = replaceSingle(
    block,
    /^(version\s*=\s*")[^"]+(".*)$/gm,
    `$1${nextVersion}$2`,
    `${label} (${packageName})`,
  );
  return `${content.slice(0, start)}${updated}${content.slice(end)}`;
}

function readDesktopLockVersion(content, label) {
  let document;
  try {
    document = JSON.parse(content);
  } catch (error) {
    fail(`${label}: invalid JSON: ${error.message}`);
  }
  const version = document.packages?.["apps/fn-knock-desktop"]?.version;
  if (typeof version !== "string") {
    fail(`${label}: missing packages["apps/fn-knock-desktop"].version`);
  }
  return version;
}

function updateDesktopLockVersion(content, nextVersion, label) {
  return replaceSingle(
    content,
    /("apps\/fn-knock-desktop"\s*:\s*\{\s*\n\s*"version"\s*:\s*")[^"]+(")/g,
    `$1${nextVersion}$2`,
    label,
  );
}

const VERSION_FILES = [
  {
    label: "root version",
    relativePath: "version.json",
    read: readJsonVersion,
    update: updateJsonVersion,
  },
  {
    label: "fnOS manifest",
    relativePath: "apps/fn-knock/manifest",
    read(content, label) {
      const match = /^version=(.+)$/m.exec(content);
      if (!match) fail(`${label}: missing version`);
      return match[1];
    },
    update(content, nextVersion, label) {
      return replaceSingle(
        content,
        /^version=.*$/gm,
        `version=${nextVersion}`,
        label,
      );
    },
  },
  {
    label: "server-admin-rs Cargo",
    relativePath: "apps/server-admin-rs/Cargo.toml",
    read: readCargoPackageVersion,
    update: updateCargoPackageVersion,
  },
  {
    label: "server-admin-rs lock",
    relativePath: "apps/server-admin-rs/Cargo.lock",
    read(content, label) {
      return readCargoLockVersion(content, "server-admin-rs", label);
    },
    update(content, nextVersion, label) {
      return updateCargoLockVersion(
        content,
        "server-admin-rs",
        nextVersion,
        label,
      );
    },
  },
  {
    label: "desktop package",
    relativePath: "apps/fn-knock-desktop/package.json",
    read: readJsonVersion,
    update: updateJsonVersion,
  },
  {
    label: "desktop package lock",
    relativePath: "package-lock.json",
    read: readDesktopLockVersion,
    update: updateDesktopLockVersion,
  },
  {
    label: "desktop Cargo",
    relativePath: "apps/fn-knock-desktop/native/Cargo.toml",
    read: readCargoPackageVersion,
    update: updateCargoPackageVersion,
  },
  {
    label: "desktop Cargo lock",
    relativePath: "apps/fn-knock-desktop/native/Cargo.lock",
    read(content, label) {
      return readCargoLockVersion(content, "fn-knock-desktop", label);
    },
    update(content, nextVersion, label) {
      return updateCargoLockVersion(
        content,
        "fn-knock-desktop",
        nextVersion,
        label,
      );
    },
  },
];

const GATEWAY_VERSION_FILES = [
  {
    label: "Go gateway source version",
    relativePath: "pkg/version/version.go",
    read(content, label) {
      const match = /^\s*Version\s*=\s*"([^"]+)"\s*$/m.exec(content);
      if (!match) fail(`${label}: missing Version variable`);
      return match[1];
    },
    update(content, nextVersion, label) {
      return replaceSingle(
        content,
        /^(\s*Version\s*=\s*")[^"]+(".*)$/gm,
        `$1${nextVersion}$2`,
        label,
      );
    },
  },
  {
    label: "Go gateway Taskfile version",
    relativePath: "Taskfile.yml",
    read(content, label) {
      const match =
        /^\s*VERSION:\s*['"]\{\{\.FN_KNOCK_VERSION\s+\|\s+default\s+"([^"]+)"\}\}['"]\s*$/m.exec(
          content,
        );
      if (!match) fail(`${label}: missing FN_KNOCK_VERSION default`);
      return match[1];
    },
    update(content, nextVersion, label) {
      return replaceSingle(
        content,
        /^(\s*VERSION:\s*['"]\{\{\.FN_KNOCK_VERSION\s+\|\s+default\s+")[^"]+("\}\}['"].*)$/gm,
        `$1${nextVersion}$2`,
        label,
      );
    },
  },
];

async function loadVersionFiles() {
  return Promise.all(
    VERSION_FILES.map(async (definition) => {
      const absolutePath = path.join(ROOT_DIR, definition.relativePath);
      const content = await readFile(absolutePath, "utf8");
      return {
        ...definition,
        absolutePath,
        content,
        version: definition.read(content, definition.label),
      };
    }),
  );
}

async function loadGatewayVersionFiles() {
  return Promise.all(
    GATEWAY_VERSION_FILES.map(async (definition) => {
      const absolutePath = path.join(GO_REPOSITORY, definition.relativePath);
      const content = await readFile(absolutePath, "utf8").catch((error) => {
        if (error.code === "ENOENT") {
          fail(
            `Go-Reauth-Proxy file is missing: ${absolutePath}; set FN_KNOCK_GO_REAUTH_PROXY_DIR to override the repository path`,
          );
        }
        throw error;
      });
      return {
        ...definition,
        absolutePath,
        content,
        version: definition.read(content, definition.label),
      };
    }),
  );
}

function assertVersionsAligned(files) {
  const current = files.find(
    (file) => file.relativePath === "version.json",
  )?.version;
  parseVersion(current, "current version");
  const mismatches = files.filter((file) => file.version !== current);
  if (mismatches.length > 0) {
    fail(
      `version files are not aligned with version.json (${current}):\n${mismatches
        .map((file) => `  ${file.relativePath}: ${file.version}`)
        .join("\n")}`,
    );
  }
  return current;
}

function assertGatewayVersionsAligned(files, expectedVersion) {
  const mismatches = files.filter((file) => file.version !== expectedVersion);
  if (mismatches.length > 0) {
    fail(
      `Go gateway versions are not aligned with fn-knock ${expectedVersion}:\n${mismatches
        .map((file) => `  ${file.relativePath}: ${file.version}`)
        .join("\n")}`,
    );
  }
}

function git(args, options) {
  return run("git", ["-C", ROOT_DIR, ...args], options);
}

function gatewayGit(args, options) {
  return run("git", ["-C", GO_REPOSITORY, ...args], options);
}

function assertGitRepository(repository, gitCommand) {
  if (
    !gitCommand(["rev-parse", "--is-inside-work-tree"], {
      allowFailure: true,
    }).ok
  ) {
    fail(`${repository} is not a Git worktree`);
  }
}

function gitStatus() {
  return git(["status", "--porcelain", "--untracked-files=normal"]).stdout;
}

function gatewayGitStatus() {
  return gatewayGit(["status", "--porcelain", "--untracked-files=normal"])
    .stdout;
}

function tagExists(tag) {
  return git(["tag", "--list", tag]).stdout === tag;
}

function findBaseTag(currentVersion) {
  const currentTag = `v${currentVersion}`;
  if (tagExists(currentTag)) return currentTag;
  const described = git(
    [
      "describe",
      "--tags",
      "--abbrev=0",
      "--match",
      "v[0-9]*.[0-9]*.[0-9]*",
      "HEAD",
    ],
    { allowFailure: true },
  );
  return described.ok ? described.stdout : "";
}

function generateReleaseNotes(currentVersion, nextVersion) {
  const baseTag = findBaseTag(currentVersion);
  const range = baseTag ? `${baseTag}..HEAD` : "HEAD";
  const subjects = git(["log", "--no-merges", "--format=%s", range])
    .stdout.split("\n")
    .map((subject) => subject.trim())
    .filter(Boolean);
  if (subjects.length === 0) {
    fail(
      `no commits were found for automatic release notes (${range}); provide --notes-file <path>`,
    );
  }
  return {
    baseTag: baseTag || "<repository start>",
    content: `# fn-knock ${nextVersion}\n\n${subjects.map((subject) => `- ${subject}`).join("\n")}\n`,
  };
}

async function resolveReleaseNotes(currentVersion, nextVersion, notesFile) {
  const targetPath = path.join(ROOT_DIR, "release-notes", `${nextVersion}.md`);
  if (notesFile) {
    const sourcePath = path.resolve(process.cwd(), notesFile);
    const content = await readFile(sourcePath, "utf8");
    if (!content.trim()) fail(`release notes file is empty: ${sourcePath}`);
    return { targetPath, content, source: sourcePath };
  }

  const existing = await readFile(targetPath, "utf8").catch((error) => {
    if (error.code === "ENOENT") return "";
    throw error;
  });
  if (existing.trim()) {
    return { targetPath, content: existing, source: "existing release notes" };
  }

  const generated = generateReleaseNotes(currentVersion, nextVersion);
  return {
    targetPath,
    content: generated.content,
    source: `Git commits after ${generated.baseTag}`,
  };
}

function runPreflight(version) {
  const script = path.join(ROOT_DIR, "scripts", "release-preflight.sh");
  const result = spawnSync("bash", [script, `v${version}`], {
    cwd: ROOT_DIR,
    env: { ...process.env, FN_KNOCK_ROOT_DIR: ROOT_DIR },
    stdio: "inherit",
  });
  if (result.error)
    fail(`failed to run release preflight: ${result.error.message}`);
  if (result.status !== 0) fail(`release preflight failed for v${version}`);
}

async function showStatus() {
  assertGitRepository(ROOT_DIR, git);
  assertGitRepository(GO_REPOSITORY, gatewayGit);
  const files = await loadVersionFiles();
  const gatewayFiles = await loadGatewayVersionFiles();
  const current = files.find(
    (file) => file.relativePath === "version.json",
  )?.version;
  console.log(`fn-knock release status`);
  console.log(`  root: ${ROOT_DIR}`);
  console.log(`  version: ${current}`);
  for (const file of files) {
    const marker = file.version === current ? "ok" : "mismatch";
    console.log(`  [${marker}] ${file.relativePath}: ${file.version}`);
  }
  const notesPath = path.join(ROOT_DIR, "release-notes", `${current}.md`);
  const notes = await readFile(notesPath, "utf8").catch(() => "");
  console.log(
    `  [${notes.trim() ? "ok" : "missing"}] release-notes/${current}.md`,
  );
  console.log(`  [${gitStatus() ? "dirty" : "clean"}] Git worktree`);
  console.log(
    `  [${tagExists(`v${current}`) ? "exists" : "missing"}] tag v${current}`,
  );
  console.log(`  Go gateway: ${GO_REPOSITORY}`);
  for (const file of gatewayFiles) {
    const marker = file.version === current ? "ok" : "mismatch";
    console.log(
      `  [${marker}] Go-Reauth-Proxy/${file.relativePath}: ${file.version}`,
    );
  }
  console.log(
    `  [${gatewayGitStatus() ? "dirty" : "clean"}] Go gateway Git worktree`,
  );
  assertVersionsAligned(files);
  assertGatewayVersionsAligned(gatewayFiles, current);
}

async function checkRelease(versionArgument) {
  assertGitRepository(ROOT_DIR, git);
  assertGitRepository(GO_REPOSITORY, gatewayGit);
  const files = await loadVersionFiles();
  const gatewayFiles = await loadGatewayVersionFiles();
  const current = assertVersionsAligned(files);
  const version = versionArgument ?? current;
  if (version !== current) {
    fail(`check version ${version} does not match version.json ${current}`);
  }
  assertGatewayVersionsAligned(gatewayFiles, version);
  runPreflight(version);
  run("git", ["-C", ROOT_DIR, "diff", "--check"]);
  run("git", ["-C", GO_REPOSITORY, "diff", "--check"]);
  console.log(`[release] v${version} is ready for the full release test suite`);
  console.log(`[release] next: bun run fn-knock:release:test`);
}

async function checkGateway(versionArgument) {
  assertGitRepository(GO_REPOSITORY, gatewayGit);
  const gatewayFiles = await loadGatewayVersionFiles();
  let version = versionArgument;
  if (!version) {
    const rootVersion = JSON.parse(
      await readFile(path.join(ROOT_DIR, "version.json"), "utf8"),
    ).version;
    version = rootVersion;
  }
  parseVersion(version, "expected Go gateway version");
  assertGatewayVersionsAligned(gatewayFiles, version);
  console.log(
    `[release] Go-Reauth-Proxy source and Taskfile versions match ${version}`,
  );
}

async function prepareRelease(targetArgument, options) {
  if (!targetArgument)
    fail("prepare requires a target version, patch, minor, or major");
  assertGitRepository(ROOT_DIR, git);
  assertGitRepository(GO_REPOSITORY, gatewayGit);
  const files = await loadVersionFiles();
  const gatewayFiles = await loadGatewayVersionFiles();
  const current = assertVersionsAligned(files);
  const nextVersion = resolveTargetVersion(current, targetArgument);
  if (compareVersions(nextVersion, current) <= 0) {
    fail(
      `target version ${nextVersion} must be greater than current version ${current}`,
    );
  }

  const nextTag = `v${nextVersion}`;
  if (tagExists(nextTag)) fail(`tag already exists locally: ${nextTag}`);

  const dirty = gitStatus();
  const gatewayDirty = gatewayGitStatus();
  if ((dirty || gatewayDirty) && !options.allowDirty && !options.dryRun) {
    fail(
      `Git worktrees must be clean; commit or stash existing changes first, or rerun with --allow-dirty:${
        dirty ? `\nfn-knock:\n${dirty}` : ""
      }${gatewayDirty ? `\nGo-Reauth-Proxy:\n${gatewayDirty}` : ""}`,
    );
  }

  const notes = await resolveReleaseNotes(
    current,
    nextVersion,
    options.notesFile,
  );
  const updates = files.map((file) => ({
    ...file,
    updatedContent: file.update(file.content, nextVersion, file.label),
  }));
  const gatewayUpdates = gatewayFiles.map((file) => ({
    ...file,
    updatedContent: file.update(file.content, nextVersion, file.label),
  }));

  console.log(`[release] ${current} -> ${nextVersion}`);
  console.log(`[release] release notes: ${notes.source}`);
  console.log(`[release] files to update:`);
  for (const update of updates) console.log(`  ${update.relativePath}`);
  console.log(`  ${path.relative(ROOT_DIR, notes.targetPath)}`);
  for (const update of gatewayUpdates) {
    console.log(`  Go-Reauth-Proxy/${update.relativePath}`);
  }

  if (options.dryRun) {
    console.log(`\n[release] release notes preview:\n`);
    process.stdout.write(notes.content);
    console.log(`\n[release] dry-run complete; no files were changed`);
    return;
  }

  for (const update of updates) {
    await writeFile(update.absolutePath, update.updatedContent, "utf8");
  }
  for (const update of gatewayUpdates) {
    await writeFile(update.absolutePath, update.updatedContent, "utf8");
  }
  await mkdir(path.dirname(notes.targetPath), { recursive: true });
  await writeFile(notes.targetPath, notes.content, "utf8");

  runPreflight(nextVersion);
  await checkGateway(nextVersion);
  console.log(`[release] prepared ${nextTag}`);
  console.log(
    `[release] review release-notes/${nextVersion}.md and both Git diffs`,
  );
  console.log(
    `[release] commit and push Go-Reauth-Proxy before creating the fn-knock tag`,
  );
  console.log(
    `[release] next: bun run release check && bun run fn-knock:release:test`,
  );
}

function printHelp() {
  console.log(`fn-knock release helper

Usage:
  bun run release status
  bun run release prepare <X.Y.Z|patch|minor|major> [options]
  bun run release check [X.Y.Z]
  bun run release gateway-check [X.Y.Z]

Prepare options:
  --dry-run             Preview changes without writing files
  --notes-file <path>   Use an existing Markdown file as release notes
  --allow-dirty         Allow preparation in a dirty Git worktree

Go-Reauth-Proxy defaults to ../Go-Reauth-Proxy. Override it with
FN_KNOCK_GO_REAUTH_PROXY_DIR when the repository is elsewhere.

The helper never commits, tags, pushes, or publishes a release.`);
}

async function main() {
  const rawArgs = process.argv.slice(2);
  if (rawArgs.length === 0 || rawArgs[0] === "--help" || rawArgs[0] === "-h") {
    printHelp();
    return;
  }
  const [command, ...args] = rawArgs;
  const parsed = parseArgs({
    args,
    allowPositionals: true,
    strict: true,
    options: {
      "allow-dirty": { type: "boolean", default: false },
      "dry-run": { type: "boolean", default: false },
      help: { type: "boolean", short: "h", default: false },
      "notes-file": { type: "string" },
    },
  });
  if (parsed.values.help || command === "help") {
    printHelp();
    return;
  }

  if (command === "status") {
    if (parsed.positionals.length > 0)
      fail("status does not accept positional arguments");
    await showStatus();
    return;
  }
  if (command === "check") {
    if (parsed.positionals.length > 1)
      fail("check accepts at most one version");
    await checkRelease(parsed.positionals[0]);
    return;
  }
  if (command === "gateway-check") {
    if (parsed.positionals.length > 1)
      fail("gateway-check accepts at most one version");
    await checkGateway(parsed.positionals[0]);
    return;
  }
  if (command === "prepare") {
    if (parsed.positionals.length !== 1) {
      fail(
        "prepare requires exactly one target version, patch, minor, or major",
      );
    }
    await prepareRelease(parsed.positionals[0], {
      allowDirty: parsed.values["allow-dirty"],
      dryRun: parsed.values["dry-run"],
      notesFile: parsed.values["notes-file"],
    });
    return;
  }
  fail(`unknown command: ${command}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
