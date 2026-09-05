import { setMaxListeners } from "node:events";
import { monitorEventLoopDelay, performance } from "node:perf_hooks";
import { setImmediate as yieldToEventLoop } from "node:timers/promises";
import { parentPort, workerData } from "node:worker_threads";
import { createLoadClient } from "./runtime-load-client.mjs";

const now = () => performance.timeOrigin + performance.now();
const abort = new AbortController();
setMaxListeners(workerData.concurrency + 1, abort.signal);
let started = false;

const run = async ({ startedAt, durationMs }) => {
  const enteredAt = now();
  const deadline = startedAt + durationMs;
  const client = createLoadClient(workerData);
  const latencyCounts = new Uint32Array(10_001);
  const loopDelay = monitorEventLoopDelay({ resolution: 10 });
  loopDelay.enable();
  let requests = 0;
  let responseBytes = 0;
  const requestLoop = async () => {
    while (!abort.signal.aborted && now() < deadline) {
      const requestStarted = now();
      const response = await client.request(abort.signal);
      if (response.status < 200 || response.status >= 300) {
        throw new Error(
          `${workerData.name} load returned HTTP ${response.status}`,
        );
      }
      if (
        workerData.expectedResponseBytes !== undefined &&
        response.bytes !== workerData.expectedResponseBytes
      ) {
        throw new Error(
          `${workerData.name} response length ${response.bytes} != ${workerData.expectedResponseBytes}`,
        );
      }
      if (workerData.responseValidation === "locale") {
        const value = JSON.parse(response.body.toString("utf8"));
        if (
          value.success !== true ||
          typeof value.data?.default_locale !== "string"
        ) {
          throw new Error(
            `${workerData.name} returned an invalid JSON response`,
          );
        }
      }
      latencyCounts[Math.min(10_000, Math.ceil(now() - requestStarted))] += 1;
      requests += 1;
      responseBytes += response.bytes;
      await yieldToEventLoop();
    }
  };
  const requestsInFlight = Array.from(
    { length: workerData.concurrency },
    requestLoop,
  );
  let failure;
  try {
    await Promise.all(requestsInFlight);
    abort.signal.throwIfAborted();
  } catch (error) {
    failure = error;
    abort.abort(error);
    await Promise.allSettled(requestsInFlight);
  } finally {
    client.close();
    loopDelay.disable();
  }
  const elapsedMs = now() - startedAt;
  const resources = client.diagnostics();
  if (Object.values(resources).some((count) => count !== 0)) {
    failure ??= new Error(
      "runtime load client did not release all request resources",
    );
  }
  if (!failure && requests === 0)
    failure = new Error("No runtime requests completed");
  const percentile = (fraction) => {
    let count = 0;
    for (let ms = 0; ms < latencyCounts.length; ms += 1) {
      count += latencyCounts[ms];
      if (count >= Math.ceil(requests * fraction)) return ms;
    }
    return null;
  };
  const result = {
    elapsed_ms: Number(elapsedMs.toFixed(3)),
    requests,
    response_bytes: responseBytes,
    requests_per_second: Number(((requests * 1000) / elapsedMs).toFixed(2)),
    request_latency_p95_ms: percentile(0.95),
    request_latency_p99_ms: percentile(0.99),
    client_start_delay_ms: Number((enteredAt - startedAt).toFixed(3)),
    client_event_loop_delay_max_ms: Number((loopDelay.max / 1e6).toFixed(3)),
    client_resources_after_completion: resources,
  };
  parentPort.postMessage({
    type: failure ? "failed" : "completed",
    error: failure?.message,
    result,
  });
  parentPort.close();
};

parentPort.on("message", (message) => {
  if (message.type === "cancel") {
    abort.abort(new Error(message.reason));
  } else if (message.type === "start" && !started) {
    started = true;
    run(message).catch((error) => {
      parentPort.postMessage({ type: "failed", error: error.message });
      parentPort.close();
    });
  }
});
parentPort.postMessage({ type: "ready" });
