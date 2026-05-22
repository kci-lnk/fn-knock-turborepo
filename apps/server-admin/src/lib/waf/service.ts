import { createHash } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  stat,
  unlink,
  writeFile,
} from "node:fs/promises";
import type { Dirent } from "node:fs";
import { basename, dirname, extname, join, relative, sep } from "node:path";
import { inflateRawSync } from "node:zlib";
import {
  goBackend,
  type WAFConfig as GatewayWAFConfig,
  type WAFStatus,
} from "../go-backend";
import { syncCommonAuthLocationExemptionsToGateway } from "../common-auth-locations";
import {
  configManager,
  DEFAULT_WAF_CONFIG,
  normalizeWAFConfig,
  redis,
  type WAFConfig,
} from "../redis";
import { wafCollector } from "./collector";

const MANIFEST_URL = "https://fn-knock.cdn.wxlnk.com/waf/manifest.json";
const MANIFEST_REFRESH_MS = 24 * 60 * 60 * 1000;
const SYSTEM_RULES_AUTO_UPDATE_MS = 10 * 60 * 1000;
const SYSTEM_RULES_AUTO_UPDATE_LOCK_TTL_SECONDS = 10 * 60;
const MAX_RULE_FILE_BYTES = 1024 * 1024;
const MAX_ZIP_BYTES = 20 * 1024 * 1024;
const MAX_UNPACKED_ZIP_BYTES = 60 * 1024 * 1024;
const CONF_EXT_RE = /\.conf$/i;
const BUNDLE_ENTRY_PATH_RE = /^[A-Za-z0-9._/-]+$/;
const SYSTEM_INITIALIZATION_RULE_FILENAME = "REQUEST-901-INITIALIZATION.conf";
// Highest-frequency false-positive CRS files from the REQUEST-900 exclusions.
const DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES = new Set([
  "REQUEST-920-PROTOCOL-ENFORCEMENT.conf",
  "REQUEST-930-APPLICATION-ATTACK-LFI.conf",
  "REQUEST-932-APPLICATION-ATTACK-RCE.conf",
  "REQUEST-941-APPLICATION-ATTACK-XSS.conf",
  "REQUEST-942-APPLICATION-ATTACK-SQLI.conf",
]);
const BLOCKED_DIRECTIVE_RE =
  /^\s*(Include|SecAuditLog|SecDebugLog|SecDataDir|SecTmpDir|SecUploadDir)\b/im;

const ROOT_DIR = DEFAULT_WAF_CONFIG.rules_dir;
const SYSTEM_DIR = join(ROOT_DIR, "system");
const CUSTOM_DIR = join(ROOT_DIR, "custom");
const MANIFEST_CACHE_PATH = join(ROOT_DIR, "manifest.json");
const SYSTEM_SYNC_PATH = join(ROOT_DIR, "system-sync.json");
const RULES_STATE_PATH = join(ROOT_DIR, "rules-state.json");
const DEFAULT_DISABLED_SYSTEM_RULES_PATCH_FLAG_KEY =
  "fn_knock:patch:waf-default-disabled-system-rules:v1";

export type WAFRuleSource = "system" | "custom";

export interface WAFManifestRule {
  filename: string;
  description: string;
}

export interface WAFRemoteManifest {
  rulesDescription?: {
    rules?: WAFManifestRule[];
  };
  packagingTime?: string;
  zipFile: string;
  zipHash: string;
  commitHash?: string;
  commitDate?: string;
}

export interface WAFRuleFile {
  source: WAFRuleSource;
  filename: string;
  description: string;
  enabled: boolean;
  size_bytes: number;
  updated_at: string;
}

export interface WAFRuleFileContent extends WAFRuleFile {
  content: string;
}

export interface WAFManifestCache {
  manifest: WAFRemoteManifest | null;
  cached_at: string | null;
  last_checked_at: string | null;
  last_error: string | null;
}

export interface WAFSystemSyncState {
  zip_file: string;
  zip_hash: string;
  synced_at: string;
  packaging_time?: string;
  commit_hash?: string;
  commit_date?: string;
}

export interface WAFDetails {
  config: WAFConfig;
  status: WAFStatus | null;
  rules_dir: string;
  system: {
    manifest: WAFRemoteManifest | null;
    manifest_cached_at: string | null;
    manifest_last_checked_at: string | null;
    manifest_last_error: string | null;
    synced: WAFSystemSyncState | null;
    update_available: boolean;
    rules: WAFRuleFile[];
  };
  custom: {
    rules: WAFRuleFile[];
  };
}

export type WAFSystemRulesAutoUpdateSkipReason =
  | "disabled"
  | "waf_disabled"
  | "locked"
  | "running"
  | "up_to_date";

export interface WAFSystemRulesAutoUpdateResult {
  checked_at: string;
  updated: boolean;
  manifest_zip_hash?: string;
  synced_zip_hash?: string | null;
  skipped_reason?: WAFSystemRulesAutoUpdateSkipReason;
}

export interface WAFRuleToggleInput {
  source: WAFRuleSource;
  filenames?: string[];
  enabled: boolean;
}

export interface WAFUploadInputFile {
  filename: string;
  content_base64: string;
}

export interface WAFUploadInput {
  files: WAFUploadInputFile[];
}

export type WAFConfigPatch = Partial<
  Pick<
    WAFConfig,
    | "enabled"
    | "system_rules_auto_update_enabled"
    | "common_location_exempt_enabled"
    | "paranoia_level"
    | "executing_paranoia_level"
  >
>;

interface WAFRulesState {
  system_enabled: Record<string, boolean>;
  custom_enabled: Record<string, boolean>;
}

interface WAFDefaultDisabledSystemRulesPatchResult {
  applied: boolean;
  disabled_filenames: string[];
}

interface ArchiveEntry {
  path: string;
  content: Buffer;
}

let systemRulesAutoUpdateTimer: ReturnType<typeof setInterval> | null = null;
let systemRulesAutoUpdateRunning = false;

const nowISO = () => new Date().toISOString();

const sha256Hex = (input: Buffer | string): string =>
  createHash("sha256").update(input).digest("hex");

const errorMessage = (error: unknown, fallback: string): string =>
  error instanceof Error && error.message ? error.message : fallback;

const cacheBustedURL = (input: string, base?: string): string => {
  const url = new URL(input, base);
  url.searchParams.set(
    "t",
    `${Date.now()}-${Math.random().toString(36).slice(2)}`,
  );
  return url.toString();
};

const ensureWAFDirectories = async (): Promise<void> => {
  await mkdir(SYSTEM_DIR, { recursive: true });
  await mkdir(CUSTOM_DIR, { recursive: true });
};

const readJSONFile = async <T>(path: string, fallback: T): Promise<T> => {
  try {
    return JSON.parse(await readFile(path, "utf8")) as T;
  } catch (error: any) {
    if (error?.code === "ENOENT") return fallback;
    throw error;
  }
};

const writeJSONFile = async (path: string, value: unknown): Promise<void> => {
  await mkdir(dirname(path), { recursive: true });
  await writeFile(path, `${JSON.stringify(value, null, 2)}\n`, "utf8");
};

const defaultRulesState = (): WAFRulesState => ({
  system_enabled: {
    [SYSTEM_INITIALIZATION_RULE_FILENAME]: true,
  },
  custom_enabled: {},
});

const isSystemRuleEnabledByDefault = (filename: string): boolean =>
  filename === SYSTEM_INITIALIZATION_RULE_FILENAME ||
  !DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES.has(filename);

const enforceRequiredRuleState = (state: WAFRulesState): WAFRulesState => ({
  system_enabled: {
    ...state.system_enabled,
    [SYSTEM_INITIALIZATION_RULE_FILENAME]: true,
  },
  custom_enabled: state.custom_enabled,
});

const readRulesState = async (): Promise<WAFRulesState> => {
  const state = await readJSONFile<WAFRulesState>(
    RULES_STATE_PATH,
    defaultRulesState(),
  );
  return enforceRequiredRuleState({
    system_enabled: state.system_enabled || {},
    custom_enabled: state.custom_enabled || {},
  });
};

const writeRulesState = async (state: WAFRulesState): Promise<void> => {
  const normalized = enforceRequiredRuleState(state);
  await writeJSONFile(RULES_STATE_PATH, {
    system_enabled: Object.fromEntries(
      Object.entries(normalized.system_enabled).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
    custom_enabled: Object.fromEntries(
      Object.entries(normalized.custom_enabled).sort(([left], [right]) =>
        left.localeCompare(right),
      ),
    ),
  });
};

const readManifestCache = async (): Promise<WAFManifestCache> =>
  readJSONFile<WAFManifestCache>(MANIFEST_CACHE_PATH, {
    manifest: null,
    cached_at: null,
    last_checked_at: null,
    last_error: null,
  });

const readSystemSyncState = async (): Promise<WAFSystemSyncState | null> =>
  readJSONFile<WAFSystemSyncState | null>(SYSTEM_SYNC_PATH, null);

const getManifestRules = (
  manifest: WAFRemoteManifest | null,
): WAFManifestRule[] =>
  Array.isArray(manifest?.rulesDescription?.rules)
    ? manifest.rulesDescription.rules
        .filter((rule) => rule && typeof rule.filename === "string")
        .map((rule) => ({
          filename: rule.filename.trim(),
          description: String(rule.description || "").trim(),
        }))
    : [];

const isManifestStale = (cache: WAFManifestCache): boolean => {
  const checkedAt = cache.last_checked_at || cache.cached_at;
  const checkedMs = checkedAt ? Date.parse(checkedAt) : 0;
  return (
    !Number.isFinite(checkedMs) || Date.now() - checkedMs > MANIFEST_REFRESH_MS
  );
};

const validateManifest = (value: any): WAFRemoteManifest => {
  if (!value || typeof value !== "object") {
    throw new Error("系统规则清单格式不正确");
  }
  const zipFile = String(value.zipFile || "").trim();
  const zipHash = String(value.zipHash || "").trim();
  if (!zipFile || !zipHash) {
    throw new Error("系统规则清单缺少 zip 文件信息");
  }
  return {
    ...value,
    zipFile,
    zipHash,
  } as WAFRemoteManifest;
};

export const refreshSystemManifestCache =
  async (): Promise<WAFManifestCache> => {
    await ensureWAFDirectories();
    const checkedAt = nowISO();
    const previous = await readManifestCache();
    try {
      const response = await fetch(cacheBustedURL(MANIFEST_URL), {
        headers: {
          "cache-control": "no-cache, no-store",
          pragma: "no-cache",
        },
      });
      if (!response.ok) {
        throw new Error(`系统规则清单请求失败: HTTP ${response.status}`);
      }
      const manifest = validateManifest(await response.json());
      const cache: WAFManifestCache = {
        manifest,
        cached_at: checkedAt,
        last_checked_at: checkedAt,
        last_error: null,
      };
      await writeJSONFile(MANIFEST_CACHE_PATH, cache);
      return cache;
    } catch (error) {
      const cache: WAFManifestCache = {
        ...previous,
        last_checked_at: checkedAt,
        last_error: errorMessage(error, "系统规则清单刷新失败"),
      };
      await writeJSONFile(MANIFEST_CACHE_PATH, cache);
      throw error;
    }
  };

const getManifestCacheForDetails = async (): Promise<WAFManifestCache> => {
  let cache = await readManifestCache();
  if (!cache.manifest || isManifestStale(cache)) {
    try {
      cache = await refreshSystemManifestCache();
    } catch {
      cache = await readManifestCache();
    }
  }
  return cache;
};

const safeRuleFilename = (value: string): string => {
  const raw = basename(value.replace(/\\/g, "/")).trim();
  if (!raw || raw === "." || raw === ".." || !CONF_EXT_RE.test(raw)) {
    throw new Error("只支持 .conf 规则文件");
  }
  const safe = raw.replace(/[^A-Za-z0-9._-]/g, "-");
  if (!safe || !CONF_EXT_RE.test(safe)) {
    throw new Error("规则文件名不正确");
  }
  return safe;
};

const decodeUTF8Rule = (content: Buffer, filename: string): string => {
  if (content.length > MAX_RULE_FILE_BYTES) {
    throw new Error(`${filename} 超过 1MB`);
  }
  const text = content.toString("utf8").replace(/^\uFEFF/, "");
  if (text.includes("\uFFFD")) {
    throw new Error(`${filename} 不是有效的 UTF-8 文本`);
  }
  if (BLOCKED_DIRECTIVE_RE.test(text)) {
    throw new Error(`${filename} 包含不允许的文件系统指令`);
  }
  return text;
};

const readUTF8RuleText = (content: Buffer, filename: string): string => {
  if (content.length > MAX_RULE_FILE_BYTES) {
    throw new Error(`${filename} 超过 1MB`);
  }
  const text = content.toString("utf8").replace(/^\uFEFF/, "");
  if (text.includes("\uFFFD")) {
    throw new Error(`${filename} 不是有效的 UTF-8 文本`);
  }
  return text;
};

const listRuleFiles = async (
  source: WAFRuleSource,
  manifest: WAFRemoteManifest | null,
  state: WAFRulesState,
): Promise<WAFRuleFile[]> => {
  const dir = source === "system" ? SYSTEM_DIR : CUSTOM_DIR;
  const enabledMap =
    source === "system" ? state.system_enabled : state.custom_enabled;
  const descriptions = new Map(
    getManifestRules(manifest).map((rule) => [rule.filename, rule.description]),
  );

  let entries: Dirent<string>[];
  try {
    entries = await readdir(dir, { withFileTypes: true, encoding: "utf8" });
  } catch (error: any) {
    if (error?.code === "ENOENT") return [];
    throw error;
  }

  const files: WAFRuleFile[] = [];
  for (const entry of entries) {
    if (!entry.isFile() || !CONF_EXT_RE.test(entry.name)) continue;
    if (
      source === "system" &&
      entry.name === SYSTEM_INITIALIZATION_RULE_FILENAME
    )
      continue;
    const filePath = join(dir, entry.name);
    const info = await stat(filePath);
    files.push({
      source,
      filename: entry.name,
      description:
        descriptions.get(entry.name) ||
        (source === "system" ? "系统安全规则" : "用户上传规则"),
      enabled:
        enabledMap[entry.name] ??
        (source === "system" ? isSystemRuleEnabledByDefault(entry.name) : true),
      size_bytes: info.size,
      updated_at: info.mtime.toISOString(),
    });
  }
  return files.sort((left, right) =>
    left.filename.localeCompare(right.filename),
  );
};

const hasSystemRuleFiles = async (): Promise<boolean> => {
  try {
    const entries = await readdir(SYSTEM_DIR, {
      withFileTypes: true,
      encoding: "utf8",
    });
    return entries.some(
      (entry) =>
        entry.isFile() &&
        CONF_EXT_RE.test(entry.name) &&
        entry.name !== SYSTEM_INITIALIZATION_RULE_FILENAME,
    );
  } catch (error: any) {
    if (error?.code === "ENOENT") return false;
    throw error;
  }
};

const getDefaultDisabledSystemRuleFilenames = (
  rules: Pick<WAFRuleFile, "filename">[],
): string[] =>
  rules
    .map((rule) => rule.filename)
    .filter((filename) => DEFAULT_DISABLED_SYSTEM_RULE_FILENAMES.has(filename))
    .sort((left, right) => left.localeCompare(right));

const shouldApplyDefaultDisabledSystemRulesPatch = (
  systemRules: WAFRuleFile[],
): boolean =>
  systemRules.length > 0 && systemRules.every((rule) => rule.enabled);

const markDefaultDisabledSystemRulesPatchApplied = async (): Promise<void> => {
  await redis.set(DEFAULT_DISABLED_SYSTEM_RULES_PATCH_FLAG_KEY, "1");
};

export const applyDefaultDisabledSystemRulesPatchIfNeeded =
  async (): Promise<WAFDefaultDisabledSystemRulesPatchResult> => {
    const patchFlag = await redis.get(
      DEFAULT_DISABLED_SYSTEM_RULES_PATCH_FLAG_KEY,
    );
    if (patchFlag === "1") {
      return { applied: false, disabled_filenames: [] };
    }

    await ensureWAFDirectories();
    const state = await readRulesState();
    const systemRules = await listRuleFiles("system", null, state);
    const disabledFilenames =
      getDefaultDisabledSystemRuleFilenames(systemRules);

    if (
      shouldApplyDefaultDisabledSystemRulesPatch(systemRules) &&
      disabledFilenames.length > 0
    ) {
      await writeRulesState({
        system_enabled: {
          ...state.system_enabled,
          ...Object.fromEntries(
            disabledFilenames.map((filename) => [filename, false]),
          ),
        },
        custom_enabled: state.custom_enabled,
      });
      await markDefaultDisabledSystemRulesPatchApplied();
      return { applied: true, disabled_filenames: disabledFilenames };
    }

    await markDefaultDisabledSystemRulesPatchApplied();
    return { applied: false, disabled_filenames: [] };
  };

const normalizeFixedWAFConfig = (
  value?: Partial<WAFConfig> | null,
): WAFConfig =>
  normalizeWAFConfig({
    ...value,
    mode: DEFAULT_WAF_CONFIG.mode,
    rules_dir: ROOT_DIR,
    active_bundle_id: "local",
    inbound_anomaly_threshold: DEFAULT_WAF_CONFIG.inbound_anomaly_threshold,
    outbound_anomaly_threshold: DEFAULT_WAF_CONFIG.outbound_anomaly_threshold,
    request_body_access: DEFAULT_WAF_CONFIG.request_body_access,
    request_body_limit_bytes: DEFAULT_WAF_CONFIG.request_body_limit_bytes,
    request_body_in_memory_limit_bytes:
      DEFAULT_WAF_CONFIG.request_body_in_memory_limit_bytes,
    response_body_access: DEFAULT_WAF_CONFIG.response_body_access,
    disabled_hosts: [],
    disabled_path_prefixes: [],
  });

const toGatewayWAFConfig = (config: WAFConfig): GatewayWAFConfig => ({
  enabled: config.enabled,
  mode: config.mode,
  active_bundle_id: config.active_bundle_id,
  rules_dir: config.rules_dir,
  paranoia_level: config.paranoia_level,
  executing_paranoia_level: config.executing_paranoia_level,
  inbound_anomaly_threshold: config.inbound_anomaly_threshold,
  outbound_anomaly_threshold: config.outbound_anomaly_threshold,
  request_body_access: config.request_body_access,
  request_body_limit_bytes: config.request_body_limit_bytes,
  request_body_in_memory_limit_bytes: config.request_body_in_memory_limit_bytes,
  response_body_access: config.response_body_access,
  disabled_hosts: config.disabled_hosts,
  disabled_path_prefixes: config.disabled_path_prefixes,
  log_retention_days: config.log_retention_days,
  drain_interval_seconds: config.drain_interval_seconds,
  updated_at: config.updated_at,
});

const hasAnyEnabledRules = (details: Pick<WAFDetails, "system" | "custom">) =>
  [...details.system.rules, ...details.custom.rules].some(
    (rule) => rule.enabled,
  );

const hasAnyEnabledRuleFiles = async (
  state: WAFRulesState,
  omit?: { source: WAFRuleSource; filename: string },
): Promise<boolean> => {
  const manifestCache = await getManifestCacheForDetails();
  const [systemRules, customRules] = await Promise.all([
    listRuleFiles("system", manifestCache.manifest, state),
    listRuleFiles("custom", manifestCache.manifest, state),
  ]);
  return [...systemRules, ...customRules].some(
    (rule) =>
      rule.enabled &&
      !(omit && rule.source === omit.source && rule.filename === omit.filename),
  );
};

const applyWAFConfigToGateway = async (
  config: WAFConfig,
  emptyRulesMessage = "至少启用一个 WAF 规则文件后再开启",
): Promise<void> => {
  const next = normalizeFixedWAFConfig(config);
  if (!next.enabled) {
    await syncWAFConfigToGateway(next);
    return;
  }

  const details = await getWAFDetails();
  if (!hasAnyEnabledRules(details)) {
    throw new Error(emptyRulesMessage);
  }
  const response = await goBackend.reloadWAFRules(toGatewayWAFConfig(next));
  if (!response.success || !response.data) {
    throw new Error(response.message || "WAF 规则加载失败");
  }
};

const applyStoredWAFConfigToGateway = async (
  emptyRulesMessage?: string,
): Promise<void> => {
  await applyWAFConfigToGateway(
    await configManager.getWAFConfig(),
    emptyRulesMessage,
  );
};

export const syncWAFConfigToGateway = async (
  config?: WAFConfig | null,
): Promise<WAFStatus> => {
  const next = normalizeFixedWAFConfig(
    config ?? (await configManager.getWAFConfig()),
  );
  const response = await goBackend.setWAFConfig(toGatewayWAFConfig(next));
  if (!response.success || !response.data) {
    throw new Error(response.message || "同步 WAF 配置到网关失败");
  }
  return response.data;
};

export const getWAFDetails = async (): Promise<WAFDetails> => {
  await ensureWAFDirectories();
  const [config, statusResponse, manifestCache, synced, state] =
    await Promise.all([
      configManager.getWAFConfig(),
      goBackend.getWAFStatus(),
      getManifestCacheForDetails(),
      readSystemSyncState(),
      readRulesState(),
    ]);
  const normalizedConfig = normalizeFixedWAFConfig(config);
  const [systemRules, customRules] = await Promise.all([
    listRuleFiles("system", manifestCache.manifest, state),
    listRuleFiles("custom", manifestCache.manifest, state),
  ]);
  const updateAvailable = Boolean(
    manifestCache.manifest?.zipHash &&
    manifestCache.manifest.zipHash !== synced?.zip_hash,
  );

  return {
    config: normalizedConfig,
    status:
      statusResponse.success && statusResponse.data
        ? statusResponse.data
        : null,
    rules_dir: ROOT_DIR,
    system: {
      manifest: manifestCache.manifest,
      manifest_cached_at: manifestCache.cached_at,
      manifest_last_checked_at: manifestCache.last_checked_at,
      manifest_last_error: manifestCache.last_error,
      synced,
      update_available: updateAvailable,
      rules: systemRules,
    },
    custom: {
      rules: customRules,
    },
  };
};

export const readWAFRuleFile = async (
  source: WAFRuleSource,
  filename: string,
): Promise<WAFRuleFileContent> => {
  await ensureWAFDirectories();
  if (source !== "system" && source !== "custom") {
    throw new Error("规则来源不正确");
  }
  const safe = safeRuleFilename(filename);
  const manifestCache = await getManifestCacheForDetails();
  const state = await readRulesState();
  const rules = await listRuleFiles(source, manifestCache.manifest, state);
  const rule = rules.find((item) => item.filename === safe);
  if (!rule) {
    throw new Error("规则文件不存在");
  }
  const dir = source === "system" ? SYSTEM_DIR : CUSTOM_DIR;
  const content = readUTF8RuleText(await readFile(join(dir, safe)), safe);
  return {
    ...rule,
    content,
  };
};

const findZipEndOfCentralDirectory = (buffer: Buffer): number => {
  const min = Math.max(0, buffer.length - 65557);
  for (let index = buffer.length - 22; index >= min; index -= 1) {
    if (buffer.readUInt32LE(index) === 0x06054b50) return index;
  }
  throw new Error("系统规则 zip 格式不正确");
};

const parseZipEntries = (buffer: Buffer): ArchiveEntry[] => {
  const eocd = findZipEndOfCentralDirectory(buffer);
  const entryCount = buffer.readUInt16LE(eocd + 10);
  let offset = buffer.readUInt32LE(eocd + 16);
  const entries: ArchiveEntry[] = [];
  let unpackedBytes = 0;
  for (let index = 0; index < entryCount; index += 1) {
    if (buffer.readUInt32LE(offset) !== 0x02014b50) {
      throw new Error("系统规则 zip 目录不正确");
    }
    const flags = buffer.readUInt16LE(offset + 8);
    const method = buffer.readUInt16LE(offset + 10);
    const compressedSize = buffer.readUInt32LE(offset + 20);
    const uncompressedSize = buffer.readUInt32LE(offset + 24);
    const nameLength = buffer.readUInt16LE(offset + 28);
    const extraLength = buffer.readUInt16LE(offset + 30);
    const commentLength = buffer.readUInt16LE(offset + 32);
    const localHeaderOffset = buffer.readUInt32LE(offset + 42);
    const name = buffer
      .subarray(offset + 46, offset + 46 + nameLength)
      .toString(flags & 0x0800 ? "utf8" : "binary");
    if (!name.endsWith("/")) {
      unpackedBytes += uncompressedSize;
      if (unpackedBytes > MAX_UNPACKED_ZIP_BYTES) {
        throw new Error("系统规则包解压后过大");
      }
      if (buffer.readUInt32LE(localHeaderOffset) !== 0x04034b50) {
        throw new Error("系统规则 zip 文件头不正确");
      }
      const localNameLength = buffer.readUInt16LE(localHeaderOffset + 26);
      const localExtraLength = buffer.readUInt16LE(localHeaderOffset + 28);
      const dataStart =
        localHeaderOffset + 30 + localNameLength + localExtraLength;
      const compressed = buffer.subarray(dataStart, dataStart + compressedSize);
      const content =
        method === 0
          ? compressed
          : method === 8
            ? inflateRawSync(compressed)
            : null;
      if (!content) {
        throw new Error(`暂不支持 zip 压缩方式 ${method}`);
      }
      if (content.length !== uncompressedSize) {
        throw new Error("系统规则 zip 文件大小不正确");
      }
      entries.push({ path: name, content });
    }
    offset += 46 + nameLength + extraLength + commentLength;
  }
  return entries;
};

const safeBundleEntryPath = (value: string): string => {
  const normalized = value.replace(/\\/g, "/");
  const segments = normalized.split("/");
  if (
    !normalized ||
    normalized !== normalized.trim() ||
    normalized.startsWith("/") ||
    normalized.includes("://") ||
    segments.some(
      (segment) => !segment || segment === "." || segment === "..",
    ) ||
    !BUNDLE_ENTRY_PATH_RE.test(normalized)
  ) {
    throw new Error(`系统规则 zip 文件路径不正确: ${value}`);
  }
  return segments.join("/");
};

const downloadSystemZip = async (
  manifest: WAFRemoteManifest,
): Promise<Buffer> => {
  const zipURL = cacheBustedURL(manifest.zipFile, MANIFEST_URL);
  const response = await fetch(zipURL, {
    headers: {
      "cache-control": "no-cache, no-store",
      pragma: "no-cache",
    },
  });
  if (!response.ok) {
    throw new Error(`系统规则下载失败: HTTP ${response.status}`);
  }
  const buffer = Buffer.from(await response.arrayBuffer());
  if (buffer.length > MAX_ZIP_BYTES) {
    throw new Error("系统规则包过大");
  }
  const actualHash = sha256Hex(buffer);
  if (actualHash !== manifest.zipHash.toLowerCase()) {
    throw new Error("系统规则包校验失败");
  }
  return buffer;
};

const syncSystemWAFRulesFromManifest = async (
  manifest: WAFRemoteManifest,
): Promise<WAFDetails> => {
  const zipBuffer = await downloadSystemZip(manifest);
  const entries = parseZipEntries(zipBuffer);
  if (entries.length === 0) {
    throw new Error("系统规则包为空");
  }

  const bundleFiles = new Map<string, Buffer>();
  const bundlePathKeys = new Set<string>();
  const ruleFiles = new Map<string, string>();
  for (const entry of entries) {
    const relativePath = safeBundleEntryPath(entry.path);
    const pathKey = relativePath.toLowerCase();
    if (bundlePathKeys.has(pathKey)) {
      throw new Error(`系统规则包内存在重复文件: ${relativePath}`);
    }
    bundlePathKeys.add(pathKey);

    const filename = basename(relativePath);
    if (CONF_EXT_RE.test(filename)) {
      if (relativePath !== filename) {
        throw new Error("系统规则包内 .conf 文件必须位于根目录");
      }
      const content = decodeUTF8Rule(entry.content, filename);
      bundleFiles.set(relativePath, Buffer.from(content, "utf8"));
      ruleFiles.set(filename, content);
    } else {
      bundleFiles.set(relativePath, entry.content);
    }
  }
  if (ruleFiles.size === 0) {
    throw new Error("系统规则包内没有 .conf 文件");
  }

  const tempDir = `${SYSTEM_DIR}.tmp-${Date.now()}`;
  await rm(tempDir, { recursive: true, force: true });
  await mkdir(tempDir, { recursive: true });
  for (const [relativePath, content] of [...bundleFiles.entries()].sort(
    ([left], [right]) => left.localeCompare(right),
  )) {
    const filePath = join(tempDir, relativePath);
    const tempRelativePath = relative(tempDir, filePath);
    if (
      tempRelativePath === ".." ||
      tempRelativePath.startsWith(`..${sep}`) ||
      tempRelativePath === ""
    ) {
      throw new Error("系统规则文件路径不正确");
    }
    await mkdir(dirname(filePath), { recursive: true });
    await writeFile(filePath, content);
  }
  await rm(SYSTEM_DIR, { recursive: true, force: true });
  await rename(tempDir, SYSTEM_DIR);

  const state = await readRulesState();
  const previousSystemState = state.system_enabled;
  state.system_enabled = Object.fromEntries(
    [...ruleFiles.keys()]
      .sort()
      .map((filename) => [
        filename,
        previousSystemState[filename] ?? isSystemRuleEnabledByDefault(filename),
      ]),
  );
  await writeRulesState(state);

  await writeJSONFile(SYSTEM_SYNC_PATH, {
    zip_file: manifest.zipFile,
    zip_hash: manifest.zipHash,
    synced_at: nowISO(),
    packaging_time: manifest.packagingTime,
    commit_hash: manifest.commitHash,
    commit_date: manifest.commitDate,
  } satisfies WAFSystemSyncState);

  await applyStoredWAFConfigToGateway();
  return getWAFDetails();
};

export const syncSystemWAFRules = async (): Promise<WAFDetails> => {
  const cache = await refreshSystemManifestCache();
  const manifest = cache.manifest;
  if (!manifest) throw new Error("系统规则清单为空");
  return syncSystemWAFRulesFromManifest(manifest);
};

export const checkAndSyncSystemWAFRulesIfNeeded =
  async (): Promise<WAFSystemRulesAutoUpdateResult> => {
    await ensureWAFDirectories();
    const checkedAt = nowISO();
    const config = normalizeFixedWAFConfig(await configManager.getWAFConfig());
    if (!config.enabled) {
      return {
        checked_at: checkedAt,
        updated: false,
        skipped_reason: "waf_disabled",
      };
    }
    if (!config.system_rules_auto_update_enabled) {
      return {
        checked_at: checkedAt,
        updated: false,
        skipped_reason: "disabled",
      };
    }

    const locked = await configManager.setLockIfNotExists(
      "waf-system-rules-auto-update",
      SYSTEM_RULES_AUTO_UPDATE_LOCK_TTL_SECONDS,
    );
    if (!locked) {
      return {
        checked_at: checkedAt,
        updated: false,
        skipped_reason: "locked",
      };
    }

    const cache = await refreshSystemManifestCache();
    const manifest = cache.manifest;
    if (!manifest) throw new Error("系统规则清单为空");

    const [synced, hasLocalRules] = await Promise.all([
      readSystemSyncState(),
      hasSystemRuleFiles(),
    ]);
    const updateAvailable = manifest.zipHash !== synced?.zip_hash;
    if (!updateAvailable && hasLocalRules) {
      return {
        checked_at: checkedAt,
        updated: false,
        manifest_zip_hash: manifest.zipHash,
        synced_zip_hash: synced?.zip_hash ?? null,
        skipped_reason: "up_to_date",
      };
    }

    await syncSystemWAFRulesFromManifest(manifest);
    return {
      checked_at: checkedAt,
      updated: true,
      manifest_zip_hash: manifest.zipHash,
      synced_zip_hash: synced?.zip_hash ?? null,
    };
  };

export const setWAFRuleEnabled = async (
  input: WAFRuleToggleInput,
): Promise<WAFDetails> => {
  await ensureWAFDirectories();
  const source = input.source === "custom" ? "custom" : "system";
  const details = await getWAFDetails();
  const existing =
    source === "system" ? details.system.rules : details.custom.rules;
  const filenames =
    input.filenames && input.filenames.length > 0
      ? input.filenames.map(safeRuleFilename)
      : existing.map((rule) => rule.filename);
  const existingNames = new Set(existing.map((rule) => rule.filename));
  const state = await readRulesState();
  const nextState: WAFRulesState = {
    system_enabled: { ...state.system_enabled },
    custom_enabled: { ...state.custom_enabled },
  };
  const enabledMap =
    source === "system" ? nextState.system_enabled : nextState.custom_enabled;
  for (const filename of filenames) {
    if (!existingNames.has(filename)) continue;
    enabledMap[filename] = Boolean(input.enabled);
  }
  if (details.config.enabled && !(await hasAnyEnabledRuleFiles(nextState))) {
    throw new Error("开启 WAF 时至少保留一个启用的规则文件");
  }
  await writeRulesState(nextState);
  await applyWAFConfigToGateway(details.config);
  return getWAFDetails();
};

const makeUniqueCustomFilename = async (filename: string): Promise<string> => {
  const ext = extname(filename);
  const base = basename(filename, ext);
  let candidate = filename;
  let index = 1;
  for (;;) {
    try {
      await stat(join(CUSTOM_DIR, candidate));
      candidate = `${base}-${index}${ext}`;
      index += 1;
    } catch (error: any) {
      if (error?.code === "ENOENT") return candidate;
      throw error;
    }
  }
};

export const uploadCustomWAFRules = async (
  input: WAFUploadInput,
): Promise<WAFDetails> => {
  await ensureWAFDirectories();
  if (!Array.isArray(input.files) || input.files.length === 0) {
    throw new Error("请选择要上传的 .conf 文件");
  }
  const state = await readRulesState();
  for (const file of input.files) {
    const filename = await makeUniqueCustomFilename(
      safeRuleFilename(file.filename),
    );
    const raw = Buffer.from(String(file.content_base64 || ""), "base64");
    const content = decodeUTF8Rule(raw, filename);
    await writeFile(join(CUSTOM_DIR, filename), content, "utf8");
    state.custom_enabled[filename] = true;
  }
  await writeRulesState(state);
  await applyStoredWAFConfigToGateway();
  return getWAFDetails();
};

export const deleteCustomWAFRule = async (
  filename: string,
): Promise<WAFDetails> => {
  await ensureWAFDirectories();
  const safe = safeRuleFilename(filename);
  const config = normalizeFixedWAFConfig(await configManager.getWAFConfig());
  if (config.enabled) {
    const state = await readRulesState();
    const canDelete = await hasAnyEnabledRuleFiles(state, {
      source: "custom",
      filename: safe,
    });
    if (!canDelete) {
      throw new Error("开启 WAF 时至少保留一个启用的规则文件");
    }
  }
  try {
    await unlink(join(CUSTOM_DIR, safe));
  } catch (error: any) {
    if (error?.code !== "ENOENT") throw error;
  }
  const state = await readRulesState();
  delete state.custom_enabled[safe];
  await writeRulesState(state);
  await applyWAFConfigToGateway(config);
  return getWAFDetails();
};

export const applyWAFConfig = async (
  patch: WAFConfigPatch,
): Promise<WAFDetails> => {
  const current = await configManager.getWAFConfig();
  const next = normalizeFixedWAFConfig({
    ...current,
    ...patch,
  });
  const saved = await configManager.updateWAFConfig(next);
  const shouldApplyToGateway =
    Object.prototype.hasOwnProperty.call(patch, "enabled") ||
    Object.prototype.hasOwnProperty.call(patch, "paranoia_level") ||
    Object.prototype.hasOwnProperty.call(patch, "executing_paranoia_level");
  if (shouldApplyToGateway) {
    await applyWAFConfigToGateway(saved);
  }
  if (
    shouldApplyToGateway ||
    Object.prototype.hasOwnProperty.call(
      patch,
      "common_location_exempt_enabled",
    )
  ) {
    await syncCommonAuthLocationExemptionsToGateway();
  }
  return getWAFDetails();
};

export const syncWAFToGatewayOnBoot = async (): Promise<void> => {
  await ensureWAFDirectories();
  const defaultDisabledRulesPatch =
    await applyDefaultDisabledSystemRulesPatchIfNeeded();
  if (defaultDisabledRulesPatch.applied) {
    console.log(
      `[waf] applied default-disabled system rules patch: ${defaultDisabledRulesPatch.disabled_filenames.join(", ")}`,
    );
  }

  const config = normalizeFixedWAFConfig(await configManager.getWAFConfig());
  if (config.enabled && config.system_rules_auto_update_enabled) {
    try {
      const cache = await readManifestCache();
      if (!cache.manifest || isManifestStale(cache)) {
        await refreshSystemManifestCache();
      }
    } catch (error) {
      console.error("[waf] failed to refresh system manifest:", error);
    }
  }

  await syncWAFConfigToGateway(config);
  await syncCommonAuthLocationExemptionsToGateway();
  if (config.enabled) {
    const response = await goBackend.reloadWAFRules(toGatewayWAFConfig(config));
    if (!response.success) {
      throw new Error(response.message || "重新加载 WAF 规则失败");
    }
  }
};

const runWAFSystemRulesAutoUpdate = async () => {
  if (systemRulesAutoUpdateRunning) {
    return {
      checked_at: nowISO(),
      updated: false,
      skipped_reason: "running",
    } satisfies WAFSystemRulesAutoUpdateResult;
  }
  systemRulesAutoUpdateRunning = true;
  try {
    return await checkAndSyncSystemWAFRulesIfNeeded();
  } finally {
    systemRulesAutoUpdateRunning = false;
  }
};

export const startWAFSystemRulesAutoUpdate = (): void => {
  if (systemRulesAutoUpdateTimer) return;
  systemRulesAutoUpdateTimer = setInterval(() => {
    runWAFSystemRulesAutoUpdate()
      .then((result) => {
        if (result.updated) {
          console.log(
            `[waf] system rules auto updated: ${result.synced_zip_hash || "none"} -> ${result.manifest_zip_hash || "unknown"}`,
          );
        }
      })
      .catch((error) => {
        console.error("[waf] failed to auto update system rules:", error);
      });
  }, SYSTEM_RULES_AUTO_UPDATE_MS);
  systemRulesAutoUpdateTimer.unref?.();
};

export const drainWAFEventsNow = async () => wafCollector.drainOnce();
