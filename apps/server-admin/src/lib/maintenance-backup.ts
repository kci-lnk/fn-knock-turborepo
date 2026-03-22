import { spawn } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  APP_BACKUP_IMPORT_VERSION_RANGE,
  APP_BACKUP_SCHEMA_VERSION,
  APP_LOCAL_VERSION,
  formatVersionRange,
  isBackupAppVersionSupported,
} from "./app-version";
import { goBackend } from "./go-backend";
import { firewallService } from "./firewall-service";
import { configManager, redis } from "./redis";
import { syncGatewayLoggingToGateway } from "./gateway-logging";
import { collectStreamOutput, waitForProcessExit } from "./runtime";
import { syncSSLDeploymentToGateway } from "./ssl-gateway";
import { whitelistManager } from "./whitelist-manager";
import {
  buildKnockBackupFilename,
  KNOCK_BACKUP_EXTENSION,
  KNOCK_BACKUP_JSON_FILENAME,
  KNOCK_BACKUP_PREFIX,
} from "../../../../packages/admin-shared/src/utils/maintenanceBackup";

const SCAN_COUNT = 200;
const PIPELINE_BATCH_SIZE = 100;
const TEMP_DIR_PREFIX = "fn-knock-backup-";
const KNOCK_BACKUP_PASSWORD = "890eced0-4561-4044-8d6b-def83b5c6016";
const BASE64_PATTERN =
  /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const SUPPORTED_BACKUP_IMPORT_VERSION_RANGE = formatVersionRange(
  APP_BACKUP_IMPORT_VERSION_RANGE,
);

type RedisBackupValueType = "string" | "hash" | "list" | "set" | "zset";
type CommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

type RedisZSetEntry = {
  member: string;
  score: number;
};

type RedisBackupEntry =
  | {
      key: string;
      type: "string";
      ttl_ms: number | null;
      value: string;
    }
  | {
      key: string;
      type: "hash";
      ttl_ms: number | null;
      value: Record<string, string>;
    }
  | {
      key: string;
      type: "list";
      ttl_ms: number | null;
      value: string[];
    }
  | {
      key: string;
      type: "set";
      ttl_ms: number | null;
      value: string[];
    }
  | {
      key: string;
      type: "zset";
      ttl_ms: number | null;
      value: RedisZSetEntry[];
    };

export type FnKnockBackupPayload = {
  version: typeof APP_BACKUP_SCHEMA_VERSION;
  app_version: string;
  prefix: typeof KNOCK_BACKUP_PREFIX;
  exported_at: string;
  entry_count: number;
  entries: RedisBackupEntry[];
};

export type FnKnockBackupArchive = {
  buffer: Buffer;
  exported_at: string;
  filename: string;
};

export type FnKnockBackupImportArchiveRequest = {
  filename?: string;
  archive_base64: string;
};

export type FnKnockBackupImportResult = {
  cleared_keys: number;
  imported_keys: number;
  warnings: string[];
  synced_steps: string[];
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isSupportedType = (value: unknown): value is RedisBackupValueType =>
  value === "string" ||
  value === "hash" ||
  value === "list" ||
  value === "set" ||
  value === "zset";

const chunk = <T>(items: T[], size: number): T[][] => {
  const safeSize = Math.max(1, Math.floor(size));
  const output: T[][] = [];
  for (let index = 0; index < items.length; index += safeSize) {
    output.push(items.slice(index, index + safeSize));
  }
  return output;
};

const normalizeTtlMs = (ttlMs: number): number | null =>
  Number.isFinite(ttlMs) && ttlMs > 0 ? ttlMs : null;

export class MaintenanceBackupError extends Error {
  status: number;

  constructor(message: string, status = 500) {
    super(message);
    this.name = "MaintenanceBackupError";
    this.status = status;
  }
}

class MaintenanceBackupService {
  private async withTempDir<T>(
    task: (tempDir: string) => Promise<T>,
  ): Promise<T> {
    const tempDir = await mkdtemp(join(tmpdir(), TEMP_DIR_PREFIX));

    try {
      return await task(tempDir);
    } finally {
      await rm(tempDir, { recursive: true, force: true }).catch(
        () => undefined,
      );
    }
  }

  private async runCommand(
    command: string,
    args: string[],
    cwd?: string,
  ): Promise<CommandResult> {
    try {
      const proc = spawn(command, args, {
        cwd,
        stdio: ["ignore", "pipe", "pipe"],
      });
      const exitPromise = waitForProcessExit(proc);

      const [stdout, stderr, exitCode] = await Promise.all([
        collectStreamOutput(proc.stdout).catch(() => ""),
        collectStreamOutput(proc.stderr).catch(() => ""),
        exitPromise,
      ]);

      return { exitCode, stdout, stderr };
    } catch (error: any) {
      const isMissingBinary = error?.code === "ENOENT";
      throw new MaintenanceBackupError(
        isMissingBinary
          ? `系统环境缺少 ${command} 命令`
          : error?.message || `执行 ${command} 命令失败`,
        500,
      );
    }
  }

  private createCommandError(
    message: string,
    result: CommandResult,
    status: number,
  ): MaintenanceBackupError {
    const detail = (result.stderr || result.stdout || "")
      .trim()
      .split("\n")
      .filter(Boolean)
      .slice(-3)
      .join(" | ");

    return new MaintenanceBackupError(
      `${message}（退出码: ${result.exitCode}）${detail ? `: ${detail}` : ""}`,
      status,
    );
  }

  private async scanKeys(prefix = KNOCK_BACKUP_PREFIX): Promise<string[]> {
    let cursor = "0";
    const keys: string[] = [];

    do {
      const result = await redis.scan(
        cursor,
        "MATCH",
        `${prefix}*`,
        "COUNT",
        SCAN_COUNT,
      );
      cursor = result[0];
      const batch = Array.isArray(result[1]) ? (result[1] as string[]) : [];
      if (batch.length > 0) {
        keys.push(...batch);
      }
    } while (cursor !== "0");

    return [...new Set(keys)].sort((left, right) => left.localeCompare(right));
  }

  private async exportEntry(key: string): Promise<RedisBackupEntry | null> {
    const [type, ttlMs] = await Promise.all([redis.type(key), redis.pttl(key)]);
    const normalizedTtlMs = normalizeTtlMs(ttlMs);

    if (type === "none") {
      return null;
    }

    if (type === "string") {
      const value = await redis.get(key);
      if (value === null) return null;
      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    if (type === "hash") {
      const value = await redis.hgetall(key);
      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    if (type === "list") {
      const value = await redis.lrange(key, 0, -1);
      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    if (type === "set") {
      const value = await redis.smembers(key);
      value.sort((left, right) => left.localeCompare(right));
      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    if (type === "zset") {
      const pairs = await redis.zrange(key, 0, -1, "WITHSCORES");
      const value: RedisZSetEntry[] = [];

      for (let index = 0; index < pairs.length; index += 2) {
        const member = pairs[index];
        const rawScore = pairs[index + 1];
        if (typeof member !== "string" || typeof rawScore !== "string") {
          continue;
        }
        const score = Number(rawScore);
        if (!Number.isFinite(score)) {
          continue;
        }
        value.push({ member, score });
      }

      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    throw new MaintenanceBackupError(
      `不支持导出的 Redis 数据类型: ${type} (${key})`,
      500,
    );
  }

  private async exportBackupPayload(): Promise<FnKnockBackupPayload> {
    const keys = await this.scanKeys();
    const entries = (
      await Promise.all(keys.map((key) => this.exportEntry(key)))
    ).filter((entry): entry is RedisBackupEntry => entry !== null);

    return {
      version: APP_BACKUP_SCHEMA_VERSION,
      app_version: APP_LOCAL_VERSION,
      prefix: KNOCK_BACKUP_PREFIX,
      exported_at: new Date().toISOString(),
      entry_count: entries.length,
      entries,
    };
  }

  async exportBackupArchive(): Promise<FnKnockBackupArchive> {
    const payload = await this.exportBackupPayload();

    return this.withTempDir(async (tempDir) => {
      const filename = buildKnockBackupFilename(payload.exported_at);
      const archivePath = join(tempDir, filename);
      const payloadPath = join(tempDir, KNOCK_BACKUP_JSON_FILENAME);

      try {
        await writeFile(payloadPath, JSON.stringify(payload, null, 2), "utf-8");

        const result = await this.runCommand(
          "zip",
          ["-q", "-j", "-P", KNOCK_BACKUP_PASSWORD, archivePath, payloadPath],
          tempDir,
        );

        if (result.exitCode !== 0) {
          throw this.createCommandError("生成备份归档失败", result, 500);
        }

        return {
          buffer: await readFile(archivePath),
          exported_at: payload.exported_at,
          filename,
        };
      } catch (error: any) {
        if (error instanceof MaintenanceBackupError) {
          throw error;
        }

        throw new MaintenanceBackupError(
          error?.message || "生成备份归档失败",
          500,
        );
      }
    });
  }

  private parseStringArray(value: unknown, label: string): string[] {
    if (!Array.isArray(value)) {
      throw new MaintenanceBackupError(`${label} 必须是字符串数组`, 400);
    }
    const output = value.filter(
      (item): item is string => typeof item === "string",
    );
    if (output.length !== value.length) {
      throw new MaintenanceBackupError(`${label} 只能包含字符串`, 400);
    }
    return output;
  }

  private parseHashValue(
    value: unknown,
    label: string,
  ): Record<string, string> {
    if (!isRecord(value)) {
      throw new MaintenanceBackupError(`${label} 必须是对象`, 400);
    }

    const output: Record<string, string> = {};
    for (const [field, rawFieldValue] of Object.entries(value)) {
      if (typeof rawFieldValue !== "string") {
        throw new MaintenanceBackupError(`${label}.${field} 必须是字符串`, 400);
      }
      output[field] = rawFieldValue;
    }
    return output;
  }

  private parseZSetValue(value: unknown, label: string): RedisZSetEntry[] {
    if (!Array.isArray(value)) {
      throw new MaintenanceBackupError(`${label} 必须是数组`, 400);
    }

    return value.map((item, index) => {
      if (!isRecord(item) || typeof item.member !== "string") {
        throw new MaintenanceBackupError(
          `${label}[${index}] 必须包含字符串 member`,
          400,
        );
      }

      const score = Number(item.score);
      if (!Number.isFinite(score)) {
        throw new MaintenanceBackupError(
          `${label}[${index}] 必须包含有效的数值 score`,
          400,
        );
      }

      return { member: item.member, score };
    });
  }

  private parseEntry(value: unknown, index: number): RedisBackupEntry {
    if (!isRecord(value)) {
      throw new MaintenanceBackupError(`entries[${index}] 必须是对象`, 400);
    }

    const key = typeof value.key === "string" ? value.key : "";
    if (!key.startsWith(KNOCK_BACKUP_PREFIX)) {
      throw new MaintenanceBackupError(
        `entries[${index}].key 必须以 ${KNOCK_BACKUP_PREFIX} 开头`,
        400,
      );
    }

    if (!isSupportedType(value.type)) {
      throw new MaintenanceBackupError(`entries[${index}].type 不受支持`, 400);
    }

    const ttlMs =
      value.ttl_ms == null
        ? null
        : Number.isFinite(Number(value.ttl_ms)) && Number(value.ttl_ms) > 0
          ? Math.floor(Number(value.ttl_ms))
          : (() => {
              throw new MaintenanceBackupError(
                `entries[${index}].ttl_ms 必须为正整数或 null`,
                400,
              );
            })();

    if (value.type === "string") {
      if (typeof value.value !== "string") {
        throw new MaintenanceBackupError(
          `entries[${index}].value 必须是字符串`,
          400,
        );
      }
      return { key, type: value.type, ttl_ms: ttlMs, value: value.value };
    }

    if (value.type === "hash") {
      return {
        key,
        type: value.type,
        ttl_ms: ttlMs,
        value: this.parseHashValue(value.value, `entries[${index}].value`),
      };
    }

    if (value.type === "list" || value.type === "set") {
      return {
        key,
        type: value.type,
        ttl_ms: ttlMs,
        value: this.parseStringArray(value.value, `entries[${index}].value`),
      };
    }

    return {
      key,
      type: value.type,
      ttl_ms: ttlMs,
      value: this.parseZSetValue(value.value, `entries[${index}].value`),
    };
  }

  private parseBackupPayload(rawPayload: unknown): FnKnockBackupPayload {
    let payload: unknown = rawPayload;
    if (typeof rawPayload === "string") {
      try {
        payload = JSON.parse(rawPayload) as unknown;
      } catch {
        throw new MaintenanceBackupError("备份文件 JSON 无法解析", 400);
      }
    }

    if (!isRecord(payload)) {
      throw new MaintenanceBackupError("备份文件内容不是有效对象", 400);
    }

    if (payload.version !== APP_BACKUP_SCHEMA_VERSION) {
      throw new MaintenanceBackupError(
        `仅支持 version=${APP_BACKUP_SCHEMA_VERSION} 的备份文件`,
        400,
      );
    }

    if (payload.prefix !== KNOCK_BACKUP_PREFIX) {
      throw new MaintenanceBackupError(
        `仅支持 ${KNOCK_BACKUP_PREFIX} 前缀的备份文件`,
        400,
      );
    }

    const appVersion =
      typeof payload.app_version === "string" ? payload.app_version.trim() : "";
    if (!appVersion) {
      throw new MaintenanceBackupError("备份文件缺少 app_version", 400);
    }

    if (!isBackupAppVersionSupported(appVersion)) {
      throw new MaintenanceBackupError(
        `当前版本 ${APP_LOCAL_VERSION} 仅允许导入 ${SUPPORTED_BACKUP_IMPORT_VERSION_RANGE} 范围内导出的备份，收到 ${appVersion}`,
        400,
      );
    }

    const exportedAt =
      typeof payload.exported_at === "string" ? payload.exported_at : "";
    if (!exportedAt) {
      throw new MaintenanceBackupError("备份文件缺少 exported_at", 400);
    }

    if (!Array.isArray(payload.entries)) {
      throw new MaintenanceBackupError("备份文件缺少 entries 数组", 400);
    }

    const entries = payload.entries.map((entry, index) =>
      this.parseEntry(entry, index),
    );
    const uniqueKeys = new Set(entries.map((entry) => entry.key));
    if (uniqueKeys.size !== entries.length) {
      throw new MaintenanceBackupError("备份文件存在重复 Redis key", 400);
    }

    return {
      version: APP_BACKUP_SCHEMA_VERSION,
      app_version: appVersion,
      prefix: KNOCK_BACKUP_PREFIX,
      exported_at: exportedAt,
      entry_count: entries.length,
      entries,
    };
  }

  private async extractPayloadFromArchive(
    archiveBuffer: Buffer,
  ): Promise<FnKnockBackupPayload> {
    return this.withTempDir(async (tempDir) => {
      const archivePath = join(tempDir, `import${KNOCK_BACKUP_EXTENSION}`);

      try {
        await writeFile(archivePath, archiveBuffer);

        const result = await this.runCommand(
          "unzip",
          [
            "-qq",
            "-P",
            KNOCK_BACKUP_PASSWORD,
            "-p",
            archivePath,
            KNOCK_BACKUP_JSON_FILENAME,
          ],
          tempDir,
        );

        if (result.exitCode !== 0) {
          const detail = `${result.stderr}\n${result.stdout}`.toLowerCase();

          if (detail.includes("filename not matched")) {
            throw new MaintenanceBackupError(
              `备份归档中缺少 ${KNOCK_BACKUP_JSON_FILENAME}`,
              400,
            );
          }

          if (
            detail.includes("incorrect password") ||
            detail.includes("wrong password")
          ) {
            throw new MaintenanceBackupError("备份归档密码校验失败", 400);
          }

          throw this.createCommandError(
            "读取 .knock 备份归档失败",
            result,
            400,
          );
        }

        return this.parseBackupPayload(result.stdout);
      } catch (error: any) {
        if (error instanceof MaintenanceBackupError) {
          throw error;
        }

        throw new MaintenanceBackupError(
          error?.message || "读取 .knock 备份归档失败",
          400,
        );
      }
    });
  }

  private async clearPrefixKeys(): Promise<number> {
    const keys = await this.scanKeys();
    if (keys.length === 0) {
      return 0;
    }

    for (const batch of chunk(keys, SCAN_COUNT)) {
      await redis.del(...batch);
    }

    return keys.length;
  }

  private async restoreEntries(entries: RedisBackupEntry[]): Promise<void> {
    let pipeline = redis.pipeline();
    let batchedKeys = 0;

    const flush = async () => {
      if (batchedKeys === 0) return;
      const result = await pipeline.exec();
      const failed = result?.find(([error]) => error != null);
      if (failed?.[0]) {
        throw new MaintenanceBackupError(
          failed[0].message || "写入 Redis 备份数据失败",
          500,
        );
      }
      pipeline = redis.pipeline();
      batchedKeys = 0;
    };

    for (const entry of entries) {
      if (entry.type === "string") {
        if (entry.ttl_ms) {
          pipeline.set(entry.key, entry.value, "PX", entry.ttl_ms);
        } else {
          pipeline.set(entry.key, entry.value);
        }
      } else if (entry.type === "hash") {
        if (Object.keys(entry.value).length > 0) {
          pipeline.hmset(entry.key, entry.value);
        }
        if (entry.ttl_ms) {
          pipeline.pexpire(entry.key, entry.ttl_ms);
        }
      } else if (entry.type === "list") {
        if (entry.value.length > 0) {
          pipeline.rpush(entry.key, ...entry.value);
        }
        if (entry.ttl_ms) {
          pipeline.pexpire(entry.key, entry.ttl_ms);
        }
      } else if (entry.type === "set") {
        if (entry.value.length > 0) {
          pipeline.sadd(entry.key, ...entry.value);
        }
        if (entry.ttl_ms) {
          pipeline.pexpire(entry.key, entry.ttl_ms);
        }
      } else {
        if (entry.value.length > 0) {
          const args = entry.value.flatMap((item) => [item.score, item.member]);
          pipeline.zadd(entry.key, ...args);
        }
        if (entry.ttl_ms) {
          pipeline.pexpire(entry.key, entry.ttl_ms);
        }
      }

      batchedKeys += 1;
      if (batchedKeys >= PIPELINE_BATCH_SIZE) {
        await flush();
      }
    }

    await flush();
  }

  private async syncRuntimeAfterImport(): Promise<{
    warnings: string[];
    syncedSteps: string[];
  }> {
    const warnings: string[] = [];
    const syncedSteps: string[] = [];

    const attempt = async (label: string, task: () => Promise<void>) => {
      try {
        await task();
        syncedSteps.push(label);
      } catch (error: any) {
        const message = error?.message || String(error) || "未知错误";
        warnings.push(`${label}: ${message}`);
      }
    };

    const config = await configManager.getConfig();

    await attempt("运行模式与网关路由", async () => {
      await firewallService.applyRunTypeConfig(config.run_type);
    });

    if (config.run_type === 0) {
      await attempt("直连模式白名单", async () => {
        const records = await whitelistManager.getAllActiveRecords();
        for (const record of records) {
          await goBackend.allowIP(record.ip);
        }
      });
    }

    await attempt("请求日志配置", async () => {
      await syncGatewayLoggingToGateway(config.gateway_logging);
    });

    await attempt("SSL 证书部署", async () => {
      await syncSSLDeploymentToGateway(config);
    });

    return { warnings, syncedSteps };
  }

  async importBackupArchive(
    request: FnKnockBackupImportArchiveRequest,
  ): Promise<FnKnockBackupImportResult> {
    const archiveBase64 = request.archive_base64?.trim() || "";
    if (!archiveBase64) {
      throw new MaintenanceBackupError("缺少备份归档内容", 400);
    }

    if (!BASE64_PATTERN.test(archiveBase64)) {
      throw new MaintenanceBackupError("备份归档不是有效的 Base64 数据", 400);
    }

    const filename = request.filename?.trim() || "";
    if (filename && !filename.toLowerCase().endsWith(KNOCK_BACKUP_EXTENSION)) {
      throw new MaintenanceBackupError(
        `备份文件扩展名必须为 ${KNOCK_BACKUP_EXTENSION}`,
        400,
      );
    }

    let archiveBuffer: Buffer;
    try {
      archiveBuffer = Buffer.from(archiveBase64, "base64");
    } catch {
      throw new MaintenanceBackupError("备份归档不是有效的 Base64 数据", 400);
    }

    if (archiveBuffer.length === 0) {
      throw new MaintenanceBackupError("备份归档内容为空", 400);
    }

    const payload = await this.extractPayloadFromArchive(archiveBuffer);
    const clearedKeys = await this.clearPrefixKeys();
    await this.restoreEntries(payload.entries);
    const syncResult = await this.syncRuntimeAfterImport();

    return {
      cleared_keys: clearedKeys,
      imported_keys: payload.entries.length,
      warnings: syncResult.warnings,
      synced_steps: syncResult.syncedSteps,
    };
  }
}

export const maintenanceBackupService = new MaintenanceBackupService();
