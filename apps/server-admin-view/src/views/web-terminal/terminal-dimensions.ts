export const TERMINAL_MIN_COLS = 40;
export const TERMINAL_MAX_COLS = 400;
export const TERMINAL_MIN_ROWS = 12;
export const TERMINAL_MAX_ROWS = 200;

const normalizeDimension = (
  value: number,
  minimum: number,
  maximum: number,
  fallback: number,
) => {
  const rounded = Number.isFinite(value) ? Math.round(value) : fallback;
  return Math.min(maximum, Math.max(minimum, rounded));
};

export const normalizeTerminalDimensions = ({
  cols,
  rows,
}: {
  cols: number;
  rows: number;
}) => ({
  cols: normalizeDimension(cols, TERMINAL_MIN_COLS, TERMINAL_MAX_COLS, 120),
  rows: normalizeDimension(rows, TERMINAL_MIN_ROWS, TERMINAL_MAX_ROWS, 32),
});
