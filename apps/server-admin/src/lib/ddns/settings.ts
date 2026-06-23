import type { DDNSSettings, DDNSStoredSettings } from "./types";
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

function buildDefaultStoredSettings(): DDNSStoredSettings {
  return {
    updateIntervalMinutes: getDefaultUpdateIntervalMinutes(),
    publicCheckSources: buildDefaultDDNSPublicCheckSources(),
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
