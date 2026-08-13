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
  const pageSource = readSource(
    "../src/views/system-settings/GatewayPortalSettings.vue",
  );
  const panelSource = readSource(
    "../src/views/system-settings/gateway-portal/GatewayPortalSettingsPanel.vue",
  );
  const choiceSource = readSource(
    "../src/views/system-settings/gateway-portal/GatewayPortalChoiceSetting.vue",
  );
  const controllerSource = readSource(
    "../src/views/system-settings/gateway-portal/useGatewayPortalSettings.ts",
  );

  assert.match(pageSource, /useGatewayPortalSettings/u);
  assert.match(pageSource, /GatewayPortalSettingsPanel/u);
  assert.doesNotMatch(pageSource, /ConfigAPI/u);
  assert.match(panelSource, /value: "v1"/u);
  assert.match(panelSource, /value: "v2"/u);
  assert.match(choiceSource, /:aria-pressed="modelValue === option\.value"/u);
  assert.match(
    choiceSource,
    /bg-foreground text-background hover:bg-foreground\/90 hover:text-background/u,
  );
  assert.match(
    controllerSource,
    /ConfigAPI\.updateGatewaySettings\(buildGatewayPortalVersionPatch\(version\)\)/u,
  );
  assert.match(
    controllerSource,
    /if \(!\(await applySavedSettings\(data\)\)\) applyPortal\(previous\);/u,
  );
  assert.match(
    controllerSource,
    /const data = await ConfigAPI\.getGatewaySettings\(\);\s*applyPortal\(data\.portal\);/u,
  );
});
