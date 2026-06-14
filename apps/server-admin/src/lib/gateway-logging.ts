import { goBackend, type GatewayLoggingConfig } from "./go-backend";
import { configManager, type GatewayLoggingSettings } from "./redis";
import { tDefault } from "./i18n";

const gatewayLoggingT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.gatewayLogging.${key}`, params);

export const getGatewayLoggingConfigForResponse = async (
  settings?: GatewayLoggingSettings | null,
): Promise<GatewayLoggingConfig> => {
  const persisted = settings ?? (await configManager.getGatewayLoggingConfig());
  const directory = await goBackend.getGatewayLoggingDirectory();

  return {
    enabled: persisted.enabled,
    max_days: persisted.max_days,
    logs_dir:
      directory.success && directory.data?.logs_dir
        ? directory.data.logs_dir
        : "",
  };
};

export const syncGatewayLoggingToGateway = async (
  settings?: GatewayLoggingSettings | null,
): Promise<GatewayLoggingConfig> => {
  const next = settings ?? (await configManager.getGatewayLoggingConfig());
  const response = await goBackend.setGatewayLoggingConfig(next);

  if (!response.success) {
    throw new Error(response.message || gatewayLoggingT("syncConfigFailed"));
  }

  return {
    enabled: next.enabled,
    max_days: next.max_days,
    logs_dir: response.data?.logs_dir || "",
  };
};
