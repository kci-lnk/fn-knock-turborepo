import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  isPowDifficultyValid,
  POW_DIFFICULTY_MAX,
  POW_DIFFICULTY_MIN,
} from "../src/lib/captcha-settings";

const captchaSettingsSource = readFileSync(
  new URL("../src/views/system-settings/CaptchaSettings.vue", import.meta.url),
  "utf8",
);

describe("captcha settings", () => {
  it("validates both PoW difficulty tiers and their ordering", () => {
    assert.equal(isPowDifficultyValid(100_000, 300_000), true);
    assert.equal(
      isPowDifficultyValid(POW_DIFFICULTY_MIN, POW_DIFFICULTY_MAX),
      true,
    );
    assert.equal(isPowDifficultyValid(9_999, 300_000), false);
    assert.equal(isPowDifficultyValid(15_000, 300_000), false);
    assert.equal(isPowDifficultyValid(100_000.5, 300_000), false);
    assert.equal(isPowDifficultyValid(400_000, 300_000), false);
    assert.equal(isPowDifficultyValid(100_000, 305_000), false);
    assert.equal(isPowDifficultyValid(100_000, 1_000_001), false);
  });

  it("renders both difficulty inputs and the uncommon-location switch", () => {
    assert.match(captchaSettingsSource, /form\.pow\.base_max_number/);
    assert.match(
      captchaSettingsSource,
      /form\.pow\.uncommon_location\.max_number/,
    );
    assert.match(
      captchaSettingsSource,
      /form\.pow\.uncommon_location\.enabled/,
    );
    assert.match(captchaSettingsSource, /POW_DIFFICULTY_STEP/);
  });
});
