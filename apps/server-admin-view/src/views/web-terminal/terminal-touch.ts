import type { Ref } from "vue";
import {
  clampTerminalFontSize,
  getTouchDistance,
  touchListIncludesIdentifier,
} from "./terminal-dom";
import {
  DEFAULT_TERMINAL_FONT_SIZE,
  DEFAULT_TERMINAL_FONT_SIZE_MOBILE,
  TERMINAL_TOUCH_DRAG_THRESHOLD_PX,
} from "./terminal-runtime";

type TerminalTouchInstance = {
  rows: number;
  scrollLines: (lines: number) => void;
};

type ApplyTerminalFontSize = (
  value: number,
  options?: { persist?: boolean },
) => void;

export const createTerminalTouchGestures = ({
  applyFontSize,
  compactViewport,
  getMountElement,
  getTerminal,
  isPinchZooming,
  persistFontSize,
  terminalFontSize,
}: {
  applyFontSize: ApplyTerminalFontSize;
  compactViewport: Ref<boolean>;
  getMountElement: () => HTMLElement | null;
  getTerminal: () => TerminalTouchInstance | null;
  isPinchZooming: Ref<boolean>;
  persistFontSize: () => void;
  terminalFontSize: Ref<number>;
}) => {
  let boundElement: HTMLElement | null = null;
  let pinchStartDistance = 0;
  let pinchStartFontSize = DEFAULT_TERMINAL_FONT_SIZE;
  let pinchZoomDirty = false;
  let trackedTouchId: number | null = null;
  let trackedTouchStartX = 0;
  let trackedTouchStartY = 0;
  let trackedTouchLastY = 0;
  let trackedTouchRemainder = 0;
  let trackedTouchMoved = false;
  let trackedTouchScrolling = false;

  const getTrackedTouch = (touches: TouchList): Touch | null => {
    if (trackedTouchId === null) return null;
    for (let index = 0; index < touches.length; index += 1) {
      const touch = touches.item(index);
      if (touch?.identifier === trackedTouchId) {
        return touch;
      }
    }
    return null;
  };

  const resetTrackedTouch = () => {
    trackedTouchId = null;
    trackedTouchStartX = 0;
    trackedTouchStartY = 0;
    trackedTouchLastY = 0;
    trackedTouchRemainder = 0;
    trackedTouchMoved = false;
    trackedTouchScrolling = false;
  };

  const getRowHeight = (): number => {
    const terminal = getTerminal();
    const mountElement = getMountElement();
    if (!terminal || !mountElement) {
      return DEFAULT_TERMINAL_FONT_SIZE_MOBILE * 1.6;
    }

    return Math.max(1, mountElement.clientHeight / Math.max(terminal.rows, 1));
  };

  const finishPinchZoom = () => {
    if (!isPinchZooming.value) return;
    isPinchZooming.value = false;
    pinchStartDistance = 0;
    pinchStartFontSize = terminalFontSize.value;
    if (pinchZoomDirty) {
      persistFontSize();
    }
    pinchZoomDirty = false;
  };

  const handleTouchStart = (event: TouchEvent) => {
    if (!compactViewport.value) return;

    if (event.touches.length === 2) {
      resetTrackedTouch();
      const distance = getTouchDistance(event.touches);
      if (!distance) return;
      pinchStartDistance = distance;
      pinchStartFontSize = terminalFontSize.value;
      pinchZoomDirty = false;
      isPinchZooming.value = true;
      return;
    }

    if (event.touches.length !== 1 || isPinchZooming.value) return;

    const touch = event.touches.item(0);
    if (!touch) return;
    trackedTouchId = touch.identifier;
    trackedTouchStartX = touch.clientX;
    trackedTouchStartY = touch.clientY;
    trackedTouchLastY = touch.clientY;
    trackedTouchRemainder = 0;
    trackedTouchMoved = false;
    trackedTouchScrolling = false;
  };

  const handleTouchMove = (event: TouchEvent) => {
    if (isPinchZooming.value && event.touches.length === 2) {
      const distance = getTouchDistance(event.touches);
      if (!distance || pinchStartDistance <= 0) return;

      event.preventDefault();
      const nextFontSize = clampTerminalFontSize(
        pinchStartFontSize * (distance / pinchStartDistance),
      );
      if (nextFontSize === terminalFontSize.value) return;

      pinchZoomDirty = true;
      applyFontSize(nextFontSize, { persist: false });
      return;
    }

    const terminal = getTerminal();
    if (
      !compactViewport.value ||
      !terminal ||
      trackedTouchId === null ||
      event.touches.length !== 1
    ) {
      return;
    }

    const touch = getTrackedTouch(event.touches);
    if (!touch) return;

    const totalDeltaX = touch.clientX - trackedTouchStartX;
    const totalDeltaY = touch.clientY - trackedTouchStartY;
    if (
      !trackedTouchMoved &&
      (Math.abs(totalDeltaX) >= TERMINAL_TOUCH_DRAG_THRESHOLD_PX ||
        Math.abs(totalDeltaY) >= TERMINAL_TOUCH_DRAG_THRESHOLD_PX)
    ) {
      trackedTouchMoved = true;
    }

    if (!trackedTouchScrolling) {
      if (
        Math.abs(totalDeltaY) < TERMINAL_TOUCH_DRAG_THRESHOLD_PX ||
        Math.abs(totalDeltaY) <= Math.abs(totalDeltaX)
      ) {
        trackedTouchLastY = touch.clientY;
        return;
      }
      trackedTouchScrolling = true;
    }

    event.preventDefault();
    const deltaY = touch.clientY - trackedTouchLastY;
    trackedTouchLastY = touch.clientY;
    trackedTouchRemainder += deltaY;

    const rowHeight = getRowHeight();
    const lines =
      trackedTouchRemainder > 0
        ? Math.floor(trackedTouchRemainder / rowHeight)
        : Math.ceil(trackedTouchRemainder / rowHeight);
    if (lines === 0) return;

    terminal.scrollLines(-lines);
    trackedTouchRemainder -= lines * rowHeight;
  };

  const handleTouchEnd = (event: TouchEvent) => {
    const wasPinchZooming = isPinchZooming.value;
    const trackedTouchEnded =
      trackedTouchId !== null &&
      touchListIncludesIdentifier(event.changedTouches, trackedTouchId);
    const shouldSuppressTerminalFocus =
      wasPinchZooming || (trackedTouchEnded && trackedTouchMoved);

    if (shouldSuppressTerminalFocus) {
      event.preventDefault();
      event.stopPropagation();
      event.stopImmediatePropagation?.();
    }

    if (trackedTouchEnded) {
      resetTrackedTouch();
    }

    finishPinchZoom();
  };

  const bind = () => {
    const element = getMountElement();
    if (!element || element === boundElement) return;

    if (boundElement) {
      unbind();
    }

    element.addEventListener("touchstart", handleTouchStart, {
      capture: true,
    });
    element.addEventListener("touchmove", handleTouchMove, {
      capture: true,
      passive: false,
    });
    element.addEventListener("touchend", handleTouchEnd, {
      capture: true,
    });
    element.addEventListener("touchcancel", handleTouchEnd, {
      capture: true,
    });
    boundElement = element;
  };

  const unbind = () => {
    if (!boundElement) return;
    boundElement.removeEventListener("touchstart", handleTouchStart, true);
    boundElement.removeEventListener("touchmove", handleTouchMove, true);
    boundElement.removeEventListener("touchend", handleTouchEnd, true);
    boundElement.removeEventListener("touchcancel", handleTouchEnd, true);
    boundElement = null;
    resetTrackedTouch();
    finishPinchZoom();
  };

  return {
    bind,
    getRowHeight,
    unbind,
  };
};
