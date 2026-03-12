import { getBooleanEnv } from "./env";

type SameSitePolicy = "Strict" | "Lax" | "None";

const resolveSameSite = (): SameSitePolicy => {
  const raw = process.env.SESSION_COOKIE_SAMESITE?.trim().toLowerCase();
  if (raw === "strict") return "Strict";
  if (raw === "none") return "None";
  return "Lax";
};

const appendSecure = (parts: string[]) => {
  const secureDefault = true;
  const secure = getBooleanEnv("SESSION_COOKIE_SECURE", secureDefault);
  if (secure) parts.push("Secure");
};

export const buildSessionCookie = (sessionId: string, maxAge: number): string => {
  const sameSite = resolveSameSite();
  const parts = [
    `x-go-reauth-proxy-session-id=${sessionId}`,
    "Path=/",
    "HttpOnly",
    `Max-Age=${maxAge}`,
    `SameSite=${sameSite}`,
  ];
  appendSecure(parts);
  return parts.join("; ");
};

export const buildSessionClearCookie = (): string => {
  const sameSite = resolveSameSite();
  const parts = [
    "x-go-reauth-proxy-session-id=",
    "Path=/",
    "HttpOnly",
    "Max-Age=0",
    `SameSite=${sameSite}`,
  ];
  appendSecure(parts);
  return parts.join("; ");
};
