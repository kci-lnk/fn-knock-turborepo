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
const powSettingsSource = readFileSync(
  new URL(
    "../src/views/system-settings/captcha/PowCaptchaSettingsFields.vue",
    import.meta.url,
  ),
  "utf8",
);
const fieldSource = readFileSync(
  new URL(
    "../src/views/system-settings/captcha/CaptchaConfigField.vue",
    import.meta.url,
  ),
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
    assert.match(captchaSettingsSource, /PowCaptchaSettingsFields/);
    assert.match(captchaSettingsSource, /v-model="form\.pow"/);
    assert.match(powSettingsSource, /v-model="baseDifficultySelection"/);
    assert.match(powSettingsSource, /v-model="uncommonDifficultySelection"/);
    assert.match(powSettingsSource, /model\.uncommon_location\.enabled/);
    assert.match(
      powSettingsSource,
      /v-if="model\.uncommon_location\.enabled"/,
    );
    assert.match(powSettingsSource, /control-class="md:w-\[300px\]"/);
    assert.match(fieldSource, /md:grid-cols-\[320px_minmax\(0,1fr\)\]/);
    assert.doesNotMatch(powSettingsSource, /\(\{\{ POW_DIFFICULTY_/);
    assert.doesNotMatch(
      powSettingsSource,
      /v-model\.number="model\.(?:base_max_number|uncommon_location\.max_number)"/,
    );
  });
});
