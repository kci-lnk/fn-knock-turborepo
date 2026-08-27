import {
  LEGACY_MOUSE_SEQUENCE_PREFIX,
  TERMINAL_MOUSE_MAX_LEGACY_COORD,
  TERMINAL_MOUSE_MOVE_FLAG,
  TERMINAL_MOUSE_WHEEL_DOWN,
  TERMINAL_MOUSE_WHEEL_UP,
  type TerminalMouseButton,
  type TerminalMouseCell,
  type TerminalMouseReportingState,
} from "./terminal-runtime";

type MouseModifierEvent = Pick<
  MouseEvent,
  "altKey" | "ctrlKey" | "metaKey" | "shiftKey"
>;

export const getTerminalMouseButton = (
  event: Pick<MouseEvent, "button">,
): TerminalMouseButton | null => {
  if (event.button === 0) return 0;
  if (event.button === 1) return 1;
  if (event.button === 2) return 2;
  return null;
};

export const getTerminalMouseModifierCode = (
  event: MouseModifierEvent,
): number => {
  let code = 0;
  if (event.shiftKey) code += 4;
  if (event.altKey || event.metaKey) code += 8;
  if (event.ctrlKey) code += 16;
  return code;
};

export const buildTerminalMouseSequence = (
  event: MouseModifierEvent,
  state: TerminalMouseReportingState,
  buttonCode: number,
  cell: TerminalMouseCell,
  options?: { release?: boolean },
): string => {
  const code = buttonCode + getTerminalMouseModifierCode(event);
  if (state.sgr) {
    return `\u001b[<${code};${cell.col};${cell.row}${
      options?.release ? "m" : "M"
    }`;
  }

  if (
    cell.col > TERMINAL_MOUSE_MAX_LEGACY_COORD ||
    cell.row > TERMINAL_MOUSE_MAX_LEGACY_COORD
  ) {
    return "";
  }

  const legacyCode =
    (options?.release ? 3 : buttonCode) + getTerminalMouseModifierCode(event);
  return `${LEGACY_MOUSE_SEQUENCE_PREFIX}${String.fromCharCode(
    legacyCode + 32,
    cell.col + 32,
    cell.row + 32,
  )}`;
};

export const getTerminalWheelStepCount = (
  event: WheelEvent,
  metrics: { rowHeight: number; rows: number },
): number => {
  const delta = Math.max(Math.abs(event.deltaY), Math.abs(event.deltaX));
  if (delta <= 0) return 0;

  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return Math.min(5, Math.max(1, Math.round(delta)));
  }

  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return Math.min(5, Math.max(1, Math.round(delta * metrics.rows)));
  }

  return Math.min(5, Math.max(1, Math.round(delta / metrics.rowHeight)));
};

type TerminalMouseReporterTerminal = {
  cols: number;
  rows: number;
  getMode: (mode: number, defaultValue: boolean) => boolean;
  hasMouseTracking?: () => boolean;
};

export const createTerminalMouseReporter = ({
  focusTerminal,
  getFrameElement,
  getMountElement,
  getRowHeight,
  getTerminal,
  queueInput,
}: {
  focusTerminal: () => void;
  getFrameElement: () => HTMLElement | null;
  getMountElement: () => HTMLElement | null;
  getRowHeight: () => number;
  getTerminal: () => TerminalMouseReporterTerminal | null;
  queueInput: (payload: string) => void;
}) => {
  let targetElement: HTMLElement | null = null;
  let pressedButton: TerminalMouseButton | null = null;
  let lastReportKey = "";

  const isModeEnabled = (mode: number): boolean => {
    try {
      return getTerminal()?.getMode(mode, false) === true;
    } catch {
      return false;
    }
  };

  const getReportingState = (): TerminalMouseReportingState => {
    const normal = isModeEnabled(1000);
    const buttonMotion = isModeEnabled(1002);
    const anyMotion = isModeEnabled(1003);
    const sgr = isModeEnabled(1006);

    let enabled = normal || buttonMotion || anyMotion;
    try {
      if (getTerminal()?.hasMouseTracking?.() === true) {
        enabled = true;
      }
    } catch {
      // Mode queries can throw while the terminal is being opened or disposed.
    }

    return {
      enabled,
      sgr,
      buttonMotion,
      anyMotion,
    };
  };

  const getCanvas = (): HTMLCanvasElement | null => {
    const canvas = getMountElement()?.querySelector("canvas");
    return canvas instanceof HTMLCanvasElement ? canvas : null;
  };

  const getMouseCell = (
    event: MouseEvent,
    options?: { clampToCanvas?: boolean },
  ): TerminalMouseCell | null => {
    const terminal = getTerminal();
    if (!terminal) return null;

    const canvas = getCanvas();
    if (!canvas) return null;

    const rect = canvas.getBoundingClientRect();
    if (
      rect.width <= 0 ||
      rect.height <= 0 ||
      terminal.cols <= 0 ||
      terminal.rows <= 0
    ) {
      return null;
    }

    const inside =
      event.clientX >= rect.left &&
      event.clientX <= rect.right &&
      event.clientY >= rect.top &&
      event.clientY <= rect.bottom;
    if (!inside && !options?.clampToCanvas) return null;

    const clientX = Math.min(Math.max(event.clientX, rect.left), rect.right);
    const clientY = Math.min(Math.max(event.clientY, rect.top), rect.bottom);
    const cellWidth = rect.width / terminal.cols;
    const cellHeight = rect.height / terminal.rows;
    const col = Math.min(
      terminal.cols,
      Math.max(1, Math.floor((clientX - rect.left) / cellWidth) + 1),
    );
    const row = Math.min(
      terminal.rows,
      Math.max(1, Math.floor((clientY - rect.top) / cellHeight) + 1),
    );

    return { col, row };
  };

  const stopMouseEvent = (event: Event) => {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation();
  };

  const queueMouseSequence = (
    sequence: string,
    reportKey: string,
    options?: { dedupe?: boolean },
  ) => {
    if (
      !sequence ||
      (options?.dedupe !== false && reportKey === lastReportKey)
    ) {
      return;
    }
    lastReportKey = reportKey;
    queueInput(sequence);
  };

  const reportMouseEvent = (
    event: MouseEvent,
    state: TerminalMouseReportingState,
    buttonCode: number,
    cell: TerminalMouseCell,
    options?: { release?: boolean; dedupe?: boolean },
  ) => {
    const sequence = buildTerminalMouseSequence(
      event,
      state,
      buttonCode,
      cell,
      {
        release: options?.release,
      },
    );
    const reportKey = [
      options?.release ? "release" : "press",
      buttonCode,
      cell.col,
      cell.row,
      getTerminalMouseModifierCode(event),
    ].join(":");
    queueMouseSequence(sequence, reportKey, { dedupe: options?.dedupe });
  };

  const handleMouseDown = (event: MouseEvent) => {
    if (event.button === 2) return;

    const state = getReportingState();
    if (!state.enabled) return;

    const button = getTerminalMouseButton(event);
    if (button === null) return;

    const cell = getMouseCell(event);
    if (!cell) return;

    pressedButton = button;
    lastReportKey = "";
    stopMouseEvent(event);
    focusTerminal();
    reportMouseEvent(event, state, button, cell);
  };

  const handleMouseMove = (event: MouseEvent) => {
    const state = getReportingState();
    if (!state.enabled) return;

    const shouldReport =
      state.anyMotion || (state.buttonMotion && pressedButton !== null);
    if (!shouldReport) return;

    const cell = getMouseCell(event);
    if (!cell) return;

    const button = pressedButton ?? 0;
    stopMouseEvent(event);
    reportMouseEvent(event, state, button + TERMINAL_MOUSE_MOVE_FLAG, cell);
  };

  const handleMouseUp = (event: MouseEvent) => {
    const state = getReportingState();
    if (!state.enabled || pressedButton === null) return;

    const cell = getMouseCell(event, { clampToCanvas: true });
    if (!cell) return;

    const button = pressedButton;
    pressedButton = null;
    stopMouseEvent(event);
    reportMouseEvent(event, state, button, cell, { release: true });
    lastReportKey = "";
  };

  const handleMouseWheel = (event: WheelEvent) => {
    const terminal = getTerminal();
    const state = getReportingState();
    if (!state.enabled) return;

    const cell = getMouseCell(event);
    if (!cell) return;

    const steps = getTerminalWheelStepCount(event, {
      rowHeight: getRowHeight(),
      rows: Math.max(1, terminal?.rows ?? 1),
    });
    if (steps <= 0) return;

    const buttonCode =
      (event.deltaY || event.deltaX) > 0
        ? TERMINAL_MOUSE_WHEEL_DOWN
        : TERMINAL_MOUSE_WHEEL_UP;
    stopMouseEvent(event);
    focusTerminal();

    for (let index = 0; index < steps; index += 1) {
      reportMouseEvent(event, state, buttonCode, cell, {
        dedupe: false,
      });
    }
    lastReportKey = "";
  };

  const handleMouseClick = (event: MouseEvent) => {
    if (event.type === "contextmenu") return;

    const state = getReportingState();
    if (!state.enabled || !getMouseCell(event)) return;

    stopMouseEvent(event);
    focusTerminal();
  };

  const bind = () => {
    if (targetElement) return;

    const target = getFrameElement() || getMountElement();
    if (!target) return;

    targetElement = target;
    target.addEventListener("mousedown", handleMouseDown, {
      capture: true,
    });
    target.addEventListener("mousemove", handleMouseMove, {
      capture: true,
    });
    target.addEventListener("wheel", handleMouseWheel, {
      capture: true,
      passive: false,
    });
    target.addEventListener("click", handleMouseClick, {
      capture: true,
    });
    target.addEventListener("dblclick", handleMouseClick, {
      capture: true,
    });
    target.addEventListener("contextmenu", handleMouseClick, {
      capture: true,
    });
    document.addEventListener("mouseup", handleMouseUp, {
      capture: true,
    });
  };

  const unbind = () => {
    const target = targetElement;
    if (target) {
      target.removeEventListener("mousedown", handleMouseDown, true);
      target.removeEventListener("mousemove", handleMouseMove, true);
      target.removeEventListener("wheel", handleMouseWheel, true);
      target.removeEventListener("click", handleMouseClick, true);
      target.removeEventListener("dblclick", handleMouseClick, true);
      target.removeEventListener("contextmenu", handleMouseClick, true);
    }
    document.removeEventListener("mouseup", handleMouseUp, true);
    targetElement = null;
    pressedButton = null;
    lastReportKey = "";
  };

  return {
    bind,
    unbind,
  };
};
