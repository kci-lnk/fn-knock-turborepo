import type { DDNSIpSource, DDNSUpdateScope } from "../ddns/types";
import {
  FN_EVENT_AUTH_LOGIN_FAILURE,
  FN_EVENT_AUTH_LOGIN_SUCCESS,
  FN_EVENT_AUTH_LOGOUT,
  FN_EVENT_AUTH_SESSION_IP_DRIFT,
  FN_EVENT_DDNS_UPDATE_COMPLETED,
  FN_EVENT_LEVEL_ERROR,
  FN_EVENT_LEVEL_INFO,
  FN_EVENT_LEVEL_WARN,
  FN_EVENT_SECURITY_SCANNER_BLOCKED,
  FN_EVENT_WAF_BLOCKED,
  FN_EVENT_SSH_IP_BLOCKED,
  FN_EVENT_SSH_LOGIN_FAILURE,
  FN_EVENT_SSH_LOGIN_SUCCESS,
  FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE,
  FN_EVENT_SYSTEM_CPU_ALERT,
  FN_EVENT_SYSTEM_CPU_RECOVERED,
  FN_EVENT_SYSTEM_MEMORY_ALERT,
  FN_EVENT_SYSTEM_MEMORY_RECOVERED,
  SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
  SYSTEM_EVENT_SOURCE_SYSTEM_MONITOR,
  SYSTEM_EVENT_SUBJECT_KIND_APPLICATION,
  SYSTEM_EVENT_SUBJECT_KIND_DDNS,
  SYSTEM_EVENT_SUBJECT_KIND_IP,
  SYSTEM_EVENT_SUBJECT_KIND_RESOURCE,
  SYSTEM_EVENT_SUBJECT_KIND_SESSION,
  SYSTEM_EVENT_SUBJECT_KIND_TUNNEL,
  FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED,
  FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED,
  FN_EVENT_TUNNEL_FRP_CONNECTED,
  FN_EVENT_TUNNEL_FRP_DISCONNECTED,
} from "./constants";
import { systemEventManager } from "./manager";
import type {
  AuthMethod,
  SystemEventSessionDriftSource,
  TunnelType,
} from "./types";

const APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS = 30 * 24 * 60 * 60;

export const emitLoginSuccessEvent = async (payload: {
  sessionId: string;
  authMethod: AuthMethod;
  authProviderName?: string;
  credentialId: string;
  credentialName: string;
  linkedTotpName?: string;
  sessionComment?: string;
  grantType: "browser_session" | "login_ip_grant";
  postLoginIpGrantMode?: "follow_session" | "disabled" | "custom" | null;
  whitelistRecordId?: string | null;
  ip: string;
  ipLocation?: string;
  userAgent: string;
  rememberMe: boolean;
  expiresAt: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_AUTH_LOGIN_SUCCESS,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_SESSION,
      id: payload.sessionId,
    },
    payload: {
      session_id: payload.sessionId,
      auth_method: payload.authMethod,
      ...(payload.authProviderName
        ? { auth_provider_name: payload.authProviderName }
        : {}),
      credential_id: payload.credentialId,
      credential_name: payload.credentialName,
      ...(payload.linkedTotpName
        ? { linked_totp_name: payload.linkedTotpName }
        : {}),
      ...(payload.sessionComment
        ? { session_comment: payload.sessionComment }
        : {}),
      grant_type: payload.grantType,
      post_login_ip_grant_mode: payload.postLoginIpGrantMode,
      whitelist_record_id: payload.whitelistRecordId,
      ip: payload.ip,
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
      user_agent: payload.userAgent,
      remember_me: payload.rememberMe,
      expires_at: payload.expiresAt,
    },
  });

export const emitLogoutEvent = async (payload: {
  sessionId: string;
  authMethod: AuthMethod;
  credentialId: string;
  credentialName: string;
  linkedTotpName?: string;
  sessionComment?: string;
  ip: string;
  ipLocation?: string;
  userAgent: string;
  loginTime?: string;
  logoutSource: "user_logout" | "admin_session_delete";
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_AUTH_LOGOUT,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_INFO,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_SESSION,
      id: payload.sessionId,
    },
    payload: {
      session_id: payload.sessionId,
      auth_method: payload.authMethod,
      credential_id: payload.credentialId,
      credential_name: payload.credentialName,
      ...(payload.linkedTotpName
        ? { linked_totp_name: payload.linkedTotpName }
        : {}),
      ...(payload.sessionComment
        ? { session_comment: payload.sessionComment }
        : {}),
      ip: payload.ip,
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
      user_agent: payload.userAgent,
      ...(payload.loginTime ? { login_time: payload.loginTime } : {}),
      logout_source: payload.logoutSource,
    },
  });

export const emitLoginFailureEvent = async (payload: {
  ip: string;
  attempts: number;
  retryAfterSeconds: number;
  blockedUntil?: string;
  method?: AuthMethod;
  credentialName?: string;
  linkedTotpName?: string;
  userAgent?: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_AUTH_LOGIN_FAILURE,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    payload: {
      ip: payload.ip,
      attempts: payload.attempts,
      retry_after_seconds: payload.retryAfterSeconds,
      ...(payload.blockedUntil ? { blocked_until: payload.blockedUntil } : {}),
      ...(payload.method ? { method: payload.method } : {}),
      ...(payload.credentialName
        ? { credential_name: payload.credentialName }
        : {}),
      ...(payload.linkedTotpName
        ? { linked_totp_name: payload.linkedTotpName }
        : {}),
      ...(payload.userAgent ? { user_agent: payload.userAgent } : {}),
    },
  });

export const emitSessionIpDriftEvent = async (payload: {
  sessionId: string;
  authMethod: AuthMethod;
  credentialId: string;
  credentialName: string;
  linkedTotpName?: string;
  sessionComment?: string;
  driftSource: SystemEventSessionDriftSource;
  fromIp: string;
  fromIpLocation?: string;
  toIp: string;
  toIpLocation?: string;
  loginTime?: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_AUTH_SESSION_IP_DRIFT,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_SESSION,
      id: payload.sessionId,
    },
    payload: {
      session_id: payload.sessionId,
      auth_method: payload.authMethod,
      credential_id: payload.credentialId,
      credential_name: payload.credentialName,
      ...(payload.linkedTotpName
        ? { linked_totp_name: payload.linkedTotpName }
        : {}),
      ...(payload.sessionComment
        ? { session_comment: payload.sessionComment }
        : {}),
      drift_source: payload.driftSource,
      from_ip: payload.fromIp,
      ...(payload.fromIpLocation
        ? { from_ip_location: payload.fromIpLocation }
        : {}),
      to_ip: payload.toIp,
      ...(payload.toIpLocation ? { to_ip_location: payload.toIpLocation } : {}),
      ...(payload.loginTime ? { login_time: payload.loginTime } : {}),
    },
  });

export const emitScannerBlockedEvent = async (payload: {
  ip: string;
  blockedAt: number;
  windowMinutes: number;
  threshold: number;
  hitCount: number;
  hits: Array<{
    path: string;
    createdAt: number;
  }>;
  ipLocation?: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_SECURITY_SCANNER_BLOCKED,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    payload: {
      ip: payload.ip,
      blocked_at: new Date(payload.blockedAt).toISOString(),
      window_minutes: payload.windowMinutes,
      threshold: payload.threshold,
      hit_count: payload.hitCount,
      hits: payload.hits.map((hit) => ({
        path: hit.path,
        created_at: new Date(hit.createdAt).toISOString(),
      })),
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
    },
  });

export const emitDDNSUpdateCompletedEvent = async (payload: {
  trigger: "cron" | "enable" | "manual_test" | "startup";
  targetId: string;
  targetName: string;
  domainSummary?: string;
  isPrimary: boolean;
  provider: string;
  success: boolean;
  message: string;
  updateScope: DDNSUpdateScope;
  ipSource: DDNSIpSource;
  previousIpv4?: string | null;
  previousIpv6?: string | null;
  nextIpv4?: string | null;
  nextIpv6?: string | null;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_DDNS_UPDATE_COMPLETED,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: payload.success ? FN_EVENT_LEVEL_INFO : FN_EVENT_LEVEL_ERROR,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_DDNS,
      id: payload.targetId,
    },
    payload: {
      trigger: payload.trigger,
      target_id: payload.targetId,
      target_name: payload.targetName,
      ...(payload.domainSummary
        ? { domain_summary: payload.domainSummary }
        : {}),
      is_primary: payload.isPrimary,
      provider: payload.provider,
      success: payload.success,
      message: payload.message,
      update_scope: payload.updateScope,
      ip_source: payload.ipSource,
      previous_ipv4: payload.previousIpv4 ?? null,
      previous_ipv6: payload.previousIpv6 ?? null,
      next_ipv4: payload.nextIpv4 ?? null,
      next_ipv6: payload.nextIpv6 ?? null,
    },
  });

export const emitWAFBlockedEvent = async (payload: {
  ip: string;
  traceId: string;
  blockedAt: string;
  mode: string;
  action: string;
  status?: number;
  host?: string;
  path?: string;
  requestUri?: string;
  routeType?: string;
  routeKey?: string;
  bundleId?: string;
  ruleIds?: number[];
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_WAF_BLOCKED,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    dedupe_key: `waf:${payload.traceId}`,
    dedupe_ttl_seconds: 24 * 60 * 60,
    happened_at: payload.blockedAt,
    tags: ["waf", "gateway"],
    payload: {
      ip: payload.ip,
      trace_id: payload.traceId,
      blocked_at: payload.blockedAt,
      mode: payload.mode,
      action: payload.action,
      ...(payload.status ? { status: payload.status } : {}),
      ...(payload.host ? { host: payload.host } : {}),
      ...(payload.path ? { path: payload.path } : {}),
      ...(payload.requestUri ? { request_uri: payload.requestUri } : {}),
      ...(payload.routeType ? { route_type: payload.routeType } : {}),
      ...(payload.routeKey ? { route_key: payload.routeKey } : {}),
      ...(payload.bundleId ? { bundle_id: payload.bundleId } : {}),
      rule_ids: payload.ruleIds || [],
    },
  });

export const emitSSHLoginSuccessEvent = async (payload: {
  ip: string;
  ipLocation?: string;
  username: string;
  authMethod?: string;
  port?: number;
  logTime: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_SSH_LOGIN_SUCCESS,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_INFO,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    payload: {
      ip: payload.ip,
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
      username: payload.username,
      ...(payload.authMethod ? { auth_method: payload.authMethod } : {}),
      ...(payload.port ? { port: payload.port } : {}),
      log_time: payload.logTime,
    },
  });

export const emitSSHLoginFailureEvent = async (payload: {
  ip: string;
  ipLocation?: string;
  username: string;
  invalidUser: boolean;
  authMethod?: string;
  port?: number;
  attempts: number;
  windowMinutes: number;
  threshold: number;
  logTime: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_SSH_LOGIN_FAILURE,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    payload: {
      ip: payload.ip,
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
      username: payload.username,
      invalid_user: payload.invalidUser,
      ...(payload.authMethod ? { auth_method: payload.authMethod } : {}),
      ...(payload.port ? { port: payload.port } : {}),
      attempts: payload.attempts,
      window_minutes: payload.windowMinutes,
      threshold: payload.threshold,
      log_time: payload.logTime,
    },
  });

export const emitSSHIPBlockedEvent = async (payload: {
  ip: string;
  ipLocation?: string;
  blockedAt: string;
  blockedUntil: string;
  blockSeconds: number;
  reason: "failed_login_threshold" | "cidr_not_allowed";
  failedCount: number;
  windowMinutes: number;
  threshold: number;
  username?: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_SSH_IP_BLOCKED,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_WARN,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_IP,
      id: payload.ip,
    },
    payload: {
      ip: payload.ip,
      ...(payload.ipLocation ? { ip_location: payload.ipLocation } : {}),
      blocked_at: payload.blockedAt,
      blocked_until: payload.blockedUntil,
      block_seconds: payload.blockSeconds,
      reason: payload.reason,
      failed_count: payload.failedCount,
      window_minutes: payload.windowMinutes,
      threshold: payload.threshold,
      ...(payload.username ? { username: payload.username } : {}),
    },
  });

export const emitResourceAlertEvent = async (payload: {
  metric: "cpu" | "memory";
  hostname: string;
  usagePercent: number;
  thresholdPercent: number;
  recoverPercent: number;
  sampleIntervalSeconds: number;
  sustainSeconds: number;
  recovered?: boolean;
  dedupeKey?: string;
  dedupeTtlSeconds?: number;
}) =>
  systemEventManager.publishSafely({
    type:
      payload.metric === "cpu"
        ? payload.recovered
          ? FN_EVENT_SYSTEM_CPU_RECOVERED
          : FN_EVENT_SYSTEM_CPU_ALERT
        : payload.recovered
          ? FN_EVENT_SYSTEM_MEMORY_RECOVERED
          : FN_EVENT_SYSTEM_MEMORY_ALERT,
    source: SYSTEM_EVENT_SOURCE_SYSTEM_MONITOR,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_RESOURCE,
      id: `${payload.hostname}:${payload.metric}`,
    },
    ...(payload.dedupeKey ? { dedupe_key: payload.dedupeKey } : {}),
    ...(payload.dedupeTtlSeconds
      ? { dedupe_ttl_seconds: payload.dedupeTtlSeconds }
      : {}),
    payload: {
      hostname: payload.hostname,
      usage_percent: payload.usagePercent,
      threshold_percent: payload.thresholdPercent,
      recover_percent: payload.recoverPercent,
      sample_interval_seconds: payload.sampleIntervalSeconds,
      sustain_seconds: payload.sustainSeconds,
    },
  });

export const emitAppUpdateAvailableEvent = async (payload: {
  localVersion: string;
  latestVersion: string;
  forceUpdate: boolean;
  releaseNotes?: string;
  checkReason?: string;
}) =>
  systemEventManager.publishSafely({
    type: FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: FN_EVENT_LEVEL_INFO,
    dedupe_key: `system:app-update:${payload.latestVersion}`,
    dedupe_ttl_seconds: APP_UPDATE_EVENT_DEDUPE_TTL_SECONDS,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_APPLICATION,
      id: "fn-knock",
    },
    payload: {
      local_version: payload.localVersion,
      latest_version: payload.latestVersion,
      force_update: payload.forceUpdate,
      ...(payload.releaseNotes ? { release_notes: payload.releaseNotes } : {}),
      ...(payload.checkReason ? { check_reason: payload.checkReason } : {}),
    },
  });

export const emitTunnelConnectivityEvent = async (payload: {
  tunnel: TunnelType;
  connected: boolean;
  pid?: number | null;
  message?: string;
  instanceId?: string;
  instanceName?: string;
  isPrimary?: boolean;
}) =>
  systemEventManager.publishSafely({
    type:
      payload.tunnel === "frp"
        ? payload.connected
          ? FN_EVENT_TUNNEL_FRP_CONNECTED
          : FN_EVENT_TUNNEL_FRP_DISCONNECTED
        : payload.connected
          ? FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED
          : FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED,
    source: SYSTEM_EVENT_SOURCE_SERVER_ADMIN,
    level: payload.connected ? FN_EVENT_LEVEL_INFO : FN_EVENT_LEVEL_ERROR,
    subject: {
      kind: SYSTEM_EVENT_SUBJECT_KIND_TUNNEL,
      id:
        payload.tunnel === "frp" && payload.instanceId
          ? `frp:${payload.instanceId}`
          : payload.tunnel,
    },
    payload: {
      tunnel: payload.tunnel,
      status: payload.connected ? "connected" : "disconnected",
      ...(typeof payload.pid === "number" && Number.isFinite(payload.pid)
        ? { pid: payload.pid }
        : {}),
      ...(payload.instanceId ? { instance_id: payload.instanceId } : {}),
      ...(payload.instanceName ? { instance_name: payload.instanceName } : {}),
      ...(typeof payload.isPrimary === "boolean"
        ? { is_primary: payload.isPrimary }
        : {}),
      ...(payload.message ? { message: payload.message } : {}),
    },
  });
