import type { GoResponse } from "../../lib/go-backend";
import { firewallService } from "../../lib/firewall-service";
import { createRequestTranslator } from "../../lib/i18n";
import {
  type AppConfig,
  configManager,
  type ProtocolMappingFeatureConfig,
} from "../../lib/redis";
import {
  getCapabilityUnavailableMessage,
  getRuntimeProfile,
  isAdminPanelProtectedRuntime,
} from "../../lib/runtime-profile";
import { syncSmartConnect } from "../../lib/smart-connect";

export const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

export const clamp = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value));

export const getAdminRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

export type RequestTranslator = ReturnType<typeof createRequestTranslator>["t"];
type AdminMessageParams = Record<
  string,
  string | number | boolean | null | undefined
>;

export const adminT = (
  t: RequestTranslator,
  key: string,
  params?: AdminMessageParams,
) => t(`server.admin.${key}`, params);

export const buildCapabilityBlockedResponse = (
  set: { status?: number | string },
  capability: Parameters<typeof getCapabilityUnavailableMessage>[0],
) => {
  set.status = 403;
  return {
    success: false,
    message: getCapabilityUnavailableMessage(capability),
  };
};

export const isPanelAuthRuntime = () =>
  isAdminPanelProtectedRuntime(getRuntimeProfile());

export const getRunTypeLabel = (t: RequestTranslator, runType: 0 | 1 | 3) => {
  if (runType === 0) return adminT(t, "runTypes.direct");
  if (runType === 1) return adminT(t, "runTypes.reverseProxy");
  return adminT(t, "runTypes.subdomain");
};

export const isSameJsonValue = (left: unknown, right: unknown): boolean =>
  JSON.stringify(left) === JSON.stringify(right);

export const ensureGoResponseSuccess = <T>(
  response: GoResponse<T>,
  fallbackMessage: string,
): GoResponse<T> => {
  if (response.success) {
    return response;
  }

  throw new Error(response.message || fallbackMessage);
};

export const rollbackConfigAndRuntime = async (
  previousConfig: AppConfig,
  t: RequestTranslator,
  locale?: string,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreConfigFailed");
  }

  try {
    await syncSmartConnect(previousConfig, locale);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreSmartConnectFailed");
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreRuntimeFailed");
  }

  return null;
};

export const rollbackProtocolMappingFeatureAndRuntime = async (
  previousSettings: ProtocolMappingFeatureConfig,
  previousConfig: AppConfig,
  t: RequestTranslator,
  locale?: string,
): Promise<string | null> => {
  try {
    await configManager.saveConfig(previousConfig);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreProtocolConfigFailed");
  }

  try {
    await configManager.updateProtocolMappingFeatureConfig(previousSettings);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreProtocolFeatureFailed");
  }

  try {
    await syncSmartConnect(previousConfig, locale);
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreSmartConnectFailed");
  }

  try {
    await firewallService.applyRunTypeConfig(
      previousConfig.run_type,
      previousConfig.run_type,
    );
  } catch (error: any) {
    return error?.message || adminT(t, "rollback.restoreProtocolRuntimeFailed");
  }

  return null;
};

export const normalizeHostLike = (value: string | undefined | null): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");
