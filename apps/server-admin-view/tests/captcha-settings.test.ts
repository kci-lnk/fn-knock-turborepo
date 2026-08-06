import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  ensureUncommonDifficultyAtLeastBase,
  isPowDifficultyPreset,
  isPowDifficultyValid,
  POW_DIFFICULTY_MAX,
  POW_DIFFICULTY_MIN,
  POW_DIFFICULTY_STANDARD,
  POW_DIFFICULTY_VERY_HARD,
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

  it("defines standard and very-hard presets", () => {
    assert.equal(POW_DIFFICULTY_STANDARD, 100_000);
    assert.equal(POW_DIFFICULTY_VERY_HARD, 300_000);
    assert.equal(isPowDifficultyPreset(POW_DIFFICULTY_STANDARD), true);
    assert.equal(isPowDifficultyPreset(POW_DIFFICULTY_VERY_HARD), true);
    assert.equal(isPowDifficultyPreset(200_000), false);
    assert.equal(ensureUncommonDifficultyAtLeastBase(300_000, 100_000), 300_000);
    assert.equal(ensureUncommonDifficultyAtLeastBase(100_000, 300_000), 300_000);
  });

  it("renders difficulty selects and hides the uncommon tier when disabled", () => {
    assert.match(captchaSettingsSource, /v-model="baseDifficultySelection"/);
    assert.match(captchaSettingsSource, /v-model="uncommonDifficultySelection"/);
    assert.match(
      captchaSettingsSource,
      /form\.pow\.uncommon_location\.enabled/,
    );
    assert.match(
      captchaSettingsSource,
      /v-if="form\.pow\.uncommon_location\.enabled"/,
    );
    assert.match(captchaSettingsSource, /captcha-difficulty-select-wrap/);
    assert.match(captchaSettingsSource, /width: min\(100%, 300px\)/);
    assert.doesNotMatch(captchaSettingsSource, /\(\{\{ POW_DIFFICULTY_/);
    assert.doesNotMatch(
      captchaSettingsSource,
      /v-model\.number="form\.pow\.(?:base_max_number|uncommon_location\.max_number)"/,
    );
  });
});
