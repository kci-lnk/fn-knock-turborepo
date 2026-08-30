import type {
  ComponentPublicInstance,
  StyleValue,
  UnwrapNestedRefs,
} from "vue";
import type { HostMapping, HostMappingGroup } from "@/types";
import type { MappingInputMode } from "./model";
import type { useMappingIcon } from "./useMappingIcon";
import type { useMappingVisibility } from "./useMappingVisibility";
import type { useStaticPathBrowser } from "./useStaticPathBrowser";

export interface SubdomainMappingDialogProps {
  basicAuthInjection: boolean;
  basicAuthValidationMessage: string;
  canRefreshMappingMetadata: boolean;
  canShowBasicAuthInjection: boolean;
  canUseRootDomainSuffix: boolean;
  composedPreviewHost: string;
  contentStyle: StyleValue;
  fullHostInputHint: string;
  gatewayHostResponseBlockedReason: string;
  gatewayProxyHeadersBlockedReason: string;
  globalWafEnabled: boolean;
  groups: HostMappingGroup[];
  handleFocusIn: (event: FocusEvent) => void;
  handleInputModeChange: (mode: MappingInputMode) => void;
  handlePortalDisabledTooltipOpenChange: (open: boolean) => void;
  handlePortalDisabledTooltipTriggerClick: () => void;
  isGatewayAdvancedLoading: boolean;
  iconEditor: UnwrapNestedRefs<ReturnType<typeof useMappingIcon>>;
  isMappingAuthService: boolean;
  isMappingValid: boolean;
  isMappingWebSocketTarget: boolean;
  isPortalDisabledTooltipOpen: boolean;
  isRefreshingMappingMetadata: boolean;
  isSavingMappings: boolean;
  mappingForm: HostMapping;
  mappingInputLabel: string;
  mappingInputMode: MappingInputMode;
  mappingModeDescription: string;
  mappingResolvedTitle: string;
  mappingSubdomain: string;
  mappingUseAuth: boolean;
  open: boolean;
  pathBrowserEditor: UnwrapNestedRefs<ReturnType<typeof useStaticPathBrowser>>;
  preserveHost: boolean;
  refreshMappingMetadata: () => void | Promise<unknown>;
  savedRootDomain: string;
  scrollStyle: StyleValue;
  sendProxyHeaders: boolean;
  setBasicAuthInjection: (value: boolean) => void;
  setMappingSubdomain: (value: string) => void;
  setMappingUseAuth: (value: boolean) => void;
  setPreserveHost: (value: boolean) => void;
  setScrollElement: (element: Element | ComponentPublicInstance | null) => void;
  setSendProxyHeaders: (value: boolean) => void;
  setShowToolbar: (value: boolean) => void;
  shouldShowPortalDisabledTooltip: boolean;
  showToolbar: boolean;
  updateMappingBasicAuth: (patch: Partial<HostMapping["basic_auth"]>) => void;
  updateMappingForm: (patch: Partial<HostMapping>) => void;
  visibilityEditor: UnwrapNestedRefs<ReturnType<typeof useMappingVisibility>>;
}

export type SubdomainMappingDialogEmits = {
  close: [];
  save: [];
  "update:open": [value: boolean];
};
