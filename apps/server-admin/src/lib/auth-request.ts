import { ipLocationService } from "./ip-location";

export type AuthClientInfo = {
  ip: string;
  location: string;
};

export const getClientIp = (request: Request): string =>
  request.headers.get("x-forwarded-for")?.split(",")[0]?.trim() || "::1";

export const resolveClientLocation = async (clientIp: string): Promise<string> => {
  try {
    const lookupIp = clientIp === "::1" ? "127.0.0.1" : clientIp;
    const ipInfo = await ipLocationService.getIpLocation(lookupIp);
    return ipInfo?.raw || "";
  } catch (error) {
    console.error("Failed to query IP location:", error);
    return "";
  }
};

export const buildClientInfo = async (clientIp: string): Promise<AuthClientInfo> => ({
  ip: clientIp,
  location: await resolveClientLocation(clientIp),
});
