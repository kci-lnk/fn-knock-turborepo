import { Elysia, t } from "elysia";
import { redis } from "../lib/redis";
import { frpManager } from "../lib/frp-manager";
import path from "node:path";
import fs from "node:fs";
import { randomUUID } from "node:crypto";
import { spawn } from "node:child_process";
import { dataPath } from '../lib/AppDirManager';
import { markTunnelRunning, markTunnelStopped, shouldResumeTunnel } from "../lib/tunnel-runtime-state";
import { DEFAULT_REDIS_LOG_BUFFER_MAX_LEN, RedisLogBuffer } from "../lib/redis-log-buffer";
import { collectStreamOutput, sleep, waitForProcessExit } from "../lib/runtime";

type RunState = {
  proc?: ReturnType<typeof spawn>;
  running: boolean;
  pid?: number;
};

class FrpcConfigValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FrpcConfigValidationError";
  }
}

const LOG_KEY = "fn_knock:frpc:logs";
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

const FRPC_DIR = path.join(dataPath, "frp");
const FRPC_TOML = path.join(FRPC_DIR, "frpc.toml");
const FRPC_WEB_STATUS_URL = "http://127.0.0.1:7995/api/status";
const FRPC_DEFAULT_WEB_USER = process.env.FRPC_WEB_USER || "admin";
const FRPC_DEFAULT_WEB_PASSWORD = process.env.FRPC_WEB_PASSWORD || randomUUID();

function defaultFrpcTemplate(): string {
  const localPort = process.env.GO_REPROXY_PORT || "7999";
  return [
    'serverAddr = ""',
    "serverPort = 7000",
    "",
    'webServer.addr = "127.0.0.1"',
    "webServer.port = 7995",
    `webServer.user = "${FRPC_DEFAULT_WEB_USER}"`,
    `webServer.password = "${FRPC_DEFAULT_WEB_PASSWORD}"`,
    "",
    "[auth]",
    'method = "token"',
    'token = ""',
    "",
    "[[proxies]]",
    'name = "reproxy"',
    'type = "tcp"',
    'localIP = "127.0.0.1"',
    `localPort = ${localPort}`,
    "remotePort = 7999",
    'transport.proxyProtocolVersion = "v2"',
    ""
  ].join("\n");
}

const escapeRegex = (value: string) => value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

const extractTomlString = (content: string, key: string): string | null => {
  const pattern = new RegExp(`^\\s*${escapeRegex(key)}\\s*=\\s*["']([^"']*)["']\\s*$`, "m");
  const match = content.match(pattern);
  return match?.[1] ?? null;
};

async function getFrpcWebAuthorizationHeader(): Promise<string | null> {
  const content = await readConfig();
  const user = extractTomlString(content, "webServer.user");
  const password = extractTomlString(content, "webServer.password");
  if (!user || !password) return null;
  const encoded = Buffer.from(`${user}:${password}`).toString("base64");
  return `Basic ${encoded}`;
}

type FrpcWebStatus = {
  statusCode: number;
  tcp: any[];
};

const extractTomlNumber = (content: string, key: string): number | null => {
  const pattern = new RegExp(`^\\s*${escapeRegex(key)}\\s*=\\s*(\\d+)\\s*$`, "m");
  const match = content.match(pattern);
  if (!match?.[1]) return null;
  const parsed = Number.parseInt(match[1], 10);
  if (!Number.isFinite(parsed) || parsed <= 0 || parsed > 65535) return null;
  return parsed;
};

const runCommand = async (
  args: string[],
  options?: { cwd?: string },
): Promise<{ exitCode: number; stdout: string; stderr: string }> => {
  const [command, ...commandArgs] = args;
  if (!command) {
    throw new Error("missing command");
  }
  const proc = spawn(command, commandArgs, {
    cwd: options?.cwd,
    stdio: ["ignore", "pipe", "pipe"],
  });
  // Attach process error/exit listeners immediately to catch ENOENT and similar spawn failures.
  const exitPromise = waitForProcessExit(proc);
  const [stdout, stderr, exitCode] = await Promise.all([
    collectStreamOutput(proc.stdout),
    collectStreamOutput(proc.stderr),
    exitPromise,
  ]);
  return { exitCode, stdout, stderr };
};

const parsePidLines = (text: string): number[] => {
  const out: number[] = [];
  for (const line of text.split(/\r?\n/)) {
    const v = Number.parseInt(line.trim(), 10);
    if (Number.isFinite(v) && v > 0) out.push(v);
  }
  return out;
};

const parseSsPids = (text: string): number[] => {
  const found = new Set<number>();
  const re = /pid=(\d+)/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const pid = Number.parseInt(m[1] || "", 10);
    if (Number.isFinite(pid) && pid > 0) found.add(pid);
  }
  return [...found];
};

const parsePsPidAndCommand = (text: string): Array<{ pid: number; command: string }> => {
  const out: Array<{ pid: number; command: string }> = [];
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const match = trimmed.match(/^(\d+)\s+(.+)$/);
    if (!match) continue;
    const pid = Number.parseInt(match[1] || "", 10);
    const command = match[2] || "";
    if (!Number.isFinite(pid) || pid <= 0) continue;
    out.push({ pid, command });
  }
  return out;
};

const isProcessAlive = (pid: number): boolean => {
  if (!Number.isFinite(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const terminatePid = async (pid: number): Promise<void> => {
  if (!Number.isFinite(pid) || pid <= 0 || pid === process.pid) return;
  if (!isProcessAlive(pid)) return;

  try {
    process.kill(pid, "SIGTERM");
  } catch {}

  for (let i = 0; i < 20; i++) {
    if (!isProcessAlive(pid)) return;
    await sleep(100);
  }

  try {
    process.kill(pid, "SIGKILL");
  } catch {}
};

const isFrpcProcess = async (pid: number): Promise<boolean> => {
  try {
    const res = await runCommand(["ps", "-p", String(pid), "-o", "command="]);
    if (res.exitCode !== 0) return false;
    return /\bfrpc(\s|$)/.test(res.stdout);
  } catch {
    return false;
  }
};

const findListeningPids = async (port: number): Promise<number[]> => {
  try {
    const lsof = await runCommand(["lsof", "-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"]);
    if (lsof.exitCode === 0 && lsof.stdout.trim()) {
      return [...new Set(parsePidLines(lsof.stdout))];
    }
  } catch {
    // ignore command-not-found and fallback to ss
  }

  try {
    const ss = await runCommand(["ss", "-ltnp", `sport = :${port}`]);
    if (ss.exitCode === 0 && ss.stdout.trim()) {
      return [...new Set(parseSsPids(ss.stdout))];
    }
  } catch {
    // ignore command-not-found
  }

  return [];
};

const findFrpcPidsByConfig = async (): Promise<number[]> => {
  try {
    const pgrep = await runCommand(["pgrep", "-f", `frpc.*${FRPC_TOML}`]);
    if (pgrep.exitCode === 0 && pgrep.stdout.trim()) {
      return [...new Set(parsePidLines(pgrep.stdout))];
    }
  } catch {
    // ignore command-not-found and fallback to ps
  }

  try {
    const ps = await runCommand(["ps", "-eo", "pid=,command="]);
    if (ps.exitCode !== 0 || !ps.stdout.trim()) return [];
    const rows = parsePsPidAndCommand(ps.stdout);
    return [...new Set(
      rows
        .filter(row => /\bfrpc(\s|$)/.test(row.command) && row.command.includes(FRPC_TOML))
        .map(row => row.pid),
    )];
  } catch {
    return [];
  }
};

const releaseFrpcWebPortIfNeeded = async (): Promise<void> => {
  const content = await readConfig();
  const webPort = extractTomlNumber(content, "webServer.port") || 7995;
  let occupiedPids = await findListeningPids(webPort);
  if (!occupiedPids.length) {
    const frpcByConfig = await findFrpcPidsByConfig();
    if (frpcByConfig.length) {
      occupiedPids = frpcByConfig;
      await appendLogs([`preflight: 未检测到 ${webPort} 监听信息，按配置路径发现 frpc 进程 pid=${occupiedPids.join(",")}`]);
    }
  }
  if (!occupiedPids.length) return;

  await appendLogs([`preflight: 检测到 ${webPort} 端口占用，尝试释放 pid=${occupiedPids.join(",")}`]);
  for (const pid of occupiedPids) {
    const ownedByFrpc = await isFrpcProcess(pid);
    if (!ownedByFrpc) {
      await appendLogs([`preflight warning: ${webPort} 端口占用进程不是 frpc，已跳过自动终止 pid=${pid}`]);
      continue;
    }
    await terminatePid(pid);
  }

  await sleep(200);
  const remains = await findListeningPids(webPort);
  if (remains.length > 0) {
    throw new Error(`FRPC 管理端口 ${webPort} 仍被占用 (pid=${remains.join(",")})`);
  }
  await appendLogs([`preflight: 已释放 ${webPort} 端口`]);
};

async function fetchFrpcWebStatus(timeoutMs: number): Promise<FrpcWebStatus> {
  const controller = new AbortController();
  const to = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const authorization = await getFrpcWebAuthorizationHeader();
    const headers: Record<string, string> = {};
    if (authorization) headers.Authorization = authorization;
    const res = await fetch(FRPC_WEB_STATUS_URL, {
      method: "GET",
      headers,
      signal: controller.signal,
    });
    const data = await res.json().catch(() => ({}));
    const tcp = res.ok && Array.isArray(data?.tcp) ? data.tcp : [];
    return { statusCode: res.status, tcp };
  } finally {
    clearTimeout(to);
  }
}

const buildFrpcRealtimeStatus = async () => {
  let tcp: any[] = [];
  try {
    const data = await fetchFrpcWebStatus(1500);
    tcp = data.tcp;
  } catch {
    tcp = [];
  }

  return {
    running: runState.running || tcp.length > 0,
    pid: runState.pid || null,
    tcp,
  };
};

async function readConfig(): Promise<string> {
  if (!fs.existsSync(FRPC_DIR)) fs.mkdirSync(FRPC_DIR, { recursive: true });
  if (!fs.existsSync(FRPC_TOML)) {
    const content = defaultFrpcTemplate();
    fs.writeFileSync(FRPC_TOML, content, "utf-8");
    return content;
  }
  return fs.readFileSync(FRPC_TOML, "utf-8");
}

async function writeConfig(content: string): Promise<void> {
  if (!fs.existsSync(FRPC_DIR)) fs.mkdirSync(FRPC_DIR, { recursive: true });
  fs.writeFileSync(FRPC_TOML, content, "utf-8");
}

const normalizeVerifyOutput = (value: string): string => {
  const normalized = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n");
  if (normalized.length <= 4000) return normalized;
  return `${normalized.slice(0, 4000)}...`;
};

const formatVerifyFailureMessage = (result: { exitCode: number; stdout: string; stderr: string }): string => {
  const detail = [result.stderr, result.stdout]
    .map(normalizeVerifyOutput)
    .filter(Boolean)
    .join("\n");
  if (detail) {
    return `frpc verify 校验失败：${detail}`;
  }
  return `frpc verify 校验失败，退出码 ${result.exitCode}`;
};

async function verifyFrpcConfig(content: string): Promise<void> {
  let bin: string;
  try {
    bin = frpManager.getExecutable("frpc");
  } catch {
    throw new FrpcConfigValidationError("FRP 未初始化，无法校验 frpc.toml，请先在系统设置中下载 FRP 资源。");
  }

  if (!fs.existsSync(FRPC_DIR)) fs.mkdirSync(FRPC_DIR, { recursive: true });

  const tempPath = path.join(FRPC_DIR, `frpc.verify.${randomUUID()}.toml`);
  try {
    fs.writeFileSync(tempPath, content, "utf-8");
    const result = await runCommand([bin, "verify", "-c", tempPath], { cwd: FRPC_DIR });
    if (result.exitCode !== 0) {
      throw new FrpcConfigValidationError(formatVerifyFailureMessage(result));
    }
  } catch (error) {
    if (error instanceof FrpcConfigValidationError) {
      throw error;
    }
    const message = error instanceof Error ? error.message : String(error);
    throw new FrpcConfigValidationError(`frpc verify 校验失败：${message}`);
  } finally {
    try {
      if (fs.existsSync(tempPath)) fs.unlinkSync(tempPath);
    } catch {}
  }
}

async function startFrpc(opts?: { releaseWebPort?: boolean }): Promise<{ pid: number }> {
  if (runState.running && runState.proc && runState.proc.exitCode === null && !runState.proc.killed) {
    return { pid: runState.proc.pid ?? 0 };
  }
  if (opts?.releaseWebPort) {
    await releaseFrpcWebPortIfNeeded();
  }
  const bin = frpManager.getExecutable("frpc");
  const proc = spawn(bin, ["-c", FRPC_TOML], {
    cwd: FRPC_DIR,
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
    throw new Error(`启动 frpc 失败: ${detail}`);
  }
  runState.proc = proc;
  runState.running = true;
  runState.pid = proc.pid;
  try {
    await markTunnelRunning("frp");
  } catch (e) {
    console.error("Failed to persist frpc running state:", e);
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
      await appendLogs([`frpc stdout read error: ${String(e)}`]);
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
        await appendLogs(parts.map(l => `[ERR] ${l}`));
      }
      if (buf) await appendLogs([`[ERR] ${buf}`]);
    } catch (e) {
      await appendLogs([`frpc stderr read error: ${String(e)}`]);
    }
  })();

  void (async () => {
    try {
      const code = await exitPromise;
      await appendLogs([`frpc exited with code ${code}`]);
    } catch (e: any) {
      await appendLogs([`frpc process error: ${e?.message || String(e)}`]);
    }

    if (runState.proc !== proc) return;
    runState.proc = undefined;
    runState.running = false;
    runState.pid = undefined;
    try {
      await markTunnelStopped("frp");
    } catch (e) {
      console.error("Failed to persist frpc stopped state:", e);
    }
  })();

  await appendLogs([`frpc started pid=${proc.pid ?? 0}`]);
  return { pid: proc.pid ?? 0 };
}

async function stopFrpc(): Promise<void> {
  const proc = runState.proc;
  if (proc && proc.exitCode === null && !proc.killed) {
    proc.kill();
  }
  runState.proc = undefined;
  runState.running = false;
  runState.pid = undefined;
  try {
    await markTunnelStopped("frp");
  } catch (e) {
    console.error("Failed to persist frpc stopped state:", e);
  }
}

export async function restoreFrpcOnBoot(): Promise<void> {
  const shouldResume = await shouldResumeTunnel("frp");
  if (!shouldResume) return;
  try {
    await appendLogs(["resume: 检测到 FRP 上次为开启状态，正在自动恢复..."]);
    await startFrpc({ releaseWebPort: true });
  } catch (e: any) {
    const msg = e?.message || String(e) || "未知错误";
    await appendLogs([`resume error: ${msg}`]);
  }
}

export const frpcRoutes = new Elysia({ prefix: "/api/admin/frpc" })
  .get("/status", async () => {
    const st = frpManager.getStatus();
    const localDefault = process.env.GO_REPROXY_PORT || "7999";
    return {
      success: true,
      data: {
        initialized: st.downloaded,
        platform: st.platform,
        running: runState.running,
        pid: runState.pid || null,
        config_path: FRPC_TOML,
        defaults: { local_port: localDefault }
      }
    };
  })
  .get("/overview", async ({ query, set }) => {
    const limit = Math.max(1, Math.min(parseInt((query.limit as any) || "200", 10), logBuffer.getMaxLen()));
    try {
      const { tcp } = await fetchFrpcWebStatus(2000);
      const logs = await logBuffer.list(limit);
      return { success: true, data: { tcp, logs } };
    } catch {
      const logs = await logBuffer.list(limit);
      set.status = 200;
      return { success: true, data: { tcp: [], logs } };
    }
  })
  .get("/web-status", async ({ set }) => {
    try {
      const { statusCode, tcp } = await fetchFrpcWebStatus(2000);
      if (statusCode >= 400) {
        set.status = statusCode;
        return { success: false, message: `HTTP ${statusCode}` };
      }
      return { success: true, data: { tcp } };
    } catch (e: any) {
      set.status = 502;
      return { success: false, message: "unreachable" };
    }
  })
  .get("/config", async () => {
    const content = await readConfig();
    return { success: true, data: { content } };
  })
  .post("/config", async ({ body, set }) => {
    try {
      await verifyFrpcConfig(body.content);
      await writeConfig(body.content);
      return { success: true };
    } catch (e: any) {
      const message = e?.message || "保存配置失败";
      set.status = e instanceof FrpcConfigValidationError ? 400 : 500;
      return { success: false, message };
    }
  }, {
    body: t.Object({ content: t.String() })
  })
  .post("/start", async ({ set }) => {
    const st = frpManager.getStatus();
    if (!st.downloaded) {
      set.status = 400;
      return { success: false, message: "FRP 未初始化" };
    }
    try {
      const { pid } = await startFrpc({ releaseWebPort: true });
      return { success: true, data: { pid } };
    } catch (e: any) {
      const msg = e?.message || "启动失败";
      await appendLogs([`start error: ${msg}`]);
      set.status = 500;
      return { success: false, message: msg };
    }
  })
  .post("/stop", async () => {
    await stopFrpc();
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
    const status = await buildFrpcRealtimeStatus();

    return {
      success: true,
      data: {
        cursor,
        reset,
        logs,
        status,
      },
    };
  }, {
    query: t.Object({
      cursor: t.Optional(t.String()),
    }),
  });
