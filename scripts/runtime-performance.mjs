import { startRuntime } from "./runtime-test-harness.mjs";
import { summarizeRuntimeSamples } from "./runtime-performance-lib.mjs";

const parseRunCount = (value, fallback, label, maximum = 15) => {
  const count = Number.parseInt(value ?? String(fallback), 10);
  if (!Number.isInteger(count) || count < 0 || count > maximum) {
    throw new Error(`${label} must be an integer from 0 to ${maximum}`);
  }
  return count;
};

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const maxCheckpoint = (samples) => {
  const fields = new Set(samples.flatMap((sample) => Object.keys(sample)));
  const result = { captured_at: samples.at(-1)?.captured_at ?? null };
  for (const field of fields) {
    if (field === "captured_at") continue;
    const values = samples
      .map((sample) => sample[field])
      .filter((value) => typeof value === "number" && Number.isFinite(value));
    result[field] = values.length > 0 ? Math.max(...values) : null;
  }
  return result;
};

const runLoadScenario = async ({
  collectCheckpoint,
  concurrency,
  durationMs,
  expectedResponseBytes,
  name,
  url,
}) => {
  let requests = 0;
  let responseBytes = 0;
  const startedAt = performance.now();
  const deadline = startedAt + durationMs;
  const checkpointSamples = [];
  let sampling = true;
  const sampler = (async () => {
    while (sampling) {
      checkpointSamples.push(await collectCheckpoint());
      await delay(500);
    }
  })();
  const worker = async () => {
    while (performance.now() < deadline) {
      const response = await fetch(url, {
        headers: { "accept-encoding": "identity" },
      });
      if (!response.ok) {
        throw new Error(`${name} load returned HTTP ${response.status}`);
      }
      const body = await response.arrayBuffer();
      if (body.byteLength !== expectedResponseBytes) {
        throw new Error(
          `${name} response length ${body.byteLength} != ${expectedResponseBytes}`,
        );
      }
      responseBytes += body.byteLength;
      requests += 1;
    }
  };
  try {
    await Promise.all(Array.from({ length: concurrency }, worker));
  } finally {
    sampling = false;
    await sampler;
  }
  checkpointSamples.push(await collectCheckpoint());
  const elapsedMs = Math.round(performance.now() - startedAt);
  return {
    name,
    concurrency,
    elapsed_ms: elapsedMs,
    requests,
    response_bytes: responseBytes,
    requests_per_second: Number((requests / (elapsedMs / 1000)).toFixed(2)),
    peak: maxCheckpoint(checkpointSamples),
  };
};

const enableWaf = async (backendUrl) => {
  const rule =
    'SecRule REQUEST_HEADERS:User-Agent "@contains fn-knock-runtime-waf-probe" ' +
    '"id:990001,phase:1,pass,nolog"\n';
  const uploadResponse = await fetch(
    `${backendUrl}/api/admin/waf/custom/upload`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        files: [
          {
            filename: "runtime-performance.conf",
            content_base64: Buffer.from(rule).toString("base64"),
          },
        ],
      }),
    },
  );
  if (!uploadResponse.ok) {
    throw new Error(
      `failed to install WAF load rule: HTTP ${uploadResponse.status} ${await uploadResponse.text()}`,
    );
  }
  const response = await fetch(`${backendUrl}/api/admin/waf/config`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      enabled: true,
      paranoia_level: 1,
      executing_paranoia_level: 1,
    }),
  });
  if (!response.ok) {
    throw new Error(
      `failed to enable WAF load path: HTTP ${response.status} ${await response.text()}`,
    );
  }
};

const verifyStaticServing = async (adminUrl, authUrl) => {
  const indexResponse = await fetch(adminUrl, {
    headers: { "accept-encoding": "identity" },
  });
  if (!indexResponse.ok) {
    throw new Error(`admin index returned HTTP ${indexResponse.status}`);
  }
  if (indexResponse.headers.get("cache-control") !== "no-cache") {
    throw new Error("admin index is missing no-cache");
  }
  if (
    !indexResponse.headers
      .getSetCookie()
      .some((value) => value.startsWith("fn_knock_locale="))
  ) {
    throw new Error("admin index did not set fn_knock_locale");
  }
  const indexHtml = await indexResponse.text();
  const assetPath = indexHtml.match(/<script[^>]+src="([^"]+)"/u)?.[1];
  if (!assetPath) throw new Error("admin index has no module script asset");
  const assetUrl = new URL(assetPath, adminUrl);
  const head = async (encoding) =>
    fetch(assetUrl, {
      method: "HEAD",
      headers: { "accept-encoding": encoding },
    });
  const brotli = await head("br");
  const etag = brotli.headers.get("etag");
  if (
    !brotli.ok ||
    brotli.headers.get("content-encoding") !== "br" ||
    !etag ||
    !brotli.headers.get("cache-control")?.includes("immutable") ||
    Number(brotli.headers.get("content-length")) <= 0 ||
    (await brotli.arrayBuffer()).byteLength !== 0
  ) {
    throw new Error("Brotli HEAD response failed the static asset contract");
  }
  const gzip = await head("gzip");
  if (!gzip.ok || gzip.headers.get("content-encoding") !== "gzip") {
    throw new Error("gzip static asset negotiation failed");
  }
  const weightedEncoding = await head("br;q=0.2, gzip;q=0.9");
  if (
    !weightedEncoding.ok ||
    weightedEncoding.headers.get("content-encoding") !== "gzip"
  ) {
    throw new Error("static compression did not honor Accept-Encoding quality");
  }
  const conditional = await fetch(assetUrl, {
    headers: { "accept-encoding": "br", "if-none-match": etag },
  });
  if (
    conditional.status !== 304 ||
    (await conditional.arrayBuffer()).byteLength !== 0
  ) {
    throw new Error("static asset If-None-Match did not return an empty 304");
  }
  const spaFallback = await fetch(
    new URL("/__runtime_static_spa_probe__", adminUrl),
  );
  if (
    !spaFallback.ok ||
    spaFallback.headers.get("cache-control") !== "no-cache" ||
    !spaFallback.headers.get("content-type")?.startsWith("text/html")
  ) {
    throw new Error("admin SPA fallback failed the index cache contract");
  }
  const rejectedSpaMutation = await fetch(
    new URL("/__runtime_static_spa_probe__", adminUrl),
    { method: "POST" },
  );
  if (
    rejectedSpaMutation.status !== 405 ||
    rejectedSpaMutation.headers.get("allow") !== "GET, HEAD"
  ) {
    throw new Error("admin SPA fallback accepted a non-read method");
  }
  const apiRoot = await fetch(new URL("/api", adminUrl));
  if (apiRoot.status !== 404) {
    throw new Error("exact API root escaped the JSON not-found boundary");
  }
  const authIndex = await fetch(authUrl);
  if (
    !authIndex.ok ||
    !authIndex.headers
      .getSetCookie()
      .some((value) => value.startsWith("fn_knock_locale="))
  ) {
    throw new Error("auth index did not set the configured locale cookie");
  }
};

const exerciseGatewayMemoryConfig = async (backendUrl) => {
  const endpoint = `${backendUrl}/api/admin/runtime-health/gateway-memory`;
  const currentResponse = await fetch(endpoint);
  if (!currentResponse.ok) {
    throw new Error(
      `failed to read gateway memory config: HTTP ${currentResponse.status}`,
    );
  }
  const current = (await currentResponse.json()).data;
  const effectiveMib = current.effective_memory_limit_bytes / (1024 * 1024);
  if (!Number.isInteger(effectiveMib)) {
    throw new Error(
      `gateway auto memory limit is not MiB-aligned: ${effectiveMib}`,
    );
  }
  const update = async (body) => {
    const response = await fetch(endpoint, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!response.ok) {
      throw new Error(
        `failed to update gateway memory config: HTTP ${response.status} ${await response.text()}`,
      );
    }
    return (await response.json()).data;
  };
  const rejectedLimitMib = Number.parseInt(
    process.env.FN_KNOCK_RUNTIME_PERF_REJECT_MEMORY_LIMIT_MIB ?? "",
    10,
  );
  if (Number.isInteger(rejectedLimitMib)) {
    const rejected = await fetch(endpoint, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ memory_limit_mib: rejectedLimitMib }),
    });
    if (rejected.status !== 400) {
      throw new Error(
        `out-of-policy memory limit ${rejectedLimitMib} MiB returned HTTP ${rejected.status}`,
      );
    }
    const afterRejectedResponse = await fetch(endpoint);
    const afterRejected = (await afterRejectedResponse.json()).data;
    if (
      afterRejected.memory_limit_mib !== current.memory_limit_mib ||
      afterRejected.effective_memory_limit_bytes !==
        current.effective_memory_limit_bytes
    ) {
      throw new Error("rejected gateway memory limit changed persisted state");
    }
  }
  const manual = await update({ memory_limit_mib: effectiveMib });
  if (
    manual.memory_limit_mib !== effectiveMib ||
    manual.effective_memory_limit_bytes !== current.effective_memory_limit_bytes
  ) {
    throw new Error("gateway manual memory limit did not apply atomically");
  }
  const automatic = await update({ memory_limit_mib: null });
  if (
    automatic.memory_limit_mib !== null ||
    automatic.effective_memory_limit_bytes !==
      current.effective_memory_limit_bytes
  ) {
    throw new Error("gateway automatic memory limit did not restore");
  }
};

const runCount = parseRunCount(
  process.env.FN_KNOCK_RUNTIME_PERF_RUNS,
  5,
  "FN_KNOCK_RUNTIME_PERF_RUNS",
);
if (runCount < 1)
  throw new Error("FN_KNOCK_RUNTIME_PERF_RUNS must be at least 1");
const loadRunCount = parseRunCount(
  process.env.FN_KNOCK_RUNTIME_PERF_LOAD_RUNS,
  3,
  "FN_KNOCK_RUNTIME_PERF_LOAD_RUNS",
  runCount,
);
const loadDurationMs =
  parseRunCount(
    process.env.FN_KNOCK_RUNTIME_PERF_LOAD_SECONDS,
    30,
    "FN_KNOCK_RUNTIME_PERF_LOAD_SECONDS",
    300,
  ) * 1000;

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
    const checkpoints = {
      readiness: await runtime.collectCheckpoint(),
    };
    if (process.env.FN_KNOCK_RUNTIME_PERF_VERIFY_STATIC !== "0") {
      await verifyStaticServing(runtime.adminUrl, runtime.authUrl);
    }
    await delay(1_000);
    checkpoints.startup_1s = await runtime.collectCheckpoint();
    await delay(9_000);
    checkpoints.stable_10s = await runtime.collectCheckpoint();
    if (process.env.FN_KNOCK_RUNTIME_PERF_EXERCISE_MEMORY_CONFIG !== "0") {
      await exerciseGatewayMemoryConfig(runtime.backendUrl);
    }

    const loads = [];
    if (index < loadRunCount) {
      loads.push(
        await runLoadScenario({
          collectCheckpoint: runtime.collectCheckpoint,
          concurrency: 64,
          durationMs: loadDurationMs,
          expectedResponseBytes: 2 * 1024 * 1024,
          name: "proxy_2mib",
          url: `${runtime.gatewayProxyUrl}/perf/fixed`,
        }),
      );
      loads.push(
        await runLoadScenario({
          collectCheckpoint: runtime.collectCheckpoint,
          concurrency: 32,
          durationMs: loadDurationMs,
          expectedResponseBytes: 2 * 1024 * 1024,
          name: "unknown_length_octet_stream",
          url: `${runtime.gatewayProxyUrl}/perf/stream`,
        }),
      );
      await enableWaf(runtime.backendUrl);
      loads.push(
        await runLoadScenario({
          collectCheckpoint: runtime.collectCheckpoint,
          concurrency: 32,
          durationMs: loadDurationMs,
          expectedResponseBytes: 1024,
          name: "waf_1kib",
          url: `${runtime.gatewayProxyUrl}/perf/waf`,
        }),
      );
      checkpoints.load_peak = maxCheckpoint(loads.map((load) => load.peak));
      await delay(30_000);
      checkpoints.post_load_30s = await runtime.collectCheckpoint();
      await runtime.reclaimGatewayMemory();
      // Runtime health probes Go every five seconds. Wait through one complete
      // probe interval so this checkpoint reflects the explicit reclaim.
      await delay(6_000);
      checkpoints.post_reclaim = await runtime.collectCheckpoint();
    }

    samples.push({
      readiness_ms: runtime.readinessMs,
      checkpoints,
      loads,
    });
  } finally {
    await runtime?.stop();
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      schema_version: 2,
      idle_sample_count: samples.length,
      load_sample_count: samples.filter((sample) => sample.loads.length > 0)
        .length,
      samples,
      summary: summarizeRuntimeSamples(samples),
    },
    null,
    2,
  )}\n`,
);
