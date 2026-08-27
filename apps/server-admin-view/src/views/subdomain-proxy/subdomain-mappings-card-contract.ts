import type { ButtonVariants } from "@/components/ui/button";
import type { HostMapping, HostMappingGroup, HostTrafficStats } from "@/types";
import type { HostMappingGroupSection } from "./host-mapping-groups";
import type { HostMappingAvailabilityState } from "./model";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";

export interface SubdomainMappingsCardProps {
  activeDeepMonitorHosts: string[];
  allMappingsCount: number;
  allRegularMappings: HostMapping[];
  authServiceMapping: HostMapping | null;
  canManageNewMappings: boolean;
  canUseDeepMonitor: boolean;
  discoverButtonDividerClass: string;
  discoverButtonVariant: ButtonVariants["variant"];
  docsHref: string;
  draggableMappings: HostMapping[];
  filteredMappings: HostMapping[];
  formatHost: (host: string) => string;
  formatAvailabilityWindow: (mapping: HostMapping) => string;
  getAvailabilityState: (mapping: HostMapping) => HostMappingAvailabilityState;
  getHostTrafficSample: (host: string) => HostTrafficStats | null;
  getMappingTitleForDisplay: (mapping: HostMapping) => string;
  globalVisibilityEnabled: boolean;
  globalWafEnabled: boolean;
  groupedView: boolean;
  groups: HostMappingGroup[];
  handleMappingStatusTooltipOpenChange: (
    host: string,
    tooltip: MappingStatusTooltip,
    open: boolean,
  ) => void;
  handleMappingStatusTooltipTriggerClick: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => void;
  handleProtocolHeadersWarningOpenChange: (host: string, open: boolean) => void;
  hasRegularHostMappings: boolean;
  isClearingAllSubdomainConfig: boolean;
  isConfigLoading: boolean;
  isDiscovering: boolean;
  isExportingBookmarks: boolean;
  isFaviconBroken: (mapping: HostMapping) => boolean;
  isGatewayPortalEnabled: boolean;
  isDefaultDomainAvailable: boolean;
  isMappingUnavailable: (mapping: HostMapping) => boolean;
  isMappingStatusTooltipOpen: (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => boolean;
  isProtocolHeadersWarningOpen: (host: string) => boolean;
  isRefreshingTitles: boolean;
  isRootDomainPendingSave: boolean;
  isSavingMappings: boolean;
  isSyncing: boolean;
  isAuthServiceTarget: (target: string) => boolean;
  markFaviconBroken: (mapping: HostMapping) => void;
  openProtocolHeadersWarning: (host: string) => void;
  rootDomainValidationMessage: string;
  savedRootDomain: string;
  scheduleCloseProtocolHeadersWarning: (host: string) => void;
  searchQuery: string;
  shouldShowProtocolHeadersWarning: (mapping: HostMapping) => boolean;
  toggleProtocolHeadersWarning: (host: string) => void;
  trafficTimestamp: number | null | undefined;
  visibleMappingsCount: number;
}

export type SubdomainMappingsCardEmits = {
  "add-auth-service": [];
  "batch-delete": [hosts: string[], onComplete: () => void];
  "batch-disable": [hosts: string[], onComplete: () => void];
  "batch-enable": [hosts: string[], onComplete: () => void];
  "batch-schedule": [hosts: string[], onComplete: () => void];
  "clear-default": [mapping: HostMapping];
  "copy-host": [mapping: HostMapping];
  delete: [host: string];
  edit: [mapping: HostMapping];
  "export-bookmarks": [];
  "open-clear-all-config": [];
  "move-mappings": [
    hosts: string[],
    groupId: string | null,
    onComplete?: () => void,
  ];
  "open-create": [groupId?: string | null];
  "open-discover": [];
  "open-discover-settings": [];
  "open-availability": [mapping: HostMapping];
  "open-gateway-locations": [host: string];
  "open-advanced-auth": [host: string];
  "open-deep-monitor": [host: string];
  "open-stale-cleanup": [];
  "open-target-optimization": [];
  "refresh-all-titles": [];
  "save-order": [];
  "save-grouped-order": [sections: HostMappingGroupSection[]];
  "save-groups": [
    groups: HostMappingGroup[],
    onComplete: (saved: boolean) => void,
  ];
  "set-default": [mapping: HostMapping];
  "sync-routes": [];
  "toggle-enabled": [mapping: HostMapping];
  "update-grouped-view": [value: boolean];
  "update:draggableMappings": [mappings: HostMapping[]];
  "update:searchQuery": [value: string];
};

export interface SubdomainMappingsTableActions {
  clearDefault: (mapping: HostMapping) => void;
  batchDelete: (hosts: string[], onComplete: () => void) => void;
  batchDisable: (hosts: string[], onComplete: () => void) => void;
  batchEnable: (hosts: string[], onComplete: () => void) => void;
  batchSchedule: (hosts: string[], onComplete: () => void) => void;
  copyHost: (mapping: HostMapping) => void;
  deleteMapping: (host: string) => void;
  edit: (mapping: HostMapping) => void;
  manageGroups: () => void;
  moveMappings: (
    hosts: string[],
    groupId: string | null,
    onComplete?: () => void,
  ) => void;
  openAdvancedAuth: (host: string) => void;
  openAvailability: (mapping: HostMapping) => void;
  openCreate: (groupId?: string | null) => void;
  openDeepMonitor: (host: string) => void;
  openGatewayLocations: (host: string) => void;
  saveFlatOrder: (mappings: HostMapping[]) => void;
  saveGroupedOrder: (sections: HostMappingGroupSection[]) => void;
  setDefault: (mapping: HostMapping) => void;
  toggleEnabled: (mapping: HostMapping) => void;
}
