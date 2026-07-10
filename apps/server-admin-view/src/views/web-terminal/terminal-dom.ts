import {
  MAX_TERMINAL_FONT_SIZE,
  MIN_TERMINAL_FONT_SIZE,
} from "./terminal-runtime";
export { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";

export const detectCompactViewport = (): boolean => {
  if (typeof window === "undefined") return false;
  return (
    window.matchMedia("(pointer: coarse)").matches || window.innerWidth < 768
  );
};

export const getVisualViewportMetrics = () => {
  if (typeof window === "undefined") {
    return {
      height: 0,
      offsetTop: 0,
      visibleBottom: 0,
      keyboardInset: 0,
    };
  }

  const viewport = window.visualViewport;
  const height = viewport?.height ?? window.innerHeight;
  const offsetTop = viewport?.offsetTop ?? 0;
  const visibleBottom = offsetTop + height;
  const keyboardInset = Math.max(0, window.innerHeight - visibleBottom);

  return {
    height,
    offsetTop,
    visibleBottom,
    keyboardInset,
  };
};

export const focusElementWithoutScroll = (element: HTMLElement) => {
  try {
    element.focus({ preventScroll: true });
  } catch {
    element.focus();
  }
};

export const createPageScrollLock = () => {
  let locked = false;
  let previousHtmlOverflow = "";
  let previousBodyOverflow = "";

  return {
    lock: () => {
      if (typeof document === "undefined" || locked) return;

      previousHtmlOverflow = document.documentElement.style.overflow;
      previousBodyOverflow = document.body.style.overflow;
      document.documentElement.style.overflow = "hidden";
      document.body.style.overflow = "hidden";
      locked = true;
    },
    unlock: () => {
      if (typeof document === "undefined" || !locked) return;

      document.documentElement.style.overflow = previousHtmlOverflow;
      document.body.style.overflow = previousBodyOverflow;
      locked = false;
    },
  };
};

export const resolveConstrainedMenuPosition = ({
  clientX,
  clientY,
  menuHeight,
  menuWidth,
  viewportGap,
  viewportHeight,
  viewportWidth,
}: {
  clientX: number;
  clientY: number;
  menuHeight: number;
  menuWidth: number;
  viewportGap: number;
  viewportHeight: number;
  viewportWidth: number;
}) => {
  const maxX = Math.max(viewportGap, viewportWidth - menuWidth - viewportGap);
  const maxY = Math.max(viewportGap, viewportHeight - menuHeight - viewportGap);

  return {
    x: Math.min(maxX, Math.max(viewportGap, clientX)),
    y: Math.min(maxY, Math.max(viewportGap, clientY)),
  };
};

export const clampTerminalFontSize = (value: number): number =>
  Math.min(
    MAX_TERMINAL_FONT_SIZE,
    Math.max(MIN_TERMINAL_FONT_SIZE, Math.round(value)),
  );

export const getTouchDistance = (touches: TouchList): number | null => {
  const first = touches.item(0);
  const second = touches.item(1);
  if (!first || !second) return null;
  return Math.hypot(
    second.clientX - first.clientX,
    second.clientY - first.clientY,
  );
};

export const touchListIncludesIdentifier = (
  touches: TouchList,
  identifier: number,
): boolean => {
  for (let index = 0; index < touches.length; index += 1) {
    if (touches.item(index)?.identifier === identifier) {
      return true;
    }
  }
  return false;
};
