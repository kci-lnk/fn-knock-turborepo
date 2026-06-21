import { extractPortFromTarget } from "./extractPortFromTarget";

export type ProxyTargetProtocol = "http" | "https" | "ws" | "wss";

export const DEFAULT_PROXY_TARGET_PROTOCOL: ProxyTargetProtocol = "http";
export const DEFAULT_PROXY_TARGET_PORTS: Record<ProxyTargetProtocol, string> = {
  http: "80",
  https: "443",
  ws: "80",
  wss: "443",
};
export const DEFAULT_PROXY_TARGET_PORT =
  DEFAULT_PROXY_TARGET_PORTS[DEFAULT_PROXY_TARGET_PROTOCOL];
export const PROXY_TARGET_PROTOCOLS: ProxyTargetProtocol[] = [
  "http",
  "https",
  "ws",
  "wss",
];

const PROXY_TARGET_PROTOCOL_SET = new Set<string>(PROXY_TARGET_PROTOCOLS);

const isNormalizedProxyTargetProtocol = (
  value: string,
): value is ProxyTargetProtocol => PROXY_TARGET_PROTOCOL_SET.has(value);

type ParsedProxyTargetParts = {
  protocol: ProxyTargetProtocol;
  endpoint: string;
  hadProtocol: boolean;
};

const TARGET_PROTOCOL_PATTERN = /^(https?|wss?):\/\/(.*)$/i;

const hasExplicitEmptyPort = (value: string): boolean => {
  const match = value.trim().match(TARGET_PROTOCOL_PATTERN);
  if (!match) return false;

  const [, , endpoint = ""] = match;
  const firstSuffixIndex = endpoint.search(/[/?#]/);
  const boundary =
    firstSuffixIndex === -1 ? endpoint.length : firstSuffixIndex;
  const authorityWithCredentials = endpoint.slice(0, boundary);
  const authority = authorityWithCredentials.slice(
    authorityWithCredentials.lastIndexOf("@") + 1,
  );

  return authority.endsWith(":");
};

export const isProxyTargetProtocol = (
  value: string | null | undefined,
): boolean => {
  const normalized = value?.toLowerCase().replace(/:$/, "") ?? "";
  return isNormalizedProxyTargetProtocol(normalized);
};

export const isHttpProxyTargetProtocol = (
  value: string | null | undefined,
): boolean => {
  const normalized = value?.toLowerCase().replace(/:$/, "") ?? "";
  return normalized === "http" || normalized === "https";
};

export const isWebSocketProxyTargetProtocol = (
  value: string | null | undefined,
): boolean => {
  const normalized = value?.toLowerCase().replace(/:$/, "") ?? "";
  return normalized === "ws" || normalized === "wss";
};

const normalizeProtocol = (
  value: string | null | undefined,
): ProxyTargetProtocol => {
  const normalized = value?.toLowerCase().replace(/:$/, "") ?? "";
  return isNormalizedProxyTargetProtocol(normalized)
    ? normalized
    : DEFAULT_PROXY_TARGET_PROTOCOL;
};

export const getDefaultProxyTargetPort = (
  protocol: ProxyTargetProtocol,
  defaultPort?: string,
): string => {
  const explicitDefaultPort = defaultPort?.trim();
  return explicitDefaultPort || DEFAULT_PROXY_TARGET_PORTS[protocol];
};

export const parseProxyTargetParts = (
  value: string,
  fallbackProtocol: ProxyTargetProtocol = DEFAULT_PROXY_TARGET_PROTOCOL,
): ParsedProxyTargetParts => {
  const trimmed = value.trim();
  if (!trimmed) {
    return {
      protocol: fallbackProtocol,
      endpoint: "",
      hadProtocol: false,
    };
  }

  const match = trimmed.match(TARGET_PROTOCOL_PATTERN);
  if (match) {
    const [, protocol = fallbackProtocol, endpoint = ""] = match;
    return {
      protocol: normalizeProtocol(protocol),
      endpoint: endpoint.trim(),
      hadProtocol: true,
    };
  }

  return {
    protocol: fallbackProtocol,
    endpoint: trimmed.replace(/^\/\//, "").trim(),
    hadProtocol: false,
  };
};

export const resolveProxyTargetInput = (
  selectedProtocol: ProxyTargetProtocol,
  endpointInput: string,
) => {
  const parsed = parseProxyTargetParts(endpointInput, selectedProtocol);
  const protocol = parsed.hadProtocol ? parsed.protocol : selectedProtocol;
  const endpoint = parsed.endpoint;

  return {
    protocol,
    endpoint,
    hadProtocol: parsed.hadProtocol,
    target: endpoint ? `${protocol}://${endpoint}` : "",
  };
};

export const ensureProxyTargetPort = (
  endpoint: string,
  defaultPort: string = DEFAULT_PROXY_TARGET_PORT,
): string => {
  const trimmed = endpoint.trim();
  if (!trimmed || extractPortFromTarget(trimmed) !== null) {
    return trimmed;
  }

  const port = defaultPort.trim();
  if (!port) {
    return trimmed;
  }

  const firstSuffixIndex = trimmed.search(/[/?#]/);
  const boundary = firstSuffixIndex === -1 ? trimmed.length : firstSuffixIndex;
  const authority = trimmed.slice(0, boundary);
  const suffix = trimmed.slice(boundary);

  if (!authority) {
    return trimmed;
  }

  if (authority.startsWith("[")) {
    const closingBracketIndex = authority.indexOf("]");
    if (closingBracketIndex === -1) {
      return trimmed;
    }

    const host = authority.slice(0, closingBracketIndex + 1);
    const rest = authority.slice(closingBracketIndex + 1);
    return `${host}:${port}${rest}${suffix}`;
  }

  if (authority.includes(":")) {
    return trimmed;
  }

  return `${authority}:${port}${suffix}`;
};

export const normalizeProxyTargetInput = (
  selectedProtocol: ProxyTargetProtocol,
  endpointInput: string,
  defaultPort?: string,
) => {
  const resolved = resolveProxyTargetInput(selectedProtocol, endpointInput);
  const endpoint = ensureProxyTargetPort(
    resolved.endpoint,
    getDefaultProxyTargetPort(resolved.protocol, defaultPort),
  );

  return {
    ...resolved,
    endpoint,
    target: endpoint ? `${resolved.protocol}://${endpoint}` : "",
  };
};

export const isSupportedProxyTargetUrl = (value: string): boolean => {
  const target = value.trim();
  if (!target) return false;
  if (hasExplicitEmptyPort(target)) return false;

  try {
    const parsed = new URL(target);
    return (
      isProxyTargetProtocol(parsed.protocol) && Boolean(parsed.hostname.trim())
    );
  } catch {
    return false;
  }
};

export const isWebSocketProxyTargetUrl = (value: string): boolean => {
  const target = value.trim();
  if (!target) return false;
  if (hasExplicitEmptyPort(target)) return false;

  try {
    const parsed = new URL(target);
    return (
      isWebSocketProxyTargetProtocol(parsed.protocol) &&
      Boolean(parsed.hostname.trim())
    );
  } catch {
    return false;
  }
};
