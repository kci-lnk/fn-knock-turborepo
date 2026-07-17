export type SessionDurationUnit =
  | "second"
  | "minute"
  | "hour"
  | "day"
  | "week"
  | "year";

export type SessionDurationField = {
  value: number;
  unit: SessionDurationUnit;
};

export type SessionDurationUnitOption = {
  value: SessionDurationUnit;
  labelKey: string;
  seconds: number;
};

export const durationUnits: SessionDurationUnitOption[] = [
  {
    value: "second",
    labelKey: "admin.sessionSettings.units.second",
    seconds: 1,
  },
  {
    value: "minute",
    labelKey: "admin.sessionSettings.units.minute",
    seconds: 60,
  },
  {
    value: "hour",
    labelKey: "admin.sessionSettings.units.hour",
    seconds: 3600,
  },
  {
    value: "day",
    labelKey: "admin.sessionSettings.units.day",
    seconds: 24 * 3600,
  },
  {
    value: "week",
    labelKey: "admin.sessionSettings.units.week",
    seconds: 7 * 24 * 3600,
  },
  {
    value: "year",
    labelKey: "admin.sessionSettings.units.year",
    seconds: 365 * 24 * 3600,
  },
];

export const ipGrantDurationUnits = durationUnits.filter(
  (unit) =>
    unit.value === "second" || unit.value === "minute" || unit.value === "hour",
);

export const mobilityWindowDurationUnits = durationUnits.filter(
  (unit) => unit.value === "minute" || unit.value === "hour",
);

const durationUnitMap = Object.fromEntries(
  durationUnits.map((item) => [item.value, item.seconds]),
) as Record<SessionDurationUnit, number>;

const clampDurationValue = (value: unknown) =>
  Math.max(1, Math.floor(Number(value) || 0));

export const toDurationSeconds = (field: SessionDurationField): number =>
  clampDurationValue(field.value) * durationUnitMap[field.unit];

export const splitDuration = (
  seconds: number,
  units = durationUnits,
): SessionDurationField => {
  const safeSeconds = Math.max(1, Math.floor(Number(seconds) || 1));
  const matchedUnit =
    [...units].reverse().find((unit) => safeSeconds % unit.seconds === 0) ??
    units[0] ??
    durationUnits[0]!;

  return {
    value: Math.max(1, safeSeconds / matchedUnit.seconds),
    unit: matchedUnit.value,
  };
};
