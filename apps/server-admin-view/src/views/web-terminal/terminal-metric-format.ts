const number = (value: number, locale: string) =>
  new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(value);
export const formatPercent = (
  value: number | null | undefined,
  locale: string,
) => (value == null ? "—" : `${number(value, locale)}%`);
export const formatBytes = (
  value: number | null | undefined,
  locale: string,
) => {
  if (value == null) return "—";
  const units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
  const index = Math.min(
    Math.floor(Math.log2(Math.max(value, 1)) / 10),
    units.length - 1,
  );
  return `${number(value / 1024 ** index, locale)} ${units[index]}`;
};
