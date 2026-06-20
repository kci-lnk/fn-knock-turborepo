import { isAuthServiceTarget } from "../../lib/auth-service";
import {
  type HostMapping,
  type ProxyMapping,
  type StreamMapping,
} from "../../lib/redis";
import { normalizeIpLocationServiceUrl } from "../../lib/ip-location-api-url";
import { isValidHostPort } from "../../../../../packages/admin-shared/src/utils/parseHostPort";
import { isSupportedProxyTargetUrl } from "../../../../../packages/admin-shared/src/utils/proxyTargetInput";

type AdminMessageParams = Record<
  string,
  string | number | boolean | null | undefined
>;

export type AdminRouteTranslator = (
  key: string,
  params?: AdminMessageParams,
) => string;

export const adminT = (
  t: AdminRouteTranslator,
  key: string,
  params?: AdminMessageParams,
) => t(`server.admin.${key}`, params);

export type HostLocationResponseInput = {
  status?: unknown;
  content_type?: unknown;
  headers?: Record<string, unknown> | null;
  body?: unknown;
};

export type HostLocationInput = {
  path?: unknown;
  match?: unknown;
  action?: unknown;
  target?: unknown;
  strip_path?: unknown;
  rewrite_html?: unknown;
  response?: HostLocationResponseInput | null;
};

export type HostMappingInput = {
  host: string;
  target: string;
  use_auth: boolean;
  access_mode: "login_first" | "strict_whitelist";
  suppress_toolbar?: boolean;
  basic_auth?: Partial<HostMapping["basic_auth"]> | null;
  service_role?: "app" | "auth";
  locations?: HostLocationInput[] | null;
};

export type ProxyMappingInput = Pick<
  ProxyMapping,
  | "path"
  | "target"
  | "rewrite_html"
  | "use_auth"
  | "use_root_mode"
  | "strip_path"
>;

export type StreamMappingInput = Pick<
  StreamMapping,
  "listen_port" | "target" | "use_auth"
> & {
  protocol?: StreamMapping["protocol"];
};

export const validateHostMappings = (
  mappings: HostMappingInput[],
  t: AdminRouteTranslator,
) => {
  for (const mapping of mappings) {
    const label = mapping.host.trim()
      ? `Host mapping ${mapping.host.trim()} target`
      : "Host mapping target";
    const message = validateProxyTargetUrl(mapping.target, label, t);
    if (message) {
      return {
        valid: false as const,
        message,
      };
    }
  }

  const authMappings = mappings.filter((mapping) =>
    isAuthServiceTarget(mapping.target),
  );
  if (authMappings.length > 1) {
    return {
      valid: false as const,
      message: adminT(t, "hostMappings.singleAuthPortMapping"),
    };
  }

  const invalidAuthMapping = authMappings.find(
    (mapping) => mapping.use_auth || mapping.access_mode === "strict_whitelist",
  );
  if (invalidAuthMapping) {
    return {
      valid: false as const,
      message: adminT(t, "hostMappings.authMappingMustBePublic", {
        host: invalidAuthMapping.host,
      }),
    };
  }

  const authMappingWithBasicAuth = authMappings.find(
    (mapping) => mapping.basic_auth?.enabled === true,
  );
  if (authMappingWithBasicAuth) {
    return {
      valid: false as const,
      message: adminT(t, "hostMappings.authMappingBasicAuthForbidden", {
        host: authMappingWithBasicAuth.host,
      }),
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
      message: adminT(t, "hostMappings.basicAuthInvalid", {
        host: invalidBasicAuthMapping.host,
      }),
    };
  }

  const invalidLocation = validateHostMappingLocations(mappings, t);
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

export const validateHostMappingLocations = (
  mappings: Array<{
    host: string;
    target: string;
    locations?: HostLocationInput[] | null;
  }>,
  t: AdminRouteTranslator,
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
        return adminT(t, "hostMappings.locationPathRequired", {
          host: mapping.host,
        });
      }
      if (!locationPath.startsWith("/")) {
        return adminT(t, "hostMappings.locationPathMustStartSlash", {
          host: mapping.host,
          path: locationPath,
        });
      }
      const cleanPath = cleanHostLocationPath(locationPath);
      if (cleanPath === "/") {
        return adminT(t, "hostMappings.locationRootForbidden", {
          host: mapping.host,
        });
      }
      if (
        cleanPath.startsWith("/__") ||
        cleanPath === "/s" ||
        cleanPath === "/s/"
      ) {
        return adminT(t, "hostMappings.locationReservedPath", {
          host: mapping.host,
          path: locationPath,
        });
      }
      const match = location.match === "exact" ? "exact" : "prefix";
      const duplicateKey = `${match}\0${cleanPath}`;
      if (seen.has(duplicateKey)) {
        return adminT(t, "hostMappings.locationDuplicate", {
          host: mapping.host,
          path: locationPath,
        });
      }
      seen.add(duplicateKey);

      const action = location.action === "response" ? "response" : "proxy";
      if (action === "proxy") {
        const target =
          typeof location.target === "string" ? location.target.trim() : "";
        if (!target) {
          return adminT(t, "hostMappings.locationTargetRequired", {
            host: mapping.host,
            path: locationPath,
          });
        }
        const invalidTargetMessage = validateProxyTargetUrl(
          target,
          `Host mapping ${mapping.host} path ${locationPath} target`,
          t,
        );
        if (invalidTargetMessage) {
          return invalidTargetMessage;
        }
      } else {
        const response = (location.response ?? {}) as Partial<
          HostMapping["locations"][number]["response"]
        >;
        const status = Math.floor(Number(response.status) || 200);
        if (status < 100 || status > 599) {
          return adminT(t, "hostMappings.locationStatusInvalid", {
            host: mapping.host,
            path: locationPath,
          });
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
            return adminT(t, "hostMappings.locationHeaderInvalid", {
              header: rawName,
              host: mapping.host,
              path: locationPath,
            });
          }
          if (forbiddenHostLocationResponseHeaders.has(name.toLowerCase())) {
            return adminT(t, "hostMappings.locationHeaderForbidden", {
              header: name,
              host: mapping.host,
              path: locationPath,
            });
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

export const normalizeHostMappingLocationsForRoute = (
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

export const normalizeHostMappingLookupKey = (value: string): string =>
  value
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "");

export const createDisabledHostBasicAuth = (): HostMapping["basic_auth"] => ({
  enabled: false,
  username: "",
  password: "",
});

export const normalizeHostBasicAuth = (
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

const isValidStreamTarget = (target: string): boolean => {
  return isValidHostPort(target);
};

export const validateProxyTargetUrl = (
  target: unknown,
  label: string,
  t: AdminRouteTranslator,
): string | null => {
  const value = typeof target === "string" ? target.trim() : "";
  if (!value) {
    return adminT(t, "validation.required", { label });
  }
  if (!isSupportedProxyTargetUrl(value)) {
    return adminT(t, "validation.proxyTargetUrlRequired", { label });
  }
  return null;
};

export const validateProxyMappings = (
  mappings: ProxyMappingInput[],
  t: AdminRouteTranslator,
) => {
  for (const mapping of mappings) {
    const label = mapping.path.trim()
      ? `Path mapping ${mapping.path.trim()} target`
      : "Path mapping target";
    const message = validateProxyTargetUrl(mapping.target, label, t);
    if (message) {
      return { valid: false as const, message };
    }
  }

  return { valid: true as const };
};

export const validateIpLocationBaseUrl = (
  value: unknown,
  label: string,
  t: AdminRouteTranslator,
) => {
  const url = normalizeIpLocationServiceUrl(value);
  if (!url) {
    return {
      valid: false as const,
      url,
      message: adminT(t, "validation.required", { label }),
    };
  }

  try {
    const parsed = new URL(url);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return {
        valid: false as const,
        url,
        message: adminT(t, "validation.httpUrlRequired", { label }),
      };
    }
  } catch {
    return {
      valid: false as const,
      url,
      message: adminT(t, "validation.invalidFormat", { label }),
    };
  }

  return { valid: true as const, url };
};

export const validateStreamMappings = (
  mappings: StreamMappingInput[],
  t: AdminRouteTranslator,
) => {
  const seenMappings = new Set<string>();

  for (const mapping of mappings) {
    const protocol = mapping.protocol === "udp" ? "udp" : "tcp";
    const listenPort = mapping.listen_port;
    const target = mapping.target;

    if (!Number.isInteger(listenPort)) {
      return {
        valid: false as const,
        message: adminT(t, "streamMappings.listenPortNotInteger", {
          port: listenPort,
        }),
      };
    }
    if (listenPort <= 0 || listenPort > 65535) {
      return {
        valid: false as const,
        message: adminT(t, "streamMappings.listenPortOutOfRange", {
          port: listenPort,
        }),
      };
    }
    const mappingKey = `${protocol}:${listenPort}`;
    if (seenMappings.has(mappingKey)) {
      return {
        valid: false as const,
        message: adminT(t, "streamMappings.duplicatePort", {
          port: listenPort,
          protocol: protocol.toUpperCase(),
        }),
      };
    }
    if (!isValidStreamTarget(target)) {
      return {
        valid: false as const,
        message: adminT(t, "streamMappings.targetMustBeHostPort", {
          target,
        }),
      };
    }
    seenMappings.add(mappingKey);
  }

  return { valid: true as const };
};
