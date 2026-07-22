#!/usr/bin/env node

import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdir, readFile, readdir, stat, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { basename, extname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { isDeepStrictEqual } from "node:util";

const require = createRequire(import.meta.url);

const KNOWN_PACKAGE_TYPES = [
  "fpk",
  "ipk",
  "apk",
  "synology",
  "linux",
  "windows",
];
const POINTER_CACHE_CONTROL = "no-cache";
const LATEST_KEY = "latest.json";
const WINDOWS_STABLE_KEY = "windows/stable/latest.json";
const SHA256_PATTERN = /^[a-f0-9]{64}$/;
const VERSION_PATTERN = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const HOSTNAME_PATTERN =
  /^(?=.{1,253}$)(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?$/i;

const fail = (message) => {
  throw new Error(`[cos-publish] ${message}`);
};

const isRecord = (value) =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const normalizeBaseUrl = (value) => value.trim().replace(/\/+$/, "");

export const normalizeCosAccelerateDomain = (value, bucket) => {
  const domain = String(value ?? "")
    .trim()
    .replace(/\.$/, "")
    .toLowerCase();
  const normalizedBucket = String(bucket ?? "")
    .trim()
    .toLowerCase();
  if (!HOSTNAME_PATTERN.test(domain)) {
    fail("COS_ACC must be a hostname without a protocol, path, or port");
  }
  if (!normalizedBucket || !domain.startsWith(`${normalizedBucket}.`)) {
    fail("COS_ACC must identify the configured COS_BUCKET");
  }
  return domain;
};

const parseVersion = (value) => {
  const match = String(value ?? "")
    .trim()
    .match(VERSION_PATTERN);
  return match ? match.slice(1).map((part) => BigInt(part)) : null;
};

export const compareVersions = (leftValue, rightValue) => {
  const left = parseVersion(leftValue);
  const right = parseVersion(rightValue);
  if (!left || !right) {
    fail(`versions must use MAJOR.MINOR.PATCH: ${leftValue}, ${rightValue}`);
  }
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] > right[index]) return 1;
    if (left[index] < right[index]) return -1;
  }
  return 0;
};

const sha256Buffer = (body) => createHash("sha256").update(body).digest("hex");

const sha256File = async (filePath) => {
  const hash = createHash("sha256");
  const stream = createReadStream(filePath);
  for await (const chunk of stream) hash.update(chunk);
  return hash.digest("hex");
};

const jsonBody = (value) =>
  Buffer.from(`${JSON.stringify(value, null, 2)}\n`, "utf8");

const bodyObject = ({ key, body, contentType, cacheControl }) => ({
  key,
  body,
  size: body.byteLength,
  sha256: sha256Buffer(body),
  contentType,
  ...(cacheControl ? { cacheControl } : {}),
});

const fileObject = ({
  key,
  path,
  size,
  sha256,
  contentType,
  cacheControl,
}) => ({
  key,
  path,
  size,
  sha256,
  contentType,
  ...(cacheControl ? { cacheControl } : {}),
});

const publicUrl = (baseUrl, key) =>
  `${baseUrl}/${key
    .split("/")
    .map((segment) => encodeURIComponent(segment))
    .join("/")}`;

const artifactContentType = (name) => {
  if (name.endsWith(".tar.gz")) return "application/gzip";
  if (name.endsWith(".exe")) {
    return "application/vnd.microsoft.portable-executable";
  }
  if (name.endsWith(".json")) return "application/json; charset=utf-8";
  if (name.endsWith(".sha256")) return "text/plain; charset=utf-8";
  return "application/octet-stream";
};

const packageIdentity = (artifact) => {
  const name = artifact.name;
  const architecture = artifact.architecture;

  if (artifact.platform === "fnos" && name.endsWith(".fpk")) {
    return {
      type: "fpk",
      arch: architecture,
      key:
        architecture === "arm64"
          ? `files/${artifact.version}/arm64/${name}`
          : `files/${artifact.version}/${name}`,
    };
  }
  if (artifact.platform === "openwrt" && name.endsWith(".ipk")) {
    return {
      type: "ipk",
      arch: architecture,
      key: `files/${artifact.version}/openwrt/${architecture}/${name}`,
    };
  }
  if (artifact.platform === "openwrt" && name.endsWith(".apk")) {
    return {
      type: "apk",
      arch: architecture,
      key: `files/${artifact.version}/openwrt/apk/${architecture}/${name}`,
    };
  }
  if (artifact.platform === "synology" && name.endsWith(".spk")) {
    return {
      type: "synology",
      arch: architecture,
      key: `files/${artifact.version}/synology/${architecture}/${name}`,
    };
  }
  if (artifact.platform === "linux" && name.endsWith(".tar.gz")) {
    const arch = architecture === "armv7" ? "arm" : architecture;
    return {
      type: "linux",
      arch,
      key: `files/${artifact.version}/linux/${arch}/${name}`,
    };
  }
  if (artifact.platform === "windows" && name.endsWith(".exe")) {
    return {
      type: "windows",
      arch: architecture,
      key: `files/${artifact.version}/windows/${architecture}/${name}`,
    };
  }

  fail(`unsupported release artifact: ${name}`);
};

export const composeReleaseNotes = (currentNotes, currentVersion, releases) => {
  if (!parseVersion(currentVersion)) {
    fail(`invalid current release version: ${currentVersion}`);
  }
  const current = currentNotes.trim();
  if (!current) fail("current release notes must not be empty");

  const unique = new Map();
  for (const release of Array.isArray(releases) ? releases : []) {
    if (!isRecord(release) || release.draft || release.prerelease) continue;
    const tag = typeof release.tag_name === "string" ? release.tag_name : "";
    const version = tag.replace(/^v/, "");
    if (
      !parseVersion(version) ||
      compareVersions(version, currentVersion) >= 0
    ) {
      continue;
    }
    const body = typeof release.body === "string" ? release.body.trim() : "";
    if (body && !unique.has(version)) unique.set(version, body);
  }

  const history = [...unique]
    .sort(([left], [right]) => compareVersions(right, left))
    .slice(0, 5)
    .map(([, body]) => body);
  return [current, ...history].join("\n\n---\n\n");
};

const assertKeys = (record, expected, label) => {
  const actual = Object.keys(record ?? {}).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(
      `${label} architectures must be ${wanted.join(", ")}; got ${actual.join(", ")}`,
    );
  }
};

const readJsonObject = async (filePath, label = basename(filePath)) => {
  let parsed;
  try {
    parsed = JSON.parse(await readFile(filePath, "utf8"));
  } catch (error) {
    fail(
      `unable to read ${label}: ${error instanceof Error ? error.message : error}`,
    );
  }
  if (!isRecord(parsed)) fail(`${label} must contain a JSON object`);
  return parsed;
};

const validateManifestArtifact = (artifact, version) => {
  if (!isRecord(artifact))
    fail("release manifest contains an invalid artifact");
  const name = typeof artifact.name === "string" ? artifact.name.trim() : "";
  if (!name || basename(name) !== name) fail(`invalid artifact name: ${name}`);
  const size = artifact.size;
  const sha256 =
    typeof artifact.sha256 === "string" ? artifact.sha256.toLowerCase() : "";
  if (!Number.isSafeInteger(size) || size < 0 || !SHA256_PATTERN.test(sha256)) {
    fail(`invalid size or SHA-256 in release manifest for ${name}`);
  }
  const platform =
    typeof artifact.platform === "string" ? artifact.platform.trim() : "";
  const architecture =
    typeof artifact.architecture === "string"
      ? artifact.architecture.trim()
      : "";
  if (!platform || !architecture) fail(`missing identity for ${name}`);
  return { name, size, sha256, platform, architecture, version };
};

const readWindowsMetadata = async ({
  directory,
  version,
  setupArtifact,
  setupEntry,
  publicBaseUrl,
  releaseNotes,
}) => {
  const entries = await readdir(directory, { withFileTypes: true });
  const names = entries
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name);
  const sidecarName = `${setupArtifact.name}.sha256`;
  const releaseName = names.find((name) =>
    name.endsWith("-windows-x86_64-unsigned-release.json"),
  );
  const updaterName = names.find((name) =>
    name.endsWith("-windows-x86_64-unsigned-updater.json"),
  );
  if (!names.includes(sidecarName) || !releaseName || !updaterName) {
    fail(
      "Windows release archive is missing SHA256, release, or updater metadata",
    );
  }

  const sidecarPath = join(directory, sidecarName);
  const sidecar = (await readFile(sidecarPath, "utf8")).trim();
  const sidecarMatch = sidecar.match(/^([a-fA-F0-9]{64})\s+[*]?(.+)$/);
  if (
    !sidecarMatch ||
    sidecarMatch[1].toLowerCase() !== setupArtifact.sha256 ||
    sidecarMatch[2].trim() !== setupArtifact.name
  ) {
    fail("Windows SHA256 sidecar does not match the release installer");
  }

  const releasePath = join(directory, releaseName);
  const updaterPath = join(directory, updaterName);
  const releaseDocument = await readJsonObject(releasePath, releaseName);
  const updaterSource = await readJsonObject(updaterPath, updaterName);
  if (
    releaseDocument.version !== version ||
    releaseDocument.runtime_target !== "windows" ||
    releaseDocument.architecture !== "x86_64" ||
    updaterSource.version !== version
  ) {
    fail("Windows metadata does not match the target release");
  }

  const prefix = `files/${version}/windows/x86_64`;
  const sha256Key = `${prefix}/${sidecarName}`;
  const releaseKey = `${prefix}/release.json`;
  const updaterKey = `${prefix}/updater.json`;
  const packageMetadata = {
    ...setupEntry,
    version,
    sha256_url: publicUrl(publicBaseUrl, sha256Key),
    release_url: publicUrl(publicBaseUrl, releaseKey),
    updater_url: publicUrl(publicBaseUrl, updaterKey),
    release_notes: releaseNotes,
    update_available: true,
    force_update: false,
  };
  const publishedReleaseDocument = {
    ...releaseDocument,
    version,
    published_at:
      typeof releaseDocument.published_at === "string" &&
      releaseDocument.published_at.trim()
        ? releaseDocument.published_at.trim()
        : typeof updaterSource.pub_date === "string"
          ? updaterSource.pub_date
          : "",
    release_notes: releaseNotes,
    file_name: setupArtifact.name,
    sha256: setupArtifact.sha256,
    size: setupArtifact.size,
    packages: {
      ...(isRecord(releaseDocument.packages) ? releaseDocument.packages : {}),
      windows: {
        x86_64: {
          url: setupEntry.download_url,
          sha256: setupArtifact.sha256,
          size: setupArtifact.size,
        },
      },
    },
  };
  const updaterDocument = {
    version,
    notes: releaseNotes,
    pub_date:
      typeof updaterSource.pub_date === "string" &&
      updaterSource.pub_date.trim()
        ? updaterSource.pub_date.trim()
        : new Date(0).toISOString(),
    platforms: {
      "windows-x86_64": { url: setupEntry.download_url },
    },
    fn_knock: { package: packageMetadata },
  };
  const updaterBody = jsonBody(updaterDocument);
  const sidecarInfo = await stat(sidecarPath);

  return {
    updaterDocument,
    versionObjects: [
      fileObject({
        key: sha256Key,
        path: sidecarPath,
        size: sidecarInfo.size,
        sha256: await sha256File(sidecarPath),
        contentType: "text/plain; charset=utf-8",
        cacheControl: POINTER_CACHE_CONTROL,
      }),
      bodyObject({
        key: releaseKey,
        body: jsonBody(publishedReleaseDocument),
        contentType: "application/json; charset=utf-8",
        cacheControl: POINTER_CACHE_CONTROL,
      }),
      bodyObject({
        key: updaterKey,
        body: updaterBody,
        contentType: "application/json; charset=utf-8",
        cacheControl: POINTER_CACHE_CONTROL,
      }),
    ],
  };
};

export const buildReleasePlan = async ({
  assetsDir,
  windowsMetadataDir,
  installScriptPath,
  releaseNotesPath,
  version,
  publicBaseUrl,
  previousReleases = [],
}) => {
  const normalizedBaseUrl = normalizeBaseUrl(publicBaseUrl);
  if (!normalizedBaseUrl || !/^https?:\/\//.test(normalizedBaseUrl)) {
    fail("COS_PUBLICBASICURL must be an absolute HTTP(S) URL");
  }
  if (!parseVersion(version)) fail(`invalid target version: ${version}`);

  const manifestPath = join(assetsDir, "release-manifest.json");
  const manifest = await readJsonObject(manifestPath, "release-manifest.json");
  if (
    manifest.schema_version !== 1 ||
    manifest.version !== version ||
    manifest.tag !== `v${version}` ||
    !Array.isArray(manifest.artifacts) ||
    manifest.artifacts.length !== 21
  ) {
    fail(
      "release-manifest.json does not describe the expected 21-file release",
    );
  }

  const currentNotes = await readFile(releaseNotesPath, "utf8");
  const releaseNotes = composeReleaseNotes(
    currentNotes,
    version,
    previousReleases,
  );
  const packages = Object.fromEntries(
    KNOWN_PACKAGE_TYPES.map((type) => [type, {}]),
  );
  const versionObjects = [];
  const artifactNames = new Set();
  let windowsArtifact = null;
  let windowsEntry = null;

  for (const value of manifest.artifacts) {
    const artifact = validateManifestArtifact(value, version);
    if (artifactNames.has(artifact.name))
      fail(`duplicate artifact: ${artifact.name}`);
    artifactNames.add(artifact.name);
    const path = join(assetsDir, artifact.name);
    const info = await stat(path).catch(() => null);
    if (!info?.isFile()) fail(`release artifact is missing: ${artifact.name}`);
    if (info.size !== artifact.size) fail(`size mismatch for ${artifact.name}`);
    if ((await sha256File(path)) !== artifact.sha256) {
      fail(`SHA-256 mismatch for ${artifact.name}`);
    }

    const identity = packageIdentity(artifact);
    if (packages[identity.type][identity.arch]) {
      fail(`duplicate ${identity.type} architecture: ${identity.arch}`);
    }
    const entry = {
      type: identity.type,
      ...(identity.type === "linux" || identity.type === "synology"
        ? { version }
        : {}),
      arch: identity.arch,
      file_name: artifact.name,
      object_key: identity.key,
      download_url: publicUrl(normalizedBaseUrl, identity.key),
      sha256: artifact.sha256,
      size: artifact.size,
    };
    packages[identity.type][identity.arch] = entry;
    versionObjects.push(
      fileObject({
        key: identity.key,
        path,
        size: artifact.size,
        sha256: artifact.sha256,
        contentType: artifactContentType(artifact.name),
        ...(["linux", "windows"].includes(identity.type)
          ? { cacheControl: POINTER_CACHE_CONTROL }
          : {}),
      }),
    );
    if (identity.type === "windows") {
      windowsArtifact = artifact;
      windowsEntry = entry;
    }
  }

  assertKeys(packages.fpk, ["amd64", "arm64"], "FPK");
  assertKeys(
    packages.ipk,
    [
      "all",
      "aarch64_cortex-a53",
      "aarch64_generic",
      "arm_cortex-a5_vfpv4",
      "arm_cortex-a7_neon-vfpv4",
      "x86_64",
    ],
    "IPK",
  );
  assertKeys(
    packages.apk,
    [
      "all",
      "aarch64_cortex-a53",
      "aarch64_generic",
      "arm_cortex-a5_vfpv4",
      "arm_cortex-a7_neon-vfpv4",
      "x86_64",
    ],
    "APK",
  );
  assertKeys(packages.synology, ["armv7", "armv8", "x86_64"], "Synology");
  assertKeys(packages.linux, ["amd64", "arm", "arm64"], "Linux");
  assertKeys(packages.windows, ["x86_64"], "Windows");

  if (
    !windowsArtifact ||
    !windowsEntry ||
    !windowsArtifact.name.includes("-unsigned-")
  ) {
    fail("the release must contain one unsigned Windows x86_64 installer");
  }
  const windows = await readWindowsMetadata({
    directory: windowsMetadataDir,
    version,
    setupArtifact: windowsArtifact,
    setupEntry: windowsEntry,
    publicBaseUrl: normalizedBaseUrl,
    releaseNotes,
  });
  versionObjects.push(...windows.versionObjects);

  const installInfo = await stat(installScriptPath).catch(() => null);
  if (!installInfo?.isFile())
    fail(`Linux install script is missing: ${installScriptPath}`);
  const mutableObjects = [
    fileObject({
      key: "install.sh",
      path: installScriptPath,
      size: installInfo.size,
      sha256: await sha256File(installScriptPath),
      contentType: "text/x-shellscript; charset=utf-8",
      cacheControl: POINTER_CACHE_CONTROL,
    }),
  ];
  for (const arch of ["amd64", "arm64", "arm"]) {
    const entry = packages.linux[arch];
    const body = Buffer.from(
      [
        `VERSION=${version}`,
        `URL=${entry.download_url}`,
        `SHA256=${entry.sha256}`,
        `SIZE=${entry.size}`,
        "",
      ].join("\n"),
      "utf8",
    );
    mutableObjects.push(
      bodyObject({
        key: `linux/latest/${arch}.env`,
        body,
        contentType: "text/plain; charset=utf-8",
        cacheControl: POINTER_CACHE_CONTROL,
      }),
    );
  }
  mutableObjects.push(
    bodyObject({
      key: WINDOWS_STABLE_KEY,
      body: jsonBody(windows.updaterDocument),
      contentType: "application/json; charset=utf-8",
      cacheControl: POINTER_CACHE_CONTROL,
    }),
  );

  const latestCore = {
    version,
    update_available: true,
    force_update: false,
    download_url: packages.fpk.amd64.download_url,
    sha256: packages.fpk.amd64.sha256,
    download_url_arm64: packages.fpk.arm64.download_url,
    sha256_arm64: packages.fpk.arm64.sha256,
    release_notes: releaseNotes,
    packages,
  };

  return {
    version,
    manifest,
    latestCore,
    versionObjects,
    mutableObjects,
  };
};

export const mergeLatestDocument = (current, latestCore) => {
  const source = isRecord(current) ? current : {};
  const currentPackages = isRecord(source.packages) ? source.packages : {};
  const unknownPackages = Object.fromEntries(
    Object.entries(currentPackages).filter(
      ([type]) => !KNOWN_PACKAGE_TYPES.includes(type),
    ),
  );
  return {
    ...source,
    ...latestCore,
    packages: { ...unknownPackages, ...latestCore.packages },
  };
};

const releaseVersionFromBody = (body) => {
  if (!body) return null;
  try {
    const parsed = JSON.parse(body.toString("utf8"));
    return isRecord(parsed) && typeof parsed.version === "string"
      ? parsed.version
      : null;
  } catch {
    fail("current COS latest.json is not valid JSON");
  }
};

const mapWithConcurrency = async (values, concurrency, operation) => {
  let next = 0;
  const workers = Array.from(
    { length: Math.min(concurrency, values.length) },
    async () => {
      while (next < values.length) {
        const index = next;
        next += 1;
        await operation(values[index]);
      }
    },
  );
  await Promise.all(workers);
};

const uploadAndVerify = async (store, object, log) => {
  await store.put(object);
  const head = await store.head(object.key);
  if (!head || head.size !== object.size || head.sha256 !== object.sha256) {
    fail(`COS read-back metadata mismatch for ${object.key}`);
  }
  log(`uploaded ${object.key} (${object.size} bytes, sha256 ${object.sha256})`);
};

const sameEtag = (left, right) => (left ?? null) === (right ?? null);

const assertPointerUnchanged = async (store, key, snapshot) => {
  const current = await store.head(key);
  if (snapshot === null) {
    if (current !== null) {
      fail(`concurrent COS pointer creation detected: ${key}`);
    }
  } else if (!current || !sameEtag(current.etag, snapshot.etag)) {
    fail(`concurrent COS pointer update detected: ${key}`);
  }
};

const assertPointersUnchanged = async (store, snapshots) => {
  for (const [key, snapshot] of snapshots) {
    await assertPointerUnchanged(store, key, snapshot);
  }
};

const descriptorFromSnapshot = (key, snapshot) =>
  bodyObject({
    key,
    body: snapshot.body,
    contentType: snapshot.contentType || "application/octet-stream",
    ...(snapshot.cacheControl ? { cacheControl: snapshot.cacheControl } : {}),
  });

const rollbackPointers = async ({ store, snapshots, attempted, log }) => {
  for (const [key, expectedBody] of [...attempted].reverse()) {
    const current = await store.get(key);
    const snapshot = snapshots.get(key);
    if (
      (snapshot === null && current === null) ||
      (snapshot !== null && current?.body.equals(snapshot.body))
    ) {
      log(`pointer remained unchanged: ${key}`);
      continue;
    }
    if (!current || !current.body.equals(expectedBody)) {
      log(`skip rollback for concurrently changed pointer ${key}`);
      continue;
    }
    if (snapshot === null) {
      await store.delete(key);
      if ((await store.get(key)) !== null) {
        fail(`failed to remove new pointer during rollback: ${key}`);
      }
    } else {
      await store.put(descriptorFromSnapshot(key, snapshot));
      const restored = await store.get(key);
      if (!restored?.body.equals(snapshot.body)) {
        fail(`failed to restore pointer during rollback: ${key}`);
      }
    }
    log(`restored ${key}`);
  }
};

export const verifyLatestDocument = (actual, expected) => {
  return (
    isRecord(actual) &&
    isRecord(expected) &&
    isDeepStrictEqual(actual, expected)
  );
};

const prepareVersionUploads = async ({ plan, store, log }) => {
  const missing = [];
  for (const object of plan.versionObjects) {
    const head = await store.head(object.key);
    if (head === null) {
      missing.push(object);
      continue;
    }
    if (head.size !== object.size) {
      fail(`same-version COS object size mismatch: ${object.key}`);
    }
    let actualSha256 = head.sha256;
    if (!actualSha256) {
      const existing = await store.get(object.key);
      if (!existing || existing.size !== object.size) {
        fail(`same-version COS object read-back mismatch: ${object.key}`);
      }
      actualSha256 = sha256Buffer(existing.body);
    }
    if (actualSha256 !== object.sha256) {
      fail(`same-version COS object SHA-256 mismatch: ${object.key}`);
    }
    log(`reused ${object.key} (${object.size} bytes, sha256 ${object.sha256})`);
  }
  return missing;
};

const verifyCdnLatest = async ({ latestUrl, expected, fetchImpl, wait }) => {
  let lastError;
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try {
      const response = await fetchImpl(latestUrl, {
        cache: "no-store",
        headers: {
          "Cache-Control": "no-cache",
          Pragma: "no-cache",
        },
      });
      if (!response.ok)
        fail(`CDN latest read-back returned HTTP ${response.status}`);
      const actual = await response.json();
      if (!verifyLatestDocument(actual, expected)) {
        fail("CDN latest read-back does not match the published release");
      }
      return;
    } catch (error) {
      lastError = error;
      if (attempt < 4) await wait(2000);
    }
  }
  throw lastError;
};

export const publishRelease = async ({
  plan,
  store,
  cdn,
  latestUrl,
  fetchImpl = fetch,
  wait = (milliseconds) =>
    new Promise((resolveWait) => setTimeout(resolveWait, milliseconds)),
  log = (message) => console.log(`[cos-publish] ${message}`),
}) => {
  const pointerKeys = [
    ...plan.mutableObjects.map((object) => object.key),
    LATEST_KEY,
  ];
  const snapshots = new Map();
  for (const key of pointerKeys) snapshots.set(key, await store.get(key));

  const currentVersion = releaseVersionFromBody(
    snapshots.get(LATEST_KEY)?.body,
  );
  const versionComparison = currentVersion
    ? compareVersions(plan.version, currentVersion)
    : null;
  if (versionComparison !== null && versionComparison < 0) {
    fail(
      `refusing to downgrade latest.json from ${currentVersion} to ${plan.version}`,
    );
  }
  const currentLatest = snapshots.get(LATEST_KEY)
    ? JSON.parse(snapshots.get(LATEST_KEY).body.toString("utf8"))
    : null;
  const latestDocument = mergeLatestDocument(currentLatest, plan.latestCore);
  const latestObject = bodyObject({
    key: LATEST_KEY,
    body: jsonBody(latestDocument),
    contentType: "application/json; charset=utf-8",
    cacheControl: POINTER_CACHE_CONTROL,
  });

  const versionUploads = await prepareVersionUploads({
    plan,
    store,
    log,
  });
  await mapWithConcurrency(versionUploads, 3, (object) =>
    uploadAndVerify(store, object, log),
  );
  await assertPointersUnchanged(store, snapshots);

  const attempted = new Map();
  let purgeAttempted = false;
  try {
    for (const object of [...plan.mutableObjects, latestObject]) {
      const body = object.body ?? (await readFile(object.path));
      await assertPointerUnchanged(
        store,
        object.key,
        snapshots.get(object.key),
      );
      attempted.set(object.key, body);
      await uploadAndVerify(store, object, log);
    }

    for (const [key, expectedBody] of attempted) {
      const current = await store.get(key);
      if (!current?.body.equals(expectedBody)) {
        fail(`COS pointer read-back mismatch for ${key}`);
      }
    }

    purgeAttempted = true;
    const taskId = await cdn.purgeAndWait(latestUrl);
    log(`CDN purge completed for ${latestUrl} (task ${taskId})`);
    await verifyCdnLatest({
      latestUrl,
      expected: latestDocument,
      fetchImpl,
      wait,
    });
    log(`verified CDN latest.json for ${plan.version}`);
    return { latestDocument, taskId };
  } catch (error) {
    try {
      await rollbackPointers({ store, snapshots, attempted, log });
      if (purgeAttempted && attempted.has(LATEST_KEY)) {
        await cdn.purgeAndWait(latestUrl);
        log(`refreshed restored CDN latest.json for ${latestUrl}`);
      }
    } catch (rollbackError) {
      throw new AggregateError(
        [error, rollbackError],
        "COS publish failed and pointer rollback was incomplete",
      );
    }
    throw error;
  }
};

export const fetchPreviousReleases = async ({
  repository,
  token,
  apiUrl = "https://api.github.com",
  fetchImpl = fetch,
}) => {
  if (!repository || !token)
    fail("GITHUB_REPOSITORY and GITHUB_TOKEN are required");
  const response = await fetchImpl(
    `${apiUrl.replace(/\/+$/, "")}/repos/${repository}/releases?per_page=100`,
    {
      headers: {
        Accept: "application/vnd.github+json",
        Authorization: `Bearer ${token}`,
        "X-GitHub-Api-Version": "2022-11-28",
      },
    },
  );
  if (!response.ok)
    fail(`GitHub releases request returned HTTP ${response.status}`);
  const releases = await response.json();
  if (!Array.isArray(releases))
    fail("GitHub releases response must be an array");
  return releases;
};

export const createTencentAdapters = ({
  secretId,
  secretKey,
  bucket,
  region,
  accelerateDomain,
  wait = (milliseconds) =>
    new Promise((resolveWait) => setTimeout(resolveWait, milliseconds)),
}) => {
  const COS = require("cos-nodejs-sdk-v5");
  const { cdn: cdnSdk } = require("tencentcloud-sdk-nodejs-cdn");
  const originCos = new COS({ SecretId: secretId, SecretKey: secretKey });
  const acceleratedCos = new COS({
    SecretId: secretId,
    SecretKey: secretKey,
    Domain: normalizeCosAccelerateDomain(accelerateDomain, bucket),
    Protocol: "https:",
    UseAccelerate: true,
  });
  const objectParams = (key) => ({ Bucket: bucket, Region: region, Key: key });
  const missing = (error) =>
    error?.statusCode === 404 || error?.code === "NoSuchKey";
  const headersOf = (data) =>
    Object.fromEntries(
      Object.entries(data?.headers ?? {}).map(([key, value]) => [
        key.toLowerCase(),
        String(value),
      ]),
    );

  const store = {
    async get(key) {
      try {
        const data = await acceleratedCos.getObject(objectParams(key));
        const headers = headersOf(data);
        return {
          body: Buffer.from(data.Body),
          etag: data.ETag ?? headers.etag ?? null,
          size: Number(headers["content-length"] ?? data.Body.length),
          sha256: headers["x-cos-meta-sha256"] ?? null,
          contentType: headers["content-type"] ?? null,
          cacheControl: headers["cache-control"] ?? null,
        };
      } catch (error) {
        if (missing(error)) return null;
        throw error;
      }
    },
    async head(key) {
      try {
        const data = await acceleratedCos.headObject(objectParams(key));
        const headers = headersOf(data);
        return {
          etag: data.ETag ?? headers.etag ?? null,
          size: Number(headers["content-length"] ?? -1),
          sha256: headers["x-cos-meta-sha256"] ?? null,
        };
      } catch (error) {
        if (missing(error)) return null;
        throw error;
      }
    },
    async put(object) {
      await acceleratedCos.putObject({
        ...objectParams(object.key),
        Body: object.body ?? createReadStream(object.path),
        ContentLength: object.size,
        ContentType: object.contentType,
        ...(object.cacheControl ? { CacheControl: object.cacheControl } : {}),
        "x-cos-meta-sha256": object.sha256,
      });
    },
    async delete(key) {
      await originCos.deleteObject(objectParams(key));
    },
  };

  const CdnClient = cdnSdk.v20180606.Client;
  const client = new CdnClient({
    credential: { secretId, secretKey },
    profile: { httpProfile: { endpoint: "cdn.tencentcloudapi.com" } },
  });
  const cdn = {
    async purgeAndWait(url) {
      const submitted = await client.PurgeUrlsCache({ Urls: [url] });
      const taskId = submitted.TaskId;
      if (!taskId) fail("Tencent CDN did not return a purge task ID");
      const deadline = Date.now() + 5 * 60 * 1000;
      while (Date.now() < deadline) {
        const result = await client.DescribePurgeTasks({
          PurgeType: "url",
          TaskId: taskId,
          Limit: 20,
        });
        const logs = Array.isArray(result.PurgeLogs) ? result.PurgeLogs : [];
        const statuses = logs
          .filter((item) => item.TaskId === taskId)
          .map((item) => String(item.Status ?? "").toLowerCase());
        if (
          statuses.length > 0 &&
          statuses.every((status) => status === "done")
        ) {
          return taskId;
        }
        if (statuses.some((status) => ["fail", "invalid"].includes(status))) {
          fail(`Tencent CDN purge task failed: ${taskId}`);
        }
        await wait(3000);
      }
      fail(`Tencent CDN purge task timed out: ${taskId}`);
    },
  };
  return { store, cdn };
};

const requireEnvironment = (name) => {
  const value = process.env[name]?.trim();
  if (!value) fail(`missing required environment variable: ${name}`);
  return value;
};

const writePlanPreview = async (plan, outputDir) => {
  await mkdir(outputDir, { recursive: true });
  const latestDocument = mergeLatestDocument(null, plan.latestCore);
  await writeFile(join(outputDir, "latest.json"), jsonBody(latestDocument));
  await writeFile(
    join(outputDir, "publish-plan.json"),
    jsonBody({
      version: plan.version,
      version_objects: plan.versionObjects.map(({ key, size, sha256 }) => ({
        key,
        size,
        sha256,
      })),
      mutable_objects: [
        ...plan.mutableObjects.map(({ key, size, sha256 }) => ({
          key,
          size,
          sha256,
        })),
        { key: LATEST_KEY, commit_point: true },
      ],
    }),
  );
};

const main = async () => {
  const command = process.argv[2] ?? "plan";
  if (!["plan", "publish"].includes(command)) {
    fail("usage: fn-knock-cos-publish.mjs <plan|publish>");
  }
  const version = requireEnvironment("FN_KNOCK_VERSION");
  const publicBaseUrl = requireEnvironment("COS_PUBLICBASICURL");
  const previousReleases = process.env.FN_KNOCK_RELEASE_HISTORY_FILE
    ? JSON.parse(
        await readFile(process.env.FN_KNOCK_RELEASE_HISTORY_FILE, "utf8"),
      )
    : await fetchPreviousReleases({
        repository: requireEnvironment("GITHUB_REPOSITORY"),
        token: requireEnvironment("GITHUB_TOKEN"),
        apiUrl: process.env.GITHUB_API_URL,
      });
  const plan = await buildReleasePlan({
    assetsDir: resolve(
      process.env.FN_KNOCK_RELEASE_ASSETS_DIR ?? "dist/release-assets",
    ),
    windowsMetadataDir: resolve(
      process.env.FN_KNOCK_WINDOWS_METADATA_DIR ?? "dist/cos-windows-release",
    ),
    installScriptPath: resolve(
      process.env.FN_KNOCK_INSTALL_SCRIPT ?? "deploy/linux/install.sh",
    ),
    releaseNotesPath: resolve(
      process.env.FN_KNOCK_RELEASE_NOTES_PATH ?? `release-notes/${version}.md`,
    ),
    version,
    publicBaseUrl,
    previousReleases,
  });
  const outputDir = resolve(
    process.env.FN_KNOCK_COS_OUTPUT_DIR ?? "dist/cos-publish",
  );
  await writePlanPreview(plan, outputDir);
  console.log(
    `[cos-publish] validated ${plan.manifest.artifacts.length} release artifacts for ${version}`,
  );
  if (command === "plan") {
    console.log(`[cos-publish] wrote dry-run plan to ${outputDir}`);
    return;
  }

  const adapters = createTencentAdapters({
    secretId: requireEnvironment("COS_SECRETID"),
    secretKey: requireEnvironment("COS_SECRETKEY"),
    bucket: requireEnvironment("COS_BUCKET"),
    region: requireEnvironment("COS_REGION"),
    accelerateDomain: requireEnvironment("COS_ACC"),
  });
  await publishRelease({
    plan,
    ...adapters,
    latestUrl: requireEnvironment("FN_KNOCK_LATEST_URL"),
  });
};

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(resolve(process.argv[1])).href
) {
  main().catch((error) => {
    if (error instanceof AggregateError) {
      console.error(`[cos-publish] ${error.message}`);
      for (const item of error.errors) console.error(item);
    } else {
      console.error(error instanceof Error ? error.message : error);
    }
    process.exitCode = 1;
  });
}
