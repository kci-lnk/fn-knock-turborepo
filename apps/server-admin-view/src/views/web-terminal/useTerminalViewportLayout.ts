import { computed, nextTick, ref, type ComponentPublicInstance } from "vue";
import {
  DEFAULT_TERMINAL_HEIGHT_PX,
  DESKTOP_TERMINAL_BOTTOM_GAP_PX,
  MAX_TERMINAL_HEIGHT_DESKTOP_PX,
  MOBILE_KEYBOARD_INSET_THRESHOLD_PX,
  MOBILE_TERMINAL_BOTTOM_GAP_PX,
} from "./terminal-runtime";
import {
  createPageScrollLock,
  detectCompactViewport,
  getVisualViewportMetrics,
} from "./terminal-dom";

export const useTerminalViewportLayout = ({
  focusTerminal,
  scheduleFit,
  syncTerminalTextInputAnchor,
}: {
  focusTerminal: () => void;
  scheduleFit: () => void;
  syncTerminalTextInputAnchor: () => void;
}) => {
  const terminalShellRef = ref<HTMLElement | null>(null);
  const terminalPanelRef = ref<HTMLElement | null>(null);
  const terminalFrameRef = ref<HTMLElement | null>(null);
  const mobileAccessoryBarRef = ref<HTMLElement | null>(null);
  const terminalStatusRef = ref<HTMLElement | null>(null);
  const terminalHeight = ref(`${DEFAULT_TERMINAL_HEIGHT_PX}px`);
  const compactViewport = ref(false);
  const isTerminalFullscreen = ref(false);
  const pageScrollLock = createPageScrollLock();

  const resolveHtmlElement = (
    target: Element | ComponentPublicInstance | null,
  ): HTMLElement | null => (target instanceof HTMLElement ? target : null);

  const setTerminalShellRef = (
    target: Element | ComponentPublicInstance | null,
  ) => {
    terminalShellRef.value = resolveHtmlElement(target);
  };

  const setTerminalPanelRef = (
    target: Element | ComponentPublicInstance | null,
  ) => {
    terminalPanelRef.value = resolveHtmlElement(target);
  };

  const setMobileAccessoryBarRef = (
    target: Element | ComponentPublicInstance | null,
  ) => {
    mobileAccessoryBarRef.value = resolveHtmlElement(target);
  };

  const setTerminalStatusRef = (
    target: Element | ComponentPublicInstance | null,
  ) => {
    terminalStatusRef.value = resolveHtmlElement(target);
  };

  const showMobileAccessoryBar = computed(() => compactViewport.value);
  const terminalPanelClass = computed(() =>
    isTerminalFullscreen.value
      ? "fixed z-50 flex overflow-hidden rounded-[24px] bg-background/94 p-2 shadow-[0_24px_96px_rgba(15,23,42,0.34)] backdrop-blur-md sm:p-3"
      : "",
  );
  const terminalPanelStyle = computed(() =>
    isTerminalFullscreen.value
      ? {
          top: "max(env(safe-area-inset-top), 0.5rem)",
          right: "max(env(safe-area-inset-right), 0.5rem)",
          bottom: "max(env(safe-area-inset-bottom), 0.5rem)",
          left: "max(env(safe-area-inset-left), 0.5rem)",
        }
      : undefined,
  );
  const terminalFrameStyle = computed(() => {
    if (isTerminalFullscreen.value) {
      return {
        minHeight: "0",
        maxHeight: "none",
      };
    }

    return compactViewport.value
      ? {
          height: terminalHeight.value,
          minHeight: terminalHeight.value,
        }
      : {
          maxHeight: terminalHeight.value,
        };
  });

  const syncViewportHeight = () => {
    compactViewport.value = detectCompactViewport();
    syncTerminalTextInputAnchor();
    const measurementTarget = isTerminalFullscreen.value
      ? terminalPanelRef.value
      : terminalFrameRef.value || terminalShellRef.value;
    if (!measurementTarget) return;

    const rect = measurementTarget.getBoundingClientRect();
    const viewportMetrics = getVisualViewportMetrics();
    const accessoryHeight =
      compactViewport.value && showMobileAccessoryBar.value
        ? (mobileAccessoryBarRef.value?.getBoundingClientRect().height ?? 0)
        : 0;
    const statusHeight =
      terminalStatusRef.value?.getBoundingClientRect().height ?? 0;
    const bottomGap = compactViewport.value
      ? MOBILE_TERMINAL_BOTTOM_GAP_PX
      : DESKTOP_TERMINAL_BOTTOM_GAP_PX;
    const reservedHeight = Math.ceil(
      accessoryHeight + statusHeight + bottomGap,
    );
    const available = Math.floor(
      viewportMetrics.visibleBottom - rect.top - reservedHeight,
    );
    const nextHeight =
      available > 0
        ? compactViewport.value
          ? available
          : Math.min(MAX_TERMINAL_HEIGHT_DESKTOP_PX, available)
        : DEFAULT_TERMINAL_HEIGHT_PX;
    terminalHeight.value = `${nextHeight}px`;
    scheduleFit();

    if (
      !isTerminalFullscreen.value &&
      compactViewport.value &&
      viewportMetrics.keyboardInset >= MOBILE_KEYBOARD_INSET_THRESHOLD_PX
    ) {
      void nextTick(() => {
        const frameRect = terminalFrameRef.value?.getBoundingClientRect();
        if (!frameRect) return;

        const visibleTop = viewportMetrics.offsetTop + 8;
        const visibleBottom = viewportMetrics.visibleBottom - 8;
        if (frameRect.top < visibleTop || frameRect.bottom > visibleBottom) {
          terminalFrameRef.value?.scrollIntoView({
            block: "nearest",
            inline: "nearest",
          });
        }
      });
    }
  };

  const setTerminalFullscreen = async (nextFullscreen: boolean) => {
    if (nextFullscreen === isTerminalFullscreen.value) {
      if (nextFullscreen) {
        focusTerminal();
      }
      return;
    }

    isTerminalFullscreen.value = nextFullscreen;
    if (nextFullscreen) {
      pageScrollLock.lock();
    } else {
      pageScrollLock.unlock();
    }

    await nextTick();
    syncViewportHeight();
    focusTerminal();
  };

  const toggleTerminalFullscreen = () => {
    void setTerminalFullscreen(!isTerminalFullscreen.value);
  };

  const startViewportTracking = () => {
    compactViewport.value = detectCompactViewport();
    window.addEventListener("resize", syncViewportHeight);
    window.visualViewport?.addEventListener("resize", syncViewportHeight);
    window.visualViewport?.addEventListener("scroll", syncViewportHeight);
  };

  const stopViewportTracking = () => {
    window.removeEventListener("resize", syncViewportHeight);
    window.visualViewport?.removeEventListener("resize", syncViewportHeight);
    window.visualViewport?.removeEventListener("scroll", syncViewportHeight);
    pageScrollLock.unlock();
  };

  return {
    compactViewport,
    isTerminalFullscreen,
    setMobileAccessoryBarRef,
    setTerminalFullscreen,
    setTerminalPanelRef,
    setTerminalShellRef,
    setTerminalStatusRef,
    showMobileAccessoryBar,
    startViewportTracking,
    stopViewportTracking,
    syncViewportHeight,
    terminalFrameRef,
    terminalFrameStyle,
    terminalPanelClass,
    terminalPanelStyle,
    toggleTerminalFullscreen,
  };
};
