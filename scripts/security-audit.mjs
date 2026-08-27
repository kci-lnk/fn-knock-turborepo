import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

function runJson(command, args) {
  if (command === "npm" && !process.env.npm_execpath) {
    throw new Error("npm_execpath is unavailable; run this gate via npm");
  }
  const executable = command === "npm" ? process.execPath : command;
  const commandArgs =
    command === "npm" ? [process.env.npm_execpath, ...args] : args;
  const result = spawnSync(executable, commandArgs, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 10 * 1024 * 1024,
  });

  if (result.error) {
    throw new Error(`failed to run ${command}: ${result.error.message}`);
  }

  try {
    return JSON.parse(result.stdout);
  } catch {
    const details = [result.stdout, result.stderr].filter(Boolean).join("\n");
    throw new Error(`${command} did not return JSON\n${details}`);
  }
}

function auditNodeDependencies() {
  const report = runJson("npm", ["audit", "--json"]);
  const counts = report.metadata?.vulnerabilities;
  if (!counts || counts.total !== 0) {
    throw new Error(
      `npm audit found ${counts?.total ?? "unknown"} vulnerabilities`,
    );
  }
  console.log("[security] npm dependency audit passed");
}

const rustAllowlist = new Map([
  [
    "apps/server-admin-rs/Cargo.lock",
    {
      vulnerabilities: new Set([
        "RUSTSEC-2023-0071:rsa:0.9.10",
        "RUSTSEC-2023-0071:rsa:0.10.0-rc.18",
      ]),
      warnings: new Set(["unmaintained:paste:1.0.15"]),
      manifest: "apps/server-admin-rs/Cargo.toml",
      reviewedPackages: new Map([
        ["crypto-glue", "0.1.15"],
        ["paste", "1.0.15"],
        ["rsa", ["0.9.10", "0.10.0-rc.18"]],
        ["webauthn-attestation-ca", "0.6.1-dev"],
        ["webauthn-rs", "0.6.1-dev"],
        ["webauthn-rs-core", "0.6.1-dev"],
      ]),
      rationale:
        "rsa 0.9.10 is transitive through webauthn-rs and is used only for public-key signature verification. rsa 0.10.0-rc.18 is required by russh for explicit user-supplied RSA SSH keys; it performs a private-key signature only during a user-initiated SSH authentication attempt, and no patched russh-compatible RSA release exists. The SSH target can observe signature timing, so Ed25519 remains the preferred credential. paste is a compile-time transitive dependency of utoipa-axum and has an unmaintained warning, not a vulnerability.",
    },
  ],
]);

function rustFindings(report) {
  const vulnerabilities = (report.vulnerabilities?.list ?? []).map(
    ({ advisory, package: dependency }) =>
      `${advisory.id}:${dependency.name}:${dependency.version}`,
  );
  const warnings = Object.values(report.warnings ?? {})
    .flat()
    .map(
      ({ kind, package: dependency }) =>
        `${kind}:${dependency.name}:${dependency.version}`,
    );
  return { vulnerabilities, warnings };
}

function assertExactFindings(actual, allowed, label) {
  const unexpected = actual.filter((finding) => !allowed.has(finding));
  const missing = [...allowed].filter((finding) => !actual.includes(finding));
  if (actual.length !== allowed.size || unexpected.length || missing.length) {
    throw new Error(
      `${label} changed; unexpected=[${unexpected.join(", ")}], missing=[${missing.join(", ")}]`,
    );
  }
}

function assertReviewedRustPackages(policy) {
  if (!policy.manifest || !policy.reviewedPackages) return;

  const metadata = runJson("cargo", [
    "metadata",
    "--locked",
    "--format-version",
    "1",
    "--manifest-path",
    policy.manifest,
  ]);
  const versionsByName = new Map();
  for (const dependency of metadata.packages) {
    const versions = versionsByName.get(dependency.name) ?? new Set();
    versions.add(dependency.version);
    versionsByName.set(dependency.name, versions);
  }

  for (const [name, expected] of policy.reviewedPackages) {
    const expectedVersions = new Set(
      Array.isArray(expected) ? expected : [expected],
    );
    const actualVersions = versionsByName.get(name) ?? new Set();
    const versionsMatch =
      actualVersions.size === expectedVersions.size &&
      [...expectedVersions].every((version) => actualVersions.has(version));
    if (!versionsMatch) {
      throw new Error(
        `${name} requires a new dependency applicability review; expected=${[...expectedVersions].join(",")}, actual=${[...actualVersions].join(",") || "missing"}`,
      );
    }
  }
}

function auditRustLockfile(lockfile) {
  const args = ["audit", "--json", "--file", lockfile];
  const advisoryDb = process.env.FN_KNOCK_RUSTSEC_ADVISORY_DB?.trim();
  if (advisoryDb) {
    args.push("--db", advisoryDb, "--no-fetch");
  }
  const report = runJson("cargo", args);
  const findings = rustFindings(report);
  const policy = rustAllowlist.get(lockfile) ?? {
    vulnerabilities: new Set(),
    warnings: new Set(),
  };

  assertExactFindings(
    findings.vulnerabilities,
    policy.vulnerabilities,
    `${lockfile} vulnerabilities`,
  );
  assertExactFindings(
    findings.warnings,
    policy.warnings,
    `${lockfile} warnings`,
  );
  assertReviewedRustPackages(policy);

  if (policy.rationale) {
    console.log(`[security] ${lockfile}: accepted narrow exception`);
    console.log(`[security] rationale: ${policy.rationale}`);
  } else {
    console.log(`[security] ${lockfile}: audit passed`);
  }
}

try {
  auditNodeDependencies();
  auditRustLockfile("apps/server-admin-rs/Cargo.lock");
  auditRustLockfile("apps/fn-knock-desktop/native/Cargo.lock");
} catch (error) {
  console.error(`[security] ${error.message}`);
  process.exitCode = 1;
}
