/// <reference types="node" />

import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  getAvailabilityWindowValidationError,
  getHostMappingAvailabilityState,
  isAvailabilityWindowOpen,
  isAvailabilityWindowValid,
  parseAvailabilityTimeToMinutes,
} from "../src/lib/host-mapping-availability";

const at = (hour: number, minute = 0) => new Date(2026, 0, 1, hour, minute);

describe("subdomain availability helpers", () => {
  it("validates HH:mm windows and rejects equal times", () => {
    assert.equal(parseAvailabilityTimeToMinutes("09:30"), 570);
    assert.equal(parseAvailabilityTimeToMinutes("24:00"), null);
    assert.equal(parseAvailabilityTimeToMinutes("9:00"), null);
    assert.equal(isAvailabilityWindowValid("09:00", "18:00"), true);
    assert.equal(isAvailabilityWindowValid("22:00", "06:00"), true);
    assert.equal(isAvailabilityWindowValid("09:00", "09:00"), false);
    assert.equal(
      getAvailabilityWindowValidationError("09:00", "09:00"),
      "same_time",
    );
    assert.equal(
      getAvailabilityWindowValidationError("9:00", "18:00"),
      "invalid_time",
    );
  });

  it("opens normal same-day windows inclusively at start and exclusively at end", () => {
    const availability = {
      enabled: true,
      start_time: "09:00",
      end_time: "18:00",
    };
    assert.equal(isAvailabilityWindowOpen(availability, at(9)), true);
    assert.equal(isAvailabilityWindowOpen(availability, at(17, 59)), true);
    assert.equal(isAvailabilityWindowOpen(availability, at(18)), false);
    assert.equal(isAvailabilityWindowOpen(availability, at(8, 59)), false);
  });

  it("supports overnight windows", () => {
    const availability = {
      enabled: true,
      start_time: "22:00",
      end_time: "06:00",
    };
    assert.equal(isAvailabilityWindowOpen(availability, at(22)), true);
    assert.equal(isAvailabilityWindowOpen(availability, at(2)), true);
    assert.equal(isAvailabilityWindowOpen(availability, at(6)), false);
    assert.equal(isAvailabilityWindowOpen(availability, at(12)), false);
  });

  it("prioritizes manual disabled over schedule state", () => {
    assert.equal(
      getHostMappingAvailabilityState(
        {
          disabled: true,
          availability: {
            enabled: true,
            start_time: "09:00",
            end_time: "18:00",
          },
        },
        at(10),
      ),
      "disabled",
    );
  });
});
