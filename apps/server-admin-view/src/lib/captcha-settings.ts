export const POW_DIFFICULTY_MIN = 10_000;
export const POW_DIFFICULTY_MAX = 1_000_000;
export const POW_DIFFICULTY_STEP = 10_000;
export const POW_DIFFICULTY_STANDARD = 100_000;
export const POW_DIFFICULTY_VERY_HARD = 300_000;

export const isPowDifficultyPreset = (value: number) =>
  value === POW_DIFFICULTY_STANDARD || value === POW_DIFFICULTY_VERY_HARD;

export const ensureUncommonDifficultyAtLeastBase = (
  baseMaxNumber: number,
  uncommonMaxNumber: number,
) => Math.max(baseMaxNumber, uncommonMaxNumber);

export const isPowDifficultyValid = (
  baseMaxNumber: number,
  uncommonMaxNumber: number,
) =>
  Number.isInteger(baseMaxNumber) &&
  Number.isInteger(uncommonMaxNumber) &&
  baseMaxNumber >= POW_DIFFICULTY_MIN &&
  baseMaxNumber <= POW_DIFFICULTY_MAX &&
  baseMaxNumber % POW_DIFFICULTY_STEP === 0 &&
  uncommonMaxNumber >= baseMaxNumber &&
  uncommonMaxNumber <= POW_DIFFICULTY_MAX &&
  uncommonMaxNumber % POW_DIFFICULTY_STEP === 0;
