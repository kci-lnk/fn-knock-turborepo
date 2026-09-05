import http from "node:http";

// Keep client allocations, timers and cancellation listeners bounded by the
// number of concurrent requests, rather than the total requests in a run.
export const createLoadClient = ({
  url,
  concurrency,
  expectedResponseBytes,
  responseValidation,
  timeoutMs = 10_000,
}) => {
  const endpoint = new URL(url);
  if (endpoint.protocol !== "http:") {
    throw new Error("runtime load requires the owned HTTP fixture endpoint");
  }
  const collectBody = responseValidation === "locale";
  const maxBodyBytes = expectedResponseBytes ?? 4096;
  const agent = new http.Agent({
    keepAlive: true,
    maxSockets: concurrency,
    maxFreeSockets: concurrency,
  });
  const counters = {
    active_requests: 0,
    active_deadlines: 0,
    active_abort_listeners: 0,
  };
  let closed = false;
  const request = (signal) =>
    new Promise((resolve, reject) => {
      if (closed) return reject(new Error("runtime load client is closed"));
      if (signal.aborted) return reject(signal.reason);
      let req;
      let response;
      let timer;
      let finished = false;
      let bytes = 0;
      const chunks = [];
      counters.active_requests += 1;
      const finish = (error, value) => {
        if (finished) return;
        finished = true;
        clearTimeout(timer);
        counters.active_deadlines -= 1;
        signal.removeEventListener("abort", onAbort);
        counters.active_abort_listeners -= 1;
        counters.active_requests -= 1;
        if (error) {
          response?.destroy();
          req?.destroy();
          reject(error);
        } else resolve(value);
      };
      const onAbort = () => finish(signal.reason);
      signal.addEventListener("abort", onAbort, { once: true });
      counters.active_abort_listeners += 1;
      timer = setTimeout(
        () =>
          finish(
            new Error(`runtime request exceeded ${timeoutMs} ms deadline`),
          ),
        timeoutMs,
      );
      counters.active_deadlines += 1;
      try {
        req = http.request(
          endpoint,
          { method: "GET", agent, headers: { "accept-encoding": "identity" } },
          (incoming) => {
            response = incoming;
            incoming.on("data", (chunk) => {
              if (finished) return;
              bytes += chunk.length;
              if (bytes > maxBodyBytes) {
                finish(
                  new Error(`runtime response exceeded ${maxBodyBytes} bytes`),
                );
                return;
              }
              // Binary fixtures need length validation, not a second copy of
              // every multi-megabyte response in the benchmark client heap.
              if (collectBody) chunks.push(chunk);
            });
            incoming.on("end", () => {
              if (!finished) {
                finish(null, {
                  status: incoming.statusCode,
                  bytes,
                  body: collectBody ? Buffer.concat(chunks, bytes) : undefined,
                });
              }
            });
            incoming.on("aborted", () =>
              finish(
                new Error("runtime response aborted before its complete body"),
              ),
            );
            incoming.on("error", finish);
          },
        );
        req.on("error", finish);
        req.end();
      } catch (error) {
        finish(error);
      }
    });
  return {
    request,
    diagnostics: () => ({ ...counters }),
    close: () => {
      closed = true;
      agent.destroy();
    },
  };
};
