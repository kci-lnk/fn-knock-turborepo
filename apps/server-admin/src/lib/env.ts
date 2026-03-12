import { randomBytes } from "node:crypto";

const TRUE_VALUES = new Set(["1", "true", "yes", "on"]);
const FALSE_VALUES = new Set(["0", "false", "no", "off"]);
const WARNED_MISSING_ENV = new Set<string>();

export const getRequiredEnv = (name: string): string => {
  const value = process.env[name]?.trim();
  if (value) return value;

  const generated = randomBytes(32).toString("hex");
  process.env[name] = generated;

  if (!WARNED_MISSING_ENV.has(name)) {
    WARNED_MISSING_ENV.add(name);
    console.warn(
      `[config] ${name} is missing. Generated an ephemeral secret for this process. Set ${name} explicitly in production.`,
    );
  }
  return generated;
};

export const getBooleanEnv = (name: string, fallback: boolean): boolean => {
  const raw = process.env[name];
  if (raw == null || raw.trim() === "") return fallback;
  const normalized = raw.trim().toLowerCase();
  if (TRUE_VALUES.has(normalized)) return true;
  if (FALSE_VALUES.has(normalized)) return false;
  return fallback;
};
