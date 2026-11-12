import type { StreamMapping, StreamMappingProtocol } from "@/types";

export const DEFAULT_STREAM_PROTOCOL: StreamMappingProtocol = "tcp";

export const normalizeProtocol = (
  protocol?: StreamMappingProtocol | string | null,
): StreamMappingProtocol =>
  protocol === "udp" ? "udp" : DEFAULT_STREAM_PROTOCOL;

export const createMappingKey = (
  protocol: StreamMappingProtocol,
  listenPort: number,
): string => `${protocol}:${listenPort}`;

export const getMappingKey = (mapping: StreamMapping): string =>
  createMappingKey(normalizeProtocol(mapping.protocol), mapping.listen_port);

// Ordinary edits omit the manual flag so a concurrent pause survives a full-list save.
export function streamMappingWritePayload(
  current: readonly StreamMapping[],
  next: readonly StreamMapping[],
): StreamMapping[] {
  const previousState = new Map(
    current.map((mapping) => [
      getMappingKey(mapping),
      mapping.manually_disabled === true,
    ]),
  );
  return next.map((mapping) => {
    if (
      mapping.manually_disabled === undefined ||
      mapping.manually_disabled !== previousState.get(getMappingKey(mapping))
    ) {
      return mapping;
    }
    const { manually_disabled: _unchanged, ...payload } = mapping;
    return payload;
  });
}

export function sameStreamRule(
  current: StreamMapping,
  expected: StreamMapping,
): boolean {
  return (
    getMappingKey(current) === getMappingKey(expected) &&
    current.target === expected.target &&
    (current.use_auth ?? true) === (expected.use_auth ?? true) &&
    (current.comment ?? "").trim() === (expected.comment ?? "").trim() &&
    (current.manually_disabled === true) ===
      (expected.manually_disabled === true)
  );
}
