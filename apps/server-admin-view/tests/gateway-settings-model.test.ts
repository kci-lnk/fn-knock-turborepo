import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { DEFAULT_REVERSE_PROXY_THROTTLE } from "../src/views/system-settings/gatewaySettingsModel";

describe("gateway settings defaults", () => {
  it("uses the expanded reverse proxy token bucket", () => {
    assert.deepEqual(DEFAULT_REVERSE_PROXY_THROTTLE, {
      enabled: true,
      requests_per_second: 500,
      burst: 1000,
      block_seconds: 30,
    });
  });
});
