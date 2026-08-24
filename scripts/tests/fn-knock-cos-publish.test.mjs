import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import {
  buildMacosReleasePlan,
  buildReleasePlan,
  checkMacosRelease,
  compareVersions,
  composeReleaseNotes,
  mergeMacosLatestDocument,
  mergeLatestDocument,
  normalizeCosAccelerateDomain,
  publishRelease,
  verifyLatestDocument,
} from "../fn-knock-cos-publish.mjs";

const VERSION = "3.4.5";
const EXPECTED_RELEASE_NOTES_HEADER = [
  "[**用户协议与隐私政策**](https://www.fnknock.cn/legal)",
  "如果您是在飞牛应用商店安装的Knock，建议在官网重新下载FPK版本，功能更全面",
  "我们推出了OpenWrt应用（IPK、Alpine APK），以及群晖SPK原生支持，欢迎在官网下载安装体验",
  "[**官网**](https://www.fnknock.cn/)  、[**文档**](https://docs.fnknock.cn/) 、 [**Docker版**](https://hub.docker.com/r/kcilnk/fn-knock)、[**Windows版**](https://www.fnknock.cn/windows) 、 [**Linux一键脚本**](https://www.fnknock.cn/linux)、[**群晖套件**](https://www.fnknock.cn/synology)",
  "QQ群：1081609274",
].join("\n\n");

const sha256 = (body) => createHash("sha256").update(body).digest("hex");

const artifactDefinitions = () => [
  [`fn-knock-${VERSION}-fnos-amd64.fpk`, "fnos", "amd64"],
  [`fn-knock-${VERSION}-fnos-arm64.fpk`, "fnos", "arm64"],
  [`fn-knock-linux-${VERSION}-amd64.tar.gz`, "linux", "amd64"],
  [`fn-knock-linux-${VERSION}-arm64.tar.gz`, "linux", "arm64"],
  [`fn-knock-linux-${VERSION}-arm.tar.gz`, "linux", "armv7"],
  [`fn-knock-macos-${VERSION}-amd64.tar.gz`, "macos", "amd64"],
  [`fn-knock-macos-${VERSION}-arm64.tar.gz`, "macos", "arm64"],
  [`app-meta-fn-knock_${VERSION}-r1_all.ipk`, "openwrt", "all"],
  [`app-meta-fn-knock-${VERSION}-r1.apk`, "openwrt", "all"],
  [
    `fn-knock_${VERSION}-1_aarch64_cortex-a53.ipk`,
    "openwrt",
    "aarch64_cortex-a53",
  ],
  [
    `fn-knock_${VERSION}-r1_aarch64_cortex-a53.apk`,
    "openwrt",
    "aarch64_cortex-a53",
  ],
  [`fn-knock_${VERSION}-1_aarch64_generic.ipk`, "openwrt", "aarch64_generic"],
  [`fn-knock_${VERSION}-r1_aarch64_generic.apk`, "openwrt", "aarch64_generic"],
  [
    `fn-knock_${VERSION}-1_arm_cortex-a5_vfpv4.ipk`,
    "openwrt",
    "arm_cortex-a5_vfpv4",
  ],
  [
    `fn-knock_${VERSION}-r1_arm_cortex-a5_vfpv4.apk`,
    "openwrt",
    "arm_cortex-a5_vfpv4",
  ],
  [
    `fn-knock_${VERSION}-1_arm_cortex-a7_neon-vfpv4.ipk`,
    "openwrt",
    "arm_cortex-a7_neon-vfpv4",
  ],
  [
    `fn-knock_${VERSION}-r1_arm_cortex-a7_neon-vfpv4.apk`,
    "openwrt",
    "arm_cortex-a7_neon-vfpv4",
  ],
  [`fn-knock_${VERSION}-1_x86_64.ipk`, "openwrt", "x86_64"],
  [`fn-knock_${VERSION}-r1_x86_64.apk`, "openwrt", "x86_64"],
  [`fn-knock-synology-x86_64-${VERSION}-0017.spk`, "synology", "x86_64"],
  [`fn-knock-synology-armv8-${VERSION}-0017.spk`, "synology", "armv8"],
  [`fn-knock-synology-armv7-${VERSION}-0017.spk`, "synology", "armv7"],
  [
    `fn-knock-${VERSION}-windows-x86_64-unsigned-setup.exe`,
    "windows",
    "x86_64",
  ],
];

const createFixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "fn-knock-cos-publish-test-"));
  const assetsDir = join(root, "assets");
  const windowsMetadataDir = join(root, "windows");
  const installScriptPath = join(root, "install.sh");
  const macosInstallScriptPath = join(root, "macos-install.sh");
  const releaseNotesPath = join(root, "release-notes.md");
  await Promise.all([
    mkdir(assetsDir, { recursive: true }),
    mkdir(windowsMetadataDir, { recursive: true }),
  ]);

  const artifacts = [];
  for (const [name, platform, architecture] of artifactDefinitions()) {
    const body = Buffer.from(`fixture:${name}\n`);
    await writeFile(join(assetsDir, name), body);
    artifacts.push({
      name,
      platform,
      architecture,
      size: body.byteLength,
      sha256: sha256(body),
    });
  }
  await writeFile(
    join(assetsDir, "release-manifest.json"),
    `${JSON.stringify(
      {
        schema_version: 1,
        version: VERSION,
        tag: `v${VERSION}`,
        artifacts,
        metadata_files: ["release-manifest.json", "SHA256SUMS"],
      },
      null,
      2,
    )}\n`,
  );

  const setup = artifacts.find((artifact) => artifact.platform === "windows");
  await writeFile(
    join(windowsMetadataDir, `${setup.name}.sha256`),
    `${setup.sha256}  ${setup.name}\n`,
  );
  await writeFile(
    join(
      windowsMetadataDir,
      `fn-knock-${VERSION}-windows-x86_64-unsigned-release.json`,
    ),
    `${JSON.stringify({
      version: VERSION,
      runtime_target: "windows",
      architecture: "x86_64",
    })}\n`,
  );
  await writeFile(
    join(
      windowsMetadataDir,
      `fn-knock-${VERSION}-windows-x86_64-unsigned-updater.json`,
    ),
    `${JSON.stringify({
      version: VERSION,
      notes: "Windows notes",
      pub_date: "2026-07-22T00:00:00.000Z",
    })}\n`,
  );
  await writeFile(installScriptPath, "#!/bin/sh\necho install\n");
  await writeFile(macosInstallScriptPath, "#!/bin/sh\necho macos install\n");
  await writeFile(
    releaseNotesPath,
    `# fn-knock ${VERSION}\n\n- Current release\n`,
  );

  return {
    root,
    assetsDir,
    windowsMetadataDir,
    installScriptPath,
    macosInstallScriptPath,
    releaseNotesPath,
    artifacts,
  };
};

const buildFixturePlan = (fixture, overrides = {}) =>
  buildReleasePlan({
    ...fixture,
    version: VERSION,
    publicBaseUrl: "https://cdn.example.test/",
    previousReleases: [],
    ...overrides,
  });

const buildMacosFixturePlan = (fixture, overrides = {}) =>
  buildMacosReleasePlan({
    assetsDir: fixture.assetsDir,
    installScriptPath: fixture.macosInstallScriptPath,
    version: VERSION,
    publicBaseUrl: "https://cdn.example.test/",
    ...overrides,
  });

const currentMacosLatest = () => ({
  version: VERSION,
  update_available: true,
  force_update: false,
  custom_root: { preserved: true },
  packages: {
    fpk: { amd64: { file_name: "kept.fpk" } },
    linux: { arm64: { file_name: "kept.tar.gz" } },
    macos: { stale: { file_name: "old-macos.tar.gz" } },
    windows: { x86_64: { file_name: "kept.exe" } },
    custom: { channel: "kept" },
  },
});

class FakeStore {
  constructor(initial = {}) {
    this.objects = new Map();
    this.operations = [];
    this.sequence = 0;
    for (const [key, body] of Object.entries(initial)) {
      this.set(key, Buffer.from(body), {
        contentType: key.endsWith(".json")
          ? "application/json; charset=utf-8"
          : "text/plain; charset=utf-8",
        cacheControl: "no-cache",
      });
    }
  }

  set(key, body, metadata = {}) {
    this.sequence += 1;
    this.objects.set(key, {
      body: Buffer.from(body),
      etag: `etag-${this.sequence}`,
      size: body.byteLength,
      sha256: sha256(body),
      contentType: metadata.contentType ?? "application/octet-stream",
      cacheControl: metadata.cacheControl ?? null,
    });
  }

  async get(key) {
    const value = this.objects.get(key);
    return value ? { ...value, body: Buffer.from(value.body) } : null;
  }

  async head(key) {
    const value = this.objects.get(key);
    return value
      ? { etag: value.etag, size: value.size, sha256: value.sha256 }
      : null;
  }

  async put(object) {
    const body = object.body ?? (await readFile(object.path));
    this.operations.push(`put:${object.key}`);
    this.set(object.key, body, object);
  }

  async delete(key) {
    this.operations.push(`delete:${key}`);
    this.objects.delete(key);
  }
}

const oldPointers = (plan) => {
  const values = Object.fromEntries(
    plan.mutableObjects.map((object) => [object.key, `old:${object.key}\n`]),
  );
  values["latest.json"] = `${JSON.stringify({
    version: "3.4.4",
    custom_root: true,
    packages: { custom: { channel: "stable" } },
  })}\n`;
  return values;
};

test("builds a complete 23-package COS plan", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);

  assert.equal(plan.manifest.artifacts.length, 23);
  assert.equal(plan.versionObjects.length, 26);
  assert.equal("header" in plan.latestCore, false);
  assert.equal(plan.latestCore.force_update, false);
  assert.ok(
    plan.latestCore.release_notes.startsWith(
      `${EXPECTED_RELEASE_NOTES_HEADER}\n\n# fn-knock ${VERSION}`,
    ),
  );
  assert.deepEqual(Object.keys(plan.latestCore.packages.fpk).sort(), [
    "amd64",
    "arm64",
  ]);
  assert.equal(plan.latestCore.packages.apk.all.arch, "all");
  assert.equal(
    plan.latestCore.packages.linux.arm.object_key,
    `files/${VERSION}/linux/arm/fn-knock-linux-${VERSION}-arm.tar.gz`,
  );
  assert.equal(
    plan.latestCore.packages.macos.arm64.object_key,
    `files/${VERSION}/macos/arm64/fn-knock-macos-${VERSION}-arm64.tar.gz`,
  );
  assert.match(
    plan.latestCore.packages.windows.x86_64.file_name,
    /-unsigned-setup\.exe$/,
  );
  const windowsRelease = plan.versionObjects.find((object) =>
    object.key.endsWith("/windows/x86_64/release.json"),
  );
  const windowsReleaseBody = JSON.parse(windowsRelease.body.toString("utf8"));
  assert.equal(
    windowsReleaseBody.packages.windows.x86_64.url,
    plan.latestCore.packages.windows.x86_64.download_url,
  );
  assert.equal(
    plan.latestCore.download_url,
    `https://cdn.example.test/files/${VERSION}/fn-knock-${VERSION}-fnos-amd64.fpk`,
  );
  assert.deepEqual(
    plan.mutableObjects.map((object) => object.key),
    [
      "install.sh",
      "macos/install.sh",
      "linux/latest/amd64.env",
      "linux/latest/arm64.env",
      "linux/latest/arm.env",
      "macos/latest/amd64.env",
      "macos/latest/arm64.env",
      "windows/stable/latest.json",
    ],
  );
  const macosArm64Pointer = plan.mutableObjects.find(
    (object) => object.key === "macos/latest/arm64.env",
  );
  assert.equal(
    macosArm64Pointer.body.toString("utf8"),
    [
      `VERSION=${VERSION}`,
      `URL=${plan.latestCore.packages.macos.arm64.download_url}`,
      `SHA256=${plan.latestCore.packages.macos.arm64.sha256}`,
      `SIZE=${plan.latestCore.packages.macos.arm64.size}`,
      "",
    ].join("\n"),
  );
});

test("marks IMPORTANT release notes as a forced important update", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  await writeFile(
    fixture.releaseNotesPath,
    `# fn-knock ${VERSION}\n\n> [!IMPORTANT]\n> Important stability update.\n`,
  );

  const plan = await buildFixturePlan(fixture);
  assert.equal(plan.latestCore.force_update, true);

  const windowsPointer = plan.mutableObjects.find(
    (object) => object.key === "windows/stable/latest.json",
  );
  const windowsLatest = JSON.parse(windowsPointer.body.toString("utf8"));
  assert.equal(windowsLatest.fn_knock.package.force_update, true);
});

test("builds a two-architecture macOS-only COS plan", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);

  assert.equal(plan.publishMode, "macos-only");
  assert.deepEqual(
    plan.manifest.artifacts.map((artifact) => artifact.architecture),
    ["amd64", "arm64"],
  );
  assert.deepEqual(
    plan.versionObjects.map((object) => object.key),
    [
      `files/${VERSION}/macos/amd64/fn-knock-macos-${VERSION}-amd64.tar.gz`,
      `files/${VERSION}/macos/arm64/fn-knock-macos-${VERSION}-arm64.tar.gz`,
    ],
  );
  assert.deepEqual(
    plan.mutableObjects.map((object) => object.key),
    ["macos/install.sh", "macos/latest/amd64.env", "macos/latest/arm64.env"],
  );
  assert.deepEqual(Object.keys(plan.latestCore.packages.macos).sort(), [
    "amd64",
    "arm64",
  ]);
  assert.ok(
    plan.cdnVerificationObjects.every((object) =>
      object.url.startsWith("https://cdn.example.test/macos/"),
    ),
  );
});

test("requires HTTPS for public package and installer URLs", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  await assert.rejects(
    buildFixturePlan(fixture, {
      publicBaseUrl: "http://cdn.example.test/",
    }),
    /absolute HTTPS URL/,
  );
});

test("composes current notes plus at most five earlier stable releases", () => {
  const releases = [
    ...Array.from({ length: 8 }, (_, index) => ({
      tag_name: `v3.4.${index}`,
      body: `notes-${index}`,
      draft: false,
      prerelease: false,
    })),
    { tag_name: "v3.4.4", body: "duplicate", draft: false, prerelease: false },
    { tag_name: "v3.4.6", body: "newer", draft: false, prerelease: false },
    { tag_name: "v3.3.9", body: "draft", draft: true, prerelease: false },
    { tag_name: "v3.3.8", body: "prerelease", draft: false, prerelease: true },
  ];
  const notes = composeReleaseNotes("current", VERSION, releases);
  assert.ok(notes.startsWith(`${EXPECTED_RELEASE_NOTES_HEADER}\n\ncurrent`));
  assert.deepEqual(notes.split("\n\n---\n\n"), [
    `${EXPECTED_RELEASE_NOTES_HEADER}\n\ncurrent`,
    "notes-4",
    "notes-3",
    "notes-2",
    "notes-1",
    "notes-0",
  ]);
  assert.equal(compareVersions("3.4.10", "3.4.9"), 1);
});

test("validates the configured COS acceleration endpoint", () => {
  assert.equal(
    normalizeCosAccelerateDomain(
      " Example-Bucket-1250000000.acceleration.example.test. ",
      "example-bucket-1250000000",
    ),
    "example-bucket-1250000000.acceleration.example.test",
  );
  assert.throws(
    () =>
      normalizeCosAccelerateDomain(
        "https://example-bucket-1250000000.example.test",
        "example-bucket-1250000000",
      ),
    /hostname without a protocol/,
  );
  assert.throws(
    () =>
      normalizeCosAccelerateDomain(
        "other-bucket-1250000000.example.test",
        "example-bucket-1250000000",
      ),
    /configured COS_BUCKET/,
  );
});

test("preserves unknown latest fields while removing the legacy root header", () => {
  const merged = mergeLatestDocument(
    {
      version: "1.0.0",
      header: "stale announcement",
      custom_root: "kept",
      packages: {
        fpk: { stale: true },
        custom: { channel: "kept" },
      },
    },
    {
      version: VERSION,
      release_notes: "notes",
      packages: Object.fromEntries(
        ["fpk", "ipk", "apk", "synology", "linux", "macos", "windows"].map(
          (type) => [type, {}],
        ),
      ),
    },
  );
  assert.equal("header" in merged, false);
  assert.equal(merged.custom_root, "kept");
  assert.deepEqual(merged.packages.custom, { channel: "kept" });
  assert.deepEqual(merged.packages.fpk, {});
});

test("macOS-only merge preserves every non-macOS latest field", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);
  const current = currentMacosLatest();
  const merged = mergeMacosLatestDocument(current, plan.latestCore);

  assert.equal(merged.version, VERSION);
  assert.deepEqual(merged.custom_root, current.custom_root);
  assert.deepEqual(merged.packages.fpk, current.packages.fpk);
  assert.deepEqual(merged.packages.linux, current.packages.linux);
  assert.deepEqual(merged.packages.windows, current.packages.windows);
  assert.deepEqual(merged.packages.custom, current.packages.custom);
  assert.deepEqual(merged.packages.macos, plan.latestCore.packages.macos);
  assert.throws(
    () =>
      mergeMacosLatestDocument(
        { ...current, version: "3.4.4" },
        plan.latestCore,
      ),
    /requires current latest\.json version 3\.4\.5/,
  );
});

test("rejects tampered artifacts and duplicate architectures", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  await writeFile(
    join(fixture.assetsDir, fixture.artifacts[0].name),
    "tampered",
  );
  await assert.rejects(
    buildFixturePlan(fixture),
    /size mismatch|SHA-256 mismatch/,
  );

  const second = await createFixture();
  context.after(() => rm(second.root, { recursive: true, force: true }));
  const manifestPath = join(second.assetsDir, "release-manifest.json");
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.artifacts[1].architecture = "amd64";
  await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  await assert.rejects(buildFixturePlan(second), /duplicate fpk architecture/);
});

test("uploads versioned objects before pointers and latest.json last", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const store = new FakeStore(oldPointers(plan));
  const events = [];
  const cdn = {
    async purgeAndWait(url) {
      events.push(`purge:${url}`);
      return "task-1";
    },
  };
  const result = await publishRelease({
    plan,
    store,
    cdn,
    latestUrl: "https://cor.example.test/latest.json",
    fetchImpl: async () => {
      events.push("fetch:latest");
      return new Response((await store.get("latest.json")).body, {
        status: 200,
      });
    },
    wait: async () => {},
    log: () => {},
  });

  const latestPut = store.operations.indexOf("put:latest.json");
  const firstPointerPut = Math.min(
    ...plan.mutableObjects.map((object) =>
      store.operations.indexOf(`put:${object.key}`),
    ),
  );
  for (const object of plan.versionObjects) {
    assert.ok(store.operations.indexOf(`put:${object.key}`) < firstPointerPut);
  }
  assert.equal(latestPut, store.operations.length - 1);
  assert.equal(events[0], "purge:https://cor.example.test/latest.json");
  assert.equal(events[1], "fetch:latest");
  assert.equal(result.latestDocument.custom_root, true);
  assert.deepEqual(result.latestDocument.packages.custom, {
    channel: "stable",
  });
  assert.ok(
    verifyLatestDocument(
      JSON.parse((await store.get("latest.json")).body.toString("utf8")),
      result.latestDocument,
    ),
  );
});

test("publishes only macOS pointers and preserves the current release", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);
  const current = currentMacosLatest();
  const initial = Object.fromEntries(
    plan.mutableObjects.map((object) => [object.key, `old:${object.key}\n`]),
  );
  initial["latest.json"] = `${JSON.stringify(current)}\n`;
  const store = new FakeStore(initial);
  const purges = [];

  const preflight = await checkMacosRelease({
    plan,
    store,
    log: () => {},
  });
  assert.equal(preflight.missingVersionObjects.length, 2);
  assert.deepEqual(store.operations, []);

  const result = await publishRelease({
    plan,
    store,
    cdn: {
      async purgeAndWait(urls) {
        purges.push(urls);
        return "macos-task";
      },
    },
    latestUrl: "https://cdn.example.test/latest.json",
    fetchImpl: async (url) => {
      if (url === "https://cdn.example.test/latest.json") {
        return new Response((await store.get("latest.json")).body, {
          status: 200,
        });
      }
      const object = plan.cdnVerificationObjects.find(
        (candidate) => candidate.url === url,
      );
      assert.ok(object, `unexpected CDN verification URL: ${url}`);
      return new Response((await store.get(object.key)).body, { status: 200 });
    },
    wait: async () => {},
    log: () => {},
  });

  assert.equal(result.latestDocument.version, VERSION);
  assert.deepEqual(result.latestDocument.custom_root, current.custom_root);
  assert.deepEqual(result.latestDocument.packages.fpk, current.packages.fpk);
  assert.deepEqual(
    result.latestDocument.packages.linux,
    current.packages.linux,
  );
  assert.deepEqual(
    result.latestDocument.packages.windows,
    current.packages.windows,
  );
  assert.deepEqual(
    result.latestDocument.packages.macos,
    plan.latestCore.packages.macos,
  );
  assert.deepEqual(purges, [
    [
      "https://cdn.example.test/latest.json",
      "https://cdn.example.test/macos/install.sh",
      "https://cdn.example.test/macos/latest/amd64.env",
      "https://cdn.example.test/macos/latest/arm64.env",
    ],
  ]);
  const writes = store.operations.filter((operation) =>
    operation.startsWith("put:"),
  );
  assert.equal(writes.at(-1), "put:latest.json");
  assert.ok(writes.every((operation) => !operation.includes("linux/")));
  assert.ok(writes.every((operation) => !operation.includes("windows/")));
});

test("macOS-only preflight refuses a different or missing current version", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);
  for (const latest of [null, { ...currentMacosLatest(), version: "3.4.4" }]) {
    const store = new FakeStore(
      latest ? { "latest.json": `${JSON.stringify(latest)}\n` } : {},
    );
    await assert.rejects(
      checkMacosRelease({ plan, store, log: () => {} }),
      /requires (an existing latest\.json|current latest\.json version 3\.4\.5)/,
    );
    assert.deepEqual(store.operations, []);
  }
});

test("macOS-only preflight refuses different same-version package bytes", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);
  const first = plan.versionObjects[0];
  const store = new FakeStore({
    "latest.json": `${JSON.stringify(currentMacosLatest())}\n`,
    [first.key]: "different macOS build\n",
  });
  await assert.rejects(
    checkMacosRelease({ plan, store, log: () => {} }),
    /same-version COS object (size|SHA-256) mismatch/,
  );
  assert.deepEqual(store.operations, []);
});

test("macOS-only publication restores every pointer after CDN failure", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildMacosFixturePlan(fixture);
  const initial = Object.fromEntries(
    plan.mutableObjects.map((object) => [object.key, `old:${object.key}\n`]),
  );
  initial["latest.json"] = `${JSON.stringify(currentMacosLatest())}\n`;
  const store = new FakeStore(initial);
  const purges = [];

  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: {
        async purgeAndWait(urls) {
          purges.push(urls);
          if (purges.length === 1) throw new Error("macOS purge failed");
          return "restore-task";
        },
      },
      latestUrl: "https://cdn.example.test/latest.json",
      fetchImpl: async () => new Response("{}", { status: 200 }),
      wait: async () => {},
      log: () => {},
    }),
    /macOS purge failed/,
  );

  assert.equal(purges.length, 2);
  assert.ok(Array.isArray(purges[0]));
  for (const [key, body] of Object.entries(initial)) {
    assert.equal((await store.get(key)).body.toString("utf8"), body);
  }
});

test("requires an exact CDN latest document", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const expected = mergeLatestDocument(
    { custom_root: true, packages: { custom: { channel: "stable" } } },
    plan.latestCore,
  );
  const wrongUrl = structuredClone(expected);
  wrongUrl.packages.fpk.amd64.download_url =
    "https://attacker.invalid/package.fpk";
  assert.equal(verifyLatestDocument(wrongUrl, expected), false);

  const wrongFlags = structuredClone(expected);
  wrongFlags.update_available = false;
  assert.equal(verifyLatestDocument(wrongFlags, expected), false);
  assert.equal(verifyLatestDocument(structuredClone(expected), expected), true);
});

test("rolls back mutable pointers when CDN purge fails", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const initial = oldPointers(plan);
  const store = new FakeStore(initial);
  let purgeCalls = 0;
  const cdn = {
    async purgeAndWait() {
      purgeCalls += 1;
      if (purgeCalls === 1) throw new Error("purge failed");
      return "rollback-task";
    },
  };

  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn,
      latestUrl: "https://cor.example.test/latest.json",
      fetchImpl: async () => new Response("{}", { status: 200 }),
      wait: async () => {},
      log: () => {},
    }),
    /purge failed/,
  );
  assert.equal(purgeCalls, 2);
  for (const [key, expected] of Object.entries(initial)) {
    assert.equal((await store.get(key)).body.toString("utf8"), expected);
  }
});

test("rolls back a pointer when PUT succeeds but HEAD verification fails", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const initial = oldPointers(plan);
  const store = new FakeStore(initial);
  const originalPut = store.put.bind(store);
  const originalHead = store.head.bind(store);
  const failedKey = plan.mutableObjects[0].key;
  let failNextHead = false;
  store.put = async (object) => {
    await originalPut(object);
    if (object.key === failedKey) failNextHead = true;
  };
  store.head = async (key) => {
    if (key === failedKey && failNextHead) {
      failNextHead = false;
      throw new Error("simulated HEAD timeout");
    }
    return originalHead(key);
  };

  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: { purgeAndWait: async () => "unused" },
      latestUrl: "https://cor.example.test/latest.json",
      log: () => {},
    }),
    /simulated HEAD timeout/,
  );
  for (const [key, expected] of Object.entries(initial)) {
    assert.equal((await store.get(key)).body.toString("utf8"), expected);
  }
});

test("refuses version downgrade before uploading any object", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const store = new FakeStore({
    ...oldPointers(plan),
    "latest.json": `${JSON.stringify({ version: "9.0.0" })}\n`,
  });
  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: { purgeAndWait: async () => "unused" },
      latestUrl: "https://cor.example.test/latest.json",
      log: () => {},
    }),
    /refusing to downgrade/,
  );
  assert.deepEqual(store.operations, []);
});

test("refuses to overwrite different same-version objects", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const first = plan.versionObjects[0];
  const store = new FakeStore({
    ...oldPointers(plan),
    "latest.json": `${JSON.stringify({ version: VERSION })}\n`,
    [first.key]: "different same-version build\n",
  });

  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: { purgeAndWait: async () => "unused" },
      latestUrl: "https://cor.example.test/latest.json",
      log: () => {},
    }),
    /same-version COS object (size|SHA-256) mismatch/,
  );
  assert.deepEqual(store.operations, []);
});

test("resumes and replaces a partial version upload while latest still points to the old release", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const completed = plan.versionObjects.slice(0, 6);
  const store = new FakeStore(oldPointers(plan));
  for (const object of completed) {
    store.set(object.key, object.body ?? (await readFile(object.path)), object);
  }
  const stale = plan.versionObjects[completed.length];
  store.set(stale.key, Buffer.from("stale same-version build\n"), stale);

  await publishRelease({
    plan,
    store,
    cdn: { purgeAndWait: async () => "task-resume" },
    latestUrl: "https://cor.example.test/latest.json",
    fetchImpl: async () =>
      new Response((await store.get("latest.json")).body, { status: 200 }),
    wait: async () => {},
    log: () => {},
  });

  for (const object of completed) {
    assert.ok(!store.operations.includes(`put:${object.key}`));
  }
  assert.ok(store.operations.includes(`put:${stale.key}`));
  assert.equal((await store.head(stale.key)).sha256, stale.sha256);
  for (const object of plan.versionObjects.slice(6)) {
    assert.ok(store.operations.includes(`put:${object.key}`));
  }
});

test("detects a pointer ETag conflict before pointer writes", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const store = new FakeStore(oldPointers(plan));
  const originalPut = store.put.bind(store);
  let changed = false;
  store.put = async (object) => {
    await originalPut(object);
    if (!changed && object.key === plan.versionObjects.at(-1).key) {
      changed = true;
      store.set(
        "latest.json",
        Buffer.from('{"version":"3.4.4","race":true}\n'),
      );
    }
  };
  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: { purgeAndWait: async () => "unused" },
      latestUrl: "https://cor.example.test/latest.json",
      log: () => {},
    }),
    /concurrent COS pointer update detected: latest.json/,
  );
  assert.ok(
    plan.mutableObjects.every(
      (object) => !store.operations.includes(`put:${object.key}`),
    ),
  );
});

test("detects a pointer conflict that occurs during pointer writes", async (context) => {
  const fixture = await createFixture();
  context.after(() => rm(fixture.root, { recursive: true, force: true }));
  const plan = await buildFixturePlan(fixture);
  const initial = oldPointers(plan);
  const store = new FakeStore(initial);
  const originalPut = store.put.bind(store);
  const firstKey = plan.mutableObjects[0].key;
  const conflictedKey = plan.mutableObjects[1].key;
  let changed = false;
  store.put = async (object) => {
    await originalPut(object);
    if (!changed && object.key === firstKey) {
      changed = true;
      store.set(conflictedKey, Buffer.from("concurrent publisher\n"));
    }
  };

  await assert.rejects(
    publishRelease({
      plan,
      store,
      cdn: { purgeAndWait: async () => "unused" },
      latestUrl: "https://cor.example.test/latest.json",
      log: () => {},
    }),
    new RegExp(`concurrent COS pointer update detected: ${conflictedKey}`),
  );
  assert.equal(
    (await store.get(firstKey)).body.toString("utf8"),
    initial[firstKey],
  );
  assert.equal(
    (await store.get(conflictedKey)).body.toString("utf8"),
    "concurrent publisher\n",
  );
  assert.equal(
    (await store.get("latest.json")).body.toString("utf8"),
    initial["latest.json"],
  );
});
