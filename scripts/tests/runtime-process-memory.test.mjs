import assert from "node:assert/strict";
import test from "node:test";
import {
  parseLinuxProcessMemory,
  readProcessMemory,
} from "../runtime-process-memory.mjs";

test("Linux sampler distinguishes current RSS from the process lifetime high-water mark", () => {
  assert.deepEqual(
    parseLinuxProcessMemory(
      "Name:\tserver-admin-rs\nVmHWM:\t 8192 kB\nVmRSS:\t 4096 kB\n",
    ),
    {
      rss_bytes: 4 * 1024 * 1024,
      peak_rss_bytes: 8 * 1024 * 1024,
    },
  );
  assert.deepEqual(parseLinuxProcessMemory("Name:\texited\n"), {
    rss_bytes: null,
    peak_rss_bytes: null,
  });
});

test(
  "OS sampler reads a live process without a runtime health endpoint",
  {
    skip: !["linux", "darwin"].includes(process.platform),
  },
  async () => {
    const memory = await readProcessMemory(process.pid);
    assert.ok(memory.rss_bytes > 0);
    if (process.platform === "linux")
      assert.ok(memory.peak_rss_bytes >= memory.rss_bytes);
    await assert.rejects(readProcessMemory(0), /positive PID/);
  },
);

test("OS sampler rejects cancellation before starting process I/O", async () => {
  const controller = new AbortController();
  controller.abort(new Error("sampling cancelled"));
  await assert.rejects(
    readProcessMemory(process.pid, controller.signal),
    /sampling cancelled/,
  );
});
