import { ref, watch, type Ref } from "vue";

export type MappingStatusTooltip =
  | "availability"
  | "default-domain"
  | "authentication"
  | "waf"
  | "visibility"
  | "toolbar"
  | "advanced-auth"
  | "location-rules";

export const useSubdomainTouchTooltips = ({
  isTouchInteraction,
  shouldShowPortalDisabledTooltip,
}: {
  isTouchInteraction: Ref<boolean>;
  shouldShowPortalDisabledTooltip: Ref<boolean>;
}) => {
  const openMappingStatusTooltipKey = ref<string | null>(null);
  const isPortalDisabledTooltipOpen = ref(false);

  const getMappingStatusTooltipKey = (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => `${host}\u0000${tooltip}`;

  const isMappingStatusTooltipOpen = (
    host: string,
    tooltip: MappingStatusTooltip,
  ): boolean =>
    openMappingStatusTooltipKey.value ===
    getMappingStatusTooltipKey(host, tooltip);

  const handleMappingStatusTooltipOpenChange = (
    host: string,
    tooltip: MappingStatusTooltip,
    nextOpen: boolean,
  ) => {
    const key = getMappingStatusTooltipKey(host, tooltip);

    if (nextOpen) {
      openMappingStatusTooltipKey.value = key;
      return;
    }

    if (openMappingStatusTooltipKey.value === key) {
      openMappingStatusTooltipKey.value = null;
    }
  };

  const handleMappingStatusTooltipTriggerClick = (
    host: string,
    tooltip: MappingStatusTooltip,
  ) => {
    if (!isTouchInteraction.value) {
      return;
    }

    const key = getMappingStatusTooltipKey(host, tooltip);
    openMappingStatusTooltipKey.value =
      openMappingStatusTooltipKey.value === key ? null : key;
  };

  const handlePortalDisabledTooltipOpenChange = (nextOpen: boolean) => {
    isPortalDisabledTooltipOpen.value = nextOpen;
  };

  const handlePortalDisabledTooltipTriggerClick = () => {
    if (!shouldShowPortalDisabledTooltip.value || !isTouchInteraction.value) {
      return;
    }

    isPortalDisabledTooltipOpen.value = !isPortalDisabledTooltipOpen.value;
  };

  watch(shouldShowPortalDisabledTooltip, (visible) => {
    if (!visible) {
      isPortalDisabledTooltipOpen.value = false;
    }
  });

  return {
    handleMappingStatusTooltipOpenChange,
    handleMappingStatusTooltipTriggerClick,
    handlePortalDisabledTooltipOpenChange,
    handlePortalDisabledTooltipTriggerClick,
    isMappingStatusTooltipOpen,
    isPortalDisabledTooltipOpen,
  };
};
