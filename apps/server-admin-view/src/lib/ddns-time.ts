import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";

export const buildDDNSTimestampTooltipLines = (input: {
  updatedAt: string | null | undefined;
  checkedAt: string | null | undefined;
  locale?: string;
  labels: {
    lastSuccessfulUpdate: string;
    lastCheck: string;
    never: string;
  };
}) => {
  const locale = input.locale || "en";

  return [
    `${input.labels.lastSuccessfulUpdate}: ${formatDateTimeSafe(input.updatedAt, {
      locale,
      emptyText: input.labels.never,
    })}`,
    `${input.labels.lastCheck}: ${formatDateTimeSafe(input.checkedAt, {
      locale,
      emptyText: input.labels.never,
    })}`,
  ];
};
