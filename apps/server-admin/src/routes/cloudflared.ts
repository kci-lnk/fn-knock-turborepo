import { Elysia, t } from "elysia";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { redis } from "../lib/redis";
import { cloudflaredManager } from "../lib/cloudflared-manager";
import path from "node:path";
import fs from "node:fs";
import { spawn } from "node:child_process";
import { dataPath } from "../lib/AppDirManager";
import {
  markTunnelRunning,
  markTunnelStopped,
  shouldResumeTunnel,
} from "../lib/tunnel-runtime-state";
import {
  DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  RedisLogBuffer,
} from "../lib/redis-log-buffer";
import { waitForProcessExit } from "../lib/runtime";
import { emitTunnelConnectivityEvent } from "../lib/system-events/helpers";

type RunState = {
  proc?: ReturnType<typeof spawn>;
  running: boolean;
  pid?: number;
};

type TunnelConnectionState = {
  connected: boolean;
  stopRequested: boolean;
};

const LOG_KEY = "fn_knock:cloudflared:logs";
const LOG_TTL_SEC = 24 * 3600;
const logBuffer = new RedisLogBuffer(redis, {
  key: LOG_KEY,
  ttlSeconds: LOG_TTL_SEC,
  maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
});

const runState: RunState = { running: false };
const connectionState: TunnelConnectionState = {
  connected: false,
  stopRequested: false,
};

const CLOUDFLARED_CONNECTED_PATTERNS = [
  /\bregistered tunnel connection\b/i,
  /\bconnection [0-9a-f-]+ registered\b/i,
] as const;

const CLOUDFLARED_DISCONNECTED_PATTERNS = [
  /\bserve tunnel error\b/i,
  /\btunnel disconnected\b/i,
  /\bfailed to serve tunnel\b/i,
] as const;

const normalizeTunnelEventMessage = (line: string) => {
  const normalized = line.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (normalized.length <= 240) return normalized;
  return `${normalized.slice(0, 240).trim()}...`;
};

const emitCloudflaredConnectivity = async (
  connected: boolean,
  message?: string,
  pid?: number | null,
) => {
  if (connected) {
    if (connectionState.connected) return;
    connectionState.connected = true;
  } else {
    if (!connectionState.connected) return;
    connectionState.connected = false;
    if (connectionState.stopRequested) return;
  }

  await emitTunnelConnectivityEvent({
    tunnel: "cloudflared",
    connected,
    pid:
      typeof pid === "number" && Number.isFinite(pid)
        ? pid
        : (runState.pid ?? null),
    ...(message ? { message } : {}),
  });
};

const handleCloudflaredRuntimeSignals = async (lines: string[]) => {
  for (const rawLine of lines) {
    const line = normalizeTunnelEventMessage(rawLine);
    if (!line) continue;

    if (CLOUDFLARED_CONNECTED_PATTERNS.some((pattern) => pattern.test(line))) {
      await emitCloudflaredConnectivity(true, line);
      continue;
    }

    if (
      CLOUDFLARED_DISCONNECTED_PATTERNS.some((pattern) => pattern.test(line))
    ) {
      await emitCloudflaredConnectivity(false, line);
    }
  }
};

const appendLogs = async (lines: string[]) => {
  const normalizedLines = lines.map((line) => line.trimEnd()).filter(Boolean);
  if (!normalizedLines.length) return;
  await logBuffer.append(normalizedLines);
  await handleCloudflaredRuntimeSignals(normalizedLines);
};

const buildCloudflaredStatus = () => ({
  running: runState.running,
  pid: runState.pid || null,
});

const CLOUDFLARED_DIR = path.join(dataPath, "cloudflared");
const CLOUDFLARED_JSON = path.join(CLOUDFLARED_DIR, "cloudflared.json");

const CLOUDFLARED_PROTOCOLS = ["auto", "http2", "quic"] as const;
type CloudflaredProtocol = (typeof CLOUDFLARED_PROTOCOLS)[number];
type CloudflaredConfig = {
  token: string;
  protocol: CloudflaredProtocol;
};

const DEFAULT_CLOUDFLARED_CONFIG: CloudflaredConfig = {
  token: "",
  protocol: "auto",
};

const isCloudflaredProtocol = (
  value: unknown,
): value is CloudflaredProtocol =>
  typeof value === "string" &&
  CLOUDFLARED_PROTOCOLS.includes(value as CloudflaredProtocol);

const normalizeCloudflaredConfig = (raw: unknown): CloudflaredConfig => {
  if (!raw || typeof raw !== "object") return { ...DEFAULT_CLOUDFLARED_CONFIG };
  const obj = raw as Record<string, unknown>;
  return {
    token: typeof obj.token === "string" ? obj.token : "",
    protocol: isCloudflaredProtocol(obj.protocol) ? obj.protocol : "auto",
  };
};

const buildCloudflaredArgs = (config: CloudflaredConfig): string[] => {
  const args = ["tunnel", "--no-autoupdate"];
  if (config.protocol !== "auto") {
    args.push("--protocol", config.protocol);
  }
  args.push("run", "--token", config.token);
  return args;
};

async function readConfig(): Promise<CloudflaredConfig> {
  if (!fs.existsSync(CLOUDFLARED_DIR))
    fs.mkdirSync(CLOUDFLARED_DIR, { recursive: true });
  if (!fs.existsSync(CLOUDFLARED_JSON)) {
    fs.writeFileSync(
      CLOUDFLARED_JSON,
      JSON.stringify(DEFAULT_CLOUDFLARED_CONFIG, null, 2),
      "utf-8",
    );
    return { ...DEFAULT_CLOUDFLARED_CONFIG };
  }
  try {
    const data = JSON.parse(fs.readFileSync(CLOUDFLARED_JSON, "utf-8"));
    return normalizeCloudflaredConfig(data);
  } catch {
    return { ...DEFAULT_CLOUDFLARED_CONFIG };
  }
}

async function writeConfig(config: CloudflaredConfig): Promise<void> {
  if (!fs.existsSync(CLOUDFLARED_DIR))
    fs.mkdirSync(CLOUDFLARED_DIR, { recursive: true });
  fs.writeFileSync(
    CLOUDFLARED_JSON,
    JSON.stringify(normalizeCloudflaredConfig(config), null, 2),
    "utf-8",
  );
}

async function startCloudflared(): Promise<{ pid: number }> {
  if (
    runState.running &&
    runState.proc &&
    runState.proc.exitCode === null &&
    !runState.proc.killed
  ) {
    return { pid: runState.proc.pid ?? 0 };
  }
  connectionState.stopRequested = false;
  const config = await readConfig();
  if (!config.token) {
    throw new Error("请先配置 Cloudflare Token");
  }

  const bin = cloudflaredManager.getExecutable();
  const proc = spawn(bin, buildCloudflaredArgs(config), {
    cwd: CLOUDFLARED_DIR,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const exitPromise = waitForProcessExit(proc);
  if (!proc.pid) {
    let detail = "spawn failed";
    try {
      await exitPromise;
    } catch (e: any) {
      detail = e?.message || String(e);
    }
    throw new Error(`启动 cloudflared 失败: ${detail}`);
  }
  runState.proc = proc;
  runState.running = true;
  runState.pid = proc.pid;
  try {
    await markTunnelRunning("cloudflared");
  } catch (e) {
    console.error("Failed to persist cloudflared running state:", e);
  }

  (async () => {
    if (!proc.stdout) return;
    let buf = "";
    try {
      for await (const chunk of proc.stdout) {
        buf += chunk.toString();
        const parts = buf.split(/\r?\n/);
        buf = parts.pop() || "";
        await appendLogs(parts);
      }
      if (buf) await appendLogs([buf]);
    } catch (e) {
      await appendLogs([`cloudflared stdout read error: ${String(e)}`]);
    }
  })();

  (async () => {
    if (!proc.stderr) return;
    let buf = "";
    try {
      for await (const chunk of proc.stderr) {
        buf += chunk.toString();
        const parts = buf.split(/\r?\n/);
        buf = parts.pop() || "";
        await appendLogs(parts); // cloudflared often outputs normal logs to stderr
      }
      if (buf) await appendLogs([buf]);
    } catch (e) {
      await appendLogs([`cloudflared stderr read error: ${String(e)}`]);
    }
  })();

  void (async () => {
    let exitMessage = "cloudflared 进程已退出";
    try {
      const code = await exitPromise;
      exitMessage = `cloudflared 进程已退出（退出码 ${code}）`;
      await appendLogs([`cloudflared exited with code ${code}`]);
    } catch (e: any) {
      exitMessage = `cloudflared 进程异常退出：${e?.message || String(e)}`;
      await appendLogs([
        `cloudflared process error: ${e?.message || String(e)}`,
      ]);
    }

    const expectedStop = connectionState.stopRequested;
    if (runState.proc !== proc) return;
    runState.proc = undefined;
    runState.running = false;
    runState.pid = undefined;
    try {
      await markTunnelStopped("cloudflared");
    } catch (e) {
      console.error("Failed to persist cloudflared stopped state:", e);
    }
    if (!expectedStop) {
      await emitCloudflaredConnectivity(false, exitMessage, proc.pid ?? 0);
    }
    connectionState.stopRequested = false;
  })();

  await appendLogs([`cloudflared started pid=${proc.pid ?? 0}`]);
  return { pid: proc.pid ?? 0 };
}

async function stopCloudflared(): Promise<void> {
  const proc = runState.proc;
  connectionState.stopRequested = true;
  connectionState.connected = false;
  if (proc && proc.exitCode === null && !proc.killed) {
    proc.kill();
  }
  runState.proc = undefined;
  runState.running = false;
  runState.pid = undefined;
  try {
    await markTunnelStopped("cloudflared");
  } catch (e) {
    console.error("Failed to persist cloudflared stopped state:", e);
  }
  if (!proc || proc.exitCode !== null || proc.killed) {
    connectionState.stopRequested = false;
  }
}

export async function restoreCloudflaredOnBoot(): Promise<void> {
  const shouldResume = await shouldResumeTunnel("cloudflared");
  if (!shouldResume) return;
  try {
    await appendLogs([
      "resume: 检测到 Cloudflared 上次为开启状态，正在自动恢复...",
    ]);
    await startCloudflared();
  } catch (e: any) {
    const msg = e?.message || String(e) || "未知错误";
    await appendLogs([`resume error: ${msg}`]);
  }
}

export const cloudflaredRoutes = new Elysia({
  prefix: "/api/admin/cloudflared",
  tags: ["Tunnel - Cloudflared"],
})
  .get(
    "/status",
    async () => {
      const st = cloudflaredManager.getStatus();
      return {
        success: true,
        data: {
          initialized: st.downloaded,
          platform: st.platform,
          running: runState.running,
          pid: runState.pid || null,
        },
      };
    },
    routeDoc("获取 Cloudflared 运行状态"),
  )
  .get(
    "/config",
    async () => {
      const config = await readConfig();
      return { success: true, data: config };
    },
    routeDoc("获取 Cloudflared 配置"),
  )
  .post(
    "/config",
    async ({ body }) => {
      await writeConfig({
        token: body.token,
        protocol: body.protocol ?? "auto",
      });
      return { success: true };
    },
    withRouteDoc("保存 Cloudflared 配置", {
      body: t.Object({
        token: t.String(),
        protocol: t.Optional(
          t.Union([t.Literal("auto"), t.Literal("http2"), t.Literal("quic")]),
        ),
      }),
    }),
  )
  .post(
    "/start",
    async ({ set }) => {
      const st = cloudflaredManager.getStatus();
      if (!st.downloaded) {
        set.status = 400;
        return { success: false, message: "Cloudflared 未初始化" };
      }
      try {
        const { pid } = await startCloudflared();
        return { success: true, data: { pid } };
      } catch (e: any) {
        const msg = e?.message || "启动失败";
        await appendLogs([`start error: ${msg}`]);
        set.status = 500;
        return { success: false, message: msg };
      }
    },
    routeDoc("启动 Cloudflared"),
  )
  .post(
    "/stop",
    async () => {
      await stopCloudflared();
      return { success: true };
    },
    routeDoc("停止 Cloudflared"),
  )
  .get(
    "/logs",
    async ({ query }) => {
      const limit = Math.max(
        1,
        Math.min(
          parseInt((query.limit as any) || "200", 10),
          logBuffer.getMaxLen(),
        ),
      );
      const logs = await logBuffer.list(limit);
      return { success: true, data: logs };
    },
    routeDoc("获取 Cloudflared 日志"),
  )
  .delete(
    "/logs",
    async () => {
      await logBuffer.clear();
      return { success: true };
    },
    routeDoc("清空 Cloudflared 日志"),
  )
  .get(
    "/poll",
    async ({ query }) => {
      const { cursor, reset, items: logs } = await logBuffer.poll(query.cursor);

      return {
        success: true,
        data: {
          cursor,
          reset,
          logs,
          status: buildCloudflaredStatus(),
        },
      };
    },
    withRouteDoc("轮询 Cloudflared 日志与状态", {
      query: t.Object({
        cursor: t.Optional(t.String()),
      }),
    }),
  );
