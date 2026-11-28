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

export function streamVisibilityInput(visibility: StreamMapping["visibility"]) {
  if (visibility?.mode !== "custom") {
    return { mode: visibility?.mode ?? "inherit" };
  }
  return {
    mode: visibility.mode,
    selections: visibility.selections.map(
      ({ province, query_city, operator }) => ({
        province,
        query_city,
        operator: operator ?? null,
      }),
    ),
    custom_cidrs: visibility.custom_cidrs,
  };
}

const sameStreamVisibility = (
  current: StreamMapping,
  expected: StreamMapping,
) =>
  JSON.stringify(streamVisibilityInput(current.visibility)) ===
  JSON.stringify(streamVisibilityInput(expected.visibility));

// Full-list writes send manual state and visibility only when they change for a listener.
export function streamMappingWritePayload(
  current: readonly StreamMapping[],
  next: readonly StreamMapping[],
): StreamMapping[] {
  const previousByKey = new Map(
    current.map((mapping) => [getMappingKey(mapping), mapping]),
  );
  return next.map((mapping) => {
    const previous = previousByKey.get(getMappingKey(mapping));
    const payload = { ...mapping };
    if (previous) {
      if (payload.manually_disabled === (previous.manually_disabled === true)) {
        delete payload.manually_disabled;
      }
      if (sameStreamVisibility(payload, previous)) {
        delete payload.visibility;
      }
    }
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
      (expected.manually_disabled === true) &&
    sameStreamVisibility(current, expected)
  );
}
