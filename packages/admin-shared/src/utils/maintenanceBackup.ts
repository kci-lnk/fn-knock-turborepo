export const KNOCK_BACKUP_PREFIX = "fn_knock:";
export const KNOCK_BACKUP_EXTENSION = ".knock";
export const KNOCK_BACKUP_JSON_FILENAME = "fn-knock-backup.json";

export const buildKnockBackupFilename = (value: Date | string = new Date()) => {
  const date =
    value instanceof Date
      ? value
      : Number.isFinite(Date.parse(value))
        ? new Date(value)
        : new Date();

  return `fn-knock-backup-${date.toISOString().replace(/[:.]/g, "-")}${KNOCK_BACKUP_EXTENSION}`;
};
