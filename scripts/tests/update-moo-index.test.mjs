import assert from "node:assert/strict";
import {
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

import {
  buildMooIndex,
  mergeMooIndex,
  updateMooIndexFile,
  publishMooIndex,
  readRelease,
} from "../update-moo-index.mjs";

const manifest = await readFile(
  new URL("../../apps/fn-knock/manifest", import.meta.url),
  "utf8",
);
const fixture = () => ({
  tag_name: "v3.4.5",
  draft: false,
  prerelease: false,
  published_at: "2026-10-01T00:00:00Z",
  body: "- Release notes",
  assets: ["amd64", "arm64"].map((architecture, index) => ({
    name: `fn-knock-3.4.5-fnos-${architecture}.fpk`,
    browser_download_url: `https://github.com/kci-lnk/fn-knock-turborepo/releases/download/v3.4.5/fn-knock-3.4.5-fnos-${architecture}.fpk`,
    digest: `sha256:${String(index + 1).repeat(64)}`,
    size: 10_000_000 + index,
  })),
});

test("maps both FPK architectures with byte sizes and their own checksums", () => {
  const release = fixture();
  const index = buildMooIndex(release, manifest);
  const app = index.apps["fn-knock"];
  assert.equal(index.schema_version, "moo");
  assert.equal(app.install_type, "root");
  assert.equal(app.service_port, 7999);
  assert.ok(!app.desc.includes('"""'));
  assert.equal(app.download_url, undefined);
  assert.equal(app.version, undefined);
  assert.deepEqual(app.platform, ["x86", "arm"]);
  const entry = app.releases["3.4.5"];
  assert.equal(entry.changelog, release.body);
  for (const [i, platform] of ["x86", "arm"].entries()) {
    assert.deepEqual(entry.packages[platform], {
      download_url: release.assets[i].browser_download_url,
      sha256: String(i + 1).repeat(64),
      size: release.assets[i].size,
    });
  }
});

test("rejects drafts, prereleases, and nonnumeric tags", () => {
  for (const patch of [
    { draft: true },
    { prerelease: true },
    { tag_name: "v3.4.5-beta" },
  ]) {
    assert.throws(
      () => buildMooIndex({ ...fixture(), ...patch }, manifest),
      /published stable/,
    );
  }
});

test("rejects missing or duplicate FPKs and unusable package metadata", () => {
  const mutations = [
    (release) => release.assets.pop(),
    (release) => release.assets.push({ ...release.assets[0] }),
    (release) => delete release.assets[0].digest,
    (release) => {
      release.assets[0].digest = "sha256:invalid";
    },
    (release) => {
      release.assets[0].size = 0;
    },
    (release) => {
      release.assets[0].browser_download_url = "https://example.com/wrong.fpk";
    },
  ];
  for (const mutate of mutations) {
    const release = fixture();
    mutate(release);
    assert.throws(
      () => buildMooIndex(release, manifest),
      /release asset|invalid SHA-256/,
    );
  }
});

test("rejects a Lite manifest and missing publication dates", () => {
  assert.throws(
    () =>
      buildMooIndex(
        fixture(),
        manifest.replace("appname=fn-knock\n", "appname=fn-knock-lite\n"),
      ),
    /full fn-knock/,
  );
  assert.throws(
    () => buildMooIndex({ ...fixture(), published_at: null }, manifest),
    /publication date/,
  );
});

const generatedIndex = () => buildMooIndex(fixture(), manifest);
const remoteFile = (index, sha = "previous-file-sha") => ({
  status: 200,
  body: {
    type: "file",
    encoding: "base64",
    sha,
    content: Buffer.from(`${JSON.stringify(index, null, 2)}\n`).toString(
      "base64",
    ),
  },
});
const previousIndex = (version) => {
  const index = generatedIndex();
  index.apps["fn-knock"].releases = {
    [version]: index.apps["fn-knock"].releases["3.4.5"],
  };
  return index;
};

test("reads the exact released tag rather than whichever release is latest", async () => {
  const release = await readRelease("v3.4.5", async (endpoint) => {
    assert.equal(endpoint, "releases/tags/v3.4.5");
    return { status: 200, body: fixture() };
  });
  assert.equal(release.tag_name, "v3.4.5");
  await assert.rejects(
    readRelease("v3.4.6", async () => ({ status: 200, body: fixture() })),
    /different release/,
  );
  await assert.rejects(
    readRelease("v3.4.5", async () => ({ status: 404 })),
    /HTTP 404/,
  );
});

test("commits only moo.json to the chosen branch with the current file SHA", async () => {
  const calls = [];
  const index = generatedIndex();
  const result = await publishMooIndex(
    index,
    "main",
    async (endpoint, options) => {
      calls.push({ endpoint, options });
      if (!options) return remoteFile(previousIndex("3.4.4"));
      assert.equal(endpoint, "contents/moo.json");
      assert.equal(options.method, "PUT");
      assert.equal(options.body.branch, "main");
      assert.equal(options.body.sha, "previous-file-sha");
      assert.equal(options.body.message, "chore: update Moo index for v3.4.5");
      assert.deepEqual(
        JSON.parse(Buffer.from(options.body.content, "base64").toString()),
        mergeMooIndex(index, previousIndex("3.4.4")).index,
      );
      return {
        status: 200,
        body: { commit: { html_url: "https://github.com/example/commit" } },
      };
    },
  );
  assert.equal(calls[0].endpoint, "contents/moo.json?ref=main");
  assert.equal(calls.length, 2);
  assert.equal(result.status, "updated");
});

test("creates a missing index without passing an obsolete SHA", async () => {
  const result = await publishMooIndex(
    generatedIndex(),
    "main",
    async (_endpoint, options) => {
      if (!options) return { status: 404 };
      assert.equal(options.body.sha, undefined);
      return { status: 201, body: {} };
    },
  );
  assert.equal(result.status, "updated");
});

test("does not commit identical content or downgrade a newer index", async () => {
  for (const [remote, expected] of [
    [generatedIndex(), "unchanged"],
    [previousIndex("3.10.0"), "skipped-newer-version"],
    [previousIndex("3.4.10"), "skipped-newer-version"],
  ]) {
    const result = await publishMooIndex(
      generatedIndex(),
      "main",
      async (_endpoint, options) => {
        assert.equal(options, undefined, "must not submit a commit");
        return remoteFile(remote);
      },
    );
    assert.equal(result.status, expected);
  }
});

test("re-reads the file SHA after a concurrent write before retrying", async () => {
  let reads = 0;
  let writes = 0;
  const result = await publishMooIndex(
    generatedIndex(),
    "main",
    async (_endpoint, options) => {
      if (!options) return remoteFile(previousIndex("3.4.4"), `sha-${++reads}`);
      assert.equal(options.body.sha, `sha-${reads}`);
      return ++writes === 1
        ? { status: 409, body: {} }
        : { status: 200, body: {} };
    },
  );
  assert.equal(result.status, "updated");
  assert.equal(reads, 2);
  assert.equal(writes, 2);
});

test("a conflict followed by a newer publication never retries the old commit", async () => {
  let reads = 0;
  let writes = 0;
  const result = await publishMooIndex(
    generatedIndex(),
    "main",
    async (_endpoint, options) => {
      if (!options)
        return remoteFile(previousIndex(++reads === 1 ? "3.4.4" : "3.4.6"));
      writes++;
      return { status: 409, body: {} };
    },
  );
  assert.equal(result.status, "skipped-newer-version");
  assert.equal(writes, 1);
});

test("surfaces branch permission failures and limits conflict retries", async () => {
  await assert.rejects(
    publishMooIndex(generatedIndex(), "main", async (_endpoint, options) => {
      if (!options) return remoteFile(previousIndex("3.4.4"));
      return { status: 403, body: { message: "Protected branch" } };
    }),
    /HTTP 403: Protected branch/,
  );
  let writes = 0;
  await assert.rejects(
    publishMooIndex(generatedIndex(), "main", async (_endpoint, options) => {
      if (!options) return remoteFile(previousIndex("3.4.4"));
      writes++;
      return { status: 409, body: {} };
    }),
    /three concurrent-write conflicts/,
  );
  assert.equal(writes, 3);
});

test("refuses to overwrite malformed remote indexes", async () => {
  for (const content of ["not-json", '{"schema_version":"moo","apps":{}}']) {
    await assert.rejects(
      publishMooIndex(generatedIndex(), "main", async (_endpoint, options) => {
        assert.equal(options, undefined);
        const response = remoteFile({});
        response.body.content = Buffer.from(content).toString("base64");
        return response;
      }),
    );
  }
});

test("rejects same-version checksum changes on either architecture before writing", async () => {
  for (const arch of ["x86", "arm"]) {
    const previous = generatedIndex();
    previous.apps["fn-knock"].releases["3.4.5"].packages[arch].sha256 =
      "a".repeat(64);
    await assert.rejects(
      publishMooIndex(generatedIndex(), "main", async (_endpoint, options) => {
        assert.equal(options, undefined, "must not submit a commit");
        return remoteFile(previous);
      }),
      new RegExp(`v3\\.4\\.5 ${arch}: SHA-256 changed.*--allow-republish`),
    );
  }
});

test("allows an explicit republish and metadata-only updates", async () => {
  const previous = generatedIndex();
  previous.apps["fn-knock"].releases["3.4.5"].packages.x86.sha256 = "a".repeat(
    64,
  );
  const result = await publishMooIndex(
    generatedIndex(),
    "main",
    async (_endpoint, options) => {
      if (!options) return remoteFile(previous);
      const written = JSON.parse(Buffer.from(options.body.content, "base64"));
      assert.equal(
        written.apps["fn-knock"].releases["3.4.5"].packages.x86.sha256,
        "1".repeat(64),
      );
      return { status: 200, body: {} };
    },
    { allowRepublish: true },
  );
  assert.equal(result.status, "updated");
  const changedNotes = generatedIndex();
  changedNotes.apps["fn-knock"].releases["3.4.5"].changelog = "Corrected notes";
  assert.equal(
    mergeMooIndex(changedNotes, generatedIndex()).index.apps["fn-knock"]
      .releases["3.4.5"].changelog,
    "Corrected notes",
  );
});

test("retains the newest versions numerically and preserves unrelated fields", () => {
  const previous = previousIndex("3.4.4");
  const app = previous.apps["fn-knock"];
  for (const v of ["3.3.9", "3.3.10", "3.2.0"])
    app.releases[v] = structuredClone(app.releases["3.4.4"]);
  previous.custom = "source extension";
  previous.source_info.custom = "source info extension";
  previous.apps.other = { display_name: "Another app" };
  app.custom = "app extension";
  app.releases["3.4.5"] = generatedIndex().apps["fn-knock"].releases["3.4.5"];
  app.releases["3.4.5"].custom = "release extension";
  app.releases["3.4.5"].packages.x86.custom = "package extension";
  const result = mergeMooIndex(generatedIndex(), previous).index;
  assert.deepEqual(Object.keys(result.apps["fn-knock"].releases), [
    "3.4.5",
    "3.4.4",
  ]);
  assert.deepEqual(
    result.apps["fn-knock"].releases["3.4.4"],
    app.releases["3.4.4"],
  );
  assert.deepEqual(result.apps.other, previous.apps.other);
  assert.equal(result.custom, previous.custom);
  assert.equal(result.source_info.custom, previous.source_info.custom);
  assert.equal(result.apps["fn-knock"].custom, app.custom);
  assert.equal(
    result.apps["fn-knock"].releases["3.4.5"].custom,
    "release extension",
  );
  assert.equal(
    result.apps["fn-knock"].releases["3.4.5"].packages.x86.custom,
    "package extension",
  );
  for (const [keep, expected] of [
    [1, ["3.4.5"]],
    [3, ["3.4.5", "3.4.4", "3.3.10"]],
    [0, ["3.4.5", "3.4.4", "3.3.10", "3.3.9", "3.2.0"]],
  ])
    assert.deepEqual(
      Object.keys(
        mergeMooIndex(generatedIndex(), previous, { keep }).index.apps[
          "fn-knock"
        ].releases,
      ),
      expected,
    );
  for (const keep of [-1, 1.5, NaN, Infinity, "2", Number.MAX_SAFE_INTEGER + 1])
    assert.throws(
      () => mergeMooIndex(generatedIndex(), previous, { keep }),
      /--keep/,
    );
});

test("a repeat publication with history is unchanged", async () => {
  const previous = mergeMooIndex(
    generatedIndex(),
    previousIndex("3.4.4"),
  ).index;
  const result = await publishMooIndex(
    generatedIndex(),
    "main",
    async (_endpoint, options) => {
      assert.equal(options, undefined);
      return remoteFile(previous);
    },
  );
  assert.equal(result.status, "unchanged");
});

test("conflict retries merge fresh history and recheck republished checksums", async () => {
  for (const changedChecksum of [false, true]) {
    let reads = 0;
    let writes = 0;
    const refreshed = previousIndex("3.4.3");
    if (changedChecksum) {
      refreshed.apps["fn-knock"].releases["3.4.5"] =
        generatedIndex().apps["fn-knock"].releases["3.4.5"];
      refreshed.apps["fn-knock"].releases["3.4.5"].packages.arm.sha256 =
        "a".repeat(64);
    }
    const operation = publishMooIndex(
      generatedIndex(),
      "main",
      async (_endpoint, options) => {
        if (!options)
          return remoteFile(++reads === 1 ? previousIndex("3.4.4") : refreshed);
        if (++writes === 1) return { status: 409, body: {} };
        const written = JSON.parse(Buffer.from(options.body.content, "base64"));
        assert.deepEqual(Object.keys(written.apps["fn-knock"].releases), [
          "3.4.5",
          "3.4.3",
        ]);
        return { status: 200, body: {} };
      },
    );
    if (changedChecksum) {
      await assert.rejects(operation, /SHA-256 changed/);
      assert.equal(writes, 1);
    } else {
      assert.equal((await operation).status, "updated");
      assert.equal(writes, 2);
    }
  }
});

test("local updates preserve history and leave files intact on rejection or downgrade", async (t) => {
  const dir = await mkdtemp(join(tmpdir(), "moo-index-test-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const output = join(dir, "moo.json");
  assert.equal(
    (await updateMooIndexFile(previousIndex("3.4.4"), output)).status,
    "updated",
  );
  assert.equal(
    (await updateMooIndexFile(generatedIndex(), output)).status,
    "updated",
  );
  const original = await readFile(output, "utf8");
  assert.deepEqual(
    Object.keys(JSON.parse(original).apps["fn-knock"].releases),
    ["3.4.5", "3.4.4"],
  );
  assert.equal(
    (await updateMooIndexFile(generatedIndex(), output)).status,
    "unchanged",
  );
  assert.equal(
    (await updateMooIndexFile(previousIndex("3.4.3"), output)).status,
    "skipped-newer-version",
  );
  const republish = generatedIndex();
  republish.apps["fn-knock"].releases["3.4.5"].packages.arm.sha256 = "b".repeat(
    64,
  );
  await assert.rejects(
    updateMooIndexFile(republish, output),
    /SHA-256 changed/,
  );
  assert.equal(await readFile(output, "utf8"), original);
  await updateMooIndexFile(republish, output, {
    allowRepublish: true,
    keep: 1,
  });
  const replaced = JSON.parse(await readFile(output, "utf8"));
  assert.deepEqual(Object.keys(replaced.apps["fn-knock"].releases), ["3.4.5"]);
  assert.equal(
    replaced.apps["fn-knock"].releases["3.4.5"].packages.arm.sha256,
    "b".repeat(64),
  );
  for (const malformed of [
    "not-json",
    "null",
    '{"schema_version":"moo","apps":{}}',
  ]) {
    await writeFile(output, malformed);
    await assert.rejects(updateMooIndexFile(generatedIndex(), output));
    assert.equal(await readFile(output, "utf8"), malformed);
  }
});

test("CLI rejects invalid retention arguments before reading a release", () => {
  for (const keep of ["-1", "1.5", "", "abc", "9007199254740992"]) {
    const result = spawnSync(
      process.execPath,
      [
        new URL("../update-moo-index.mjs", import.meta.url).pathname,
        `--keep=${keep}`,
        "/nonexistent-release.json",
      ],
      { encoding: "utf8" },
    );
    assert.equal(result.status, 1);
    assert.match(result.stderr, /--keep must be a nonnegative safe integer/);
  }
});

test("CLI applies retention and explicit republish options to local files", async (t) => {
  const dir = await realpath(await mkdtemp(join(tmpdir(), "moo-cli-test-")));
  t.after(() => rm(dir, { recursive: true, force: true }));
  await mkdir(join(dir, "scripts"));
  await mkdir(join(dir, "apps/fn-knock"), { recursive: true });
  const script = join(dir, "scripts/update-moo-index.mjs");
  await writeFile(
    script,
    await readFile(new URL("../update-moo-index.mjs", import.meta.url)),
  );
  await writeFile(join(dir, "apps/fn-knock/manifest"), manifest);
  const release = join(dir, "release.json");
  const output = join(dir, "moo.json");
  await writeFile(release, JSON.stringify(fixture()));
  await writeFile(output, JSON.stringify(previousIndex("3.4.4")));
  const run = (...args) =>
    spawnSync(process.execPath, [script, release, ...args], {
      encoding: "utf8",
    });
  assert.equal(run().status, 0);
  const original = await readFile(output, "utf8");
  assert.deepEqual(
    Object.keys(JSON.parse(original).apps["fn-knock"].releases),
    ["3.4.5", "3.4.4"],
  );
  const replacement = fixture();
  replacement.assets[0].digest = `sha256:${"a".repeat(64)}`;
  await writeFile(release, JSON.stringify(replacement));
  assert.equal(run().status, 1);
  assert.equal(await readFile(output, "utf8"), original);
  assert.equal(run("--allow-republish", "--keep", "1").status, 0);
  const updated = JSON.parse(await readFile(output, "utf8"));
  assert.deepEqual(Object.keys(updated.apps["fn-knock"].releases), ["3.4.5"]);
  assert.equal(
    updated.apps["fn-knock"].releases["3.4.5"].packages.x86.sha256,
    "a".repeat(64),
  );
});
