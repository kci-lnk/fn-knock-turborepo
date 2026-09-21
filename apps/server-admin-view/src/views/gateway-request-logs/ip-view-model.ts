/** Keep the recorded offset: this clock belongs to the selected log day. */
export const formatLogClock = (value: string) =>
  value.match(/T(\d{2}:\d{2}:\d{2})/)?.[1] ?? "—";
