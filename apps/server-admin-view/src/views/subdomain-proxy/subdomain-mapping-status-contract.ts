import type { HostMapping } from "@/types";
import type { HostMappingAvailabilityState } from "./model";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";

export interface SubdomainMappingStatusIndicatorsProps {
  availabilityState: HostMappingAvailabilityState;
  availabilityWindow: string;
  formatHost: (host: string) => string;
  handleMappingStatusTooltipOpenChange: (
    host: string,
    tooltip: MappingStatusTooltip,
    open: boolean,
  ) => void;
  handleMappingStatusTooltipTriggerClick: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => void;
  globalVisibilityEnabled: boolean;
  globalWafEnabled: boolean;
  isAuthService: boolean;
  isGatewayPortalEnabled: boolean;
  isDefaultDomainAvailable: boolean;
  isMappingStatusTooltipOpen: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => boolean;
  mapping: HostMapping;
}
