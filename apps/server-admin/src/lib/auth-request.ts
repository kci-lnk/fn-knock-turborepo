export type AuthClientInfo = {
  ip: string;
};

export const getClientIp = (request: Request): string =>
  request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "::1";

export const buildClientInfo = (clientIp: string): AuthClientInfo => ({
  ip: clientIp,
});
