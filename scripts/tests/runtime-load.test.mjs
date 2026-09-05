import assert from "node:assert/strict";
import { getEventListeners } from "node:events";
import http from "node:http";
import test from "node:test";
import { createLoadClient } from "../runtime-load-client.mjs";
import {
  assessSamplingQuality,
  collectLoadSample,
  runLoadScenario,
} from "../runtime-load.mjs";
import { readProcessMemory } from "../runtime-process-memory.mjs";

const withServer = async (handler, run) => {
  const server = http.createServer(handler);
  const sockets = new Set();
  server.on("connection", (socket) => {
    sockets.add(socket);
    socket.once("close", () => sockets.delete(socket));
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    await run(`http://127.0.0.1:${server.address().port}`, sockets);
  } finally {
    server.closeAllConnections();
    await new Promise((resolve) => server.close(resolve));
  }
};
const assertReleased = (client) =>
  assert.deepEqual(client.diagnostics(), {
    active_requests: 0,
    active_deadlines: 0,
    active_abort_listeners: 0,
  });

test("load client reuses connections and avoids copying binary responses", async () => {
  const bytes = Buffer.alloc(128 * 1024, 7);
  await withServer(
    (_req, res) => res.end(bytes),
    async (url, sockets) => {
      const client = createLoadClient({
        url,
        concurrency: 1,
        expectedResponseBytes: bytes.length,
      });
      try {
        for (let index = 0; index < 8; index += 1) {
          const response = await client.request(new AbortController().signal);
          assert.equal(response.bytes, bytes.length);
          assert.equal(response.body, undefined);
          assertReleased(client);
        }
        assert.equal(sockets.size, 1);
      } finally {
        client.close();
      }
    },
  );
});

test("request deadlines, cancellation, and overlong bodies release timers/listeners", async () => {
  await withServer(
    (req, res) => {
      res.writeHead(200);
      if (req.url === "/long") res.end("too many bytes");
      else res.write("{");
    },
    async (url) => {
      const timeoutClient = createLoadClient({
        url,
        concurrency: 1,
        responseValidation: "locale",
        timeoutMs: 40,
      });
      try {
        await assert.rejects(
          timeoutClient.request(new AbortController().signal),
          /deadline/,
        );
        assertReleased(timeoutClient);
      } finally {
        timeoutClient.close();
      }
      const client = createLoadClient({
        url,
        concurrency: 1,
        responseValidation: "locale",
      });
      try {
        const controller = new AbortController();
        const request = client.request(controller.signal);
        controller.abort(new Error("cancel fixture"));
        await assert.rejects(request, /cancel fixture/);
        await assert.rejects(
          client.request(controller.signal),
          /cancel fixture/,
        );
        assertReleased(client);
      } finally {
        client.close();
      }
      const oversized = createLoadClient({
        url: `${url}/long`,
        concurrency: 1,
        expectedResponseBytes: 4,
      });
      try {
        await assert.rejects(
          oversized.request(new AbortController().signal),
          /exceeded 4 bytes/,
        );
        assertReleased(oversized);
      } finally {
        oversized.close();
      }
    },
  );
});

test("quality checks include the start/end of the sampled window and duration drift", () => {
  const good = {
    durationMs: 1000,
    result: { elapsed_ms: 1001, client_start_delay_ms: 1 },
    memoryTimes: [1, 101, 201, 301, 401, 501, 601, 701, 801, 901],
  };
  assert.equal(assessSamplingQuality(good).passed, true);
  assert.equal(
    assessSamplingQuality({ ...good, memoryTimes: [1, 101, 201, 301, 401] })
      .passed,
    false,
  );
  assert.equal(
    assessSamplingQuality({ ...good, memoryTimes: [601, 701, 801, 901, 1000] })
      .passed,
    false,
  );
  assert.equal(
    assessSamplingQuality({
      ...good,
      result: { ...good.result, elapsed_ms: 1800 },
    }).passed,
    false,
  );
});

test(
  "high-rate locale requests do not starve OS sampling",
  { skip: !["linux", "darwin"].includes(process.platform) },
  async () => {
    await withServer(
      (_req, res) =>
        res.end('{"success":true,"data":{"default_locale":"en-US"}}'),
      async (url) => {
        const sample = async () => ({
          management_rss_bytes: (await readProcessMemory(process.pid))
            .rss_bytes,
        });
        const result = await runLoadScenario({
          name: "locale_fixture",
          url,
          concurrency: 16,
          durationMs: 1000,
          responseValidation: "locale",
          collectMemorySample: sample,
          collectCheckpoint: sample,
        });
        assert.ok(result.requests > 10);
        assert.equal(result.sampling_quality.passed, true);
        assert.ok(result.peak.management_rss_bytes > 0);
        assert.ok(
          result.samples.some((sample) => sample.kind === "process_memory"),
        );
        assert.deepEqual(result.client_resources_after_completion, {
          active_requests: 0,
          active_deadlines: 0,
          active_abort_listeners: 0,
        });
      },
    );
  },
);

test("sampler failure cancels an in-flight stalled body and drains the worker", async () => {
  let received;
  const requestReceived = new Promise((resolve) => {
    received = resolve;
  });
  await withServer(
    (_req, res) => {
      res.writeHead(200);
      res.write("{");
      received();
    },
    async (url) => {
      const started = performance.now();
      await assert.rejects(
        runLoadScenario({
          name: "cancel_fixture",
          url,
          concurrency: 2,
          durationMs: 10_000,
          responseValidation: "locale",
          collectCheckpoint: async () => ({ management_rss_bytes: 1 }),
          collectMemorySample: async () => {
            await requestReceived;
            throw new Error("fixture sampling failure");
          },
        }),
        /fixture sampling failure/,
      );
      assert.ok(performance.now() - started < 2000);
    },
  );
});

test("load rejects successful HTTP responses with wrong contents", async () => {
  await withServer(
    (_req, res) => res.end('{"success":true,"data":{}}'),
    async (url) => {
      await assert.rejects(
        runLoadScenario({
          name: "invalid_locale",
          url,
          concurrency: 1,
          durationMs: 1000,
          responseValidation: "locale",
          collectCheckpoint: async () => ({ management_rss_bytes: 1 }),
          collectMemorySample: async () => ({ management_rss_bytes: 1 }),
        }),
        /invalid JSON response/,
      );
    },
  );
});

test("individual sample deadlines bound uncooperative collectors and release listeners", async () => {
  const controller = new AbortController();
  let collectorSignal;
  const started = performance.now();
  await assert.rejects(
    collectLoadSample(
      (signal) => {
        collectorSignal = signal;
        return new Promise(() => {});
      },
      controller.signal,
      "fixture",
      30,
    ),
    /fixture sampler exceeded 30 ms deadline/,
  );
  assert.ok(performance.now() - started < 1000);
  assert.equal(collectorSignal.aborted, true);
  assert.equal(getEventListeners(collectorSignal, "abort").length, 0);
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
  assert.equal(
    await collectLoadSample(async () => 42, controller.signal, "fixture"),
    42,
  );
  await assert.rejects(
    collectLoadSample(
      async () => {
        throw new Error("collection failed");
      },
      controller.signal,
      "fixture",
    ),
    /collection failed/,
  );
  assert.equal(getEventListeners(controller.signal, "abort").length, 0);
});

test("load termination cannot hang on a collector that ignores cancellation", { timeout: 3000 }, async () => {
  await withServer(
    (_req, res) =>
      res.end('{"success":true,"data":{"default_locale":"en-US"}}'),
    async (url) => {
      let collectorSignal;
      const started = performance.now();
      await assert.rejects(
        runLoadScenario({
          name: "uncooperative_sampler",
          url,
          concurrency: 1,
          durationMs: 250,
          responseValidation: "locale",
          collectCheckpoint: async () => ({ management_rss_bytes: 1 }),
          collectMemorySample: (signal) => {
            collectorSignal = signal;
            return new Promise(() => {});
          },
        }),
        /failed sampling\/duration quality limits/,
      );
      assert.ok(performance.now() - started < 1500);
      assert.equal(collectorSignal.aborted, true);
      assert.equal(getEventListeners(collectorSignal, "abort").length, 0);
    },
  );
});

test("normal completion cancels the last in-flight sample without failing a valid run", { timeout: 3000 }, async () => {
  await withServer(
    (_req, res) =>
      res.end('{"success":true,"data":{"default_locale":"en-US"}}'),
    async (url) => {
      let collections = 0;
      let activeCollections = 0;
      let cancellations = 0;
      const result = await runLoadScenario({
        name: "last_sample_cancel",
        url,
        concurrency: 1,
        durationMs: 700,
        responseValidation: "locale",
        collectMemorySample: async () => ({ management_rss_bytes: 1 }),
        collectCheckpoint: (signal) => {
          collections += 1;
          if (collections === 1) return { management_rss_bytes: 1 };
          activeCollections += 1;
          return new Promise((_, reject) => {
            signal.addEventListener(
              "abort",
              () => {
                activeCollections -= 1;
                cancellations += 1;
                reject(signal.reason);
              },
              { once: true },
            );
          });
        },
      });
      assert.equal(result.sampling_quality.passed, true);
      assert.equal(collections, 2);
      assert.equal(cancellations, 1);
      assert.equal(activeCollections, 0);
    },
  );
});
