import { ref, watch, type Ref } from "vue";

export const useSubdomainTouchTooltips = ({
  isTouchInteraction,
  shouldShowPortalDisabledTooltip,
}: {
  isTouchInteraction: Ref<boolean>;
  shouldShowPortalDisabledTooltip: Ref<boolean>;
}) => {
  const openLocationRulesTooltipHost = ref<string | null>(null);
  const isPortalDisabledTooltipOpen = ref(false);

  const isLocationRulesTooltipOpen = (host: string): boolean =>
    openLocationRulesTooltipHost.value === host;

  const handleLocationRulesTooltipOpenChange = (
    host: string,
    nextOpen: boolean,
  ) => {
    if (nextOpen) {
      openLocationRulesTooltipHost.value = host;
      return;
    }

    if (openLocationRulesTooltipHost.value === host) {
      openLocationRulesTooltipHost.value = null;
    }
  };

  const handleLocationRulesTooltipTriggerClick = (host: string) => {
    if (!isTouchInteraction.value) {
      return;
    }

    openLocationRulesTooltipHost.value =
      openLocationRulesTooltipHost.value === host ? null : host;
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
    handleLocationRulesTooltipOpenChange,
    handleLocationRulesTooltipTriggerClick,
    handlePortalDisabledTooltipOpenChange,
    handlePortalDisabledTooltipTriggerClick,
    isLocationRulesTooltipOpen,
    isPortalDisabledTooltipOpen,
  };
};
