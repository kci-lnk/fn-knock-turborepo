import { DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG } from "./config/defaults";
import type { GatewayCrawlerBlockerConfig } from "./config/types";
import { goBackend } from "./go-backend";
import { tDefault } from "./i18n";
import { normalizeGatewayCrawlerBlockerConfig } from "./config/app-config";

const gatewayCrawlerBlockerT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.gatewayCrawlerBlocker.${key}`, params);

export const normalizeGatewayCrawlerBlockerConfigForSync = (
  config?: Partial<GatewayCrawlerBlockerConfig> | null,
): GatewayCrawlerBlockerConfig => normalizeGatewayCrawlerBlockerConfig(config);

export const syncGatewayCrawlerBlockerToGateway = async (
  config?: Partial<GatewayCrawlerBlockerConfig> | null,
): Promise<GatewayCrawlerBlockerConfig> => {
  const next = normalizeGatewayCrawlerBlockerConfigForSync(
    config ?? DEFAULT_GATEWAY_CRAWLER_BLOCKER_CONFIG,
  );
  const response = await goBackend.setCrawlerBlockerConfig(next);
  if (!response.success) {
    throw new Error(response.message || gatewayCrawlerBlockerT("syncFailed"));
  }
  return next;
};
