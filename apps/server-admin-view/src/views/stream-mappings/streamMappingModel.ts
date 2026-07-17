import type { StreamMapping, StreamMappingProtocol } from "@/types";

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

export const formatProtocolLabel = (protocol: StreamMappingProtocol): string =>
  protocol.toUpperCase();

export const formatMappingLabel = (mapping: StreamMapping): string =>
  `${formatProtocolLabel(normalizeProtocol(mapping.protocol))}/${mapping.listen_port}`;
