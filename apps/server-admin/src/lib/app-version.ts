import appMetadata from "../../../../version.json";

/**
 * 本地当前版本号
 */
export const APP_LOCAL_VERSION = appMetadata.version;

export const APP_GITHUB_URL = appMetadata.githubUrl;

export const APP_BACKUP_SCHEMA_VERSION = appMetadata.backupSchemaVersion;

export const APP_BACKUP_IMPORT_VERSION_RANGE = {
  min: appMetadata.backupImportMinVersion,
  max: APP_LOCAL_VERSION,
} as const;

const normalizeVersion = (version: string): number[] => {
  return version
    .trim()
    .split(".")
    .map((part) => {
      const match = part.match(/\d+/);
      return match ? Number.parseInt(match[0], 10) : 0;
    });
};

export const compareVersion = (a: string, b: string): number => {
  const pa = normalizeVersion(a);
  const pb = normalizeVersion(b);
  const max = Math.max(pa.length, pb.length, 3);

  for (let i = 0; i < max; i += 1) {
    const av = pa[i] ?? 0;
    const bv = pb[i] ?? 0;
    if (av > bv) return 1;
    if (av < bv) return -1;
  }

  return 0;
};

export const formatVersionRange = (range: {
  min: string;
  max: string;
}): string => {
  return range.min === range.max ? range.min : `${range.min} ~ ${range.max}`;
};

export const isBackupAppVersionSupported = (version: string): boolean => {
  const normalizedVersion = version.trim();
  if (!normalizedVersion) {
    return false;
  }

  return (
    compareVersion(normalizedVersion, APP_BACKUP_IMPORT_VERSION_RANGE.min) >= 0 &&
    compareVersion(normalizedVersion, APP_BACKUP_IMPORT_VERSION_RANGE.max) <= 0
  );
};
