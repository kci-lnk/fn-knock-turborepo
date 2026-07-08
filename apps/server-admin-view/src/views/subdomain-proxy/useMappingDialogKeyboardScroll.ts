import { computed, ref, type ComponentPublicInstance, type Ref } from "vue";

const MAPPING_DIALOG_MOBILE_MAX_HEIGHT_DVH = 82;

export const useMappingDialogKeyboardScroll = ({
  isDialogOpen,
}: {
  isDialogOpen: Ref<boolean>;
}) => {
  const mappingDialogScrollRef = ref<HTMLElement | null>(null);
  const mappingDialogKeyboardInset = ref(0);
  let mappingDialogKeyboardScrollTimer: number | null = null;

  const mappingDialogContentStyle = computed(() => {
    const keyboardInset = `${mappingDialogKeyboardInset.value}px`;
    const mobileMaxHeight = `calc(${MAPPING_DIALOG_MOBILE_MAX_HEIGHT_DVH}dvh - ${keyboardInset})`;

    return {
      "--mapping-dialog-keyboard-inset": keyboardInset,
      "--mapping-dialog-mobile-max-height": mobileMaxHeight,
    };
  });

  const mappingDialogScrollStyle = computed(() => ({
    scrollPaddingTop: "96px",
    scrollPaddingBottom: `${Math.max(mappingDialogKeyboardInset.value, 96)}px`,
  }));

  const setMappingDialogScrollElement = (
    element: Element | ComponentPublicInstance | null,
  ) => {
    mappingDialogScrollRef.value =
      element instanceof HTMLElement ? element : null;
  };

  const clearMappingDialogKeyboardScrollTimer = () => {
    if (mappingDialogKeyboardScrollTimer === null) return;
    window.clearTimeout(mappingDialogKeyboardScrollTimer);
    mappingDialogKeyboardScrollTimer = null;
  };

  const resolveMappingDialogKeyboardInset = (): number => {
    const viewport = window.visualViewport;
    if (!viewport) return 0;
    const inset = window.innerHeight - viewport.height - viewport.offsetTop;
    return inset > 80 ? Math.ceil(inset) : 0;
  };

  const updateMappingDialogKeyboardInset = () => {
    mappingDialogKeyboardInset.value = isDialogOpen.value
      ? resolveMappingDialogKeyboardInset()
      : 0;
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
    updateMappingDialogKeyboardInset();

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

    window.setTimeout(() => {
      target.scrollIntoView({
        block: "center",
        inline: "nearest",
        behavior,
      });
    }, 0);
  };

  const scheduleMappingDialogInputScrollIntoView = (target: HTMLElement) => {
    clearMappingDialogKeyboardScrollTimer();

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
    scheduleMappingDialogInputScrollIntoView(target);
  };

  const handleMappingDialogViewportResize = () => {
    updateMappingDialogKeyboardInset();
    if (!isDialogOpen.value) return;
    const activeElement = document.activeElement;
    if (!isMappingDialogKeyboardInput(activeElement)) return;

    scheduleMappingDialogInputScrollIntoView(activeElement);
  };

  const resetMappingDialogKeyboardScroll = () => {
    clearMappingDialogKeyboardScrollTimer();
    mappingDialogKeyboardInset.value = 0;
  };

  return {
    clearMappingDialogKeyboardScrollTimer,
    handleMappingDialogFocusIn,
    handleMappingDialogViewportResize,
    mappingDialogContentStyle,
    mappingDialogScrollStyle,
    resetMappingDialogKeyboardScroll,
    setMappingDialogScrollElement,
  };
};
