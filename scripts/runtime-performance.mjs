import { fetchRuntime, startRuntime } from "./runtime-test-harness.mjs";
import { summarizeRuntimeSamples } from "./runtime-performance-lib.mjs";
import { maxCheckpoint, runLoadScenario } from "./runtime-load.mjs";

const parseRunCount = (value, fallback, label, maximum = 15) => {
  const count = Number(value ?? fallback);
  if (!Number.isInteger(count) || count < 0 || count > maximum) {
    throw new Error(`${label} must be an integer from 0 to ${maximum}`);
  }
  return count;
};

const delay = (milliseconds) =>
  new Promise((resolve) => setTimeout(resolve, milliseconds));

const enableWaf = async (backendUrl) => {
  const rule =
    'SecRule REQUEST_HEADERS:User-Agent "@contains fn-knock-runtime-waf-probe" ' +
    '"id:990001,phase:1,pass,nolog"\n';
  const uploadResponse = await fetchRuntime(
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
  const response = await fetchRuntime(`${backendUrl}/api/admin/waf/config`, {
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
  const indexResponse = await fetchRuntime(adminUrl, {
    headers: { "accept-encoding": "identity" },
  });
  if (!indexResponse.ok) {
    throw new Error(`admin index returned HTTP ${indexResponse.status}`);
  }
  if (
    indexResponse.headers.get("cache-control") !==
    "private, no-store, no-cache, max-age=0, must-revalidate"
  ) {
    throw new Error("admin index is missing the non-storable cache policy");
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
    fetchRuntime(assetUrl, {
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
  const conditional = await fetchRuntime(assetUrl, {
    headers: { "accept-encoding": "br", "if-none-match": etag },
  });
  if (
    conditional.status !== 304 ||
    (await conditional.arrayBuffer()).byteLength !== 0
  ) {
    throw new Error("static asset If-None-Match did not return an empty 304");
  }
  const spaFallback = await fetchRuntime(
    new URL("/__runtime_static_spa_probe__", adminUrl),
  );
  if (
    !spaFallback.ok ||
    spaFallback.headers.get("cache-control") !==
      "private, no-store, no-cache, max-age=0, must-revalidate" ||
    !spaFallback.headers.get("content-type")?.startsWith("text/html")
  ) {
    throw new Error("admin SPA fallback failed the index cache contract");
  }
  const rejectedSpaMutation = await fetchRuntime(
    new URL("/__runtime_static_spa_probe__", adminUrl),
    { method: "POST" },
  );
  if (
    rejectedSpaMutation.status !== 405 ||
    rejectedSpaMutation.headers.get("allow") !== "GET, HEAD"
  ) {
    throw new Error("admin SPA fallback accepted a non-read method");
  }
  const apiRoot = await fetchRuntime(new URL("/api", adminUrl));
  if (apiRoot.status !== 404) {
    throw new Error("exact API root escaped the JSON not-found boundary");
  }
  const authIndex = await fetchRuntime(authUrl);
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
  const currentResponse = await fetchRuntime(endpoint);
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
    const response = await fetchRuntime(endpoint, {
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
    const rejected = await fetchRuntime(endpoint, {
      method: "PUT",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ memory_limit_mib: rejectedLimitMib }),
    });
    if (rejected.status !== 400) {
      throw new Error(
        `out-of-policy memory limit ${rejectedLimitMib} MiB returned HTTP ${rejected.status}`,
      );
    }
    const afterRejectedResponse = await fetchRuntime(endpoint);
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

if (loadRunCount > 0 && loadDurationMs === 0) {
  throw new Error(
    "FN_KNOCK_RUNTIME_PERF_LOAD_SECONDS must be positive when load runs are enabled",
  );
}

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
          collectMemorySample: runtime.collectMemorySample,
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
          collectMemorySample: runtime.collectMemorySample,
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
          collectMemorySample: runtime.collectMemorySample,
          concurrency: 32,
          durationMs: loadDurationMs,
          expectedResponseBytes: 1024,
          name: "waf_1kib",
          url: `${runtime.gatewayProxyUrl}/perf/waf`,
        }),
      );
      loads.push(
        await runLoadScenario({
          collectCheckpoint: runtime.collectCheckpoint,
          collectMemorySample: runtime.collectMemorySample,
          concurrency: 16,
          durationMs: loadDurationMs,
          name: "management_locale",
          url: `${runtime.backendUrl}/api/admin/config/locale`,
          responseValidation: "locale",
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
  } catch (error) {
    if (error.samples) {
      // Keep invalid trials inspectable in the CI log without presenting them
      // as successful measurements or silently retrying them.
      console.error(
        JSON.stringify({
          failed_run: index + 1,
          error: error.message,
          measurement: error.measurement ?? null,
          samples: error.samples,
        }),
      );
    }
    throw error;
  } finally {
    await runtime?.stop();
  }
}

process.stdout.write(
  `${JSON.stringify(
    {
      schema_version: 2,
      management_rss_sampling: ["linux", "darwin"].includes(process.platform)
        ? { source: "operating_system", interval_ms: 100 }
        : { source: "runtime_health_snapshot", interval_ms: 100 },
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
