import { onUnmounted, ref, watch } from "vue";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";

export const useHostTrafficOverlayInteraction = () => {
  const open = ref(false);
  const dialogOpen = ref(false);
  const isTouchInteraction = useMediaQueryMatch(
    "(hover: none), (pointer: coarse), (max-width: 767px)",
  );
  const lastTriggerPointerType = ref<string | null>(null);
  const suppressNextFocusOpen = ref(false);
  let closeTimer: number | null = null;

  const clearCloseTimer = () => {
    if (closeTimer === null) return;
    window.clearTimeout(closeTimer);
    closeTimer = null;
  };

  const openPanel = () => {
    if (isTouchInteraction.value) return;
    clearCloseTimer();
    open.value = true;
  };

  const scheduleClosePanel = () => {
    if (isTouchInteraction.value) return;
    clearCloseTimer();
    closeTimer = window.setTimeout(() => {
      open.value = false;
      closeTimer = null;
    }, 140);
  };

  const handleOpenChange = (nextOpen: boolean) => {
    clearCloseTimer();
    open.value = isTouchInteraction.value ? false : nextOpen;
  };

  const handleTriggerPointerDown = (event: PointerEvent) => {
    lastTriggerPointerType.value = event.pointerType;
    if (event.pointerType === "mouse") return;
    suppressNextFocusOpen.value = true;
    clearCloseTimer();
    open.value = false;
  };

  const handleTriggerPointerEnter = (event: PointerEvent) => {
    if (event.pointerType === "mouse") openPanel();
  };

  const handleTriggerPointerLeave = (event: PointerEvent) => {
    if (event.pointerType === "mouse") scheduleClosePanel();
  };

  const handleTriggerFocus = () => {
    if (!suppressNextFocusOpen.value) openPanel();
  };

  const handleTriggerBlur = () => {
    suppressNextFocusOpen.value = false;
    scheduleClosePanel();
  };

  const handleTriggerClick = () => {
    if (
      isTouchInteraction.value ||
      (lastTriggerPointerType.value !== null &&
        lastTriggerPointerType.value !== "mouse")
    ) {
      clearCloseTimer();
      open.value = false;
      dialogOpen.value = true;
      suppressNextFocusOpen.value = false;
      lastTriggerPointerType.value = null;
      return;
    }
    openPanel();
  };

  const handleDialogOpenChange = (nextOpen: boolean) => {
    dialogOpen.value = nextOpen;
    if (!nextOpen) return;
    clearCloseTimer();
    open.value = false;
  };

  const closeOverlays = () => {
    clearCloseTimer();
    open.value = false;
    dialogOpen.value = false;
  };

  watch(
    isTouchInteraction,
    (isTouch) => {
      if (isTouch) {
        open.value = false;
        clearCloseTimer();
      } else {
        dialogOpen.value = false;
      }
    },
    { immediate: true },
  );

  onUnmounted(clearCloseTimer);

  return {
    closeOverlays,
    dialogOpen,
    handleContentPointerEnter: handleTriggerPointerEnter,
    handleContentPointerLeave: handleTriggerPointerLeave,
    handleDialogOpenChange,
    handleOpenChange,
    handleTriggerBlur,
    handleTriggerClick,
    handleTriggerFocus,
    handleTriggerPointerDown,
    handleTriggerPointerEnter,
    handleTriggerPointerLeave,
    isTouchInteraction,
    open,
  };
};
