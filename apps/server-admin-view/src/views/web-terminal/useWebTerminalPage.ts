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
import { TerminalAPI } from "@/lib/api/terminal";
import type {
  TerminalAttachmentRecord,
  TerminalRuntimeStatus,
  TerminalSessionRecord,
  TerminalTransport,
} from "../../types";
import { useConfigStore } from "../../store/config";
import { toolbarModifierLabels } from "./terminal-runtime";
import { useTerminalInputQueue } from "./useTerminalInputQueue";
import { useTerminalResizeQueue } from "./useTerminalResizeQueue";
import { useTerminalFontSize } from "./useTerminalFontSize";
import { useTerminalContextMenu } from "./useTerminalContextMenu";
import { useTerminalViewportLayout } from "./useTerminalViewportLayout";
import { useTerminalDialogs } from "./useTerminalDialogs";
import { useTerminalSessionController } from "./useTerminalSessionController";
import { useTerminalEmulator } from "./useTerminalEmulator";

export const useWebTerminalPage = () => {
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

  // The viewport callbacks close over the emulator before it is initialized.
  // eslint-disable-next-line prefer-const
  let emulator!: ReturnType<typeof useTerminalEmulator>;
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
    focusTerminal: () => emulator.focusTerminal(),
    scheduleFit: () => emulator.scheduleFit(),
    syncTerminalTextInputAnchor: () => emulator.syncTerminalTextInputAnchor(),
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
    getTerminal: () => emulator.getTerminal(),
    scheduleFit: () => emulator.scheduleFit(),
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
  const {
    flushPendingResize,
    markSyncedResize,
    resetResizeState,
    scheduleResize,
  } = useTerminalResizeQueue({
    activeAttachment,
    getTerminal: () => emulator.getTerminal(),
    resizeAttachment: (attachmentId, cols, rows) =>
      TerminalAPI.resizeAttachment(attachmentId, cols, rows),
    restartPollingFromSnapshot: (attachment) =>
      restartHttpPollingFromSnapshot(attachment),
    sessions,
  });

  emulator = useTerminalEmulator({
    applyFontSize: applyTerminalFontSize,
    canAcceptInput: () => Boolean(activeAttachment.value),
    compactViewport,
    persistFontSize: persistTerminalFontSize,
    queueInput: queueTerminalInput,
    queueRemoteResponse: queueRemoteTerminalResponse,
    scheduleResize,
    terminalFontSize,
    terminalFrameRef,
    translate: (key) => t(key),
  });
  const {
    applyOutputChunk,
    armedModifier,
    clearArmedModifier,
    clearTerminal,
    ensureTerminalReady,
    focusTerminal,
    isPinchZooming,
    resetOutputState,
    terminalMountRef,
    toggleArmedModifier,
  } = emulator;
  const setTerminalFrameElement = (element: unknown) => {
    terminalFrameRef.value = element as HTMLElement | null;
  };
  const setTerminalMountElement = (element: unknown) => {
    terminalMountRef.value = element as HTMLElement | null;
  };

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
    return title
      ? t("admin.webTerminal.destroyDescriptionWithTitle", { title })
      : t("admin.webTerminal.destroyDescription");
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
  let disposed = false;

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
    clearArmedModifier,
    focusTerminal,
    selectedSession,
    sendPayloadNow: sendTerminalPayloadNow,
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
    clearArmedModifier,
    focusTerminal,
    getTerminal: () => emulator.getTerminal(),
    openManualPasteDialog,
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
    if (event instanceof PointerEvent && event.pointerType !== "mouse") return;
    event.preventDefault();
    focusTerminal();
  };
  const sendToolbarShortcut = (value: string) => {
    clearArmedModifier();
    queueTerminalInput(value, { immediate: true });
    focusTerminal();
  };

  const {
    bootstrapPage,
    createSession,
    dispose: disposeTerminalSession,
    destroySelectedSession,
    handleSessionTabChange,
    reconnectSession,
    restartHttpPollingFromSnapshot,
  } = useTerminalSessionController({
    activeAttachment,
    activeTransport,
    applyOutputChunk,
    clearArmedModifier,
    clearPendingInput,
    clearTerminal,
    connectionError,
    connectionState,
    ensureConfig: async () => {
      if (!configStore.config) await configStore.loadConfig();
    },
    ensureTerminalReady,
    flushPendingInput,
    flushPendingResize,
    getOutputCursor: emulator.getOutputCursor,
    getPendingInputSnapshot,
    getTerminalSize: emulator.getTerminalSize,
    isBooting,
    isCreating,
    isKilling,
    markSyncedResize,
    onBootstrapLayoutReady: syncViewportHeight,
    resetOutputState,
    resetResizeState,
    runtimeStatus,
    scheduleResize,
    selectedSession,
    selectedSessionId,
    sessions,
    terminalEnabled,
  });

  onMounted(async () => {
    startViewportTracking();
    loadTerminalFontSize();
    await bootstrapPage();
    if (disposed) return;
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
      void nextTick().then(syncViewportHeight);
    },
  );
  onBeforeUnmount(() => {
    disposed = true;
    stopViewportTracking();
    window.removeEventListener("keydown", handleWindowKeydown);
    document.removeEventListener("pointerdown", handleDocumentPointerDown);
    emulator.dispose();
    void disposeTerminalSession();
  });

  return {
    activeAttachment,
    armedModifier,
    armedModifierLabel,
    closeTerminalContextMenu,
    connectionError,
    connectionState,
    copyTerminalSelectionFromMenu,
    createSession,
    destroySelectedSession,
    destroySessionDescription,
    focusTerminalAfterDialogClose,
    handleSessionTabChange,
    handleTerminalContextMenu,
    isBooting,
    isCreating,
    isKilling,
    isPinchZooming,
    isRenamingSession,
    isSendingDialogPayload,
    isTerminalFullscreen,
    keepTerminalFocused,
    nudgeTerminalFontSize,
    openRenameDialog,
    openSendDialog,
    pasteClipboardToTerminal,
    reconnectSession,
    renameDialogOpen,
    renameDialogValue,
    resetTerminalFontSize,
    router,
    runtimeStatus,
    selectAllTerminalText,
    selectedSession,
    selectedSessionId,
    sendDialogOpen,
    sendDialogPayload,
    sendToolbarShortcut,
    sessions,
    setMobileAccessoryBarRef,
    setTerminalContextMenuRef,
    setTerminalFrameElement,
    setTerminalMountElement,
    setTerminalPanelRef,
    setTerminalShellRef,
    setTerminalStatusRef,
    showMobileAccessoryBar,
    showMobileToolbar,
    statusTone,
    submitRenameDialog,
    submitSendDialog,
    t,
    terminalContextMenuHasSelection,
    terminalContextMenuOpen,
    terminalContextMenuStyle,
    terminalEnabled,
    terminalFontSize,
    terminalFrameStyle,
    terminalFullscreenLabel,
    terminalPanelClass,
    terminalPanelStyle,
    terminalWindowSubtitle,
    terminalWindowTitle,
    toggleArmedModifier,
    toggleTerminalFullscreen,
    toolbarDisabled,
  };
};

export type WebTerminalPageController = ReturnType<typeof useWebTerminalPage>;
