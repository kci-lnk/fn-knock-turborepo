import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createPollingLifecycle } from "../src/lib/pollingLifecycle";

describe("polling lifecycle", () => {
  it("does not revive polling after deactivation during initialization", async () => {
    let resolveInitialization: (() => void) | undefined;
    let initializationCount = 0;
    let startCount = 0;
    const lifecycle = createPollingLifecycle({
      initialize: () => {
        initializationCount += 1;
        return new Promise<void>((resolve) => {
          resolveInitialization = resolve;
        });
      },
      start: () => {
        startCount += 1;
      },
    });

    const firstActivation = lifecycle.activate();
    lifecycle.deactivate();
    resolveInitialization?.();
    await firstActivation;

    assert.equal(startCount, 0);

    await lifecycle.activate();
    assert.equal(initializationCount, 1);
    assert.equal(startCount, 1);
  });
});
