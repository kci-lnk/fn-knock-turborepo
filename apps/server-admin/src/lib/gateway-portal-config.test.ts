import assert from "node:assert/strict";
import test from "node:test";

import { normalizeGatewayPortalConfigValue } from "./gateway-portal-config";

test("gateway portal config defaults icon drag mode to corners", () => {
  assert.equal(normalizeGatewayPortalConfigValue({}).icon_drag_mode, "corners");
});

test("gateway portal config preserves free icon drag mode", () => {
  assert.equal(
    normalizeGatewayPortalConfigValue({ icon_drag_mode: "free" })
      .icon_drag_mode,
    "free",
  );
});

test("gateway portal config normalizes invalid icon drag mode to corners", () => {
  assert.equal(
    normalizeGatewayPortalConfigValue({
      icon_drag_mode: "somewhere" as never,
    }).icon_drag_mode,
    "corners",
  );
});
