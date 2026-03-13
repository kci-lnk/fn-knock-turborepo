import { getBooleanEnv } from "./env";

type SameSitePolicy = "Strict" | "Lax" | "None";

export const FNOS_SHARE_SESSION_COOKIE_NAME = "fn-knock-fnos-share-session";

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

const buildCookie = ({
  name,
  value,
  maxAge,
  path,
  httpOnly = true,
}: {
  name: string;
  value: string;
  maxAge: number;
  path: string;
  httpOnly?: boolean;
}): string => {
  const sameSite = resolveSameSite();
  const parts = [
    `${name}=${value}`,
    `Path=${path}`,
    `SameSite=${sameSite}`,
    `Max-Age=${maxAge}`,
  ];
  if (httpOnly) parts.splice(2, 0, "HttpOnly");
  appendSecure(parts);
  return parts.join("; ");
};

export const buildSessionCookie = (sessionId: string, maxAge: number): string =>
  buildCookie({
    name: "x-go-reauth-proxy-session-id",
    value: sessionId,
    maxAge,
    path: "/",
  });

export const buildSessionClearCookie = (): string => {
  return buildCookie({
    name: "x-go-reauth-proxy-session-id",
    value: "",
    maxAge: 0,
    path: "/",
  });
};

export const buildFnosShareSessionCookie = (
  sessionId: string,
  maxAge: number,
): string =>
  buildCookie({
    name: FNOS_SHARE_SESSION_COOKIE_NAME,
    value: sessionId,
    maxAge,
    path: "/s",
  });

export const buildFnosShareSessionClearCookie = (): string =>
  buildCookie({
    name: FNOS_SHARE_SESSION_COOKIE_NAME,
    value: "",
    maxAge: 0,
    path: "/s",
  });
