import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";

export type ProxyTargetProtocol = "http" | "https";

export const DEFAULT_PROXY_TARGET_PROTOCOL: ProxyTargetProtocol = "http";
export const DEFAULT_PROXY_TARGET_PORT = "80";

type ParsedProxyTargetParts = {
  protocol: ProxyTargetProtocol;
  endpoint: string;
  hadProtocol: boolean;
};

const TARGET_PROTOCOL_PATTERN = /^(https?):\/\/(.*)$/i;

const normalizeProtocol = (
  value: string | null | undefined,
): ProxyTargetProtocol => (value?.toLowerCase() === "https" ? "https" : "http");

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

  return `${authority}:${port}${suffix}`;
};

export const normalizeProxyTargetInput = (
  selectedProtocol: ProxyTargetProtocol,
  endpointInput: string,
  defaultPort: string = DEFAULT_PROXY_TARGET_PORT,
) => {
  const resolved = resolveProxyTargetInput(selectedProtocol, endpointInput);
  const endpoint = ensureProxyTargetPort(resolved.endpoint, defaultPort);

  return {
    ...resolved,
    endpoint,
    target: endpoint ? `${resolved.protocol}://${endpoint}` : "",
  };
};
