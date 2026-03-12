type FormatDateTimeSafeOptions = {
  locale?: string;
  formatOptions?: Intl.DateTimeFormatOptions;
  emptyText?: string;
  keepInvalidRawText?: boolean;
};

export const formatDateTimeSafe = (
  value: string | number | Date | null | undefined,
  options: FormatDateTimeSafeOptions = {},
): string => {
  const { locale, formatOptions, emptyText = '-', keepInvalidRawText = true } = options;

  if (value === null || value === undefined || value === '') return emptyText;

  const date = value instanceof Date ? value : new Date(value);
  if (Number.isNaN(date.getTime())) {
    return keepInvalidRawText ? String(value) : emptyText;
  }

  if (locale) return date.toLocaleString(locale, formatOptions);
  return date.toLocaleString(undefined, formatOptions);
};
