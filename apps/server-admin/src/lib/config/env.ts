export const parseEnvInt = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  return Number.isFinite(parsed) ? parsed : fallback;
};

export const ACME_RUNTIME_LOCK_TTL_SECONDS = Math.max(
  300,
  Math.min(6 * 60 * 60, parseEnvInt(process.env.ACME_RUNTIME_LOCK_TTL, 900)),
);
