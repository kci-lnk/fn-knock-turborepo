import { tDefault } from "../i18n";

export const backupT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.maintenanceBackup.${key}`, params);
