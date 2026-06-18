import { isHttpProxyTargetProtocol } from "../../../../packages/admin-shared/src/utils/proxyTargetInput";

export const isHttpProxyTargetUrl = (target: string): boolean => {
  try {
    const parsed = new URL(target.trim());
    return (
      isHttpProxyTargetProtocol(parsed.protocol) && Boolean(parsed.hostname)
    );
  } catch {
    return false;
  }
};

export const resolveProxyTargetPort = (target: string): number | null => {
  try {
    const parsed = new URL(target.trim());
    if (parsed.port) {
      const port = parseInt(parsed.port, 10);
      return Number.isFinite(port) && port > 0 ? port : null;
    }

    switch (parsed.protocol) {
      case "http:":
      case "ws:":
        return 80;
      case "https:":
      case "wss:":
        return 443;
      default:
        return null;
    }
  } catch {
    return null;
  }
};
