import assert from "node:assert/strict";
import test from "node:test";
import { createVisibilityPoller } from "../src/composables/useVisibilityPolling";

test("visibility poller does not overlap refresh requests", async () => {
  let calls = 0;
  let finish!: () => void;
  const poller = createVisibilityPoller({
    intervalMs: 60_000,
    immediate: false,
    task: async () => {
      calls += 1;
      await new Promise<void>((resolve) => {
        finish = resolve;
      });
    },
  });
  poller.start();
  const first = poller.refresh();
  const second = poller.refresh();
  await Promise.resolve();
  assert.equal(calls, 1);
  finish();
  await Promise.all([first, second]);
  poller.stop();
});

test("visibility poller aborts an in-flight task when stopped", async () => {
  let observedSignal: AbortSignal | null = null;
  const poller = createVisibilityPoller({
    intervalMs: 60_000,
    immediate: false,
    task: (signal) => {
      observedSignal = signal;
      return new Promise<void>((resolve) => {
        signal.addEventListener("abort", () => resolve(), { once: true });
      });
    },
  });
  poller.start();
  const pending = poller.refresh();
  await Promise.resolve();
  poller.stop();
  await pending;
  assert.equal(observedSignal?.aborted, true);
});

test("visibility poller aborts obsolete work before rescheduling", async () => {
  const signals: AbortSignal[] = [];
  const poller = createVisibilityPoller({
    intervalMs: 60_000,
    immediate: false,
    task: (signal) => {
      signals.push(signal);
      if (signals.length > 1) return Promise.resolve();
      return new Promise<void>((resolve) => {
        signal.addEventListener("abort", () => resolve(), { once: true });
      });
    },
  });
  poller.start();
  const pending = poller.refresh();
  await Promise.resolve();
  poller.sync();
  await pending;
  await new Promise((resolve) => setTimeout(resolve, 5));
  assert.equal(signals[0]?.aborted, true);
  assert.equal(signals.length, 2);
  poller.stop();
});

test("visibility poller discards queued refreshes when a cycle is paused", async () => {
  let enabled = true;
  let calls = 0;
  const poller = createVisibilityPoller({
    intervalMs: 60_000,
    immediate: false,
    enabled: () => enabled,
    task: (signal) => {
      calls += 1;
      if (calls > 1) return Promise.resolve();
      return new Promise<void>((resolve) => {
        signal.addEventListener("abort", () => resolve(), { once: true });
      });
    },
  });
  poller.start();
  const pending = poller.refresh();
  await Promise.resolve();
  void poller.refresh();
  enabled = false;
  poller.sync();
  await pending;

  enabled = true;
  poller.sync();
  await new Promise((resolve) => setTimeout(resolve, 10));
  assert.equal(calls, 2);
  poller.stop();
});
