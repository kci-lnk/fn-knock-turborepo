import { execFile, spawn } from "node:child_process";
import {
  access,
  mkdir,
  mkdtemp,
  readFile,
  readdir,
  rm,
} from "node:fs/promises";
import http from "node:http";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";
import { readProcessMemory } from "./runtime-process-memory.mjs";

const execFileAsync = promisify(execFile);
const idleMeasurementDelayMs = 1_000;
const gatewayMetricTimeoutMs = 7_000;
const runtimeMetricPollMs = 100;

export const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

// Keep the deadline attached to the response so stalled body reads also abort.
export const fetchRuntime = (url, options = {}, timeoutMs = 10_000) =>
  fetch(url, {
    ...options,
    signal: options.signal
      ? AbortSignal.any([options.signal, AbortSignal.timeout(timeoutMs)])
      : AbortSignal.timeout(timeoutMs),
  });

const getFreePort = () =>
  new Promise((resolve, reject) => {
    const server = net.createServer();
    server.unref();
    server.on("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => resolve(address.port));
    });
  });

export const waitForHttp = async (url, timeoutMs = 60_000, signal) => {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    signal?.throwIfAborted();
    try {
      const response = await fetchRuntime(
        url,
        { signal },
        Math.max(1, Math.min(2_000, deadline - Date.now())),
      );
      // Readiness only needs the status; release the connection immediately.
      await response.body?.cancel();
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    signal?.throwIfAborted();
    const remaining = deadline - Date.now();
    if (remaining > 0) {
      await new Promise((resolve) =>
        setTimeout(resolve, Math.min(100, remaining)),
      );
    }
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
};

const ensureRuntimeArtifacts = async (
  serverBinary,
  adminStaticPath,
  authStaticPath,
) => {
  const required = [
    serverBinary,
    path.join(adminStaticPath, "index.html"),
    path.join(authStaticPath, "index.html"),
  ];
  for (const artifactPath of required) {
    try {
      await access(artifactPath);
    } catch {
      const displayPath = path.relative(rootDir, artifactPath);
      throw new Error(
        `Missing ${displayPath}. Run the frontend builds and npm run ` +
          `runtime:build before the runtime audit.`,
      );
    }
  }
};

const buildGateway = async (output) => {
  const gatewayDir = path.join(rootDir, "..", "Go-Reauth-Proxy");
  await access(path.join(gatewayDir, ".git"));
  const manifest = JSON.parse(
    await readFile(path.join(rootDir, "version.json"), "utf8"),
  );
  const { stdout } = await execFileAsync("git", ["rev-parse", "HEAD"], {
    cwd: gatewayDir,
  });
  const actualCommit = stdout.trim().toLowerCase();
  await execFileAsync(
    "go",
    [
      "build",
      "-trimpath",
      "-ldflags",
      `-s -w -X go-reauth-proxy/pkg/version.Version=${manifest.version} ` +
        `-X go-reauth-proxy/pkg/version.Commit=${actualCommit}`,
      "-o",
      output,
      "./cmd/server",
    ],
    {
      cwd: gatewayDir,
      env: { ...process.env, CGO_ENABLED: "0", GOFLAGS: "-mod=readonly" },
      maxBuffer: 10 * 1024 * 1024,
    },
  );
  return output;
};

const resolveGatewayBinary = async (explicitBinary, tempDir) => {
  if (!explicitBinary) {
    try {
      return await buildGateway(path.join(tempDir, "go-reauth-proxy"));
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  const platform = process.platform;
  const architecture = process.arch;
  const platformSuffix =
    platform === "darwin"
      ? `darwin-${architecture === "arm64" ? "arm64" : "amd64"}`
      : platform === "linux"
        ? `linux-${
            architecture === "arm64"
              ? "arm64"
              : architecture === "arm"
                ? "arm"
                : "amd64"
          }`
        : platform === "win32"
          ? "windows-amd64.exe"
          : "";
  const candidates = [
    explicitBinary,
    platformSuffix
      ? path.join(
          rootDir,
          "..",
          "Go-Reauth-Proxy",
          "build",
          `go-reauth-proxy-${platformSuffix}`,
        )
      : "",
    platformSuffix
      ? path.join(
          rootDir,
          "apps",
          "fn-knock-lite",
          "app",
          "server",
          `go-reauth-proxy-${platformSuffix}`,
        )
      : "",
  ].filter(Boolean);

  for (const candidate of candidates) {
    try {
      await access(candidate);
      return candidate;
    } catch {
      // Try the next supported local runtime artifact.
    }
  }
  throw new Error(
    "Missing a native Go gateway binary. Pass gatewayBinary or build the " +
      "sibling Go-Reauth-Proxy checkout.",
  );
};

const childHasExited = (child) =>
  child.exitCode !== null ||
  child.signalCode !== null ||
  child.pid === undefined;

const signalAndWaitForChild = (child, signal, timeoutMs) =>
  new Promise((resolve, reject) => {
    const finish = (error) => {
      clearTimeout(timer);
      child.removeListener("exit", onExit);
      child.removeListener("error", onError);
      if (error) reject(error);
      else resolve(childHasExited(child));
    };
    const onExit = () => finish();
    const onError = (error) => finish(error);
    const timer = setTimeout(() => finish(), timeoutMs);
    // Install listeners before signaling, including children that exit immediately.
    child.once("exit", onExit);
    child.once("error", onError);
    if (childHasExited(child)) finish();
    else child.kill(signal);
  });

export const stopChild = async (child, graceMs = 3_000) => {
  if (childHasExited(child)) return;
  if (await signalAndWaitForChild(child, "SIGTERM", graceMs)) return;
  if (!(await signalAndWaitForChild(child, "SIGKILL", 3_000))) {
    throw new Error(
      `Timed out waiting for child ${child.pid} to exit after SIGKILL`,
    );
  }
};

const numberOrNull = (value) =>
  typeof value === "number" && Number.isFinite(value) && value >= 0
    ? Math.round(value)
    : null;

const countProcessFDs = async (pid, signal) => {
  signal?.throwIfAborted();
  if (!Number.isInteger(pid) || pid <= 0) return null;
  if (process.platform === "linux") {
    try {
      const entries = await readdir(`/proc/${pid}/fd`);
      signal?.throwIfAborted();
      return entries.length;
    } catch {
      signal?.throwIfAborted();
      return null;
    }
  }
  try {
    const { stdout } = await execFileAsync(
      "lsof",
      ["-a", "-p", String(pid), "-Fn"],
      { timeout: 2_000, signal },
    );
    return stdout.split("\n").filter((line) => /^f\d+$/u.test(line)).length;
  } catch {
    signal?.throwIfAborted();
    return null;
  }
};

const runtimeCheckpointFromPayload = async (
  payload,
  managementPid,
  gatewayPid,
  signal,
) => {
  const components = payload?.data?.components;
  const management = components?.management;
  const gateway = components?.gateway_process;
  const [managementMemory, gatewayMemory] = await Promise.all([
    readProcessMemory(managementPid, signal),
    readProcessMemory(gatewayPid, signal),
  ]);
  const managementRSS =
    managementMemory?.rss_bytes ?? numberOrNull(management?.rss_bytes);
  const gatewayRSS =
    gatewayMemory?.rss_bytes ?? numberOrNull(gateway?.rss_bytes);
  if (managementRSS === null || gatewayRSS === null) return null;
  const [managementFDs, gatewayFDs] = await Promise.all([
    countProcessFDs(managementPid, signal),
    countProcessFDs(gatewayPid, signal),
  ]);
  return {
    captured_at: new Date().toISOString(),
    management_rss_bytes: managementRSS,
    gateway_rss_bytes: gatewayRSS,
    management_peak_rss_bytes: managementMemory?.peak_rss_bytes ?? null,
    gateway_heap_alloc_bytes: numberOrNull(gateway?.heap_alloc_bytes),
    gateway_heap_sys_bytes: numberOrNull(gateway?.heap_sys_bytes),
    gateway_managed_memory_bytes: numberOrNull(gateway?.managed_memory_bytes),
    gateway_memory_limit_bytes: numberOrNull(gateway?.memory_limit_bytes),
    gateway_num_gc: numberOrNull(gateway?.num_gc),
    gateway_goroutines: numberOrNull(gateway?.goroutines),
    management_unix_fds: managementFDs,
    gateway_unix_fds: gatewayFDs,
    active_proxy_requests: numberOrNull(gateway?.active_proxy_requests),
    active_client_connections: numberOrNull(gateway?.active_client_connections),
    idle_client_connections: numberOrNull(gateway?.idle_client_connections),
    open_upstream_connections: numberOrNull(gateway?.open_upstream_connections),
    udp_sessions: numberOrNull(gateway?.udp_sessions),
    udp_queued_bytes: numberOrNull(gateway?.udp_queued_bytes),
    udp_queued_bytes_peak: numberOrNull(gateway?.udp_queued_bytes_peak),
    udp_queue_drops: numberOrNull(gateway?.udp_queue_drops),
  };
};

export const collectRuntimeCheckpoint = async (
  backendUrl,
  managementPid,
  gatewayPid,
  signal,
) => {
  signal?.throwIfAborted();
  const deadline = Date.now() + gatewayMetricTimeoutMs;
  let lastStatus = "no response";
  while (Date.now() < deadline) {
    signal?.throwIfAborted();
    try {
      const response = await fetchRuntime(
        `${backendUrl}/api/admin/runtime-health`,
        { signal },
        Math.max(1, Math.min(2_000, deadline - Date.now())),
      );
      if (response.ok) {
        const checkpoint = await runtimeCheckpointFromPayload(
          await response.json(),
          managementPid,
          gatewayPid,
          signal,
        );
        if (checkpoint) return checkpoint;
        lastStatus = "RSS fields are not ready";
      } else {
        lastStatus = `HTTP ${response.status}`;
        await response.body?.cancel();
      }
    } catch (error) {
      signal?.throwIfAborted();
      lastStatus = error?.message ?? String(error);
    }
    await delay(runtimeMetricPollMs, undefined, { signal });
  }
  throw new Error(`Runtime health metrics did not stabilize: ${lastStatus}`);
};

const collectRuntimeRSS = async (backendUrl, managementPid, gatewayPid) => {
  await new Promise((resolve) => setTimeout(resolve, idleMeasurementDelayMs));
  const checkpoint = await collectRuntimeCheckpoint(
    backendUrl,
    managementPid,
    gatewayPid,
  );
  const managementRSS = checkpoint.management_rss_bytes;
  const gatewayRSS = checkpoint.gateway_rss_bytes;
  return {
    management_rss_bytes: managementRSS,
    gateway_rss_bytes: gatewayRSS,
  };
};

const createLoadUpstream = async () => {
  const fixedBody = Buffer.alloc(2 * 1024 * 1024, 0x61);
  const streamChunk = Buffer.alloc(32 * 1024, 0x62);
  const wafBody = Buffer.alloc(1024, 0x63);
  const server = http.createServer((request, response) => {
    if (request.url?.startsWith("/stream")) {
      response.writeHead(200, { "content-type": "application/octet-stream" });
      for (let index = 0; index < 64; index += 1) response.write(streamChunk);
      response.end();
      return;
    }
    const body = request.url?.startsWith("/waf") ? wafBody : fixedBody;
    response.writeHead(200, {
      "content-length": String(body.length),
      "content-type": "application/octet-stream",
    });
    response.end(body);
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const address = server.address();
  return {
    url: `http://127.0.0.1:${address.port}`,
    stop: async () => {
      server.closeAllConnections?.();
      await new Promise((resolve) => server.close(resolve));
    },
  };
};

const configurePerformanceRoute = async (backendUrl, upstreamUrl) => {
  const modeResponse = await fetchRuntime(
    `${backendUrl}/api/admin/config/run_type`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ run_type: 1, reverse_proxy_submode: "path" }),
    },
  );
  if (!modeResponse.ok) {
    throw new Error(
      `failed to configure reverse-proxy runtime mode: HTTP ${modeResponse.status} ${await modeResponse.text()}`,
    );
  }
  const response = await fetchRuntime(
    `${backendUrl}/api/admin/config/proxy_mappings`,
    {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        mappings: [
          {
            path: "/perf",
            target: upstreamUrl,
            rewrite_html: false,
            use_auth: false,
            use_root_mode: false,
            strip_path: true,
          },
        ],
      }),
    },
  );
  if (!response.ok) {
    throw new Error(
      `failed to configure performance proxy route: HTTP ${response.status} ${await response.text()}`,
    );
  }
};

export const startRuntime = async ({
  externalAdminUrl,
  externalAuthUrl,
  gatewayBinary,
  serverBinary = process.env.FN_KNOCK_RUNTIME_SERVER_BIN,
  protectedAdmin = false,
  collectMetrics = !protectedAdmin,
  adminStaticPath = process.env.FN_KNOCK_RUNTIME_ADMIN_STATIC_PATH,
  authStaticPath = process.env.FN_KNOCK_RUNTIME_AUTH_STATIC_PATH,
  runtimeTarget = protectedAdmin ? "docker" : "fpk-lite",
  tempPrefix = "fn-knock-runtime-audit-",
} = {}) => {
  if (externalAdminUrl || externalAuthUrl) {
    if (!externalAdminUrl || !externalAuthUrl) {
      throw new Error("Set both externalAdminUrl and externalAuthUrl.");
    }
    await Promise.all([
      waitForHttp(externalAdminUrl),
      waitForHttp(externalAuthUrl),
    ]);
    const adminUrl = externalAdminUrl.replace(/\/+$/, "");
    return {
      adminUrl,
      authUrl: externalAuthUrl.replace(/\/+$/, ""),
      backendUrl: adminUrl,
      stop: async () => {},
    };
  }

  const resolvedServerBinary = path.resolve(
    rootDir,
    serverBinary ??
      path.join(
        "apps",
        "server-admin-rs",
        "target",
        "release",
        "server-admin-rs",
      ),
  );
  const resolvedAdminStaticPath = path.resolve(
    rootDir,
    adminStaticPath ?? "apps/server-admin-view/dist",
  );
  const resolvedAuthStaticPath = path.resolve(
    rootDir,
    authStaticPath ?? "apps/server-auth-view/dist",
  );
  await ensureRuntimeArtifacts(
    resolvedServerBinary,
    resolvedAdminStaticPath,
    resolvedAuthStaticPath,
  );
  const tempDir = await mkdtemp(path.join(os.tmpdir(), tempPrefix));
  const loadUpstream = await createLoadUpstream();
  let resolvedGatewayBinary;
  try {
    resolvedGatewayBinary = await resolveGatewayBinary(gatewayBinary, tempDir);
  } catch (error) {
    await loadUpstream.stop();
    await rm(tempDir, { recursive: true, force: true });
    throw error;
  }
  const selectedPorts = new Set();
  const portCount = protectedAdmin ? 5 : 4;
  while (selectedPorts.size < portCount) selectedPorts.add(await getFreePort());
  const [backendPort, authPort, goBackendPort, goProxyPort, adminViewPort] =
    selectedPorts;
  const gatewayConfigDir = path.join(tempDir, "gateway");
  await mkdir(gatewayConfigDir, { recursive: true });
  const sharedEnv = {
    ...process.env,
    BACKEND_PORT: String(backendPort),
    FN_KNOCK_INTERNAL_RPC_TOKEN: "runtime-audit-internal-token",
    HMAC_SECRET: "runtime-audit-hmac-secret",
  };
  const gateway = spawn(
    resolvedGatewayBinary,
    [
      "-c",
      path.join(gatewayConfigDir, "config.json"),
      "-admin-port",
      String(goBackendPort),
      "-proxy-port",
      String(goProxyPort),
    ],
    {
      cwd: rootDir,
      env: sharedEnv,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );
  const child = spawn(resolvedServerBinary, [], {
    cwd: rootDir,
    env: {
      ...sharedEnv,
      ADMIN_STATIC_PATH: resolvedAdminStaticPath,
      ...(protectedAdmin
        ? {
            ADMIN_VIEW_HOST: "127.0.0.1",
            ADMIN_VIEW_PORT: String(adminViewPort),
          }
        : {}),
      AUTH_HOST: "127.0.0.1",
      AUTH_PORT: String(authPort),
      AUTH_STATIC_PATH: resolvedAuthStaticPath,
      BACKEND_HOST: "127.0.0.1",
      BACKEND_PORT: String(backendPort),
      EXPOSE_RUNTIME_HMAC_SECRET: "1",
      FN_KNOCK_DATA_DIR: tempDir,
      FN_KNOCK_GATEWAY_CONFIG_DIR: gatewayConfigDir,
      FN_KNOCK_RUNTIME_TARGET: runtimeTarget,
      FN_KNOCK_SQLITE_PATH: path.join(tempDir, "state.sqlite3"),
      GO_BACKEND_PORT: String(goBackendPort),
      RUST_LOG: "error",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });

  const startup = new AbortController();
  child.on("error", (error) => startup.abort(error));
  gateway.on("error", (error) => startup.abort(error));
  let output = "";
  const appendOutput = (chunk) => {
    output = `${output}${chunk}`.slice(-12_000);
  };
  child.stdout.on("data", appendOutput);
  child.stderr.on("data", appendOutput);
  gateway.stdout.on("data", appendOutput);
  gateway.stderr.on("data", appendOutput);

  const backendUrl = `http://127.0.0.1:${backendPort}`;
  const adminUrl = protectedAdmin
    ? `http://127.0.0.1:${adminViewPort}`
    : backendUrl;
  const authUrl = `http://127.0.0.1:${authPort}`;
  const startedAt = performance.now();
  try {
    await Promise.all([
      waitForHttp(`${backendUrl}/__fn-knock/readyz`, 60_000, startup.signal),
      waitForHttp(adminUrl, 60_000, startup.signal),
      waitForHttp(authUrl, 60_000, startup.signal),
    ]);
    const readinessMs = Math.round(performance.now() - startedAt);
    if (collectMetrics) {
      await configurePerformanceRoute(backendUrl, loadUpstream.url);
    }
    const metrics = collectMetrics
      ? {
          readiness_ms: readinessMs,
          ...(await collectRuntimeRSS(backendUrl, child.pid, gateway.pid)),
        }
      : undefined;

    return {
      adminUrl,
      authUrl,
      backendUrl,
      gatewayProxyUrl: `http://127.0.0.1:${goProxyPort}`,
      metrics,
      readinessMs,
      collectCheckpoint: (signal) =>
        collectRuntimeCheckpoint(backendUrl, child.pid, gateway.pid, signal),
      // Keep fast RSS sampling independent of health RPCs, SQLite and lsof.
      collectMemorySample: async (signal) => {
        const memory = await readProcessMemory(child.pid, signal);
        if (!memory)
          return collectRuntimeCheckpoint(
            backendUrl,
            child.pid,
            gateway.pid,
            signal,
          );
        return {
          captured_at: new Date().toISOString(),
          management_rss_bytes: memory.rss_bytes,
          management_peak_rss_bytes: memory.peak_rss_bytes,
        };
      },
      reclaimGatewayMemory: async () => {
        const response = await fetchRuntime(
          `${backendUrl}/api/admin/runtime-health/gateway-memory/reclaim`,
          {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: "{}",
          },
        );
        if (!response.ok) {
          throw new Error(
            `gateway memory reclaim failed: HTTP ${response.status}`,
          );
        }
        return response.json();
      },
      stop: async () => {
        await Promise.all([stopChild(child), stopChild(gateway)]);
        await loadUpstream.stop();
        await rm(tempDir, { recursive: true, force: true });
      },
    };
  } catch (error) {
    startup.abort(error);
    await Promise.all([stopChild(child), stopChild(gateway)]);
    await loadUpstream.stop();
    await rm(tempDir, { recursive: true, force: true });
    throw new Error(`${error.message}\n${output}`);
  }
};
