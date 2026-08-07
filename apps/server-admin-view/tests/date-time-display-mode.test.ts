import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  applyDateTimeDisplayConfig,
  applyDateTimeDisplayMode,
  normalizeDateTimeDisplayMode,
  useDateTimeDisplayState,
} from "../../../packages/admin-shared/src/composables/useDateTimeDisplayState";

const readSource = (path: string) =>
  readFileSync(new URL(path, import.meta.url), "utf8");

test("date-time display mode defaults invalid and legacy values to human-friendly", () => {
  assert.equal(normalizeDateTimeDisplayMode(), "human_friendly");
  assert.equal(normalizeDateTimeDisplayMode("invalid"), "human_friendly");
  assert.equal(normalizeDateTimeDisplayMode("full"), "full");
});

test("date-time display config updates shared reactive state", () => {
  const { dateTimeDisplayMode } = useDateTimeDisplayState();

  applyDateTimeDisplayConfig({ date_time_display_mode: "full" });
  assert.equal(dateTimeDisplayMode.value, "full");

  applyDateTimeDisplayConfig(null);
  assert.equal(dateTimeDisplayMode.value, "human_friendly");

  applyDateTimeDisplayMode("human_friendly");
});

test("HumanFriendlyTime shows the opposite format in its default tooltip", () => {
  const source = readSource(
    "../../../packages/admin-shared/src/components/common/HumanFriendlyTime.vue",
  );

  assert.match(source, /dateTimeDisplayMode\.value === "full"/u);
  assert.match(source, /\? fullText\.value\s*:\s*humanFriendlyText\.value/u);
  assert.match(
    source,
    /dateTimeDisplayMode\.value === "full"\s*\? humanFriendlyText\.value\s*:\s*fullText\.value/u,
  );
  assert.match(
    source,
    /displayMode === "full" && customTooltipLineCount > 0/u,
  );
  assert.match(source, /stopTimer\(\)/u);
  assert.match(source, /customTooltipLines\.value\.length > 0/u);
});

test("features settings exposes an auto-saving segmented selector", () => {
  const viewSource = readSource(
    "../src/views/system-settings/FeaturesSettings.vue",
  );
  const rowSource = readSource(
    "../src/views/system-settings/DateTimeDisplaySettingRow.vue",
  );
  const controllerSource = readSource(
    "../src/views/system-settings/useFeaturesSettings.ts",
  );

  assert.match(viewSource, /<DateTimeDisplaySettingRow/u);
  assert.match(viewSource, /@change="saveDateTimeDisplayMode"/u);
  assert.match(rowSource, /role="group"/u);
  assert.match(rowSource, /selectMode\('human_friendly'\)/u);
  assert.match(rowSource, /selectMode\('full'\)/u);
  assert.match(
    controllerSource,
    /date_time_display_mode:\s*nextValue/u,
  );
  assert.match(controllerSource, /applyDateTimeDisplayMode\(previousValue\)/u);
});
