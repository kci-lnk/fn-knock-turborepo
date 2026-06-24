import type {
  DDNSHttpTransport,
  DDNSSettings,
  DDNSStoredSettings,
} from "./types";
import {
  buildDefaultDDNSPublicCheckSources,
  normalizeDDNSPublicCheckSources,
} from "./public-check-sources";
import {
  getDefaultUpdateIntervalMinutes,
  normalizeUpdateIntervalMinutes,
} from "./update-interval";

const isRecord = (value: unknown): value is Record<string, unknown> =>
  !!value && typeof value === "object" && !Array.isArray(value);

export const DEFAULT_DDNS_HTTP_TRANSPORT: DDNSHttpTransport = "curl";

export function normalizeDDNSHttpTransport(
  value: unknown,
): DDNSHttpTransport {
  if (value === "node" || value === "fetch") {
    return "node";
  }
  return DEFAULT_DDNS_HTTP_TRANSPORT;
}

function buildDefaultStoredSettings(): DDNSStoredSettings {
  return {
    updateIntervalMinutes: getDefaultUpdateIntervalMinutes(),
    publicCheckSources: buildDefaultDDNSPublicCheckSources(),
    httpTransport: DEFAULT_DDNS_HTTP_TRANSPORT,
  };
}

function withDefaultPublicCheckSources(
  settings: DDNSStoredSettings,
): DDNSSettings {
  return {
    ...settings,
    defaultPublicCheckSources: buildDefaultDDNSPublicCheckSources(),
  };
}

export function normalizeStoredDDNSSettings(value: unknown): DDNSSettings {
  if (!isRecord(value)) {
    return withDefaultPublicCheckSources(buildDefaultStoredSettings());
  }

  const defaults = buildDefaultStoredSettings();
  let publicCheckSources = defaults.publicCheckSources;

  try {
    publicCheckSources = normalizeDDNSPublicCheckSources(
      value.publicCheckSources,
    );
  } catch {
    publicCheckSources = buildDefaultDDNSPublicCheckSources();
  }

  return withDefaultPublicCheckSources({
    updateIntervalMinutes: normalizeUpdateIntervalMinutes(
      value.updateIntervalMinutes,
    ),
    publicCheckSources,
    httpTransport: normalizeDDNSHttpTransport(value.httpTransport),
  });
}

export function parseDDNSSettingsRaw(
  raw: string | null | undefined,
): DDNSSettings {
  if (!raw) {
    return withDefaultPublicCheckSources(buildDefaultStoredSettings());
  }

  try {
    return normalizeStoredDDNSSettings(JSON.parse(raw));
  } catch {
    return withDefaultPublicCheckSources(buildDefaultStoredSettings());
  }
}
