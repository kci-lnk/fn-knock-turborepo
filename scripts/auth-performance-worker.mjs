import http from "node:http";
import { monitorEventLoopDelay } from "node:perf_hooks";
import { parentPort, workerData } from "node:worker_threads";
import { requestSpec, validateResponse } from "./auth-performance-lib.mjs";

const agent = new http.Agent({
  keepAlive: true,
  maxSockets: workerData.concurrency,
  maxFreeSockets: workerData.concurrency,
});
const now = () => performance.timeOrigin + performance.now();
const request = (spec) =>
  new Promise((resolve, reject) => {
    let req;
    const timer = setTimeout(
      () => req?.destroy(new Error("request deadline exceeded")),
      workerData.timeoutMs,
    );
    req = http.request(
      {
        hostname: "127.0.0.1",
        port: 27999,
        localAddress: "198.18.0.1",
        path: spec.path,
        method: "GET",
        headers: spec.headers,
        agent,
      },
      (response) => {
        let body = "";
        response.setEncoding("utf8");
        response.on("data", (chunk) => {
          body += chunk;
          if (body.length > 1024 * 1024)
            req.destroy(new Error("response exceeded 1MiB"));
        });
        response.on("error", reject);
        response.on("aborted", () => reject(new Error("response aborted")));
        response.on("end", () =>
          resolve({
            status: response.statusCode,
            headers: response.headers,
            body,
          }),
        );
      },
    );
    req.on("error", reject);
    req.on("close", () => clearTimeout(timer));
    req.end();
  });

parentPort.on("message", async ({ type, startedAt, durationMs, phase }) => {
  if (type === "close") {
    agent.destroy();
    parentPort.close();
    return;
  }
  if (type !== "run") return;
  try {
    const enteredAt = now();
    const loop = monitorEventLoopDelay({ resolution: 10 });
    loop.enable();
    const histogram = new Array(60001).fill(0);
    const statuses = {},
      anomalies = [];
    let requests = 0,
      failures = 0,
      next = workerData.workerIndex;
    const finite = workerData.scenario === "grant_renewal";
    await Promise.all(
      Array.from({ length: workerData.concurrency }, async () => {
        while (
          now() < startedAt + durationMs &&
          (!finite || phase === "warm" || next < workerData.renewals)
        ) {
          const index = next;
          next += workerData.workerCount;
          // Warm the full requested interval without reusing measured renewal tokens.
          const warmReuse =
            finite && phase === "warm" && index >= workerData.renewals;
          const spec = requestSpec(
            warmReuse ? "grant_hit" : workerData.scenario,
            index,
            phase,
          );
          const began = now();
          let error;
          try {
            const response = await request(spec);
            statuses[response.status] = (statuses[response.status] ?? 0) + 1;
            error = validateResponse(spec, response);
          } catch (caught) {
            error = caught.message;
            statuses.client_error = (statuses.client_error ?? 0) + 1;
          }
          histogram[Math.min(60000, Math.ceil(now() - began))]++;
          requests++;
          if (error) {
            failures++;
            if (anomalies.length < 5) anomalies.push({ error, index });
          }
        }
      }),
    );
    loop.disable();
    parentPort.postMessage({
      type: "result",
      phase,
      requests,
      failures,
      statuses,
      anomalies,
      histogram,
      elapsed_ms: now() - startedAt,
      start_delay_ms: enteredAt - startedAt,
      event_loop_delay_max_ms: loop.max / 1e6,
      all_tokens_consumed: finite && next >= workerData.renewals,
    });
  } catch (error) {
    parentPort.postMessage({ type: "failure", error: error.stack });
  }
});
parentPort.postMessage({ type: "ready" });
