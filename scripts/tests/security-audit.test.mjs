import assert from "node:assert/strict";
import test from "node:test";
import { assertExactFindings, rustFindings } from "../security-audit.mjs";
import {
  goFindings,
  parseJsonStream,
  validateGoBuildInfo,
} from "../security-audit-go.mjs";

test("Rust scanner errors and incomplete reports fail closed", () => {
  for (const report of [
    {},
    { error: "database unavailable" },
    { vulnerabilities: {} },
  ]) {
    assert.throws(() => rustFindings(report));
  }
  assert.deepEqual(
    rustFindings({ vulnerabilities: { list: [] }, warnings: {} }),
    { vulnerabilities: [], warnings: [] },
  );
});

test("RSA exceptions accept only exact findings and require review when removed", () => {
  const accepted = new Set(["RUSTSEC-2023-0071:rsa:0.9.10"]);
  assertExactFindings([...accepted], accepted, "test");
  for (const findings of [
    [],
    ["RUSTSEC-2023-0071:rsa:0.9.11"],
    [...accepted, "NEW-CVE:other:1.0.0"],
    [...accepted, ...accepted],
  ]) {
    assert.throws(() => assertExactFindings(findings, accepted, "test"));
  }
});

test("Go JSON streams handle quoted braces and reject truncation or tool errors", () => {
  const objects = [
    { message: 'quoted \\" } { text' },
    { config: { mode: "source" } },
  ];
  assert.deepEqual(
    parseJsonStream(objects.map((o) => JSON.stringify(o, null, 2)).join("\n")),
    objects,
  );
  for (const text of ["", "{}\n{", "tool failed", '{"error": "unfinished'])
    assert.throws(() => parseJsonStream(text));
});

function messages(mode, finding) {
  return [
    { config: { scanner_name: "govulncheck", scan_mode: mode } },
    { SBOM: { modules: [] } },
    { finding },
  ];
}
const openpgp = {
  osv: "GO-2026-5932",
  trace: [
    {
      module: "golang.org/x/crypto",
      version: "v0.57.0",
      package: "golang.org/x/crypto/openpgp",
      function: "*",
    },
  ],
};

test("binary OpenPGP finding requires positive target-specific absence evidence", () => {
  const absent = new Set(["net/http", "crypto/tls"]);
  assert.deepEqual(
    goFindings(messages("binary", openpgp), "binary", absent).notImported,
    [openpgp.osv],
  );
  for (const imports of [
    undefined,
    new Set(),
    new Set(["golang.org/x/crypto/openpgp/armor"]),
  ]) {
    assert.deepEqual(
      goFindings(messages("binary", openpgp), "binary", imports).rejected,
      [openpgp.osv],
    );
  }
  for (const finding of [
    { ...openpgp, osv: "NEW-CVE" },
    { ...openpgp, trace: [{ ...openpgp.trace[0], version: "v0.58.0" }] },
    {
      ...openpgp,
      trace: [{ ...openpgp.trace[0], package: "golang.org/x/crypto/ssh" }],
    },
    { ...openpgp, trace: [{ ...openpgp.trace[0], function: "ReadMessage" }] },
  ]) {
    assert.equal(
      goFindings(messages("binary", finding), "binary", absent).rejected.length,
      1,
    );
  }
});

test("source reachability and scanner failure are distinct", () => {
  assert.equal(
    goFindings(messages("source", openpgp), "source").rejected.length,
    1,
  );
  const unused = {
    osv: openpgp.osv,
    trace: [{ module: "golang.org/x/crypto", version: "v0.57.0" }],
  };
  assert.deepEqual(
    goFindings(messages("source", unused), "source").rejected,
    [],
  );
  assert.throws(() => goFindings([], "source"));
  assert.throws(() => goFindings(messages("source", {}), "source"));
});

test("artifact validation rejects stale, dirty, mismatched toolchain and dependency builds", () => {
  const modules = [
    { Path: "golang.org/x/net", Version: "v0.60.0", Sum: "checksum" },
  ];
  const info = {
    Path: "go-reauth-proxy/cmd/server",
    Main: { Path: "go-reauth-proxy" },
    GoVersion: "go1.26.10",
    Deps: modules,
    Settings: Object.entries({
      "vcs.revision": "selected",
      "vcs.modified": "false",
      GOOS: "linux",
      GOARCH: "amd64",
      CGO_ENABLED: "0",
    }).map(([Key, Value]) => ({ Key, Value })),
  };
  validateGoBuildInfo(info, "selected", "go1.26.10", modules);
  assert.throws(() => validateGoBuildInfo(info, "other", "go1.26.10", modules));
  assert.throws(() =>
    validateGoBuildInfo(info, "selected", "go1.26.7", modules),
  );
  assert.throws(() =>
    validateGoBuildInfo(
      { ...info, Deps: [{ ...modules[0], Version: "v0.58.0" }] },
      "selected",
      "go1.26.10",
      modules,
    ),
  );
  assert.throws(() =>
    validateGoBuildInfo(
      { ...info, Deps: [{ ...modules[0], Replace: { Path: "./unreviewed" } }] },
      "selected",
      "go1.26.10",
      modules,
    ),
  );
  const dirty = structuredClone(info);
  dirty.Settings.find((s) => s.Key === "vcs.modified").Value = "true";
  assert.throws(() =>
    validateGoBuildInfo(dirty, "selected", "go1.26.10", modules),
  );
  const tagged = structuredClone(info);
  tagged.Settings.push({ Key: "-tags", Value: "custom" });
  assert.throws(() =>
    validateGoBuildInfo(tagged, "selected", "go1.26.10", modules),
  );
});
