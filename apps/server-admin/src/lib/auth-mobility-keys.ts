import { createHash } from "node:crypto";

export type MobilitySubjectType =
  | "proxy-session"
  | "fnos-token"
  | "trim-media-token";

const PREFIX = "fn_knock:auth_mobility";

const subjectHash = (
  subjectType: MobilitySubjectType,
  subjectKey: string,
): string =>
  createHash("sha256").update(`${subjectType}:${subjectKey}`).digest("hex");

export const authMobilityKeys = {
  activeIpDetails: (sessionId: string): string =>
    `${PREFIX}:active_ip_details:${sessionId}`,

  activeIpWhitelistOwner: (sessionId: string, ip: string): string =>
    `auth-mobility:active-ip:${sessionId}:${ip}`,

  activeIpZset: (sessionId: string): string =>
    `${PREFIX}:active_ips:${sessionId}`,

  binding: (subjectType: MobilitySubjectType, subjectKey: string): string =>
    `${PREFIX}:binding:${subjectType}:${subjectHash(subjectType, subjectKey)}`,

  isBindingForSubject: (
    storageKey: string,
    subjectType: MobilitySubjectType,
  ): boolean => storageKey.startsWith(`${PREFIX}:binding:${subjectType}:`),

  legacyWhitelistOwner: (sessionId: string): string =>
    `auth-mobility:legacy:${sessionId}`,

  sessionIndex: (sessionId: string): string => `${PREFIX}:session:${sessionId}`,

  subjectHash,

  summary: (sessionId: string): string => `${PREFIX}:summary:${sessionId}`,

  timeline: (sessionId: string): string => `${PREFIX}:timeline:${sessionId}`,

  whitelistOwner: (whitelistRecordId: string): string =>
    `${PREFIX}:whitelist:${whitelistRecordId}:session`,
};
