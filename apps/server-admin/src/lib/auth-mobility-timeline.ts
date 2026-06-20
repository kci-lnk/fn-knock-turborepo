export type MobilityDriftSource =
  | "proxy-session"
  | "fnos-token"
  | "session-refresh"
  | "browser-session";

export type MobilityTimelineEvent =
  | {
      version: 1;
      kind: "login";
      happenedAt: string;
      source: "login";
      toIp: string;
      toIpLocation?: string;
    }
  | {
      version: 1;
      kind: "drift";
      happenedAt: string;
      source: MobilityDriftSource;
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

export type SessionMobilitySummary = {
  hasHistory: boolean;
  driftCount: number;
  lastDriftAt: string | null;
  lastDriftSource: MobilityDriftSource | null;
};

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: MobilityTimelineEvent[];
};

export const buildMobilityLoginEvent = (args: {
  ip: string;
  ipLocation?: string;
  happenedAt?: string;
}): MobilityTimelineEvent => ({
  version: 1,
  kind: "login",
  happenedAt: args.happenedAt || new Date().toISOString(),
  source: "login",
  toIp: args.ip,
  ...(args.ipLocation ? { toIpLocation: args.ipLocation } : {}),
});

export const buildMobilityDriftEvent = (args: {
  source: MobilityDriftSource;
  fromIp: string;
  fromIpLocation?: string;
  toIp: string;
  toIpLocation?: string;
}): MobilityTimelineEvent => ({
  version: 1,
  kind: "drift",
  happenedAt: new Date().toISOString(),
  source: args.source,
  fromIp: args.fromIp,
  ...(args.fromIpLocation ? { fromIpLocation: args.fromIpLocation } : {}),
  toIp: args.toIp,
  ...(args.toIpLocation ? { toIpLocation: args.toIpLocation } : {}),
});

export const buildMobilitySummary = (
  events: MobilityTimelineEvent[],
): SessionMobilitySummary => {
  const driftEvents = events.filter(
    (event): event is Extract<MobilityTimelineEvent, { kind: "drift" }> =>
      event.kind === "drift",
  );
  const lastDrift = driftEvents[driftEvents.length - 1];
  return {
    hasHistory: events.length > 0,
    driftCount: driftEvents.length,
    lastDriftAt: lastDrift?.happenedAt ?? null,
    lastDriftSource: lastDrift?.source ?? null,
  };
};

export const limitMobilityTimelineEvents = (
  events: MobilityTimelineEvent[],
  maxEvents: number,
): MobilityTimelineEvent[] => {
  if (events.length <= maxEvents) return events;

  const firstEvent = events[0];
  if (firstEvent?.kind === "login") {
    const tailCount = Math.max(0, maxEvents - 1);
    return [firstEvent, ...events.slice(-tailCount)];
  }

  return events.slice(-maxEvents);
};

export const nextMobilitySummaryFromEvent = (
  events: MobilityTimelineEvent[],
  storedSummary: SessionMobilitySummary | null,
  event: MobilityTimelineEvent,
  seedLoginEvent?: MobilityTimelineEvent,
): SessionMobilitySummary => {
  const baseline =
    storedSummary ??
    buildMobilitySummary(
      events.length === 0 && seedLoginEvent ? [seedLoginEvent] : events,
    );

  if (event.kind !== "drift") {
    return baseline;
  }

  return {
    hasHistory: true,
    driftCount: baseline.driftCount + 1,
    lastDriftAt: event.happenedAt,
    lastDriftSource: event.source,
  };
};
