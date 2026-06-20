import type { SystemEventEnvelope } from "../../system-events/types";
import { normalizeAutoIpGrantComment } from "../../post-login-ip-grant";
import type { NotificationMessageFact } from "../types";
import {
  detailT,
  getActiveNotificationLocale,
  ntfT,
} from "./context";
import { formatAuthMethodLabel } from "./labels";

export const formatSeconds = (value: string) =>
  value ? detailT("units.seconds", { count: value }) : "";

export const formatMinutes = (value: string) =>
  value ? detailT("units.minutes", { count: value }) : "";

export const formatTimes = (value: string) =>
  value ? detailT("units.times", { count: value }) : "";

export const formatRatePerSecond = (value: string) =>
  value ? detailT("units.ratePerSecond", { count: value }) : "";

export const joinLocalizedList = (items: string[]) =>
  items.filter(Boolean).join(detailT("listSeparator"));

export const readPayloadValue = (
  event: SystemEventEnvelope,
  key: string,
) => {
  const payload = event.payload as Record<string, unknown>;
  const value = payload[key];
  if (value === undefined || value === null || value === "") return "";
  return String(value);
};

export const readSessionComment = (event: SystemEventEnvelope): string =>
  normalizeAutoIpGrantComment(
    readPayloadValue(event, "session_comment"),
    getActiveNotificationLocale(),
  );

export const joinCompactParts = (...parts: Array<string | undefined>) =>
  parts
    .map((part) => String(part || "").trim())
    .filter(Boolean)
    .join(" | ");

export const formatCredentialContext = (
  event: SystemEventEnvelope,
  fallback = "",
) => {
  const credentialName = readPayloadValue(event, "credential_name");
  const linkedTotpName = readPayloadValue(event, "linked_totp_name");
  const authMethod = formatAuthMethodLabel(
    readPayloadValue(event, "auth_method"),
  );

  if (linkedTotpName) {
    return ntfT("credentialLinkedTotp", {
      authMethod: authMethod || ntfT("credential"),
      credential: credentialName || ntfT("unknownCredential"),
      totp: linkedTotpName,
    });
  }
  if (credentialName) {
    return ntfT("credentialName", { credential: credentialName });
  }
  return fallback;
};

export const formatSessionCommentCompact = (value: string) =>
  value
    ? ntfT("sessionCommentCompact", {
        comment: normalizeAutoIpGrantComment(
          value,
          getActiveNotificationLocale(),
        ),
      })
    : "";

export const appendSessionComment = (text: string, sessionComment: string) =>
  sessionComment
    ? ntfT("appendSessionComment", {
        text,
        comment: normalizeAutoIpGrantComment(
          sessionComment,
          getActiveNotificationLocale(),
        ),
      })
    : text;

export const formatDateTime = (value: string) => {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) {
    return String(value || "").trim();
  }

  return new Intl.DateTimeFormat(getActiveNotificationLocale(), {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
    .format(date)
    .replaceAll("/", "-");
};

export const formatBoolean = (value: string) => {
  if (value === "true") return ntfT("yes");
  if (value === "false") return ntfT("no");
  return value;
};

export const formatIpTransition = (previousIp: string, nextIp: string) => {
  if (previousIp && nextIp) return `${previousIp} -> ${nextIp}`;
  return previousIp || nextIp;
};

export const truncateText = (value: string, maxLength = 180) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength).trim()}...`;
};

export const pushFact = (
  facts: NotificationMessageFact[],
  label: string,
  value: string | undefined,
) => {
  const normalized = String(value || "").trim();
  if (!normalized) return;
  facts.push({ label, value: normalized });
};

export const buildBodyText = (args: {
  overview: string;
  aggregation?: string;
  advice?: string;
}) =>
  [
    args.overview ? `${args.overview}` : "",
    args.aggregation ? `${args.aggregation}` : "",
    args.advice ? `${args.advice}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

export const buildBodyMarkdown = (args: {
  overview: string;
  aggregation?: string;
  advice?: string;
}) =>
  [
    args.overview ? `**${ntfT("sections.overview")}**\n${args.overview}` : "",
    args.aggregation
      ? `**${ntfT("sections.aggregation")}**\n${args.aggregation}`
      : "",
    args.advice ? `**${ntfT("sections.advice")}**\n${args.advice}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

export const buildAggregationText = (
  matchedCount: number,
  windowSeconds: number,
) =>
  matchedCount > 1
    ? ntfT("aggregationText", { count: matchedCount, seconds: windowSeconds })
    : "";

export const getScannerPaths = (event: SystemEventEnvelope) => {
  const hits = (event.payload as Record<string, unknown>).hits;
  if (!Array.isArray(hits)) return [];

  return hits
    .map((item) => {
      if (!item || typeof item !== "object") return "";
      return String((item as { path?: string }).path || "").trim();
    })
    .filter(Boolean);
};
