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
import { Card, CardContent } from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { LoaderCircle } from "lucide-vue-next";
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
import {
  RECENT_SESSION_KEY,
  ensureGhostty,
  toolbarModifierLabels,
  toolbarNavigationShortcuts,
  toolbarPrimaryShortcuts,
  type ArmedModifier,
  type GhosttyModule,
} from "./web-terminal/terminal-runtime";
import { focusElementWithoutScroll } from "./web-terminal/terminal-dom";
import {
  createLegacyTitleSequenceStripper,
  decodeBase64ToBytes,
  encodeCtrlInput,
} from "./web-terminal/terminal-input";
import {
  createTerminalMouseReporter,
} from "./web-terminal/terminal-mouse";
import { createTerminalFitController } from "./web-terminal/terminal-fit";
import { createTerminalTouchGestures } from "./web-terminal/terminal-touch";
import { useTerminalInputQueue } from "./web-terminal/useTerminalInputQueue";
import { useTerminalResizeQueue } from "./web-terminal/useTerminalResizeQueue";
import { useTerminalFontSize } from "./web-terminal/useTerminalFontSize";
import { useTerminalContextMenu } from "./web-terminal/useTerminalContextMenu";
import { useTerminalViewportLayout } from "./web-terminal/useTerminalViewportLayout";
import { useTerminalDialogs } from "./web-terminal/useTerminalDialogs";
import TerminalConnectionErrorAlert from "./web-terminal/TerminalConnectionErrorAlert.vue";
import TerminalContextMenu from "./web-terminal/TerminalContextMenu.vue";
import TerminalGateAlerts from "./web-terminal/TerminalGateAlerts.vue";
import TerminalMobileToolbar from "./web-terminal/TerminalMobileToolbar.vue";
import TerminalRenameDialog from "./web-terminal/TerminalRenameDialog.vue";
import TerminalSendDialog from "./web-terminal/TerminalSendDialog.vue";
import TerminalSessionToolbar from "./web-terminal/TerminalSessionToolbar.vue";
import TerminalWindowChrome from "./web-terminal/TerminalWindowChrome.vue";

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
const isPinchZooming = ref(false);
const armedModifier = ref<ArmedModifier | null>(null);

let term: InstanceType<GhosttyModule["Terminal"]> | null = null;
let fitAddon: InstanceType<GhosttyModule["FitAddon"]> | null = null;
let pollGeneration = 0;
let lastOutputCursor = 0;
let outputTextDecoder = new TextDecoder();
const legacyTitleSequenceStripper = createLegacyTitleSequenceStripper();
let remoteOutputWriteDepth = 0;
let terminalInternalResponseDropDepth = 0;
const terminalFitController = createTerminalFitController({
  getFitAddon: () => fitAddon,
  getMountElement: () => terminalMountRef.value,
  getTerminal: () => term,
  runTerminalMutation: (mutation) =>
    runTerminalInternalMutation(mutation, { dropResponses: true }),
});
const {
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
} = useTerminalViewportLayout({
  focusTerminal: () => focusTerminal(),
  scheduleFit: () => terminalFitController.schedule(),
  syncTerminalTextInputAnchor: () => syncTerminalTextInputAnchor(),
});
const {
  applyTerminalFontSize,
  loadTerminalFontSize,
  nudgeTerminalFontSize,
  persistTerminalFontSize,
  resetTerminalFontSize,
  terminalFontSize,
} = useTerminalFontSize({
  compactViewport,
  getTerminal: () => term,
  scheduleFit: () => terminalFitController.schedule(),
});
const terminalTouchGestures = createTerminalTouchGestures({
  applyFontSize: (value, options) => applyTerminalFontSize(value, options),
  compactViewport,
  getMountElement: () => terminalMountRef.value,
  getTerminal: () => term,
  isPinchZooming,
  persistFontSize: () => persistTerminalFontSize(),
  terminalFontSize,
});
const {
  clearPendingInput,
  flushPendingInput,
  getPendingInputSnapshot,
  queueRemoteTerminalResponse,
  queueTerminalInput,
  sendTerminalPayloadNow,
} = useTerminalInputQueue({
  activeAttachment,
  connectionError,
  connectionState,
  selectedSessionId,
  sendInput: (attachmentId, payload) =>
    TerminalAPI.sendInput(attachmentId, payload),
  translate: (key) => t(key),
});
const terminalMouseReporter = createTerminalMouseReporter({
  focusTerminal: () => focusTerminal(),
  getFrameElement: () => terminalFrameRef.value,
  getMountElement: () => terminalMountRef.value,
  getRowHeight: () => terminalTouchGestures.getRowHeight(),
  getTerminal: () => term,
  queueInput: (payload) => queueTerminalInput(payload, { immediate: true }),
});
const {
  flushPendingResize,
  markSyncedResize,
  resetResizeState,
  scheduleResize,
} = useTerminalResizeQueue({
  activeAttachment,
  getTerminal: () => term,
  resizeAttachment: (attachmentId, cols, rows) =>
    TerminalAPI.resizeAttachment(attachmentId, cols, rows),
  restartPollingFromSnapshot: (attachment) =>
    restartHttpPollingFromSnapshot(attachment),
  sessions,
});

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

const toolbarDisabled = computed(() => !activeAttachment.value);
const armedModifierLabel = computed(() =>
  armedModifier.value ? toolbarModifierLabels[armedModifier.value] : "",
);
const terminalFullscreenLabel = computed(() =>
  isTerminalFullscreen.value
    ? t("admin.webTerminal.exitFullscreen")
    : t("admin.webTerminal.enterFullscreen"),
);

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

const bindTerminalMouseReporting = () => {
  terminalMouseReporter.bind();
};

const unbindTerminalMouseReporting = () => {
  terminalMouseReporter.unbind();
};

const scheduleTerminalFit = () => {
  terminalFitController.schedule();
};

const observeTerminalMountSize = () => {
  terminalFitController.observeMountSize();
};

const bindTerminalTouchGestures = () => {
  terminalTouchGestures.bind();
};

const unbindTerminalTouchGestures = () => {
  terminalTouchGestures.unbind();
};

const rememberRecentSession = (sessionId: string) => {
  localStorage.setItem(RECENT_SESSION_KEY, sessionId);
};

const resetOutputState = () => {
  lastOutputCursor = 0;
  outputTextDecoder = new TextDecoder();
  legacyTitleSequenceStripper.reset();
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

const {
  focusTerminalAfterDialogClose,
  isRenamingSession,
  isSendingDialogPayload,
  openManualPasteDialog,
  openRenameDialog,
  openSendDialog,
  renameDialogOpen,
  renameDialogValue,
  sendDialogOpen,
  sendDialogPayload,
  submitRenameDialog,
  submitSendDialog,
} = useTerminalDialogs({
  activeAttachment,
  clearArmedModifier: () => clearArmedModifier(),
  focusTerminal: () => focusTerminal(),
  selectedSession,
  sendPayloadNow: (payload) => sendTerminalPayloadNow(payload),
  sessions,
  translate: (key) => t(key),
  updateSessionTitle: (sessionId, title) =>
    TerminalAPI.updateSessionTitle(sessionId, title),
});

const {
  closeTerminalContextMenu,
  copyTerminalSelectionFromMenu,
  handleDocumentPointerDown,
  handleTerminalContextMenu,
  pasteClipboardToTerminal,
  selectAllTerminalText,
  setTerminalContextMenuRef,
  terminalContextMenuHasSelection,
  terminalContextMenuOpen,
  terminalContextMenuStyle,
} = useTerminalContextMenu({
  activeAttachment,
  clearArmedModifier: () => clearArmedModifier(),
  focusTerminal: () => focusTerminal(),
  getTerminal: () => term,
  openManualPasteDialog: () => openManualPasteDialog(),
  translate: (key) => t(key),
});

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
    legacyTitleSequenceStripper.reset();
  }

  if (chunk.data_base64) {
    const payload = legacyTitleSequenceStripper.strip(
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

const refreshSessions = async () => {
  sessions.value = await TerminalAPI.listSessions();
  if (
    selectedSessionId.value &&
    !sessions.value.some((item) => item.id === selectedSessionId.value)
  ) {
    selectedSessionId.value = "";
  }
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

function restartHttpPollingFromSnapshot(attachment: TerminalAttachmentRecord) {
  if (activeAttachment.value?.id !== attachment.id) return;

  resetOutputState();
  void startHttpPolling(attachment);
}

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
    const pendingInput = getPendingInputSnapshot();
    if (pendingInput.hasPendingInput) {
      console.warn(
        "[terminal] clearing buffered input after attachment failed",
        {
          sessionId: session.id,
          bufferedBytes: pendingInput.byteLength,
        },
      );
    }
    clearPendingInput();
    throw error;
  }

  activeAttachment.value = attachment;
  const pendingInput = getPendingInputSnapshot();
  if (pendingInput.hasPendingInput) {
    console.warn("[terminal] attachment ready, flushing buffered input", {
      sessionId: session.id,
      attachmentId: attachment.id,
      bufferedBytes: pendingInput.byteLength,
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
  const { Terminal, FitAddon, ghostty } = await ensureGhostty();
  term = new Terminal({
    ghostty,
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
  terminalFitController.apply();
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
      error instanceof Error
        ? error.message
        : t("admin.webTerminal.initFailed");
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
  startViewportTracking();
  loadTerminalFontSize();
  await bootstrapPage();
  window.addEventListener("keydown", handleWindowKeydown);
  document.addEventListener("pointerdown", handleDocumentPointerDown);
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
  stopViewportTracking();
  window.removeEventListener("keydown", handleWindowKeydown);
  document.removeEventListener("pointerdown", handleDocumentPointerDown);
  terminalFitController.dispose();
  void stopCurrentConnection();
  fitAddon?.dispose();
  term?.dispose();
  fitAddon = null;
  term = null;
});
</script>

<template>
  <div class="flex h-full min-w-0 flex-col gap-3 sm:gap-4">
    <TerminalGateAlerts
      v-if="!terminalEnabled || runtimeStatus?.blockedReason"
      :blocked-reason="runtimeStatus?.blockedReason || ''"
      :terminal-enabled="terminalEnabled"
      @go-settings="router.push('/system?tab=terminal')"
    />

    <div
      v-else
      :ref="setTerminalShellRef"
      class="min-h-0 min-w-0 md:min-h-[80vh]"
    >
      <Card class="min-h-0 min-w-0 h-full py-3 sm:py-6">
        <CardContent
          class="flex h-full min-h-0 min-w-0 flex-col gap-2.5 px-3 sm:gap-3 sm:px-6"
        >
          <TerminalSessionToolbar
            :connection-state="connectionState"
            :create-session="createSession"
            :destroy-selected-session="destroySelectedSession"
            :destroy-session-description="destroySessionDescription"
            :handle-session-tab-change="handleSessionTabChange"
            :is-booting="isBooting"
            :is-creating="isCreating"
            :is-killing="isKilling"
            :is-renaming-session="isRenamingSession"
            :keep-terminal-focused="keepTerminalFocused"
            :open-rename-dialog="openRenameDialog"
            :open-send-dialog="openSendDialog"
            :reconnect-session="reconnectSession"
            :selected-session="selectedSession"
            :selected-session-id="selectedSessionId"
            :sessions="sessions"
            :status-tone="statusTone"
            :toolbar-disabled="toolbarDisabled"
          />

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

          <TerminalConnectionErrorAlert :message="connectionError" />

          <div
            v-if="!isBooting && sessions.length > 0"
            :ref="setTerminalPanelRef"
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
              <TerminalWindowChrome
                :fullscreen="isTerminalFullscreen"
                :fullscreen-label="terminalFullscreenLabel"
                :title="terminalWindowTitle"
                :toggle-fullscreen="toggleTerminalFullscreen"
              />

              <div class="relative min-h-0 flex-1 bg-[#1c1c1e]">
                <div
                  class="pointer-events-none absolute inset-x-0 top-0 h-px bg-white/4"
                />
                <div
                  ref="terminalMountRef"
                  class="absolute inset-0 box-border min-h-0 min-w-0 overflow-hidden px-1.5 py-2.5 sm:px-2.5 sm:py-3"
                  @contextmenu.capture="handleTerminalContextMenu"
                />
                <TerminalContextMenu
                  :ref="setTerminalContextMenuRef"
                  :can-paste="!!activeAttachment"
                  :has-selection="terminalContextMenuHasSelection"
                  :menu-style="terminalContextMenuStyle"
                  :open="terminalContextMenuOpen"
                  @copy="copyTerminalSelectionFromMenu"
                  @paste="pasteClipboardToTerminal"
                  @select-all="selectAllTerminalText"
                />
              </div>
            </div>

            <div
              v-if="showMobileAccessoryBar"
              :ref="setMobileAccessoryBarRef"
              class="shrink-0 rounded-2xl border border-border/70 bg-muted/15 p-2 pb-[calc(env(safe-area-inset-bottom)+0.5rem)] sm:p-2.5 sm:pb-2.5"
            >
              <TerminalMobileToolbar
                :armed-modifier="armedModifier"
                :armed-modifier-label="armedModifierLabel"
                :disabled="toolbarDisabled"
                :font-size="terminalFontSize"
                :keep-focused="keepTerminalFocused"
                :modifier-labels="toolbarModifierLabels"
                :navigation-shortcuts="toolbarNavigationShortcuts"
                :nudge-font-size="nudgeTerminalFontSize"
                :primary-shortcuts="toolbarPrimaryShortcuts"
                :reset-font-size="resetTerminalFontSize"
                :send-shortcut="sendToolbarShortcut"
                :show="showMobileToolbar"
                :toggle-modifier="toggleArmedModifier"
              />
            </div>

            <div
              :ref="setTerminalStatusRef"
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

    <TerminalSendDialog
      v-model:open="sendDialogOpen"
      v-model:payload="sendDialogPayload"
      :disabled="toolbarDisabled"
      :on-close-auto-focus="focusTerminalAfterDialogClose"
      :sending="isSendingDialogPayload"
      @submit="submitSendDialog"
    />

    <TerminalRenameDialog
      v-model:open="renameDialogOpen"
      v-model:value="renameDialogValue"
      :renaming="isRenamingSession"
      @submit="submitRenameDialog"
    />
  </div>
</template>
