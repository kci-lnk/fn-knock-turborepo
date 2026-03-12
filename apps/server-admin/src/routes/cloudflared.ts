import { Elysia, t } from "elysia";
import { redis } from "../lib/redis";
import { cloudflaredManager } from "../lib/cloudflared-manager";
import path from "node:path";
import fs from "node:fs";
import { spawn } from "node:child_process";
import { dataPath } from '../lib/AppDirManager';
import { markTunnelRunning, markTunnelStopped, shouldResumeTunnel } from "../lib/tunnel-runtime-state";
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, RedisLogBuffer } from "../lib/redis-log-buffer";
import { waitForProcessExit } from "../lib/runtime";

type RunState = {
  proc?: ReturnType<typeof spawn>;
  running: boolean;
  pid?: number;
};

const LOG_KEY = "fn_knock:cloudflared:logs";
const LOG_TTL_SEC = 24 * 3600;
const logBuffer = new RedisLogBuffer(redis, {
  key: LOG_KEY,
  ttlSeconds: LOG_TTL_SEC,
  maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
});

const runState: RunState = { running: false };

const appendLogs = async (lines: string[]) => {
  await logBuffer.append(lines.map(l => l.trimEnd()));
};

const buildCloudflaredStatus = () => ({
  running: runState.running,
  pid: runState.pid || null,
});

const CLOUDFLARED_DIR = path.join(dataPath, "cloudflared");
const CLOUDFLARED_JSON = path.join(CLOUDFLARED_DIR, "cloudflared.json");

async function readConfig(): Promise<string> {
  if (!fs.existsSync(CLOUDFLARED_DIR)) fs.mkdirSync(CLOUDFLARED_DIR, { recursive: true });
  if (!fs.existsSync(CLOUDFLARED_JSON)) {
    const defaultTemplate = { token: "" };
    fs.writeFileSync(CLOUDFLARED_JSON, JSON.stringify(defaultTemplate, null, 2), "utf-8");
    return defaultTemplate.token;
  }
  try {
      const data = JSON.parse(fs.readFileSync(CLOUDFLARED_JSON, "utf-8"));
      return data.token || "";
  } catch {
      return "";
  }
}

async function writeConfig(token: string): Promise<void> {
  if (!fs.existsSync(CLOUDFLARED_DIR)) fs.mkdirSync(CLOUDFLARED_DIR, { recursive: true });
  fs.writeFileSync(CLOUDFLARED_JSON, JSON.stringify({ token }, null, 2), "utf-8");
}

async function startCloudflared(): Promise<{ pid: number }> {
  if (runState.running && runState.proc && runState.proc.exitCode === null && !runState.proc.killed) {
    return { pid: runState.proc.pid ?? 0 };
  }
  const token = await readConfig();
  if (!token) {
      throw new Error("请先配置 Cloudflare Token");
  }

  const bin = cloudflaredManager.getExecutable();
  const proc = spawn(bin, ["tunnel", "--no-autoupdate", "run", "--token", token], {
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
    try {
      const code = await exitPromise;
      await appendLogs([`cloudflared exited with code ${code}`]);
    } catch (e: any) {
      await appendLogs([`cloudflared process error: ${e?.message || String(e)}`]);
    }

    if (runState.proc !== proc) return;
    runState.proc = undefined;
    runState.running = false;
    runState.pid = undefined;
    try {
      await markTunnelStopped("cloudflared");
    } catch (e) {
      console.error("Failed to persist cloudflared stopped state:", e);
    }
  })();

  await appendLogs([`cloudflared started pid=${proc.pid ?? 0}`]);
  return { pid: proc.pid ?? 0 };
}

async function stopCloudflared(): Promise<void> {
  const proc = runState.proc;
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
}

export async function restoreCloudflaredOnBoot(): Promise<void> {
  const shouldResume = await shouldResumeTunnel("cloudflared");
  if (!shouldResume) return;
  try {
    await appendLogs(["resume: 检测到 Cloudflared 上次为开启状态，正在自动恢复..."]);
    await startCloudflared();
  } catch (e: any) {
    const msg = e?.message || String(e) || "未知错误";
    await appendLogs([`resume error: ${msg}`]);
  }
}

export const cloudflaredRoutes = new Elysia({ prefix: "/api/admin/cloudflared" })
  .get("/status", async () => {
    const st = cloudflaredManager.getStatus();
    return {
      success: true,
      data: {
        initialized: st.downloaded,
        platform: st.platform,
        running: runState.running,
        pid: runState.pid || null
      }
    };
  })
  .get("/config", async () => {
    const token = await readConfig();
    return { success: true, data: { token } };
  })
  .post("/config", async ({ body }) => {
    await writeConfig(body.token);
    return { success: true };
  }, {
    body: t.Object({ token: t.String() })
  })
  .post("/start", async ({ set }) => {
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
  })
  .post("/stop", async () => {
    await stopCloudflared();
    return { success: true };
  })
  .get("/logs", async ({ query }) => {
    const limit = Math.max(1, Math.min(parseInt((query.limit as any) || "200", 10), logBuffer.getMaxLen()));
    const logs = await logBuffer.list(limit);
    return { success: true, data: logs };
  })
  .delete("/logs", async () => {
    await logBuffer.clear();
    return { success: true };
  })
  .get("/poll", async ({ query }) => {
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
  }, {
    query: t.Object({
      cursor: t.Optional(t.String()),
    }),
  });
