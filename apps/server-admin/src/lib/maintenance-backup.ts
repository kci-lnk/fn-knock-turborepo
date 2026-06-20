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
import { APP_BACKUP_SCHEMA_VERSION, APP_LOCAL_VERSION } from "./app-version";
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
import { MaintenanceBackupError } from "./maintenance-backup/errors";
import { shouldExportBackupKey } from "./maintenance-backup/key-filter";
import { backupT } from "./maintenance-backup/messages";
import { parseBackupPayload } from "./maintenance-backup/payload";
import { createPasswordProtectedZip } from "./maintenance-backup/zip-crypto";
import type {
  FnKnockBackupPayload,
  RedisBackupEntry,
  RedisStreamEntry,
  RedisZSetEntry,
} from "./maintenance-backup/payload";
import {
  buildKnockBackupFilename,
  KNOCK_BACKUP_EXTENSION,
  KNOCK_BACKUP_JSON_FILENAME,
  KNOCK_BACKUP_PREFIX,
} from "../../../../packages/admin-shared/src/utils/maintenanceBackup";

export { MaintenanceBackupError } from "./maintenance-backup/errors";
export type {
  FnKnockBackupPayload,
  RedisBackupEntry,
  RedisStreamEntry,
  RedisZSetEntry,
} from "./maintenance-backup/payload";

const SCAN_COUNT = 200;
const PIPELINE_BATCH_SIZE = 100;
const TEMP_DIR_PREFIX = "fn-knock-backup-";
const KNOCK_BACKUP_PASSWORD = "890eced0-4561-4044-8d6b-def83b5c6016";
const OPENWRT_OPKG_COMMAND = "opkg";
const DEBIAN_APT_GET_PATH = "/usr/bin/apt-get";
const BACKUP_DIRECTORY_NAME = "backup";
const MAX_BACKUP_DIRECTORY_SCAN_DEPTH = 5;
const MAX_BACKUP_DIRECTORY_FILES = 500;
const MAX_BACKUP_ARCHIVE_SIZE = 128 * 1024 * 1024;
const BASE64_PATTERN =
  /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/;

type CommandResult = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

type ArchiveCommandName = "unzip";

type ArchiveCommandSpec = {
  command: ArchiveCommandName;
  packageName: string;
  probeArgs: string[];
};

const ARCHIVE_COMMAND_SPECS: ArchiveCommandSpec[] = [
  {
    command: "unzip",
    packageName: "unzip",
    probeArgs: ["-v"],
  },
];

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
          ? backupT("commandMissing", { command })
          : error?.message || backupT("commandFailed", { command }),
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
        error?.message || backupT("commandCheckFailed", { command }),
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
    const packages = [
      ...new Set(missingCommands.map((item) => item.packageName)),
    ];

    const canUseOpkg = await this.isCommandAvailable(OPENWRT_OPKG_COMMAND, [
      "--version",
    ]);
    if (canUseOpkg) {
      const updateResult = await this.runCommand(OPENWRT_OPKG_COMMAND, [
        "update",
      ]);
      if (updateResult.exitCode !== 0) {
        throw this.createCommandError(
          backupT("opkgUpdateFailed"),
          updateResult,
          500,
        );
      }

      const installResult = await this.runCommand(OPENWRT_OPKG_COMMAND, [
        "install",
        ...packages,
      ]);
      if (installResult.exitCode !== 0) {
        throw this.createCommandError(
          backupT("packageInstallFailed", { packages: packages.join(", ") }),
          installResult,
          500,
        );
      }
    } else {
      const canUseAptGet = await this.isCommandAvailable(DEBIAN_APT_GET_PATH, [
        "--version",
      ]);
      if (!canUseAptGet) {
        throw new MaintenanceBackupError(
          backupT("commandsMissingNoPackageManager", {
            commands: missingNames,
          }),
          500,
        );
      }

      const updateResult = await this.runCommand(DEBIAN_APT_GET_PATH, [
        "update",
      ]);
      if (updateResult.exitCode !== 0) {
        throw this.createCommandError(
          backupT("aptUpdateFailed"),
          updateResult,
          500,
        );
      }

      const installResult = await this.runCommand(DEBIAN_APT_GET_PATH, [
        "install",
        "-y",
        ...packages,
      ]);
      if (installResult.exitCode !== 0) {
        throw this.createCommandError(
          backupT("packageInstallFailed", { packages: packages.join(", ") }),
          installResult,
          500,
        );
      }
    }

    const remainingCommands = await this.findMissingArchiveCommands();
    if (remainingCommands.length > 0) {
      throw new MaintenanceBackupError(
        backupT("commandsStillMissingAfterInstall", {
          commands: remainingCommands.map((item) => item.command).join(", "),
        }),
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
      detail
        ? backupT("commandErrorWithDetail", {
            message,
            code: result.exitCode,
            detail,
          })
        : backupT("commandError", { message, code: result.exitCode }),
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
        backupT("shareDirectoryMissing"),
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
      throw new MaintenanceBackupError(backupT("invalidBackupPath"), 400);
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
            backupT("invalidRedisStreamData", { key, id }),
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
      backupT("unsupportedRedisExportType", { type, key }),
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
    const payload = await this.exportBackupPayload();
    const filename = buildKnockBackupFilename(payload.exported_at);
    const archiveBuffer = createPasswordProtectedZip(
      KNOCK_BACKUP_JSON_FILENAME,
      Buffer.from(JSON.stringify(payload, null, 2), "utf-8"),
      KNOCK_BACKUP_PASSWORD,
      new Date(payload.exported_at),
    );

    return {
      buffer: archiveBuffer,
      exported_at: payload.exported_at,
      filename,
    };
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
        backupT("invalidBackupExtension", {
          extension: KNOCK_BACKUP_EXTENSION,
        }),
        400,
      );
    }
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
              backupT("archiveMissingPayload", {
                filename: KNOCK_BACKUP_JSON_FILENAME,
              }),
              400,
            );
          }

          if (
            detail.includes("incorrect password") ||
            detail.includes("wrong password")
          ) {
            throw new MaintenanceBackupError(
              backupT("archivePasswordInvalid"),
              400,
            );
          }

          throw this.createCommandError(
            backupT("readArchiveFailed"),
            result,
            400,
          );
        }

        return parseBackupPayload(result.stdout);
      } catch (error: any) {
        if (error instanceof MaintenanceBackupError) {
          throw error;
        }

        throw new MaintenanceBackupError(
          error?.message || backupT("readArchiveFailed"),
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
          failed[0].message || backupT("writeRedisFailed"),
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
        const message = error?.message || String(error) || backupT("unknownError");
        warnings.push(`${label}: ${message}`);
      }
    };

    const config = await configManager.getConfig();

    await attempt(backupT("syncSteps.runModeGatewayRoutes"), async () => {
      await firewallService.applyRunTypeConfig(config.run_type);
    });

    if (config.run_type === 0) {
      await attempt(backupT("syncSteps.directModeWhitelist"), async () => {
        const records = await whitelistManager.getAllActiveConcreteTargets();
        for (const record of records) {
          await goBackend.allowIP(record.target);
        }
      });
    }

    await attempt(backupT("syncSteps.gatewayLogging"), async () => {
      await syncGatewayLoggingToGateway(config.gateway_logging);
    });

    await attempt(backupT("syncSteps.sslDeployment"), async () => {
      await syncSSLDeploymentToGateway(config);
    });

    await attempt(backupT("syncSteps.legacyAuthLogCleanup"), async () => {
      await cleanupLegacyAuthLogStorage();
    });

    await attempt(backupT("syncSteps.systemResourceMonitorReset"), async () => {
      await systemResourceMonitor.resetStates();
    });

    return { warnings, syncedSteps };
  }

  private async importBackupArchiveBuffer(
    archiveBuffer: Buffer,
  ): Promise<FnKnockBackupImportResult> {
    if (archiveBuffer.length === 0) {
      throw new MaintenanceBackupError(backupT("archiveEmpty"), 400);
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
      throw new MaintenanceBackupError(backupT("directoryImportFileOnly"), 400);
    }
    if (!isBackupArchiveFile(filePath)) {
      throw new MaintenanceBackupError(
        backupT("directoryImportExtensionOnly", {
          extension: KNOCK_BACKUP_EXTENSION,
        }),
        400,
      );
    }
    if (fileStats.size > MAX_BACKUP_ARCHIVE_SIZE) {
      throw new MaintenanceBackupError(backupT("directoryImportTooLarge"), 400);
    }

    return this.importBackupArchiveBuffer(await readFile(filePath));
  }

  async importBackupArchive(
    request: FnKnockBackupImportArchiveRequest,
  ): Promise<FnKnockBackupImportResult> {
    const archiveBase64 = request.archive_base64?.trim() || "";
    if (!archiveBase64) {
      throw new MaintenanceBackupError(backupT("archiveContentMissing"), 400);
    }

    if (!BASE64_PATTERN.test(archiveBase64)) {
      throw new MaintenanceBackupError(backupT("archiveBase64Invalid"), 400);
    }

    const filename = request.filename?.trim() || "";
    this.validateBackupFilename(filename);

    let archiveBuffer: Buffer;
    try {
      archiveBuffer = Buffer.from(archiveBase64, "base64");
    } catch {
      throw new MaintenanceBackupError(backupT("archiveBase64Invalid"), 400);
    }

    return this.importBackupArchiveBuffer(archiveBuffer);
  }
}

export const maintenanceBackupService = new MaintenanceBackupService();
