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
      vulnerabilities: new Set(["RUSTSEC-2023-0071:rsa:0.9.10"]),
      warnings: new Set(["yanked:spin:0.9.8"]),
      manifest: "apps/server-admin-rs/Cargo.toml",
      reviewedPackages: new Map([
        ["crypto-glue", "0.1.15"],
        ["rsa", "0.9.10"],
        ["webauthn-attestation-ca", "0.6.1-dev"],
        ["webauthn-rs", "0.6.1-dev"],
        ["webauthn-rs-core", "0.6.1-dev"],
      ]),
      rationale:
        "rsa is transitive through webauthn-rs and is used only for public-key signature verification; the advisory requires observable private-key operations. No patched rsa release exists. The exact yanked spin version is also transitive through rsa/num-bigint-dig and has no RustSec vulnerability.",
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

  for (const [name, expectedVersion] of policy.reviewedPackages) {
    const actualVersions = versionsByName.get(name) ?? new Set();
    if (actualVersions.size !== 1 || !actualVersions.has(expectedVersion)) {
      throw new Error(
        `${name} requires a new RSA applicability review; expected=${expectedVersion}, actual=${[...actualVersions].join(",") || "missing"}`,
      );
    }
  }
}

function auditRustLockfile(lockfile) {
  const report = runJson("cargo", ["audit", "--json", "--file", lockfile]);
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
