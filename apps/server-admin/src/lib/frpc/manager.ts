import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { spawn, type ChildProcess } from "node:child_process";
import { frpManager } from "../frp-manager";
import { redis } from "../redis";
import { RedisLogBuffer } from "../redis-log-buffer";
import { collectStreamOutput, sleep, waitForProcessExit } from "../runtime";
import { tDefault } from "../i18n";
import {
  markTunnelRunning,
  markTunnelStopped,
  shouldResumeTunnel,
} from "../tunnel-runtime-state";
import { emitTunnelConnectivityEvent } from "../system-events/helpers";
import {
  FRPC_PRIMARY_INSTANCE_ID,
  type FrpcInstanceDetail,
  type FrpcInstanceMeta,
  type FrpcInstanceRuntime,
  type FrpcInstanceStatus,
  type FrpcInstanceSummary,
  type FrpcInstancesOverview,
} from "./types";
import {
  isFrpcProcessArgsForConfig,
  parseProcessCommandLine,
} from "./process-command";
import {
  mergeDetectedFrpcRuntime,
  shouldPersistDetectedFrpcRuntime,
} from "./runtime-reconcile";
import {
  KEY_PREFIX,
  defaultFrpcTemplate,
  defaultRuntime,
  ensureFrpcLayout,
  ensurePrimaryInstance,
  extraInstancePaths,
  getAllMetas,
  getPidPath,
  instanceKey,
  readInstanceIds,
  readMeta,
  readRuntime,
  sanitizeInstanceId,
  writeInstanceIds,
  writeMeta,
  writeRuntime,
} from "./instance-store";

const LOG_TTL_SEC = 24 * 3600;
const PRIMARY_LOG_MAX_LEN = 1000;
const EXTRA_LOG_MAX_LEN = 500;
const EXTRA_INSTANCE_LIMIT = 20;
const frpcT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.frpc.${key}`, params);

type AttachedProcess = {
  proc: ChildProcess;
};

type TunnelConnectionState = {
  connected: boolean;
  stopRequested: boolean;
};

export class FrpcConfigValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "FrpcConfigValidationError";
  }
}

export class FrpcInstanceNotFoundError extends Error {
  constructor(id: string) {
    super(frpcT("instanceNotFound", { id }));
    this.name = "FrpcInstanceNotFoundError";
  }
}

export class FrpcInstanceLimitError extends Error {
  constructor(limit: number) {
    super(frpcT("instanceLimitExceeded", { limit }));
    this.name = "FrpcInstanceLimitError";
  }
}

const attachedProcesses = new Map<string, AttachedProcess>();
const connectionStates = new Map<string, TunnelConnectionState>();
const logBuffers = new Map<string, RedisLogBuffer>();

const nowIso = () => new Date().toISOString();

const logKey = (id: string) => `${KEY_PREFIX}:instance:${id}:logs`;

const getLogBuffer = (id: string) => {
  const existing = logBuffers.get(id);
  if (existing) return existing;
  const buffer = new RedisLogBuffer(redis, {
    key: logKey(id),
    seqKey: `${logKey(id)}:seq`,
    ttlSeconds: LOG_TTL_SEC,
    maxLen:
      id === FRPC_PRIMARY_INSTANCE_ID ? PRIMARY_LOG_MAX_LEN : EXTRA_LOG_MAX_LEN,
  });
  logBuffers.set(id, buffer);
  return buffer;
};

const getMetaOrThrow = async (id: string): Promise<FrpcInstanceMeta> => {
  const safeId = sanitizeInstanceId(id);
  if (!safeId) throw new FrpcInstanceNotFoundError(id);
  await ensurePrimaryInstance();
  const meta = await readMeta(safeId);
  if (!meta) throw new FrpcInstanceNotFoundError(id);
  return meta;
};

const readPidFile = (pidPath: string): number | null => {
  try {
    if (!fs.existsSync(pidPath)) return null;
    const parsed = Number.parseInt(
      fs.readFileSync(pidPath, "utf-8").trim(),
      10,
    );
    if (!Number.isFinite(parsed) || parsed <= 0) return null;
    return parsed;
  } catch {
    return null;
  }
};

const writePidFile = (pidPath: string, pid: number) => {
  fs.mkdirSync(path.dirname(pidPath), { recursive: true });
  fs.writeFileSync(pidPath, `${pid}\n`, "utf-8");
};

const removePidFile = (pidPath: string) => {
  try {
    if (fs.existsSync(pidPath)) fs.unlinkSync(pidPath);
  } catch {}
};

const isProcessAlive = (pid: number): boolean => {
  if (!Number.isFinite(pid) || pid <= 0 || pid === process.pid) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

const runCommand = async (
  args: string[],
  options?: { cwd?: string },
): Promise<{ exitCode: number; stdout: string; stderr: string }> => {
  const [command, ...commandArgs] = args;
  if (!command) throw new Error("missing command");
  const proc = spawn(command, commandArgs, {
    cwd: options?.cwd,
    stdio: ["ignore", "pipe", "pipe"],
  });
  const exitPromise = waitForProcessExit(proc);
  const [stdout, stderr, exitCode] = await Promise.all([
    collectStreamOutput(proc.stdout),
    collectStreamOutput(proc.stderr),
    exitPromise,
  ]);
  return { exitCode, stdout, stderr };
};

const readProcCmdlineArgs = (pid: number): string[] | null => {
  try {
    const args = parseProcessCommandLine(
      fs.readFileSync(`/proc/${pid}/cmdline`),
    );
    return args.length > 0 ? args : null;
  } catch {
    return null;
  }
};

const readProcessCommand = async (pid: number): Promise<string | null> => {
  try {
    const result = await runCommand([
      "ps",
      "-ww",
      "-p",
      String(pid),
      "-o",
      "args=",
    ]);
    if (result.exitCode !== 0) return null;
    return result.stdout.trim() || null;
  } catch {
    return null;
  }
};

const readProcessArgs = async (pid: number): Promise<string[] | null> => {
  const procArgs = readProcCmdlineArgs(pid);
  if (procArgs) return procArgs;
  const command = await readProcessCommand(pid);
  if (!command) return null;
  const args = parseProcessCommandLine(command);
  return args.length > 0 ? args : null;
};

const isOwnedFrpcPid = async (
  pid: number | null | undefined,
  configPath: string,
): Promise<boolean> => {
  if (!pid || !isProcessAlive(pid)) return false;
  const args = await readProcessArgs(pid);
  return Boolean(args && isFrpcProcessArgsForConfig(args, configPath));
};

const isAttachedProcessAlive = (
  meta: FrpcInstanceMeta,
  pid?: number | null,
): boolean => {
  const proc = attachedProcesses.get(meta.id)?.proc;
  if (!proc?.pid) return false;
  if (pid && proc.pid !== pid) return false;
  return proc.exitCode === null && !proc.killed && isProcessAlive(proc.pid);
};

const findFrpcPidByConfigPath = async (
  configPath: string,
): Promise<number | null> => {
  let entries: string[];
  try {
    entries = fs.readdirSync("/proc");
  } catch {
    return null;
  }

  for (const entry of entries) {
    if (!/^\d+$/.test(entry)) continue;
    const pid = Number.parseInt(entry, 10);
    if (!Number.isFinite(pid) || pid <= 0 || pid === process.pid) continue;
    const args = readProcCmdlineArgs(pid);
    if (!args) continue;
    if (isFrpcProcessArgsForConfig(args, configPath) && isProcessAlive(pid)) {
      return pid;
    }
  }

  return null;
};

const appendLogs = async (
  meta: FrpcInstanceMeta,
  lines: string[],
  options?: { inspectSignals?: boolean },
) => {
  const normalizedLines = lines.map((line) => line.trimEnd()).filter(Boolean);
  if (!normalizedLines.length) return;
  await getLogBuffer(meta.id).append(normalizedLines);
  if (options?.inspectSignals !== false) {
    await handleFrpcRuntimeSignals(meta, normalizedLines);
  }
};

const FRPC_CONNECTED_PATTERNS = [
  /\blogin to server success\b/i,
  /\bstart proxy success\b/i,
] as const;

const FRPC_DISCONNECTED_PATTERNS = [
  /\bconnect to server error\b/i,
  /\blogin to the server failed\b/i,
  /\bsession shutdown\b/i,
] as const;

const normalizeTunnelEventMessage = (line: string) => {
  const normalized = line
    .replace(/^\[ERR\]\s*/i, "")
    .replace(/\s+/g, " ")
    .trim();
  if (!normalized) return "";
  if (normalized.length <= 240) return normalized;
  return `${normalized.slice(0, 240).trim()}...`;
};

const getConnectionState = (id: string): TunnelConnectionState => {
  const existing = connectionStates.get(id);
  if (existing) return existing;
  const next = { connected: false, stopRequested: false };
  connectionStates.set(id, next);
  return next;
};

const emitFrpcConnectivity = async (
  meta: FrpcInstanceMeta,
  connected: boolean,
  message?: string,
  pid?: number | null,
) => {
  const state = getConnectionState(meta.id);
  if (connected) {
    if (state.connected) return;
    state.connected = true;
  } else {
    if (!state.connected) return;
    state.connected = false;
    if (state.stopRequested) return;
  }

  await emitTunnelConnectivityEvent({
    tunnel: "frp",
    connected,
    pid,
    instanceId: meta.id,
    instanceName: meta.name,
    isPrimary: meta.isPrimary,
    ...(message ? { message: `${meta.name}: ${message}` } : {}),
  });
};

const handleFrpcRuntimeSignals = async (
  meta: FrpcInstanceMeta,
  lines: string[],
) => {
  for (const rawLine of lines) {
    const line = normalizeTunnelEventMessage(rawLine);
    if (!line) continue;

    if (FRPC_CONNECTED_PATTERNS.some((pattern) => pattern.test(line))) {
      await emitFrpcConnectivity(
        meta,
        true,
        line,
        attachedProcesses.get(meta.id)?.proc.pid ?? null,
      );
      continue;
    }

    if (FRPC_DISCONNECTED_PATTERNS.some((pattern) => pattern.test(line))) {
      await emitFrpcConnectivity(
        meta,
        false,
        line,
        attachedProcesses.get(meta.id)?.proc.pid ?? null,
      );
    }
  }
};

const extractTomlValue = (content: string, key: string): string | null => {
  const escaped = key.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = content.match(
    new RegExp(
      `^\\s*${escaped}\\s*=\\s*(?:"([^"]*)"|'([^']*)'|(\\d+))\\s*$`,
      "m",
    ),
  );
  return match?.[1] ?? match?.[2] ?? match?.[3] ?? null;
};

const firstProxyBlock = (content: string): string => {
  const lines = content.split(/\r?\n/);
  const start = lines.findIndex((line) => /^\s*\[\[proxies\]\]\s*$/.test(line));
  if (start < 0) return "";
  const block: string[] = [];
  for (let index = start + 1; index < lines.length; index += 1) {
    const line = lines[index] ?? "";
    if (/^\s*\[\[/.test(line)) break;
    block.push(line);
  }
  return block.join("\n");
};

const buildSummary = (content: string): FrpcInstanceSummary => {
  const proxy = firstProxyBlock(content);
  return {
    serverAddr:
      extractTomlValue(content, "serverAddr") ??
      extractTomlValue(content, "server_addr") ??
      "",
    serverPort:
      extractTomlValue(content, "serverPort") ??
      extractTomlValue(content, "server_port") ??
      "7000",
    localPort:
      extractTomlValue(proxy, "localPort") ??
      extractTomlValue(proxy, "local_port") ??
      "",
    remotePort:
      extractTomlValue(proxy, "remotePort") ??
      extractTomlValue(proxy, "remote_port") ??
      "",
  };
};

const readConfigForMeta = async (meta: FrpcInstanceMeta): Promise<string> => {
  ensureFrpcLayout();
  if (!fs.existsSync(meta.workDir))
    fs.mkdirSync(meta.workDir, { recursive: true });
  if (!fs.existsSync(meta.configPath)) {
    const content = defaultFrpcTemplate();
    fs.writeFileSync(meta.configPath, content, "utf-8");
    return content;
  }
  return fs.readFileSync(meta.configPath, "utf-8");
};

const writeConfigForMeta = async (meta: FrpcInstanceMeta, content: string) => {
  ensureFrpcLayout();
  if (!fs.existsSync(meta.workDir))
    fs.mkdirSync(meta.workDir, { recursive: true });
  fs.writeFileSync(meta.configPath, content, "utf-8");
};

const normalizeVerifyOutput = (value: string): string => {
  const normalized = value
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n");
  if (normalized.length <= 4000) return normalized;
  return `${normalized.slice(0, 4000)}...`;
};

const formatVerifyFailureMessage = (result: {
  exitCode: number;
  stdout: string;
  stderr: string;
}): string => {
  const detail = [result.stderr, result.stdout]
    .map(normalizeVerifyOutput)
    .filter(Boolean)
    .join("\n");
  if (detail) return frpcT("verifyFailedWithDetail", { detail });
  return frpcT("verifyFailedWithCode", { code: result.exitCode });
};

const verifyFrpcConfigForMeta = async (
  meta: FrpcInstanceMeta,
  content: string,
): Promise<void> => {
  let bin: string;
  try {
    bin = frpManager.getExecutable("frpc");
  } catch {
    throw new FrpcConfigValidationError(
      frpcT("verifyFrpNotInitialized"),
    );
  }

  if (!fs.existsSync(meta.workDir))
    fs.mkdirSync(meta.workDir, { recursive: true });
  const tempPath = path.join(meta.workDir, `frpc.verify.${randomUUID()}.toml`);
  try {
    fs.writeFileSync(tempPath, content, "utf-8");
    const result = await runCommand([bin, "verify", "-c", tempPath], {
      cwd: meta.workDir,
    });
    if (result.exitCode !== 0) {
      throw new FrpcConfigValidationError(formatVerifyFailureMessage(result));
    }
  } catch (error) {
    if (error instanceof FrpcConfigValidationError) throw error;
    const message = error instanceof Error ? error.message : String(error);
    throw new FrpcConfigValidationError(
      frpcT("verifyFailedWithDetail", { detail: message }),
    );
  } finally {
    try {
      if (fs.existsSync(tempPath)) fs.unlinkSync(tempPath);
    } catch {}
  }
};

const readCandidatePid = async (
  meta: FrpcInstanceMeta,
  runtime: FrpcInstanceRuntime,
): Promise<number | null> => {
  const attachedPid = attachedProcesses.get(meta.id)?.proc.pid ?? null;
  if (attachedPid && isAttachedProcessAlive(meta, attachedPid))
    return attachedPid;
  if (attachedPid && (await isOwnedFrpcPid(attachedPid, meta.configPath)))
    return attachedPid;
  if (runtime.pid && (await isOwnedFrpcPid(runtime.pid, meta.configPath)))
    return runtime.pid;
  const filePid = readPidFile(getPidPath(meta));
  if (filePid && (await isOwnedFrpcPid(filePid, meta.configPath)))
    return filePid;
  return findFrpcPidByConfigPath(meta.configPath);
};

const reconcileRuntime = async (
  meta: FrpcInstanceMeta,
): Promise<FrpcInstanceRuntime & { running: boolean; attached: boolean }> => {
  const runtime = await readRuntime(meta.id);
  const pid = await readCandidatePid(meta, runtime);
  const attached = Boolean(pid && isAttachedProcessAlive(meta, pid));
  if (pid) {
    const next = mergeDetectedFrpcRuntime(runtime, pid, nowIso);
    if (shouldPersistDetectedFrpcRuntime(runtime, next)) {
      await writeRuntime(meta.id, next);
    }
    writePidFile(getPidPath(meta), pid);
    return { ...next, running: true, attached };
  }

  const hadPid = Boolean(runtime.pid || readPidFile(getPidPath(meta)));
  removePidFile(getPidPath(meta));
  if (runtime.pid !== null || hadPid) {
    const next = {
      ...runtime,
      pid: null,
      stoppedAt: runtime.stoppedAt ?? nowIso(),
      lastMessage: runtime.lastMessage ?? frpcT("pidInvalidForInstance"),
    };
    await writeRuntime(meta.id, next);
    return { ...next, running: false, attached: false };
  }
  return { ...runtime, running: false, attached: false };
};

const buildStatus = async (
  meta: FrpcInstanceMeta,
): Promise<FrpcInstanceStatus> => {
  const runtime = await reconcileRuntime(meta);
  const content = await readConfigForMeta(meta);
  return {
    ...meta,
    desiredRunning: runtime.desiredRunning,
    pid: runtime.pid,
    startedAt: runtime.startedAt,
    stoppedAt: runtime.stoppedAt,
    lastExitCode: runtime.lastExitCode,
    lastMessage: runtime.lastMessage,
    running: runtime.running,
    attached: runtime.attached,
    summary: buildSummary(content),
  };
};

const countRunningInstances = async (): Promise<number> => {
  const metas = await getAllMetas();
  const statuses = await Promise.all(metas.map((meta) => buildStatus(meta)));
  return statuses.filter((status) => status.running).length;
};

const updateAggregateTunnelState = async () => {
  try {
    if ((await countRunningInstances()) > 0) {
      await markTunnelRunning("frp");
    } else {
      await markTunnelStopped("frp");
    }
  } catch (error) {
    console.error("Failed to persist frpc aggregate running state:", error);
  }
};

const attachProcessStreams = (meta: FrpcInstanceMeta, proc: ChildProcess) => {
  void (async () => {
    if (!proc.stdout) return;
    let buf = "";
    try {
      for await (const chunk of proc.stdout) {
        buf += chunk.toString();
        const parts = buf.split(/\r?\n/);
        buf = parts.pop() || "";
        await appendLogs(meta, parts);
      }
      if (buf) await appendLogs(meta, [buf]);
    } catch (error) {
      await appendLogs(meta, [`frpc stdout read error: ${String(error)}`], {
        inspectSignals: false,
      });
    }
  })();

  void (async () => {
    if (!proc.stderr) return;
    let buf = "";
    try {
      for await (const chunk of proc.stderr) {
        buf += chunk.toString();
        const parts = buf.split(/\r?\n/);
        buf = parts.pop() || "";
        await appendLogs(
          meta,
          parts.map((line) => `[ERR] ${line}`),
        );
      }
      if (buf) await appendLogs(meta, [`[ERR] ${buf}`]);
    } catch (error) {
      await appendLogs(meta, [`frpc stderr read error: ${String(error)}`], {
        inspectSignals: false,
      });
    }
  })();
};

const handleProcessExit = (
  meta: FrpcInstanceMeta,
  proc: ChildProcess,
  exitPromise: Promise<number>,
) => {
  void (async () => {
    let code = -1;
    let exitMessage = frpcT("processExited");
    try {
      code = await exitPromise;
      exitMessage = frpcT("processExitedWithCode", { code });
      await appendLogs(meta, [`frpc exited with code ${code}`], {
        inspectSignals: false,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      exitMessage = frpcT("processCrashed", { message });
      await appendLogs(meta, [`frpc process error: ${message}`], {
        inspectSignals: false,
      });
    }

    const current = attachedProcesses.get(meta.id);
    if (current?.proc !== proc) return;
    attachedProcesses.delete(meta.id);
    removePidFile(getPidPath(meta));

    const state = getConnectionState(meta.id);
    const expectedStop = state.stopRequested;
    const runtime = await readRuntime(meta.id);
    await writeRuntime(meta.id, {
      ...runtime,
      pid: null,
      stoppedAt: nowIso(),
      lastExitCode: code,
      lastMessage: exitMessage,
    });
    await updateAggregateTunnelState();
    if (!expectedStop) {
      await emitFrpcConnectivity(meta, false, exitMessage, proc.pid ?? null);
    }
    state.stopRequested = false;
  })();
};

const terminatePid = async (pid: number): Promise<void> => {
  if (!Number.isFinite(pid) || pid <= 0 || pid === process.pid) return;
  if (!isProcessAlive(pid)) return;

  try {
    process.kill(pid, "SIGTERM");
  } catch {}

  for (let i = 0; i < 20; i += 1) {
    if (!isProcessAlive(pid)) return;
    await sleep(100);
  }

  try {
    process.kill(pid, "SIGKILL");
  } catch {}

  for (let i = 0; i < 10; i += 1) {
    if (!isProcessAlive(pid)) return;
    await sleep(100);
  }

  if (isProcessAlive(pid)) {
    throw new Error(frpcT("processStillRunning", { pid }));
  }
};

export const frpcInstanceManager = {
  primaryId: FRPC_PRIMARY_INSTANCE_ID,

  defaultContent(): string {
    return defaultFrpcTemplate();
  },

  async hasAnyRuntimeData(): Promise<boolean> {
    const ids = await readInstanceIds();
    if (!ids.length) return false;
    const exists = await Promise.all(
      ids.map((id) => redis.exists(instanceKey(id, "runtime"))),
    );
    return exists.some((value) => value > 0);
  },

  async ensureInitialized(): Promise<void> {
    await ensurePrimaryInstance();
  },

  async getOverview(): Promise<FrpcInstancesOverview> {
    const metas = await getAllMetas();
    const statuses = await Promise.all(metas.map((meta) => buildStatus(meta)));
    const st = frpManager.getStatus();
    return {
      initialized: st.downloaded,
      platform: st.platform,
      primaryInstanceId: FRPC_PRIMARY_INSTANCE_ID,
      total: statuses.length,
      extraCount: statuses.filter((item) => !item.isPrimary).length,
      runningCount: statuses.filter((item) => item.running).length,
      defaults: { local_port: process.env.GO_REPROXY_PORT || "7999" },
      items: statuses,
    };
  },

  async getStatus(id: string): Promise<FrpcInstanceStatus> {
    return buildStatus(await getMetaOrThrow(id));
  },

  async getDetail(id: string, logLimit = 200): Promise<FrpcInstanceDetail> {
    const meta = await getMetaOrThrow(id);
    const [item, content, logs] = await Promise.all([
      buildStatus(meta),
      readConfigForMeta(meta),
      getLogBuffer(meta.id).list(logLimit),
    ]);
    return { item, content, logs };
  },

  async readConfig(id: string): Promise<string> {
    return readConfigForMeta(await getMetaOrThrow(id));
  },

  async saveConfig(id: string, content: string): Promise<FrpcInstanceStatus> {
    const meta = await getMetaOrThrow(id);
    await verifyFrpcConfigForMeta(meta, content);
    await writeConfigForMeta(meta, content);
    const nextMeta = { ...meta, updatedAt: nowIso() };
    await writeMeta(nextMeta);
    return buildStatus(nextMeta);
  },

  async updateInstance(
    id: string,
    payload: { name?: string; content?: string },
  ): Promise<FrpcInstanceStatus> {
    const meta = await getMetaOrThrow(id);
    let nextMeta = { ...meta, updatedAt: nowIso() };
    if (typeof payload.name === "string") {
      const name = payload.name.trim();
      nextMeta = {
        ...nextMeta,
        name: name || (meta.isPrimary ? frpcT("primaryName") : frpcT("instanceName")),
      };
    }
    if (typeof payload.content === "string") {
      await verifyFrpcConfigForMeta(nextMeta, payload.content);
      await writeConfigForMeta(nextMeta, payload.content);
    }
    await writeMeta(nextMeta);
    return buildStatus(nextMeta);
  },

  async createInstance(payload: {
    name?: string;
    content?: string;
  }): Promise<FrpcInstanceStatus> {
    await ensurePrimaryInstance();
    const metas = await getAllMetas();
    const extraCount = metas.filter((meta) => !meta.isPrimary).length;
    if (extraCount >= EXTRA_INSTANCE_LIMIT) {
      throw new FrpcInstanceLimitError(EXTRA_INSTANCE_LIMIT);
    }
    const id = randomUUID();
    const paths = extraInstancePaths(id);
    const timestamp = nowIso();
    const meta: FrpcInstanceMeta = {
      id,
      name: payload.name?.trim() || frpcT("instanceName"),
      isPrimary: false,
      configPath: paths.configPath,
      workDir: paths.workDir,
      createdAt: timestamp,
      updatedAt: timestamp,
      sortOrder:
        metas.reduce((max, item) => Math.max(max, item.sortOrder), 0) + 1,
    };
    const content = payload.content ?? defaultFrpcTemplate();
    try {
      await verifyFrpcConfigForMeta(meta, content);
      fs.mkdirSync(meta.workDir, { recursive: true });
      await writeConfigForMeta(meta, content);
      await writeMeta(meta);
      await writeRuntime(meta.id, defaultRuntime());
      await writeInstanceIds([...metas.map((item) => item.id), meta.id]);
      await appendLogs(meta, ["frpc instance created"], {
        inspectSignals: false,
      });
      return buildStatus(meta);
    } catch (error) {
      try {
        if (fs.existsSync(meta.workDir))
          fs.rmSync(meta.workDir, { recursive: true, force: true });
      } catch {}
      await redis.del(
        instanceKey(meta.id, "meta"),
        instanceKey(meta.id, "runtime"),
        logKey(meta.id),
        `${logKey(meta.id)}:seq`,
      );
      logBuffers.delete(meta.id);
      await writeInstanceIds(metas.map((item) => item.id));
      throw error;
    }
  },

  async deleteInstance(id: string): Promise<void> {
    const meta = await getMetaOrThrow(id);
    if (meta.isPrimary) {
      throw new Error(frpcT("primaryDeleteDenied"));
    }
    const status = await buildStatus(meta);
    if (status.running) {
      await this.stop(id);
    }
    await redis.del(instanceKey(meta.id, "meta"));
    await redis.del(instanceKey(meta.id, "runtime"));
    await getLogBuffer(meta.id).clear();
    const ids = await readInstanceIds();
    await writeInstanceIds(ids.filter((item) => item !== meta.id));
    try {
      if (fs.existsSync(meta.workDir))
        fs.rmSync(meta.workDir, { recursive: true, force: true });
    } catch {}
    logBuffers.delete(meta.id);
  },

  async start(id: string): Promise<{ pid: number }> {
    const meta = await getMetaOrThrow(id);
    const st = frpManager.getStatus();
    if (!st.downloaded) throw new Error(frpcT("notInitialized"));
    const content = await readConfigForMeta(meta);
    await verifyFrpcConfigForMeta(meta, content);

    const current = await buildStatus(meta);
    if (current.running && current.pid) {
      const runtime = await readRuntime(meta.id);
      await writeRuntime(meta.id, {
        ...runtime,
        desiredRunning: true,
        pid: current.pid,
      });
      return { pid: current.pid };
    }

    const bin = frpManager.getExecutable("frpc");
    const proc = spawn(bin, ["-c", meta.configPath], {
      cwd: meta.workDir,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const exitPromise = waitForProcessExit(proc);
    if (!proc.pid) {
      let detail = "spawn failed";
      try {
        await exitPromise;
      } catch (error) {
        detail = error instanceof Error ? error.message : String(error);
      }
      throw new Error(frpcT("startFailedWithDetail", { detail }));
    }

    const state = getConnectionState(meta.id);
    state.stopRequested = false;
    attachedProcesses.set(meta.id, { proc });
    writePidFile(getPidPath(meta), proc.pid);
    await writeRuntime(meta.id, {
      desiredRunning: true,
      pid: proc.pid,
      startedAt: nowIso(),
      stoppedAt: null,
      lastExitCode: null,
      lastMessage: `frpc started pid=${proc.pid}`,
    });
    attachProcessStreams(meta, proc);
    handleProcessExit(meta, proc, exitPromise);
    await appendLogs(meta, [`frpc started pid=${proc.pid}`], {
      inspectSignals: false,
    });
    await updateAggregateTunnelState();
    return { pid: proc.pid };
  },

  async stop(id: string): Promise<void> {
    const meta = await getMetaOrThrow(id);
    const status = await buildStatus(meta);
    const state = getConnectionState(meta.id);
    state.stopRequested = true;
    state.connected = false;
    if (!status.pid) {
      const runtime = await readRuntime(meta.id);
      await writeRuntime(meta.id, {
        ...runtime,
        desiredRunning: false,
        pid: null,
        stoppedAt: nowIso(),
        lastMessage: "frpc already stopped",
      });
      removePidFile(getPidPath(meta));
      state.stopRequested = false;
      await updateAggregateTunnelState();
      return;
    }

    if (!(await isOwnedFrpcPid(status.pid, meta.configPath))) {
      const runtime = await readRuntime(meta.id);
      await writeRuntime(meta.id, {
        ...runtime,
        desiredRunning: false,
        pid: null,
        stoppedAt: nowIso(),
        lastMessage: frpcT("pidCleanedForInstance"),
      });
      removePidFile(getPidPath(meta));
      state.stopRequested = false;
      await updateAggregateTunnelState();
      return;
    }

    await terminatePid(status.pid);
    attachedProcesses.delete(meta.id);
    removePidFile(getPidPath(meta));
    const runtime = await readRuntime(meta.id);
    await writeRuntime(meta.id, {
      ...runtime,
      desiredRunning: false,
      pid: null,
      stoppedAt: nowIso(),
      lastMessage: `frpc stopped pid=${status.pid}`,
    });
    await appendLogs(meta, [`frpc stopped pid=${status.pid}`], {
      inspectSignals: false,
    });
    state.stopRequested = false;
    await updateAggregateTunnelState();
  },

  async restart(id: string): Promise<{ pid: number }> {
    await this.stop(id);
    return this.start(id);
  },

  async listLogs(id: string, limit: number): Promise<string[]> {
    const meta = await getMetaOrThrow(id);
    return getLogBuffer(meta.id).list(limit);
  },

  async clearLogs(id: string): Promise<void> {
    const meta = await getMetaOrThrow(id);
    await getLogBuffer(meta.id).clear();
  },

  async poll(id: string, cursor?: number | string | null) {
    const meta = await getMetaOrThrow(id);
    const {
      cursor: nextCursor,
      reset,
      items,
    } = await getLogBuffer(meta.id).poll(cursor);
    const status = await buildStatus(meta);
    return { cursor: nextCursor, reset, logs: items, status };
  },

  async restoreOnBoot(): Promise<void> {
    const hadRuntime = await this.hasAnyRuntimeData();
    await ensurePrimaryInstance();
    if (!hadRuntime && (await shouldResumeTunnel("frp"))) {
      const runtime = await readRuntime(FRPC_PRIMARY_INSTANCE_ID);
      await writeRuntime(FRPC_PRIMARY_INSTANCE_ID, {
        ...runtime,
        desiredRunning: true,
      });
    }

    const metas = await getAllMetas();
    for (const meta of metas) {
      const status = await buildStatus(meta);
      if (!status.desiredRunning || status.running) continue;
      try {
        await appendLogs(
          meta,
          [frpcT("resumeOnBoot")],
          {
            inspectSignals: false,
          },
        );
        await this.start(meta.id);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        await appendLogs(meta, [`resume error: ${message}`], {
          inspectSignals: false,
        });
      }
    }
    await updateAggregateTunnelState();
  },
};

export type {
  FrpcInstanceDetail,
  FrpcInstanceMeta,
  FrpcInstanceRuntime,
  FrpcInstanceStatus,
  FrpcInstanceSummary,
  FrpcInstancesOverview,
};
