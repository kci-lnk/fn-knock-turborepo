import type { SystemEventRecord } from "../../types";

export type SystemEventTranslate = (
  key: string,
  params?: Record<string, unknown>,
) => string;

const DRIFT_SOURCE_LABEL_KEYS: Record<string, string> = {
  "proxy-session": "admin.eventCenter.events.driftSource.proxySession",
  "fnos-token": "admin.eventCenter.events.driftSource.fnosToken",
  "session-refresh": "admin.eventCenter.events.driftSource.sessionRefresh",
  "browser-session": "admin.eventCenter.events.driftSource.browserSession",
};
const CHECK_REASON_LABEL_KEYS: Record<string, string> = {
  cron: "admin.eventCenter.events.checkReason.cron",
  manual: "admin.eventCenter.events.checkReason.manual",
  "manual-check-and-download":
    "admin.eventCenter.events.checkReason.manualCheckAndDownload",
  "download-bootstrap":
    "admin.eventCenter.events.checkReason.downloadBootstrap",
};
const AUTO_IP_GRANT_COMMENT_VALUES = new Set([
  "server.auth.autoIpGrantComment",
  "登录后自动授权",
  "登入後自動授權",
  "Automatically authorized after sign-in",
]);

export const createSystemEventValueFormatters = (
  translate: SystemEventTranslate,
) => {
  const translateValue = (
    prefix: string,
    value: unknown,
    keyMap: Record<string, string> = {},
  ) => {
    const key = String(value ?? "");
    if (!key) return "";
    const messageKey = keyMap[key] || `${prefix}.${key}`;
    const translated = translate(messageKey);
    return translated === messageKey ? key : translated;
  };
  const formatSubjectKindLabel = (
    kind: NonNullable<SystemEventRecord["subject"]>["kind"],
  ) => translateValue("admin.eventCenter.events.subjectKind", kind);
  const formatLogoutSourceLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.logoutSource", value);
  const formatAuthMethodLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.authMethod", value);
  const formatDriftSourceLabel = (value: unknown) =>
    translateValue(
      "admin.eventCenter.events.driftSource",
      value,
      DRIFT_SOURCE_LABEL_KEYS,
    );
  const formatGrantTypeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.grantType", value);
  const formatPostLoginGrantModeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.postLoginGrantMode", value);
  const formatUpdateScopeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.updateScope", value);
  const formatIpSourceLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.ipSource", value);
  const formatCheckReasonLabel = (value: unknown) =>
    translateValue(
      "admin.eventCenter.events.checkReason",
      value,
      CHECK_REASON_LABEL_KEYS,
    );
  const formatTunnelLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.tunnel", value);
  const formatTunnelStatusLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.tunnelStatus", value);
  const formatWafModeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.wafMode", value);
  const formatWafActionLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.wafAction", value);
  const shortId = (value: string, size = 10) =>
    value.length <= size
      ? value
      : `${value.slice(0, Math.max(4, size - 5))}...${value.slice(-4)}`;
  const formatSubject = (
    subject: SystemEventRecord["subject"] | undefined,
    shortenId = false,
  ) => {
    if (!subject) return "-";
    const kind = formatSubjectKindLabel(subject.kind) || subject.kind;
    const id = shortenId ? shortId(subject.id, 18) : subject.id;
    return `${kind} · ${id}`;
  };
  const shortenMiddle = (value: string, leading = 12, trailing = 10) =>
    value.length <= leading + trailing + 3
      ? value
      : `${value.slice(0, leading)}...${value.slice(-trailing)}`;
  const formatIpDisplay = (value: unknown) => {
    const ip = String(value ?? "").trim();
    if (!ip) return "-";
    if (ip.includes(":") && ip.length > 24) return shortenMiddle(ip, 14, 11);
    if (ip.length > 24) return shortenMiddle(ip, 12, 8);
    return ip;
  };
  const formatPercentage = (value: unknown) =>
    value === undefined || value === null || value === ""
      ? "-"
      : `${String(value)}%`;
  const formatBoolean = (value: unknown) =>
    value === undefined || value === null
      ? "-"
      : value
        ? translate("admin.eventCenter.events.yes")
        : translate("admin.eventCenter.events.no");
  const formatCredentialDisplay = (
    credentialName: unknown,
    linkedTotpName: unknown,
    authMethod: unknown,
  ) => {
    const credential =
      String(credentialName ?? "").trim() ||
      translate("admin.eventCenter.events.unknownCredential");
    const linkedTotp = String(linkedTotpName ?? "").trim();
    return String(authMethod ?? "") === "PASSKEY" && linkedTotp
      ? `Passkey「${credential}」 / TOTP「${linkedTotp}」`
      : credential;
  };
  const formatSessionCommentInline = (value: unknown) => {
    const rawComment = String(value ?? "").trim();
    const comment = AUTO_IP_GRANT_COMMENT_VALUES.has(rawComment)
      ? translate("auth.autoIpGrantComment")
      : rawComment;
    return comment
      ? translate("admin.eventCenter.events.sessionComment", { comment })
      : "";
  };
  const isWAFBlockingAction = (action: unknown, mode: unknown) => {
    const normalizedAction = String(action ?? "").toLowerCase();
    if (normalizedAction === "block" || normalizedAction === "deny") return true;
    if (["detect", "log", "pass"].includes(normalizedAction)) return false;
    return String(mode ?? "").toLowerCase() === "blocking";
  };
  const formatWAFOutcomeLabel = (action: unknown, mode: unknown) =>
    isWAFBlockingAction(action, mode)
      ? formatWafActionLabel("block")
      : formatWafActionLabel(action) || formatWafActionLabel("log");

  return {
    formatAuthMethodLabel,
    formatBoolean,
    formatCheckReasonLabel,
    formatCredentialDisplay,
    formatDriftSourceLabel,
    formatGrantTypeLabel,
    formatIpDisplay,
    formatIpSourceLabel,
    formatLogoutSourceLabel,
    formatPercentage,
    formatPostLoginGrantModeLabel,
    formatSessionCommentInline,
    formatSubject,
    formatTunnelLabel,
    formatTunnelStatusLabel,
    formatUpdateScopeLabel,
    formatWafActionLabel,
    formatWafModeLabel,
    formatWAFOutcomeLabel,
    shortId,
  };
};

export type SystemEventValueFormatters = ReturnType<
  typeof createSystemEventValueFormatters
>;
