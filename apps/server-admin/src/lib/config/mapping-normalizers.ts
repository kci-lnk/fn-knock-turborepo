import { isAuthServiceTarget } from "../auth-service";
import { normalizePositiveInt } from "./normalizers";
import type {
  HostAccessMode,
  HostLocation,
  HostLocationAction,
  HostLocationMatch,
  HostLocationResponse,
  HostMapping,
  HostMappingBasicAuth,
  HostServiceRole,
  StreamMapping,
  StreamMappingProtocol,
  SubdomainModeConfig,
} from "./types";

export const DEFAULT_SUBDOMAIN_AUTH_TARGET = `http://localhost:${
  process.env.AUTH_PORT || "7997"
}`;
export const DEFAULT_SUBDOMAIN_AUTH_CACHE_TTL_SECONDS = 1;
export const DEFAULT_SUBDOMAIN_AUTH_CACHE_UNAUTHORIZED_TTL_SECONDS = 1;

export const normalizeHost = (value: unknown): string => {
  if (typeof value !== "string") return "";
  return value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "");
};

export const normalizeHostAccessMode = (value: unknown): HostAccessMode =>
  value === "strict_whitelist" ? "strict_whitelist" : "login_first";

export const normalizeHostServiceRole = (value: unknown): HostServiceRole =>
  value === "auth" ? "auth" : "app";

export const normalizeStreamProtocol = (
  value: unknown,
): StreamMappingProtocol => (value === "udp" ? "udp" : "tcp");

export const createDisabledHostBasicAuth = (): HostMappingBasicAuth => ({
  enabled: false,
  username: "",
  password: "",
});

export const normalizeHostBasicAuth = (
  value?: Partial<HostMappingBasicAuth> | null,
): HostMappingBasicAuth => {
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

export const DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE =
  "text/plain; charset=utf-8";

export const forbiddenHostLocationResponseHeaders = new Set([
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

export const normalizeHostLocationMatch = (
  value: unknown,
): HostLocationMatch => (value === "exact" ? "exact" : "prefix");

export const normalizeHostLocationAction = (
  value: unknown,
): HostLocationAction => (value === "response" ? "response" : "proxy");

export const isValidHTTPHeaderName = (value: string): boolean =>
  /^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/.test(value);

export const cleanHostLocationPath = (value: string): string => {
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

export const normalizeHostLocationResponseHeaders = (
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

export const normalizeHostLocationResponse = (
  value?: Partial<HostLocationResponse> | null,
): HostLocationResponse => {
  const raw = value ?? {};
  const status = Math.floor(Number(raw.status) || 200);
  return {
    status: status >= 100 && status <= 599 ? status : 200,
    content_type:
      typeof raw.content_type === "string" && raw.content_type.trim()
        ? raw.content_type.trim()
        : DEFAULT_HOST_LOCATION_RESPONSE_CONTENT_TYPE,
    headers: normalizeHostLocationResponseHeaders(raw.headers),
    body: typeof raw.body === "string" ? raw.body : "",
  };
};

export const normalizeHostLocation = (
  value?: Partial<HostLocation> | null,
): HostLocation | null => {
  const raw = value ?? {};
  const path =
    typeof raw.path === "string" ? cleanHostLocationPath(raw.path) : "";
  if (!path || !path.startsWith("/") || path === "/") return null;
  if (path.startsWith("/__") || path === "/s" || path === "/s/") return null;

  const action = normalizeHostLocationAction(raw.action);
  const target = typeof raw.target === "string" ? raw.target.trim() : "";
  if (action === "proxy" && !target) return null;

  return {
    path,
    match: normalizeHostLocationMatch(raw.match),
    action,
    target: action === "proxy" ? target : "",
    strip_path: action === "proxy" ? raw.strip_path !== false : false,
    rewrite_html: action === "proxy" ? raw.rewrite_html !== false : false,
    response:
      action === "response"
        ? normalizeHostLocationResponse(raw.response)
        : normalizeHostLocationResponse(null),
  };
};

export const normalizeHostLocations = (
  value?: Array<Partial<HostLocation>> | null,
): HostLocation[] => {
  if (!Array.isArray(value)) return [];

  const locations: HostLocation[] = [];
  const seen = new Set<string>();
  for (const item of value) {
    const normalized = normalizeHostLocation(item);
    if (!normalized) continue;
    const key = `${normalized.match}\0${normalized.path}`;
    if (seen.has(key)) continue;
    seen.add(key);
    locations.push(normalized);
  }
  return locations;
};

export const normalizeHostMapping = (
  value?: Partial<HostMapping> | null,
): HostMapping => {
  const raw = value ?? {};
  const target = typeof raw.target === "string" ? raw.target.trim() : "";
  const serviceRole = isAuthServiceTarget(target)
    ? "auth"
    : normalizeHostServiceRole(raw.service_role);

  return {
    host: normalizeHost(raw.host),
    target,
    use_auth: serviceRole === "auth" ? false : raw.use_auth !== false,
    access_mode:
      serviceRole === "auth"
        ? "login_first"
        : normalizeHostAccessMode(raw.access_mode),
    suppress_toolbar:
      serviceRole === "auth" ? false : raw.suppress_toolbar === true,
    preserve_host: true,
    basic_auth:
      serviceRole === "auth"
        ? createDisabledHostBasicAuth()
        : normalizeHostBasicAuth(raw.basic_auth),
    locations:
      serviceRole === "auth" ? [] : normalizeHostLocations(raw.locations),
    service_role: serviceRole,
    title: typeof raw.title === "string" ? raw.title.trim() : "",
    title_override:
      typeof raw.title_override === "string" ? raw.title_override.trim() : "",
    favicon: typeof raw.favicon === "string" ? raw.favicon.trim() : "",
  };
};

export const normalizeHostMappings = (
  value?: Array<Partial<HostMapping>> | null,
): HostMapping[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeHostMapping(item))
    .filter((item) => item.host && item.target);
};

export const normalizeStreamMapping = (
  value?: Partial<StreamMapping> | null,
): StreamMapping => {
  const raw = value ?? {};

  return {
    protocol: normalizeStreamProtocol(raw.protocol),
    listen_port: normalizePositiveInt(raw.listen_port, 0, {
      min: 1,
      max: 65535,
    }),
    target: typeof raw.target === "string" ? raw.target.trim() : "",
    use_auth: raw.use_auth !== false,
  };
};

export const normalizeStreamMappings = (
  value?: Array<Partial<StreamMapping>> | null,
): StreamMapping[] => {
  if (!Array.isArray(value)) return [];
  return value
    .map((item) => normalizeStreamMapping(item))
    .filter((item) => item.listen_port > 0 && item.target);
};

export const normalizeSubdomainModeConfig = (
  value?: Partial<SubdomainModeConfig> | null,
): SubdomainModeConfig => {
  const raw = value ?? {};
  const hasOwn = (key: keyof SubdomainModeConfig) =>
    Object.prototype.hasOwnProperty.call(raw, key);
  const normalizePublicPort = (input: unknown): number => {
    const port =
      typeof input === "number"
        ? input
        : Number.parseInt(String(input ?? ""), 10);
    if (!Number.isFinite(port) || port <= 0) return 0;
    return Math.floor(port);
  };
  const normalizeCacheTTL = (input: unknown, fallback: number): number => {
    const ttl =
      typeof input === "number"
        ? input
        : Number.parseInt(String(input ?? ""), 10);
    if (!Number.isFinite(ttl) || ttl < 0) return fallback;
    return Math.floor(ttl);
  };

  let edgeClientIPEnabled = raw.edge_client_ip_enabled === true;
  let aliyunESAEnabled = raw.aliyun_esa_enabled === true;
  let tencentEdgeOneEnabled = raw.tencent_edgeone_enabled === true;

  if (
    !hasOwn("edge_client_ip_enabled") &&
    (aliyunESAEnabled || tencentEdgeOneEnabled)
  ) {
    edgeClientIPEnabled = true;
  }

  if (!edgeClientIPEnabled) {
    aliyunESAEnabled = false;
    tencentEdgeOneEnabled = false;
  }

  if (tencentEdgeOneEnabled && aliyunESAEnabled) {
    aliyunESAEnabled = false;
  }

  return {
    root_domain:
      typeof raw.root_domain === "string"
        ? raw.root_domain.trim().toLowerCase()
        : "",
    auth_host: normalizeHost(raw.auth_host),
    auth_target:
      typeof raw.auth_target === "string" && raw.auth_target.trim()
        ? raw.auth_target.trim()
        : DEFAULT_SUBDOMAIN_AUTH_TARGET,
    cookie_domain:
      typeof raw.cookie_domain === "string" ? raw.cookie_domain.trim() : "",
    edge_client_ip_enabled: edgeClientIPEnabled,
    aliyun_esa_enabled: aliyunESAEnabled,
    tencent_edgeone_enabled: tencentEdgeOneEnabled,
    public_auth_base_url:
      typeof raw.public_auth_base_url === "string"
        ? raw.public_auth_base_url.trim().replace(/\/+$/, "")
        : "",
    public_http_port: normalizePublicPort(raw.public_http_port),
    public_https_port: normalizePublicPort(raw.public_https_port),
    auth_cache_ttl_seconds: normalizeCacheTTL(
      raw.auth_cache_ttl_seconds,
      DEFAULT_SUBDOMAIN_AUTH_CACHE_TTL_SECONDS,
    ),
    auth_cache_unauthorized_ttl_seconds: normalizeCacheTTL(
      raw.auth_cache_unauthorized_ttl_seconds,
      DEFAULT_SUBDOMAIN_AUTH_CACHE_UNAUTHORIZED_TTL_SECONDS,
    ),
    default_access_mode: normalizeHostAccessMode(raw.default_access_mode),
    auto_add_whitelist_on_login: raw.auto_add_whitelist_on_login !== false,
    passkey_rp_mode:
      raw.passkey_rp_mode === "parent_domain" ? "parent_domain" : "auth_host",
    passkey_rp_id:
      typeof raw.passkey_rp_id === "string"
        ? raw.passkey_rp_id.trim().toLowerCase()
        : "",
  };
};
