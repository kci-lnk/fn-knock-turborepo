import {
  APP_BACKUP_IMPORT_VERSION_RANGE,
  APP_BACKUP_SCHEMA_VERSION,
  APP_LOCAL_VERSION,
  formatVersionRange,
  isBackupAppVersionSupported,
} from "../app-version";
import {
  KNOCK_BACKUP_PREFIX,
} from "../../../../../packages/admin-shared/src/utils/maintenanceBackup";
import { MaintenanceBackupError } from "./errors";
import { backupT } from "./messages";

const SUPPORTED_BACKUP_IMPORT_VERSION_RANGE = formatVersionRange(
  APP_BACKUP_IMPORT_VERSION_RANGE,
);

export type RedisBackupValueType =
  | "string"
  | "hash"
  | "list"
  | "set"
  | "zset"
  | "stream";

export type RedisZSetEntry = {
  member: string;
  score: number;
};

export type RedisStreamEntry = {
  id: string;
  fields: string[];
};

export type RedisBackupEntry =
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

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value);

const isSupportedType = (value: unknown): value is RedisBackupValueType =>
  value === "string" ||
  value === "hash" ||
  value === "list" ||
  value === "set" ||
  value === "zset" ||
  value === "stream";

const parseStringArray = (value: unknown, label: string): string[] => {
  if (!Array.isArray(value)) {
    throw new MaintenanceBackupError(
      backupT("stringArrayRequired", { label }),
      400,
    );
  }

  const output = value.filter(
    (item): item is string => typeof item === "string",
  );
  if (output.length !== value.length) {
    throw new MaintenanceBackupError(
      backupT("stringArrayOnlyStrings", { label }),
      400,
    );
  }
  return output;
};

const parseHashValue = (
  value: unknown,
  label: string,
): Record<string, string> => {
  if (!isRecord(value)) {
    throw new MaintenanceBackupError(backupT("objectRequired", { label }), 400);
  }

  const output: Record<string, string> = {};
  for (const [field, rawFieldValue] of Object.entries(value)) {
    if (typeof rawFieldValue !== "string") {
      throw new MaintenanceBackupError(
        backupT("fieldStringRequired", { label, field }),
        400,
      );
    }
    output[field] = rawFieldValue;
  }
  return output;
};

const parseZSetValue = (value: unknown, label: string): RedisZSetEntry[] => {
  if (!Array.isArray(value)) {
    throw new MaintenanceBackupError(backupT("arrayRequired", { label }), 400);
  }

  return value.map((item, index) => {
    if (!isRecord(item) || typeof item.member !== "string") {
      throw new MaintenanceBackupError(
        backupT("zsetMemberRequired", { label, index }),
        400,
      );
    }

    const score = Number(item.score);
    if (!Number.isFinite(score)) {
      throw new MaintenanceBackupError(
        backupT("zsetScoreRequired", { label, index }),
        400,
      );
    }

    return { member: item.member, score };
  });
};

const parseStreamValue = (
  value: unknown,
  label: string,
): RedisStreamEntry[] => {
  if (!Array.isArray(value)) {
    throw new MaintenanceBackupError(backupT("arrayRequired", { label }), 400);
  }

  return value.map((item, index) => {
    if (!isRecord(item) || typeof item.id !== "string") {
      throw new MaintenanceBackupError(
        backupT("streamIdRequired", { label, index }),
        400,
      );
    }

    const fields = parseStringArray(item.fields, `${label}[${index}].fields`);
    if (fields.length === 0 || fields.length % 2 !== 0) {
      throw new MaintenanceBackupError(
        backupT("streamFieldsInvalid", { label, index }),
        400,
      );
    }

    return {
      id: item.id,
      fields,
    };
  });
};

const parseEntry = (value: unknown, index: number): RedisBackupEntry => {
  if (!isRecord(value)) {
    throw new MaintenanceBackupError(backupT("entryObjectRequired", { index }), 400);
  }

  const key = typeof value.key === "string" ? value.key : "";
  if (!key.startsWith(KNOCK_BACKUP_PREFIX)) {
    throw new MaintenanceBackupError(
      backupT("entryKeyPrefixRequired", {
        index,
        prefix: KNOCK_BACKUP_PREFIX,
      }),
      400,
    );
  }

  if (!isSupportedType(value.type)) {
    throw new MaintenanceBackupError(
      backupT("entryTypeUnsupported", { index }),
      400,
    );
  }

  const ttlMs =
    value.ttl_ms == null
      ? null
      : Number.isFinite(Number(value.ttl_ms)) && Number(value.ttl_ms) > 0
        ? Math.floor(Number(value.ttl_ms))
        : (() => {
            throw new MaintenanceBackupError(
              backupT("entryTtlInvalid", { index }),
              400,
            );
          })();

  if (value.type === "string") {
    if (typeof value.value !== "string") {
      throw new MaintenanceBackupError(
        backupT("entryValueStringRequired", { index }),
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
      value: parseHashValue(value.value, `entries[${index}].value`),
    };
  }

  if (value.type === "list" || value.type === "set") {
    return {
      key,
      type: value.type,
      ttl_ms: ttlMs,
      value: parseStringArray(value.value, `entries[${index}].value`),
    };
  }

  if (value.type === "stream") {
    return {
      key,
      type: value.type,
      ttl_ms: ttlMs,
      value: parseStreamValue(value.value, `entries[${index}].value`),
    };
  }

  return {
    key,
    type: value.type,
    ttl_ms: ttlMs,
    value: parseZSetValue(value.value, `entries[${index}].value`),
  };
};

export const parseBackupPayload = (
  rawPayload: unknown,
): FnKnockBackupPayload => {
  let payload: unknown = rawPayload;
  if (typeof rawPayload === "string") {
    try {
      payload = JSON.parse(rawPayload) as unknown;
    } catch {
      throw new MaintenanceBackupError(backupT("jsonParseFailed"), 400);
    }
  }

  if (!isRecord(payload)) {
    throw new MaintenanceBackupError(backupT("payloadObjectInvalid"), 400);
  }

  if (payload.version !== APP_BACKUP_SCHEMA_VERSION) {
    throw new MaintenanceBackupError(
      backupT("unsupportedSchemaVersion", {
        version: APP_BACKUP_SCHEMA_VERSION,
      }),
      400,
    );
  }

  if (payload.prefix !== KNOCK_BACKUP_PREFIX) {
    throw new MaintenanceBackupError(
      backupT("unsupportedPrefix", { prefix: KNOCK_BACKUP_PREFIX }),
      400,
    );
  }

  const appVersion =
    typeof payload.app_version === "string" ? payload.app_version.trim() : "";
  if (!appVersion) {
    throw new MaintenanceBackupError(backupT("missingAppVersion"), 400);
  }

  if (!isBackupAppVersionSupported(appVersion)) {
    throw new MaintenanceBackupError(
      backupT("appVersionUnsupported", {
        currentVersion: APP_LOCAL_VERSION,
        range: SUPPORTED_BACKUP_IMPORT_VERSION_RANGE,
        appVersion,
      }),
      400,
    );
  }

  const exportedAt =
    typeof payload.exported_at === "string" ? payload.exported_at : "";
  if (!exportedAt) {
    throw new MaintenanceBackupError(backupT("missingExportedAt"), 400);
  }

  if (!Array.isArray(payload.entries)) {
    throw new MaintenanceBackupError(backupT("missingEntries"), 400);
  }

  const entries = payload.entries.map((entry, index) =>
    parseEntry(entry, index),
  );
  const uniqueKeys = new Set(entries.map((entry) => entry.key));
  if (uniqueKeys.size !== entries.length) {
    throw new MaintenanceBackupError(backupT("duplicateRedisKey"), 400);
  }

  return {
    version: APP_BACKUP_SCHEMA_VERSION,
    app_version: appVersion,
    prefix: KNOCK_BACKUP_PREFIX,
    exported_at: exportedAt,
    entry_count: entries.length,
    entries,
  };
};
