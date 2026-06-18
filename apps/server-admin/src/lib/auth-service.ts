import type { HostMapping } from "./redis";
import { isHttpProxyTargetProtocol } from "../../../../packages/admin-shared/src/utils/proxyTargetInput";

export const parseTargetPort = (target: string): number | null => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return null;

  try {
    const parsed = new URL(normalizedTarget);
    const port = Number.parseInt(parsed.port, 10);
    if (Number.isFinite(port) && port > 0) {
      return port;
    }
    if (parsed.protocol === "https:" || parsed.protocol === "wss:") return 443;
    if (parsed.protocol === "http:" || parsed.protocol === "ws:") return 80;
  } catch {
    // ignore and fallback below
  }

  const match = normalizedTarget.match(/:(\d+)(?:\/|$)/);
  if (!match) return null;

  const port = Number.parseInt(match[1] || "", 10);
  return Number.isFinite(port) && port > 0 ? port : null;
};

export const resolveAuthServicePort = (): number => {
  const parsed = Number.parseInt(process.env.AUTH_PORT || "7997", 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 7997;
};

const isHttpTargetUrl = (target: string): boolean => {
  try {
    const parsed = new URL(target.trim());
    return (
      isHttpProxyTargetProtocol(parsed.protocol) && Boolean(parsed.hostname)
    );
  } catch {
    return false;
  }
};

export const isAuthServiceTarget = (target: string): boolean =>
  isHttpTargetUrl(target) &&
  parseTargetPort(target) === resolveAuthServicePort();

export const isAuthServiceMapping = (
  mapping: Pick<HostMapping, "target"> | null | undefined,
): boolean => Boolean(mapping && isAuthServiceTarget(mapping.target));
