import fs from "node:fs";
import path from "node:path";
import { dataPath } from "../AppDirManager";
import { redis } from "../redis";
import { tDefault } from "../i18n";
import {
  FRPC_PRIMARY_INSTANCE_ID,
  type FrpcInstanceMeta,
  type FrpcInstanceRuntime,
} from "./types";

export const FRPC_DIR = path.join(dataPath, "frp");
export const FRPC_INSTANCES_DIR = path.join(FRPC_DIR, "instances");
export const FRPC_PRIMARY_TOML = path.join(FRPC_DIR, "frpc.toml");
export const FRPC_PRIMARY_PID = path.join(FRPC_DIR, "frpc.pid");
export const KEY_PREFIX = "fn_knock:frpc:v2";
export const INSTANCE_IDS_KEY = `${KEY_PREFIX}:instance_ids`;
export const PRIMARY_INSTANCE_ID_KEY = `${KEY_PREFIX}:primary_instance_id`;

const frpcT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.frpc.${key}`, params);

const nowIso = () => new Date().toISOString();

export const ensureFrpcLayout = () => {
  if (!fs.existsSync(FRPC_DIR)) fs.mkdirSync(FRPC_DIR, { recursive: true });
  if (!fs.existsSync(FRPC_INSTANCES_DIR)) {
    fs.mkdirSync(FRPC_INSTANCES_DIR, { recursive: true });
  }
};

export const sanitizeInstanceId = (id: string): string | null => {
  const trimmed = String(id || "").trim();
  if (!/^[a-zA-Z0-9-]{1,80}$/.test(trimmed)) return null;
  return trimmed;
};

export const instanceKey = (id: string, part: "meta" | "runtime") =>
  `${KEY_PREFIX}:instance:${id}:${part}`;

export const defaultRuntime = (): FrpcInstanceRuntime => ({
  desiredRunning: false,
  pid: null,
  startedAt: null,
  stoppedAt: null,
  lastExitCode: null,
  lastMessage: null,
});

export const defaultFrpcTemplate = (): string => {
  const localPort = process.env.GO_REPROXY_PORT || "7999";
  return [
    'serverAddr = ""',
    "serverPort = 7000",
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
    "",
  ].join("\n");
};

export const safeJsonParse = (value: string | null): unknown => {
  if (!value) return null;
  try {
    return JSON.parse(value);
  } catch {
    return null;
  }
};

export const readInstanceIds = async (): Promise<string[]> => {
  const parsed = safeJsonParse(await redis.get(INSTANCE_IDS_KEY));
  if (!Array.isArray(parsed)) return [];
  const ids = parsed
    .map((value) =>
      typeof value === "string" ? sanitizeInstanceId(value) : null,
    )
    .filter((value): value is string => Boolean(value));
  return [...new Set(ids)];
};

export const writeInstanceIds = async (ids: string[]) => {
  const unique = [...new Set(ids)];
  await redis.set(INSTANCE_IDS_KEY, JSON.stringify(unique));
  await redis.set(PRIMARY_INSTANCE_ID_KEY, FRPC_PRIMARY_INSTANCE_ID);
};

export const normalizeMeta = (
  raw: unknown,
  fallback: FrpcInstanceMeta,
): FrpcInstanceMeta => {
  if (!raw || typeof raw !== "object") return fallback;
  const obj = raw as Record<string, unknown>;
  return {
    id: typeof obj.id === "string" ? obj.id : fallback.id,
    name: typeof obj.name === "string" ? obj.name : fallback.name,
    isPrimary:
      typeof obj.isPrimary === "boolean" ? obj.isPrimary : fallback.isPrimary,
    configPath:
      typeof obj.configPath === "string" ? obj.configPath : fallback.configPath,
    workDir: typeof obj.workDir === "string" ? obj.workDir : fallback.workDir,
    createdAt:
      typeof obj.createdAt === "string" ? obj.createdAt : fallback.createdAt,
    updatedAt:
      typeof obj.updatedAt === "string" ? obj.updatedAt : fallback.updatedAt,
    sortOrder:
      typeof obj.sortOrder === "number" ? obj.sortOrder : fallback.sortOrder,
  };
};

export const normalizeRuntime = (raw: unknown): FrpcInstanceRuntime => {
  if (!raw || typeof raw !== "object") return defaultRuntime();
  const obj = raw as Record<string, unknown>;
  const pid =
    typeof obj.pid === "number" && Number.isFinite(obj.pid) && obj.pid > 0
      ? obj.pid
      : null;
  return {
    desiredRunning:
      typeof obj.desiredRunning === "boolean"
        ? obj.desiredRunning
        : typeof obj.desired_running === "boolean"
          ? obj.desired_running
          : false,
    pid,
    startedAt:
      typeof obj.startedAt === "string"
        ? obj.startedAt
        : typeof obj.started_at === "string"
          ? obj.started_at
          : null,
    stoppedAt:
      typeof obj.stoppedAt === "string"
        ? obj.stoppedAt
        : typeof obj.stopped_at === "string"
          ? obj.stopped_at
          : null,
    lastExitCode:
      typeof obj.lastExitCode === "number"
        ? obj.lastExitCode
        : typeof obj.last_exit_code === "number"
          ? obj.last_exit_code
          : null,
    lastMessage:
      typeof obj.lastMessage === "string"
        ? obj.lastMessage
        : typeof obj.last_message === "string"
          ? obj.last_message
          : null,
  };
};

export const primaryMeta = (): FrpcInstanceMeta => {
  const timestamp = nowIso();
  return {
    id: FRPC_PRIMARY_INSTANCE_ID,
    name: frpcT("primaryName"),
    isPrimary: true,
    configPath: FRPC_PRIMARY_TOML,
    workDir: FRPC_DIR,
    createdAt: timestamp,
    updatedAt: timestamp,
    sortOrder: 0,
  };
};

export const extraInstancePaths = (id: string) => {
  const workDir = path.join(FRPC_INSTANCES_DIR, id);
  return {
    workDir,
    configPath: path.join(workDir, "frpc.toml"),
    pidPath: path.join(workDir, "frpc.pid"),
  };
};

export const getPidPath = (meta: FrpcInstanceMeta) =>
  meta.isPrimary ? FRPC_PRIMARY_PID : path.join(meta.workDir, "frpc.pid");

export const readMeta = async (
  id: string,
): Promise<FrpcInstanceMeta | null> => {
  const fallback =
    id === FRPC_PRIMARY_INSTANCE_ID
      ? primaryMeta()
      : (() => {
          const paths = extraInstancePaths(id);
          const timestamp = nowIso();
          return {
            id,
            name: frpcT("instanceName"),
            isPrimary: false,
            configPath: paths.configPath,
            workDir: paths.workDir,
            createdAt: timestamp,
            updatedAt: timestamp,
            sortOrder: 1000,
          };
        })();
  const raw = await redis.get(instanceKey(id, "meta"));
  if (!raw) return null;
  return normalizeMeta(safeJsonParse(raw), fallback);
};

export const writeMeta = async (meta: FrpcInstanceMeta) => {
  await redis.set(instanceKey(meta.id, "meta"), JSON.stringify(meta));
};

export const readRuntime = async (
  id: string,
): Promise<FrpcInstanceRuntime> =>
  normalizeRuntime(safeJsonParse(await redis.get(instanceKey(id, "runtime"))));

export const writeRuntime = async (
  id: string,
  runtime: FrpcInstanceRuntime,
) => {
  await redis.set(instanceKey(id, "runtime"), JSON.stringify(runtime));
};

export const ensurePrimaryInstance = async () => {
  ensureFrpcLayout();
  const ids = await readInstanceIds();
  if (!ids.includes(FRPC_PRIMARY_INSTANCE_ID)) {
    await writeInstanceIds([FRPC_PRIMARY_INSTANCE_ID, ...ids]);
  }
  const existing = await readMeta(FRPC_PRIMARY_INSTANCE_ID);
  if (!existing) {
    await writeMeta(primaryMeta());
  }
  if (!fs.existsSync(FRPC_PRIMARY_TOML)) {
    fs.writeFileSync(FRPC_PRIMARY_TOML, defaultFrpcTemplate(), "utf-8");
  }
};

export const getAllMetas = async (): Promise<FrpcInstanceMeta[]> => {
  await ensurePrimaryInstance();
  const ids = await readInstanceIds();
  const metas = (await Promise.all(ids.map((id) => readMeta(id)))).filter(
    (value): value is FrpcInstanceMeta => Boolean(value),
  );
  return metas.sort(
    (a, b) =>
      a.sortOrder - b.sortOrder || a.createdAt.localeCompare(b.createdAt),
  );
};
