import { parseHostPort } from "./parseHostPort";

export const extractPortFromTarget = (target: string): number | null => {
  const normalizedTarget = target.trim();
  if (!normalizedTarget) return null;

  // URL parser handles fully qualified targets first.
  try {
    const parsed = new URL(normalizedTarget);
    return parsed.port ? Number(parsed.port) : null;
  } catch {
    // Fallback for host:port without protocol.
    return parseHostPort(normalizedTarget)?.port ?? null;
  }
};
