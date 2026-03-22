import { normalizeIp } from "./ip-normalize";

export type AuthClientInfo = {
  ip: string;
};

export const getClientIp = (request: Request): string => {
  const forwarded = request.headers.get("x-forwarded-for");
  const forwardedIp = normalizeIp(forwarded?.split(",")[0]?.trim());
  if (forwardedIp) return forwardedIp;

  const realIp = normalizeIp(request.headers.get("x-real-ip"));
  if (realIp) return realIp;

  return "";
};

export const buildClientInfo = (clientIp: string): AuthClientInfo => ({
  ip: clientIp || "unknown",
});
