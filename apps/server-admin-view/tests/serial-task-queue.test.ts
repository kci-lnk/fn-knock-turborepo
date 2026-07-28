import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createSerialTaskQueue } from "../src/lib/serialTaskQueue";

describe("serial task queue", () => {
  it("runs tasks in order and continues after a rejected task", async () => {
    const run = createSerialTaskQueue();
    const order: string[] = [];
    let releaseFirst!: () => void;
    const firstGate = new Promise<void>((resolve) => {
      releaseFirst = resolve;
    });

    const first = run(async () => {
      order.push("first:start");
      await firstGate;
      order.push("first:end");
    });
    const second = run(async () => {
      order.push("second");
    });

    await Promise.resolve();
    assert.deepEqual(order, ["first:start"]);
    releaseFirst();
    await Promise.all([first, second]);
    assert.deepEqual(order, ["first:start", "first:end", "second"]);

    await assert.rejects(
      run(async () => {
        throw new Error("expected");
      }),
    );
    assert.equal(await run(async () => "recovered"), "recovered");
  });
});
