#!/usr/bin/env node

import { readFile, rename, writeFile } from "node:fs/promises";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";

const REPOSITORY = "kci-lnk/fn-knock-turborepo";
const HOMEPAGE = `https://github.com/${REPOSITORY}`;
const ROOT = new URL("../", import.meta.url);

function manifestValue(manifest, key) {
  const value = manifest.match(new RegExp(`^${key}=(.*)$`, "m"))?.[1]?.trim();
  if (!value) throw new Error(`manifest is missing ${key}`);
  return value.replace(/^"""|"""$/g, "");
}

export function buildMooIndex(release, manifest) {
  const version = release.tag_name?.match(/^v(\d+\.\d+\.\d+)$/)?.[1];
  if (!version || release.draft !== false || release.prerelease !== false) {
    throw new Error("Moo index requires a published stable vX.Y.Z release");
  }
  if (
    typeof release.published_at !== "string" ||
    !Number.isFinite(Date.parse(release.published_at))
  ) {
    throw new Error("release is missing a valid publication date");
  }

  const packages = {};
  for (const [architecture, platform] of [
    ["amd64", "x86"],
    ["arm64", "arm"],
  ]) {
    const name = `fn-knock-${version}-fnos-${architecture}.fpk`;
    const matches =
      release.assets?.filter((asset) => asset.name === name) ?? [];
    if (matches.length !== 1)
      throw new Error(`expected one release asset: ${name}`);
    const asset = matches[0];
    const sha256 = asset.digest?.match(/^sha256:([0-9a-f]{64})$/i)?.[1];
    const downloadUrl = `${HOMEPAGE}/releases/download/${release.tag_name}/${name}`;
    if (
      !sha256 ||
      !Number.isSafeInteger(asset.size) ||
      asset.size <= 0 ||
      asset.browser_download_url !== downloadUrl
    ) {
      throw new Error(`invalid SHA-256, size, or download URL for ${name}`);
    }
    packages[platform] = {
      download_url: downloadUrl,
      sha256: sha256.toLowerCase(),
      size: asset.size,
    };
  }

  const appname = manifestValue(manifest, "appname");
  if (appname !== "fn-knock")
    throw new Error("expected the full fn-knock manifest");
  const rawBase = `https://raw.githubusercontent.com/${REPOSITORY}/${release.tag_name}`;
  return {
    schema_version: "moo",
    source_info: {
      name: "fn-knock · 敲门",
      author: manifestValue(manifest, "maintainer"),
      homepage: HOMEPAGE,
      description: "fn-knock 官方 fnOS 原生 FPK 应用源。",
      updated_at: release.published_at,
    },
    apps: {
      [appname]: {
        display_name: manifestValue(manifest, "display_name"),
        app_type: "fpk",
        platform: ["x86", "arm"],
        categories: ["网络工具", "系统工具"],
        desc: manifestValue(manifest, "desc"),
        author: manifestValue(manifest, "maintainer"),
        author_url: "https://github.com/kci-lnk",
        distributor: manifestValue(manifest, "distributor"),
        homepage: "https://www.fnknock.cn/",
        bug_report_url: `${HOMEPAGE}/issues`,
        license: "MIT",
        install_type: manifestValue(manifest, "install_type"),
        service_port: Number(manifestValue(manifest, "service_port")),
        icon_url: `${rawBase}/apps/fn-knock/ICON_256.PNG`,
        readme_url: `${rawBase}/README.md`,
        updated_at: release.published_at,
        releases: {
          [version]: {
            changelog: release.body ?? "",
            updated_at: release.published_at,
            packages,
          },
        },
      },
    },
  };
}

async function githubRequest(endpoint, { method = "GET", body } = {}) {
  const response = await fetch(
    `https://api.github.com/repos/${REPOSITORY}/${endpoint}`,
    {
      method,
      headers: {
        Accept: "application/vnd.github+json",
        "X-GitHub-Api-Version": "2026-03-10",
        "User-Agent": "fn-knock-moo-index",
        ...(body ? { "Content-Type": "application/json" } : {}),
        ...(process.env.GITHUB_TOKEN
          ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` }
          : {}),
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
      signal: AbortSignal.timeout(30_000),
    },
  );
  return { status: response.status, body: await response.json() };
}

export async function readRelease(tag, request = githubRequest) {
  if (tag && !/^v\d+\.\d+\.\d+$/.test(tag))
    throw new Error("expected a vX.Y.Z release tag");
  const response = await request(
    tag ? `releases/tags/${encodeURIComponent(tag)}` : "releases/latest",
  );
  if (response.status !== 200)
    throw new Error(
      `GitHub release request failed: HTTP ${response.status}: ${response.body?.message ?? "unknown error"}`,
    );
  if (tag && response.body.tag_name !== tag)
    throw new Error(`GitHub returned a different release than ${tag}`);
  return response.body;
}

function compareVersions(left, right) {
  const a = left.split(".").map(BigInt);
  const b = right.split(".").map(BigInt);
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] > b[i] ? 1 : -1;
  }
  return 0;
}

export async function publishMooIndex(index, branch, request = githubRequest) {
  if (!branch)
    throw new Error("a target branch is required to publish the Moo index");
  const versions = Object.keys(index.apps["fn-knock"].releases);
  if (versions.length !== 1 || !/^\d+\.\d+\.\d+$/.test(versions[0])) {
    throw new Error("expected one stable version in the generated Moo index");
  }
  const version = versions[0];
  const content = `${JSON.stringify(index, null, 2)}\n`;
  const endpoint = "contents/moo.json";
  for (let attempt = 0; attempt < 3; attempt++) {
    const current = await request(
      `${endpoint}?ref=${encodeURIComponent(branch)}`,
    );
    let sha;
    if (current.status === 200) {
      if (
        current.body.type !== "file" ||
        current.body.encoding !== "base64" ||
        !current.body.sha
      ) {
        throw new Error("remote moo.json is not an editable file");
      }
      sha = current.body.sha;
      const previousContent = Buffer.from(
        current.body.content,
        "base64",
      ).toString("utf8");
      if (previousContent === content) return { status: "unchanged", version };
      const previous = JSON.parse(previousContent);
      const previousVersions = Object.keys(
        previous.apps?.["fn-knock"]?.releases ?? {},
      );
      if (
        previous.schema_version !== "moo" ||
        previousVersions.length === 0 ||
        previousVersions.some((v) => !/^\d+\.\d+\.\d+$/.test(v))
      ) {
        throw new Error(
          "remote moo.json has an invalid stable version index; refusing to overwrite it",
        );
      }
      if (previousVersions.some((v) => compareVersions(v, version) > 0)) {
        return { status: "skipped-newer-version", version };
      }
    } else if (current.status !== 404) {
      throw new Error(
        `GitHub moo.json read failed: HTTP ${current.status}: ${current.body?.message ?? "unknown error"}`,
      );
    }

    const updated = await request(endpoint, {
      method: "PUT",
      body: {
        message: `chore: update Moo index for v${version}`,
        content: Buffer.from(content).toString("base64"),
        branch,
        ...(sha ? { sha } : {}),
      },
    });
    if (updated.status === 200 || updated.status === 201) {
      return {
        status: "updated",
        version,
        commit: updated.body.commit?.html_url,
      };
    }
    // A concurrent commit can invalidate the file SHA. Read again before retrying.
    if (updated.status !== 409) {
      throw new Error(
        `GitHub moo.json update failed: HTTP ${updated.status}: ${updated.body.message ?? "unknown error"}`,
      );
    }
  }
  throw new Error(
    "GitHub moo.json update failed after three concurrent-write conflicts",
  );
}

async function main() {
  const { values, positionals } = parseArgs({
    options: {
      tag: { type: "string" },
      publish: { type: "boolean" },
      branch: { type: "string" },
    },
    allowPositionals: true,
  });
  if (positionals.length > 1 || (positionals.length && values.tag)) {
    throw new Error(
      "provide either a Release API response file or --tag vX.Y.Z",
    );
  }
  if (
    values.publish &&
    (!values.tag ||
      !values.branch ||
      !process.env.GITHUB_TOKEN ||
      positionals.length)
  ) {
    throw new Error("publishing requires --tag, --branch, and GITHUB_TOKEN");
  }
  const releaseFile = positionals[0];
  let release;
  if (releaseFile) {
    release = JSON.parse(await readFile(releaseFile, "utf8"));
  } else {
    release = await readRelease(values.tag);
  }
  const manifest = await readFile(
    new URL("apps/fn-knock/manifest", ROOT),
    "utf8",
  );
  const index = buildMooIndex(release, manifest);
  if (values.publish) {
    const result = await publishMooIndex(index, values.branch);
    console.log(
      `[moo-index] ${result.status}: ${release.tag_name} on ${values.branch}${result.commit ? ` (${result.commit})` : ""}`,
    );
    return;
  }
  const output = new URL("moo.json", ROOT);
  const temporary = new URL(`moo.json.${process.pid}.tmp`, ROOT);
  await writeFile(temporary, `${JSON.stringify(index, null, 2)}\n`);
  await rename(temporary, output);
  console.log(
    `[moo-index] updated ${fileURLToPath(output)} from ${release.tag_name}`,
  );
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    console.error(`[moo-index] ${error.message}`);
    process.exitCode = 1;
  });
}
