import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import { useTerminalAttachment } from "./useTerminalAttachment";
import { extractTerminalErrorMessage } from "./terminal-errors";
import { useTerminalEmulator } from "./useTerminalEmulator";
import { useTerminalFontSize } from "./useTerminalFontSize";
import { useTerminalInputQueue } from "./useTerminalInputQueue";
import { useTerminalInteractions } from "./useTerminalInteractions";
import { useTerminalPresentation } from "./useTerminalPresentation";
import { useTerminalResizeQueue } from "./useTerminalResizeQueue";
import { useTerminalSessionActions } from "./useTerminalSessionActions";
import { useTerminalSessionConnection } from "./useTerminalSessionConnection";
import { useTerminalSessionRefresh } from "./useTerminalSessionRefresh";
import { useTerminalSessions } from "./useTerminalSessions";
import { useTerminalTargetEditor } from "./useTerminalTargetEditor";
import { normalizeTerminalDimensions } from "./terminal-dimensions";
import { useTerminalTargetDeletion } from "./useTerminalTargetDeletion";
import { useTerminalTargets } from "./useTerminalTargets";
import { useTerminalViewport } from "./useTerminalViewport";

export const useWebTerminalPage = () => {
  const { t } = useI18n();
  const booting = ref(true);
  const pageError = ref("");
  const runtimeRestarted = ref(false);
  let disposed = false;
  let sessionRefreshTimer: number | null = null;

  const targetsController = useTerminalTargets();

  const sessionsController = useTerminalSessions({
    selectedTargetId: targetsController.selectedTargetId,
    onRuntimeChanged: () => {
      runtimeRestarted.value = true;
      void sessionConnection.detach();
      toast.info(
        t(
          "admin.webTerminal.runtimeRestarted",
          "The terminal service restarted. Previous sessions have ended.",
        ),
      );
    },
  });

  const viewport = useTerminalViewport({
    focusTerminal: () => emulator?.focusTerminal(),
    scheduleFit: () => emulator?.scheduleFit(),
    syncTerminalTextInputAnchor: () => emulator?.syncTerminalTextInputAnchor(),
  });

  const fontSize = useTerminalFontSize({
    compactViewport: viewport.compactViewport,
    getTerminal: () => emulator?.getTerminal() ?? null,
    scheduleFit: () => emulator?.scheduleFit(),
  });

  const attachmentController = useTerminalAttachment({
    getTerminalSize: () =>
      normalizeTerminalDimensions(
        emulator?.getTerminalSize() ?? { cols: 120, rows: 32 },
      ),
    onOutput: (event) => emulator?.applyOutputEvent(event),
    onReset: () => emulator?.clearTerminal(),
    onSessionState: (sessionId, phase, details) =>
      sessionsController.updateSessionPhase(sessionId, phase, details),
  });

  const controllingAttachment = computed(() =>
    attachmentController.canInput.value
      ? attachmentController.attachment.value
      : null,
  );

  const inputQueue = useTerminalInputQueue({
    activeAttachment: controllingAttachment,
    connectionState: attachmentController.inputConnectionState,
    onSendError: (reason) => {
      const failure = attachmentController.reportRequestError(
        reason,
        t("admin.webTerminal.inputSendFailed"),
      );
      if (failure.errorCode !== "controller_conflict") {
        void attachmentController.reconnect().catch(() => undefined);
      }
    },
    selectedSessionId: sessionsController.selectedSessionId,
    sendInput: (_attachmentId, payload) =>
      attachmentController.sendInput(payload),
    translate: (key) => t(key),
  });

  const resizeQueue = useTerminalResizeQueue({
    activeAttachment: controllingAttachment,
    getTerminal: () => emulator?.getTerminal() ?? null,
    onResizeSynced: sessionsController.updateSessionDimensions,
    resizeAttachment: (_attachmentId, cols, rows) =>
      attachmentController.resize(cols, rows),
  });

  const emulator = useTerminalEmulator({
    applyFontSize: fontSize.applyTerminalFontSize,
    canAcceptInput: () => attachmentController.canInput.value,
    compactViewport: viewport.compactViewport,
    persistFontSize: fontSize.persistTerminalFontSize,
    queueInput: inputQueue.queueTerminalInput,
    queueRemoteResponse: inputQueue.queueRemoteTerminalResponse,
    scheduleResize: resizeQueue.scheduleResize,
    terminalFontSize: fontSize.terminalFontSize,
    terminalFrameRef: viewport.terminalFrameRef,
    translate: (key) => t(key),
  });

  const sessionConnection = useTerminalSessionConnection({
    attach: attachmentController.attach,
    canInput: attachmentController.canInput,
    clearDetachedState: () => {
      inputQueue.clearPendingInput();
      resizeQueue.resetResizeState();
      emulator.clearTerminal();
    },
    detachAttachment: attachmentController.detach,
    ensureTerminalReady: emulator.ensureTerminalReady,
    flushPendingInput: inputQueue.flushPendingInput,
    markSyncedResize: resizeQueue.markSyncedResize,
    scheduleResize: resizeQueue.scheduleResize,
    selectSession: sessionsController.selectSession,
    stopControlQueues: () => {
      inputQueue.clearPendingInput();
      resizeQueue.resetResizeState();
    },
  });

  const targetEditor = useTerminalTargetEditor({
    cancelPendingSave: targetsController.cancelEdits,
    createTarget: targetsController.createTarget,
    updateTarget: async (targetId, payload, force, confirmationToken) => {
      const updated = await targetsController.updateTarget(
        targetId,
        payload,
        force,
        confirmationToken,
      );
      if (force) {
        const attachedSession = sessionsController.sessions.value.find(
          (session) => session.id === attachmentController.sessionId.value,
        );
        if (attachedSession?.targetId === targetId) {
          await sessionConnection.detach();
        }
        await sessionsController.loadSessions();
      }
      return updated;
    },
  });

  const selectedTargetActiveSessionCount = computed(() =>
    sessionsController.activeSessionCount(
      targetEditor.editingTarget.value?.id ??
        targetsController.selectedTargetId.value,
    ),
  );

  const targetDeletion = useTerminalTargetDeletion({
    activeSessionCount: sessionsController.activeSessionCount,
    attachedSessionId: attachmentController.sessionId,
    deleteTarget: targetsController.deleteTarget,
    detach: sessionConnection.detach,
    removeSessionsForTarget: sessionsController.removeSessionsForTarget,
    sessions: sessionsController.sessions,
    translate: (key, fallback) => t(key, fallback),
  });

  const presentation = useTerminalPresentation({
    activeAttachment: attachmentController.attachment,
    armedModifier: emulator.armedModifier,
    attachmentState: attachmentController.state,
    compactViewport: viewport.compactViewport,
    isTerminalFullscreen: viewport.isTerminalFullscreen,
    lastAttachmentError: attachmentController.lastError,
    pageError,
    readOnly: attachmentController.readOnly,
    selectedSession: sessionsController.selectedSession,
    selectedTarget: targetsController.selectedTarget,
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });

  const sessionActions = useTerminalSessionActions({
    beginTargetCreate: targetEditor.beginCreate,
    beginTargetEdit: targetEditor.beginEdit,
    connect: sessionConnection.connect,
    createSession: sessionsController.createSession,
    detach: sessionConnection.detach,
    endSession: sessionsController.endSession,
    getTerminalSize: emulator.getTerminalSize,
    isAttachedTo: attachmentController.isAttachedTo,
    onConnectStart: () => {
      pageError.value = "";
      runtimeRestarted.value = false;
    },
    reconnectAttachment: attachmentController.reconnect,
    selectedSession: sessionsController.selectedSession,
    selectedSessionId: sessionsController.selectedSessionId,
    selectedTarget: targetsController.selectedTarget,
    sessions: sessionsController.sessions,
    translate: (key) => t(key),
  });

  const selectTarget = (targetId: string) => {
    if (targetId === targetsController.selectedTargetId.value) {
      viewport.closeTargetDrawer();
      return;
    }
    sessionsController.cancelMutations();
    targetsController.selectTarget(targetId);
    viewport.closeTargetDrawer();
  };

  const refreshSessions = useTerminalSessionRefresh({
    attachmentSessionId: attachmentController.sessionId,
    connectToSession: sessionActions.connectToSession,
    detach: sessionConnection.detach,
    isDisposed: () => disposed,
    loadSessions: sessionsController.loadSessions,
    runtimeRestarted,
    selectedSession: sessionsController.selectedSession,
    sessionExists: (id) =>
      sessionsController.sessions.value.some((session) => session.id === id),
  });

  const interactions = useTerminalInteractions({
    activeAttachment: controllingAttachment,
    cancelRenameSession: sessionsController.cancelRename,
    emulator,
    inputQueue,
    isTerminalFullscreen: viewport.isTerminalFullscreen,
    renameSession: sessionsController.renameSession,
    selectedSession: sessionsController.selectedSession,
    sessions: sessionsController.sessions,
    setTerminalFullscreen: viewport.setTerminalFullscreen,
    translate: (key) => t(key),
  });

  const bootstrap = async () => {
    booting.value = true;
    pageError.value = "";
    try {
      await Promise.all([
        targetsController.loadTargets(),
        sessionsController.loadSessions(),
      ]);
      if (disposed) return;
      sessionsController.reconcileSelection();
      booting.value = false;
      await nextTick();
      await viewport.syncViewportHeight();
      const session = sessionsController.selectedSession.value;
      if (session) await sessionActions.connectToSession(session);
    } catch (reason) {
      if (disposed) return;
      pageError.value = extractTerminalErrorMessage(reason);
    } finally {
      if (!disposed) booting.value = false;
    }
  };

  onMounted(async () => {
    viewport.startViewportTracking();
    fontSize.loadTerminalFontSize();
    await bootstrap();
    if (disposed) return;
    interactions.start();
    sessionRefreshTimer = window.setInterval(() => {
      void refreshSessions().catch(() => undefined);
    }, 10_000);
  });

  watch(
    [
      () => sessionsController.sessions.value.length,
      presentation.connectionState,
      presentation.connectionError,
      viewport.showMobileAccessoryBar,
      viewport.isTerminalFullscreen,
      viewport.sidebarCollapsed,
    ],
    () => void nextTick().then(viewport.syncViewportHeight),
  );

  watch(sessionsController.selectedSession, (session) => {
    if (session) attachmentController.syncSession(session);
  });

  let targetSelectionGeneration = 0;
  watch(targetsController.selectedTargetId, async (targetId, previousId) => {
    if (disposed || booting.value || !previousId || targetId === previousId) {
      return;
    }
    const generation = ++targetSelectionGeneration;
    await sessionConnection.detach();
    if (generation !== targetSelectionGeneration) return;
    sessionsController.reconcileSelection();
    const session = sessionsController.selectedSession.value;
    if (!session || session.targetId !== targetId) return;
    await nextTick();
    if (generation !== targetSelectionGeneration) return;
    await sessionActions.connectToSession(session).catch((reason) => {
      pageError.value = extractTerminalErrorMessage(reason);
    });
  });

  onBeforeUnmount(() => {
    disposed = true;
    targetSelectionGeneration += 1;
    if (sessionRefreshTimer) window.clearInterval(sessionRefreshTimer);
    interactions.stop();
    viewport.stopViewportTracking();
    targetsController.dispose();
    targetDeletion.dispose();
    sessionsController.dispose();
    sessionConnection.dispose();
    emulator.dispose();
    void attachmentController.dispose();
  });

  const setTerminalFrameElement = (element: unknown) =>
    (viewport.terminalFrameRef.value = element as HTMLElement | null);
  const setTerminalMountElement = (element: unknown) =>
    (emulator.terminalMountRef.value = element as HTMLElement | null);

  return {
    ...interactions,
    ...presentation,
    ...sessionActions,
    ...targetDeletion,
    activeAttachment: attachmentController.attachment,
    armedModifier: emulator.armedModifier,
    canClaimControl: attachmentController.canClaimControl,
    claimControl: attachmentController.claimControl,
    isBooting: booting,
    isCreating: sessionsController.creating,
    isKilling: sessionsController.ending,
    isPinchZooming: emulator.isPinchZooming,
    isTerminalFullscreen: viewport.isTerminalFullscreen,
    nudgeTerminalFontSize: fontSize.nudgeTerminalFontSize,
    openTargetCreate: targetEditor.beginCreate,
    openTargetEdit: targetEditor.beginEdit,
    readOnly: attachmentController.readOnly,
    resetTerminalFontSize: fontSize.resetTerminalFontSize,
    runtimeRestarted,
    selectedSession: sessionsController.selectedSession,
    selectedSessionId: sessionsController.selectedSessionId,
    selectedTarget: targetsController.selectedTarget,
    selectedTargetActiveSessionCount,
    selectedTargetId: targetsController.selectedTargetId,
    selectTarget,
    sessions: sessionsController.sessions,
    sessionsForTarget: sessionsController.sessionsForTarget,
    setMobileAccessoryBarRef: viewport.setMobileAccessoryBarRef,
    setTargetDrawerOpen: viewport.setTargetDrawerOpen,
    setTerminalFrameElement,
    setTerminalMountElement,
    setTerminalPanelRef: viewport.setTerminalPanelRef,
    setTerminalShellRef: viewport.setTerminalShellRef,
    setTerminalStatusRef: viewport.setTerminalStatusRef,
    showMobileAccessoryBar: viewport.showMobileAccessoryBar,
    sidebarCollapsed: viewport.sidebarCollapsed,
    t,
    targetDrawerOpen: viewport.targetDrawerOpen,
    targetEditor,
    targets: targetsController.targets,
    targetsLoading: targetsController.loading,
    terminalFontSize: fontSize.terminalFontSize,
    terminalFrameStyle: viewport.terminalFrameStyle,
    terminalPanelClass: viewport.terminalPanelClass,
    terminalPanelStyle: viewport.terminalPanelStyle,
    toggleArmedModifier: emulator.toggleArmedModifier,
    toggleSidebar: viewport.toggleSidebar,
    toggleTerminalFullscreen: viewport.toggleTerminalFullscreen,
  };
};

export type WebTerminalPageController = ReturnType<typeof useWebTerminalPage>;
