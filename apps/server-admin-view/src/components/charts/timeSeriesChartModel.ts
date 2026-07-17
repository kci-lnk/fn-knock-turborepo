import type uPlot from "uplot";

export type TimeSeriesPoint = readonly [
  number | string | Date,
  number | null | undefined,
];

export type TimeSeriesChartSeries = {
  name: string;
  data: readonly TimeSeriesPoint[];
  color: string;
  fill?: string;
  width?: number;
};

export type TimeSeriesLegendItem = {
  name: string;
  color: string;
};

export const toTimeSeriesTimestampMs = (
  value: TimeSeriesPoint[0],
): number | null => {
  if (value instanceof Date) {
    const time = value.getTime();
    return Number.isFinite(time) ? time : null;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value)) return null;
    return Math.abs(value) < 100_000_000_000 ? value * 1000 : value;
  }
  const parsed = new Date(value).getTime();
  return Number.isFinite(parsed) ? parsed : null;
};

const normalizeNumber = (
  value: number | null | undefined,
): number | null => {
  if (value === null || value === undefined) return null;
  const normalized = Number(value);
  return Number.isFinite(normalized) ? normalized : null;
};

export const alignTimeSeriesData = (
  series: readonly TimeSeriesChartSeries[],
): uPlot.AlignedData => {
  const xSet = new Set<number>();
  const seriesMaps = series.map((item) => {
    const points = new Map<number, number | null>();
    for (const point of item.data) {
      const x = toTimeSeriesTimestampMs(point[0]);
      if (x === null) continue;
      xSet.add(x);
      points.set(x, normalizeNumber(point[1]));
    }
    return points;
  });
  const xValues = Array.from(xSet).sort((left, right) => left - right);
  return [
    xValues,
    ...seriesMaps.map((points) =>
      xValues.map((x) => points.get(x) ?? null),
    ),
  ] as uPlot.AlignedData;
};

export const hasRenderableTimeSeriesData = (data: uPlot.AlignedData) =>
  data
    .slice(1)
    .some((values) =>
      values.some((value) => value !== null && value !== undefined),
    );

export const buildTimeSeriesLegendItems = (
  series: readonly TimeSeriesChartSeries[],
): TimeSeriesLegendItem[] =>
  series
    .map((item, index) => ({
      name: item.name.trim() || `Series ${index + 1}`,
      color: item.color,
    }))
    .filter((item) => item.name);
