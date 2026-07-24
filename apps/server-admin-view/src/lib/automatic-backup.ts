import type { SharedDataFileEntry } from "../types";

export const AUTOMATIC_BACKUP_INTERVAL_RANGE = {
  min: 1,
  max: 8760,
} as const;

export const AUTOMATIC_BACKUP_RETENTION_RANGE = {
  min: 1,
  max: 3650,
} as const;

export const isAutomaticBackupConfigValid = (
  intervalHours: number,
  retentionDays: number,
): boolean =>
  Number.isInteger(intervalHours) &&
  intervalHours >= AUTOMATIC_BACKUP_INTERVAL_RANGE.min &&
  intervalHours <= AUTOMATIC_BACKUP_INTERVAL_RANGE.max &&
  Number.isInteger(retentionDays) &&
  retentionDays >= AUTOMATIC_BACKUP_RETENTION_RANGE.min &&
  retentionDays <= AUTOMATIC_BACKUP_RETENTION_RANGE.max;

export const automaticBackupSourceIsAvailable = (
  automaticFileCount: number,
): boolean => automaticFileCount > 0;

export const backupSourceMenuIsRequired = (
  supportsSharedBackup: boolean,
  automaticFileCount: number,
): boolean =>
  supportsSharedBackup || automaticBackupSourceIsAvailable(automaticFileCount);

export const AUTOMATIC_BACKUP_RESULT_POLL_LIMIT = 300;

export const automaticBackupAttemptCompleted = (
  previousAttempt: string | null | undefined,
  nextAttempt: string | null | undefined,
): boolean => !!nextAttempt && nextAttempt !== previousAttempt;

export const automaticBackupAttemptSucceeded = (
  previousSuccess: string | null | undefined,
  nextSuccess: string | null | undefined,
): boolean => !!nextSuccess && nextSuccess !== previousSuccess;

export const buildAutomaticBackupSelectionSummary = (
  file: SharedDataFileEntry,
  size: string,
  sourceLabel: string,
) => ({
  name: file.name,
  size,
  sourceLabel,
  location: file.relativePath,
});
