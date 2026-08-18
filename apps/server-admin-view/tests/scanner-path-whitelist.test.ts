import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  normalizeScannerWhitelistPath,
  validateScannerWhitelistEntries,
} from "../src/views/system-settings/scanner-path-whitelist/scannerPathWhitelistModel";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

describe("scanner path whitelist model", () => {
  it("normalizes query strings, fragments, whitespace, and trailing slashes", () => {
    assert.equal(
      normalizeScannerWhitelistPath(" /app/path/?source=test#section "),
      "/app/path",
    );
    assert.equal(normalizeScannerWhitelistPath("/"), "/");
    assert.equal(normalizeScannerWhitelistPath("   "), "");
  });

  it("validates absolute paths and duplicates after normalization", () => {
    const errors = validateScannerWhitelistEntries([
      { id: 1, value: "/app/path/" },
      { id: 2, value: "/app/path?source=test" },
      { id: 3, value: "relative" },
      { id: 4, value: "" },
      { id: 5, value: "/bad\npath" },
      { id: 6, value: "/trailing-control\n" },
      { id: 7, value: "/unicode-control\u0085path" },
    ]);
    assert.equal(errors.get(1), "duplicate");
    assert.equal(errors.get(2), "duplicate");
    assert.equal(errors.get(3), "absolute");
    assert.equal(errors.get(4), "required");
    assert.equal(errors.get(5), "controlCharacters");
    assert.equal(errors.get(6), "controlCharacters");
    assert.equal(errors.get(7), "controlCharacters");
  });

  it("keeps path matching case-sensitive", () => {
    assert.notEqual(
      normalizeScannerWhitelistPath("/CaseSensitive"),
      normalizeScannerWhitelistPath("/casesensitive"),
    );
  });
});

describe("scanner path whitelist UI boundaries", () => {
  it("keeps the route, settings entry, breadcrumb, and draft dock wired", () => {
    const router = readSource("../src/router/index.ts");
    const firewall = readSource(
      "../src/views/system-settings/ScannerFirewallSettings.vue",
    );
    const page = readSource(
      "../src/views/system-settings/ScannerPathWhitelistSettings.vue",
    );

    assert.match(router, /path: "system\/scanner-path-whitelist"/u);
    assert.match(firewall, /ScannerPathWhitelistEntry/u);
    assert.match(page, /useScannerPathWhitelistSettings/u);
    assert.match(page, /BreadcrumbSeparator/u);
    assert.match(page, /FloatingActionDock/u);
  });

  it("keeps false-positive behavior in a hook and the shared table generic", () => {
    const dialog = readSource(
      "../src/views/session-management/IpBlacklistDetailDialog.vue",
    );
    const controller = readSource(
      "../src/views/session-management/useIpBlacklistPage.ts",
    );
    const sharedTable = readSource(
      "../../../packages/admin-shared/src/components/session/BlacklistHitsTable.vue",
    );

    assert.match(dialog, /#action="\{ row \}"/u);
    assert.match(controller, /useScannerFalsePositive/u);
    assert.match(sharedTable, /<slot name="action" :row="row" \/>/u);
    assert.doesNotMatch(sharedTable, /falsePositive|ScannerAPI/u);
  });
});
