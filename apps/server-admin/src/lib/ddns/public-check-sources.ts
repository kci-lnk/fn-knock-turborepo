import { ddnsTranslate } from "./providers/helpers";
import type { DDNSPublicCheckSources } from "./types";

export const DEFAULT_DDNS_PUBLIC_CHECK_SOURCES: DDNSPublicCheckSources = {
  ipv4: ["https://4.fnknock.cn", "http://ipv4.icanhazip.com"],
  ipv6: ["https://6.fnknock.cn", "https://ipv6.icanhazip.com/"],
};

const ddnsT = ddnsTranslate;

const clonePublicCheckSources = (
  sources: DDNSPublicCheckSources,
): DDNSPublicCheckSources => ({
  ipv4: [...sources.ipv4],
  ipv6: [...sources.ipv6],
});

const isRecord = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);

const EXPLICIT_SCHEME_RE = /^([a-z][a-z0-9+.-]*):/i;
const COMPLETE_HTTP_URL_RE = /^https?:\/\//i;

const toFamilyLabel = (family: keyof DDNSPublicCheckSources) =>
  family === "ipv4" ? "IPv4" : "IPv6";

function getExplicitScheme(source: string): string | null {
  return source.match(EXPLICIT_SCHEME_RE)?.[1]?.toLowerCase() || null;
}

function buildCandidateSourceURL(source: string, familyLabel: string): string {
  const scheme = getExplicitScheme(source);
  if (!scheme) {
    return `https://${source}`;
  }

  if (scheme !== "http" && scheme !== "https") {
    throw new Error(
      ddnsT("publicCheckSourceUnsupportedProtocol", {
        family: familyLabel,
        source,
      }),
    );
  }

  if (!COMPLETE_HTTP_URL_RE.test(source)) {
    throw new Error(
      ddnsT("publicCheckSourceInvalidUrl", {
        family: familyLabel,
        source,
      }),
    );
  }

  return source;
}

export function normalizeDDNSPublicCheckSource(
  value: unknown,
  family: keyof DDNSPublicCheckSources,
): string {
  const source = String(value ?? "").trim();
  const familyLabel = toFamilyLabel(family);

  if (!source) {
    throw new Error(ddnsT("publicCheckSourceEmpty", { family: familyLabel }));
  }

  const candidate = buildCandidateSourceURL(source, familyLabel);

  let parsed: URL;
  try {
    parsed = new URL(candidate);
  } catch {
    throw new Error(
      ddnsT("publicCheckSourceInvalidUrl", {
        family: familyLabel,
        source,
      }),
    );
  }

  if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
    throw new Error(
      ddnsT("publicCheckSourceUnsupportedProtocol", {
        family: familyLabel,
        source,
      }),
    );
  }

  if (!parsed.hostname) {
    throw new Error(
      ddnsT("publicCheckSourceInvalidUrl", {
        family: familyLabel,
        source,
      }),
    );
  }

  return candidate;
}

function normalizeDDNSPublicCheckSourceList(
  value: unknown,
  family: keyof DDNSPublicCheckSources,
  fallback: string[],
): string[] {
  if (typeof value === "undefined") {
    return [...fallback];
  }

  if (!Array.isArray(value)) {
    throw new Error(
      ddnsT("publicCheckSourceInvalidUrl", {
        family: toFamilyLabel(family),
        source: String(value ?? ""),
      }),
    );
  }

  const seen = new Set<string>();
  const normalized: string[] = [];

  for (const item of value) {
    const source = normalizeDDNSPublicCheckSource(item, family);
    if (!seen.has(source)) {
      seen.add(source);
      normalized.push(source);
    }
  }

  return normalized;
}

export function normalizeDDNSPublicCheckSources(
  value: unknown,
  fallback: DDNSPublicCheckSources = DEFAULT_DDNS_PUBLIC_CHECK_SOURCES,
): DDNSPublicCheckSources {
  if (typeof value === "undefined" || value === null) {
    return clonePublicCheckSources(fallback);
  }

  if (!isRecord(value)) {
    throw new Error(
      ddnsT("publicCheckSourceInvalidUrl", {
        family: "IPv4/IPv6",
        source: String(value),
      }),
    );
  }

  return {
    ipv4: normalizeDDNSPublicCheckSourceList(value.ipv4, "ipv4", fallback.ipv4),
    ipv6: normalizeDDNSPublicCheckSourceList(value.ipv6, "ipv6", fallback.ipv6),
  };
}

export function buildDefaultDDNSPublicCheckSources(): DDNSPublicCheckSources {
  return clonePublicCheckSources(DEFAULT_DDNS_PUBLIC_CHECK_SOURCES);
}
