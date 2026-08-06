export const POW_DIFFICULTY_MIN = 10_000;
export const POW_DIFFICULTY_MAX = 1_000_000;
export const POW_DIFFICULTY_STEP = 10_000;

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
