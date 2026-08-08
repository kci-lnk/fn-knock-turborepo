import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  buildGatewayPortalVersionPatch,
  normalizeGatewayPortalConfig,
  normalizeGatewayPortalVersion,
} from "../src/lib/gatewayPortal";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

test("gateway portal version defaults invalid and legacy values to v1", () => {
  assert.equal(normalizeGatewayPortalVersion(), "v1");
  assert.equal(normalizeGatewayPortalVersion(""), "v1");
  assert.equal(normalizeGatewayPortalVersion("future"), "v1");
  assert.equal(normalizeGatewayPortalVersion("v2"), "v2");
  assert.equal(normalizeGatewayPortalConfig().version, "v1");
  assert.equal(normalizeGatewayPortalConfig({ version: "v2" }).version, "v2");
  assert.equal(normalizeGatewayPortalConfig().show_wol, true);
  assert.equal(normalizeGatewayPortalConfig({ show_wol: false }).show_wol, false);
});

test("gateway portal version builds a partial immediate-save patch", () => {
  assert.deepEqual(buildGatewayPortalVersionPatch("v2"), {
    portal: { version: "v2" },
  });
});

test("gateway portal settings exposes v1 and v2 state and rolls back failed saves", () => {
  const source = readSource(
    "../src/views/system-settings/GatewayPortalSettings.vue",
  );

  assert.match(source, /:aria-pressed="form\.version === 'v1'"/u);
  assert.match(source, /:aria-pressed="form\.version === 'v2'"/u);
  assert.match(
    source,
    /bg-foreground text-background hover:bg-foreground\/90 hover:text-background/u,
  );
  assert.match(
    source,
    /ConfigAPI\.updateGatewaySettings\(buildGatewayPortalVersionPatch\(version\)\)/u,
  );
  assert.match(
    source,
    /if \(!\(await applySavedSettings\(data\)\)\) \{\s*applyPortal\(previous\);\s*\}/u,
  );
  assert.match(
    source,
    /const data = await ConfigAPI\.getGatewaySettings\(\);\s*applyPortal\(data\.portal\);/u,
  );
});
