export type GhosttyModule = typeof import("ghostty-web");

let ghosttyModulePromise: Promise<GhosttyModule> | null = null;

export const textEncoder = new TextEncoder();
export const LEGACY_MOUSE_SEQUENCE_PREFIX = "\u001b[M";
export const REMOTE_RESPONSE_CODEPOINT_SAMPLE_LIMIT = 12;
export const ASCII_TERMINAL_RESPONSE_PATTERN = /^[\u0000-\u007f]*$/;

export const ensureGhostty = async () => {
  if (!ghosttyModulePromise) {
    ghosttyModulePromise = import("ghostty-web").then(async (module) => {
      await module.init();
      return module;
    });
  }
  return ghosttyModulePromise;
};

export const RECENT_SESSION_KEY = "fn-knock:terminal:last-session";
export const TERMINAL_FONT_SIZE_KEY = "fn-knock:terminal:font-size";
export const INPUT_BATCH_WINDOW_MS = 10;
export const INPUT_BATCH_MAX_BYTES = 1024;
export const RESIZE_BATCH_WINDOW_MS = 320;
export const DEFAULT_TERMINAL_HEIGHT_PX = 460;
export const MAX_TERMINAL_HEIGHT_DESKTOP_PX = 780;
export const MOBILE_TERMINAL_BOTTOM_GAP_PX = 12;
export const DESKTOP_TERMINAL_BOTTOM_GAP_PX = 24;
export const MOBILE_KEYBOARD_INSET_THRESHOLD_PX = 120;
export const DEFAULT_TERMINAL_FONT_SIZE = 14;
export const DEFAULT_TERMINAL_FONT_SIZE_MOBILE = 12;
export const MIN_TERMINAL_FONT_SIZE = 13;
export const MAX_TERMINAL_FONT_SIZE = 20;
export const TERMINAL_TOUCH_DRAG_THRESHOLD_PX = 10;
export const TERMINAL_MOUSE_MOVE_FLAG = 32;
export const TERMINAL_MOUSE_WHEEL_UP = 64;
export const TERMINAL_MOUSE_WHEEL_DOWN = 65;
export const TERMINAL_MOUSE_MAX_LEGACY_COORD = 223;
export const TERMINAL_CONTEXT_MENU_WIDTH = 176;
export const TERMINAL_CONTEXT_MENU_HEIGHT = 132;
export const TERMINAL_CONTEXT_MENU_VIEWPORT_GAP = 8;

export type ArmedModifier = "ctrl" | "alt";
export type TerminalMouseButton = 0 | 1 | 2;
export type TerminalMouseCell = {
  col: number;
  row: number;
};
export type TerminalMouseReportingState = {
  enabled: boolean;
  sgr: boolean;
  buttonMotion: boolean;
  anyMotion: boolean;
};

type ToolbarShortcut = {
  id: string;
  label: string;
  value: string;
};

export const toolbarModifierLabels: Record<ArmedModifier, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
};
export const toolbarPrimaryShortcuts: ToolbarShortcut[] = [
  { id: "esc", label: "Esc", value: "\u001b" },
  { id: "tab", label: "Tab", value: "\t" },
  { id: "shift-tab", label: "S-Tab", value: "\u001b[Z" },
];
export const toolbarNavigationShortcuts: ToolbarShortcut[] = [
  { id: "home", label: "Home", value: "\u001b[H" },
  { id: "arrow-left", label: "←", value: "\u001b[D" },
  { id: "arrow-up", label: "↑", value: "\u001b[A" },
  { id: "arrow-down", label: "↓", value: "\u001b[B" },
  { id: "arrow-right", label: "→", value: "\u001b[C" },
  { id: "end", label: "End", value: "\u001b[F" },
];
