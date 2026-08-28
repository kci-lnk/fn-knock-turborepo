/// <reference types="node" />

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { describe, it } from "node:test";

describe("FNOS network tuning visibility", () => {
  it("keeps BBR visible and disables it with a reason when support is unavailable", async () => {
    const source = await readFile(
      new URL("../src/views/system-settings/FnosSettings.vue", import.meta.url),
      "utf8",
    );

    assert.match(source, /v-if="canUseFnosNetworkTuning"/u);
    assert.doesNotMatch(
      source,
      /v-if="canUseFnosNetworkTuning && isBbrSupported"/u,
    );
    assert.match(
      source,
      /!isNetworkTuningAvailable \|\|\s*!isBbrSupported \|\|\s*isNetworkTuningSaving/u,
    );
    assert.match(source, /networkTuningStatus\.bbr\.supported/u);
    assert.match(source, /bbrSupportDescription/u);
  });
});
