import type { GatewayLogAnalyticsBucket } from "@/types";
import { AUTH_DECISION_LABEL_KEYS } from "@/lib/gatewayLogLabels";

export type RequestAnalyticsRangeKey = "today" | "7d" | "30d";

export const REQUEST_ANALYTICS_RANGE_OPTIONS = [
  { key: "today", labelKey: "admin.requestAnalysis.ranges.today" },
  { key: "7d", labelKey: "admin.requestAnalysis.ranges.last7Days" },
  { key: "30d", labelKey: "admin.requestAnalysis.ranges.last30Days" },
] as const;

export interface AnalyticsBreakdownItem extends GatewayLogAnalyticsBucket {
  label: string;
}

export interface AnalyticsGeoRegionBucket extends GatewayLogAnalyticsBucket {
  country_code?: string;
  province?: string;
  city?: string;
}

type Translator = (key: string, params?: Record<string, unknown>) => string;

const localDateString = (date: Date) => {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const todayDateString = () => localDateString(new Date());

export const subtractCalendarDays = (date: string, days: number) => {
  const value = new Date(`${date}T00:00:00Z`);
  value.setUTCDate(value.getUTCDate() - days);
  return value.toISOString().slice(0, 10);
};

export const resolveAnalyticsRange = (
  key: RequestAnalyticsRangeKey,
  today = todayDateString(),
) => {
  const days = key === "today" ? 1 : key === "7d" ? 7 : 30;
  return { from: subtractCalendarDays(today, days - 1), to: today };
};

export const analyticsRangeDays = (from: string, to: string) => {
  const start = Date.parse(`${from}T00:00:00Z`);
  const end = Date.parse(`${to}T00:00:00Z`);
  if (!Number.isFinite(start) || !Number.isFinite(end)) return 0;
  return Math.floor((end - start) / 86_400_000) + 1;
};

const isCalendarDate = (value: string) => {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(`${value}T00:00:00Z`);
  return (
    !Number.isNaN(parsed.getTime()) &&
    parsed.toISOString().slice(0, 10) === value
  );
};

export const isValidAnalyticsRange = (
  from: string,
  to: string,
  latestDate = todayDateString(),
) =>
  isCalendarDate(from) &&
  isCalendarDate(to) &&
  from <= to &&
  to <= latestDate &&
  analyticsRangeDays(from, to) <= 30;

export const formatAnalyticsNumber = (value: number, locale: string) =>
  new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(
    Number.isFinite(value) ? value : 0,
  );

export const formatAnalyticsPercent = (value: number, locale: string) =>
  new Intl.NumberFormat(locale, {
    style: "percent",
    maximumFractionDigits: value > 0 && value < 0.01 ? 2 : 1,
  }).format(Number.isFinite(value) ? value : 0);

export const formatAnalyticsDuration = (value: number, locale: string) => {
  if (!Number.isFinite(value) || value <= 0) return "0 ms";
  if (value < 1000) {
    return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 0 }).format(value)} ms`;
  }
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(value / 1000)} s`;
};

export const formatAnalyticsBytes = (value: number, locale: string) => {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(
    units.length - 1,
    Math.floor(Math.log(value) / Math.log(1024)),
  );
  const normalized = value / 1024 ** exponent;
  return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 2 }).format(normalized)} ${units[exponent]}`;
};

export const analyticsTimestampOffsetMinutes = (value: string) => {
  if (/z$/iu.test(value.trim())) return 0;
  const match = value.trim().match(/([+-])(\d{2}):(\d{2})$/u);
  if (!match) return 0;
  const hours = Number(match[2]);
  const minutes = Number(match[3]);
  if (hours > 23 || minutes > 59) return 0;
  const offset = hours * 60 + minutes;
  return match[1] === "-" ? -offset : offset;
};

const specialLabelKeys: Record<string, string> = {
  bot: "admin.requestAnalysis.values.bot",
  desktop: "admin.requestAnalysis.values.desktop",
  mobile: "admin.requestAnalysis.values.mobile",
  tablet: "admin.requestAnalysis.values.tablet",
  tv: "admin.requestAnalysis.values.tv",
  unknown: "admin.requestAnalysis.values.unknown",
  direct: "admin.requestAnalysis.values.direct",
  none: "admin.requestAnalysis.values.none",
  blocked: "admin.requestAnalysis.values.blocked",
  hit: "admin.requestAnalysis.values.hit",
  block: "admin.wafLogs.actions.block",
  deny: "admin.wafLogs.actions.block",
  log: "admin.wafLogs.actions.record",
  detect: "admin.wafLogs.actions.record",
  pass: "admin.wafLogs.actions.pass",
  lt_50: "admin.requestAnalysis.latencyBands.lt50",
  "50_100": "admin.requestAnalysis.latencyBands.from50To100",
  "100_250": "admin.requestAnalysis.latencyBands.from100To250",
  "250_500": "admin.requestAnalysis.latencyBands.from250To500",
  "500_1000": "admin.requestAnalysis.latencyBands.from500To1000",
  gte_1000: "admin.requestAnalysis.latencyBands.gte1000",
};

export const analyticsDimensionLabel = (key: string, t: Translator) => {
  const normalized = key.trim();
  const normalizedKey = normalized.toLowerCase();
  const translationKey =
    AUTH_DECISION_LABEL_KEYS[normalizedKey] || specialLabelKeys[normalizedKey];
  return translationKey
    ? t(translationKey)
    : normalized || t("admin.requestAnalysis.values.unknown");
};

export const analyticsRegionLabel = (
  item: AnalyticsGeoRegionBucket,
  locale: string,
  t: Translator,
) => {
  if (item.key === "unknown") return t("admin.requestAnalysis.values.unknown");
  const parts = [
    item.country_code
      ? analyticsCountryLabel(item.country_code, locale, t)
      : "",
    item.province?.trim() || "",
    item.city?.trim() || "",
  ].filter((value, index, values) => {
    if (!value) return false;
    return values.findIndex((candidate) => candidate === value) === index;
  });
  return parts.join(" · ") || t("admin.requestAnalysis.values.unknown");
};

export const analyticsCountryLabel = (
  key: string,
  locale: string,
  t: Translator,
) => {
  if (key === "unknown") return t("admin.requestAnalysis.values.unknown");
  try {
    return new Intl.DisplayNames([locale], { type: "region" }).of(key) || key;
  } catch {
    return key;
  }
};

export const mapAnalyticsBuckets = (
  items: GatewayLogAnalyticsBucket[],
  label: (key: string) => string,
): AnalyticsBreakdownItem[] =>
  items.map((item) => ({ ...item, label: label(item.key) }));
