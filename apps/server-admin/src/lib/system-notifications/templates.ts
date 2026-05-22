import type { SystemEventEnvelope } from "../system-events/types";
import { normalizeNotificationMessage } from "./brand";
import type {
  NotificationMessage,
  NotificationMessageFact,
  NotificationRule,
  NotificationSeverity,
} from "./types";

const EVENT_LABELS: Record<SystemEventEnvelope["type"], string> = {
  FN_EVENT_AUTH_LOGIN_SUCCESS: "登录成功",
  FN_EVENT_AUTH_LOGOUT: "退出登录",
  FN_EVENT_AUTH_LOGIN_FAILURE: "登录失败",
  FN_EVENT_AUTH_SESSION_IP_DRIFT: "会话 IP 漂移",
  FN_EVENT_SECURITY_SCANNER_BLOCKED: "扫描器拦截",
  FN_EVENT_DDNS_UPDATE_COMPLETED: "DDNS 更新",
  FN_EVENT_GATEWAY_THROTTLE_BLOCKED: "网关节流封锁",
  FN_EVENT_WAF_BLOCKED: "WAF 阻断",
  FN_EVENT_SSH_LOGIN_SUCCESS: "SSH 登录成功",
  FN_EVENT_SSH_LOGIN_FAILURE: "SSH 登录失败",
  FN_EVENT_SSH_IP_BLOCKED: "SSH IP 封锁",
  FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE: "应用更新提示",
  FN_EVENT_SYSTEM_CPU_ALERT: "CPU 告警",
  FN_EVENT_SYSTEM_CPU_RECOVERED: "CPU 恢复",
  FN_EVENT_SYSTEM_MEMORY_ALERT: "内存告警",
  FN_EVENT_SYSTEM_MEMORY_RECOVERED: "内存恢复",
  FN_EVENT_TUNNEL_FRP_CONNECTED: "FRP 已连上",
  FN_EVENT_TUNNEL_FRP_DISCONNECTED: "FRP 已断开",
  FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED: "Cloudflared 已连上",
  FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED: "Cloudflared 已断开",
};

const APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH = 360;

export const formatNotificationEventLabel = (
  type: SystemEventEnvelope["type"],
) => EVENT_LABELS[type] || type;

export const buildNotificationRuleName = (type: SystemEventEnvelope["type"]) =>
  `${formatNotificationEventLabel(type)} 通知`;

const EVENT_LEVEL_LABELS: Record<SystemEventEnvelope["level"], string> = {
  INFO: "信息",
  WARN: "注意",
  ERROR: "错误",
  CRITICAL: "严重",
};

const EVENT_SOURCE_LABELS: Record<SystemEventEnvelope["source"], string> = {
  SERVER_ADMIN: "管理后台",
  GO_REAUTH_PROXY: "认证代理",
  SYSTEM_MONITOR: "系统监控",
};

const AUTH_METHOD_LABELS = {
  TOTP: "TOTP",
  PASSKEY: "Passkey",
  OIDC: "外部账号",
} as const;

const GRANT_TYPE_LABELS = {
  browser_session: "浏览器会话",
  login_ip_grant: "登录 IP 授权",
} as const;

const WAF_MODE_LABELS = {
  detection: "检测",
  blocking: "阻断",
  off: "关闭",
} as const;

const WAF_ACTION_LABELS = {
  block: "阻断",
  deny: "拒绝",
  detect: "检测",
  log: "记录",
  pass: "放行",
} as const;

const LOGOUT_SOURCE_LABELS = {
  user_logout: "用户主动退出",
  admin_session_delete: "管理员下线",
} as const;

const DRIFT_SOURCE_LABELS = {
  "proxy-session": "代理会话",
  "fnos-token": "飞牛令牌",
  "session-refresh": "会话刷新",
  "browser-session": "浏览器会话",
} as const;

const DDNS_TRIGGER_LABELS = {
  cron: "定时任务",
  enable: "启用后首次执行",
  manual_test: "手动测试",
} as const;

const DDNS_UPDATE_SCOPE_LABELS = {
  dual_stack: "IPv4 + IPv6",
  ipv4_only: "仅 IPv4",
  ipv6_only: "仅 IPv6",
} as const;

const DDNS_IP_SOURCE_LABELS = {
  public: "公网探测",
  interface: "网卡读取",
} as const;

const UPDATE_CHECK_REASON_LABELS = {
  cron: "定时检查",
  manual: "手动检查",
  "manual-check-and-download": "手动检查并下载",
  "download-bootstrap": "下载前检查",
} as const;

const TUNNEL_LABELS = {
  frp: "FRP",
  cloudflared: "Cloudflared",
} as const;

const readPayloadValue = (event: SystemEventEnvelope, key: string) => {
  const payload = event.payload as Record<string, unknown>;
  const value = payload[key];
  if (value === undefined || value === null || value === "") return "";
  return String(value);
};

const joinCompactParts = (...parts: Array<string | undefined>) =>
  parts
    .map((part) => String(part || "").trim())
    .filter(Boolean)
    .join(" | ");

const formatCredentialContext = (event: SystemEventEnvelope, fallback = "") => {
  const credentialName = readPayloadValue(event, "credential_name");
  const linkedTotpName = readPayloadValue(event, "linked_totp_name");
  const authMethod =
    AUTH_METHOD_LABELS[
      readPayloadValue(event, "auth_method") as keyof typeof AUTH_METHOD_LABELS
    ] || readPayloadValue(event, "auth_method");

  if (linkedTotpName) {
    return `${authMethod || "凭证"}「${credentialName || "未知凭证"}」关联 TOTP「${linkedTotpName}」`;
  }
  if (credentialName) {
    return `凭证「${credentialName}」`;
  }
  return fallback;
};

const formatSessionCommentCompact = (value: string) =>
  value ? `备注：${value}` : "";

const appendSessionComment = (text: string, sessionComment: string) =>
  sessionComment ? `${text}（备注：${sessionComment}）` : text;

const formatEventLevelLabel = (level: SystemEventEnvelope["level"]) =>
  EVENT_LEVEL_LABELS[level] || level;

const formatEventSourceLabel = (source: SystemEventEnvelope["source"]) =>
  EVENT_SOURCE_LABELS[source] || source;

const formatDateTime = (value: string) => {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) {
    return String(value || "").trim();
  }

  return new Intl.DateTimeFormat("zh-CN", {
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

const formatBoolean = (value: string) => {
  if (value === "true") return "是";
  if (value === "false") return "否";
  return value;
};

const formatIpTransition = (previousIp: string, nextIp: string) => {
  if (previousIp && nextIp) return `${previousIp} -> ${nextIp}`;
  return previousIp || nextIp;
};

const formatWAFActionLabel = (value: string) =>
  WAF_ACTION_LABELS[value as keyof typeof WAF_ACTION_LABELS] || value;

const formatWAFModeLabel = (value: string) =>
  WAF_MODE_LABELS[value as keyof typeof WAF_MODE_LABELS] || value;

const isWAFBlockingAction = (action: string, mode: string) => {
  const normalizedAction = action.toLowerCase();
  if (normalizedAction === "block" || normalizedAction === "deny") return true;
  if (
    normalizedAction === "detect" ||
    normalizedAction === "log" ||
    normalizedAction === "pass"
  ) {
    return false;
  }
  return mode.toLowerCase() === "blocking";
};

const formatWAFOutcomeLabel = (action: string, mode: string) => {
  if (isWAFBlockingAction(action, mode)) return "阻断";
  const actionLabel = formatWAFActionLabel(action);
  return actionLabel || "记录";
};

const truncateText = (value: string, maxLength = 180) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength).trim()}...`;
};

const pushFact = (
  facts: NotificationMessageFact[],
  label: string,
  value: string | undefined,
) => {
  const normalized = String(value || "").trim();
  if (!normalized) return;
  facts.push({ label, value: normalized });
};

const buildBodyText = (args: {
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

const buildBodyMarkdown = (args: {
  overview: string;
  aggregation?: string;
  advice?: string;
}) =>
  [
    args.overview ? `**事件概述**\n${args.overview}` : "",
    args.aggregation ? `**聚合情况**\n${args.aggregation}` : "",
    args.advice ? `**处理建议**\n${args.advice}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

const buildAggregationText = (matchedCount: number, windowSeconds: number) =>
  matchedCount > 1
    ? `本次通知已在 ${windowSeconds} 秒窗口内聚合 ${matchedCount} 条相似事件。`
    : "";

const getScannerPaths = (event: SystemEventEnvelope) => {
  const hits = (event.payload as Record<string, unknown>).hits;
  if (!Array.isArray(hits)) return [];

  return hits
    .map((item) => {
      if (!item || typeof item !== "object") return "";
      return String((item as { path?: string }).path || "").trim();
    })
    .filter(Boolean);
};

const buildNotificationDetails = (args: {
  event: SystemEventEnvelope;
  rule: NotificationRule;
  matchedCount: number;
}) => {
  const { event, rule, matchedCount } = args;
  const facts: NotificationMessageFact[] = [];
  const aggregation = buildAggregationText(matchedCount, rule.window_seconds);

  let summary =
    formatEventSummary(event) || formatNotificationEventLabel(event.type);
  let overview = summary;
  let advice = "";

  switch (event.type) {
    case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
      const credentialName =
        readPayloadValue(event, "credential_name") || "未知凭证";
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readPayloadValue(event, "session_comment");
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const ipLocation = readPayloadValue(event, "ip_location");
      const authMethodRaw = readPayloadValue(event, "auth_method");
      const authProviderName = readPayloadValue(event, "auth_provider_name");
      const authMethod =
        AUTH_METHOD_LABELS[
          authMethodRaw as keyof typeof AUTH_METHOD_LABELS
        ] || authMethodRaw;
      const isOidcLogin = authMethodRaw === "OIDC";
      const loginMethodText =
        isOidcLogin && authProviderName
          ? `通过 ${authProviderName} 登录`
          : `使用 ${authMethod || "未知方式"}`;
      const loginAuthText =
        isOidcLogin && authProviderName
          ? `通过 ${authProviderName}`
          : `使用 ${authMethod || "未知方式"}`;
      const grantType =
        GRANT_TYPE_LABELS[
          readPayloadValue(
            event,
            "grant_type",
          ) as keyof typeof GRANT_TYPE_LABELS
        ] || readPayloadValue(event, "grant_type");
      const rememberMe = formatBoolean(readPayloadValue(event, "remember_me"));
      const expiresAt = formatDateTime(readPayloadValue(event, "expires_at"));

      summary = appendSessionComment(
        isOidcLogin
          ? `${credentialName} ${loginMethodText}成功，来源 IP ${ip}${linkedTotpName ? `，关联 TOTP「${linkedTotpName}」` : ""}`
          : linkedTotpName
          ? `${authMethod || "凭证"}「${credentialName}」关联 TOTP「${linkedTotpName}」从 ${ip} 登录成功`
          : `凭证「${credentialName}」从 ${ip} 登录成功`,
        sessionComment,
      );
      overview = `本次登录${loginAuthText}完成认证，授权方式为 ${grantType || "未知"}${ipLocation ? `，登录位置为 ${ipLocation}` : ""}。${sessionComment ? `当前会话备注为「${sessionComment}」。` : ""}`;
      advice = "如该登录并非本人操作，建议尽快撤销会话并检查访问策略。";

      pushFact(facts, "凭证名称", credentialName);
      pushFact(facts, "关联 TOTP", linkedTotpName);
      pushFact(facts, "会话备注", sessionComment);
      pushFact(facts, "登录 IP", ip);
      pushFact(facts, "IP 位置", ipLocation);
      pushFact(facts, "认证方式", authMethod);
      pushFact(facts, "登录提供商", authProviderName);
      pushFact(facts, "授权方式", grantType);
      pushFact(facts, "记住登录", rememberMe);
      pushFact(facts, "会话到期", expiresAt);
      pushFact(facts, "会话 ID", readPayloadValue(event, "session_id"));
      break;
    }
    case "FN_EVENT_AUTH_LOGOUT": {
      const credentialName =
        readPayloadValue(event, "credential_name") || "未知凭证";
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readPayloadValue(event, "session_comment");
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const ipLocation = readPayloadValue(event, "ip_location");
      const authMethod =
        AUTH_METHOD_LABELS[
          readPayloadValue(
            event,
            "auth_method",
          ) as keyof typeof AUTH_METHOD_LABELS
        ] || readPayloadValue(event, "auth_method");
      const logoutSource =
        LOGOUT_SOURCE_LABELS[
          readPayloadValue(
            event,
            "logout_source",
          ) as keyof typeof LOGOUT_SOURCE_LABELS
        ] || readPayloadValue(event, "logout_source");

      summary = appendSessionComment(
        linkedTotpName
          ? `${authMethod || "凭证"}「${credentialName}」关联 TOTP「${linkedTotpName}」已退出登录`
          : `凭证「${credentialName}」已退出登录`,
        sessionComment,
      );
      overview = `该会话已从 ${ip}${ipLocation ? `（${ipLocation}）` : ""} 退出，退出方式为 ${logoutSource || "未知"}。${sessionComment ? `当前会话备注为「${sessionComment}」。` : ""}`;
      advice = "如该退出不符合预期，请核查是否存在管理员下线或异常会话清理。";

      pushFact(facts, "凭证名称", credentialName);
      pushFact(facts, "关联 TOTP", linkedTotpName);
      pushFact(facts, "会话备注", sessionComment);
      pushFact(facts, "登录 IP", ip);
      pushFact(facts, "IP 位置", ipLocation);
      pushFact(facts, "退出方式", logoutSource);
      pushFact(
        facts,
        "登录时间",
        formatDateTime(readPayloadValue(event, "login_time")),
      );
      pushFact(facts, "会话 ID", readPayloadValue(event, "session_id"));
      break;
    }
    case "FN_EVENT_AUTH_LOGIN_FAILURE": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const attempts = readPayloadValue(event, "attempts") || "0";
      const retryAfter = readPayloadValue(event, "retry_after_seconds");
      const blockedUntil = formatDateTime(
        readPayloadValue(event, "blocked_until"),
      );
      const method =
        AUTH_METHOD_LABELS[
          readPayloadValue(event, "method") as keyof typeof AUTH_METHOD_LABELS
        ] || readPayloadValue(event, "method");
      const credentialName = readPayloadValue(event, "credential_name");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");

      summary = `来自 ${ip} 的登录失败已累计 ${attempts} 次`;
      overview = `检测到登录认证连续失败，当前来源 IP 为 ${ip}${retryAfter ? `，需等待 ${retryAfter} 秒后再尝试` : ""}${blockedUntil ? `，限制将持续到 ${blockedUntil}` : ""}。`;
      advice =
        "如非本人操作，建议立即检查凭证安全，并考虑封禁来源 IP 或提高登录防护等级。";

      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "失败次数", `${attempts} 次`);
      pushFact(facts, "认证方式", method);
      pushFact(facts, "凭证名称", credentialName);
      pushFact(facts, "关联 TOTP", linkedTotpName);
      pushFact(facts, "重试等待", retryAfter ? `${retryAfter} 秒` : "");
      pushFact(facts, "限制截止", blockedUntil);
      break;
    }
    case "FN_EVENT_AUTH_SESSION_IP_DRIFT": {
      const credentialName = readPayloadValue(event, "credential_name");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readPayloadValue(event, "session_comment");
      const authMethod =
        AUTH_METHOD_LABELS[
          readPayloadValue(
            event,
            "auth_method",
          ) as keyof typeof AUTH_METHOD_LABELS
        ] || readPayloadValue(event, "auth_method");
      const fromIp = readPayloadValue(event, "from_ip") || "未知 IP";
      const toIp = readPayloadValue(event, "to_ip") || "未知 IP";
      const source =
        DRIFT_SOURCE_LABELS[
          readPayloadValue(
            event,
            "drift_source",
          ) as keyof typeof DRIFT_SOURCE_LABELS
        ] || readPayloadValue(event, "drift_source");
      const sessionLabel = formatCredentialContext(event, "当前会话");

      summary = appendSessionComment(
        `${sessionLabel} IP 从 ${fromIp} 切换到 ${toIp}`,
        sessionComment,
      );
      overview = `检测到${sessionLabel}的访问来源 IP 发生变化，来源判定为 ${source || "未知"}。${sessionComment ? `当前会话备注为「${sessionComment}」。` : ""}这通常与网络切换、代理变化或会话异常有关。`;
      advice =
        "若这次 IP 变化并不符合预期，请尽快核查当前会话是否存在被接管风险。";

      pushFact(facts, "凭证名称", credentialName);
      pushFact(facts, "关联 TOTP", linkedTotpName);
      pushFact(facts, "会话备注", sessionComment);
      pushFact(facts, "认证方式", authMethod);
      pushFact(facts, "原始 IP", fromIp);
      pushFact(facts, "原始位置", readPayloadValue(event, "from_ip_location"));
      pushFact(facts, "当前 IP", toIp);
      pushFact(facts, "当前位置", readPayloadValue(event, "to_ip_location"));
      pushFact(facts, "变化来源", source);
      pushFact(
        facts,
        "登录时间",
        formatDateTime(readPayloadValue(event, "login_time")),
      );
      pushFact(facts, "会话 ID", readPayloadValue(event, "session_id"));
      break;
    }
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const windowMinutes = readPayloadValue(event, "window_minutes") || "0";
      const hitCount = readPayloadValue(event, "hit_count") || "0";
      const threshold = readPayloadValue(event, "threshold") || "0";
      const scannerPaths = getScannerPaths(event).slice(0, 3);

      summary = `${ip} 因扫描行为已被拦截`;
      overview = `该来源在 ${windowMinutes} 分钟内累计触发 ${hitCount} 次扫描行为，已超过阈值 ${threshold} 次${scannerPaths.length > 0 ? `；最近命中的路径包括 ${scannerPaths.join("、")}` : ""}。`;
      advice =
        "建议结合网关日志确认是否为恶意探测；如确认为误报，可进一步调整扫描阈值。";

      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "IP 位置", readPayloadValue(event, "ip_location"));
      pushFact(facts, "命中次数", `${hitCount} 次`);
      pushFact(facts, "观察窗口", `${windowMinutes} 分钟`);
      pushFact(facts, "触发阈值", `${threshold} 次`);
      pushFact(
        facts,
        "拦截时间",
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      pushFact(facts, "最近路径", scannerPaths.join("、"));
      break;
    }
    case "FN_EVENT_DDNS_UPDATE_COMPLETED": {
      const targetName =
        readPayloadValue(event, "target_name") ||
        readPayloadValue(event, "domain_summary") ||
        "DDNS 条目";
      const provider = readPayloadValue(event, "provider") || "未知提供商";
      const success = readPayloadValue(event, "success") === "true";
      const resultMessage = readPayloadValue(event, "message");
      const trigger =
        DDNS_TRIGGER_LABELS[
          readPayloadValue(event, "trigger") as keyof typeof DDNS_TRIGGER_LABELS
        ] || readPayloadValue(event, "trigger");
      const updateScope =
        DDNS_UPDATE_SCOPE_LABELS[
          readPayloadValue(
            event,
            "update_scope",
          ) as keyof typeof DDNS_UPDATE_SCOPE_LABELS
        ] || readPayloadValue(event, "update_scope");
      const ipSource =
        DDNS_IP_SOURCE_LABELS[
          readPayloadValue(
            event,
            "ip_source",
          ) as keyof typeof DDNS_IP_SOURCE_LABELS
        ] || readPayloadValue(event, "ip_source");
      const ipv4Change = formatIpTransition(
        readPayloadValue(event, "previous_ipv4"),
        readPayloadValue(event, "next_ipv4"),
      );
      const ipv6Change = formatIpTransition(
        readPayloadValue(event, "previous_ipv6"),
        readPayloadValue(event, "next_ipv6"),
      );

      summary = `${targetName} DDNS ${success ? "更新成功" : "更新失败"}`;
      overview = `${trigger || "本次任务"}已执行 DDNS 更新，范围为 ${updateScope || "未知"}，IP 来源为 ${ipSource || "未知"}。${resultMessage ? `结果说明：${resultMessage}` : ""}`;
      advice = success
        ? "如解析尚未生效，可继续等待 DNS 缓存刷新后再验证外部访问。"
        : "建议检查提供商凭证、解析记录配置，以及公网 IP 获取状态是否正常。";

      pushFact(facts, "条目", targetName);
      pushFact(facts, "提供商", provider);
      pushFact(
        facts,
        "条目类型",
        readPayloadValue(event, "is_primary") === "true" ? "主域" : "附加域",
      );
      pushFact(facts, "执行方式", trigger);
      pushFact(facts, "更新范围", updateScope);
      pushFact(facts, "IP 来源", ipSource);
      pushFact(facts, "IPv4 变化", ipv4Change);
      pushFact(facts, "IPv6 变化", ipv6Change);
      pushFact(facts, "执行结果", resultMessage);
      break;
    }
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const blockSeconds = readPayloadValue(event, "block_seconds") || "0";
      const requestsPerSecond =
        readPayloadValue(event, "requests_per_second") || "0";
      const burst = readPayloadValue(event, "burst") || "0";
      const host = readPayloadValue(event, "host");
      const path = readPayloadValue(event, "path");

      summary = `${ip} 因请求过快被封锁 ${blockSeconds} 秒`;
      overview = `该来源触发了网关节流保护，限流阈值为 ${requestsPerSecond} 次/秒，突发容量为 ${burst}${host || path ? `，目标请求为 ${joinCompactParts(host, path)}` : ""}。`;
      advice =
        "请结合访问日志确认是否为突发流量、误伤或恶意请求，并按需调整限流策略。";

      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "封锁时长", `${blockSeconds} 秒`);
      pushFact(
        facts,
        "封锁截止",
        formatDateTime(readPayloadValue(event, "blocked_until")),
      );
      pushFact(facts, "限流阈值", `${requestsPerSecond} 次/秒`);
      pushFact(facts, "突发容量", burst);
      pushFact(facts, "目标主机", host);
      pushFact(facts, "请求路径", path);
      pushFact(facts, "路由类型", readPayloadValue(event, "route_type"));
      pushFact(
        facts,
        "认证路由",
        formatBoolean(readPayloadValue(event, "is_auth_route")),
      );
      break;
    }
    case "FN_EVENT_WAF_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const host = readPayloadValue(event, "host");
      const path =
        readPayloadValue(event, "request_uri") ||
        readPayloadValue(event, "path");
      const ruleIds = readPayloadValue(event, "rule_ids");
      const traceId = readPayloadValue(event, "trace_id");
      const action = readPayloadValue(event, "action");
      const mode = readPayloadValue(event, "mode");
      const actionLabel = formatWAFActionLabel(action);
      const modeLabel = formatWAFModeLabel(mode);
      const outcomeLabel = formatWAFOutcomeLabel(action, mode);

      summary = `${ip} 的请求被 WAF ${outcomeLabel}`;
      overview = `WAF 已${outcomeLabel}来源 ${ip}${host ? ` 访问 ${host}` : ""}${path ? ` ${path}` : ""}${actionLabel ? `，动作为${actionLabel}` : ""}${modeLabel ? `，当前模式为${modeLabel}` : ""}。${ruleIds ? `命中规则：${ruleIds}。` : ""}`;
      advice =
        outcomeLabel === "阻断"
          ? "请在 WAF 日志中按 Trace ID 查看命中详情；如确认为误报，请及时向项目方反馈BUG。"
          : "请在 WAF 日志中按 Trace ID 查看命中详情，并结合规则与请求上下文判断是否需要调整策略。";

      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "Trace ID", traceId);
      pushFact(facts, "Host", host);
      pushFact(facts, "请求地址", path);
      pushFact(facts, "处理结果", outcomeLabel);
      pushFact(facts, "WAF 动作", actionLabel);
      pushFact(facts, "WAF 模式", modeLabel);
      pushFact(facts, "规则 ID", ruleIds);
      pushFact(facts, "规则包", readPayloadValue(event, "bundle_id"));
      pushFact(facts, "状态码", readPayloadValue(event, "status"));
      pushFact(
        facts,
        "拦截时间",
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      break;
    }
    case "FN_EVENT_SSH_LOGIN_SUCCESS": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const ipLocation = readPayloadValue(event, "ip_location");
      const username = readPayloadValue(event, "username") || "未知用户";
      const authMethod = readPayloadValue(event, "auth_method");

      summary = `SSH 用户「${username}」从 ${ip} 登录成功`;
      overview = `检测到一次 SSH 登录成功，来源为 ${ip}${ipLocation ? `（${ipLocation}）` : ""}${authMethod ? `，认证方式为 ${authMethod}` : ""}。`;
      advice = "如该登录并非预期，请检查 SSH 账号、密钥和来源访问策略。";

      pushFact(facts, "用户", username);
      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "IP 位置", ipLocation);
      pushFact(facts, "认证方式", authMethod);
      pushFact(facts, "端口", readPayloadValue(event, "port"));
      pushFact(
        facts,
        "日志时间",
        formatDateTime(readPayloadValue(event, "log_time")),
      );
      break;
    }
    case "FN_EVENT_SSH_LOGIN_FAILURE": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const ipLocation = readPayloadValue(event, "ip_location");
      const username = readPayloadValue(event, "username") || "未知用户";
      const attempts = readPayloadValue(event, "attempts") || "0";
      const threshold = readPayloadValue(event, "threshold") || "0";
      const windowMinutes = readPayloadValue(event, "window_minutes") || "0";

      summary = `SSH 用户「${username}」从 ${ip} 登录失败`;
      overview = `该来源在 ${windowMinutes} 分钟窗口内累计 ${attempts}/${threshold} 次 SSH 登录失败${ipLocation ? `，位置为 ${ipLocation}` : ""}。`;
      advice =
        "请关注失败次数是否接近封锁阈值，必要时收紧 SSH 暴露范围或调整凭据。";

      pushFact(facts, "用户", username);
      pushFact(
        facts,
        "无效用户",
        formatBoolean(readPayloadValue(event, "invalid_user")),
      );
      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "IP 位置", ipLocation);
      pushFact(facts, "认证方式", readPayloadValue(event, "auth_method"));
      pushFact(facts, "端口", readPayloadValue(event, "port"));
      pushFact(facts, "失败次数", attempts);
      pushFact(facts, "阈值", threshold);
      pushFact(facts, "窗口", `${windowMinutes} 分钟`);
      break;
    }
    case "FN_EVENT_SSH_IP_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || "未知 IP";
      const ipLocation = readPayloadValue(event, "ip_location");
      const reason = readPayloadValue(event, "reason");
      const reasonLabel =
        reason === "cidr_not_allowed" ? "不在允许地区范围" : "失败次数达到阈值";

      summary = `${ip} 已被 SSH 安全封锁`;
      overview = `SSH 安全已封锁来源 ${ip}${ipLocation ? `（${ipLocation}）` : ""}，原因是${reasonLabel}。`;
      advice =
        "请确认该来源是否可信；如为误封，可在 SSH 安全的封锁列表中解除。";

      pushFact(facts, "来源 IP", ip);
      pushFact(facts, "IP 位置", ipLocation);
      pushFact(facts, "封锁原因", reasonLabel);
      pushFact(facts, "关联用户", readPayloadValue(event, "username"));
      pushFact(facts, "失败次数", readPayloadValue(event, "failed_count"));
      pushFact(
        facts,
        "窗口",
        `${readPayloadValue(event, "window_minutes")} 分钟`,
      );
      pushFact(facts, "阈值", readPayloadValue(event, "threshold"));
      pushFact(
        facts,
        "封锁时间",
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      pushFact(
        facts,
        "封锁截止",
        formatDateTime(readPayloadValue(event, "blocked_until")),
      );
      break;
    }
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE": {
      const localVersion =
        readPayloadValue(event, "local_version") || "当前版本未知";
      const latestVersion =
        readPayloadValue(event, "latest_version") || "目标版本未知";
      const forceUpdate = readPayloadValue(event, "force_update") === "true";
      const checkReason =
        UPDATE_CHECK_REASON_LABELS[
          readPayloadValue(
            event,
            "check_reason",
          ) as keyof typeof UPDATE_CHECK_REASON_LABELS
        ] || readPayloadValue(event, "check_reason");
      const releaseNotes = truncateText(
        readPayloadValue(event, "release_notes"),
        APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH,
      );

      summary = `发现新版本 ${latestVersion}`;
      overview = `${checkReason || "本次检查"}发现 fn-knock 可从 ${localVersion} 升级到 ${latestVersion}${forceUpdate ? "，建议尽快安排更新" : ""}。`;
      advice = releaseNotes
        ? `更新说明：${releaseNotes}`
        : "建议在合适的维护窗口完成更新，并在安装前确认当前配置与服务状态。";

      pushFact(facts, "当前版本", localVersion);
      pushFact(facts, "最新版本", latestVersion);
      pushFact(facts, "检查方式", checkReason);
      pushFact(facts, "强制更新", forceUpdate ? "是" : "否");
      pushFact(facts, "更新说明", releaseNotes);
      break;
    }
    case "FN_EVENT_SYSTEM_CPU_ALERT":
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED": {
      const isCpuEvent =
        event.type === "FN_EVENT_SYSTEM_CPU_ALERT" ||
        event.type === "FN_EVENT_SYSTEM_CPU_RECOVERED";
      const recovered =
        event.type === "FN_EVENT_SYSTEM_CPU_RECOVERED" ||
        event.type === "FN_EVENT_SYSTEM_MEMORY_RECOVERED";
      const metricLabel = isCpuEvent ? "CPU" : "内存";
      const hostname = readPayloadValue(event, "hostname") || "未知主机";
      const usagePercent = readPayloadValue(event, "usage_percent") || "0";
      const thresholdPercent =
        readPayloadValue(event, "threshold_percent") || "0";
      const recoverPercent = readPayloadValue(event, "recover_percent") || "0";

      summary = recovered
        ? `${hostname} ${metricLabel} 使用率已恢复至 ${usagePercent}%`
        : `${hostname} ${metricLabel} 使用率已升至 ${usagePercent}%`;
      overview = recovered
        ? `${hostname} 的 ${metricLabel} 使用率已回落到 ${usagePercent}%，恢复线为 ${recoverPercent}%，此前告警阈值为 ${thresholdPercent}%。`
        : `${hostname} 的 ${metricLabel} 使用率当前为 ${usagePercent}%，已超过告警阈值 ${thresholdPercent}%，恢复线设置为 ${recoverPercent}%。`;
      advice = recovered
        ? "当前资源已回到相对安全区间，建议继续观察后续是否还有反复波动。"
        : "建议尽快检查高负载进程、后台任务或外部流量变化，避免资源持续打满。";

      pushFact(facts, "主机名", hostname);
      pushFact(facts, "当前使用率", `${usagePercent}%`);
      pushFact(facts, "告警阈值", `${thresholdPercent}%`);
      pushFact(facts, "恢复阈值", `${recoverPercent}%`);
      pushFact(
        facts,
        "采样间隔",
        readPayloadValue(event, "sample_interval_seconds")
          ? `${readPayloadValue(event, "sample_interval_seconds")} 秒`
          : "",
      );
      pushFact(
        facts,
        "持续时间",
        readPayloadValue(event, "sustain_seconds")
          ? `${readPayloadValue(event, "sustain_seconds")} 秒`
          : "",
      );
      break;
    }
    case "FN_EVENT_TUNNEL_FRP_CONNECTED":
    case "FN_EVENT_TUNNEL_FRP_DISCONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED": {
      const tunnel =
        TUNNEL_LABELS[
          readPayloadValue(event, "tunnel") as keyof typeof TUNNEL_LABELS
        ] || (event.type.includes("CLOUDFLARED") ? "Cloudflared" : "FRP");
      const connected = readPayloadValue(event, "status") === "connected";
      const message = truncateText(readPayloadValue(event, "message"), 200);
      const pid = readPayloadValue(event, "pid");

      summary = `${tunnel} ${connected ? "已连上" : "已断开"}`;
      overview = connected
        ? `${tunnel} 隧道连接已经恢复${message ? `，运行反馈为：${message}` : ""}。`
        : `${tunnel} 隧道连接已断开${message ? `，当前反馈为：${message}` : ""}。`;
      advice = connected
        ? "如你之前正在排查访问问题，现在可以重新验证外部入口是否已经恢复。"
        : "建议检查隧道配置、上游网络状态，以及远端服务是否可达。";

      pushFact(facts, "隧道类型", tunnel);
      pushFact(facts, "连接状态", connected ? "已连上" : "已断开");
      pushFact(facts, "进程 PID", pid);
      pushFact(facts, "运行反馈", message);
      break;
    }
  }

  pushFact(facts, "事件类型", formatNotificationEventLabel(event.type));
  pushFact(facts, "风险级别", formatEventLevelLabel(event.level));
  pushFact(facts, "事件来源", formatEventSourceLabel(event.source));
  pushFact(facts, "发生时间", formatDateTime(event.happened_at));
  pushFact(
    facts,
    "聚合统计",
    matchedCount > 1
      ? `${matchedCount} 条 / ${rule.window_seconds} 秒窗口`
      : "",
  );

  return {
    summary,
    body_text: buildBodyText({
      overview,
      aggregation,
      advice,
    }),
    body_markdown: buildBodyMarkdown({
      overview,
      aggregation,
      advice,
    }),
    facts,
  };
};

const formatEventSummary = (event: SystemEventEnvelope) => {
  switch (event.type) {
    case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
      const authMethod = readPayloadValue(event, "auth_method");
      const authProviderName = readPayloadValue(event, "auth_provider_name");
      if (authMethod === "OIDC" && authProviderName) {
        return joinCompactParts(
          `通过 ${authProviderName} 登录`,
          readPayloadValue(event, "credential_name") || "未知凭证",
          formatSessionCommentCompact(readPayloadValue(event, "session_comment")),
          readPayloadValue(event, "ip"),
        );
      }
      return joinCompactParts(
        readPayloadValue(event, "credential_name") || "未知凭证",
        formatSessionCommentCompact(readPayloadValue(event, "session_comment")),
        readPayloadValue(event, "ip"),
      );
    }
    case "FN_EVENT_AUTH_LOGOUT":
      return joinCompactParts(
        readPayloadValue(event, "credential_name") || "未知凭证",
        formatSessionCommentCompact(readPayloadValue(event, "session_comment")),
        readPayloadValue(event, "ip"),
      );
    case "FN_EVENT_AUTH_LOGIN_FAILURE":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "attempts")
          ? `${readPayloadValue(event, "attempts")}次失败`
          : "",
      );
    case "FN_EVENT_AUTH_SESSION_IP_DRIFT":
      return joinCompactParts(
        formatCredentialContext(event),
        formatSessionCommentCompact(readPayloadValue(event, "session_comment")),
        formatIpTransition(
          readPayloadValue(event, "from_ip"),
          readPayloadValue(event, "to_ip"),
        ),
      );
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "hit_count")
          ? `${readPayloadValue(event, "hit_count")}次扫描`
          : "扫描拦截",
      );
    case "FN_EVENT_DDNS_UPDATE_COMPLETED":
      return joinCompactParts(
        readPayloadValue(event, "target_name") ||
          readPayloadValue(event, "domain_summary") ||
          readPayloadValue(event, "provider"),
        readPayloadValue(event, "success") === "true" ? "成功" : "失败",
      );
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "block_seconds")
          ? `封锁${readPayloadValue(event, "block_seconds")}s`
          : "触发封锁",
      );
    case "FN_EVENT_WAF_BLOCKED": {
      const outcomeLabel = formatWAFOutcomeLabel(
        readPayloadValue(event, "action"),
        readPayloadValue(event, "mode"),
      );
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "host"),
        `WAF ${outcomeLabel}`,
        readPayloadValue(event, "rule_ids")
          ? `规则 ${readPayloadValue(event, "rule_ids")}`
          : "",
      );
    }
    case "FN_EVENT_SSH_LOGIN_SUCCESS":
      return joinCompactParts(
        readPayloadValue(event, "username"),
        readPayloadValue(event, "ip"),
        "SSH 登录成功",
      );
    case "FN_EVENT_SSH_LOGIN_FAILURE":
      return joinCompactParts(
        readPayloadValue(event, "username"),
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "attempts")
          ? `${readPayloadValue(event, "attempts")}次失败`
          : "SSH 登录失败",
      );
    case "FN_EVENT_SSH_IP_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "reason") === "cidr_not_allowed"
          ? "地区不允许"
          : "失败阈值",
      );
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE":
      return joinCompactParts(
        readPayloadValue(event, "latest_version"),
        readPayloadValue(event, "local_version")
          ? `当前 ${readPayloadValue(event, "local_version")}`
          : "",
      );
    case "FN_EVENT_SYSTEM_CPU_ALERT":
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED":
      return joinCompactParts(
        readPayloadValue(event, "hostname"),
        readPayloadValue(event, "usage_percent")
          ? `${readPayloadValue(event, "usage_percent")}%`
          : "",
      );
    case "FN_EVENT_TUNNEL_FRP_CONNECTED":
    case "FN_EVENT_TUNNEL_FRP_DISCONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED":
      return joinCompactParts(
        TUNNEL_LABELS[
          readPayloadValue(event, "tunnel") as keyof typeof TUNNEL_LABELS
        ] || (event.type.includes("CLOUDFLARED") ? "Cloudflared" : "FRP"),
        readPayloadValue(event, "status") === "connected" ? "已连上" : "已断开",
      );
    default:
      return "";
  }
};

const buildNotificationTitle = (
  event: SystemEventEnvelope,
  matchedCount: number,
) => {
  const driftCredentialName = readPayloadValue(event, "credential_name");
  const baseTitle =
    event.type === "FN_EVENT_DDNS_UPDATE_COMPLETED"
      ? readPayloadValue(event, "success") === "true"
        ? `${readPayloadValue(event, "target_name") || readPayloadValue(event, "domain_summary") || "DDNS"} 更新成功`
        : `${readPayloadValue(event, "target_name") || readPayloadValue(event, "domain_summary") || "DDNS"} 更新失败`
      : event.type === "FN_EVENT_AUTH_SESSION_IP_DRIFT"
        ? driftCredentialName
          ? `凭证「${driftCredentialName}」IP 漂移`
          : formatNotificationEventLabel(event.type)
        : event.type === "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"
          ? `发现新版本 ${readPayloadValue(event, "latest_version") || ""}`.trim()
          : formatNotificationEventLabel(event.type);

  return matchedCount > 1 ? `${baseTitle} x${matchedCount}` : baseTitle;
};

const toSeverity = (event: SystemEventEnvelope): NotificationSeverity => {
  switch (event.level) {
    case "INFO":
      return "info";
    case "WARN":
      return "warn";
    case "ERROR":
      return "error";
    case "CRITICAL":
      return "critical";
    default:
      return "info";
  }
};

export const buildNotificationMessage = (args: {
  event: SystemEventEnvelope;
  rule: NotificationRule;
  matchedCount: number;
  groupKey: string;
}): NotificationMessage => {
  const details = buildNotificationDetails(args);

  return normalizeNotificationMessage({
    title: buildNotificationTitle(args.event, args.matchedCount),
    summary: details.summary,
    body_text: details.body_text,
    body_markdown: details.body_markdown,
    severity: toSeverity(args.event),
    facts: details.facts,
    actions: [],
    mentions: [],
    dedupe_key: `${args.rule.id}:${args.groupKey}`,
    occurred_at: args.event.happened_at,
    event_id: args.event.id,
    metadata: {
      event_type: args.event.type,
      event_level: args.event.level,
      event_source: args.event.source,
      rule_id: args.rule.id,
      rule_name: args.rule.name,
      group_key: args.groupKey,
      matched_count: args.matchedCount,
      window_seconds: args.rule.window_seconds,
      threshold_count: args.rule.threshold_count,
    },
  });
};
