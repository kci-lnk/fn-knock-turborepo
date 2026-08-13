import type {
  DiscoveredServiceInfo,
  ScanDiscoverResponse,
} from "@/lib/api/scan";
import type { HostMapping } from "@/types";

export {
  formatHostMappingAvailabilityWindow,
  getAvailabilityWindowValidationError,
  getHostMappingAvailabilityState,
  isAvailabilityWindowOpen,
  isAvailabilityWindowValid,
  isHostMappingUnavailable,
  normalizeHostMappingAvailability,
  parseAvailabilityTimeToMinutes,
  type HostMappingAvailabilityState,
} from "@/lib/host-mapping-availability";

export type MappingInputMode = "subdomain" | "full_host";

export type DiscoveredHostService = DiscoveredServiceInfo & {
  suggestedSubdomain: string;
};

export type DiscoveredHostResponse = Omit<ScanDiscoverResponse, "services"> & {
  services: DiscoveredHostService[];
};

export type EdgeClientIpProvider = "aliyun_esa" | "tencent_edgeone";

export const DEFAULT_AUTH_SUBDOMAIN = "auth";
export const DEFAULT_ACCESS_MODE: HostMapping["access_mode"] = "login_first";
export const DEFAULT_PROTOCOL_MODE: HostMapping["protocol_mode"] = "auto";
export const DEFAULT_TARGET_PATH_MODE: HostMapping["target_path_mode"] =
  "entry";
export const HOME_ASSISTANT_TARGET_PORT = 8123;

export type DeleteDialogState =
  | {
      kind: "clear_all";
      step: 1 | 2;
    }
  | {
      kind: "mapping";
      host: string;
    };

export type TranslationParams = Record<string, string | number>;

export interface TranslationSpec {
  key: string;
  params?: TranslationParams;
}

export interface DeleteDialogCopy {
  title: TranslationSpec;
  description: TranslationSpec;
  confirmLabel: TranslationSpec;
}
