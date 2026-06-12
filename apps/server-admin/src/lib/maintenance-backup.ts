import { spawn } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, extname, join, relative, resolve } from "node:path";
import {
  APP_BACKUP_IMPORT_VERSION_RANGE,
  APP_BACKUP_SCHEMA_VERSION,
  APP_LOCAL_VERSION,
  formatVersionRange,
  isBackupAppVersionSupported,
} from "./app-version";
import { getConfiguredShareDirectory } from "./fnos-data-share";
import { goBackend } from "./go-backend";
import { firewallService } from "./firewall-service";
import { cleanupLegacyAuthLogStorage } from "./cleanup-legacy-auth-logs";
import { configManager, redis } from "./redis";
import { syncGatewayLoggingToGateway } from "./gateway-logging";
import { collectStreamOutput, waitForProcessExit } from "./runtime";
import { syncSSLDeploymentToGateway } from "./ssl-gateway";
import { systemResourceMonitor } from "./system-resource-monitor";
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
const DEBIAN_APT_GET_PATH = "/usr/bin/apt-get";
const BACKUP_DIRECTORY_NAME = "backup";
const MAX_BACKUP_DIRECTORY_SCAN_DEPTH = 5;
const MAX_BACKUP_DIRECTORY_FILES = 500;
const MAX_BACKUP_ARCHIVE_SIZE = 128 * 1024 * 1024;
const BASE64_PATTERN =
  /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;
const SUPPORTED_BACKUP_IMPORT_VERSION_RANGE = formatVersionRange(
  APP_BACKUP_IMPORT_VERSION_RANGE,
);

const BACKUP_EXCLUDED_KEY_PREFIXES = [
  "fn_knock:acme:job:",
  "fn_knock:acme:logs:",
  "fn_knock:auth_log_data:",
  "fn_knock:auth_logs:",
  "fn_knock:auth_mobility:",
  "fn_knock:backoff:",
  "fn_knock:cidr:",
  "fn_knock:cloudflared:logs",
  "fn_knock:common_auth_locations:runtime",
  "fn_knock:config:backup:",
  "fn_knock:docker_admin:login_backoff:",
  "fn_knock:docker_admin:session:",
  "fn_knock:errors:",
  "fn_knock:events:",
  "fn_knock:fnos-share:session:",
  "fn_knock:fnos-share:validation:",
  "fn_knock:gateway:",
  "fn_knock:ip_location:",
  "fn_knock:lock:",
  "fn_knock:login_backoff:",
  "fn_knock:nonce:",
  "fn_knock:notifications:deliveries:",
  "fn_knock:notifications:runtime:",
  "fn_knock:notifications:triggers:",
  "fn_knock:oidc:invite:",
  "fn_knock:oidc:login_error:",
  "fn_knock:oidc:state:",
  "fn_knock:passkey:bind:",
  "fn_knock:passkey:challenge:",
  "fn_knock:recent_auth_ips:",
  "fn_knock:reverse-proxy:",
  "fn_knock:scanner:blacklist:",
  "fn_knock:scanner:suspicious:",
  "fn_knock:session:",
  "fn_knock:smart-connect:runtime",
  "fn_knock:ssh_security:",
  "fn_knock:terminal:",
  "fn_knock:traffic:",
  "fn_knock:tunnel:runtime",
  "fn_knock:ui:",
  "fn_knock:update:",
  "fn_knock:waf:log:",
  "fn_knock:waf:logs:",
  "fn_knock:waf:stats:",
  "fn_knock:welcome-guide:",
] as const;

const BACKUP_EXCLUDED_KEY_PATTERNS = [
  /^fn_knock:acme:runtime-lock$/,
  /^fn_knock:ddns:last_(?:ip|check)$/,
  /^fn_knock:ddns:logs(?::seq)?$/,
  /^fn_knock:ddns:v2:target:[^:]+:last_(?:ip|check)$/,
  /^fn_knock:frpc:v2:instance:[^:]+:(?:runtime|logs(?::seq)?)$/,
] as const;

const shouldExportBackupKey = (key: string): boolean =>
  !BACKUP_EXCLUDED_KEY_PREFIXES.some((prefix) => key.startsWith(prefix)) &&
  !BACKUP_EXCLUDED_KEY_PATTERNS.some((pattern) => pattern.test(key));

type RedisBackupValueType =
  | "string"
  | "hash"
  | "list"
  | "set"
  | "zset"
  | "stream";
type CommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

type ArchiveCommandName = "zip" | "unzip";

type ArchiveCommandSpec = {
  command: ArchiveCommandName;
  packageName: string;
  probeArgs: string[];
};

const ARCHIVE_COMMAND_SPECS: ArchiveCommandSpec[] = [
  {
    command: "zip",
    packageName: "zip",
    probeArgs: ["-v"],
  },
  {
    command: "unzip",
    packageName: "unzip",
    probeArgs: ["-v"],
  },
];

type RedisZSetEntry = {
  member: string;
  score: number;
};

type RedisStreamEntry = {
  id: string;
  fields: string[];
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
    }
  | {
      key: string;
      type: "stream";
      ttl_ms: number | null;
      value: RedisStreamEntry[];
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

export type BackupDirectoryFileEntry = {
  name: string;
  relativePath: string;
  extension: string;
  size: number;
  modifiedAt: string;
};

export type BackupDirectoryFilesPayload = {
  shareName: string;
  available: boolean;
  files: BackupDirectoryFileEntry[];
};

export type FnKnockBackupExportToDirectoryResult = {
  filename: string;
  relativePath: string;
  filePath: string;
  size: number;
  exportedAt: string;
};

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isSupportedType = (value: unknown): value is RedisBackupValueType =>
  value === "string" ||
  value === "hash" ||
  value === "list" ||
  value === "set" ||
  value === "zset" ||
  value === "stream";

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

const normalizeExtension = (value: string): string =>
  extname(value).toLowerCase();

const isBackupArchiveFile = (value: string): boolean =>
  normalizeExtension(value) === KNOCK_BACKUP_EXTENSION;

export class MaintenanceBackupError extends Error {
  status: number;

  constructor(message: string, status = 500) {
    super(message);
    this.name = "MaintenanceBackupError";
    this.status = status;
  }
}

class MaintenanceBackupService {
  private archiveCommandsReady = false;
  private archiveCommandSetupPromise: Promise<void> | null = null;

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

  private async isCommandAvailable(
    command: string,
    args: string[] = ["-v"],
  ): Promise<boolean> {
    try {
      const proc = spawn(command, args, {
        stdio: ["ignore", "ignore", "ignore"],
      });
      await waitForProcessExit(proc);
      return true;
    } catch (error: any) {
      if (error?.code === "ENOENT") {
        return false;
      }

      throw new MaintenanceBackupError(
        error?.message || `检测 ${command} 命令失败`,
        500,
      );
    }
  }

  private async findMissingArchiveCommands(): Promise<ArchiveCommandSpec[]> {
    const checks = await Promise.all(
      ARCHIVE_COMMAND_SPECS.map(async (spec) => ({
        spec,
        available: await this.isCommandAvailable(spec.command, spec.probeArgs),
      })),
    );

    return checks.filter((item) => !item.available).map((item) => item.spec);
  }

  private async installArchiveCommandsIfNeeded(): Promise<void> {
    const missingCommands = await this.findMissingArchiveCommands();
    if (missingCommands.length === 0) {
      return;
    }

    const missingNames = missingCommands.map((item) => item.command).join(", ");
    const canUseAptGet = await this.isCommandAvailable(DEBIAN_APT_GET_PATH, [
      "--version",
    ]);
    if (!canUseAptGet) {
      throw new MaintenanceBackupError(
        `系统环境缺少 ${missingNames} 命令，且未找到 Debian apt-get，无法自动安装`,
        500,
      );
    }

    const updateResult = await this.runCommand(DEBIAN_APT_GET_PATH, ["update"]);
    if (updateResult.exitCode !== 0) {
      throw this.createCommandError(
        "apt-get update 执行失败",
        updateResult,
        500,
      );
    }

    const packages = [
      ...new Set(missingCommands.map((item) => item.packageName)),
    ];
    const installResult = await this.runCommand(DEBIAN_APT_GET_PATH, [
      "install",
      "-y",
      ...packages,
    ]);
    if (installResult.exitCode !== 0) {
      throw this.createCommandError(
        `安装 ${packages.join(", ")} 失败`,
        installResult,
        500,
      );
    }

    const remainingCommands = await this.findMissingArchiveCommands();
    if (remainingCommands.length > 0) {
      throw new MaintenanceBackupError(
        `自动安装完成后仍未检测到 ${remainingCommands
          .map((item) => item.command)
          .join(", ")} 命令`,
        500,
      );
    }
  }

  private async ensureArchiveCommandsReady(): Promise<void> {
    if (this.archiveCommandsReady) {
      return;
    }

    if (!this.archiveCommandSetupPromise) {
      this.archiveCommandSetupPromise = this.installArchiveCommandsIfNeeded()
        .then(() => {
          this.archiveCommandsReady = true;
        })
        .finally(() => {
          this.archiveCommandSetupPromise = null;
        });
    }

    return this.archiveCommandSetupPromise;
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

  private getBackupDirectoryPath(): string {
    const shareDirectory = getConfiguredShareDirectory();
    return shareDirectory ? join(shareDirectory, BACKUP_DIRECTORY_NAME) : "";
  }

  private getRequiredBackupDirectoryPath(): string {
    const directoryPath = this.getBackupDirectoryPath();
    if (!directoryPath) {
      throw new MaintenanceBackupError(
        "未找到飞牛共享目录，请确认应用资源已正确配置",
        404,
      );
    }
    return directoryPath;
  }

  private async ensureBackupDirectory(): Promise<string> {
    const directoryPath = this.getRequiredBackupDirectoryPath();
    await mkdir(directoryPath, { recursive: true });
    return directoryPath;
  }

  private resolveBackupArchivePath(relativePath: string): string {
    const directoryPath = this.getRequiredBackupDirectoryPath();
    const sanitized = relativePath.replace(/\\/g, "/").trim();
    const resolvedPath = resolve(directoryPath, sanitized);
    const relativeToRoot = relative(directoryPath, resolvedPath);

    if (
      !sanitized ||
      sanitized.startsWith("/") ||
      relativeToRoot.startsWith("..") ||
      resolvedPath === directoryPath
    ) {
      throw new MaintenanceBackupError("非法的备份文件路径", 400);
    }

    return resolvedPath;
  }

  private toBackupDirectoryEntry(
    directoryPath: string,
    filePath: string,
    size: number,
    modifiedAt: Date,
  ): BackupDirectoryFileEntry {
    return {
      name: basename(filePath),
      relativePath: relative(directoryPath, filePath).split("\\").join("/"),
      extension: normalizeExtension(filePath),
      size,
      modifiedAt: modifiedAt.toISOString(),
    };
  }

  private async collectBackupDirectoryFiles(
    currentPath: string,
    directoryPath: string,
    bucket: BackupDirectoryFileEntry[],
    depth: number,
  ): Promise<void> {
    if (bucket.length >= MAX_BACKUP_DIRECTORY_FILES) {
      return;
    }

    const entries = await readdir(currentPath, { withFileTypes: true });

    for (const entry of entries) {
      if (bucket.length >= MAX_BACKUP_DIRECTORY_FILES) {
        return;
      }

      const entryPath = join(currentPath, entry.name);

      if (entry.isDirectory()) {
        if (depth >= MAX_BACKUP_DIRECTORY_SCAN_DEPTH) {
          continue;
        }
        await this.collectBackupDirectoryFiles(
          entryPath,
          directoryPath,
          bucket,
          depth + 1,
        );
        continue;
      }

      if (!entry.isFile() || !isBackupArchiveFile(entry.name)) {
        continue;
      }

      const entryStats = await stat(entryPath);
      bucket.push(
        this.toBackupDirectoryEntry(
          directoryPath,
          entryPath,
          entryStats.size,
          entryStats.mtime,
        ),
      );
    }
  }

  private async scanKeys(
    prefix = KNOCK_BACKUP_PREFIX,
    options: { exportableOnly?: boolean } = {},
  ): Promise<string[]> {
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

    const uniqueKeys = [...new Set(keys)].sort((left, right) =>
      left.localeCompare(right),
    );

    if (!options.exportableOnly) {
      return uniqueKeys;
    }

    return uniqueKeys.filter(shouldExportBackupKey);
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

    if (type === "stream") {
      const response = (await (redis as any).xrange(
        key,
        "-",
        "+",
      )) as Array<[string, Array<string>]> | null;
      const value: RedisStreamEntry[] = [];

      for (const item of response ?? []) {
        const [id, fields] = item;
        if (typeof id !== "string" || !Array.isArray(fields)) {
          continue;
        }

        const normalizedFields = fields.filter(
          (field): field is string => typeof field === "string",
        );
        if (
          normalizedFields.length !== fields.length ||
          normalizedFields.length === 0 ||
          normalizedFields.length % 2 !== 0
        ) {
          throw new MaintenanceBackupError(
            `Redis stream 数据格式无效: ${key} (${id})`,
            500,
          );
        }

        value.push({
          id,
          fields: normalizedFields,
        });
      }

      return { key, type, ttl_ms: normalizedTtlMs, value };
    }

    throw new MaintenanceBackupError(
      `不支持导出的 Redis 数据类型: ${type} (${key})`,
      500,
    );
  }

  private async exportBackupPayload(): Promise<FnKnockBackupPayload> {
    const keys = await this.scanKeys(KNOCK_BACKUP_PREFIX, {
      exportableOnly: true,
    });
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
    await this.ensureArchiveCommandsReady();
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

  async listBackupDirectoryFiles(): Promise<BackupDirectoryFilesPayload> {
    const directoryPath = this.getBackupDirectoryPath();
    if (!directoryPath) {
      return {
        shareName: "fn-knock / backup",
        available: false,
        files: [],
      };
    }

    await mkdir(directoryPath, { recursive: true });
    const files: BackupDirectoryFileEntry[] = [];

    await this.collectBackupDirectoryFiles(
      directoryPath,
      directoryPath,
      files,
      0,
    );
    files.sort((left, right) => {
      const timeDiff =
        new Date(right.modifiedAt).getTime() -
        new Date(left.modifiedAt).getTime();
      if (timeDiff !== 0) {
        return timeDiff;
      }
      return left.relativePath.localeCompare(right.relativePath, "zh-CN");
    });

    return {
      shareName: "fn-knock / backup",
      available: true,
      files,
    };
  }

  async exportBackupArchiveToDirectory(): Promise<FnKnockBackupExportToDirectoryResult> {
    const [archive, directoryPath] = await Promise.all([
      this.exportBackupArchive(),
      this.ensureBackupDirectory(),
    ]);
    const filePath = join(directoryPath, archive.filename);

    await writeFile(filePath, archive.buffer);

    const fileStats = await stat(filePath);
    return {
      filename: archive.filename,
      relativePath: archive.filename,
      filePath,
      size: fileStats.size,
      exportedAt: archive.exported_at,
    };
  }

  private validateBackupFilename(filename: string) {
    if (filename && !filename.toLowerCase().endsWith(KNOCK_BACKUP_EXTENSION)) {
      throw new MaintenanceBackupError(
        `备份文件扩展名必须为 ${KNOCK_BACKUP_EXTENSION}`,
        400,
      );
    }
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

  private parseStreamValue(value: unknown, label: string): RedisStreamEntry[] {
    if (!Array.isArray(value)) {
      throw new MaintenanceBackupError(`${label} 必须是数组`, 400);
    }

    return value.map((item, index) => {
      if (!isRecord(item) || typeof item.id !== "string") {
        throw new MaintenanceBackupError(
          `${label}[${index}] 必须包含字符串 id`,
          400,
        );
      }

      const fields = this.parseStringArray(
        item.fields,
        `${label}[${index}].fields`,
      );
      if (fields.length === 0 || fields.length % 2 !== 0) {
        throw new MaintenanceBackupError(
          `${label}[${index}].fields 必须是偶数长度且非空的字符串数组`,
          400,
        );
      }

      return {
        id: item.id,
        fields,
      };
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

    if (value.type === "stream") {
      return {
        key,
        type: value.type,
        ttl_ms: ttlMs,
        value: this.parseStreamValue(value.value, `entries[${index}].value`),
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
    let batchedCommands = 0;

    const queue = (task: () => void) => {
      task();
      batchedCommands += 1;
    };

    const flush = async () => {
      if (batchedCommands === 0) return;
      const result = await pipeline.exec();
      const failed = result?.find(([error]) => error != null);
      if (failed?.[0]) {
        throw new MaintenanceBackupError(
          failed[0].message || "写入 Redis 备份数据失败",
          500,
        );
      }
      pipeline = redis.pipeline();
      batchedCommands = 0;
    };

    for (const entry of entries) {
      if (entry.type === "string") {
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.set(entry.key, entry.value, "PX", entry.ttl_ms!);
          });
        } else {
          queue(() => {
            pipeline.set(entry.key, entry.value);
          });
        }
      } else if (entry.type === "hash") {
        if (Object.keys(entry.value).length > 0) {
          queue(() => {
            pipeline.hmset(entry.key, entry.value);
          });
        }
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.pexpire(entry.key, entry.ttl_ms!);
          });
        }
      } else if (entry.type === "list") {
        if (entry.value.length > 0) {
          queue(() => {
            pipeline.rpush(entry.key, ...entry.value);
          });
        }
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.pexpire(entry.key, entry.ttl_ms!);
          });
        }
      } else if (entry.type === "set") {
        if (entry.value.length > 0) {
          queue(() => {
            pipeline.sadd(entry.key, ...entry.value);
          });
        }
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.pexpire(entry.key, entry.ttl_ms!);
          });
        }
      } else if (entry.type === "zset") {
        if (entry.value.length > 0) {
          const args = entry.value.flatMap((item) => [item.score, item.member]);
          queue(() => {
            pipeline.zadd(entry.key, ...args);
          });
        }
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.pexpire(entry.key, entry.ttl_ms!);
          });
        }
      } else {
        for (const item of entry.value) {
          queue(() => {
            pipeline.call("XADD", entry.key, item.id, ...item.fields);
          });
          if (batchedCommands >= PIPELINE_BATCH_SIZE) {
            await flush();
          }
        }
        if (entry.ttl_ms) {
          queue(() => {
            pipeline.pexpire(entry.key, entry.ttl_ms!);
          });
        }
      }

      if (batchedCommands >= PIPELINE_BATCH_SIZE) {
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
        const records = await whitelistManager.getAllActiveConcreteTargets();
        for (const record of records) {
          await goBackend.allowIP(record.target);
        }
      });
    }

    await attempt("请求日志配置", async () => {
      await syncGatewayLoggingToGateway(config.gateway_logging);
    });

    await attempt("SSL 证书部署", async () => {
      await syncSSLDeploymentToGateway(config);
    });

    await attempt("废弃登录日志清理", async () => {
      await cleanupLegacyAuthLogStorage();
    });

    await attempt("系统资源监控状态重置", async () => {
      await systemResourceMonitor.resetStates();
    });

    return { warnings, syncedSteps };
  }

  private async importBackupArchiveBuffer(
    archiveBuffer: Buffer,
  ): Promise<FnKnockBackupImportResult> {
    if (archiveBuffer.length === 0) {
      throw new MaintenanceBackupError("备份归档内容为空", 400);
    }

    await this.ensureArchiveCommandsReady();
    const payload = await this.extractPayloadFromArchive(archiveBuffer);
    const importableEntries = payload.entries.filter((entry) =>
      shouldExportBackupKey(entry.key),
    );
    const clearedKeys = await this.clearPrefixKeys();
    await this.restoreEntries(importableEntries);
    const syncResult = await this.syncRuntimeAfterImport();

    return {
      cleared_keys: clearedKeys,
      imported_keys: importableEntries.length,
      warnings: syncResult.warnings,
      synced_steps: syncResult.syncedSteps,
    };
  }

  async importBackupArchiveFromDirectory(
    relativePath: string,
  ): Promise<FnKnockBackupImportResult> {
    const filePath = this.resolveBackupArchivePath(relativePath);
    const fileStats = await stat(filePath);

    if (!fileStats.isFile()) {
      throw new MaintenanceBackupError("只能导入备份目录中的文件", 400);
    }
    if (!isBackupArchiveFile(filePath)) {
      throw new MaintenanceBackupError(
        `仅支持导入 ${KNOCK_BACKUP_EXTENSION} 备份文件`,
        400,
      );
    }
    if (fileStats.size > MAX_BACKUP_ARCHIVE_SIZE) {
      throw new MaintenanceBackupError("备份文件过大，无法从飞牛目录导入", 400);
    }

    return this.importBackupArchiveBuffer(await readFile(filePath));
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
    this.validateBackupFilename(filename);

    let archiveBuffer: Buffer;
    try {
      archiveBuffer = Buffer.from(archiveBase64, "base64");
    } catch {
      throw new MaintenanceBackupError("备份归档不是有效的 Base64 数据", 400);
    }

    return this.importBackupArchiveBuffer(archiveBuffer);
  }
}

export const maintenanceBackupService = new MaintenanceBackupService();
