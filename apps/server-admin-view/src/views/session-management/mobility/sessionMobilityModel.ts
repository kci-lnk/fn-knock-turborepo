import type { SessionMobilityEvent } from "@/types";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

export type SessionMobilityTimelineEntry = {
  id: string;
  event: SessionMobilityEvent;
  title: string;
  subtitle: string;
  gapLabel: string | null;
  happenedAtMs: number;
};

export const middleEllipsis = (text: string, max = 16) => {
  if (!text) return "";
  if (text.length <= max) return text;
  const head = Math.ceil((max - 1) / 2);
  const tail = Math.floor((max - 1) / 2);
  return `${text.slice(0, head)}……${text.slice(text.length - tail)}`;
};

export const formatSessionMobilitySource = (
  source: SessionMobilityEvent["source"],
  translate: Translate,
) => {
  if (source === "login") {
    return translate("admin.sessions.mobilityPage.source.login");
  }
  if (source === "fnos-token") {
    return translate("admin.sessions.mobilityPage.source.fnosToken");
  }
  if (source === "session-refresh") {
    return translate("admin.sessions.mobilityPage.source.sessionRefresh");
  }
  if (source === "browser-session") {
    return translate("admin.sessions.mobilityPage.source.browserSession");
  }
  return translate("admin.sessions.mobilityPage.source.proxySession");
};

export const formatSessionMobilityDuration = (
  milliseconds: number,
  translate: Translate,
) => {
  const totalMinutes = Math.max(0, Math.floor(milliseconds / 60_000));
  const days = Math.floor(totalMinutes / (60 * 24));
  const hours = Math.floor((totalMinutes % (60 * 24)) / 60);
  const minutes = totalMinutes % 60;

  if (days > 0) {
    return hours > 0
      ? translate("admin.sessions.mobilityPage.duration.daysHours", {
          days,
          hours,
        })
      : translate("admin.sessions.mobilityPage.duration.days", { days });
  }
  if (hours > 0) {
    return minutes > 0
      ? translate("admin.sessions.mobilityPage.duration.hoursMinutes", {
          hours,
          minutes,
        })
      : translate("admin.sessions.mobilityPage.duration.hours", { hours });
  }
  if (minutes > 0) {
    return translate("admin.sessions.mobilityPage.duration.minutes", {
      minutes,
    });
  }
  return translate("admin.sessions.mobilityPage.duration.lessThanMinute");
};

export const buildSessionMobilityTimeline = (
  sourceEvents: SessionMobilityEvent[],
  translate: Translate,
): SessionMobilityTimelineEntry[] => {
  const events = [...sourceEvents].sort(
    (a, b) =>
      (Date.parse(a.happenedAt) || 0) - (Date.parse(b.happenedAt) || 0),
  );

  return events.map((event, index) => {
    const previous = index > 0 ? events[index - 1] : null;
    const happenedAtMs = Date.parse(event.happenedAt) || 0;
    const previousMs = previous ? Date.parse(previous.happenedAt) || 0 : 0;
    const gapMs =
      previous && happenedAtMs > previousMs ? happenedAtMs - previousMs : 0;

    if (event.kind === "login") {
      return {
        id: `login-${event.happenedAt}-${index}`,
        event,
        title: translate("admin.sessions.mobilityPage.loginTitle"),
        subtitle: translate("admin.sessions.mobilityPage.loginSubtitle"),
        gapLabel: null,
        happenedAtMs,
      };
    }

    return {
      id: `drift-${event.happenedAt}-${index}`,
      event,
      title: formatSessionMobilitySource(event.source, translate),
      subtitle: translate("admin.sessions.mobilityPage.driftSubtitle"),
      gapLabel:
        gapMs > 0
          ? translate("admin.sessions.mobilityPage.gapLabel", {
              duration: formatSessionMobilityDuration(gapMs, translate),
            })
          : null,
      happenedAtMs,
    };
  });
};

export const getSessionMobilityTimelineSpan = (
  entries: SessionMobilityTimelineEntry[],
) => {
  if (entries.length < 2) return 0;
  return Math.max(
    0,
    (entries[entries.length - 1]?.happenedAtMs ?? 0) -
      (entries[0]?.happenedAtMs ?? 0),
  );
};
