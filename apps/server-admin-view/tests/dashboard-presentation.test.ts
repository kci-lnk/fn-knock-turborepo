import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { DASHBOARD_TRAFFIC_COLORS } from "../src/views/dashboard/useDashboardViewModel";

describe("dashboard presentation", () => {
  it("uses distinct cool and warm colors for inbound and outbound traffic", () => {
    assert.deepEqual(DASHBOARD_TRAFFIC_COLORS, {
      ingress: "#0f766e",
      egress: "#c2410c",
    });
  });
});
