import { Elysia, t } from "elysia";
import {
  DEFAULT_IP_LOCATION_API_CONFIG,
  DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
  type AppConfig,
  configManager,
  DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
  DEFAULT_GATEWAY_PORTAL_CONFIG,
  DEFAULT_GATEWAY_VISIBILITY_CONFIG,
  DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  type GatewayHostResponseRuntimeState,
  type GatewayProxyHeadersRuntimeState,
  type GatewayVisibilityRuntimeState,
  type HostMapping,
  type IpLocationApiMode,
  type LoginSession,
  type ProtocolMappingFeatureConfig,
  type ProxyMapping,
  redis,
  type RunModePromptPreferences,
  type StreamMapping,
} from "../lib/redis";
import { generateSecret, generateURI, verifySync } from "otplib";
import { goBackend, type GoResponse } from "../lib/go-backend";
import { firewallService } from "../lib/firewall-service";
import { randomBytes } from "node:crypto";
import { authMobilitySessionManager } from "../lib/auth-mobility-session";
import { ipLocationRefs, ipLocationService } from "../lib/ip-location";
import { revokeCustomPostLoginIpGrant } from "../lib/post-login-ip-grant";
import { systemEventManager } from "../lib/system-events/manager";
import {
  FN_EVENT_AUTH_LOGIN_FAILURE,
  FN_EVENT_SECURITY_SCANNER_BLOCKED,
} from "../lib/system-events/constants";
import { emitLogoutEvent } from "../lib/system-events/helpers";
import {
  scheduleSyncReverseProxyTrustedIPs,
  syncReverseProxyTrustedIPsNow,
} from "../lib/reverse-proxy-trusted-ips";
import { whitelistManager } from "../lib/whitelist-manager";
import {
  buildGatewayAuthConfig,
  buildSubdomainCertificateInventoryCoverage,
  getAuthHostMapping,
  resolvePublicPortForScheme,
} from "../lib/subdomain-mode";
import { isAuthServiceTarget } from "../lib/auth-service";
import {
  resolveHostMappingDisplayTitle,
  refreshAllHostMappingTitles,
  scheduleHostMappingsMetadataRefresh,
} from "../lib/host-mapping-metadata";
import {
  applyGatewayPortalTitleHostRulesPatchIfNeeded,
  syncGatewayPortalHostRulesIfTitleMode,
  syncGatewayPortalToGateway,
} from "../lib/gateway-portal";
import { fetchUrlMetadata } from "../lib/url-metadata";
import { probeBasicAuthTarget } from "../lib/basic-auth-probe";
import {
  buildHostMappingsBookmarkFilename,
  buildHostMappingsBookmarksDocument,
} from "../lib/host-mapping-bookmarks";
import { getGatewayLoggingConfigForResponse } from "../lib/gateway-logging";
import { syncWAFConfigToGateway } from "../lib/waf/service";
import { wafLogStore } from "../lib/waf/log-store";
import {
  buildGatewayHostResponseSummary,
  compileGatewayHostResponseState,
  getGatewayHostResponseDetails,
  syncGatewayHostResponseRuntimeForConfig,
  syncGatewayHostResponseToGateway,
} from "../lib/gateway-host-response";
import {
  buildGatewayProxyHeadersSummary,
  compileGatewayProxyHeadersState,
  getGatewayProxyHeadersDetails,
  syncGatewayProxyHeadersRuntimeForConfig,
  syncGatewayProxyHeadersToGateway,
} from "../lib/gateway-proxy-headers";
import {
  buildGatewayVisibilitySummary,
  compileGatewayVisibilityConfig,
  getGatewayVisibilityDetails,
  syncGatewayVisibilityToGateway,
} from "../lib/gateway-visibility";
import {
  getSmartConnectDetails,
  scheduleSmartConnectSyncAfterHostMappingsChange,
  syncSmartConnect,
} from "../lib/smart-connect";
import { syncSSLDeploymentToGateway } from "../lib/ssl-gateway";
import {
  isAnySubdomainRoutingMode,
  isReverseProxySubdomainMode,
} from "../lib/reverse-proxy-submode";
import {
  resolveAccessEntryInfo,
  shouldOmitPublicAccessEntryPort,
} from "../lib/access-entry";
import {
  MaintenanceBackupError,
  maintenanceBackupService,
} from "../lib/maintenance-backup";
import {
  getCapabilityUnavailableMessage,
  getRuntimeCapabilities,
  getRuntimeProfile,
} from "../lib/runtime-profile";
import { getClientIp } from "../lib/auth-request";
import { dockerAdminPanelManager } from "../lib/docker-admin-panel";
import {
  buildAdminPanelSessionClearCookie,
  buildAdminPanelSessionCookie,
} from "../lib/session-cookie";
import { isValidHostPort } from "../../../../packages/admin-shared/src/utils/parseHostPort";
import {
  buildIpLocationApiUrl,
  normalizeIpLocationServiceUrl,
} from "../lib/ip-location-api-url";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import {
  autoHttpsRedirectManager,
  type AutoHttpsConfig,
} from "../lib/auto-https-redirect";
import { oidcAuthService } from "../lib/auth/oidc/service";

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

const buildCapabilityBlockedResponse = (
  set: { status?: number | string },
  capability: Parameters<typeof getCapabilityUnavailableMessage>[0],
) => {
  set.status = 403;
  return {
    success: false,
    message: getCapabilityUnavailableMessage(capability),
  };
};

const getRunTypeLabel = (runType: 0 | 1 | 3) => {
  if (runType === 0) return "直连模式";
  if (runType === 1) return "反代模式";
  return "子域模式";
};

type HostLocationResponseInput = {
  status?: unknown;
  content_type?: unknown;
  headers?: Record<string, unknown> | null;
  body?: unknown;
};

type HostLocationInput = {
  path?: unknown;
  match?: unknown;
  action?: unknown;
  target?: unknown;
  strip_path?: unknown;
  rewrite_html?: unknown;
  response?: HostLocationResponseInput | null;
};

const validateHostMappings = (
  mappings: Array<{
    host: string;
    target: string;
    use_auth: boolean;
    access_mode: "login_first" | "strict_whitelist";
    suppress_toolbar?: boolean;
    basic_auth?: Partial<HostMapping["basic_auth"]> | null;
    service_role?: "app" | "auth";
    locations?: HostLocationInput[] | null;
  }>,
) => {
  const authMappings = mappings.filter((mapping) =>
    isAuthServiceTarget(mapping.target),
  );
  if (authMappings.length > 1) {
    return {
      valid: false as const,
      message: "只能有一个 Host 映射指向 AUTH_PORT 作为鉴权服务",
    };
  }

  const invalidAuthMapping = authMappings.find(
    (mapping) => mapping.use_auth || mapping.access_mode === "strict_whitelist",
  );
  if (invalidAuthMapping) {
    return {
      valid: false as const,
      message: `鉴权服务 ${invalidAuthMapping.host} 必须保持公开入口，不能开启自身鉴权或严格白名单，否则会导致登录入口不可达`,
    };
  }

  const authMappingWithBasicAuth = authMappings.find(
    (mapping) => mapping.basic_auth?.enabled === true,
  );
  if (authMappingWithBasicAuth) {
    return {
      valid: false as const,
      message: `鉴权服务 ${authMappingWithBasicAuth.host} 不能开启凭证注入`,
    };
  }

  const invalidBasicAuthMapping = mappings.find((mapping) => {
    if (mapping.basic_auth?.enabled !== true) return false;
    const username =
      typeof mapping.basic_auth.username === "string"
        ? mapping.basic_auth.username.trim()
        : "";
    const password =
      typeof mapping.basic_auth.password === "string"
        ? mapping.basic_auth.password
        : "";
    return !username || !password || username.includes(":");
  });
  if (invalidBasicAuthMapping) {
    return {
      valid: false as const,
      message: `Host 映射 ${invalidBasicAuthMapping.host} 的凭证注入需要填写用户名和密码，且用户名不能包含冒号`,
    };
  }

  const invalidLocation = validateHostMappingLocations(mappings);
  if (invalidLocation) {
    return {
      valid: false as const,
      message: invalidLocation,
    };
  }

  return { valid: true as const };
};

const forbiddenHostLocationResponseHeaders = new Set([
  "connection",
  "keep-alive",
  "proxy-connection",
  "proxy-authenticate",
  "proxy-authorization",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "content-length",
  "content-type",
]);

const isValidHTTPHeaderName = (value: string): boolean =>
  /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(value);

const cleanHostLocationPath = (value: string): string => {
  const raw = value.trim();
  if (!raw.startsWith("/")) return raw;

  const segments: string[] = [];
  for (const segment of raw.split("/")) {
    if (!segment || segment === ".") continue;
    if (segment === "..") {
      segments.pop();
      continue;
    }
    segments.push(segment);
  }

  return `/${segments.join("/")}`;
};

const validateHostMappingLocations = (
  mappings: Array<{
    host: string;
    target: string;
    locations?: HostLocationInput[] | null;
  }>,
): string | null => {
  for (const mapping of mappings) {
    if (isAuthServiceTarget(mapping.target.trim())) {
      continue;
    }
    const locations = Array.isArray(mapping.locations) ? mapping.locations : [];
    const seen = new Set<string>();
    for (const location of locations) {
      const locationPath =
        typeof location.path === "string" ? location.path.trim() : "";
      if (!locationPath) {
        return `Host 映射 ${mapping.host} 的路径规则需要填写路径`;
      }
      if (!locationPath.startsWith("/")) {
        return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 必须以 / 开头`;
      }
      const cleanPath = cleanHostLocationPath(locationPath);
      if (cleanPath === "/") {
        return `Host 映射 ${mapping.host} 不允许配置根路径 / 作为路径规则`;
      }
      if (
        cleanPath.startsWith("/__") ||
        cleanPath === "/s" ||
        cleanPath === "/s/"
      ) {
        return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 使用了保留路径`;
      }
      const match = location.match === "exact" ? "exact" : "prefix";
      const duplicateKey = `${match}\0${cleanPath}`;
      if (seen.has(duplicateKey)) {
        return `Host 映射 ${mapping.host} 存在重复路径规则 ${locationPath}`;
      }
      seen.add(duplicateKey);

      const action = location.action === "response" ? "response" : "proxy";
      if (action === "proxy") {
        const target =
          typeof location.target === "string" ? location.target.trim() : "";
        if (!target) {
          return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 需要填写目标`;
        }
      } else {
        const response = (location.response ?? {}) as Partial<
          HostMapping["locations"][number]["response"]
        >;
        const status = Math.floor(Number(response.status) || 200);
        if (status < 100 || status > 599) {
          return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 响应状态码必须在 100 到 599 之间`;
        }
        const headers =
          response.headers &&
          typeof response.headers === "object" &&
          !Array.isArray(response.headers)
            ? response.headers
            : {};
        for (const rawName of Object.keys(headers)) {
          const name = rawName.trim();
          if (!name || !isValidHTTPHeaderName(name)) {
            return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 包含非法响应头 ${rawName}`;
          }
          if (forbiddenHostLocationResponseHeaders.has(name.toLowerCase())) {
            return `Host 映射 ${mapping.host} 的路径规则 ${locationPath} 不能自定义响应头 ${name}`;
          }
        }
      }
    }
  }

  return null;
};

const DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE = "text/plain; charset=utf-8";

const normalizeHostLocationResponseHeaders = (
  value?: Record<string, unknown> | null,
): Record<string, string> => {
  if (!value || typeof value !== "object" || Array.isArray(value)) return {};

  const headers: Record<string, string> = {};
  for (const [rawName, rawValue] of Object.entries(value)) {
    const name = rawName.trim();
    if (
      !name ||
      !isValidHTTPHeaderName(name) ||
      forbiddenHostLocationResponseHeaders.has(name.toLowerCase())
    ) {
      continue;
    }
    headers[name] = typeof rawValue === "string" ? rawValue : String(rawValue);
  }
  return headers;
};

const normalizeHostMappingLocationsForRoute = (
  locations?: HostLocationInput[] | null,
): HostMapping["locations"] => {
  if (!Array.isArray(locations)) return [];

  return locations.map((location) => {
    const action = location.action === "response" ? "response" : "proxy";
    const response = (location.response ?? {}) as Partial<
      HostMapping["locations"][number]["response"]
    >;
    const status = Math.floor(Number(response.status) || 200);
    return {
      path:
        typeof location.path === "string"
          ? cleanHostLocationPath(location.path)
          : "",
      match: location.match === "exact" ? "exact" : "prefix",
      action,
      target:
        action === "proxy" && typeof location.target === "string"
          ? location.target.trim()
          : "",
      strip_path: action === "proxy" ? location.strip_path !== false : false,
      rewrite_html:
        action === "proxy" ? location.rewrite_html !== false : false,
      response:
        action === "response"
          ? {
              status: status >= 100 && status <= 599 ? status : 200,
              content_type:
                typeof response.content_type === "string" &&
                response.content_type.trim()
                  ? response.content_type.trim()
                  : DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE,
              headers: normalizeHostLocationResponseHeaders(response.headers),
              body: typeof response.body === "string" ? response.body : "",
            }
          : {
              status: 200,
              content_type: DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE,
              headers: {},
              body: "",
            },
    };
  });
};

const normalizeHostMappingLookupKey = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "");

const createDisabledHostBasicAuth = (): HostMapping["basic_auth"] => ({
  enabled: false,
  username: "",
  password: "",
});

const normalizeHostBasicAuth = (
  value?: Partial<HostMapping["basic_auth"]> | null,
): HostMapping["basic_auth"] => {
  const raw = value ?? {};
  const username = typeof raw.username === "string" ? raw.username.trim() : "";
  const password = typeof raw.password === "string" ? raw.password : "";

  if (
    raw.enabled !== true ||
    !username ||
    !password ||
    username.includes(":")
  ) {
    return createDisabledHostBasicAuth();
  }

  return {
    enabled: true,
    username,
    password,
  };
};

const toHostRuleSyncPayload = (
  mapping: Pick<
    HostMapping,
    | "host"
    | "target"
    | "use_auth"
    | "access_mode"
    | "suppress_toolbar"
    | "preserve_host"
    | "basic_auth"
    | "locations"
    | "title"
    | "title_override"
  >,
) => ({
  host: normalizeHostMappingLookupKey(mapping.host),
  target: mapping.target.trim(),
  use_auth: mapping.use_auth,
  access_mode: mapping.access_mode,
  suppress_toolbar: mapping.suppress_toolbar,
  preserve_host: mapping.preserve_host,
  title: resolveHostMappingDisplayTitle(mapping),
  basic_auth: normalizeHostBasicAuth(mapping.basic_auth),
  locations: mapping.locations ?? [],
});

const haveSyncedHostRulesChanged = (
  previousMappings: HostMapping[],
  nextMappings: HostMapping[],
): boolean =>
  JSON.stringify(previousMappings.map(toHostRuleSyncPayload)) !==
  JSON.stringify(nextMappings.map(toHostRuleSyncPayload));

const isSameJsonValue = (left: unknown, right: unknown): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

const ensureGoResponseSuccess = <T>(
  response: GoResponse<T>,
  fallbackMessage: string,
): GoResponse<T> => {
  if (response.success) {
    return response;
  }

  throw new Error(response.message || fallbackMessage);
};

const isValidStreamTarget = (target: string): boolean => {
  return isValidHostPort(target);
};

const validateIpLocationBaseUrl = (value: unknown, label: string) => {
  const url = normalizeIpLocationServiceUrl(value);
  if (!url) {
    return {
      valid: false as const,
      url,
      message: `${label}不能为空`,
    };
  }

  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return {
        valid: false as const,
        url,
        message: `${label}必须以 http:// 或 https:// 开头`,
      };
    }
  } catch {
    return {
      valid: false as const,
      url,
      message: `${label}格式不正确`,
    };
  }

  return { valid: true as const, url };
};

type StreamMappingInput = Pick<
  StreamMapping,
  "listen_port" | "target" | "use_auth"
> & {
  protocol?: StreamMapping["protocol"];
};

const validateStreamMappings = (mappings: StreamMappingInput[]) => {
  const seenMappings = new Set<string>();

  for (const mapping of mappings) {
    const protocol = mapping.protocol === "udp" ? "udp" : "tcp";
    const listenPort = mapping.listen_port;
    const target = mapping.target;

    if (!Number.isInteger(listenPort)) {
      return {
        valid: false as const,
        message: `监听端口 ${listenPort} 不是有效整数`,
      };
    }
    if (listenPort <= 0 || listenPort > 65535) {
      return {
        valid: false as const,
        message: `监听端口 ${listenPort} 超出有效范围`,
      };
    }
    const mappingKey = `${protocol}:${listenPort}`;
    if (seenMappings.has(mappingKey)) {
      return {
        valid: false as const,
        message: `${protocol.toUpperCase()} 监听端口 ${listenPort} 重复，请保持协议 + 端口唯一`,
      };
    }
    if (!isValidStreamTarget(target)) {
      return {
        valid: false as const,
        message: `目标地址 ${target} 必须是 host:port 形式`,
      };
    }
    seenMappings.add(mappingKey);
  }

  return { valid: true as const };
};

const normalizeHostLike = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const resolveBookmarkScheme = (
  config: Pick<AppConfig, "ssl">,
): "http" | "https" =>
  config.ssl.cert.trim() && config.ssl.key.trim() ? "https" : "http";

const validatePasskeyRpConfig = (
  config: Awaited<ReturnType<typeof configManager.getConfig>>,
) => {
  const mode =
    config.subdomain_mode?.passkey_rp_mode === "parent_domain"
      ? "parent_domain"
      : "auth_host";
  if (mode !== "parent_domain") {
    return { valid: true as const };
  }

  const rpId = normalizeHostLike(
    config.subdomain_mode?.passkey_rp_id || config.subdomain_mode?.root_domain,
  );
  if (!rpId) {
    return {
      valid: false as const,
      message:
        "启用父域 Passkey RP 时，请先填写根域名，或显式指定一个父域 RP ID。",
    };
  }

  const authHost = normalizeHostLike(
    getAuthHostMapping(config)?.host || config.subdomain_mode?.auth_host,
  );
  if (authHost && authHost !== rpId && !authHost.endsWith(`.${rpId}`)) {
    return {
      valid: false as const,
      message: `父域 Passkey RP ID ${rpId} 必须与鉴权服务 ${authHost} 相同，或是它的父域。`,
    };
  }

  return { valid: true as const };
};

const resolveSessionDefaultComment = async (
  sessionId: string,
  session: LoginSession,
): Promise<string | undefined> => {
  const sessionGrantRecord = session.postLoginIpGrantRecordId
    ? await whitelistManager.getRecordById(session.postLoginIpGrantRecordId)
    : null;
  if (
    sessionGrantRecord?.status === "active" &&
    sessionGrantRecord.comment !== undefined
  ) {
    return sessionGrantRecord.comment;
  }

  const [whitelistRecordId] =
    await authMobilitySessionManager.listSessionWhitelistRecordIds(sessionId);
  const boundRecord = whitelistRecordId
    ? await whitelistManager.getRecordById(whitelistRecordId)
    : null;
  if (boundRecord?.status === "active" && boundRecord.comment !== undefined) {
    return boundRecord.comment;
  }

  const latestRecord = await whitelistManager.getLatestActiveRecordByIP(
    session.ip,
  );
  if (!latestRecord || latestRecord.comment === undefined) {
    return undefined;
  }

  return latestRecord.comment;
};

const ensureSessionComment = async (
  sessionId: string,
  session: LoginSession,
): Promise<LoginSession> => {
  if (session.comment !== undefined) {
    return session;
  }

  const comment = await resolveSessionDefaultComment(sessionId, session);
  if (comment === undefined) {
    return session;
  }

  return (
    (await configManager.updateSession(sessionId, { comment })) ?? {
      ...session,
      comment,
    }
  );
};

const rollbackConfigAndRuntime = async (
  previousConfig: AppConfig,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复之前的配置失败";
  }

  try {
    await syncSmartConnect(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复之前的智能连接运行态失败";
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || "恢复之前的运行态失败";
  }

  return null;
};

const rollbackProtocolMappingFeatureAndRuntime = async (
  previousSettings: ProtocolMappingFeatureConfig,
  previousConfig: AppConfig,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复协议映射配置失败";
  }

  try {
    await configManager.updateProtocolMappingFeatureConfig(previousSettings);
  } catch (error: any) {
    return error?.message || "恢复协议映射功能开关失败";
  }

  try {
    await syncSmartConnect(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复智能连接运行态失败";
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || "恢复协议映射运行态失败";
  }

  return null;
};

const rollbackGatewayVisibilityConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayVisibilityRuntimeState,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复可见性原始配置失败";
  }

  try {
    await configManager.saveGatewayVisibilityRuntimeState(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复可见性运行时 CIDR 失败";
  }

  try {
    await syncGatewayVisibilityToGateway(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复网关可见性运行态失败";
  }

  return null;
};

const buildAutoHttpsDetails = async (settings?: AutoHttpsConfig) => {
  const config = settings ?? (await configManager.getAutoHttpsConfig());
  return {
    ...config,
    runtime: autoHttpsRedirectManager.getRuntimeState(),
  };
};

const rollbackGatewayProxyHeadersConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayProxyHeadersRuntimeState,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复协议头原始配置失败";
  }

  try {
    await configManager.saveGatewayProxyHeadersRuntimeState(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复协议头运行态失败";
  }

  try {
    await syncGatewayProxyHeadersToGateway(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复网关协议头运行态失败";
  }

  return null;
};

const rollbackGatewayHostResponseConfigAndRuntime = async (
  previousConfig: AppConfig,
  previousRuntime: GatewayHostResponseRuntimeState,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || "恢复 Host 响应原始配置失败";
  }

  try {
    await configManager.saveGatewayHostResponseRuntimeState(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复 Host 响应运行态失败";
  }

  try {
    await syncGatewayHostResponseToGateway(previousRuntime);
  } catch (error: any) {
    return error?.message || "恢复网关 Host 响应运行态失败";
  }

  return null;
};

const buildGatewaySettingsResponse = (
  config: Pick<
    AppConfig,
    | "subdomain_mode"
    | "reverse_proxy_throttle"
    | "gateway_visibility"
    | "gateway_proxy_headers"
    | "gateway_host_response"
    | "gateway_portal"
    | "host_mappings"
  >,
  visibilityRuntime: GatewayVisibilityRuntimeState,
  proxyHeadersRuntime: GatewayProxyHeadersRuntimeState,
  hostResponseRuntime: GatewayHostResponseRuntimeState,
) => ({
  auth_cache_ttl_seconds: config.subdomain_mode?.auth_cache_ttl_seconds ?? 1,
  auth_cache_unauthorized_ttl_seconds:
    config.subdomain_mode?.auth_cache_unauthorized_ttl_seconds ?? 1,
  reverse_proxy_throttle: config.reverse_proxy_throttle ?? {
    ...DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
  },
  visibility: buildGatewayVisibilitySummary(
    config.gateway_visibility ?? DEFAULT_GATEWAY_VISIBILITY_CONFIG,
    visibilityRuntime,
  ),
  proxy_headers: buildGatewayProxyHeadersSummary(
    compileGatewayProxyHeadersState(
      {
        run_type: 3,
        reverse_proxy_submode: "subdomain",
        host_mappings: config.host_mappings,
        gateway_proxy_headers:
          config.gateway_proxy_headers ?? DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
      },
      config.gateway_proxy_headers ?? DEFAULT_GATEWAY_PROXY_HEADERS_CONFIG,
    ).items,
    proxyHeadersRuntime,
  ),
  host_response: buildGatewayHostResponseSummary(
    compileGatewayHostResponseState(
      {
        run_type: 3,
        reverse_proxy_submode: "subdomain",
        host_mappings: config.host_mappings,
        gateway_host_response:
          config.gateway_host_response ?? DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
      },
      config.gateway_host_response ?? DEFAULT_GATEWAY_HOST_RESPONSE_CONFIG,
    ).items,
    hostResponseRuntime,
  ),
  portal: config.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
});

const syncHostMappingsRuntime = async (
  previousConfig: AppConfig,
  nextConfig: AppConfig,
  normalizedMappings: HostMapping[],
): Promise<void> => {
  const previousGatewayAuthConfig = buildGatewayAuthConfig(previousConfig);
  const nextGatewayAuthConfig = buildGatewayAuthConfig(nextConfig);

  if (
    haveSyncedHostRulesChanged(previousConfig.host_mappings, normalizedMappings)
  ) {
    ensureGoResponseSuccess(
      await goBackend.setHostRules(normalizedMappings),
      "同步 Host 路由失败",
    );
  }

  if (!isSameJsonValue(previousGatewayAuthConfig, nextGatewayAuthConfig)) {
    ensureGoResponseSuccess(
      await goBackend.setAuthConfig(nextGatewayAuthConfig),
      "同步鉴权网关配置失败",
    );
  }

  await syncGatewayProxyHeadersRuntimeForConfig(nextConfig, {
    saveConfig: true,
  });
  await syncGatewayHostResponseRuntimeForConfig(nextConfig, {
    saveConfig: true,
  });
};

const buildCountSeries = (
  timestamps: number[],
  fromMs: number,
  toMs: number,
  bucketCount: number,
) => {
  const span = Math.max(1, toMs - fromMs);
  const step = Math.max(1, Math.ceil(span / bucketCount));
  const buckets = Array.from({ length: bucketCount }, () => 0);
  for (const ts of timestamps) {
    if (!Number.isFinite(ts)) continue;
    const idx = Math.min(
      bucketCount - 1,
      Math.max(0, Math.floor((ts - fromMs) / step)),
    );
    const current = buckets[idx] ?? 0;
    buckets[idx] = current + 1;
  }
  return buckets.map(
    (count, index) => [fromMs + index * step, count] as [number, number],
  );
};

export const adminRoutes = new Elysia({
  prefix: "/api/admin",
  tags: ["Admin"],
})
  .get(
    "/panel/bootstrap",
    async ({ request }) => {
      return {
        success: true,
        data: await dockerAdminPanelManager.buildBootstrapState(
          request,
          getRuntimeProfile().is_docker,
        ),
      };
    },
    routeDoc("获取 Docker 管理面板登录状态"),
  )
  .post(
    "/panel/password",
    async ({ request, body, set }) => {
      if (!getRuntimeProfile().is_docker) {
        set.status = 400;
        return {
          success: false,
          message: "当前运行模式不需要设置 Docker 管理面板密码",
        };
      }

      try {
        await dockerAdminPanelManager.setPassword(body.password);
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error ? error.message : "设置管理面板密码失败",
        };
      }

      const session = await dockerAdminPanelManager.createSession({
        ip: getClientIp(request) || "unknown",
        userAgent: request.headers.get("user-agent") || "",
      });
      await dockerAdminPanelManager.resetLoginFailures(getClientIp(request));
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        dockerAdminPanelManager.sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          session_expires_at: session.expires_at,
        },
      };
    },
    withRouteDoc("首次设置 Docker 管理面板密码", {
      body: t.Object({
        password: t.String(),
      }),
    }),
  )
  .post(
    "/panel/password/change",
    async ({ request, body, set }) => {
      if (!getRuntimeProfile().is_docker) {
        set.status = 400;
        return {
          success: false,
          message: "当前运行模式不支持修改 Docker 管理面板密码",
        };
      }

      try {
        await dockerAdminPanelManager.changePassword(body.password);
      } catch (error) {
        set.status = 400;
        return {
          success: false,
          message:
            error instanceof Error ? error.message : "修改管理面板密码失败",
        };
      }

      const session = await dockerAdminPanelManager.createSession({
        ip: getClientIp(request) || "unknown",
        userAgent: request.headers.get("user-agent") || "",
      });
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        dockerAdminPanelManager.sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          session_expires_at: session.expires_at,
        },
      };
    },
    withRouteDoc("修改 Docker 管理面板密码", {
      body: t.Object({
        password: t.String(),
      }),
    }),
  )
  .post(
    "/panel/login",
    async ({ request, body, set }) => {
      if (!getRuntimeProfile().is_docker) {
        return {
          success: true,
          data: await dockerAdminPanelManager.buildBootstrapState(
            request,
            false,
          ),
        };
      }

      const clientIp = getClientIp(request) || "unknown";
      const gate = await dockerAdminPanelManager.ensureLoginAllowed(clientIp);
      if (!gate.allowed) {
        set.status = 429;
        if (gate.retryAfter) {
          set.headers["Retry-After"] = String(gate.retryAfter);
        }
        return {
          success: false,
          message: gate.retryAfter
            ? `尝试过于频繁，请在 ${gate.retryAfter} 秒后重试`
            : "尝试过于频繁，请稍后重试",
          retryAfter: gate.retryAfter,
          blockedUntil: gate.blockedUntil,
        };
      }

      const passwordConfigured =
        await dockerAdminPanelManager.isPasswordConfigured();
      if (!passwordConfigured) {
        set.status = 409;
        return {
          success: false,
          message: "当前还没有设置管理面板密码，请先完成首次设置",
        };
      }

      const passwordValid = await dockerAdminPanelManager.verifyPassword(
        body.password,
      );
      if (!passwordValid) {
        const failure =
          await dockerAdminPanelManager.registerLoginFailure(clientIp);
        set.status = 429;
        set.headers["Retry-After"] = String(failure.retryAfter);
        return {
          success: false,
          message: `管理面板密码错误，请在 ${failure.retryAfter} 秒后重试`,
          retryAfter: failure.retryAfter,
          blockedUntil: failure.blockedUntil,
        };
      }

      await dockerAdminPanelManager.resetLoginFailures(clientIp);
      const session = await dockerAdminPanelManager.createSession({
        ip: clientIp,
        userAgent: request.headers.get("user-agent") || "",
      });
      set.headers["set-cookie"] = buildAdminPanelSessionCookie(
        session.id,
        dockerAdminPanelManager.sessionTtlSeconds,
        {
          secure: dockerAdminPanelManager.isSecureRequest(request),
        },
      );

      return {
        success: true,
        data: {
          enabled: true,
          password_configured: true,
          authenticated: true,
          session_expires_at: session.expires_at,
        },
      };
    },
    withRouteDoc("登录 Docker 管理面板", {
      body: t.Object({
        password: t.String(),
      }),
    }),
  )
  .post(
    "/panel/logout",
    async ({ request, set }) => {
      await dockerAdminPanelManager.deleteSessionFromRequest(request);
      set.headers["set-cookie"] = buildAdminPanelSessionClearCookie({
        secure: dockerAdminPanelManager.isSecureRequest(request),
      });

      return {
        success: true,
        data: await dockerAdminPanelManager.buildBootstrapState(
          request,
          getRuntimeProfile().is_docker,
        ),
      };
    },
    routeDoc("退出 Docker 管理面板"),
  )
  .get(
    "/healthz",
    async ({ set }) => {
      let redisReachable = false;
      let redisError: string | null = null;

      try {
        redisReachable = (await redis.ping()) === "PONG";
      } catch (error) {
        redisError =
          error instanceof Error ? error.message : "Redis is unavailable";
      }

      const gatewayProbe = await goBackend.getServerInfo();
      const isHealthy = redisReachable && gatewayProbe.success;

      if (!isHealthy) {
        set.status = 503;
      }

      return {
        success: isHealthy,
        data: {
          node: {
            alive: true,
            pid: process.pid,
          },
          redis: {
            reachable: redisReachable,
            error: redisError,
          },
          runtime_profile: getRuntimeProfile(),
          gateway_admin: {
            reachable: gatewayProbe.success,
            version: gatewayProbe.data?.version ?? null,
            error:
              gatewayProbe.success === true
                ? null
                : gatewayProbe.message || "Gateway admin probe failed",
          },
        },
      };
    },
    routeDoc("获取运行时健康检查状态"),
  )
  .get(
    "/config",
    async () => {
      const [config, gatewayLogging] = await Promise.all([
        configManager.getConfigSafe(),
        getGatewayLoggingConfigForResponse(),
      ]);

      return {
        success: true,
        data: {
          ...config,
          gateway_logging: gatewayLogging,
        },
      };
    },
    routeDoc("获取管理端完整配置"),
  )
  .post(
    "/config/run_type",
    async ({ body, set }) => {
      if (
        body.run_type === 0 &&
        !getRuntimeCapabilities().direct_mode_available
      ) {
        return buildCapabilityBlockedResponse(set, "direct_mode_available");
      }

      const [config, previousProtocolMappingFeature] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      const previousRunType = config.run_type;
      try {
        await configManager.updateRunType(
          body.run_type,
          body.reverse_proxy_submode,
        );
        if (body.run_type !== 3) {
          await configManager.updateProtocolMappingFeatureConfig({
            enabled: false,
          });
        }
        await syncSmartConnect(await configManager.getConfig());
        await firewallService.applyRunTypeConfig(
          body.run_type,
          previousRunType,
        );
        if (body.run_type === 0) {
          try {
            const removedAutoGrantCount =
              await whitelistManager.removeRecordsBySource("auto");
            if (removedAutoGrantCount > 0) {
              scheduleSyncReverseProxyTrustedIPs({
                reason: "run-type-direct-cleanup",
              });
            }
          } catch (cleanupError) {
            console.error(
              "[admin][run_type] failed to clear login IP grants after switching to direct mode:",
              cleanupError,
            );
          }
        }
      } catch (error: any) {
        const rollbackError = await rollbackProtocolMappingFeatureAndRuntime(
          previousProtocolMappingFeature,
          config,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "切换运行模式失败"}；回滚失败：${rollbackError}`
            : error?.message || "切换运行模式失败，已回滚配置",
        };
      }

      return { success: true };
    },
    withRouteDoc("切换运行模式", {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
        reverse_proxy_submode: t.Optional(
          t.Union([t.Literal("path"), t.Literal("subdomain")]),
        ),
      }),
    }),
  )
  .post(
    "/config/auto_manage_firewall",
    async ({ body, set }) => {
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }

      const next = await configManager.updateAutoManageFirewall(
        body.auto_manage_firewall,
      );
      return {
        success: true,
        data: {
          auto_manage_firewall: next,
        },
      };
    },
    withRouteDoc("更新防火墙自动管理开关", {
      body: t.Object({
        auto_manage_firewall: t.Boolean(),
      }),
    }),
  )
  .post(
    "/firewall/reset",
    async ({ body, set }) => {
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }
      if (
        body.run_type === 0 &&
        !getRuntimeCapabilities().direct_mode_available
      ) {
        return buildCapabilityBlockedResponse(set, "direct_mode_available");
      }

      try {
        const result = await firewallService.resetFirewallForRunType(
          body.run_type,
        );
        const whitelistMessage =
          body.run_type === 0
            ? `，并同步 ${result.whitelistSynced} 条白名单 IP`
            : "";
        const exemptPortsMessage =
          body.run_type === 0 || body.run_type === 3
            ? `，保留入口端口 ${result.exemptPorts.join("、")}`
            : "";

        return {
          success: true,
          data: result,
          message: `已按${getRunTypeLabel(body.run_type)}重设防火墙${whitelistMessage}${exemptPortsMessage}`,
        };
      } catch (error: any) {
        set.status = 502;
        return {
          success: false,
          message: error?.message || "重设防火墙失败",
        };
      }
    },
    withRouteDoc("按运行模式重置防火墙", {
      body: t.Object({
        run_type: t.Union([t.Literal(0), t.Literal(1), t.Literal(3)]),
      }),
    }),
  )
  .post(
    "/firewall/clear",
    async ({ set }) => {
      if (!getRuntimeCapabilities().host_firewall_available) {
        return buildCapabilityBlockedResponse(set, "host_firewall_available");
      }

      try {
        const result = await firewallService.clearFirewall();
        return {
          success: true,
          data: result,
          message: `已清空防火墙规则，并移除 ${result.gatewayPort} 端口相关的历史重定向`,
        };
      } catch (error: any) {
        set.status = 502;
        return {
          success: false,
          message: error?.message || "清空防火墙失败",
        };
      }
    },
    routeDoc("清空防火墙规则"),
  )
  .get(
    "/config/run_mode_prompt_preferences",
    async () => {
      const preferences = await configManager.getRunModePromptPreferences();
      return { success: true, data: preferences };
    },
    routeDoc("获取运行模式提示偏好"),
  )
  .get(
    "/config/welcome_guide",
    async () => {
      const status = await configManager.getWelcomeGuideStatus();
      return { success: true, data: status };
    },
    routeDoc("获取欢迎向导状态"),
  )
  .post(
    "/config/welcome_guide/complete",
    async () => {
      const status = await configManager.completeWelcomeGuide();
      return { success: true, data: status };
    },
    routeDoc("完成欢迎向导"),
  )
  .post(
    "/config/run_mode_prompt_preferences",
    async ({ body }) => {
      const patch: Partial<RunModePromptPreferences> = {};

      if (body.directToReverseProxy !== undefined) {
        patch.directToReverseProxy = body.directToReverseProxy;
      }
      if (body.reverseProxyToDirect !== undefined) {
        patch.reverseProxyToDirect = body.reverseProxyToDirect;
      }
      if (body.switchToSubdomain !== undefined) {
        patch.switchToSubdomain = body.switchToSubdomain;
      }
      if (body.subdomainToReverseProxy !== undefined) {
        patch.subdomainToReverseProxy = body.subdomainToReverseProxy;
      }

      const preferences =
        await configManager.updateRunModePromptPreferences(patch);
      return { success: true, data: preferences };
    },
    withRouteDoc("更新运行模式提示偏好", {
      body: t.Object({
        directToReverseProxy: t.Optional(t.Boolean()),
        reverseProxyToDirect: t.Optional(t.Boolean()),
        switchToSubdomain: t.Optional(t.Boolean()),
        subdomainToReverseProxy: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/protocol_mapping_feature",
    async () => {
      const settings = await configManager.getProtocolMappingFeatureConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取协议映射功能开关"),
  )
  .post(
    "/config/protocol_mapping_feature",
    async ({ body, set }) => {
      const [previousConfig, previousSettings] = await Promise.all([
        configManager.getConfig(),
        configManager.getProtocolMappingFeatureConfig(),
      ]);
      if (body.enabled === true && previousConfig.run_type !== 3) {
        set.status = 400;
        return {
          success: false,
          message: "协议映射仅可在子域模式下启用",
        };
      }
      try {
        const next =
          await configManager.updateProtocolMappingFeatureConfig(body);
        if (next.enabled === false) {
          await configManager.updateStreamMappings([]);
        }
        await firewallService.applyRunTypeConfig(
          previousConfig.run_type,
          previousConfig.run_type,
        );
        return { success: true, data: next };
      } catch (error: any) {
        const rollbackError = await rollbackProtocolMappingFeatureAndRuntime(
          previousSettings,
          previousConfig,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新协议映射功能开关失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新协议映射功能开关失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新协议映射功能开关", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/smart_connect/details",
    async () => {
      const details = await getSmartConnectDetails();
      return { success: true, data: details };
    },
    routeDoc("获取智能连接详情"),
  )
  .post(
    "/config/smart_connect",
    async ({ body, set }) => {
      if (!getRuntimeCapabilities().smart_connect_available) {
        return buildCapabilityBlockedResponse(set, "smart_connect_available");
      }

      const previousConfig = await configManager.getConfig();
      if (body.enabled === true && previousConfig.run_type !== 3) {
        set.status = 400;
        return {
          success: false,
          message: "智能连接仅可在子域模式下启用",
        };
      }

      const nextConfig: AppConfig = {
        ...previousConfig,
        smart_connect: {
          ...(previousConfig.smart_connect ?? {
            enabled: false,
            selected_ipv4: "",
          }),
          ...(body.enabled !== undefined ? { enabled: body.enabled } : {}),
          ...(body.selected_ipv4 !== undefined
            ? { selected_ipv4: body.selected_ipv4 }
            : {}),
        },
      };

      try {
        await configManager.saveConfig(nextConfig);
        const details = await syncSmartConnect(nextConfig);
        await firewallService.applyRunTypeConfig(
          nextConfig.run_type,
          previousConfig.run_type,
        );
        return { success: true, data: details };
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新智能连接失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新智能连接失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新智能连接配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        selected_ipv4: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/config/fnos_share_bypass",
    async () => {
      const settings = await configManager.getFnosShareBypassConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取飞牛共享绕过配置"),
  )
  .post(
    "/config/fnos_share_bypass",
    async ({ body }) => {
      const next = await configManager.updateFnosShareBypassConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新飞牛共享绕过配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        upstream_timeout_ms: t.Optional(t.Number()),
        validation_cache_ttl_seconds: t.Optional(t.Number()),
        validation_lock_ttl_seconds: t.Optional(t.Number()),
        session_ttl_seconds: t.Optional(t.Number()),
      }),
    }),
  )
  .get(
    "/config/fnos_port_icon_hijack",
    async () => {
      const settings = await configManager.getFnosPortIconHijackConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取飞牛端口图标接管配置"),
  )
  .post(
    "/config/fnos_port_icon_hijack",
    async ({ body, set }) => {
      const previousConfig = await configManager.getConfig();
      const next = await configManager.updateFnosPortIconHijackConfig(body);
      try {
        ensureGoResponseSuccess(
          await goBackend.setFnosPortIconHijackConfig(next),
          "同步飞牛端口图标接管配置到网关失败",
        );
      } catch (error: any) {
        let rollbackError: string | null = null;
        try {
          const rollbackConfig = await configManager.getConfig();
          rollbackConfig.fnos_port_icon_hijack =
            previousConfig.fnos_port_icon_hijack;
          await configManager.saveConfig(rollbackConfig);
        } catch (innerError: any) {
          rollbackError = innerError?.message || "恢复之前的配置失败";
        }
        set.status = 502;
        const message = error?.message || "同步飞牛端口图标接管配置到网关失败";
        return {
          success: false,
          message: rollbackError
            ? `${message}；回滚失败：${rollbackError}`
            : message,
        };
      }
      return { success: true, data: next };
    },
    withRouteDoc("更新飞牛端口图标接管配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/gateway",
    async () => {
      const [
        config,
        visibilityRuntime,
        proxyHeadersRuntime,
        hostResponseRuntime,
      ] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayVisibilityRuntimeState(),
        configManager.getGatewayProxyHeadersRuntimeState(),
        configManager.getGatewayHostResponseRuntimeState(),
      ]);
      return {
        success: true,
        data: buildGatewaySettingsResponse(
          config,
          visibilityRuntime,
          proxyHeadersRuntime,
          hostResponseRuntime,
        ),
      };
    },
    routeDoc("获取网关配置"),
  )
  .post(
    "/config/gateway",
    async ({ body, set }) => {
      const previousConfig = await configManager.getConfig();

      try {
        const nextAuthConfigPatch: Partial<AppConfig["subdomain_mode"]> = {};
        if (body.auth_cache_ttl_seconds !== undefined) {
          nextAuthConfigPatch.auth_cache_ttl_seconds =
            body.auth_cache_ttl_seconds;
        }
        if (body.auth_cache_unauthorized_ttl_seconds !== undefined) {
          nextAuthConfigPatch.auth_cache_unauthorized_ttl_seconds =
            body.auth_cache_unauthorized_ttl_seconds;
        }

        if (Object.keys(nextAuthConfigPatch).length > 0) {
          await configManager.updateSubdomainModeConfig(nextAuthConfigPatch);
        }

        if (body.reverse_proxy_throttle) {
          await configManager.updateReverseProxyThrottleConfig(
            body.reverse_proxy_throttle,
          );
        }

        if (body.portal) {
          await configManager.updateGatewayPortalConfig(body.portal);
        }

        const updatedConfig = await configManager.getConfig();
        const [
          visibilityRuntime,
          proxyHeadersRuntime,
          hostResponseRuntime,
          authConfigResult,
          reverseProxyThrottleResult,
        ] = await Promise.all([
          configManager.getGatewayVisibilityRuntimeState(),
          configManager.getGatewayProxyHeadersRuntimeState(),
          configManager.getGatewayHostResponseRuntimeState(),
          goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig)),
          goBackend.setReverseProxyThrottle(
            updatedConfig.reverse_proxy_throttle ??
              DEFAULT_REVERSE_PROXY_THROTTLE_CONFIG,
          ),
        ]);

        const syncErrors: string[] = [];
        if (!authConfigResult.success) {
          syncErrors.push(
            authConfigResult.message || "同步鉴权缓存配置到网关失败",
          );
        }
        if (!reverseProxyThrottleResult.success) {
          syncErrors.push(
            reverseProxyThrottleResult.message || "同步网关节流配置到网关失败",
          );
        }
        if (syncErrors.length > 0) {
          throw new Error(syncErrors.join("；"));
        }

        try {
          await syncReverseProxyTrustedIPsNow({
            config: updatedConfig,
          });
        } catch (error) {
          console.error(
            "[reverse-proxy-trusted-ips] failed to sync after gateway config update:",
            error,
          );
          throw error;
        }

        await syncGatewayPortalToGateway(
          updatedConfig.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
        );
        await applyGatewayPortalTitleHostRulesPatchIfNeeded(updatedConfig);

        return {
          success: true,
          data: buildGatewaySettingsResponse(
            updatedConfig,
            visibilityRuntime,
            proxyHeadersRuntime,
            hostResponseRuntime,
          ),
        };
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig);
        let portalRollbackError: string | null = null;
        try {
          await syncGatewayPortalToGateway(
            previousConfig.gateway_portal ?? DEFAULT_GATEWAY_PORTAL_CONFIG,
          );
        } catch (innerError: any) {
          portalRollbackError =
            innerError?.message || "恢复传送门显示运行态失败";
        }
        set.status = 502;
        const extraRollbackError = [rollbackError, portalRollbackError]
          .filter(Boolean)
          .join("；");
        return {
          success: false,
          message: extraRollbackError
            ? `${error?.message || "更新网关配置失败"}；回滚失败：${extraRollbackError}`
            : error?.message || "更新网关配置失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新网关配置", {
      body: t.Object({
        auth_cache_ttl_seconds: t.Optional(t.Number()),
        auth_cache_unauthorized_ttl_seconds: t.Optional(t.Number()),
        reverse_proxy_throttle: t.Optional(
          t.Object({
            enabled: t.Optional(t.Boolean()),
            requests_per_second: t.Optional(t.Number()),
            burst: t.Optional(t.Number()),
            block_seconds: t.Optional(t.Number()),
          }),
        ),
        portal: t.Optional(
          t.Object({
            display_style: t.Optional(
              t.Union([t.Literal("domain"), t.Literal("title")]),
            ),
          }),
        ),
      }),
    }),
  )
  .get(
    "/config/gateway/visibility",
    async () => {
      const details = await getGatewayVisibilityDetails();
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关可见性配置"),
  )
  .post(
    "/config/gateway/visibility",
    async ({ body, set }) => {
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayVisibilityRuntimeState(),
      ]);

      try {
        const compiled = await compileGatewayVisibilityConfig({
          enabled: body.enabled,
          selections: body.selections,
          custom_cidrs: body.custom_cidrs,
        });

        const [savedConfig, savedRuntime] = await Promise.all([
          configManager.updateGatewayVisibilityConfig(compiled.config),
          configManager.saveGatewayVisibilityRuntimeState(compiled.runtime),
        ]);

        await syncGatewayVisibilityToGateway(savedRuntime);

        return {
          success: true,
          data: {
            config: savedConfig,
            summary: buildGatewayVisibilitySummary(savedConfig, savedRuntime),
          },
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayVisibilityConfigAndRuntime(
          previousConfig,
          previousRuntime,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新网关可见性失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新网关可见性失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新网关可见性配置", {
      body: t.Object({
        enabled: t.Boolean(),
        selections: t.Array(
          t.Object({
            province: t.String(),
            query_city: t.Optional(t.Union([t.String(), t.Null()])),
          }),
        ),
        custom_cidrs: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/config/gateway/proxy-headers",
    async () => {
      const details = await getGatewayProxyHeadersDetails();
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关代理请求头配置"),
  )
  .post(
    "/config/gateway/proxy-headers",
    async ({ body, set }) => {
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayProxyHeadersRuntimeState(),
      ]);

      if (!isAnySubdomainRoutingMode(previousConfig)) {
        set.status = 400;
        return {
          success: false,
          message: "协议头仅可在子域映射模式下编辑",
        };
      }

      try {
        const compiled = compileGatewayProxyHeadersState(previousConfig, {
          disabled_hosts: body.disabled_hosts,
        });

        await Promise.all([
          configManager.updateGatewayProxyHeadersConfig(compiled.config),
          configManager.saveGatewayProxyHeadersRuntimeState(compiled.runtime),
        ]);

        await syncGatewayProxyHeadersToGateway(compiled.runtime);

        return {
          success: true,
          data: await getGatewayProxyHeadersDetails(),
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayProxyHeadersConfigAndRuntime(
          previousConfig,
          previousRuntime,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新网关协议头失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新网关协议头失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新网关代理请求头配置", {
      body: t.Object({
        disabled_hosts: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/config/gateway/host-response",
    async () => {
      const details = await getGatewayHostResponseDetails();
      return {
        success: true,
        data: details,
      };
    },
    routeDoc("获取网关 Host 响应配置"),
  )
  .post(
    "/config/gateway/host-response",
    async ({ body, set }) => {
      const [previousConfig, previousRuntime] = await Promise.all([
        configManager.getConfig(),
        configManager.getGatewayHostResponseRuntimeState(),
      ]);

      if (!isAnySubdomainRoutingMode(previousConfig)) {
        set.status = 400;
        return {
          success: false,
          message: "Host 响应仅可在子域映射模式下编辑",
        };
      }

      try {
        await syncGatewayHostResponseRuntimeForConfig(
          {
            run_type: previousConfig.run_type,
            reverse_proxy_submode: previousConfig.reverse_proxy_submode,
            host_mappings: previousConfig.host_mappings,
            gateway_host_response: {
              disabled_hosts: body.disabled_hosts,
            },
          },
          {
            saveConfig: true,
          },
        );

        return {
          success: true,
          data: await getGatewayHostResponseDetails(),
        };
      } catch (error: any) {
        const rollbackError = await rollbackGatewayHostResponseConfigAndRuntime(
          previousConfig,
          previousRuntime,
        );
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新网关 Host 响应失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新网关 Host 响应失败，已回滚配置",
        };
      }
    },
    withRouteDoc("更新网关 Host 响应配置", {
      body: t.Object({
        disabled_hosts: t.Array(t.String()),
      }),
    }),
  )
  .get(
    "/config/captcha",
    async () => {
      const settings = await configManager.getCaptchaSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取验证码配置"),
  )
  .post(
    "/config/captcha",
    async ({ body, set }) => {
      if (body.provider === "turnstile") {
        const siteKey = body.turnstile?.site_key?.trim() || "";
        const secretKey = body.turnstile?.secret_key?.trim() || "";
        if (!siteKey || !secretKey) {
          set.status = 400;
          return {
            success: false,
            message:
              "启用 Cloudflare Turnstile 时，site_key 和 secret_key 都必须填写",
          };
        }
      }

      const next = await configManager.updateCaptchaSettings({
        provider: body.provider,
        turnstile: body.turnstile,
      });
      return { success: true, data: next };
    },
    withRouteDoc("更新验证码配置", {
      body: t.Object({
        provider: t.Union([t.Literal("pow"), t.Literal("turnstile")]),
        turnstile: t.Object({
          site_key: t.String(),
          secret_key: t.String(),
        }),
      }),
    }),
  )
  .get(
    "/config/ip_location_api",
    async () => {
      const settings = await configManager.getIpLocationApiSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取 IP 属地 API 配置"),
  )
  .post(
    "/config/ip_location_api",
    async ({ body, set }) => {
      const ipLookupMode: IpLocationApiMode = body.ip_lookup_mode;
      const cidrMode: IpLocationApiMode = body.cidr_mode;
      const ipLookupUrl =
        ipLookupMode === "custom"
          ? validateIpLocationBaseUrl(body.ip_lookup_url, "IP 识别库地址")
          : {
              valid: true as const,
              url: DEFAULT_IP_LOCATION_API_CONFIG.ip_lookup_url,
            };
      const cidrUrl =
        cidrMode === "custom"
          ? validateIpLocationBaseUrl(body.cidr_url, "CIDR 地址库地址")
          : {
              valid: true as const,
              url: DEFAULT_IP_LOCATION_API_CONFIG.cidr_url,
            };

      if (!ipLookupUrl.valid) {
        set.status = 400;
        return { success: false, message: ipLookupUrl.message };
      }
      if (!cidrUrl.valid) {
        set.status = 400;
        return { success: false, message: cidrUrl.message };
      }

      const next = await configManager.updateIpLocationApiSettings({
        ip_lookup_mode: ipLookupMode,
        ip_lookup_url: ipLookupUrl.url,
        cidr_mode: cidrMode,
        cidr_url: cidrUrl.url,
      });
      return { success: true, data: next };
    },
    withRouteDoc("更新 IP 属地 API 配置", {
      body: t.Object({
        ip_lookup_mode: t.Union([t.Literal("online"), t.Literal("custom")]),
        ip_lookup_url: t.String(),
        cidr_mode: t.Union([t.Literal("online"), t.Literal("custom")]),
        cidr_url: t.String(),
      }),
    }),
  )
  .post(
    "/config/ip_location_api/test-ip-lookup",
    async ({ body, set }) => {
      const validation = validateIpLocationBaseUrl(body.url, "URL");
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.message };
      }

      const timeoutMs = 5000;

      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const url = buildIpLocationApiUrl(validation.url, "ip/lookup");
        url.searchParams.set("ip", "8.8.8.8");

        const response = await fetch(url, {
          signal: controller.signal,
          headers: { "User-Agent": "fn-knock-server-admin/1.0" },
        });

        clearTimeout(timer);

        if (!response.ok) {
          return {
            success: false,
            message: `服务返回错误状态码 ${response.status}`,
          };
        }

        const data = (await response.json().catch(() => null)) as {
          code?: number;
          result?: unknown;
          msg?: string;
        } | null;
        if (!data || data.code !== 0 || !data.result) {
          return {
            success: false,
            message: data?.msg || "服务返回数据异常",
          };
        }

        return { success: true, message: "连接成功" };
      } catch (error: any) {
        if (error?.name === "AbortError") {
          return { success: false, message: "连接超时" };
        }
        return {
          success: false,
          message: error?.message || "连接失败",
        };
      }
    },
    withRouteDoc("测试 IP 识别库连接", {
      body: t.Object({
        url: t.String(),
      }),
    }),
  )
  .post(
    "/config/ip_location_api/test-cidr",
    async ({ body, set }) => {
      const validation = validateIpLocationBaseUrl(body.url, "URL");
      if (!validation.valid) {
        set.status = 400;
        return { success: false, message: validation.message };
      }

      const timeoutMs = 5000;

      try {
        const controller = new AbortController();
        const timer = setTimeout(() => controller.abort(), timeoutMs);
        const url = buildIpLocationApiUrl(validation.url, "provinces");

        const response = await fetch(url, {
          signal: controller.signal,
          headers: { "User-Agent": "fn-knock-server-admin/1.0" },
        });

        clearTimeout(timer);

        if (!response.ok) {
          return {
            success: false,
            message: `服务返回错误状态码 ${response.status}`,
          };
        }

        const data = (await response.json().catch(() => null)) as {
          code?: number;
          data?: unknown;
          message?: string;
        } | null;
        if (!data || data.code !== 0 || !data.data) {
          return {
            success: false,
            message: data?.message || "服务返回数据异常",
          };
        }

        return { success: true, message: "连接成功" };
      } catch (error: any) {
        if (error?.name === "AbortError") {
          return { success: false, message: "连接超时" };
        }
        return {
          success: false,
          message: error?.message || "连接失败",
        };
      }
    },
    withRouteDoc("测试 CIDR 库连接", {
      body: t.Object({
        url: t.String(),
      }),
    }),
  )
  .get(
    "/config/terminal_feature",
    async () => {
      const settings = await configManager.getTerminalFeatureConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取终端功能配置"),
  )
  .get(
    "/config/auth_credential_settings",
    async () => {
      const settings = await configManager.getAuthCredentialSettings();
      return { success: true, data: settings };
    },
    routeDoc("获取认证凭据配置"),
  )
  .get(
    "/config/dashboard_display",
    async () => {
      const settings = await configManager.getDashboardDisplayConfig();
      return { success: true, data: settings };
    },
    routeDoc("获取首页展示配置"),
  )
  .get(
    "/config/auto_https",
    async () => {
      const details = await buildAutoHttpsDetails();
      return { success: true, data: details };
    },
    routeDoc("获取自动 HTTPS 配置"),
  )
  .post(
    "/config/auth_credential_settings",
    async ({ body }) => {
      const previous = await configManager.getAuthCredentialSettings();
      const next = await configManager.previewAuthCredentialSettingsUpdate(body);
      const sessionIpMobilityChanged =
        previous.session_ip_mobility_enabled !==
          next.session_ip_mobility_enabled ||
        previous.session_ip_mobility_window_seconds !==
          next.session_ip_mobility_window_seconds;
      if (sessionIpMobilityChanged) {
        await authMobilitySessionManager.reconcileSessionIpMobilityPolicy(
          previous,
          next,
          { scheduleSync: false },
        );
      }
      let saved = next;
      try {
        saved = await configManager.updateAuthCredentialSettings(body);
      } catch (error) {
        if (sessionIpMobilityChanged) {
          try {
            await authMobilitySessionManager.reconcileSessionIpMobilityPolicy(
              next,
              previous,
              { scheduleSync: false },
            );
            scheduleSyncReverseProxyTrustedIPs({
              reason: "session-ip-mobility-config-rollback",
              delayMs: 0,
            });
          } catch (rollbackError) {
            console.error(
              "[auth-mobility] failed to rollback session IP mobility reconciliation after config save failure:",
              rollbackError,
            );
          }
        }
        throw error;
      }
      if (sessionIpMobilityChanged) {
        scheduleSyncReverseProxyTrustedIPs({
          reason: "session-ip-mobility-config-updated",
          delayMs: 0,
        });
      }
      return { success: true, data: saved };
    },
    withRouteDoc("更新认证凭据配置", {
      body: t.Object({
        session_ttl_seconds: t.Optional(t.Number()),
        remember_me_ttl_seconds: t.Optional(t.Number()),
        post_login_ip_grant_mode: t.Optional(
          t.Union([
            t.Literal("follow_session"),
            t.Literal("disabled"),
            t.Literal("custom"),
          ]),
        ),
        post_login_ip_grant_ttl_seconds: t.Optional(
          t.Union([t.Number(), t.Null()]),
        ),
        session_ip_mobility_enabled: t.Optional(t.Boolean()),
        session_ip_mobility_window_seconds: t.Optional(t.Number()),
        passkey_bind_prompt_enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/config/dashboard_display",
    async ({ body }) => {
      const next = await configManager.updateDashboardDisplayConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新首页展示配置", {
      body: t.Object({
        show_entry_status_module: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/config/auto_https",
    async ({ body, set }) => {
      if (body.enabled === true && getRuntimeProfile().is_docker) {
        set.status = 403;
        return {
          success: false,
          message: "Docker 版本不支持自动 HTTPS",
        };
      }

      if (body.enabled === true) {
        const runtime = await autoHttpsRedirectManager.applyConfig({
          enabled: true,
        });
        const next = await configManager.updateAutoHttpsConfig({
          enabled: runtime.status === "active",
        });
        return {
          success: true,
          data: {
            ...next,
            runtime,
          },
          message:
            runtime.status === "error"
              ? runtime.last_error || "自动 HTTPS 启动失败"
              : undefined,
        };
      }

      const next = await configManager.updateAutoHttpsConfig(body);
      const runtime = await autoHttpsRedirectManager.applyConfig(next);
      return {
        success: true,
        data: {
          ...next,
          runtime,
        },
        message:
          runtime.status === "error"
            ? runtime.last_error || "自动 HTTPS 启动失败"
            : undefined,
      };
    },
    withRouteDoc("更新自动 HTTPS 配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
      }),
    }),
  )
  .post(
    "/config/terminal_feature",
    async ({ body }) => {
      const next = await configManager.updateTerminalFeatureConfig(body);
      return { success: true, data: next };
    },
    withRouteDoc("更新终端功能配置", {
      body: t.Object({
        enabled: t.Optional(t.Boolean()),
        default_cwd: t.Optional(t.String()),
        max_sessions: t.Optional(t.Number()),
        idle_timeout_seconds: t.Optional(t.Number()),
        resume_backend: t.Optional(t.Literal("tmux")),
        allow_mobile_toolbar: t.Optional(t.Boolean()),
        dangerously_run_as_current_user: t.Optional(t.Boolean()),
      }),
    }),
  )
  .get(
    "/config/default_route",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: { default_route: config.default_route } };
    },
    routeDoc("获取默认路由"),
  )
  .post(
    "/config/default_route",
    async ({ body }) => {
      await configManager.updateDefaultRoute(body.path);
      await goBackend.setDefaultRoute(body.path);
      return { success: true };
    },
    withRouteDoc("更新默认路由", {
      body: t.Object({
        path: t.String(),
      }),
    }),
  )
  .post(
    "/config/default_tunnel",
    async ({ body }) => {
      await configManager.updateDefaultTunnel(body.tunnel);
      return { success: true };
    },
    withRouteDoc("设置默认隧道类型", {
      body: t.Object({
        tunnel: t.Union([t.Literal("frp"), t.Literal("cloudflared")]),
      }),
    }),
  )
  .post(
    "/config/proxy_mappings",
    async ({ body }) => {
      await configManager.updateProxyMappings(body.mappings);
      await goBackend.setRules(body.mappings);
      return { success: true };
    },
    withRouteDoc("更新路径代理映射", {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            path: t.String(),
            target: t.String(),
            rewrite_html: t.Boolean(),
            use_auth: t.Boolean(),
            use_root_mode: t.Boolean(),
            strip_path: t.Boolean(),
          }),
        ),
      }),
    }),
  )
  .get(
    "/config/host_mappings",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.host_mappings };
    },
    routeDoc("获取 Host 映射列表"),
  )
  .post(
    "/config/host_mappings",
    async ({ body, set }) => {
      const validation = validateHostMappings(body.mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const config = await configManager.getConfig();
      const previousByHost = new Map(
        config.host_mappings.map((mapping) => [
          normalizeHostMappingLookupKey(mapping.host),
          mapping,
        ]),
      );
      const normalizedMappings: HostMapping[] = body.mappings.map((mapping) => {
        const previous = previousByHost.get(
          normalizeHostMappingLookupKey(mapping.host),
        );
        const normalizedTarget = mapping.target.trim();
        const serviceRole = isAuthServiceTarget(normalizedTarget)
          ? "auth"
          : "app";
        const canReusePreviousMetadata =
          previous?.target.trim() === normalizedTarget;
        const normalizedBasicAuth =
          serviceRole === "auth"
            ? createDisabledHostBasicAuth()
            : normalizeHostBasicAuth(
                mapping.basic_auth ?? previous?.basic_auth,
              );
        const normalizedLocations =
          serviceRole === "auth"
            ? []
            : normalizeHostMappingLocationsForRoute(mapping.locations);

        return {
          ...mapping,
          target: normalizedTarget,
          service_role: serviceRole,
          basic_auth: normalizedBasicAuth,
          locations: normalizedLocations,
          title:
            typeof mapping.title === "string"
              ? mapping.title.trim()
              : canReusePreviousMetadata
                ? (previous?.title ?? "")
                : "",
          title_override:
            typeof mapping.title_override === "string"
              ? mapping.title_override.trim()
              : (previous?.title_override ?? ""),
          favicon:
            typeof mapping.favicon === "string"
              ? mapping.favicon.trim()
              : canReusePreviousMetadata
                ? (previous?.favicon ?? "")
                : "",
        };
      });
      const nextConfig = {
        ...config,
        host_mappings: normalizedMappings,
      };
      const passkeyValidation = validatePasskeyRpConfig(nextConfig);
      if (!passkeyValidation.valid) {
        set.status = 400;
        return {
          success: false,
          message: passkeyValidation.message,
        };
      }

      const updatedConfig = {
        ...config,
        host_mappings: normalizedMappings,
      };

      try {
        await configManager.saveConfig(updatedConfig);
        await syncHostMappingsRuntime(
          config,
          updatedConfig,
          normalizedMappings,
        );
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(config);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "更新 Host 映射失败"}；回滚失败：${rollbackError}`
            : error?.message || "更新 Host 映射失败，已回滚配置",
        };
      }

      scheduleHostMappingsMetadataRefresh(
        normalizedMappings,
        config.host_mappings,
      );
      scheduleSmartConnectSyncAfterHostMappingsChange(updatedConfig);

      return { success: true, data: normalizedMappings };
    },
    withRouteDoc("更新 Host 映射列表", {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            host: t.String(),
            target: t.String(),
            use_auth: t.Boolean(),
            access_mode: t.Union([
              t.Literal("login_first"),
              t.Literal("strict_whitelist"),
            ]),
            suppress_toolbar: t.Boolean(),
            preserve_host: t.Boolean(),
            basic_auth: t.Optional(
              t.Object({
                enabled: t.Boolean(),
                username: t.String(),
                password: t.String(),
              }),
            ),
            locations: t.Optional(
              t.Array(
                t.Object({
                  path: t.String(),
                  match: t.Union([t.Literal("exact"), t.Literal("prefix")]),
                  action: t.Union([t.Literal("proxy"), t.Literal("response")]),
                  target: t.Optional(t.String()),
                  strip_path: t.Optional(t.Boolean()),
                  rewrite_html: t.Optional(t.Boolean()),
                  response: t.Optional(
                    t.Object({
                      status: t.Optional(t.Number()),
                      content_type: t.Optional(t.String()),
                      headers: t.Optional(t.Record(t.String(), t.String())),
                      body: t.Optional(t.String()),
                    }),
                  ),
                }),
              ),
            ),
            service_role: t.Optional(
              t.Union([t.Literal("app"), t.Literal("auth")]),
            ),
            title: t.Optional(t.String()),
            title_override: t.Optional(t.String()),
            favicon: t.Optional(t.String()),
          }),
        ),
      }),
    }),
  )
  .post(
    "/config/host_mappings/basic_auth_probe",
    async ({ body }) => {
      const result = await probeBasicAuthTarget(body.target);
      return {
        success: true,
        data: result,
      };
    },
    withRouteDoc("探测目标 Basic Auth", {
      body: t.Object({
        target: t.String(),
      }),
    }),
  )
  .post(
    "/config/host_mappings/metadata",
    async ({ body, set }) => {
      const metadata = await fetchUrlMetadata(body.target, {
        basicAuth: normalizeHostBasicAuth(body.basic_auth),
      });
      if (!metadata.ok) {
        set.status = 400;
        return {
          success: false,
          message: metadata.error || "目标地址标题刷新失败",
        };
      }

      return {
        success: true,
        data: metadata.data,
      };
    },
    withRouteDoc("抓取目标地址元数据", {
      body: t.Object({
        target: t.String(),
        basic_auth: t.Optional(
          t.Object({
            enabled: t.Boolean(),
            username: t.String(),
            password: t.String(),
          }),
        ),
      }),
    }),
  )
  .post(
    "/config/host_mappings/refresh_titles",
    async () => {
      const config = await configManager.getConfig();
      const { mappings, summary } = await refreshAllHostMappingTitles(
        config.host_mappings,
      );

      await configManager.updateHostMappings(mappings);
      await syncGatewayPortalHostRulesIfTitleMode({
        run_type: config.run_type,
        reverse_proxy_submode: config.reverse_proxy_submode,
        gateway_portal: config.gateway_portal,
        host_mappings: mappings,
      });

      return {
        success: true,
        data: summary,
      };
    },
    routeDoc("批量刷新 Host 映射标题"),
  )
  .get(
    "/config/host_mappings/bookmarks/export",
    async () => {
      const config = await configManager.getConfig();
      const scheme = resolveBookmarkScheme(config);
      const accessEntryPort =
        resolvePublicPortForScheme(
          config,
          scheme,
          config.subdomain_mode?.public_auth_base_url || "",
        ) ?? null;
      const document = buildHostMappingsBookmarksDocument({
        mappings: config.host_mappings,
        scheme,
        accessEntryPort: accessEntryPort ?? resolveAccessEntryInfo(config).port,
        omitAccessEntryPort:
          shouldOmitPublicAccessEntryPort(config) && accessEntryPort === null,
        folderTitle: config.subdomain_mode?.root_domain?.trim()
          ? `${config.subdomain_mode.root_domain.trim()} 子域映射`
          : "fn-knock 子域映射",
      });
      const filename = buildHostMappingsBookmarkFilename(
        config.subdomain_mode?.root_domain,
      );
      const body = new Blob([document], {
        type: "text/html;charset=UTF-8",
      });

      return new Response(body, {
        headers: {
          "Content-Type": "text/html; charset=UTF-8",
          "Content-Disposition": `attachment; filename="${filename}"`,
          "Cache-Control": "no-store",
        },
      });
    },
    routeDoc("导出 Host 映射书签"),
  )
  .get(
    "/config/stream_mappings",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.stream_mappings };
    },
    routeDoc("获取协议映射列表"),
  )
  .post(
    "/config/stream_mappings",
    async ({ body, set }) => {
      const validation = validateStreamMappings(body.mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const previousConfig = await configManager.getConfig();
      await configManager.updateStreamMappings(body.mappings);
      const updatedConfig = await configManager.getConfig();
      try {
        await firewallService.applyRunTypeConfig(
          updatedConfig.run_type,
          updatedConfig.run_type,
        );
      } catch (error: any) {
        const rollbackError = await rollbackConfigAndRuntime(previousConfig);
        set.status = 502;
        return {
          success: false,
          message: rollbackError
            ? `${error?.message || "同步 协议映射与网关端口放行规则失败"}；回滚失败：${rollbackError}`
            : error?.message ||
              "同步 协议映射与网关端口放行规则失败，已回滚配置",
        };
      }

      return { success: true };
    },
    withRouteDoc("更新协议映射列表", {
      body: t.Object({
        mappings: t.Array(
          t.Object({
            protocol: t.Optional(t.Union([t.Literal("tcp"), t.Literal("udp")])),
            listen_port: t.Number(),
            target: t.String(),
            use_auth: t.Boolean(),
          }),
        ),
      }),
    }),
  )
  .get(
    "/config/subdomain_mode",
    async () => {
      const config = await configManager.getConfig();
      return { success: true, data: config.subdomain_mode };
    },
    routeDoc("获取子域模式配置"),
  )
  .post(
    "/config/subdomain_mode",
    async ({ body, set }) => {
      const config = await configManager.getConfig();
      const nextConfig = {
        ...config,
        subdomain_mode: {
          ...config.subdomain_mode,
          ...body,
        },
      };
      const validation = validateHostMappings(nextConfig.host_mappings);
      if (!validation.valid) {
        set.status = 400;
        return {
          success: false,
          message: validation.message,
        };
      }

      const passkeyValidation = validatePasskeyRpConfig(nextConfig);
      if (!passkeyValidation.valid) {
        set.status = 400;
        return {
          success: false,
          message: passkeyValidation.message,
        };
      }

      const next = await configManager.updateSubdomainModeConfig(body);
      const updatedConfig = await configManager.getConfig();
      await goBackend.setAuthConfig(buildGatewayAuthConfig(updatedConfig));

      const sslStatus = await configManager.getSSLStatus();
      const inventoryCoverage = buildSubdomainCertificateInventoryCoverage({
        config: updatedConfig,
        certificates: sslStatus.certificates.map((certificate) => ({
          id: certificate.id,
          certificateDomains: certificate.certInfo?.dnsNames || [],
        })),
        activeCertificateId: sslStatus.activeCertId,
        deploymentMode: sslStatus.deploymentMode,
      });

      let sslAutoSelection: {
        applied: boolean;
        certificate_id?: string;
        label?: string;
        message: string;
      } | null = null;

      if (
        inventoryCoverage.can_auto_activate &&
        inventoryCoverage.suggested_certificate_id
      ) {
        const previousActiveId = sslStatus.activeCertId || null;
        const candidate = await configManager.activateSSLCertificate(
          inventoryCoverage.suggested_certificate_id,
        );

        if (candidate) {
          try {
            await syncSSLDeploymentToGateway();
            sslAutoSelection = {
              applied: true,
              certificate_id: candidate.id,
              label: candidate.label,
              message: "已自动切换到更适合当前子域模式的证书。",
            };
          } catch (error: any) {
            await configManager.activateSSLCertificate(previousActiveId);
            await syncSSLDeploymentToGateway().catch(() => undefined);

            sslAutoSelection = {
              applied: false,
              certificate_id: candidate.id,
              label: candidate.label,
              message:
                error?.message ||
                "已找到推荐证书，但同步到网关失败，未自动切换。",
            };
          }
        }
      }

      return {
        success: true,
        data: {
          ...next,
          ssl_auto_selection: sslAutoSelection,
        },
      };
    },
    withRouteDoc("更新子域模式配置", {
      body: t.Object({
        root_domain: t.Optional(t.String()),
        auth_host: t.Optional(t.String()),
        auth_target: t.Optional(t.String()),
        cookie_domain: t.Optional(t.String()),
        edge_client_ip_enabled: t.Optional(t.Boolean()),
        aliyun_esa_enabled: t.Optional(t.Boolean()),
        tencent_edgeone_enabled: t.Optional(t.Boolean()),
        public_auth_base_url: t.Optional(t.String()),
        public_http_port: t.Optional(t.Number()),
        public_https_port: t.Optional(t.Number()),
        auth_cache_ttl_seconds: t.Optional(t.Number()),
        auth_cache_unauthorized_ttl_seconds: t.Optional(t.Number()),
        default_access_mode: t.Optional(
          t.Union([t.Literal("login_first"), t.Literal("strict_whitelist")]),
        ),
        auto_add_whitelist_on_login: t.Optional(t.Boolean()),
        passkey_rp_mode: t.Optional(
          t.Union([t.Literal("auth_host"), t.Literal("parent_domain")]),
        ),
        passkey_rp_id: t.Optional(t.String()),
      }),
    }),
  )
  // TOTP 认证管理
  .get(
    "/totp/status",
    async () => {
      const credentials = await configManager.getTOTPCredentials();
      return {
        success: true,
        data: { bound: credentials.length > 0, credentials },
      };
    },
    routeDoc("获取 TOTP 绑定状态"),
  )
  .post(
    "/totp/setup",
    async () => {
      const secret = generateSecret();
      const uri = generateURI({
        issuer: "fn-knock",
        label: "admin",
        secret,
        strategy: "totp",
      });
      return { success: true, data: { secret, uri } };
    },
    routeDoc("生成 TOTP 绑定信息"),
  )
  .post(
    "/totp/bind",
    async ({ body, set }) => {
      const { valid } = verifySync({
        strategy: "totp",
        token: body.token,
        secret: body.secret,
      });
      if (!valid) {
        set.status = 400;
        return { success: false, message: "验证码不正确，请重试" };
      }
      await configManager.addTOTPCredential({
        id: randomBytes(8).toString("hex"),
        secret: body.secret,
        comment: body.comment || "New Token",
        createdAt: new Date().toISOString(),
      });
      return { success: true };
    },
    withRouteDoc("绑定 TOTP 凭据", {
      body: t.Object({
        secret: t.String(),
        token: t.String(),
        comment: t.Optional(t.String()),
      }),
    }),
  )
  .delete(
    "/totp/:id",
    async ({ params, set }) => {
      const deleted = await configManager.deleteTOTPCredential(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: "TOTP not found" };
      }
      await oidcAuthService.deleteBindingsByTotp(params.id);
      return { success: true };
    },
    withRouteDoc("删除 TOTP 凭据", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .patch(
    "/totp/:id/comment",
    async ({ params, body, set }) => {
      const updated = await configManager.updateTOTPCredential(
        params.id,
        body.comment,
      );
      if (!updated) {
        set.status = 404;
        return { success: false, message: "TOTP not found" };
      }
      return { success: true };
    },
    withRouteDoc("更新 TOTP 凭据备注", {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    }),
  )
  .get(
    "/totp/:totpId/passkeys",
    async ({ params }) => {
      const passkeys = await configManager.getPasskeys();
      const filtered = passkeys.filter((pk) => pk.totpId === params.totpId);
      return { success: true, data: filtered };
    },
    withRouteDoc("获取 TOTP 关联的 Passkey 列表", {
      params: t.Object({ totpId: t.String() }),
    }),
  )
  .delete(
    "/passkeys/:id",
    async ({ params, set }) => {
      const deleted = await configManager.deletePasskey(params.id);
      if (!deleted) {
        set.status = 404;
        return { success: false, message: "Passkey not found" };
      }
      return { success: true };
    },
    withRouteDoc("删除 Passkey", {
      params: t.Object({
        id: t.String(),
      }),
    }),
  )
  .post(
    "/sync-routes",
    async ({ set }) => {
      try {
        const [config, protocolMappingFeature] = await Promise.all([
          configManager.getConfig(),
          configManager.getProtocolMappingFeatureConfig(),
        ]);
        await firewallService.applyRunTypeConfig(
          config.run_type,
          config.run_type,
        );

        const loggingResult = await goBackend.setGatewayLoggingConfig(
          config.gateway_logging ?? {
            enabled: false,
            max_days: 7,
          },
        );
        if (!loggingResult.success) {
          set.status = 502;
          return {
            success: false,
            message: `同步部分失败: gateway_logging=${loggingResult.success}`,
          };
        }

        let syncedWAF = true;
        try {
          await syncWAFConfigToGateway(config.waf ?? null);
        } catch (error) {
          syncedWAF = false;
          set.status = 502;
          return {
            success: false,
            message: `同步部分失败: gateway_logging=${loggingResult.success}, waf=${syncedWAF}`,
          };
        }

        const syncedRules =
          config.run_type === 1 && !isReverseProxySubdomainMode(config)
            ? config.proxy_mappings.length
            : 0;
        const syncedHostRules = isAnySubdomainRoutingMode(config)
          ? config.host_mappings.length
          : 0;
        const syncedStreamRules =
          config.run_type === 3 && protocolMappingFeature.enabled === true
            ? config.stream_mappings.length
            : 0;

        return {
          success: true,
          data: {
            synced_rules: syncedRules,
            synced_host_rules: syncedHostRules,
            synced_stream_rules: syncedStreamRules,
            synced_gateway_logging: true,
            synced_waf: syncedWAF,
            waf_bundle_id: config.waf?.active_bundle_id || "",
          },
          message: `已按当前运行模式同步 ${syncedRules} 条路径路由、${syncedHostRules} 条 Host 路由、${syncedStreamRules} 条 协议映射、请求日志配置与 WAF 配置`,
        };
      } catch (e: any) {
        set.status = 500;
        return { success: false, message: e?.message ?? String(e) };
      }
    },
    routeDoc("按当前配置同步路由与网关"),
  )
  .get(
    "/maintenance/backup/export",
    async () => {
      const archive = await maintenanceBackupService.exportBackupArchive();
      const body = new Blob([Uint8Array.from(archive.buffer)], {
        type: "application/octet-stream",
      });

      return new Response(body, {
        headers: {
          "Content-Type": "application/octet-stream",
          "Content-Disposition": `attachment; filename="${archive.filename}"`,
          "Cache-Control": "no-store",
        },
      });
    },
    routeDoc("导出系统备份归档"),
  )
  .get(
    "/maintenance/backup/files",
    async ({ set }) => {
      try {
        return {
          success: true,
          data: await maintenanceBackupService.listBackupDirectoryFiles(),
        };
      } catch (error: any) {
        set.status = 500;
        return {
          success: false,
          message: error?.message || "读取飞牛备份目录失败",
        };
      }
    },
    routeDoc("获取飞牛备份目录文件列表"),
  )
  .post(
    "/maintenance/backup/export/fnos",
    async ({ set }) => {
      try {
        const result =
          await maintenanceBackupService.exportBackupArchiveToDirectory();
        return {
          success: true,
          data: result,
          message: "备份已导出到飞牛目录",
        };
      } catch (error: any) {
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status = status;
        return {
          success: false,
          message: error?.message || "导出到飞牛目录失败",
        };
      }
    },
    routeDoc("导出备份到飞牛目录"),
  )
  .post(
    "/maintenance/backup/import",
    async ({ body, set }) => {
      try {
        const result = await maintenanceBackupService.importBackupArchive(body);
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? "备份已导入，但部分运行态同步失败"
              : "备份已导入并完成运行态同步",
        };
      } catch (error: any) {
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status = status;
        return {
          success: false,
          message: error?.message || "导入备份失败",
        };
      }
    },
    withRouteDoc("导入本地备份归档", {
      body: t.Object({
        filename: t.Optional(t.String()),
        archive_base64: t.String(),
      }),
    }),
  )
  .post(
    "/maintenance/backup/import/fnos",
    async ({ body, set }) => {
      try {
        const result =
          await maintenanceBackupService.importBackupArchiveFromDirectory(
            body.path,
          );
        return {
          success: true,
          data: result,
          message:
            result.warnings.length > 0
              ? "飞牛备份已导入，但部分运行态同步失败"
              : "飞牛备份已导入并完成运行态同步",
        };
      } catch (error: any) {
        const message = error?.message || "从飞牛导入备份失败";
        const status =
          error instanceof MaintenanceBackupError ? error.status : 500;
        set.status =
          error?.code === "ENOENT"
            ? 404
            : error?.code === "EACCES"
              ? 403
              : status;
        return {
          success: false,
          message,
        };
      }
    },
    withRouteDoc("从飞牛目录导入备份", {
      body: t.Object({
        path: t.String(),
      }),
    }),
  )
  .get(
    "/security/overview",
    async ({ query }) => {
      const rangeSec = clamp(
        parseIntSafe(query.rangeSec, 3600),
        60,
        30 * 24 * 3600,
      );
      const nowMs = Date.now();
      const fromMs = nowMs - rangeSec * 1000;
      const bucketCount = Math.min(
        48,
        Math.max(12, Math.round(rangeSec / 900)),
      );
      const events = await systemEventManager.listByRange({
        fromMs,
        toMs: nowMs,
        types: [FN_EVENT_AUTH_LOGIN_FAILURE, FN_EVENT_SECURITY_SCANNER_BLOCKED],
      });
      const wafTimestamps = await wafLogStore.listTimestampsByRange({
        fromMs,
        toMs: nowMs,
      });
      const failedTimestamps = events
        .filter((item) => item.event.type === FN_EVENT_AUTH_LOGIN_FAILURE)
        .map((item) => item.timestamp);
      const blockedTimestamps = events
        .filter((item) => item.event.type === FN_EVENT_SECURITY_SCANNER_BLOCKED)
        .map((item) => item.timestamp);

      return {
        success: true,
        data: {
          rangeSec,
          totals: {
            failedLogins: failedTimestamps.length,
            blockedScanners: blockedTimestamps.length,
            wafEvents: wafTimestamps.length,
          },
          series: {
            failedLogins: buildCountSeries(
              failedTimestamps,
              fromMs,
              nowMs,
              bucketCount,
            ),
            blockedScanners: buildCountSeries(
              blockedTimestamps,
              fromMs,
              nowMs,
              bucketCount,
            ),
            wafEvents: buildCountSeries(
              wafTimestamps,
              fromMs,
              nowMs,
              bucketCount,
            ),
          },
        },
      };
    },
    withRouteDoc("获取安全概览统计", {
      query: t.Object({
        rangeSec: t.Optional(t.String()),
      }),
    }),
  )
  // Session management
  .get(
    "/sessions",
    async () => {
      const list = await configManager.listSessions();
      const mapped = await Promise.all(
        list.map(async ({ id, data }) => {
          const session = await ensureSessionComment(id, data);
          const [mobility, fnosAttachments, trimMediaAttachments] =
            await Promise.all([
              authMobilitySessionManager.getSessionMobilitySummary(id),
              authMobilitySessionManager.listSessionFnosAttachments(id),
              authMobilitySessionManager.listSessionTrimMediaAttachments(id),
            ]);
          return {
            id,
            ...session,
            mobility,
            fnosAttachments,
            trimMediaAttachments,
          };
        }),
      );
      await ipLocationService.hydrateIpLocationRecords(mapped, (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: mapped };
    },
    routeDoc("获取会话列表"),
  )
  .get(
    "/sessions/:id",
    async ({ params, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }
      const session = await ensureSessionComment(params.id, sess);
      const [mobility, fnosAttachments, trimMediaAttachments] =
        await Promise.all([
          authMobilitySessionManager.getSessionMobilitySummary(params.id),
          authMobilitySessionManager.listSessionFnosAttachments(params.id),
          authMobilitySessionManager.listSessionTrimMediaAttachments(params.id),
        ]);
      const record = {
        id: params.id,
        ...session,
        mobility,
        fnosAttachments,
        trimMediaAttachments,
      };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    withRouteDoc("获取会话详情", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .patch(
    "/sessions/:id/comment",
    async ({ params, body, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }

      const updated = await configManager.updateSession(params.id, {
        comment: body.comment,
      });
      if (!updated) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }

      const whitelistRecordIds = new Set<string>();
      if (updated.postLoginIpGrantRecordId) {
        whitelistRecordIds.add(updated.postLoginIpGrantRecordId);
      }

      for (const mobilityWhitelistRecordId of await authMobilitySessionManager.listSessionWhitelistRecordIds(
        params.id,
      )) {
        whitelistRecordIds.add(mobilityWhitelistRecordId);
      }

      for (const whitelistRecordId of whitelistRecordIds) {
        await whitelistManager.updateComment(whitelistRecordId, body.comment);
      }

      const record = { id: params.id, ...updated };
      await ipLocationService.hydrateIpLocationRecords([record], (session) =>
        ipLocationRefs.session(session.id),
      );
      return { success: true, data: record };
    },
    withRouteDoc("更新会话备注", {
      params: t.Object({ id: t.String() }),
      body: t.Object({ comment: t.String() }),
    }),
  )
  .get(
    "/sessions/:id/mobility",
    async ({ params, set }) => {
      const sess = await configManager.getSession(params.id);
      if (!sess) {
        set.status = 404;
        return { success: false, message: "Session not found" };
      }
      const details =
        await authMobilitySessionManager.getSessionMobilityDetails(params.id);
      await ipLocationService.hydrateMobilityEvents(details.events, params.id);
      return {
        success: true,
        data: details,
      };
    },
    withRouteDoc("获取会话漫游详情", {
      params: t.Object({ id: t.String() }),
    }),
  )
  .delete(
    "/sessions/:id",
    async ({ params }) => {
      const sess = await configManager.getSession(params.id);
      if (sess) {
        const config = await configManager.getConfig();
        await authMobilitySessionManager.destroySession(params.id);
        await configManager.deleteSession(params.id);
        await revokeCustomPostLoginIpGrant(sess, config, sess.ip);
        scheduleSyncReverseProxyTrustedIPs({
          reason: "admin-session-delete",
        });
        await emitLogoutEvent({
          sessionId: params.id,
          authMethod: sess.method,
          credentialId: sess.credentialId,
          credentialName: sess.credentialName,
          ...(sess.linkedTotpName
            ? { linkedTotpName: sess.linkedTotpName }
            : {}),
          ...(sess.comment ? { sessionComment: sess.comment } : {}),
          ip: sess.ip,
          ...(sess.ipLocation ? { ipLocation: sess.ipLocation } : {}),
          userAgent: sess.userAgent,
          ...(sess.loginTime ? { loginTime: sess.loginTime } : {}),
          logoutSource: "admin_session_delete",
        });
      } else {
        await configManager.deleteSession(params.id);
      }
      return { success: true };
    },
    withRouteDoc("强制注销会话", {
      params: t.Object({ id: t.String() }),
    }),
  );
