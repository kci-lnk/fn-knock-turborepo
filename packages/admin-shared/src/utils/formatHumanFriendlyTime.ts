type HumanFriendlyTimeOptions = {
  locale?: string;
  emptyText?: string;
  keepInvalidRawText?: boolean;
  now?: number;
};

const SECOND = 1000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;
const MONTH = 30 * DAY;
const YEAR = 365 * DAY;

export const resolveDateValue = (
  value: string | number | Date | null | undefined,
): Date | null => {
  if (value === null || value === undefined || value === '') return null;

  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) return null;
  return date;
};

export const formatHumanFriendlyTime = (
  value: string | number | Date | null | undefined,
  options: HumanFriendlyTimeOptions = {},
): string => {
  const {
    locale = 'zh-CN',
    emptyText = '-',
    keepInvalidRawText = true,
    now = Date.now(),
  } = options;

  if (value === null || value === undefined || value === '') return emptyText;

  const date = resolveDateValue(value);
  if (!date) return keepInvalidRawText ? String(value) : emptyText;

  const diff = now - date.getTime();
  const absDiff = Math.abs(diff);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });

  if (absDiff < MINUTE) {
    return rtf.format(-Math.round(diff / SECOND), 'second');
  }
  if (absDiff < HOUR) {
    return rtf.format(-Math.round(diff / MINUTE), 'minute');
  }
  if (absDiff < DAY) {
    return rtf.format(-Math.round(diff / HOUR), 'hour');
  }
  if (absDiff < MONTH) {
    return rtf.format(-Math.round(diff / DAY), 'day');
  }
  if (absDiff < YEAR) {
    return rtf.format(-Math.round(diff / MONTH), 'month');
  }
  return rtf.format(-Math.round(diff / YEAR), 'year');
};
