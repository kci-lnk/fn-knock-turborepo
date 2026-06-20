import { resolve4, resolve6 } from "node:dns/promises";
import { normalizeIp } from "../ip-normalize";
import { whitelistManagerT } from "./messages";

const isNoDataResolveError = (error: unknown): boolean => {
  const code = String((error as any)?.code || "").toUpperCase();
  return (
    code === "ENODATA" ||
    code === "ENOTFOUND" ||
    code === "EAI_NODATA" ||
    code === "EAI_NONAME"
  );
};

const formatResolveError = (label: "A" | "AAAA", error: unknown): string => {
  const code = String((error as any)?.code || "").trim();
  const message = (error as any)?.message || String(error);
  return code
    ? whitelistManagerT("dnsRecordQueryFailedWithCode", {
        label,
        code,
        message,
      })
    : whitelistManagerT("dnsRecordQueryFailed", { label, message });
};

export const resolveCnameTargets = async (
  domain: string,
): Promise<string[]> => {
  const [ipv4Result, ipv6Result] = await Promise.allSettled([
    resolve4(domain),
    resolve6(domain),
  ]);
  const hardErrors: string[] = [];
  const resolvedTargets = new Set<string>();

  if (ipv4Result.status === "fulfilled") {
    for (const ip of ipv4Result.value) {
      const normalized = normalizeIp(ip);
      if (normalized) resolvedTargets.add(normalized);
    }
  } else if (!isNoDataResolveError(ipv4Result.reason)) {
    hardErrors.push(formatResolveError("A", ipv4Result.reason));
  }

  if (ipv6Result.status === "fulfilled") {
    for (const ip of ipv6Result.value) {
      const normalized = normalizeIp(ip);
      if (normalized) resolvedTargets.add(normalized);
    }
  } else if (!isNoDataResolveError(ipv6Result.reason)) {
    hardErrors.push(formatResolveError("AAAA", ipv6Result.reason));
  }

  if (hardErrors.length > 0) {
    throw new Error(hardErrors.join("；"));
  }

  return [...resolvedTargets].sort((left, right) =>
    left.localeCompare(right),
  );
};
