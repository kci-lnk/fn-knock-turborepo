import { fetchWithRelaxedTls } from "./relaxed-tls-fetch";

const DEFAULT_BASIC_AUTH_PROBE_TIMEOUT_MS = 3000;
const BASIC_AUTH_PROBE_USER_AGENT = "fn-knock-server-admin-basic-auth-probe/1.0";

export interface BasicAuthProbeResult {
  requiresBasicAuth: boolean;
  httpStatus: number | null;
  error?: string;
}

export const hasBasicAuthChallenge = (
  wwwAuthenticate?: string | null,
): boolean => {
  if (!wwwAuthenticate) return false;
  return /(?:^|,)\s*basic(?:\s|,|$)/i.test(wwwAuthenticate);
};

export const headersRequireBasicAuth = (
  headers?: Record<string, string> | null,
): boolean => hasBasicAuthChallenge(headers?.["www-authenticate"]);

const normalizeHttpProbeUrl = (value: string): string => {
  const trimmed = value.trim();
  if (!trimmed) return "";

  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "";
    }
    parsed.hash = "";
    return parsed.toString();
  } catch {
    return "";
  }
};

export const probeBasicAuthTarget = async (
  inputUrl: string,
  timeoutMs = DEFAULT_BASIC_AUTH_PROBE_TIMEOUT_MS,
): Promise<BasicAuthProbeResult> => {
  const normalizedUrl = normalizeHttpProbeUrl(inputUrl);
  if (!normalizedUrl) {
    return {
      requiresBasicAuth: false,
      httpStatus: null,
      error: "Only http/https targets are supported",
    };
  }

  try {
    const response = await fetchWithRelaxedTls(normalizedUrl, {
      headers: {
        Accept: "text/html,application/xhtml+xml,*/*;q=0.8",
        "User-Agent": BASIC_AUTH_PROBE_USER_AGENT,
        Connection: "close",
      },
      redirect: "follow",
      signal: AbortSignal.timeout(timeoutMs),
    });

    return {
      requiresBasicAuth: hasBasicAuthChallenge(
        response.headers.get("www-authenticate"),
      ),
      httpStatus: response.status,
    };
  } catch (error) {
    return {
      requiresBasicAuth: false,
      httpStatus: null,
      error:
        error instanceof Error
          ? error.message
          : "Failed to probe Basic Auth challenge",
    };
  }
};
