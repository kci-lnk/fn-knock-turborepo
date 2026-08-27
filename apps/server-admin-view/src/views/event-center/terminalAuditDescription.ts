import type { SystemEventRecord } from "../../types";
import type { SystemEventTranslate } from "./systemEventValueFormatters";

const terminalAuditActions = new Set([
  "target_created",
  "target_updated",
  "target_deleted",
  "host_key_confirmed",
  "connection_test_succeeded",
  "connection_test_failed",
  "session_creation_started",
  "session_ended",
  "session_exited",
  "session_lost",
]);

export const describeTerminalAuditEvent = (
  event: SystemEventRecord,
  translate: SystemEventTranslate,
  shortId: (value: string, length?: number) => string,
) => {
  const payload = event.payload ?? {};
  const actionKey = String(payload.action || "unknown");
  const action = terminalAuditActions.has(actionKey)
    ? translate(`admin.eventCenter.events.terminalAuditActions.${actionKey}`)
    : actionKey;
  const sessionId = String(payload.session_id || "").trim();
  const targetId = String(payload.target_id || "").trim();
  const resource = sessionId
    ? translate("admin.eventCenter.events.terminalAuditSession", {
        session: shortId(sessionId, 14),
      })
    : targetId
      ? translate("admin.eventCenter.events.terminalAuditTarget", {
          target: shortId(targetId, 14),
        })
      : String(event.subject?.id || "-");
  const errorCode = String(payload.error_code || "").trim();
  return translate("admin.eventCenter.events.terminalAuditDescription", {
    action,
    resource,
    error: errorCode
      ? translate("admin.eventCenter.events.terminalAuditError", {
          error: errorCode,
        })
      : "",
  });
};
