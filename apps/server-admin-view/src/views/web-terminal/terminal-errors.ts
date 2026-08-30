import { extractErrorMessage } from "@frontend-core/errors/extractErrorMessage";
import type {
  TerminalErrorCode,
  TerminalErrorEnvelope,
} from "@/lib/api/terminal";

const terminalErrorCodes = new Set<TerminalErrorCode>([
  "invalid_request",
  "target_not_found",
  "session_not_found",
  "host_key_required",
  "host_key_mismatch",
  "authentication_failed",
  "pty_rejected",
  "session_limit_reached",
  "session_lost",
  "attachment_expired",
  "controller_conflict",
  "target_revision_conflict",
  "local_terminal_unsupported",
  "local_terminal_disabled",
  "local_terminal_risk_acknowledgement_required",
  "local_terminal_revision_conflict",
  "local_shell_unavailable",
  "local_pty_start_failed",
  "connect_timeout",
  "conflict",
  "upstream_unavailable",
  "internal_error",
]);

export type TerminalRequestError = {
  activeSessionCount: number | null;
  confirmationToken: string | null;
  errorCode: TerminalErrorCode | null;
  message: string;
};

const terminalErrorTranslationKeys: Partial<Record<TerminalErrorCode, string>> =
  {
    local_terminal_unsupported:
      "admin.webTerminal.terminalError.localTerminalUnsupported",
    local_terminal_disabled:
      "admin.webTerminal.terminalError.localTerminalDisabled",
    local_terminal_risk_acknowledgement_required:
      "admin.webTerminal.terminalError.localTerminalRiskAcknowledgementRequired",
    local_terminal_revision_conflict:
      "admin.webTerminal.terminalError.localTerminalRevisionConflict",
    local_shell_unavailable:
      "admin.webTerminal.terminalError.localShellUnavailable",
    local_pty_start_failed:
      "admin.webTerminal.terminalError.localPtyStartFailed",
  };

export const localizeTerminalError = (
  failure: Pick<TerminalRequestError, "errorCode" | "message">,
  translate: (key: string) => string,
) => {
  const key = failure.errorCode
    ? terminalErrorTranslationKeys[failure.errorCode]
    : undefined;
  return key ? translate(key) : failure.message;
};

const getResponseData = (
  error: unknown,
): (Partial<TerminalErrorEnvelope> & Record<string, unknown>) | null => {
  if (!error || typeof error !== "object") return null;
  const responseData = (error as { response?: { data?: unknown } }).response
    ?.data;
  return responseData && typeof responseData === "object"
    ? (responseData as Partial<TerminalErrorEnvelope> & Record<string, unknown>)
    : null;
};

export const extractTerminalErrorCode = (
  error: unknown,
): TerminalErrorCode | null => {
  const code = getResponseData(error)?.errorCode;
  return typeof code === "string" &&
    terminalErrorCodes.has(code as TerminalErrorCode)
    ? (code as TerminalErrorCode)
    : null;
};

export const extractTerminalError = (
  error: unknown,
  fallback = "Terminal request failed",
): TerminalRequestError => {
  const activeSessionCount = getResponseData(error)?.activeSessionCount;
  const confirmationToken = getResponseData(error)?.confirmationToken;
  return {
    activeSessionCount:
      typeof activeSessionCount === "number" &&
      Number.isSafeInteger(activeSessionCount) &&
      activeSessionCount >= 0
        ? activeSessionCount
        : null,
    confirmationToken:
      typeof confirmationToken === "string" && confirmationToken.trim()
        ? confirmationToken
        : null,
    errorCode: extractTerminalErrorCode(error),
    message: extractErrorMessage(error, fallback),
  };
};

export const extractTerminalErrorMessage = (
  error: unknown,
  fallback?: string,
) => extractTerminalError(error, fallback).message;
