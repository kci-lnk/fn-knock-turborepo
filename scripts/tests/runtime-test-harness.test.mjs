import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import http from "node:http";
import test from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import {
  collectRuntimeCheckpoint,
  fetchRuntime,
  stopChild,
  waitForHttp,
} from "../runtime-test-harness.mjs";

const withServer = async (handler, run) => {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    await run(`http://127.0.0.1:${server.address().port}`);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
};

test("runtime readiness respects its deadline when a server never sends headers", async () => {
  await withServer(
    () => {},
    async (url) => {
      const started = performance.now();
      await assert.rejects(waitForHttp(url, 80), /timeout|abort/i);
      assert.ok(performance.now() - started < 1_000);
    },
  );
});

test("runtime request deadline also covers stalled response bodies", async () => {
  await withServer(
    (_request, response) => {
      response.writeHead(200, { "content-type": "application/json" });
      response.write('{"waiting":');
    },
    async (url) => {
      const response = await fetchRuntime(url, {}, 80);
      await assert.rejects(response.json(), /abort/i);
    },
  );
});

test("runtime readiness cancels a successful streaming body and caller cancellation", async () => {
  await withServer(
    (_request, response) => {
      response.writeHead(200);
      response.write("ready");
    },
    async (url) => {
      await waitForHttp(url, 1_000);
      const controller = new AbortController();
      controller.abort(new Error("startup failed"));
      await assert.rejects(
        waitForHttp(url, 1_000, controller.signal),
        /startup failed/,
      );
    },
  );
});

test("runtime cleanup waits for SIGKILL and recognizes a signal-exited child", async () => {
  const child = spawn(
    process.execPath,
    [
      "-e",
      `
    process.on("SIGTERM", () => {});
    process.stdout.write("ready");
    setInterval(() => {}, 1000);
  `,
    ],
    { stdio: ["ignore", "pipe", "ignore"] },
  );
  try {
    await once(child.stdout, "data");
    await stopChild(child, 30);
    assert.equal(child.signalCode, "SIGKILL");
    assert.equal(child.listenerCount("exit"), 0);
    const started = performance.now();
    await stopChild(child);
    assert.ok(performance.now() - started < 100);
  } finally {
    if (child.exitCode === null && child.signalCode === null) {
      await stopChild(child, 1);
    }
  }
});

test("health checkpoint cancellation interrupts stalled bodies and retry waits", async () => {
  for (const stalledBody of [true, false]) {
    let received;
    const requestReceived = new Promise((resolve) => {
      received = resolve;
    });
    let requests = 0;
    await withServer(
      (_request, response) => {
        requests += 1;
        if (stalledBody) {
          response.writeHead(200, { "content-type": "application/json" });
          response.write('{"waiting":');
        } else {
          response.writeHead(503);
          response.end("retry");
        }
        received();
      },
      async (url) => {
        const controller = new AbortController();
        const checkpoint = collectRuntimeCheckpoint(
          url,
          process.pid,
          process.pid,
          controller.signal,
        );
        const rejected = assert.rejects(checkpoint, /sampling cancelled|abort/i);
        await requestReceived;
        await delay(20);
        const started = performance.now();
        controller.abort(new Error("sampling cancelled"));
        await rejected;
        assert.ok(performance.now() - started < 1000);
        assert.equal(requests, 1);
      },
    );
  }
});
