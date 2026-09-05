import { monitorEventLoopDelay } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";
import { Worker } from "node:worker_threads";

// A collector may fail to honor cancellation. Race it against our own bound,
// while passing the same cancellation to cooperative I/O so it can clean up.
export const collectLoadSample = async (
  collect,
  signal,
  kind,
  timeoutMs = 3_000,
) => {
  signal.throwIfAborted();
  const controller = new AbortController();
  const onStop = () => controller.abort(signal.reason);
  let rejectCancellation;
  const cancelled = new Promise((_, reject) => {
    rejectCancellation = reject;
  });
  const onAbort = () => rejectCancellation(controller.signal.reason);
  controller.signal.addEventListener("abort", onAbort, { once: true });
  signal.addEventListener("abort", onStop, { once: true });
  const timer = setTimeout(
    () =>
      controller.abort(
        new Error(`${kind} sampler exceeded ${timeoutMs} ms deadline`),
      ),
    timeoutMs,
  );
  try {
    return await Promise.race([
      Promise.resolve().then(() => {
        controller.signal.throwIfAborted();
        return collect(controller.signal);
      }),
      cancelled,
    ]);
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", onStop);
    controller.signal.removeEventListener("abort", onAbort);
  }
};

export const maxCheckpoint = (samples) => {
  const fields = new Set(samples.flatMap((sample) => Object.keys(sample)));
  const result = { captured_at: samples.at(-1)?.captured_at ?? null };
  for (const field of fields) {
    if (field === "captured_at") continue;
    const values = samples
      .map((sample) => sample[field])
      .filter(Number.isFinite);
    result[field] = values.length > 0 ? Math.max(...values) : null;
  }
  return result;
};

export const assessSamplingQuality = ({ durationMs, result, memoryTimes }) => {
  const gaps = memoryTimes.length
    ? [
        memoryTimes[0],
        ...memoryTimes.slice(1).map((time, index) => time - memoryTimes[index]),
        Math.max(0, result.elapsed_ms - memoryTimes.at(-1)),
      ]
    : [result.elapsed_ms];
  const quality = {
    max_allowed_gap_ms: 500,
    max_gap_ms: Math.max(...gaps),
    sample_count: memoryTimes.length,
    minimum_samples: Math.max(1, Math.floor(durationMs / 200)),
    duration_overrun_ms: result.elapsed_ms - durationMs,
    max_allowed_overrun_ms: Math.max(250, durationMs * 0.05),
    client_start_delay_ms: result.client_start_delay_ms,
  };
  quality.passed =
    result.elapsed_ms >= durationMs &&
    quality.duration_overrun_ms <= quality.max_allowed_overrun_ms &&
    quality.max_gap_ms <= quality.max_allowed_gap_ms &&
    quality.sample_count >= quality.minimum_samples &&
    quality.client_start_delay_ms >= 0 &&
    quality.client_start_delay_ms <= 100;
  return quality;
};

export const runLoadScenario = async ({
  collectCheckpoint,
  collectMemorySample,
  concurrency,
  durationMs,
  expectedResponseBytes,
  responseValidation,
  name,
  url,
}) => {
  if (
    !Number.isInteger(concurrency) ||
    concurrency < 1 ||
    concurrency > 128 ||
    !Number.isInteger(durationMs) ||
    durationMs < 1 ||
    durationMs > 300_000 ||
    (responseValidation !== undefined && responseValidation !== "locale") ||
    (expectedResponseBytes === undefined && responseValidation === undefined) ||
    (expectedResponseBytes !== undefined &&
      (!Number.isSafeInteger(expectedResponseBytes) ||
        expectedResponseBytes < 0))
  ) {
    throw new Error("invalid runtime load parameters");
  }
  const worker = new Worker(
    new URL("./runtime-load-worker.mjs", import.meta.url),
    {
      workerData: {
        concurrency,
        expectedResponseBytes,
        responseValidation,
        name,
        url,
      },
    },
  );
  let readyResolve;
  let readyReject;
  let doneResolve;
  let doneReject;
  let completed = false;
  const ready = new Promise((resolve, reject) => {
    readyResolve = resolve;
    readyReject = reject;
  });
  const done = new Promise((resolve, reject) => {
    doneResolve = resolve;
    doneReject = reject;
  });
  ready.catch(() => {});
  done.catch(() => {});
  const fail = (error) => {
    readyReject(error);
    doneReject(error);
  };
  worker.on("error", fail);
  worker.on("exit", (code) => {
    if (!completed)
      fail(new Error(`runtime load worker exited before completion: ${code}`));
  });
  worker.on("message", (message) => {
    if (message.type === "ready") readyResolve();
    else if (message.type === "failed") {
      const error = new Error(message.error);
      error.measurement = message.result;
      fail(error);
    } else if (message.type === "completed") {
      completed = true;
      doneResolve(message.result);
    }
  });
  const startupTimeout = setTimeout(
    () => fail(new Error("runtime load worker startup timed out")),
    10_000,
  );
  try {
    await ready;
  } catch (error) {
    await worker.terminate();
    throw error;
  } finally {
    clearTimeout(startupTimeout);
  }

  // Sampling and the upstream fixture server remain on the coordinator. A
  // high-throughput request loop must not starve ps stdout or its deadline.
  const startedAt = performance.timeOrigin + performance.now();
  const elapsed = () => performance.timeOrigin + performance.now() - startedAt;
  const loopDelay = monitorEventLoopDelay({ resolution: 10 });
  loopDelay.enable();
  const samples = [];
  const samplingAbort = new AbortController();
  let sampling = true;
  let samplingError;
  const sample = (collect, interval, kind) =>
    (async () => {
      while (sampling) {
        const started = elapsed();
        const checkpoint = await collectLoadSample(
          collect,
          samplingAbort.signal,
          kind,
        );
        if (!sampling) return;
        if (
          kind === "process_memory" &&
          !(
            Number.isFinite(checkpoint?.management_rss_bytes) &&
            checkpoint.management_rss_bytes >= 0
          )
        ) {
          throw new Error("runtime memory sampler returned no valid Rust RSS");
        }
        samples.push({
          kind,
          elapsed_ms: elapsed(),
          collection_ms: elapsed() - started,
          checkpoint,
        });
        await delay(interval, undefined, { signal: samplingAbort.signal });
      }
    })().catch((error) => {
      // Completing the load cancels the last collection and interval wait.
      // Failures observed while the load is active still invalidate the run.
      if (!sampling && samplingAbort.signal.aborted) return;
      samplingError ??= error;
      sampling = false;
      samplingAbort.abort(error);
      worker.postMessage({ type: "cancel", reason: error.message });
    });
  const samplers = [
    sample(collectCheckpoint, 500, "runtime_checkpoint"),
    sample(collectMemorySample, 100, "process_memory"),
  ];
  const timeout = setTimeout(
    () => fail(new Error("runtime load exceeded its bounded runtime")),
    durationMs + 15_000,
  );
  let result;
  let failure;
  try {
    worker.postMessage({ type: "start", startedAt, durationMs });
    result = await done;
  } catch (error) {
    failure = samplingError ?? error;
  } finally {
    clearTimeout(timeout);
    sampling = false;
    samplingAbort.abort(new Error("runtime load sampling completed"));
    await worker.terminate();
    await Promise.all(samplers);
    loopDelay.disable();
  }
  if (failure || samplingError) {
    const error = failure ?? samplingError;
    error.samples = samples;
    throw error;
  }
  const measuredSamples = samples.filter(
    (sample) => sample.elapsed_ms <= result.elapsed_ms,
  );
  const quality = assessSamplingQuality({
    durationMs,
    result,
    memoryTimes: measuredSamples
      .filter((sample) => sample.kind === "process_memory")
      .map((sample) => sample.elapsed_ms),
  });
  if (!quality.passed) {
    const error = new Error(
      `${name} failed sampling/duration quality limits: ${JSON.stringify(quality)}`,
    );
    error.measurement = result;
    error.samples = samples;
    throw error;
  }
  return {
    name,
    concurrency,
    ...result,
    requested_duration_ms: durationMs,
    coordinator_event_loop_delay_max_ms: Number(
      (loopDelay.max / 1e6).toFixed(3),
    ),
    sampling_quality: quality,
    memory_sampling_source: ["linux", "darwin"].includes(process.platform)
      ? "os"
      : "health-fallback",
    peak: maxCheckpoint(measuredSamples.map((sample) => sample.checkpoint)),
    samples,
  };
};
