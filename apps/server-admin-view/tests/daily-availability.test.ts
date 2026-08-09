/// <reference types="node" />

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import {
  isAvailabilityWindowOpen,
  normalizeDailyAvailability,
} from "../src/lib/daily-availability";

describe("daily availability", () => {
  it("normalizes valid windows and safely clears invalid legacy values", () => {
    assert.deepEqual(
      normalizeDailyAvailability({
        enabled: true,
        start_time: " 22:00 ",
        end_time: "06:00",
      }),
      { enabled: true, start_time: "22:00", end_time: "06:00" },
    );
    assert.equal(
      normalizeDailyAvailability({
        enabled: true,
        start_time: "9:00",
        end_time: "18:00",
      }),
      null,
    );
  });

  it("evaluates the window in the server timezone", () => {
    const availability = {
      enabled: true,
      start_time: "09:00",
      end_time: "18:00",
    };
    const now = new Date("2026-01-01T02:00:00.000Z");
    assert.equal(
      isAvailabilityWindowOpen(availability, now, "Asia/Shanghai"),
      true,
    );
    assert.equal(
      isAvailabilityWindowOpen(availability, now, "America/New_York"),
      false,
    );
  });

  it("keeps start inclusive and end exclusive in the server timezone", () => {
    const availability = {
      enabled: true as const,
      start_time: "22:00",
      end_time: "06:00",
    };
    assert.equal(
      isAvailabilityWindowOpen(
        availability,
        new Date("2026-01-01T14:00:00.000Z"),
        "Asia/Shanghai",
      ),
      true,
    );
    assert.equal(
      isAvailabilityWindowOpen(
        availability,
        new Date("2026-01-01T22:00:00.000Z"),
        "Asia/Shanghai",
      ),
      false,
    );
  });

  it("applies the normalized API response before forcing a config refresh", () => {
    const source = readFileSync(
      new URL(
        "../src/views/stream-mappings/useStreamMappingAvailability.ts",
        import.meta.url,
      ),
      "utf8",
    );
    assert.match(source, /protocol_mapping_feature:\s*updated/u);
    assert.match(source, /loadConfig\(\{ force: true \}\)/u);
    assert.match(source, /if \(!systemClockStore\.status\)/u);
  });
});
