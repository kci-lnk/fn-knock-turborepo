import { startRuntime } from "./runtime-test-harness.mjs";
import { summarizeRuntimeSamples } from "./runtime-performance-lib.mjs";

const parseRunCount = (value) => {
  const count = Number.parseInt(value ?? "5", 10);
  if (!Number.isInteger(count) || count < 1 || count > 15) {
    throw new Error(
      "FN_KNOCK_RUNTIME_PERF_RUNS must be an integer from 1 to 15",
    );
  }
  return count;
};

const runCount = parseRunCount(process.env.FN_KNOCK_RUNTIME_PERF_RUNS);
const samples = [];
for (let index = 0; index < runCount; index += 1) {
  let runtime;
  try {
    runtime = await startRuntime({
      gatewayBinary:
        process.env.FN_KNOCK_RUNTIME_PERF_GATEWAY_BIN ??
        process.env.FN_KNOCK_RUNTIME_E2E_GATEWAY_BIN,
      serverBinary: process.env.FN_KNOCK_RUNTIME_SERVER_BIN,
      tempPrefix: "fn-knock-runtime-performance-",
    });
    samples.push(runtime.metrics);
  } finally {
    await runtime?.stop();
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      schema_version: 1,
      sample_count: samples.length,
      samples,
      summary: summarizeRuntimeSamples(samples),
    },
    null,
    2,
  )}\n`,
);
