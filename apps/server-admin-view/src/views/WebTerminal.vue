<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useRouter } from "vue-router";
import { init as initGhostty, Terminal, FitAddon } from "ghostty-web";
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
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCcw,
  Send,
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

let ghosttyInitPromise: Promise<void> | null = null;
const textEncoder = new TextEncoder();

const ensureGhostty = () => {
  if (!ghosttyInitPromise) {
    ghosttyInitPromise = initGhostty();
  }
  return ghosttyInitPromise;
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
const DEFAULT_TERMINAL_FONT_SIZE = 14;
const DEFAULT_TERMINAL_FONT_SIZE_MOBILE = 12;
const MIN_TERMINAL_FONT_SIZE = 13;
const MAX_TERMINAL_FONT_SIZE = 20;
const TERMINAL_TOUCH_DRAG_THRESHOLD_PX = 10;
type ArmedModifier = "ctrl" | "alt";
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
const terminalFrameRef = ref<HTMLElement | null>(null);
const mobileAccessoryBarRef = ref<HTMLElement | null>(null);
const terminalStatusRef = ref<HTMLElement | null>(null);
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

let term: Terminal | null = null;
let fitAddon: FitAddon | null = null;
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
let outputTextDecoder = new TextDecoder();
let pendingLegacyTitleSequence = "";

const selectedSession = computed(
  () =>
    sessions.value.find((session) => session.id === selectedSessionId.value) ||
    null,
);
const terminalWindowTitle = computed(
  () => selectedSession.value?.title?.trim() || "Web终端",
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
    return "结束后当前网页终端会话会立即断开并删除，此操作不可恢复。";
  }

  return `结束后会话“${title}”会立即断开并删除，此操作不可恢复。`;
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
const terminalFrameStyle = computed(() =>
  compactViewport.value
    ? {
        height: terminalHeight.value,
        minHeight: terminalHeight.value,
      }
    : {
        maxHeight: terminalHeight.value,
      },
);

const statusTone = computed(() => {
  if (connectionState.value === "connected") return "已连接";
  if (connectionState.value === "connecting") return "准备连接";
  if (connectionState.value === "error") return "连接异常";
  return "尚未连接";
});

const encodeInputToBase64 = (value: string): string => {
  const bytes = textEncoder.encode(value);
  let binary = "";
  bytes.forEach((byte) => {
    binary += String.fromCharCode(byte);
  });
  return btoa(binary);
};

const getInputByteLength = (value: string): number =>
  textEncoder.encode(value).byteLength;

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

const applyTerminalFit = () => {
  if (!term || !fitAddon) return;

  const dimensions = fitAddon.proposeDimensions();
  if (!dimensions) return;
  if (dimensions.cols === term.cols && dimensions.rows === term.rows) return;

  term.resize(dimensions.cols, dimensions.rows);
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

const clearTerminal = () => {
  resetOutputState();
  if (!term) return;

  term.clear?.();
  term.reset();
  term.write("\u001b[2J\u001b[3J\u001b[H");
  term.focus();
};
const focusTerminal = () => {
  term?.focus();
  void nextTick(() => term?.focus());
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
      term.write(payload);
    }
  }

  lastOutputCursor = chunk.cursor;
  nextTick(() => term?.focus());
};

const syncViewportHeight = () => {
  compactViewport.value = detectCompactViewport();
  const measurementTarget = terminalFrameRef.value || terminalShellRef.value;
  if (!measurementTarget) return;

  const rect = measurementTarget.getBoundingClientRect();
  const viewport = window.visualViewport?.height || window.innerHeight;
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
    viewport - rect.top - reservedHeight,
  );
  const nextHeight =
    available > 0
      ? compactViewport.value
        ? available
        : Math.min(MAX_TERMINAL_HEIGHT_DESKTOP_PX, available)
      : DEFAULT_TERMINAL_HEIGHT_PX;
  terminalHeight.value = `${nextHeight}px`;
  scheduleTerminalFit();
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
        error instanceof Error ? error.message : "终端输入发送失败";
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
    console.warn("[terminal] buffering early input before attachment is ready", {
      connectionState: connectionState.value,
      selectedSessionId: selectedSessionId.value || null,
      byteLength: getInputByteLength(data),
      immediate: options?.immediate === true,
    });
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
    throw new Error("当前没有可用的终端连接");
  }

  try {
    await TerminalAPI.sendInput(attachmentId, encodeInputToBase64(payload));
  } catch (error) {
    console.error(error);
    connectionState.value = "error";
    connectionError.value =
      error instanceof Error ? error.message : "终端输入发送失败";
    throw error;
  }
};

const openSendDialog = () => {
  if (toolbarDisabled.value) return;
  sendDialogOpen.value = true;
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
    toast.error("切换会话失败", {
      description:
        error instanceof Error ? error.message : "无法切换到所选终端会话",
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
    toast.error("重命名失败", {
      description: error instanceof Error ? error.message : "无法更新会话名称",
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
    toast.error("发送失败", {
      description:
        error instanceof Error ? error.message : "无法将内容发送到终端",
    });
  } finally {
    isSendingDialogPayload.value = false;
  }
};

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
        timeout_ms: 15_000,
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
        error instanceof Error ? error.message : "HTTP 长轮询已断开";
      return;
    }
  }
};

const ensureTerminalReady = async () => {
  if (term) return;
  await nextTick();
  await initializeTerminal();
  if (!term) {
    throw new Error("终端视图尚未准备完成");
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
      console.warn("[terminal] clearing buffered input after attachment failed", {
        sessionId: session.id,
        bufferedBytes: pendingInputBytes,
      });
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
      toast.success("终端会话已创建");
    }
    return session;
  } catch (error) {
    toast.error("创建失败", {
      description: error instanceof Error ? error.message : "无法创建终端会话",
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
    toast.error("重连失败", {
      description: error instanceof Error ? error.message : "无法重新附着终端",
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
    toast.success("终端会话已结束");
  } catch (error) {
    toast.error("结束失败", {
      description: error instanceof Error ? error.message : "无法结束终端会话",
    });
  } finally {
    isKilling.value = false;
  }
};

const initializeTerminal = async () => {
  if (!terminalMountRef.value || term) return;
  await ensureGhostty();
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
  bindTerminalTouchGestures();
  applyTerminalFit();
  fitAddon.observeResize();
  scheduleTerminalFit();
  term.focus();
  term.onData((data) => {
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
      error instanceof Error ? error.message : "网页终端初始化失败";
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
        error instanceof Error ? error.message : "网页终端初始化失败";
    }
  }
};

onMounted(async () => {
  compactViewport.value = detectCompactViewport();
  loadTerminalFontSize();
  await bootstrapPage();
  window.addEventListener("resize", syncViewportHeight);
  window.visualViewport?.addEventListener("resize", syncViewportHeight);
  window.visualViewport?.addEventListener("scroll", syncViewportHeight);
});

watch(
  [
    () => sessions.value.length,
    connectionState,
    connectionError,
    showMobileAccessoryBar,
  ],
  () => {
    void nextTick().then(() => {
      syncViewportHeight();
    });
  },
);

onBeforeUnmount(() => {
  unbindTerminalTouchGestures();
  window.removeEventListener("resize", syncViewportHeight);
  window.visualViewport?.removeEventListener("resize", syncViewportHeight);
  window.visualViewport?.removeEventListener("scroll", syncViewportHeight);
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
      <AlertTitle>网页终端尚未启用</AlertTitle>
      <AlertDescription class="space-y-3">
        <p>请先在系统设置里开启终端功能，再回到这里创建和附着会话。</p>
        <Button size="sm" @click="router.push('/system?tab=terminal')">
          前往系统设置
        </Button>
      </AlertDescription>
    </Alert>

    <Alert
      v-else-if="runtimeStatus?.blockedReason"
      variant="destructive"
      class="border-destructive/40"
    >
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>当前不可用</AlertTitle>
      <AlertDescription>{{ runtimeStatus.blockedReason }}</AlertDescription>
    </Alert>

    <div v-else ref="terminalShellRef" class="min-h-[80vh] min-w-0">
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
                  active-label="已连接"
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
                  aria-label="新建终端会话"
                  title="新建会话"
                  @click="createSession"
                >
                  <LoaderCircle
                    v-if="isCreating"
                    class="h-4 w-4 animate-spin"
                  />
                  <Plus v-else class="h-4 w-4" />
                  <span class="sr-only">新建会话</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="!selectedSession || isRenamingSession"
                  aria-label="重命名会话"
                  title="重命名会话"
                  @click="openRenameDialog"
                >
                  <LoaderCircle
                    v-if="isRenamingSession"
                    class="h-4 w-4 animate-spin"
                  />
                  <Pencil v-else class="h-4 w-4" />
                  <span class="sr-only">重命名会话</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="
                    !selectedSession || connectionState === 'connecting'
                  "
                  aria-label="重连终端"
                  title="重连"
                  @pointerdown="keepTerminalFocused"
                  @click="reconnectSession"
                >
                  <RefreshCcw class="h-4 w-4" />
                  <span class="sr-only">重连</span>
                </Button>
                <Button
                  variant="outline"
                  size="icon-sm"
                  class="rounded-lg border-border/70 bg-background/85 shadow-none"
                  :disabled="toolbarDisabled"
                  aria-label="发送内容到终端"
                  title="发送"
                  @click="openSendDialog"
                >
                  <Send class="h-4 w-4" />
                  <span class="sr-only">发送</span>
                </Button>
              </div>

              <div class="h-8 w-px shrink-0 bg-border/70" />

              <ConfirmDangerPopover
                title="确认结束会话？"
                :description="destroySessionDescription"
                confirm-text="结束会话"
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
                    aria-label="结束当前会话"
                    title="结束会话"
                  >
                    <Trash2 class="h-4 w-4" />
                    <span class="sr-only">结束会话</span>
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
            初始化时未能生成默认终端会话。点击左上角加号可重新创建一个可恢复的
            tmux 会话。
          </div>

          <Alert
            v-if="connectionError"
            variant="destructive"
            class="shrink-0 border-destructive/40"
          >
            <AlertTriangle class="h-4 w-4" />
            <AlertTitle>终端连接异常</AlertTitle>
            <AlertDescription>{{ connectionError }}</AlertDescription>
          </Alert>

          <div
            v-if="!isBooting && sessions.length > 0"
            class="flex min-h-0 flex-1 flex-col gap-2.5 sm:gap-3"
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
                  <span
                    class="h-[11px] w-[11px] rounded-full border border-black/18 bg-[#28c840] shadow-[inset_0_1px_0.5px_rgba(255,255,255,0.18)]"
                  />
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
                />
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


                  <div
                  class="flex shrink-0 items-center gap-1.5"
                >
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
                    {{ armedModifierLabel }} 已锁定
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
                <span>连接中…</span>
              </span>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>

    <Dialog v-model:open="sendDialogOpen">
      <DialogContent class="sm:max-w-[560px]">
        <DialogHeader>
          <DialogTitle>发送到终端</DialogTitle>
          <DialogDescription>
            输入要粘贴到当前终端的内容，发送后会按原样写入会话。
          </DialogDescription>
        </DialogHeader>

        <Textarea
          v-model="sendDialogPayload"
          class="min-h-[180px] font-mono text-sm"
          placeholder="输入要发送到终端的内容"
          :disabled="toolbarDisabled || isSendingDialogPayload"
          @keydown.ctrl.enter.prevent="submitSendDialog"
        />

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isSendingDialogPayload"
            @click="sendDialogOpen = false"
          >
            取消
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
            发送
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>

    <Dialog v-model:open="renameDialogOpen">
      <DialogContent class="sm:max-w-[420px]">
        <DialogHeader>
          <DialogTitle>重命名会话</DialogTitle>
          <DialogDescription>
            为当前终端会话设置一个更容易识别的名称。
          </DialogDescription>
        </DialogHeader>

        <Input
          v-model="renameDialogValue"
          placeholder="输入会话名称"
          :disabled="isRenamingSession"
          @keydown.enter.prevent="submitRenameDialog"
        />

        <DialogFooter>
          <Button
            variant="outline"
            :disabled="isRenamingSession"
            @click="renameDialogOpen = false"
          >
            取消
          </Button>
          <Button
            :disabled="!renameDialogValue.trim().length || isRenamingSession"
            @click="submitRenameDialog"
          >
            <LoaderCircle
              v-if="isRenamingSession"
              class="mr-1.5 h-4 w-4 animate-spin"
            />
            保存
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
