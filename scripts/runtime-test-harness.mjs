import { execFile, spawn } from "node:child_process";
import { access, mkdtemp, readFile, rm } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";

const execFileAsync = promisify(execFile);
const idleMeasurementDelayMs = 1_000;
const gatewayMetricTimeoutMs = 7_000;

export const rootDir = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

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

export const waitForHttp = async (url, timeoutMs = 60_000) => {
  const deadline = Date.now() + timeoutMs;
  let lastError;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
      lastError = new Error(`${url} returned ${response.status}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw lastError || new Error(`Timed out waiting for ${url}`);
};

const ensureRuntimeArtifacts = async (serverBinary) => {
  const required = [
    serverBinary,
    path.join(rootDir, "apps/server-admin-view/dist/index.html"),
    path.join(rootDir, "apps/server-auth-view/dist/index.html"),
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

const buildPinnedGateway = async (output) => {
  const gatewayDir = path.join(rootDir, "..", "Go-Reauth-Proxy");
  await access(path.join(gatewayDir, ".git"));
  const manifest = JSON.parse(
    await readFile(path.join(rootDir, "version.json"), "utf8"),
  );
  const { stdout } = await execFileAsync("git", ["rev-parse", "HEAD"], {
    cwd: gatewayDir,
  });
  const actualCommit = stdout.trim().toLowerCase();
  if (actualCommit !== manifest.gatewayCommit) {
    throw new Error(
      `Go gateway HEAD ${actualCommit} does not match version.json ` +
        `gatewayCommit ${manifest.gatewayCommit}`,
    );
  }
  await execFileAsync(
    "go",
    [
      "build",
      "-trimpath",
      "-ldflags",
      `-s -w -X go-reauth-proxy/pkg/version.Version=${manifest.version} ` +
        `-X go-reauth-proxy/pkg/version.Commit=${manifest.gatewayCommit}`,
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
      return await buildPinnedGateway(path.join(tempDir, "go-reauth-proxy"));
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

const stopChild = async (child) => {
  if (child.exitCode !== null) return;
  child.kill("SIGTERM");
  await Promise.race([
    new Promise((resolve) => child.once("exit", resolve)),
    new Promise((resolve) => setTimeout(resolve, 3_000)),
  ]);
  if (child.exitCode === null) child.kill("SIGKILL");
};

const numberOrNull = (value) =>
  typeof value === "number" && Number.isFinite(value) && value >= 0
    ? Math.round(value)
    : null;

const collectRuntimeRSS = async (backendUrl) => {
  await new Promise((resolve) => setTimeout(resolve, idleMeasurementDelayMs));
  const deadline = Date.now() + gatewayMetricTimeoutMs;
  let lastStatus = "no response";
  while (Date.now() < deadline) {
    const response = await fetch(`${backendUrl}/api/admin/runtime-health`);
    if (!response.ok) {
      lastStatus = `HTTP ${response.status}`;
    } else {
      const payload = await response.json();
      const components = payload?.data?.components;
      const managementRSS = numberOrNull(components?.management?.rss_bytes);
      const gatewayRSS = numberOrNull(components?.gateway_process?.rss_bytes);
      if (managementRSS !== null && gatewayRSS !== null) {
        return {
          management_rss_bytes: managementRSS,
          gateway_rss_bytes: gatewayRSS,
        };
      }
      lastStatus = `management RSS ${managementRSS ?? "missing"}, gateway RSS ${gatewayRSS ?? "missing"}`;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Runtime health metrics did not stabilize: ${lastStatus}`);
};

export const startRuntime = async ({
  externalAdminUrl,
  externalAuthUrl,
  gatewayBinary,
  serverBinary = process.env.FN_KNOCK_RUNTIME_SERVER_BIN,
  protectedAdmin = false,
  collectMetrics = !protectedAdmin,
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
  await ensureRuntimeArtifacts(resolvedServerBinary);
  const tempDir = await mkdtemp(path.join(os.tmpdir(), tempPrefix));
  let resolvedGatewayBinary;
  try {
    resolvedGatewayBinary = await resolveGatewayBinary(gatewayBinary, tempDir);
  } catch (error) {
    await rm(tempDir, { recursive: true, force: true });
    throw error;
  }
  const selectedPorts = new Set();
  const portCount = protectedAdmin ? 5 : 4;
  while (selectedPorts.size < portCount) selectedPorts.add(await getFreePort());
  const [backendPort, authPort, goBackendPort, goProxyPort, adminViewPort] =
    selectedPorts;
  const gatewayConfigDir = path.join(tempDir, "gateway");
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
      gatewayConfigDir,
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
      ADMIN_STATIC_PATH: path.join(rootDir, "apps/server-admin-view/dist"),
      ...(protectedAdmin
        ? {
            ADMIN_VIEW_HOST: "127.0.0.1",
            ADMIN_VIEW_PORT: String(adminViewPort),
          }
        : {}),
      AUTH_HOST: "127.0.0.1",
      AUTH_PORT: String(authPort),
      AUTH_STATIC_PATH: path.join(rootDir, "apps/server-auth-view/dist"),
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
      waitForHttp(`${backendUrl}/__fn-knock/readyz`),
      waitForHttp(adminUrl),
      waitForHttp(authUrl),
    ]);
    const readinessMs = Math.round(performance.now() - startedAt);
    const metrics = collectMetrics
      ? { readiness_ms: readinessMs, ...(await collectRuntimeRSS(backendUrl)) }
      : undefined;

    return {
      adminUrl,
      authUrl,
      backendUrl,
      metrics,
      stop: async () => {
        await Promise.all([stopChild(child), stopChild(gateway)]);
        await rm(tempDir, { recursive: true, force: true });
      },
    };
  } catch (error) {
    await Promise.all([stopChild(child), stopChild(gateway)]);
    await rm(tempDir, { recursive: true, force: true });
    throw new Error(`${error.message}\n${output}`);
  }
};
