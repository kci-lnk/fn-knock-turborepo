import { computed, ref, type ComponentPublicInstance, type Ref } from "vue";

const MAPPING_DIALOG_MOBILE_MAX_HEIGHT_DVH = 82;

export const useMappingDialogKeyboardScroll = ({
  isDialogOpen,
}: {
  isDialogOpen: Ref<boolean>;
}) => {
  const mappingDialogScrollRef = ref<HTMLElement | null>(null);
  const mappingDialogKeyboardInset = ref(0);
  const mappingDialogInputFocused = ref(false);
  const mappingDialogKeyboardSessionActive = ref(false);
  const mappingDialogViewportTop = ref("0px");
  const mappingDialogViewportHeight = ref("100dvh");
  let mappingDialogKeyboardScrollTimer: number | null = null;
  let mappingDialogInputSettleTimer: number | null = null;
  let mappingDialogFocusOutTimer: number | null = null;

  const isMappingDialogKeyboardActive = computed(
    () =>
      mappingDialogKeyboardSessionActive.value &&
      (mappingDialogInputFocused.value ||
        mappingDialogKeyboardInset.value > 0),
  );
  const isMappingDialogSoftKeyboardVisible = computed(
    () =>
      mappingDialogKeyboardSessionActive.value &&
      mappingDialogKeyboardInset.value > 0,
  );

  const mappingDialogContentStyle = computed(() => {
    return {
      "--mapping-dialog-mobile-max-height": `${MAPPING_DIALOG_MOBILE_MAX_HEIGHT_DVH}dvh`,
      "--mapping-dialog-viewport-height": mappingDialogViewportHeight.value,
      "--mapping-dialog-viewport-top": mappingDialogViewportTop.value,
    };
  });

  const mappingDialogScrollStyle = computed(() => ({
    scrollPaddingTop: "96px",
    scrollPaddingBottom: "96px",
  }));

  const setMappingDialogScrollElement = (
    element: Element | ComponentPublicInstance | null,
  ) => {
    mappingDialogScrollRef.value =
      element instanceof HTMLElement ? element : null;
  };

  const clearMappingDialogInputScrollTimer = () => {
    if (mappingDialogKeyboardScrollTimer !== null) {
      window.clearTimeout(mappingDialogKeyboardScrollTimer);
      mappingDialogKeyboardScrollTimer = null;
    }
    if (mappingDialogInputSettleTimer !== null) {
      window.clearTimeout(mappingDialogInputSettleTimer);
      mappingDialogInputSettleTimer = null;
    }
  };

  const clearMappingDialogFocusOutTimer = () => {
    if (mappingDialogFocusOutTimer === null) return;
    window.clearTimeout(mappingDialogFocusOutTimer);
    mappingDialogFocusOutTimer = null;
  };

  const clearMappingDialogKeyboardScrollTimer = () => {
    clearMappingDialogInputScrollTimer();
    clearMappingDialogFocusOutTimer();
  };

  const resolveMappingDialogKeyboardInset = (): number => {
    const viewport = window.visualViewport;
    if (!viewport) return 0;
    const inset = window.innerHeight - viewport.height - viewport.offsetTop;
    return inset > 80 ? Math.ceil(inset) : 0;
  };

  const updateMappingDialogViewport = () => {
    const viewport = window.visualViewport;
    mappingDialogViewportTop.value = viewport
      ? `${Math.max(0, viewport.offsetTop)}px`
      : "0px";
    mappingDialogViewportHeight.value = viewport
      ? `${Math.max(0, viewport.height)}px`
      : "100dvh";
    mappingDialogKeyboardInset.value = isDialogOpen.value
      ? resolveMappingDialogKeyboardInset()
      : 0;
    if (
      !mappingDialogInputFocused.value &&
      mappingDialogKeyboardInset.value === 0
    ) {
      mappingDialogKeyboardSessionActive.value = false;
    }
  };

  const isMappingDialogKeyboardInput = (
    target: Element | null,
  ): target is HTMLElement => {
    if (!(target instanceof HTMLElement)) return false;
    const tagName = target.tagName.toLowerCase();
    if (tagName !== "input" && tagName !== "textarea") return false;
    return mappingDialogScrollRef.value?.contains(target) === true;
  };

  const scrollMappingDialogInputIntoView = (
    target: HTMLElement,
    behavior: ScrollBehavior = "smooth",
  ) => {
    updateMappingDialogViewport();

    const container = mappingDialogScrollRef.value;
    if (!container) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior });
      return;
    }

    const targetRect = target.getBoundingClientRect();
    const containerRect = container.getBoundingClientRect();
    const viewport = window.visualViewport;
    const viewportTop = viewport?.offsetTop ?? 0;
    const viewportBottom = viewport
      ? viewport.offsetTop + viewport.height
      : window.innerHeight;
    const visibleTop = Math.max(containerRect.top, viewportTop + 12);
    const visibleBottom = Math.min(containerRect.bottom, viewportBottom - 16);
    const visibleHeight = visibleBottom - visibleTop;

    if (visibleHeight <= 0) {
      target.scrollIntoView({ block: "center", inline: "nearest", behavior });
      return;
    }

    const desiredCenter = visibleTop + visibleHeight / 2;
    const targetCenter = targetRect.top + targetRect.height / 2;
    const maxScrollTop = Math.max(
      0,
      container.scrollHeight - container.clientHeight,
    );
    const nextScrollTop = Math.min(
      maxScrollTop,
      Math.max(0, container.scrollTop + targetCenter - desiredCenter),
    );

    container.scrollTo({
      top: nextScrollTop,
      behavior,
    });

    if (mappingDialogInputSettleTimer !== null) {
      window.clearTimeout(mappingDialogInputSettleTimer);
    }
    mappingDialogInputSettleTimer = window.setTimeout(() => {
      mappingDialogInputSettleTimer = null;
      if (!isDialogOpen.value || !target.isConnected) return;
      target.scrollIntoView({
        block: "center",
        inline: "nearest",
        behavior,
      });
    }, 0);
  };

  const scheduleMappingDialogInputScrollIntoView = (target: HTMLElement) => {
    clearMappingDialogInputScrollTimer();

    let attempts = 0;
    const run = () => {
      scrollMappingDialogInputIntoView(
        target,
        attempts === 0 ? "auto" : "smooth",
      );
      attempts += 1;
      if (attempts >= 4) {
        mappingDialogKeyboardScrollTimer = null;
        return;
      }
      mappingDialogKeyboardScrollTimer = window.setTimeout(
        run,
        attempts === 1 ? 120 : 240,
      );
    };

    run();
  };

  const handleMappingDialogFocusIn = (event: FocusEvent) => {
    const target = event.target as Element | null;
    if (!isMappingDialogKeyboardInput(target)) return;
    clearMappingDialogFocusOutTimer();
    mappingDialogInputFocused.value = true;
    mappingDialogKeyboardSessionActive.value = true;
    updateMappingDialogViewport();
    scheduleMappingDialogInputScrollIntoView(target);
  };

  const handleMappingDialogFocusOut = (event: FocusEvent) => {
    const target = event.target as Element | null;
    if (!isMappingDialogKeyboardInput(target)) return;

    clearMappingDialogInputScrollTimer();
    clearMappingDialogFocusOutTimer();
    mappingDialogFocusOutTimer = window.setTimeout(() => {
      mappingDialogFocusOutTimer = null;
      mappingDialogInputFocused.value = isMappingDialogKeyboardInput(
        document.activeElement,
      );
      updateMappingDialogViewport();
    }, 0);
  };

  const handleMappingDialogViewportResize = () => {
    const activeElement = document.activeElement;
    const hasFocusedInput =
      isDialogOpen.value && isMappingDialogKeyboardInput(activeElement);
    mappingDialogInputFocused.value = hasFocusedInput;
    updateMappingDialogViewport();
    if (!hasFocusedInput) return;

    mappingDialogKeyboardSessionActive.value = true;
    scheduleMappingDialogInputScrollIntoView(activeElement);
  };

  const resetMappingDialogKeyboardScroll = () => {
    clearMappingDialogKeyboardScrollTimer();
    mappingDialogKeyboardInset.value = 0;
    mappingDialogInputFocused.value = false;
    mappingDialogKeyboardSessionActive.value = false;
    mappingDialogViewportTop.value = "0px";
    mappingDialogViewportHeight.value = "100dvh";
  };

  return {
    clearMappingDialogKeyboardScrollTimer,
    handleMappingDialogFocusIn,
    handleMappingDialogFocusOut,
    handleMappingDialogViewportResize,
    isMappingDialogKeyboardActive,
    isMappingDialogSoftKeyboardVisible,
    mappingDialogContentStyle,
    mappingDialogScrollStyle,
    resetMappingDialogKeyboardScroll,
    setMappingDialogScrollElement,
  };
};
