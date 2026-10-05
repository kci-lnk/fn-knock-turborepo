import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  buildMooIndex,
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
        index,
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
