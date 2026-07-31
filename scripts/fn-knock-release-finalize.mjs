#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readdir, readFile, stat, unlink, writeFile } from "node:fs/promises";
import path from "node:path";

function fail(message) {
  throw new Error(`[release-finalize] ${message}`);
}

async function sha256(file) {
  const hash = createHash("sha256");
  hash.update(await readFile(file));
  return hash.digest("hex");
}

async function listFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      fail(`release assets must be flat; found directory ${entry.name}`);
    }
    if (entry.isFile()) files.push(entry.name);
  }
  return files.sort();
}

function requireNames(files, names) {
  for (const name of names) {
    if (!files.includes(name)) fail(`missing required release asset: ${name}`);
  }
}

function requireCount(files, expression, expected, label) {
  const matching = files.filter((name) => expression.test(name));
  if (matching.length !== expected) {
    fail(
      `${label}: expected ${expected}, found ${matching.length}: ${matching.join(", ")}`,
    );
  }
}

function isPerArtifactMetadata(name) {
  return name.endsWith(".sha256") || name.endsWith(".json");
}

function classify(name) {
  if (name.endsWith(".fpk"))
    return {
      platform: "fnos",
      architecture: name.includes("arm64") ? "arm64" : "amd64",
    };
  const synologyMatch = name.match(/^fn-knock-synology-(x86_64|armv8|armv7)-/);
  if (synologyMatch)
    return { platform: "synology", architecture: synologyMatch[1] };
  if (name.includes("windows-x86_64"))
    return { platform: "windows", architecture: "x86_64" };
  if (name.startsWith("app-meta-"))
    return { platform: "openwrt", architecture: "all" };
  const openwrtMatch = name.match(
    /^fn-knock_.+?-(?:1|r1)_(aarch64_cortex-a53|aarch64_generic|arm_cortex-a5_vfpv4|arm_cortex-a7_neon-vfpv4|x86_64)\.(?:ipk|apk)$/,
  );
  if (openwrtMatch) {
    return { platform: "openwrt", architecture: openwrtMatch[1] };
  }
  const linuxMatch = name.match(/linux-[^-]+-(amd64|arm64|arm)\.tar\.gz$/);
  if (linuxMatch) {
    return {
      platform: "linux",
      architecture: linuxMatch[1] === "arm" ? "armv7" : linuxMatch[1],
    };
  }
  return { platform: "metadata", architecture: "all" };
}

async function main() {
  const directory = path.resolve(
    process.argv[2] ?? process.env.FN_KNOCK_RELEASE_ASSETS_DIR ?? "",
  );
  const version = process.env.FN_KNOCK_VERSION;
  const tag = process.env.FN_KNOCK_RELEASE_TAG;
  const sourceCommit = process.env.FN_KNOCK_SOURCE_COMMIT;
  const gatewayCommit = process.env.FN_KNOCK_GO_SOURCE_COMMIT;
  const controlApiVersion = Number(
    process.env.FN_KNOCK_CONTROL_API_VERSION ?? "",
  );
  const dockerImage = process.env.FN_KNOCK_DOCKER_IMAGE ?? "kcilnk/fn-knock";
  const dockerDigest = process.env.FN_KNOCK_DOCKER_DIGEST ?? "";
  const requireDocker = process.env.FN_KNOCK_REQUIRE_DOCKER === "1";

  if (!directory || !version || !tag || !sourceCommit || !gatewayCommit) {
    fail(
      "assets directory, version, tag, source commit, and gateway commit are required",
    );
  }
  if (tag !== `v${version}`)
    fail(`tag ${tag} does not match version ${version}`);
  if (!Number.isSafeInteger(controlApiVersion) || controlApiVersion <= 0) {
    fail(`control API version is invalid: ${controlApiVersion}`);
  }
  for (const [label, value] of [
    ["source", sourceCommit],
    ["gateway", gatewayCommit],
  ]) {
    if (!/^[0-9a-f]{40}$/i.test(value))
      fail(`${label} commit is invalid: ${value}`);
  }
  if (requireDocker && !/^sha256:[0-9a-f]{64}$/i.test(dockerDigest)) {
    fail(
      `Docker digest is required for a published release: ${dockerDigest || "<empty>"}`,
    );
  }

  const ignored = new Set(["SHA256SUMS", "release-manifest.json"]);
  const inventory = (await listFiles(directory)).filter(
    (name) => !ignored.has(name),
  );
  const excludedFiles = inventory.filter(isPerArtifactMetadata);
  const files = inventory.filter((name) => !isPerArtifactMetadata(name));
  if (files.length !== 21) {
    fail(
      `public release inventory must contain exactly 21 deliverables before metadata; found ${files.length}`,
    );
  }
  requireNames(files, [
    `fn-knock-${version}-fnos-amd64.fpk`,
    `fn-knock-${version}-fnos-arm64.fpk`,
    `fn-knock-linux-${version}-amd64.tar.gz`,
    `fn-knock-linux-${version}-arm64.tar.gz`,
    `fn-knock-linux-${version}-arm.tar.gz`,
    `fn-knock-${version}-windows-x86_64-unsigned-setup.exe`,
    `app-meta-fn-knock_${version}-r1_all.ipk`,
    `app-meta-fn-knock-${version}-r1.apk`,
  ]);
  for (const profile of [
    "aarch64_cortex-a53",
    "aarch64_generic",
    "arm_cortex-a7_neon-vfpv4",
    "arm_cortex-a5_vfpv4",
    "x86_64",
  ]) {
    requireNames(files, [
      `fn-knock_${version}-1_${profile}.ipk`,
      `fn-knock_${version}-r1_${profile}.apk`,
    ]);
  }
  requireCount(
    files,
    /^fn-knock_.+\.(ipk|apk)$/,
    10,
    "OpenWrt architecture packages",
  );
  requireCount(
    files,
    /^app-meta-fn-knock.*\.(ipk|apk)$/,
    2,
    "OpenWrt metadata packages",
  );
  for (const architecture of ["x86_64", "armv8", "armv7"]) {
    requireCount(
      files,
      new RegExp(
        `^fn-knock-synology-${architecture}-${version.replaceAll(".", "\\.")}-\\d+\\.spk$`,
      ),
      1,
      `Synology ${architecture} package`,
    );
  }

  const artifacts = [];
  for (const name of files) {
    const fullPath = path.join(directory, name);
    const info = await stat(fullPath);
    const identity = classify(name);
    artifacts.push({
      name,
      ...identity,
      size: info.size,
      sha256: await sha256(fullPath),
    });
  }

  const manifest = {
    schema_version: 1,
    version,
    tag,
    source_commit: sourceCommit.toLowerCase(),
    gateway_commit: gatewayCommit.toLowerCase(),
    control_api_version: controlApiVersion,
    built_at: new Date().toISOString(),
    artifacts,
    metadata_files: ["release-manifest.json", "SHA256SUMS"],
    docker: {
      published: Boolean(dockerDigest),
      image: dockerImage,
      tag: version,
      reference: `${dockerImage}:${version}`,
      digest: dockerDigest || null,
      platforms: ["linux/amd64", "linux/arm64", "linux/arm/v7"],
    },
  };
  const manifestPath = path.join(directory, "release-manifest.json");
  await writeFile(
    manifestPath,
    `${JSON.stringify(manifest, null, 2)}\n`,
    "utf8",
  );

  const checksumNames = [...files, "release-manifest.json"].sort();
  const checksumLines = [];
  for (const name of checksumNames) {
    checksumLines.push(`${await sha256(path.join(directory, name))}  ${name}`);
  }
  await writeFile(
    path.join(directory, "SHA256SUMS"),
    `${checksumLines.join("\n")}\n`,
    "utf8",
  );
  await Promise.all(
    excludedFiles.map((name) => unlink(path.join(directory, name))),
  );
  console.log(
    `[release-finalize] finalized ${files.length} public deliverables and removed ${excludedFiles.length} per-artifact metadata files in ${directory}`,
  );
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
