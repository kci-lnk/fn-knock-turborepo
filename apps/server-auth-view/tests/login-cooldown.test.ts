/// <reference types="node" />

import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  appendRetryAfterSuffix,
  extractRetryAfterSeconds,
  useLoginCooldown,
} from "../src/composables/useLoginCooldown";

describe("login cooldown helpers", () => {
  it("extracts retry-after seconds from API payloads and errors", () => {
    assert.equal(extractRetryAfterSeconds({ retryAfter: 1.2 }), 2);
    assert.equal(
      extractRetryAfterSeconds({ response: { data: { retryAfter: "3" } } }),
      3,
    );
    assert.equal(
      extractRetryAfterSeconds({
        response: { headers: { "retry-after": ["4"] } },
      }),
      4,
    );
    assert.equal(extractRetryAfterSeconds({ retryAfter: 0 }), 0);
    assert.equal(extractRetryAfterSeconds({ retryAfter: "invalid" }), 0);
  });

  it("adds a localized suffix only when the message does not include it", () => {
    assert.equal(appendRetryAfterSuffix("Failed", 3, " (3s)"), "Failed (3s)");
    assert.equal(
      appendRetryAfterSuffix("Retry in 3 seconds", 3, " (3s)"),
      "Retry in 3 seconds",
    );
    assert.equal(
      appendRetryAfterSuffix("Failed (3s)", 3, " (3s)"),
      "Failed (3s)",
    );
    assert.equal(appendRetryAfterSuffix("Failed", 0, " (0s)"), "Failed");
  });

  it("tracks countdown state and clears the scheduler at zero", () => {
    let tick: (() => void) | null = null;
    const clearedHandles: unknown[] = [];
    const cooldown = useLoginCooldown({
      formatRetrySuffix: (seconds) => ` (${seconds}s)`,
      scheduler: {
        setInterval(callback) {
          tick = callback;
          return "timer";
        },
        clearInterval(handle) {
          clearedHandles.push(handle);
        },
      },
    });

    assert.equal(
      cooldown.resolveMessage("Failed", { retryAfter: 2 }),
      "Failed (2s)",
    );
    assert.equal(cooldown.remainingSeconds.value, 2);
    assert.equal(cooldown.isCoolingDown.value, true);

    assert.ok(tick);
    (tick as () => void)();
    assert.equal(cooldown.remainingSeconds.value, 1);
    (tick as () => void)();
    assert.equal(cooldown.remainingSeconds.value, 0);
    assert.equal(cooldown.isCoolingDown.value, false);
    assert.deepEqual(clearedHandles, ["timer"]);
  });
});
