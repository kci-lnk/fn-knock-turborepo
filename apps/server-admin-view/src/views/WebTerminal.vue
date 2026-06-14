<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import {
  AlertTriangle,
  ClipboardPaste,
  Copy,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCcw,
  Send,
  TextSelect,
  Trash2,
} from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { TerminalAPI } from "../lib/api";
import type {
  TerminalAttachmentRecord,
  TerminalOutputChunk,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTransport,
} from "../types";
import { useConfigStore } from "../store/config";

type GhosttyModule = typeof import("ghostty-web");

let ghosttyModulePromise: Promise<GhosttyModule> | null = null;
const textEncoder = new TextEncoder();
const LEGACY_MOUSE_SEQUENCE_PREFIX = "\u001b[M";
const REMOTE_RESPONSE_CODEPOINT_SAMPLE_LIMIT = 12;
const ASCII_TERMINAL_RESPONSE_PATTERN = /^[\u0000-\u007f]*$/;

const ensureGhostty = async () => {
  if (!ghosttyModulePromise) {
    ghosttyModulePromise = import("ghostty-web").then(async (module) => {
      await module.init();
      return module;
    });
  }
  return ghosttyModulePromise;
};

const RECENT_SESSION_KEY = "fn-knock:terminal:last-session";
const TERMINAL_FONT_SIZE_KEY = "fn-knock:terminal:font-size";
const INPUT_BATCH_WINDOW_MS = 10;
const INPUT_BATCH_MAX_BYTES = 1024;
const RESIZE_BATCH_WINDOW_MS = 320;
const DEFAULT_TERMINAL_HEIGHT_PX = 460;
const MAX_TERMINAL_HEIGHT_DESKTOP_PX = 780;
const MOBILE_TERMINAL_BOTTOM_GAP_PX = 12;
const DESKTOP_TERMINAL_BOTTOM_GAP_PX = 24;
const MOBILE_KEYBOARD_INSET_THRESHOLD_PX = 120;
const DEFAULT_TERMINAL_FONT_SIZE = 14;
const DEFAULT_TERMINAL_FONT_SIZE_MOBILE = 12;
const MIN_TERMINAL_FONT_SIZE = 13;
const MAX_TERMINAL_FONT_SIZE = 20;
const TERMINAL_TOUCH_DRAG_THRESHOLD_PX = 10;
const TERMINAL_MOUSE_MOVE_FLAG = 32;
const TERMINAL_MOUSE_WHEEL_UP = 64;
const TERMINAL_MOUSE_WHEEL_DOWN = 65;
const TERMINAL_MOUSE_MAX_LEGACY_COORD = 223;
const TERMINAL_CONTEXT_MENU_WIDTH = 176;
const TERMINAL_CONTEXT_MENU_HEIGHT = 132;
const TERMINAL_CONTEXT_MENU_VIEWPORT_GAP = 8;
type ArmedModifier = "ctrl" | "alt";
type TerminalMouseButton = 0 | 1 | 2;
type TerminalMouseCell = {
  col: number;
  row: number;
};
type TerminalMouseReportingState = {
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

const toolbarModifierLabels: Record<ArmedModifier, string> = {
  ctrl: "Ctrl",
  alt: "Alt",
};
const toolbarPrimaryShortcuts: ToolbarShortcut[] = [
  { id: "esc", label: "Esc", value: "\u001b" },
  { id: "tab", label: "Tab", value: "\t" },
  { id: "shift-tab", label: "S-Tab", value: "\u001b[Z" },
];
const toolbarNavigationShortcuts: ToolbarShortcut[] = [
  { id: "home", label: "Home", value: "\u001b[H" },
  { id: "arrow-left", label: "←", value: "\u001b[D" },
  { id: "arrow-up", label: "↑", value: "\u001b[A" },
  { id: "arrow-down", label: "↓", value: "\u001b[B" },
  { id: "arrow-right", label: "→", value: "\u001b[C" },
  { id: "end", label: "End", value: "\u001b[F" },
];

const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();

const runtimeStatus = ref<TerminalRuntimeStatus | null>(null);
const sessions = ref<TerminalSessionRecord[]>([]);
const isBooting = ref(true);
const isCreating = ref(false);
const isKilling = ref(false);
const selectedSessionId = ref("");
const connectionState = ref<"idle" | "connecting" | "connected" | "error">(
  "idle",
);
const connectionError = ref("");
const activeTransport = ref<TerminalTransport | null>(null);
const activeAttachment = ref<TerminalAttachmentRecord | null>(null);
const terminalMountRef = ref<HTMLElement | null>(null);
const terminalShellRef = ref<HTMLElement | null>(null);
const terminalPanelRef = ref<HTMLElement | null>(null);
const terminalFrameRef = ref<HTMLElement | null>(null);
const mobileAccessoryBarRef = ref<HTMLElement | null>(null);
const terminalStatusRef = ref<HTMLElement | null>(null);
const terminalContextMenuRef = ref<HTMLElement | null>(null);
const terminalHeight = ref(`${DEFAULT_TERMINAL_HEIGHT_PX}px`);
const compactViewport = ref(false);
const terminalFontSize = ref(DEFAULT_TERMINAL_FONT_SIZE);
const isPinchZooming = ref(false);
const armedModifier = ref<ArmedModifier | null>(null);
const sendDialogOpen = ref(false);
const sendDialogPayload = ref("");
const isSendingDialogPayload = ref(false);
const renameDialogOpen = ref(false);
const renameDialogValue = ref("");
const isRenamingSession = ref(false);
const isTerminalFullscreen = ref(false);
const terminalContextMenuOpen = ref(false);
const terminalContextMenuX = ref(0);
const terminalContextMenuY = ref(0);
const terminalContextMenuHasSelection = ref(false);

let term: InstanceType<GhosttyModule["Terminal"]> | null = null;
let fitAddon: InstanceType<GhosttyModule["FitAddon"]> | null = null;
let pollGeneration = 0;
let lastOutputCursor = 0;
let resizeTimer: number | null = null;
let inputFlushTimer: number | null = null;
let pendingInputBuffer = "";
let pendingInputBytes = 0;
let inputSendQueue: Promise<void> = Promise.resolve();
let resizeSuppressed = false;
let pendingResizeTarget: { cols: number; rows: number } | null = null;
let lastSyncedResizeKey = "";
let lastRequestedResizeKey = "";
let resizeSendQueue: Promise<void> = Promise.resolve();
let terminalFitFrame: number | null = null;
let terminalFitTimer: number | null = null;
let terminalFitAttemptsRemaining = 0;
let pinchStartDistance = 0;
let pinchStartFontSize = DEFAULT_TERMINAL_FONT_SIZE;
let pinchZoomDirty = false;
let trackedTerminalTouchId: number | null = null;
let trackedTerminalTouchStartX = 0;
let trackedTerminalTouchStartY = 0;
let trackedTerminalTouchLastY = 0;
let trackedTerminalTouchRemainder = 0;
let trackedTerminalTouchMoved = false;
let trackedTerminalTouchScrolling = false;
let terminalMouseReportingTarget: HTMLElement | null = null;
let terminalMousePressedButton: TerminalMouseButton | null = null;
let lastTerminalMouseReportKey = "";
let outputTextDecoder = new TextDecoder();
let pendingLegacyTitleSequence = "";
let remoteOutputWriteDepth = 0;
let terminalInternalResponseDropDepth = 0;
let terminalResizeObserver: ResizeObserver | null = null;
let pageScrollLocked = false;
let previousHtmlOverflow = "";
let previousBodyOverflow = "";

const selectedSession = computed(
  () =>
    sessions.value.find((session) => session.id === selectedSessionId.value) ||
    null,
);
const terminalWindowTitle = computed(
  () => selectedSession.value?.title?.trim() || t("admin.webTerminal.title"),
);
const terminalWindowSubtitle = computed(() => {
  const session = selectedSession.value;
  const shellSegments = session?.shell.split("/").filter(Boolean) || [];
  const cwdSegments =
    session?.cwd.replace(/\/+$/, "").split("/").filter(Boolean) || [];
  const shell = shellSegments[shellSegments.length - 1] || "shell";
  const cwd = cwdSegments[cwdSegments.length - 1];

  return `${shell} · ${cwd || "~"}`;
});
const destroySessionDescription = computed(() => {
  const title = selectedSession.value?.title?.trim();
  if (!title) {
    return t("admin.webTerminal.destroyDescription");
  }

  return t("admin.webTerminal.destroyDescriptionWithTitle", { title });
});

const terminalEnabled = computed(
  () => configStore.config?.terminal_feature?.enabled === true,
);

const showMobileToolbar = computed(() => {
  if (configStore.config?.terminal_feature?.allow_mobile_toolbar === false) {
    return false;
  }
  return compactViewport.value;
});

const showMobileAccessoryBar = computed(() => compactViewport.value);
const toolbarDisabled = computed(() => !activeAttachment.value);
const armedModifierLabel = computed(() =>
  armedModifier.value ? toolbarModifierLabels[armedModifier.value] : "",
);
const terminalFullscreenLabel = computed(() =>
  isTerminalFullscreen.value
    ? t("admin.webTerminal.exitFullscreen")
    : t("admin.webTerminal.enterFullscreen"),
);
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
const terminalContextMenuStyle = computed(() => ({
  left: `${terminalContextMenuX.value}px`,
  top: `${terminalContextMenuY.value}px`,
}));
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

const statusTone = computed(() => {
  if (connectionState.value === "connected") {
    return t("admin.webTerminal.statusConnected");
  }
  if (connectionState.value === "connecting") {
    return t("admin.webTerminal.statusConnecting");
  }
  if (connectionState.value === "error") {
    return t("admin.webTerminal.statusError");
  }
  return t("admin.webTerminal.statusDisconnected");
});

const encodeInputToBase64 = (value: string): string => {
  const bytes = encodeTerminalInputToBytes(value);
  let binary = "";
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
};

const appendUtf8Bytes = (target: number[], value: string): void => {
  textEncoder.encode(value).forEach((byte) => target.push(byte));
};

const encodeTerminalInputToBytes = (value: string): Uint8Array => {
  if (!value.includes(LEGACY_MOUSE_SEQUENCE_PREFIX)) {
    return textEncoder.encode(value);
  }

  const bytes: number[] = [];
  let cursor = 0;

  while (cursor < value.length) {
    const sequenceStart = value.indexOf(LEGACY_MOUSE_SEQUENCE_PREFIX, cursor);
    if (sequenceStart === -1 || sequenceStart + 6 > value.length) {
      appendUtf8Bytes(bytes, value.slice(cursor));
      break;
    }

    appendUtf8Bytes(bytes, value.slice(cursor, sequenceStart));
    bytes.push(0x1b, 0x5b, 0x4d);
    for (let offset = 3; offset < 6; offset += 1) {
      bytes.push(value.charCodeAt(sequenceStart + offset) & 0xff);
    }
    cursor = sequenceStart + 6;
  }

  return Uint8Array.from(bytes);
};

const getInputByteLength = (value: string): number =>
  encodeTerminalInputToBytes(value).byteLength;

const hasAsciiControlByte = (value: string): boolean => {
  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    if (code < 0x20 || code === 0x7f) {
      return true;
    }
  }
  return false;
};

const isSafeRemoteTerminalResponse = (value: string): boolean =>
  value.length > 0 &&
  ASCII_TERMINAL_RESPONSE_PATTERN.test(value) &&
  (value.includes("\u001b") || hasAsciiControlByte(value));

const summarizeTerminalResponseCodePoints = (value: string): string =>
  Array.from(value)
    .slice(0, REMOTE_RESPONSE_CODEPOINT_SAMPLE_LIMIT)
    .map((char) => `U+${char.codePointAt(0)?.toString(16).toUpperCase()}`)
    .join(" ");

const decodeBase64ToBytes = (value: string): Uint8Array => {
  const binary = atob(value);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
};

const buildTerminalSizeKey = (cols: number, rows: number): string =>
  `${cols}x${rows}`;

const encodeCtrlInput = (value: string): string | null => {
  if (value.length !== 1) return null;
  const directMap: Record<string, string> = {
    " ": "\u0000",
    "@": "\u0000",
    "`": "\u0000",
    "2": "\u0000",
    "[": "\u001b",
    "{": "\u001b",
    "3": "\u001b",
    "\\": "\u001c",
    "|": "\u001c",
    "4": "\u001c",
    "]": "\u001d",
    "}": "\u001d",
    "5": "\u001d",
    "^": "\u001e",
    "~": "\u001e",
    "6": "\u001e",
    _: "\u001f",
    "7": "\u001f",
    "?": "\u007f",
    "8": "\u007f",
  };

  if (directMap[value]) {
    return directMap[value];
  }

  const code = value.toUpperCase().charCodeAt(0);
  if (code >= 65 && code <= 90) {
    return String.fromCharCode(code - 64);
  }

  return null;
};

const detectCompactViewport = (): boolean => {
  if (typeof window === "undefined") return false;
  return (
    window.matchMedia("(pointer: coarse)").matches || window.innerWidth < 768
  );
};

const getVisualViewportMetrics = () => {
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

const copyTextToClipboard = async (text: string) => {
  if (typeof navigator !== "undefined" && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }

  if (typeof document === "undefined") {
    throw new Error("Clipboard API unavailable");
  }

  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.setAttribute("readonly", "");
  textarea.style.position = "fixed";
  textarea.style.top = "0";
  textarea.style.left = "0";
  textarea.style.opacity = "0";

  document.body.appendChild(textarea);
  textarea.focus();
  textarea.select();
  textarea.setSelectionRange(0, textarea.value.length);

  const copied = document.execCommand("copy");
  document.body.removeChild(textarea);

  if (!copied) {
    throw new Error("execCommand copy failed");
  }
};

const readTextFromClipboard = async (): Promise<string> => {
  if (typeof navigator !== "undefined" && navigator.clipboard?.readText) {
    return navigator.clipboard.readText();
  }

  throw new Error(t("admin.webTerminal.clipboardPermissionDenied"));
};

const lockPageScroll = () => {
  if (typeof document === "undefined" || pageScrollLocked) return;

  previousHtmlOverflow = document.documentElement.style.overflow;
  previousBodyOverflow = document.body.style.overflow;
  document.documentElement.style.overflow = "hidden";
  document.body.style.overflow = "hidden";
  pageScrollLocked = true;
};

const unlockPageScroll = () => {
  if (typeof document === "undefined" || !pageScrollLocked) return;

  document.documentElement.style.overflow = previousHtmlOverflow;
  document.body.style.overflow = previousBodyOverflow;
  pageScrollLocked = false;
};

const clampTerminalFontSize = (value: number): number =>
  Math.min(
    MAX_TERMINAL_FONT_SIZE,
    Math.max(MIN_TERMINAL_FONT_SIZE, Math.round(value)),
  );

const persistTerminalFontSize = () => {
  localStorage.setItem(TERMINAL_FONT_SIZE_KEY, String(terminalFontSize.value));
};

const loadTerminalFontSize = () => {
  const stored = Number(localStorage.getItem(TERMINAL_FONT_SIZE_KEY) || "");
  if (Number.isFinite(stored)) {
    terminalFontSize.value = clampTerminalFontSize(stored);
    return;
  }
  terminalFontSize.value = compactViewport.value
    ? DEFAULT_TERMINAL_FONT_SIZE_MOBILE
    : DEFAULT_TERMINAL_FONT_SIZE;
};

const applyTerminalFontSize = (
  value: number,
  options?: { persist?: boolean },
) => {
  const nextFontSize = clampTerminalFontSize(value);
  if (nextFontSize === terminalFontSize.value) {
    if (options?.persist !== false) {
      persistTerminalFontSize();
    }
    return;
  }

  terminalFontSize.value = nextFontSize;
  if (options?.persist !== false) {
    persistTerminalFontSize();
  }

  if (!term) return;
  term.options.fontSize = nextFontSize;
  scheduleTerminalFit();
};

const nudgeTerminalFontSize = (delta: number) => {
  applyTerminalFontSize(terminalFontSize.value + delta);
};

const resetTerminalFontSize = () => {
  applyTerminalFontSize(
    compactViewport.value
      ? DEFAULT_TERMINAL_FONT_SIZE_MOBILE
      : DEFAULT_TERMINAL_FONT_SIZE,
  );
};

const getTouchDistance = (touches: TouchList): number | null => {
  const first = touches.item(0);
  const second = touches.item(1);
  if (!first || !second) return null;
  return Math.hypot(
    second.clientX - first.clientX,
    second.clientY - first.clientY,
  );
};

const touchListIncludesIdentifier = (
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

const getTrackedTerminalTouch = (touches: TouchList): Touch | null => {
  if (trackedTerminalTouchId === null) return null;
  for (let index = 0; index < touches.length; index += 1) {
    const touch = touches.item(index);
    if (touch?.identifier === trackedTerminalTouchId) {
      return touch;
    }
  }
  return null;
};

const resetTrackedTerminalTouch = () => {
  trackedTerminalTouchId = null;
  trackedTerminalTouchStartX = 0;
  trackedTerminalTouchStartY = 0;
  trackedTerminalTouchLastY = 0;
  trackedTerminalTouchRemainder = 0;
  trackedTerminalTouchMoved = false;
  trackedTerminalTouchScrolling = false;
};

const getTerminalTouchRowHeight = (): number => {
  if (!term || !terminalMountRef.value) {
    return DEFAULT_TERMINAL_FONT_SIZE_MOBILE * 1.6;
  }

  return Math.max(
    1,
    terminalMountRef.value.clientHeight / Math.max(term.rows, 1),
  );
};

const isTerminalModeEnabled = (mode: number): boolean => {
  try {
    return term?.getMode(mode, false) === true;
  } catch {
    return false;
  }
};

const getTerminalMouseReportingState = (): TerminalMouseReportingState => {
  const normal = isTerminalModeEnabled(1000);
  const buttonMotion = isTerminalModeEnabled(1002);
  const anyMotion = isTerminalModeEnabled(1003);
  const sgr = isTerminalModeEnabled(1006);

  let enabled = normal || buttonMotion || anyMotion;
  try {
    if (term?.hasMouseTracking() === true) {
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

const getTerminalCanvas = (): HTMLCanvasElement | null => {
  const canvas = terminalMountRef.value?.querySelector("canvas");
  return canvas instanceof HTMLCanvasElement ? canvas : null;
};

const getTerminalMouseCell = (
  event: MouseEvent,
  options?: { clampToCanvas?: boolean },
): TerminalMouseCell | null => {
  if (!term) return null;

  const canvas = getTerminalCanvas();
  if (!canvas) return null;

  const rect = canvas.getBoundingClientRect();
  if (rect.width <= 0 || rect.height <= 0 || term.cols <= 0 || term.rows <= 0) {
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
  const cellWidth = rect.width / term.cols;
  const cellHeight = rect.height / term.rows;
  const col = Math.min(
    term.cols,
    Math.max(1, Math.floor((clientX - rect.left) / cellWidth) + 1),
  );
  const row = Math.min(
    term.rows,
    Math.max(1, Math.floor((clientY - rect.top) / cellHeight) + 1),
  );

  return { col, row };
};

const getTerminalMouseButton = (
  event: MouseEvent,
): TerminalMouseButton | null => {
  if (event.button === 0) return 0;
  if (event.button === 1) return 1;
  if (event.button === 2) return 2;
  return null;
};

const getTerminalMouseModifierCode = (event: MouseEvent): number => {
  let code = 0;
  if (event.shiftKey) code += 4;
  if (event.altKey || event.metaKey) code += 8;
  if (event.ctrlKey) code += 16;
  return code;
};

const buildTerminalMouseSequence = (
  event: MouseEvent,
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

const stopTerminalMouseEvent = (event: Event) => {
  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation();
};

const queueTerminalMouseSequence = (
  sequence: string,
  reportKey: string,
  options?: { dedupe?: boolean },
) => {
  if (
    !sequence ||
    (options?.dedupe !== false && reportKey === lastTerminalMouseReportKey)
  ) {
    return;
  }
  lastTerminalMouseReportKey = reportKey;
  queueTerminalInput(sequence, { immediate: true });
};

const reportTerminalMouseEvent = (
  event: MouseEvent,
  state: TerminalMouseReportingState,
  buttonCode: number,
  cell: TerminalMouseCell,
  options?: { release?: boolean; dedupe?: boolean },
) => {
  const sequence = buildTerminalMouseSequence(event, state, buttonCode, cell, {
    release: options?.release,
  });
  const reportKey = [
    options?.release ? "release" : "press",
    buttonCode,
    cell.col,
    cell.row,
    getTerminalMouseModifierCode(event),
  ].join(":");
  queueTerminalMouseSequence(sequence, reportKey, { dedupe: options?.dedupe });
};

const handleTerminalMouseDown = (event: MouseEvent) => {
  if (event.button === 2) return;

  const state = getTerminalMouseReportingState();
  if (!state.enabled) return;

  const button = getTerminalMouseButton(event);
  if (button === null) return;

  const cell = getTerminalMouseCell(event);
  if (!cell) return;

  terminalMousePressedButton = button;
  lastTerminalMouseReportKey = "";
  stopTerminalMouseEvent(event);
  focusTerminal();
  reportTerminalMouseEvent(event, state, button, cell);
};

const handleTerminalMouseMove = (event: MouseEvent) => {
  const state = getTerminalMouseReportingState();
  if (!state.enabled) return;

  const shouldReport =
    state.anyMotion ||
    (state.buttonMotion && terminalMousePressedButton !== null);
  if (!shouldReport) return;

  const cell = getTerminalMouseCell(event);
  if (!cell) return;

  const button = terminalMousePressedButton ?? 0;
  stopTerminalMouseEvent(event);
  reportTerminalMouseEvent(
    event,
    state,
    button + TERMINAL_MOUSE_MOVE_FLAG,
    cell,
  );
};

const handleTerminalMouseUp = (event: MouseEvent) => {
  const state = getTerminalMouseReportingState();
  if (!state.enabled || terminalMousePressedButton === null) return;

  const cell = getTerminalMouseCell(event, { clampToCanvas: true });
  if (!cell) return;

  const button = terminalMousePressedButton;
  terminalMousePressedButton = null;
  stopTerminalMouseEvent(event);
  reportTerminalMouseEvent(event, state, button, cell, { release: true });
  lastTerminalMouseReportKey = "";
};

const getTerminalWheelStepCount = (event: WheelEvent): number => {
  const delta = Math.max(Math.abs(event.deltaY), Math.abs(event.deltaX));
  if (delta <= 0) return 0;

  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) {
    return Math.min(5, Math.max(1, Math.round(delta)));
  }
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) {
    return Math.min(
      5,
      Math.max(1, Math.round(delta * Math.max(1, term?.rows ?? 1))),
    );
  }

  const rowHeight = getTerminalTouchRowHeight();
  return Math.min(5, Math.max(1, Math.round(delta / rowHeight)));
};

const handleTerminalMouseWheel = (event: WheelEvent) => {
  const state = getTerminalMouseReportingState();
  if (!state.enabled) return;

  const cell = getTerminalMouseCell(event);
  if (!cell) return;

  const steps = getTerminalWheelStepCount(event);
  if (steps <= 0) return;

  const buttonCode =
    (event.deltaY || event.deltaX) > 0
      ? TERMINAL_MOUSE_WHEEL_DOWN
      : TERMINAL_MOUSE_WHEEL_UP;
  stopTerminalMouseEvent(event);
  focusTerminal();

  for (let index = 0; index < steps; index += 1) {
    reportTerminalMouseEvent(event, state, buttonCode, cell, {
      dedupe: false,
    });
  }
  lastTerminalMouseReportKey = "";
};

const handleTerminalMouseClick = (event: MouseEvent) => {
  if (event.type === "contextmenu") return;

  const state = getTerminalMouseReportingState();
  if (!state.enabled || !getTerminalMouseCell(event)) return;

  stopTerminalMouseEvent(event);
  focusTerminal();
};

const bindTerminalMouseReporting = () => {
  if (terminalMouseReportingTarget) return;

  const target = terminalFrameRef.value || terminalMountRef.value;
  if (!target) return;

  terminalMouseReportingTarget = target;
  target.addEventListener("mousedown", handleTerminalMouseDown, {
    capture: true,
  });
  target.addEventListener("mousemove", handleTerminalMouseMove, {
    capture: true,
  });
  target.addEventListener("wheel", handleTerminalMouseWheel, {
    capture: true,
    passive: false,
  });
  target.addEventListener("click", handleTerminalMouseClick, {
    capture: true,
  });
  target.addEventListener("dblclick", handleTerminalMouseClick, {
    capture: true,
  });
  target.addEventListener("contextmenu", handleTerminalMouseClick, {
    capture: true,
  });
  document.addEventListener("mouseup", handleTerminalMouseUp, {
    capture: true,
  });
};

const unbindTerminalMouseReporting = () => {
  const target = terminalMouseReportingTarget;
  if (target) {
    target.removeEventListener("mousedown", handleTerminalMouseDown, true);
    target.removeEventListener("mousemove", handleTerminalMouseMove, true);
    target.removeEventListener("wheel", handleTerminalMouseWheel, true);
    target.removeEventListener("click", handleTerminalMouseClick, true);
    target.removeEventListener("dblclick", handleTerminalMouseClick, true);
    target.removeEventListener("contextmenu", handleTerminalMouseClick, true);
  }
  document.removeEventListener("mouseup", handleTerminalMouseUp, true);
  terminalMouseReportingTarget = null;
  terminalMousePressedButton = null;
  lastTerminalMouseReportKey = "";
};

const applyTerminalFit = () => {
  if (!term || !fitAddon) return;

  const dimensions = fitAddon.proposeDimensions();
  if (!dimensions) return;
  if (dimensions.cols === term.cols && dimensions.rows === term.rows) return;

  runTerminalInternalMutation(
    () => {
      term?.resize(dimensions.cols, dimensions.rows);
    },
    { dropResponses: true },
  );
};

const hasTerminalCanvasHeightGap = (): boolean => {
  if (!terminalMountRef.value) return false;

  const canvas = terminalMountRef.value.querySelector("canvas");
  if (!(canvas instanceof HTMLCanvasElement)) return false;

  const mountHeight = terminalMountRef.value.clientHeight;
  const canvasHeight = Math.round(canvas.getBoundingClientRect().height);
  if (mountHeight <= 0 || canvasHeight <= 0) return false;

  return Math.abs(mountHeight - canvasHeight) > 24;
};

const runTerminalFitAttempt = () => {
  applyTerminalFit();

  if (
    terminalFitAttemptsRemaining <= 0 ||
    !hasTerminalCanvasHeightGap() ||
    typeof window === "undefined"
  ) {
    terminalFitAttemptsRemaining = 0;
    return;
  }

  terminalFitAttemptsRemaining -= 1;
  terminalFitTimer = window.setTimeout(() => {
    terminalFitTimer = null;
    runTerminalFitAttempt();
  }, 120);
};

const scheduleTerminalFit = () => {
  if (typeof window === "undefined") return;

  if (terminalFitFrame !== null) {
    window.cancelAnimationFrame(terminalFitFrame);
  }
  if (terminalFitTimer !== null) {
    window.clearTimeout(terminalFitTimer);
    terminalFitTimer = null;
  }
  terminalFitAttemptsRemaining = 8;

  void nextTick(() => {
    terminalFitFrame = window.requestAnimationFrame(() => {
      terminalFitFrame = null;
      runTerminalFitAttempt();
    });
  });
};

const observeTerminalMountSize = () => {
  if (
    terminalResizeObserver ||
    typeof ResizeObserver === "undefined" ||
    !terminalMountRef.value
  ) {
    return;
  }

  terminalResizeObserver = new ResizeObserver(() => {
    scheduleTerminalFit();
  });
  terminalResizeObserver.observe(terminalMountRef.value);
};

const handleTerminalTouchStart = (event: TouchEvent) => {
  if (!compactViewport.value) return;

  if (event.touches.length === 2) {
    resetTrackedTerminalTouch();
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
  trackedTerminalTouchId = touch.identifier;
  trackedTerminalTouchStartX = touch.clientX;
  trackedTerminalTouchStartY = touch.clientY;
  trackedTerminalTouchLastY = touch.clientY;
  trackedTerminalTouchRemainder = 0;
  trackedTerminalTouchMoved = false;
  trackedTerminalTouchScrolling = false;
};

const handleTerminalTouchMove = (event: TouchEvent) => {
  if (isPinchZooming.value && event.touches.length === 2) {
    const distance = getTouchDistance(event.touches);
    if (!distance || pinchStartDistance <= 0) return;

    event.preventDefault();
    const nextFontSize = clampTerminalFontSize(
      pinchStartFontSize * (distance / pinchStartDistance),
    );
    if (nextFontSize === terminalFontSize.value) return;

    pinchZoomDirty = true;
    applyTerminalFontSize(nextFontSize, { persist: false });
    return;
  }

  if (
    !compactViewport.value ||
    !term ||
    trackedTerminalTouchId === null ||
    event.touches.length !== 1
  ) {
    return;
  }

  const touch = getTrackedTerminalTouch(event.touches);
  if (!touch) return;

  const totalDeltaX = touch.clientX - trackedTerminalTouchStartX;
  const totalDeltaY = touch.clientY - trackedTerminalTouchStartY;
  if (
    !trackedTerminalTouchMoved &&
    (Math.abs(totalDeltaX) >= TERMINAL_TOUCH_DRAG_THRESHOLD_PX ||
      Math.abs(totalDeltaY) >= TERMINAL_TOUCH_DRAG_THRESHOLD_PX)
  ) {
    trackedTerminalTouchMoved = true;
  }

  if (!trackedTerminalTouchScrolling) {
    if (
      Math.abs(totalDeltaY) < TERMINAL_TOUCH_DRAG_THRESHOLD_PX ||
      Math.abs(totalDeltaY) <= Math.abs(totalDeltaX)
    ) {
      trackedTerminalTouchLastY = touch.clientY;
      return;
    }
    trackedTerminalTouchScrolling = true;
  }

  event.preventDefault();
  const deltaY = touch.clientY - trackedTerminalTouchLastY;
  trackedTerminalTouchLastY = touch.clientY;
  trackedTerminalTouchRemainder += deltaY;

  const rowHeight = getTerminalTouchRowHeight();
  const lines =
    trackedTerminalTouchRemainder > 0
      ? Math.floor(trackedTerminalTouchRemainder / rowHeight)
      : Math.ceil(trackedTerminalTouchRemainder / rowHeight);
  if (lines === 0) return;

  term.scrollLines(-lines);
  trackedTerminalTouchRemainder -= lines * rowHeight;
};

const finishTerminalPinchZoom = () => {
  if (!isPinchZooming.value) return;
  isPinchZooming.value = false;
  pinchStartDistance = 0;
  pinchStartFontSize = terminalFontSize.value;
  if (pinchZoomDirty) {
    persistTerminalFontSize();
  }
  pinchZoomDirty = false;
};

const handleTerminalTouchEnd = (event: TouchEvent) => {
  const wasPinchZooming = isPinchZooming.value;
  const trackedTouchEnded =
    trackedTerminalTouchId !== null &&
    touchListIncludesIdentifier(event.changedTouches, trackedTerminalTouchId);
  const shouldSuppressGhosttyFocus =
    wasPinchZooming || (trackedTouchEnded && trackedTerminalTouchMoved);

  if (shouldSuppressGhosttyFocus) {
    event.preventDefault();
    event.stopPropagation();
    event.stopImmediatePropagation?.();
  }

  if (trackedTouchEnded) {
    resetTrackedTerminalTouch();
  }

  finishTerminalPinchZoom();
};

const bindTerminalTouchGestures = () => {
  if (!terminalMountRef.value) return;
  terminalMountRef.value.addEventListener(
    "touchstart",
    handleTerminalTouchStart,
    { capture: true },
  );
  terminalMountRef.value.addEventListener(
    "touchmove",
    handleTerminalTouchMove,
    {
      capture: true,
      passive: false,
    },
  );
  terminalMountRef.value.addEventListener("touchend", handleTerminalTouchEnd, {
    capture: true,
  });
  terminalMountRef.value.addEventListener(
    "touchcancel",
    handleTerminalTouchEnd,
    { capture: true },
  );
};

const unbindTerminalTouchGestures = () => {
  if (!terminalMountRef.value) return;
  terminalMountRef.value.removeEventListener(
    "touchstart",
    handleTerminalTouchStart,
    true,
  );
  terminalMountRef.value.removeEventListener(
    "touchmove",
    handleTerminalTouchMove,
    true,
  );
  terminalMountRef.value.removeEventListener(
    "touchend",
    handleTerminalTouchEnd,
    true,
  );
  terminalMountRef.value.removeEventListener(
    "touchcancel",
    handleTerminalTouchEnd,
    true,
  );
};

const syncSessionDimensions = (
  sessionId: string,
  cols: number,
  rows: number,
) => {
  sessions.value = sessions.value.map((session) =>
    session.id === sessionId ? { ...session, cols, rows } : session,
  );
};

const markSyncedResize = (sessionId: string, cols: number, rows: number) => {
  lastSyncedResizeKey = buildTerminalSizeKey(cols, rows);
  lastRequestedResizeKey = lastSyncedResizeKey;
  syncSessionDimensions(sessionId, cols, rows);
};

const resetResizeState = () => {
  if (resizeTimer) {
    window.clearTimeout(resizeTimer);
    resizeTimer = null;
  }
  resizeSuppressed = false;
  pendingResizeTarget = null;
  lastSyncedResizeKey = "";
  lastRequestedResizeKey = "";
  resizeSendQueue = Promise.resolve();
};

const rememberRecentSession = (sessionId: string) => {
  localStorage.setItem(RECENT_SESSION_KEY, sessionId);
};

const resetOutputState = () => {
  lastOutputCursor = 0;
  outputTextDecoder = new TextDecoder();
  pendingLegacyTitleSequence = "";
};

const stripLegacyTitleSequences = (value: string): string => {
  if (!value && !pendingLegacyTitleSequence) {
    return "";
  }

  const source = `${pendingLegacyTitleSequence}${value}`;
  pendingLegacyTitleSequence = "";

  let sanitized = "";
  let cursor = 0;

  while (cursor < source.length) {
    const sequenceStart = source.indexOf("\u001bk", cursor);
    if (sequenceStart === -1) {
      sanitized += source.slice(cursor);
      break;
    }

    sanitized += source.slice(cursor, sequenceStart);

    const belTerminatorIndex = source.indexOf("\u0007", sequenceStart + 2);
    const stTerminatorIndex = source.indexOf("\u001b\\", sequenceStart + 2);

    let sequenceEnd = -1;
    let terminatorLength = 0;
    if (
      belTerminatorIndex !== -1 &&
      (stTerminatorIndex === -1 || belTerminatorIndex < stTerminatorIndex)
    ) {
      sequenceEnd = belTerminatorIndex;
      terminatorLength = 1;
    } else if (stTerminatorIndex !== -1) {
      sequenceEnd = stTerminatorIndex;
      terminatorLength = 2;
    }

    if (sequenceEnd === -1) {
      pendingLegacyTitleSequence = source.slice(sequenceStart);
      break;
    }

    cursor = sequenceEnd + terminatorLength;
  }

  return sanitized;
};

const writeRemoteTerminalOutput = (payload: string) => {
  if (!term) return;

  remoteOutputWriteDepth += 1;
  try {
    term.write(payload);
  } finally {
    remoteOutputWriteDepth -= 1;
  }
};

const runTerminalInternalMutation = (
  action: () => void,
  options?: { dropResponses?: boolean },
) => {
  remoteOutputWriteDepth += 1;
  if (options?.dropResponses) {
    terminalInternalResponseDropDepth += 1;
  }
  try {
    action();
  } finally {
    if (options?.dropResponses) {
      terminalInternalResponseDropDepth -= 1;
    }
    remoteOutputWriteDepth -= 1;
  }
};

const clearTerminal = () => {
  resetOutputState();
  if (!term) return;

  term.clear?.();
  term.reset();
  term.write("\u001b[2J\u001b[3J\u001b[H");
  focusTerminal();
};

const getTerminalTextInput = (): HTMLTextAreaElement | null => {
  const input = terminalMountRef.value?.querySelector("textarea");
  return input instanceof HTMLTextAreaElement ? input : null;
};

const focusElementWithoutScroll = (element: HTMLElement) => {
  try {
    element.focus({ preventScroll: true });
  } catch {
    element.focus();
  }
};

const syncTerminalTextInputAnchor = () => {
  const textInput = getTerminalTextInput();
  if (!textInput) return;

  textInput.style.position = compactViewport.value ? "fixed" : "absolute";
  textInput.style.left = "0";
  textInput.style.top = "0";
  textInput.style.width = "1px";
  textInput.style.height = "1px";
  textInput.style.padding = "0";
  textInput.style.border = "none";
  textInput.style.margin = "0";
  textInput.style.opacity = "0";
  textInput.style.clipPath = "inset(50%)";
  textInput.style.overflow = "hidden";
  textInput.style.whiteSpace = "nowrap";
  textInput.style.resize = "none";
  textInput.style.pointerEvents = "none";
  textInput.style.fontSize = "16px";
};

const focusTerminal = () => {
  syncTerminalTextInputAnchor();

  if (compactViewport.value) {
    const textInput = getTerminalTextInput();
    if (textInput) {
      focusElementWithoutScroll(textInput);
      void nextTick(() => {
        const nextInput = getTerminalTextInput();
        if (nextInput) {
          focusElementWithoutScroll(nextInput);
        }
      });
      return;
    }
  }

  term?.focus();
  void nextTick(() => term?.focus());
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
    lockPageScroll();
  } else {
    unlockPageScroll();
  }

  await nextTick();
  syncViewportHeight();
  focusTerminal();
};

const toggleTerminalFullscreen = () => {
  void setTerminalFullscreen(!isTerminalFullscreen.value);
};

const handleWindowKeydown = (event: KeyboardEvent) => {
  if (event.key !== "Escape") return;

  if (terminalContextMenuOpen.value) {
    event.preventDefault();
    closeTerminalContextMenu();
    focusTerminal();
    return;
  }

  if (!isTerminalFullscreen.value) return;

  event.preventDefault();
  void setTerminalFullscreen(false);
};

const closeTerminalContextMenu = () => {
  terminalContextMenuOpen.value = false;
};

const handleDocumentPointerDown = (event: PointerEvent) => {
  if (!terminalContextMenuOpen.value) return;

  const target = event.target;
  if (
    target instanceof Node &&
    terminalContextMenuRef.value?.contains(target)
  ) {
    return;
  }

  closeTerminalContextMenu();
};

const handleTerminalContextMenu = (event: MouseEvent) => {
  event.preventDefault();
  event.stopPropagation();
  event.stopImmediatePropagation();

  const selectedText = term?.getSelection() || "";
  const viewportWidth = window.innerWidth || TERMINAL_CONTEXT_MENU_WIDTH;
  const viewportHeight = window.innerHeight || TERMINAL_CONTEXT_MENU_HEIGHT;
  const maxX = Math.max(
    TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
    viewportWidth -
      TERMINAL_CONTEXT_MENU_WIDTH -
      TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
  );
  const maxY = Math.max(
    TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
    viewportHeight -
      TERMINAL_CONTEXT_MENU_HEIGHT -
      TERMINAL_CONTEXT_MENU_VIEWPORT_GAP,
  );

  terminalContextMenuHasSelection.value = selectedText.length > 0;
  terminalContextMenuX.value = Math.min(
    maxX,
    Math.max(TERMINAL_CONTEXT_MENU_VIEWPORT_GAP, event.clientX),
  );
  terminalContextMenuY.value = Math.min(
    maxY,
    Math.max(TERMINAL_CONTEXT_MENU_VIEWPORT_GAP, event.clientY),
  );
  terminalContextMenuOpen.value = true;
  void nextTick(() => {
    focusElementWithoutScroll(terminalContextMenuRef.value || document.body);
  });
};

const copyTerminalSelectionFromMenu = async () => {
  const selectedText = term?.getSelection() || "";
  closeTerminalContextMenu();

  if (!selectedText.length) {
    toast.info(t("admin.webTerminal.noSelection"));
    focusTerminal();
    return;
  }

  try {
    await copyTextToClipboard(selectedText);
    toast.success(t("admin.webTerminal.selectionCopied"));
  } catch (error) {
    toast.error(t("admin.webTerminal.copyFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.copySelectionFailed"),
    });
  } finally {
    focusTerminal();
  }
};

const pasteClipboardToTerminal = async () => {
  closeTerminalContextMenu();

  if (!activeAttachment.value) {
    toast.error(t("admin.webTerminal.noConnection"));
    focusTerminal();
    return;
  }

  try {
    const text = await readTextFromClipboard();
    if (!text) {
      toast.info(t("admin.webTerminal.emptyClipboard"));
      focusTerminal();
      return;
    }

    clearArmedModifier();
    term?.paste(text);
    focusTerminal();
  } catch (error) {
    console.warn("[terminal] clipboard read unavailable, using manual paste", {
      error: error instanceof Error ? error.message : String(error ?? "unknown"),
    });
    openManualPasteDialog();
  }
};

const selectAllTerminalText = () => {
  closeTerminalContextMenu();
  term?.selectAll();
  terminalContextMenuHasSelection.value = true;
  focusTerminal();
};

const keepTerminalFocused = (event: Event) => {
  if (event instanceof PointerEvent && event.pointerType !== "mouse") {
    return;
  }
  event.preventDefault();
  focusTerminal();
};

const clearArmedModifier = () => {
  armedModifier.value = null;
};

const applyArmedModifierToInput = (value: string): string => {
  const currentModifier = armedModifier.value;
  if (!currentModifier) return value;

  armedModifier.value = null;
  if (currentModifier === "alt") {
    return `\u001b${value}`;
  }

  return encodeCtrlInput(value) ?? value;
};

const toggleArmedModifier = (modifier: ArmedModifier) => {
  if (!activeAttachment.value) return;
  armedModifier.value = armedModifier.value === modifier ? null : modifier;
  focusTerminal();
};

const applyOutputChunk = (chunk: TerminalOutputChunk) => {
  if (!term) return;

  if (chunk.reset) {
    term.reset();
    outputTextDecoder = new TextDecoder();
    lastOutputCursor = 0;
    pendingLegacyTitleSequence = "";
  }

  if (chunk.data_base64) {
    const payload = stripLegacyTitleSequences(
      outputTextDecoder.decode(decodeBase64ToBytes(chunk.data_base64), {
        stream: true,
      }),
    );
    if (payload) {
      writeRemoteTerminalOutput(payload);
    }
  }

  lastOutputCursor = chunk.cursor;
  void nextTick(() => {
    focusTerminal();
  });
};

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
  const reservedHeight = Math.ceil(accessoryHeight + statusHeight + bottomGap);
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
  scheduleTerminalFit();

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

const refreshSessions = async () => {
  sessions.value = await TerminalAPI.listSessions();
  if (
    selectedSessionId.value &&
    !sessions.value.some((item) => item.id === selectedSessionId.value)
  ) {
    selectedSessionId.value = "";
  }
};

const shouldFlushInputImmediately = (data: string): boolean =>
  data.includes("\r") ||
  data.includes("\n") ||
  data.includes("\u0003") ||
  data.includes("\u0004") ||
  data.includes("\u001b") ||
  getInputByteLength(data) >= INPUT_BATCH_MAX_BYTES;

const clearPendingInput = () => {
  if (inputFlushTimer) {
    window.clearTimeout(inputFlushTimer);
    inputFlushTimer = null;
  }
  pendingInputBuffer = "";
  pendingInputBytes = 0;
};

const queueInputPayload = (attachmentId: string, payload: string) => {
  inputSendQueue = inputSendQueue
    .catch(() => undefined)
    .then(async () => {
      if (activeAttachment.value?.id !== attachmentId) return;
      await TerminalAPI.sendInput(attachmentId, encodeInputToBase64(payload));
    })
    .catch((error) => {
      if (activeAttachment.value?.id !== attachmentId) return;
      console.error(error);
      connectionState.value = "error";
      connectionError.value =
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.inputSendFailed");
    });
  return inputSendQueue;
};

const flushPendingInput = async () => {
  if (inputFlushTimer) {
    window.clearTimeout(inputFlushTimer);
    inputFlushTimer = null;
  }

  const attachmentId = activeAttachment.value?.id;
  if (!pendingInputBuffer) return;
  if (!attachmentId) {
    console.warn("[terminal] input flush deferred until attachment is ready", {
      connectionState: connectionState.value,
      bufferedBytes: pendingInputBytes,
      selectedSessionId: selectedSessionId.value || null,
    });
    return;
  }

  const payload = pendingInputBuffer;
  pendingInputBuffer = "";
  pendingInputBytes = 0;
  await queueInputPayload(attachmentId, payload);
};

const scheduleInputFlush = () => {
  if (inputFlushTimer) return;
  inputFlushTimer = window.setTimeout(() => {
    inputFlushTimer = null;
    void flushPendingInput();
  }, INPUT_BATCH_WINDOW_MS);
};

const queueTerminalInput = (
  data: string,
  options?: { immediate?: boolean },
) => {
  if (!data) {
    return;
  }

  if (!activeAttachment.value && connectionState.value !== "connecting") {
    console.warn("[terminal] dropping input without active attachment", {
      connectionState: connectionState.value,
      selectedSessionId: selectedSessionId.value || null,
      byteLength: getInputByteLength(data),
      immediate: options?.immediate === true,
    });
    return;
  }

  if (!activeAttachment.value && pendingInputBuffer.length === 0) {
    console.warn(
      "[terminal] buffering early input before attachment is ready",
      {
        connectionState: connectionState.value,
        selectedSessionId: selectedSessionId.value || null,
        byteLength: getInputByteLength(data),
        immediate: options?.immediate === true,
      },
    );
  }

  pendingInputBuffer += data;
  pendingInputBytes += getInputByteLength(data);

  if (
    options?.immediate ||
    shouldFlushInputImmediately(data) ||
    pendingInputBytes >= INPUT_BATCH_MAX_BYTES
  ) {
    void flushPendingInput();
    return;
  }

  scheduleInputFlush();
};

const queueRemoteTerminalResponse = (data: string) => {
  if (!isSafeRemoteTerminalResponse(data)) {
    console.warn("[terminal] dropped unexpected terminal response", {
      codePoints: summarizeTerminalResponseCodePoints(data),
      length: data.length,
    });
    return;
  }

  queueTerminalInput(data, { immediate: true });
};

const sendShortcut = (value: string) => {
  queueTerminalInput(value, { immediate: true });
  term?.focus();
};

const sendToolbarShortcut = (value: string) => {
  clearArmedModifier();
  sendShortcut(value);
  focusTerminal();
};

const sendTerminalPayloadNow = async (payload: string) => {
  if (!payload) return;
  await flushPendingInput().catch(() => undefined);

  const attachmentId = activeAttachment.value?.id;
  if (!attachmentId) {
    throw new Error(t("admin.webTerminal.noConnection"));
  }

  try {
    await TerminalAPI.sendInput(attachmentId, encodeInputToBase64(payload));
  } catch (error) {
    console.error(error);
    connectionState.value = "error";
    connectionError.value =
      error instanceof Error
        ? error.message
        : t("admin.webTerminal.inputSendFailed");
    throw error;
  }
};

const openSendDialog = () => {
  if (toolbarDisabled.value) return;
  sendDialogOpen.value = true;
};

const focusSendDialogTextarea = () => {
  void nextTick(() => {
    const textarea = document.getElementById("terminal-send-payload");
    if (textarea instanceof HTMLElement) {
      focusElementWithoutScroll(textarea);
    }
  });
};

const focusTerminalAfterDialogClose = (event: Event) => {
  event.preventDefault();
  void nextTick(() => {
    focusTerminal();
    window.requestAnimationFrame(() => focusTerminal());
  });
};

const openManualPasteDialog = () => {
  if (toolbarDisabled.value) return;
  sendDialogPayload.value = "";
  sendDialogOpen.value = true;
  focusSendDialogTextarea();
  toast.info(t("admin.webTerminal.manualPasteInfo"));
};

const openRenameDialog = () => {
  if (!selectedSession.value) return;
  renameDialogValue.value = selectedSession.value.title;
  renameDialogOpen.value = true;
};

const handleSessionTabChange = async (sessionId: string | number) => {
  const nextSessionId = String(sessionId || "");
  if (!nextSessionId || nextSessionId === selectedSessionId.value) return;
  const nextSession =
    sessions.value.find((session) => session.id === nextSessionId) || null;
  if (!nextSession) return;

  try {
    await connectToSession(nextSession);
  } catch (error) {
    toast.error(t("admin.webTerminal.switchFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.switchFailedDescription"),
    });
  }
};

const submitRenameDialog = async () => {
  const targetSession = selectedSession.value;
  const nextTitle = renameDialogValue.value.trim();
  if (!targetSession || !nextTitle) return;

  isRenamingSession.value = true;
  try {
    const updatedSession = await TerminalAPI.updateSessionTitle(
      targetSession.id,
      nextTitle,
    );
    sessions.value = sessions.value.map((session) =>
      session.id === updatedSession.id ? updatedSession : session,
    );
    renameDialogOpen.value = false;
    focusTerminal();
  } catch (error) {
    toast.error(t("admin.webTerminal.renameFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.renameFailedDescription"),
    });
  } finally {
    isRenamingSession.value = false;
  }
};

const submitSendDialog = async () => {
  const payload = sendDialogPayload.value;
  if (!payload.length) return;

  isSendingDialogPayload.value = true;
  try {
    clearArmedModifier();
    await sendTerminalPayloadNow(payload);
    sendDialogPayload.value = "";
    sendDialogOpen.value = false;
    focusTerminal();
  } catch (error) {
    toast.error(t("admin.webTerminal.sendFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.sendFailedDescription"),
    });
  } finally {
    isSendingDialogPayload.value = false;
  }
};

function restartHttpPollingFromSnapshot(attachment: TerminalAttachmentRecord) {
  if (activeAttachment.value?.id !== attachment.id) return;

  resetOutputState();
  void startHttpPolling(attachment);
}

const flushPendingResize = async () => {
  if (resizeTimer) {
    window.clearTimeout(resizeTimer);
    resizeTimer = null;
  }

  const attachment = activeAttachment.value;
  const nextTarget = pendingResizeTarget;
  if (!attachment || !nextTarget) return;

  const resizeKey = buildTerminalSizeKey(nextTarget.cols, nextTarget.rows);
  if (
    resizeKey === lastSyncedResizeKey ||
    resizeKey === lastRequestedResizeKey
  ) {
    pendingResizeTarget = null;
    return;
  }

  pendingResizeTarget = null;
  lastRequestedResizeKey = resizeKey;

  resizeSendQueue = resizeSendQueue
    .catch(() => undefined)
    .then(async () => {
      if (activeAttachment.value?.id !== attachment.id) return;
      const session = await TerminalAPI.resizeAttachment(
        attachment.id,
        nextTarget.cols,
        nextTarget.rows,
      );
      if (activeAttachment.value?.id !== attachment.id) return;
      markSyncedResize(session.id, session.cols, session.rows);
      restartHttpPollingFromSnapshot(attachment);
    })
    .catch((error) => {
      if (activeAttachment.value?.id !== attachment.id) return;
      console.error(error);
      lastRequestedResizeKey = lastSyncedResizeKey;
    });

  await resizeSendQueue;
};

const scheduleResize = () => {
  if (!term || !activeAttachment.value || resizeSuppressed) return;
  const nextTarget = { cols: term.cols, rows: term.rows };
  const resizeKey = buildTerminalSizeKey(nextTarget.cols, nextTarget.rows);
  if (
    resizeKey === lastSyncedResizeKey ||
    resizeKey === lastRequestedResizeKey
  ) {
    return;
  }
  pendingResizeTarget = nextTarget;
  if (resizeTimer) window.clearTimeout(resizeTimer);
  resizeTimer = window.setTimeout(async () => {
    void flushPendingResize();
  }, RESIZE_BATCH_WINDOW_MS);
};

const stopCurrentConnection = async (detach = true) => {
  pollGeneration += 1;
  await flushPendingInput().catch(() => undefined);
  await flushPendingResize().catch(() => undefined);
  clearPendingInput();
  clearArmedModifier();
  resetResizeState();

  const attachmentId = activeAttachment.value?.id;
  activeAttachment.value = null;
  activeTransport.value = null;
  connectionState.value = "idle";
  connectionError.value = "";
  resetOutputState();

  if (detach && attachmentId) {
    await TerminalAPI.detachAttachment(attachmentId).catch(() => undefined);
  }
};

const startHttpPolling = async (attachment: TerminalAttachmentRecord) => {
  const generation = ++pollGeneration;
  connectionState.value = "connected";
  activeTransport.value = "http-polling";

  while (
    generation === pollGeneration &&
    activeAttachment.value?.id === attachment.id
  ) {
    try {
      const result = await TerminalAPI.pollAttachment(attachment.id, {
        cursor: lastOutputCursor,
        timeout_ms: 4500,
      });
      if (
        generation !== pollGeneration ||
        activeAttachment.value?.id !== attachment.id
      ) {
        return;
      }
      if (result.changed && result.chunk) {
        applyOutputChunk(result.chunk);
      }
    } catch (error) {
      if (generation !== pollGeneration) return;
      connectionState.value = "error";
      connectionError.value =
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.pollingDisconnected");
      return;
    }
  }
};

const ensureTerminalReady = async () => {
  if (term) return;
  await nextTick();
  await initializeTerminal();
  if (!term) {
    throw new Error(t("admin.webTerminal.notReady"));
  }
};

const connectToSession = async (session: TerminalSessionRecord) => {
  selectedSessionId.value = session.id;
  await ensureTerminalReady();
  await stopCurrentConnection();
  selectedSessionId.value = session.id;
  rememberRecentSession(session.id);

  connectionState.value = "connecting";
  connectionError.value = "";
  markSyncedResize(session.id, session.cols, session.rows);
  clearTerminal();

  let attachment: TerminalAttachmentRecord;
  try {
    attachment = await TerminalAPI.createAttachment(session.id);
  } catch (error) {
    if (pendingInputBuffer) {
      console.warn(
        "[terminal] clearing buffered input after attachment failed",
        {
          sessionId: session.id,
          bufferedBytes: pendingInputBytes,
        },
      );
    }
    clearPendingInput();
    throw error;
  }

  activeAttachment.value = attachment;
  if (pendingInputBuffer) {
    console.warn("[terminal] attachment ready, flushing buffered input", {
      sessionId: session.id,
      attachmentId: attachment.id,
      bufferedBytes: pendingInputBytes,
    });
  }
  scheduleResize();
  void startHttpPolling(attachment);
  void flushPendingInput();
};

const createSession = async (
  options: { toastOnSuccess?: boolean; connect?: boolean } = {},
): Promise<TerminalSessionRecord | null> => {
  const { toastOnSuccess = true, connect = true } = options;
  isCreating.value = true;
  try {
    const session = await TerminalAPI.createSession({
      cols: term?.cols || 120,
      rows: term?.rows || 32,
    });
    await refreshSessions();
    if (connect) {
      await connectToSession(session);
    }
    if (toastOnSuccess) {
      toast.success(t("admin.webTerminal.sessionCreated"));
    }
    return session;
  } catch (error) {
    toast.error(t("admin.webTerminal.createFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.createFailedDescription"),
    });
    return null;
  } finally {
    isCreating.value = false;
  }
};

const reconnectSession = async () => {
  if (!selectedSession.value) return;
  try {
    await connectToSession(selectedSession.value);
  } catch (error) {
    toast.error(t("admin.webTerminal.reconnectFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.reconnectFailedDescription"),
    });
  }
};

const destroySelectedSession = async () => {
  if (!selectedSession.value) return;

  isKilling.value = true;
  try {
    await stopCurrentConnection();
    await TerminalAPI.deleteSession(selectedSession.value.id);
    await refreshSessions();
    const next = sessions.value[0];
    if (next) {
      await connectToSession(next);
    } else {
      selectedSessionId.value = "";
      clearTerminal();
    }
    toast.success(t("admin.webTerminal.sessionEnded"));
  } catch (error) {
    toast.error(t("admin.webTerminal.endFailed"), {
      description:
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.endFailedDescription"),
    });
  } finally {
    isKilling.value = false;
  }
};

const initializeTerminal = async () => {
  if (!terminalMountRef.value || term) return;
  const { Terminal, FitAddon } = await ensureGhostty();
  term = new Terminal({
    fontSize: terminalFontSize.value,
    cursorBlink: true,
    fontFamily:
      '"SFMono-Regular", "SF Mono", ui-monospace, Menlo, Monaco, Consolas, monospace',
    theme: {
      background: "#1c1c1e",
      foreground: "#ebeef2",
      cursor: "#f8fafc",
      black: "#141416",
      red: "#f87171",
      green: "#4ade80",
      yellow: "#facc15",
      blue: "#60a5fa",
      magenta: "#f472b6",
      cyan: "#22d3ee",
      white: "#e2e8f0",
      brightBlack: "#475569",
      brightRed: "#fb7185",
      brightGreen: "#86efac",
      brightYellow: "#fde047",
      brightBlue: "#93c5fd",
      brightMagenta: "#f9a8d4",
      brightCyan: "#67e8f9",
      brightWhite: "#f8fafc",
    },
  });
  fitAddon = new FitAddon();
  term.loadAddon(fitAddon);
  term.open(terminalMountRef.value);
  syncTerminalTextInputAnchor();
  bindTerminalMouseReporting();
  bindTerminalTouchGestures();
  applyTerminalFit();
  observeTerminalMountSize();
  scheduleTerminalFit();
  focusTerminal();
  term.onData((data) => {
    if (terminalInternalResponseDropDepth > 0) {
      return;
    }

    if (remoteOutputWriteDepth > 0) {
      queueRemoteTerminalResponse(data);
      return;
    }

    queueTerminalInput(applyArmedModifierToInput(data));
  });
  term.onResize(() => {
    if (resizeSuppressed) return;
    scheduleResize();
  });
};

const ensureDefaultSessionOnEntry = async (
  status: TerminalRuntimeStatus,
  sessionList: TerminalSessionRecord[],
): Promise<TerminalSessionRecord[]> => {
  if (
    sessionList.length > 0 ||
    !terminalEnabled.value ||
    !status.enabled ||
    status.blockedReason
  ) {
    return sessionList;
  }

  const session = await createSession({
    toastOnSuccess: false,
    connect: false,
  });
  if (!session) {
    return sessionList;
  }

  return sessions.value.length > 0 ? sessions.value : [session];
};

const bootstrapPage = async () => {
  let initialSession: TerminalSessionRecord | null = null;
  let shouldConnectInitialSession = false;

  try {
    if (!configStore.config) {
      await configStore.loadConfig();
    }
    const [status, sessionList] = await Promise.all([
      TerminalAPI.getStatus(),
      TerminalAPI.listSessions(),
    ]);
    runtimeStatus.value = status;
    const resolvedSessions = await ensureDefaultSessionOnEntry(
      status,
      sessionList,
    );
    sessions.value = resolvedSessions;

    const remembered = localStorage.getItem(RECENT_SESSION_KEY) || "";
    const firstSession =
      resolvedSessions.find((item) => item.id === remembered) ||
      resolvedSessions[0];
    if (firstSession && terminalEnabled.value && !status.blockedReason) {
      selectedSessionId.value = firstSession.id;
      initialSession = firstSession;
      shouldConnectInitialSession = true;
    }
  } catch (error) {
    connectionState.value = "error";
    connectionError.value =
      error instanceof Error ? error.message : t("admin.webTerminal.initFailed");
  } finally {
    isBooting.value = false;
    await nextTick();
    syncViewportHeight();
  }

  if (initialSession && shouldConnectInitialSession) {
    try {
      await connectToSession(initialSession);
    } catch (error) {
      connectionState.value = "error";
      connectionError.value =
        error instanceof Error
          ? error.message
          : t("admin.webTerminal.initFailed");
    }
  }
};

onMounted(async () => {
  compactViewport.value = detectCompactViewport();
  loadTerminalFontSize();
  await bootstrapPage();
  window.addEventListener("resize", syncViewportHeight);
  window.addEventListener("keydown", handleWindowKeydown);
  document.addEventListener("pointerdown", handleDocumentPointerDown);
  window.visualViewport?.addEventListener("resize", syncViewportHeight);
  window.visualViewport?.addEventListener("scroll", syncViewportHeight);
});

watch(
  [
    () => sessions.value.length,
    connectionState,
    connectionError,
    showMobileAccessoryBar,
    isTerminalFullscreen,
  ],
  () => {
    void nextTick().then(() => {
      syncViewportHeight();
    });
  },
);

onBeforeUnmount(() => {
  unbindTerminalMouseReporting();
  unbindTerminalTouchGestures();
  window.removeEventListener("resize", syncViewportHeight);
  window.removeEventListener("keydown", handleWindowKeydown);
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  window.visualViewport?.removeEventListener("resize", syncViewportHeight);
  window.visualViewport?.removeEventListener("scroll", syncViewportHeight);
  unlockPageScroll();
  if (terminalFitFrame !== null) {
    window.cancelAnimationFrame(terminalFitFrame);
    terminalFitFrame = null;
  }
  if (terminalFitTimer !== null) {
    window.clearTimeout(terminalFitTimer);
    terminalFitTimer = null;
  }
  terminalFitAttemptsRemaining = 0;
  void stopCurrentConnection();
  terminalResizeObserver?.disconnect();
  terminalResizeObserver = null;
  fitAddon?.dispose();
  term?.dispose();
  fitAddon = null;
  term = null;
});
</script>

<template>
  <div class="flex h-full min-w-0 flex-col gap-3 sm:gap-4">
    <Alert v-if="!terminalEnabled" class="border-border/60">
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>{{ t("admin.webTerminal.disabledTitle") }}</AlertTitle>
      <AlertDescription class="space-y-3">
        <p>{{ t("admin.webTerminal.disabledDescription") }}</p>
        <Button size="sm" @click="router.push('/system?tab=terminal')">
          {{ t("admin.webTerminal.goSettings") }}
        </Button>
      </AlertDescription>
    </Alert>

    <Alert
      v-else-if="runtimeStatus?.blockedReason"
      variant="destructive"
      class="border-destructive/40"
    >
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>{{ t("admin.webTerminal.unavailableTitle") }}</AlertTitle>
      <AlertDescription>{{ runtimeStatus.blockedReason }}</AlertDescription>
    </Alert>

    <div v-else ref="terminalShellRef" class="min-h-0 min-w-0 md:min-h-[80vh]">
      <Card class="min-h-0 min-w-0 h-full py-3 sm:py-6">
        <CardContent
          class="flex h-full min-h-0 min-w-0 flex-col gap-2.5 px-3 sm:gap-3 sm:px-6"
        >
          <div
            class="shrink-0 flex flex-col gap-2.5 lg:flex-row lg:items-center"
          >
            <div class="flex flex-wrap items-center gap-2">
              <div class="flex items-center gap-2 pl-2">
                <LiveStatusBadge
                  v-if="connectionState === 'connected'"
                  :active="true"
                  :active-label="t('admin.webTerminal.statusConnected')"
                  class="mt-px mr-3"
                />
                <span
                  v-else
                  :aria-label="statusTone"
                  :title="statusTone"
                  class="inline-flex h-2 w-2 shrink-0 rounded-full bg-zinc-300 align-middle"
                  role="status"
                />

                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="isCreating || isBooting"
                  :aria-label="t('admin.webTerminal.newSessionAria')"
                  :title="t('admin.webTerminal.newSession')"
                  @click="createSession"
                >
                  <LoaderCircle
                    v-if="isCreating"
                    class="h-4 w-4 animate-spin"
                  />
                  <Plus v-else class="h-4 w-4" />
                  <span class="sr-only">{{
                    t("admin.webTerminal.newSession")
                  }}</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="!selectedSession || isRenamingSession"
                  :aria-label="t('admin.webTerminal.renameSession')"
                  :title="t('admin.webTerminal.renameSession')"
                  @click="openRenameDialog"
                >
                  <LoaderCircle
                    v-if="isRenamingSession"
                    class="h-4 w-4 animate-spin"
                  />
                  <Pencil v-else class="h-4 w-4" />
                  <span class="sr-only">{{
                    t("admin.webTerminal.renameSession")
                  }}</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="
                    !selectedSession || connectionState === 'connecting'
                  "
                  :aria-label="t('admin.webTerminal.reconnectAria')"
                  :title="t('admin.webTerminal.reconnect')"
                  @pointerdown="keepTerminalFocused"
                  @click="reconnectSession"
                >
                  <RefreshCcw class="h-4 w-4" />
                  <span class="sr-only">{{
                    t("admin.webTerminal.reconnect")
                  }}</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="toolbarDisabled"
                  :aria-label="t('admin.webTerminal.sendAria')"
                  :title="t('admin.webTerminal.send')"
                  @click="openSendDialog"
                >
                  <Send class="h-4 w-4" />
                  <span class="sr-only">{{ t("admin.webTerminal.send") }}</span>
                </Button>
              </div>

              <div class="h-8 w-px shrink-0 bg-border/70" />

              <ConfirmDangerPopover
                :title="t('admin.webTerminal.endConfirmTitle')"
                :description="destroySessionDescription"
                :confirm-text="t('admin.webTerminal.endSession')"
                :loading="isKilling"
                :disabled="!selectedSession || isKilling"
                :on-confirm="destroySelectedSession"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    variant="ghost"
                    size="icon-sm"
                    class="rounded-lg text-destructive hover:bg-destructive/10 hover:text-destructive"
                    :disabled="!selectedSession || isKilling"
                    :aria-label="t('admin.webTerminal.endCurrentSession')"
                    :title="t('admin.webTerminal.endSession')"
                  >
                    <Trash2 class="h-4 w-4" />
                    <span class="sr-only">{{
                      t("admin.webTerminal.endSession")
                    }}</span>
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>

            <div
              v-if="sessions.length > 1"
              class="h-px w-full shrink-0 bg-border/70 lg:h-9 lg:w-px"
            />

            <Tabs
              v-if="sessions.length > 1"
              :model-value="selectedSessionId"
              @update:model-value="handleSessionTabChange"
              class="min-w-0 flex-1"
            >
              <div
                class="overflow-x-auto pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden lg:pb-0"
              >
                <TabsList
                  class="inline-flex h-9 min-w-max items-center gap-1 rounded-lg border border-border/70 bg-background/72 p-1 lg:ml-auto"
                >
                  <TabsTrigger
                    v-for="session in sessions"
                    :key="session.id"
                    :value="session.id"
                    class="h-7 min-w-[92px] max-w-[148px] rounded-md px-2.5 text-[11px] font-medium sm:min-w-[110px] sm:max-w-[180px] sm:text-xs"
                  >
                    <span class="truncate">{{ session.title }}</span>
                  </TabsTrigger>
                </TabsList>
              </div>
            </Tabs>
          </div>

          <div v-if="isBooting" class="space-y-3">
            <Skeleton class="h-12 w-full rounded-xl" />
            <Skeleton class="h-14 w-full rounded-xl" />
          </div>

          <div
            v-else-if="sessions.length === 0"
            class="rounded-xl border border-dashed border-border/80 bg-muted/10 px-4 py-5 text-sm text-muted-foreground"
          >
            {{ t("admin.webTerminal.noDefaultSession") }}
          </div>

          <Alert
            v-if="connectionError"
            variant="destructive"
            class="shrink-0 border-destructive/40"
          >
            <AlertTriangle class="h-4 w-4" />
            <AlertTitle>{{ t("admin.webTerminal.connectionErrorTitle") }}</AlertTitle>
            <AlertDescription>{{ connectionError }}</AlertDescription>
          </Alert>

          <div
            v-if="!isBooting && sessions.length > 0"
            ref="terminalPanelRef"
            :class="[
              'flex min-h-0 flex-1 flex-col gap-2.5 sm:gap-3',
              terminalPanelClass,
            ]"
            :style="terminalPanelStyle"
          >
            <div
              ref="terminalFrameRef"
              :class="[
                'relative flex min-h-0 min-w-0 flex-1 flex-col overflow-hidden rounded-[18px] border bg-[#1d1d1f] shadow-[0_14px_34px_rgba(15,23,42,0.18)] transition-[box-shadow,border-color] duration-200',
                isPinchZooming
                  ? 'border-cyan-400/65 shadow-[0_0_0_1px_rgba(34,211,238,0.26),0_16px_38px_rgba(8,145,178,0.14)]'
                  : 'border-white/8',
              ]"
              :style="terminalFrameStyle"
              :title="terminalWindowSubtitle"
            >
              <div
                class="relative flex h-[31px] shrink-0 items-center border-b border-black/28 bg-[linear-gradient(180deg,rgba(58,58,61,0.94)_0%,rgba(45,45,48,0.94)_100%)] px-3 backdrop-blur-xl rounded-t-[18px]"
              >
                <div
                  class="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/10"
                />
                <div class="z-10 flex items-center gap-2">
                  <span
                    class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#ff5f57] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.2)]"
                  />
                  <span
                    class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#febc2e] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.18)]"
                  />
                  <button
                    type="button"
                    class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#28c840] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.18)] transition-transform hover:scale-110 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[#7ee78d]/70"
                    :class="isTerminalFullscreen ? 'ring-1 ring-white/35' : ''"
                    :aria-label="terminalFullscreenLabel"
                    :aria-pressed="isTerminalFullscreen"
                    :title="terminalFullscreenLabel"
                    @click="toggleTerminalFullscreen"
                  >
                    <span class="sr-only">{{ terminalFullscreenLabel }}</span>
                  </button>
                </div>

                <div
                  class="pointer-events-none absolute left-1/2 top-1/2 min-w-0 max-w-[56%] -translate-x-1/2 -translate-y-1/2 text-center"
                >
                  <p
                    class="truncate text-[12px] font-medium tracking-[-0.01em] text-white/68"
                  >
                    {{ terminalWindowTitle }}
                  </p>
                </div>
              </div>

              <div class="relative min-h-0 flex-1 bg-[#1c1c1e]">
                <div
                  class="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/4"
                />
                <div
                  ref="terminalMountRef"
                  class="absolute inset-0 box-border min-h-0 min-w-0 overflow-hidden px-1.5 py-2.5 sm:px-2.5 sm:py-3"
                  @contextmenu.capture="handleTerminalContextMenu"
                />
                <div
                  v-if="terminalContextMenuOpen"
                  ref="terminalContextMenuRef"
                  :style="terminalContextMenuStyle"
                  class="fixed z-[70] w-44 overflow-hidden rounded-lg border border-white/12 bg-[#29292d]/95 p-1 text-sm text-white shadow-[0_16px_44px_rgba(0,0,0,0.38)] outline-none backdrop-blur-xl"
                  role="menu"
                  tabindex="-1"
                  @contextmenu.prevent.stop
                  @pointerdown.stop
                  @click.stop
                >
                  <button
                    type="button"
                    class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:text-white/35 disabled:hover:bg-transparent"
                    role="menuitem"
                    :disabled="!terminalContextMenuHasSelection"
                    @click="copyTerminalSelectionFromMenu"
                  >
                    <Copy class="h-4 w-4" />
                    <span>{{ t("admin.webTerminal.copy") }}</span>
                  </button>
                  <button
                    type="button"
                    class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10 disabled:cursor-not-allowed disabled:text-white/35 disabled:hover:bg-transparent"
                    role="menuitem"
                    :disabled="!activeAttachment"
                    @click="pasteClipboardToTerminal"
                  >
                    <ClipboardPaste class="h-4 w-4" />
                    <span>{{ t("admin.webTerminal.paste") }}</span>
                  </button>
                  <button
                    type="button"
                    class="flex h-9 w-full items-center gap-2 rounded-md px-2.5 text-left transition-colors hover:bg-white/10"
                    role="menuitem"
                    @click="selectAllTerminalText"
                  >
                    <TextSelect class="h-4 w-4" />
                    <span>{{ t("admin.webTerminal.selectAll") }}</span>
                  </button>
                </div>
              </div>
            </div>

            <div
              v-if="showMobileAccessoryBar"
              ref="mobileAccessoryBarRef"
              class="shrink-0 rounded-2xl border border-border/70 bg-muted/15 p-2 pb-[calc(env(safe-area-inset-bottom)+0.5rem)] sm:p-2.5 sm:pb-2.5"
            >
              <div
                class="flex items-center gap-2 overflow-x-auto pb-1 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden"
              >
                <template v-if="showMobileToolbar">
                  <Button
                    v-for="item in toolbarPrimaryShortcuts"
                    :key="item.id"
                    size="sm"
                    variant="outline"
                    class="h-9 shrink-0 rounded-xl border-border/70 bg-background/80 px-3 shadow-none"
                    :disabled="toolbarDisabled"
                    @pointerdown="keepTerminalFocused"
                    @click="sendToolbarShortcut(item.value)"
                  >
                    {{ item.label }}
                  </Button>

                  <Button
                    v-for="modifier in Object.keys(toolbarModifierLabels)"
                    :key="modifier"
                    size="sm"
                    variant="outline"
                    class="h-9 shrink-0 rounded-xl px-3 shadow-none transition-colors"
                    :class="
                      armedModifier === modifier
                        ? 'border-primary/50 bg-primary/10 text-primary'
                        : 'border-border/70 bg-background/80'
                    "
                    :aria-pressed="armedModifier === modifier"
                    :disabled="toolbarDisabled"
                    @pointerdown="keepTerminalFocused"
                    @click="toggleArmedModifier(modifier as ArmedModifier)"
                  >
                    {{ toolbarModifierLabels[modifier as ArmedModifier] }}
                  </Button>

                  <Button
                    v-for="item in toolbarNavigationShortcuts"
                    :key="item.id"
                    size="sm"
                    variant="outline"
                    class="h-9 shrink-0 rounded-xl border-border/70 bg-background/80 px-3 shadow-none"
                    :disabled="toolbarDisabled"
                    @pointerdown="keepTerminalFocused"
                    @click="sendToolbarShortcut(item.value)"
                  >
                    {{ item.label }}
                  </Button>

                  <div class="h-8 w-px shrink-0 bg-border/70" />

                  <div class="flex shrink-0 items-center gap-1.5">
                    <Button
                      size="sm"
                      variant="ghost"
                      class="h-9 rounded-xl px-3 text-[13px] font-semibold"
                      @pointerdown="keepTerminalFocused"
                      @click="nudgeTerminalFontSize(-1)"
                    >
                      A-
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      class="h-9 min-w-[64px] rounded-xl px-3 font-mono text-[12px] text-muted-foreground"
                      @pointerdown="keepTerminalFocused"
                      @click="resetTerminalFontSize"
                    >
                      {{ terminalFontSize }}px
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      class="h-9 rounded-xl px-3 text-[13px] font-semibold"
                      @pointerdown="keepTerminalFocused"
                      @click="nudgeTerminalFontSize(1)"
                    >
                      A+
                    </Button>
                  </div>

                  <div
                    v-if="armedModifier"
                    class="shrink-0 rounded-xl border border-primary/35 bg-primary/10 px-3 py-2 text-[11px] font-medium text-primary"
                  >
                    {{
                      t("admin.webTerminal.modifierLocked", {
                        modifier: armedModifierLabel,
                      })
                    }}
                  </div>
                </template>
              </div>
            </div>

            <div
              ref="terminalStatusRef"
              class="shrink-0 flex flex-col gap-2 text-xs text-muted-foreground sm:flex-row sm:items-center sm:justify-between"
            >
              <span
                v-if="connectionState === 'connecting'"
                class="inline-flex items-center gap-1.5"
              >
                <LoaderCircle class="h-3.5 w-3.5 animate-spin" />
                <span>{{ t("admin.webTerminal.connecting") }}</span>
              </span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>

    <Dialog v-model:open="sendDialogOpen">
      <DialogContent
        class="sm:max-w-[560px]"
        @close-auto-focus="focusTerminalAfterDialogClose"
      >
        <DialogHeader>
          <DialogTitle>{{ t("admin.webTerminal.sendDialogTitle") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.webTerminal.sendDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <Textarea
          id="terminal-send-payload"
          v-model="sendDialogPayload"
          class="min-h-[180px] font-mono text-sm"
          :placeholder="t('admin.webTerminal.sendDialogPlaceholder')"
          :disabled="toolbarDisabled || isSendingDialogPayload"
          @keydown.ctrl.enter.prevent="submitSendDialog"
        />

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isSendingDialogPayload"
            @click="sendDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="
              !sendDialogPayload.length ||
              toolbarDisabled ||
              isSendingDialogPayload
            "
            @click="submitSendDialog"
          >
            <LoaderCircle
              v-if="isSendingDialogPayload"
              class="mr-1.5 h-4 w-4 animate-spin"
            />
            {{ t("admin.webTerminal.send") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="renameDialogOpen">
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>{{ t("admin.webTerminal.renameSession") }}</DialogTitle>
          <DialogDescription>
            {{ t("admin.webTerminal.renameDialogDescription") }}
          </DialogDescription>
        </DialogHeader>

        <Input
          v-model="renameDialogValue"
          :placeholder="t('admin.webTerminal.renameDialogPlaceholder')"
          :disabled="isRenamingSession"
          @keydown.enter.prevent="submitRenameDialog"
        />

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isRenamingSession"
            @click="renameDialogOpen = false"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            :disabled="!renameDialogValue.trim().length || isRenamingSession"
            @click="submitRenameDialog"
          >
            <LoaderCircle
              v-if="isRenamingSession"
              class="mr-1.5 h-4 w-4 animate-spin"
            />
            {{ t("common.save") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
