import type { BackupEmailConfig } from "@/types";

export const defaultBackupEmail = (): BackupEmailConfig => ({
  enabled: false,
  smtp: {
    host: "",
    port: 465,
    security: "ssl_tls",
    auth_mode: "auto",
    username: "",
    timeout_seconds: 30,
  },
  from_address: "",
  from_name: "fn-knock",
  to_addresses: [],
  attachment_limit_mib: 20,
  password_configured: false,
});
export type BackupEmailForm = BackupEmailConfig & {
  password?: string;
  clear_password?: boolean;
};
export function backupEmailPayload(form: BackupEmailForm) {
  const {
    password_configured: _configured,
    password,
    clear_password,
    ...config
  } = form;
  return {
    ...config,
    ...(clear_password ? { clear_password: true } : {}),
    ...(password ? { password } : {}),
  };
}
export function isBackupEmailValid(form: BackupEmailForm) {
  const validNumber = (value: number, min: number, max: number) =>
    Number.isInteger(value) && value >= min && value <= max;
  if (
    !validNumber(form.smtp.port, 1, 65535) ||
    !validNumber(form.smtp.timeout_seconds, 1, 120) ||
    !validNumber(form.attachment_limit_mib, 1, 100)
  )
    return false;
  if (!form.enabled) return true;
  return !!(
    form.smtp.host.trim() &&
    form.from_address.trim() &&
    form.to_addresses.length &&
    form.to_addresses.every((address) => address.trim()) &&
    (form.smtp.auth_mode === "none" || form.smtp.username.trim())
  );
}

export function backupEmailErrorKey(code: string) {
  const keys: Record<string, string> = {
    smtp_unavailable: "emailErrorConnection",
    smtp_rejected: "emailErrorRejected",
    smtp_timeout: "emailErrorTimeout",
    attachment_too_large: "emailErrorAttachment",
    backup_missing: "emailErrorFile",
    backup_unreadable: "emailErrorFile",
    invalid_backup_path: "emailErrorFile",
    delivery_expired: "emailErrorExpired",
    credential_unavailable: "emailErrorCredentials",
  };
  return `admin.maintenanceSettings.${keys[code] ?? "emailErrorInvalid"}`;
}
