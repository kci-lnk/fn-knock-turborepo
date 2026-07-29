import type { StreamMapping, StreamMappingProtocol } from "@/types";
import { parseHostPort } from "@admin-shared/utils/parseHostPort";

export type StreamMappingEditorSubmission = {
  editingKey: string | null;
  mappings: StreamMapping[];
};

export const DEFAULT_STREAM_PROTOCOL: StreamMappingProtocol = "tcp";
export const STREAM_PROTOCOLS: StreamMappingProtocol[] = ["tcp", "udp"];

export const normalizeProtocol = (
  protocol?: StreamMappingProtocol | string | null,
): StreamMappingProtocol =>
  protocol === "udp" ? "udp" : DEFAULT_STREAM_PROTOCOL;

export const normalizeProtocolSelection = (
  protocols: StreamMappingProtocol[] | undefined,
): StreamMappingProtocol[] => {
  const selected = new Set(
    (protocols ?? []).map((protocol) => normalizeProtocol(protocol)),
  );
  const normalized = STREAM_PROTOCOLS.filter((protocol) =>
    selected.has(protocol),
  );
  return normalized.length > 0 ? normalized : [DEFAULT_STREAM_PROTOCOL];
};

export const normalizeStreamMapping = (
  mapping: StreamMapping,
): StreamMapping => ({
  ...mapping,
  protocol: normalizeProtocol(mapping.protocol),
  comment: mapping.comment?.trim() ?? "",
});

export const createMappingKey = (
  protocol: StreamMappingProtocol,
  listenPort: number,
): string => `${protocol}:${listenPort}`;

export const getMappingKey = (mapping: StreamMapping): string =>
  createMappingKey(normalizeProtocol(mapping.protocol), mapping.listen_port);

export const compareStreamMappings = (
  a: StreamMapping,
  b: StreamMapping,
): number =>
  a.listen_port === b.listen_port
    ? a.protocol.localeCompare(b.protocol)
    : a.listen_port - b.listen_port;

const isObviousLocalTargetHost = (host: string): boolean => {
  const normalized = host.trim().replace(/\.$/u, "").toLowerCase();
  if (
    normalized === "localhost" ||
    normalized === "0.0.0.0" ||
    normalized === "::" ||
    normalized === "::1" ||
    normalized === "0:0:0:0:0:0:0:0" ||
    normalized === "0:0:0:0:0:0:0:1"
  ) {
    return true;
  }
  if (/^127(?:\.\d{1,3}){3}$/u.test(normalized)) return true;
  return /^::ffff:127(?:\.\d{1,3}){3}$/u.test(normalized);
};

export const isObviousLocalStreamTargetLoop = (
  target: string,
  listenPort: number,
): boolean => {
  const parsed = parseHostPort(target);
  return (
    parsed !== null &&
    parsed.port === listenPort &&
    isObviousLocalTargetHost(parsed.host)
  );
};

const normalizeStreamMappings = (
  mappings: readonly StreamMapping[],
): StreamMapping[] =>
  [...mappings].map(normalizeStreamMapping).sort(compareStreamMappings);

export const applyStreamMappingSubmission = (
  mappings: readonly StreamMapping[],
  submission: StreamMappingEditorSubmission,
): StreamMapping[] => {
  const next = normalizeStreamMappings(mappings);
  const existingIndex = next.findIndex(
    (mapping) => getMappingKey(mapping) === submission.editingKey,
  );
  if (existingIndex >= 0) {
    next.splice(existingIndex, 1, ...submission.mappings);
  } else {
    next.push(...submission.mappings);
  }
  return next;
};

export const removeStreamMapping = (
  mappings: readonly StreamMapping[],
  key: string,
): StreamMapping[] =>
  normalizeStreamMappings(mappings).filter(
    (mapping) => getMappingKey(mapping) !== key,
  );

export const updateStreamMappingComment = (
  mappings: readonly StreamMapping[],
  key: string,
  comment: string,
): StreamMapping[] =>
  normalizeStreamMappings(mappings).map((mapping) =>
    getMappingKey(mapping) === key ? { ...mapping, comment } : mapping,
  );

export const formatProtocolLabel = (protocol: StreamMappingProtocol): string =>
  protocol.toUpperCase();

export const formatMappingLabel = (mapping: StreamMapping): string =>
  `${formatProtocolLabel(normalizeProtocol(mapping.protocol))}/${mapping.listen_port}`;
