import { readonly, ref } from "vue";

export const DATE_TIME_DISPLAY_MODES = ["human_friendly", "full"] as const;

export type DateTimeDisplayMode = (typeof DATE_TIME_DISPLAY_MODES)[number];

export const DEFAULT_DATE_TIME_DISPLAY_MODE: DateTimeDisplayMode =
  "human_friendly";

const dateTimeDisplayMode = ref<DateTimeDisplayMode>(
  DEFAULT_DATE_TIME_DISPLAY_MODE,
);

export const normalizeDateTimeDisplayMode = (
  value: unknown,
): DateTimeDisplayMode =>
  DATE_TIME_DISPLAY_MODES.includes(value as DateTimeDisplayMode)
    ? (value as DateTimeDisplayMode)
    : DEFAULT_DATE_TIME_DISPLAY_MODE;

export const useDateTimeDisplayState = () => ({
  dateTimeDisplayMode: readonly(dateTimeDisplayMode),
});

export const applyDateTimeDisplayMode = (value: unknown) => {
  const normalized = normalizeDateTimeDisplayMode(value);
  dateTimeDisplayMode.value = normalized;
  return normalized;
};

export const applyDateTimeDisplayConfig = (
  value?: { date_time_display_mode?: unknown } | null,
) => applyDateTimeDisplayMode(value?.date_time_display_mode);
